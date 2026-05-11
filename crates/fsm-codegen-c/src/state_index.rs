//! Canonical state indexing — assigns one `u8` per state node for use as the
//! `M_StateId_t` enum value and as parent-table / function-pointer-table
//! indices.
//!
//! The root-region itself gets index 0 (`ROOT`); every descendant state node
//! (simple, composite, parallel, history, choice, junction, final, etc.) is
//! assigned the next free index in **document order** (depth-first
//! left-to-right). This matches Doc 11 §4 naming determinism and gives the
//! parent-table walk a stable layout.
//!
//! Pseudo-states that are *not* runtime states (Initial, EntryPoint,
//! ExitPoint, Fork, Join) are deliberately included in the index — codegen
//! still needs to refer to them by index in the LCA + history paths — but
//! they are flagged as non-active so the runtime never observes them in
//! `m->_state`.

use std::collections::HashMap;

use fsm_ir::{MachineObject, RegionObject, StateNode};

/// The reserved root sentinel index. The parent of the root region's
/// immediate children. Used by the ancestor-walk dispatch to detect when
/// it has run out of ancestors.
pub const ROOT_SENTINEL: u8 = 0;

/// Per-state metadata produced by the indexer.
#[derive(Clone, Debug)]
pub struct StateRecord {
    /// Stable C-identifier name (uppercased, underscores, deduplicated).
    pub c_name: String,
    /// Original IR `id` string.
    pub ir_id: String,
    /// Original DSL name (used in comments).
    pub dsl_name: String,
    /// Parent index. `ROOT_SENTINEL` for the root region's direct children.
    pub parent: u8,
    /// Kind tag — picks which dispatch / entry / exit / history code paths
    /// the codegen emits for this state.
    pub kind: StateRecordKind,
    /// Default-target index when this state is the root of an active
    /// configuration that needs to expand into substates (initial pseudo of
    /// the containing region, or composite initial). `None` for leaves.
    pub initial_child: Option<u8>,
    /// For composite states with a declared `history`, the index of the
    /// history pseudo-state. Used by the deep-history / shallow-history
    /// emitter.
    pub history_pseudo: Option<u8>,
}

/// State kind tag — drives the codegen branch selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StateRecordKind {
    /// The synthetic root sentinel. Never appears as `m->_state`.
    Root,
    Simple,
    Composite,
    Parallel,
    Initial,
    Final,
    Choice,
    Junction,
    History,
    Fork,
    Join,
    Submachine,
    EntryPoint,
    ExitPoint,
}

impl StateRecordKind {
    /// `true` when an instance of this kind can be observed in
    /// `m->_state` at rest. Pseudo-states never appear at rest — they are
    /// always resolved during the same RTC step that targets them.
    pub fn is_active_at_rest(self) -> bool {
        matches!(
            self,
            StateRecordKind::Simple
                | StateRecordKind::Composite
                | StateRecordKind::Parallel
                | StateRecordKind::Final
                | StateRecordKind::Submachine
        )
    }
}

/// State indexing result.
#[derive(Clone, Debug)]
pub struct StateIndex {
    /// Records in canonical (index) order.
    pub records: Vec<StateRecord>,
    /// IR id → index lookup.
    pub by_ir_id: HashMap<String, u8>,
}

impl StateIndex {
    /// Number of indexed entries including the root sentinel.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// Resolve an IR id to its codegen index. Returns `None` if the id was
    /// never registered (e.g. a transition target that points outside the
    /// machine — the analyzer is responsible for catching that case).
    pub fn lookup(&self, ir_id: &str) -> Option<u8> {
        self.by_ir_id.get(ir_id).copied()
    }

    /// Resolve an IR id, panicking with a diagnostic message on miss. Used
    /// inside emit code where we've already validated via the analyzer that
    /// every referenced id exists.
    pub fn must_lookup(&self, ir_id: &str) -> u8 {
        self.lookup(ir_id).unwrap_or_else(|| {
            panic!(
                "fsm-codegen-c: unindexed state id `{}` — analyzer was supposed to catch this",
                ir_id
            )
        })
    }

    /// Record for a given index. Cheap unchecked access.
    pub fn get(&self, idx: u8) -> &StateRecord {
        &self.records[idx as usize]
    }
}

/// Build a [`StateIndex`] for a single [`MachineObject`].
pub fn build_state_index(machine: &MachineObject) -> StateIndex {
    let mut builder = IndexBuilder {
        records: Vec::new(),
        by_ir_id: HashMap::new(),
        used_c_names: HashMap::new(),
    };
    // Index 0 — synthetic root sentinel.
    builder.push(StateRecord {
        c_name: "ROOT".to_owned(),
        ir_id: format!("__root:{}", machine.id),
        dsl_name: "__root".to_owned(),
        parent: ROOT_SENTINEL,
        kind: StateRecordKind::Root,
        initial_child: None,
        history_pseudo: None,
    });
    // The root region's "initial" is recorded on the root record once its
    // children are indexed.
    let root_initial_target = machine.root.initial.clone();
    walk_region(&mut builder, &machine.root, ROOT_SENTINEL);
    // Patch the root sentinel with its `initial_child` now that the target
    // is indexed.
    if let Some(idx) = builder.by_ir_id.get(&root_initial_target).copied() {
        builder.records[ROOT_SENTINEL as usize].initial_child = Some(idx);
    }
    StateIndex {
        records: builder.records,
        by_ir_id: builder.by_ir_id,
    }
}

struct IndexBuilder {
    records: Vec<StateRecord>,
    by_ir_id: HashMap<String, u8>,
    used_c_names: HashMap<String, u32>,
}

impl IndexBuilder {
    fn push(&mut self, mut rec: StateRecord) -> u8 {
        // De-duplicate C identifier names — the IR allows two states named
        // `Idle` in different regions, but the C enum cannot have two
        // members of the same name.
        let base = rec.c_name.clone();
        let count = self.used_c_names.entry(base.clone()).or_insert(0);
        if *count > 0 {
            rec.c_name = format!("{}_{}", base, count);
        }
        *count += 1;

        let idx = u8::try_from(self.records.len()).expect(
            "fsm-codegen-c: state count exceeds 255; v1.0 codegen does not support that. \
             Increase the index type or split the machine.",
        );
        self.by_ir_id.insert(rec.ir_id.clone(), idx);
        self.records.push(rec);
        idx
    }
}

fn walk_region(b: &mut IndexBuilder, region: &RegionObject, parent: u8) {
    for state in &region.states {
        walk_state(b, state, parent);
    }
}

fn walk_state(b: &mut IndexBuilder, state: &StateNode, parent: u8) {
    match state {
        StateNode::Simple(s) => {
            b.push(StateRecord {
                c_name: c_ident(&s.name),
                ir_id: s.id.clone(),
                dsl_name: s.name.clone(),
                parent,
                kind: StateRecordKind::Simple,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Composite(c) => {
            let idx = b.push(StateRecord {
                c_name: c_ident(&c.name),
                ir_id: c.id.clone(),
                dsl_name: c.name.clone(),
                parent,
                kind: StateRecordKind::Composite,
                initial_child: None,
                history_pseudo: None,
            });
            for region in &c.regions {
                walk_region(b, region, idx);
            }
            // After children are indexed, patch the initial child + history
            // pseudo if applicable.
            if let Some(region) = c.regions.first() {
                if let Some(init_idx) = b.by_ir_id.get(&region.initial).copied() {
                    b.records[idx as usize].initial_child = Some(init_idx);
                }
            }
            if let Some(h) = &c.history {
                // The history pseudo-state is normally a member of the
                // composite's inner region. Ensure it's indexed.
                let h_idx = match b.by_ir_id.get(&h.id).copied() {
                    Some(i) => i,
                    None => b.push(StateRecord {
                        c_name: c_ident(&format!("{}_History", c.name)),
                        ir_id: h.id.clone(),
                        dsl_name: format!("{}.History", c.name),
                        parent: idx,
                        kind: StateRecordKind::History,
                        initial_child: None,
                        history_pseudo: None,
                    }),
                };
                b.records[idx as usize].history_pseudo = Some(h_idx);
            }
        }
        StateNode::Parallel(p) => {
            let idx = b.push(StateRecord {
                c_name: c_ident(&p.name),
                ir_id: p.id.clone(),
                dsl_name: p.name.clone(),
                parent,
                kind: StateRecordKind::Parallel,
                initial_child: None,
                history_pseudo: None,
            });
            for region in &p.regions {
                walk_region(b, region, idx);
            }
        }
        StateNode::Initial(i) => {
            b.push(StateRecord {
                c_name: c_ident("Initial"),
                ir_id: i.id.clone(),
                dsl_name: "<initial>".into(),
                parent,
                kind: StateRecordKind::Initial,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Final(f) => {
            b.push(StateRecord {
                c_name: c_ident("Final"),
                ir_id: f.id.clone(),
                dsl_name: "<final>".into(),
                parent,
                kind: StateRecordKind::Final,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Choice(c) => {
            b.push(StateRecord {
                c_name: c_ident("Choice"),
                ir_id: c.id.clone(),
                dsl_name: "<choice>".into(),
                parent,
                kind: StateRecordKind::Choice,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Junction(j) => {
            b.push(StateRecord {
                c_name: c_ident("Junction"),
                ir_id: j.id.clone(),
                dsl_name: "<junction>".into(),
                parent,
                kind: StateRecordKind::Junction,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::History(h) => {
            b.push(StateRecord {
                c_name: c_ident("History"),
                ir_id: h.id.clone(),
                dsl_name: "<history>".into(),
                parent,
                kind: StateRecordKind::History,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Fork(f) => {
            b.push(StateRecord {
                c_name: c_ident("Fork"),
                ir_id: f.id.clone(),
                dsl_name: "<fork>".into(),
                parent,
                kind: StateRecordKind::Fork,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Join(j) => {
            b.push(StateRecord {
                c_name: c_ident("Join"),
                ir_id: j.id.clone(),
                dsl_name: "<join>".into(),
                parent,
                kind: StateRecordKind::Join,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::Submachine(s) => {
            b.push(StateRecord {
                c_name: c_ident(&s.name),
                ir_id: s.id.clone(),
                dsl_name: s.name.clone(),
                parent,
                kind: StateRecordKind::Submachine,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::EntryPoint(e) => {
            b.push(StateRecord {
                c_name: c_ident(&format!("EntryPoint_{}", e.name)),
                ir_id: e.id.clone(),
                dsl_name: e.name.clone(),
                parent,
                kind: StateRecordKind::EntryPoint,
                initial_child: None,
                history_pseudo: None,
            });
        }
        StateNode::ExitPoint(e) => {
            b.push(StateRecord {
                c_name: c_ident(&format!("ExitPoint_{}", e.name)),
                ir_id: e.id.clone(),
                dsl_name: e.name.clone(),
                parent,
                kind: StateRecordKind::ExitPoint,
                initial_child: None,
                history_pseudo: None,
            });
        }
    }
}

/// Convert a DSL name into a C-safe identifier fragment. Per Doc 11 §4:
/// uppercase, non-alphanumeric → `_`, consecutive underscores collapsed.
pub fn c_ident(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    // Strip leading/trailing underscores so `__state` and `state__` don't
    // produce ugly enum names.
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "STATE".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{ContextSchema, InitialPseudo, QueueConfig, RegionObject, SimpleState, StateNode};

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    fn flat_machine() -> MachineObject {
        MachineObject {
            id: "m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-init".into(),
                        target: "s-idle".into(),
                        loc: loc(),
                    }),
                    StateNode::Simple(SimpleState {
                        id: "s-idle".into(),
                        stable_id: "M:state:Idle".into(),
                        name: "Idle".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        loc: loc(),
                    }),
                ],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig::default(),
            targets: vec![],
            loc: loc(),
        }
    }

    #[test]
    fn root_sentinel_at_index_zero() {
        let idx = build_state_index(&flat_machine());
        assert_eq!(idx.records[0].kind, StateRecordKind::Root);
        assert_eq!(idx.records[0].parent, ROOT_SENTINEL);
        // Initial pseudo of the root region populates `initial_child`.
        let init_idx = idx.lookup("ps-init").unwrap();
        assert_eq!(idx.records[0].initial_child, Some(init_idx));
    }

    #[test]
    fn simple_state_is_active_at_rest() {
        let idx = build_state_index(&flat_machine());
        let s_idx = idx.lookup("s-idle").unwrap();
        assert_eq!(idx.get(s_idx).kind, StateRecordKind::Simple);
        assert!(idx.get(s_idx).kind.is_active_at_rest());
    }

    #[test]
    fn initial_pseudo_is_not_active_at_rest() {
        let idx = build_state_index(&flat_machine());
        let p_idx = idx.lookup("ps-init").unwrap();
        assert!(!idx.get(p_idx).kind.is_active_at_rest());
    }

    #[test]
    fn c_ident_handles_nested_dotted_names() {
        // Doc 11 §4 example: `Running.Normal` → `RUNNING_NORMAL`.
        assert_eq!(c_ident("Running.Normal"), "RUNNING_NORMAL");
        assert_eq!(c_ident("Hb-Active"), "HB_ACTIVE");
        assert_eq!(c_ident("__weird_"), "WEIRD");
    }

    #[test]
    fn c_ident_collapses_consecutive_separators() {
        assert_eq!(c_ident("A--B..C"), "A_B_C");
    }

    #[test]
    fn duplicate_state_names_are_uniquified() {
        // Build a machine with two simple states both named "Idle" — the
        // index MUST give them distinct C identifiers.
        let m = MachineObject {
            id: "m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "s-idle-1".into(),
                states: vec![
                    StateNode::Simple(SimpleState {
                        id: "s-idle-1".into(),
                        stable_id: "M:state:Idle1".into(),
                        name: "Idle".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        loc: loc(),
                    }),
                    StateNode::Simple(SimpleState {
                        id: "s-idle-2".into(),
                        stable_id: "M:state:Idle2".into(),
                        name: "Idle".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        loc: loc(),
                    }),
                ],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig::default(),
            targets: vec![],
            loc: loc(),
        };
        let idx = build_state_index(&m);
        let a = idx.lookup("s-idle-1").unwrap();
        let b = idx.lookup("s-idle-2").unwrap();
        assert_ne!(idx.get(a).c_name, idx.get(b).c_name);
    }
}
