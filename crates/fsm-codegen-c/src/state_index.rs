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

use fsm_ir::{walk_state, IrVisitor, MachineObject, StateNode};

use crate::emit::EmitError;

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

    /// Resolve an IR id, falling back to the root sentinel index on a miss.
    ///
    /// **Audit P1-8 (2026-05-14)**: this used to `panic!` if a transition
    /// target's id was missing from the codegen index — turning a recoverable
    /// "analyzer should have caught this" invariant violation into a process
    /// abort with stderr backtrace. `fsm generate` now pre-flights every
    /// transition source/target through [`Self::lookup`] in
    /// `crate::emit::emit` and surfaces a missing id as
    /// [`EmitError::UnknownStateId`] *before* any emitter runs — so this
    /// fallback is unreachable in production. The `debug_assert!` keeps the
    /// invariant honest in debug builds. The fallback returns
    /// [`ROOT_SENTINEL`] (always valid), so even if a regression slipped
    /// through, the worst case is a malformed-but-non-panicking C99 emission
    /// instead of a CLI crash.
    pub fn must_lookup(&self, ir_id: &str) -> u8 {
        match self.lookup(ir_id) {
            Some(i) => i,
            None => {
                debug_assert!(
                    false,
                    "fsm-codegen-c: unindexed state id `{}` reached must_lookup — \
                     emit::pre_flight_validate should have caught this. \
                     (P1-8 invariant)",
                    ir_id,
                );
                ROOT_SENTINEL
            }
        }
    }

    /// Record for a given index. Cheap unchecked access.
    pub fn get(&self, idx: u8) -> &StateRecord {
        &self.records[idx as usize]
    }
}

/// Build a [`StateIndex`] for a single [`MachineObject`].
///
/// Returns [`EmitError::TooManyStates`] if the machine exceeds the v1.0
/// hardware cap of 255 indexed nodes (256 minus the root sentinel). Audit
/// P1-8 (2026-05-14): this used to `expect("…")` and abort the process —
/// `fsm generate` now propagates the error through and exits with code 2 +
/// a human-readable diagnostic instead.
pub fn build_state_index(machine: &MachineObject) -> Result<StateIndex, EmitError> {
    let mut builder = IndexBuilder {
        records: Vec::new(),
        by_ir_id: HashMap::new(),
        used_c_names: HashMap::new(),
        parent_stack: vec![ROOT_SENTINEL],
        error: None,
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
    })?;
    // The root region's "initial" is recorded on the root record once its
    // children are indexed.
    let root_initial_target = machine.root.initial.clone();
    // R2.1 (2026-05-15): traversal goes through `IrVisitor::walk_region` so
    // recursion structure lives in `fsm-ir` only. Per-state push logic
    // stays here; the parent index is threaded via a stack on the builder.
    builder.visit_region(&machine.root);
    if let Some(err) = builder.error.take() {
        return Err(err);
    }
    // Patch the root sentinel with its `initial_child` now that the target
    // is indexed.
    if let Some(idx) = builder.by_ir_id.get(&root_initial_target).copied() {
        builder.records[ROOT_SENTINEL as usize].initial_child = Some(idx);
    }
    Ok(StateIndex {
        records: builder.records,
        by_ir_id: builder.by_ir_id,
    })
}

struct IndexBuilder {
    records: Vec<StateRecord>,
    by_ir_id: HashMap<String, u8>,
    used_c_names: HashMap<String, u32>,
    /// Stack of parent indices threaded by the IrVisitor recursion. Pushed
    /// before descending into a composite/parallel state's regions, popped
    /// after. The top is the current parent for newly-pushed records.
    parent_stack: Vec<u8>,
    /// First error seen during the visitor walk. The visitor trait returns
    /// `()`, so we stash on the struct and short-circuit each visit.
    error: Option<EmitError>,
}

impl IndexBuilder {
    fn push(&mut self, mut rec: StateRecord) -> Result<u8, EmitError> {
        // De-duplicate C identifier names — the IR allows two states named
        // `Idle` in different regions, but the C enum cannot have two
        // members of the same name.
        let base = rec.c_name.clone();
        let count = self.used_c_names.entry(base.clone()).or_insert(0);
        if *count > 0 {
            rec.c_name = format!("{}_{}", base, count);
        }
        *count += 1;

        // Audit P1-8 (2026-05-14): the index width is `u8`. `expect("…")`
        // here used to abort `fsm generate` with a panic+backtrace when a
        // machine had >255 states. Now bubble up as `TooManyStates` so the
        // CLI exits 2 with a diagnostic.
        let idx = u8::try_from(self.records.len()).map_err(|_| EmitError::TooManyStates)?;
        self.by_ir_id.insert(rec.ir_id.clone(), idx);
        self.records.push(rec);
        Ok(idx)
    }

    fn current_parent(&self) -> u8 {
        *self.parent_stack.last().unwrap_or(&ROOT_SENTINEL)
    }
}

impl IrVisitor for IndexBuilder {
    fn visit_state(&mut self, s: &StateNode) {
        if self.error.is_some() {
            return;
        }
        let parent = self.current_parent();
        // The per-variant push is the only place codegen needs per-state
        // bookkeeping (initial-child + history patching for Composite).
        // R2.1: traversal — i.e. *which* states get visited — moves to
        // `walk_state` (fsm-ir/visitor.rs); the per-variant emit logic
        // stays here.
        match s {
            StateNode::Simple(ss) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident(&ss.name),
                    ir_id: ss.id.clone(),
                    dsl_name: ss.name.clone(),
                    parent,
                    kind: StateRecordKind::Simple,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Composite(c) => {
                let idx = match self.push(StateRecord {
                    c_name: c_ident(&c.name),
                    ir_id: c.id.clone(),
                    dsl_name: c.name.clone(),
                    parent,
                    kind: StateRecordKind::Composite,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    Ok(idx) => idx,
                    Err(e) => {
                        self.error = Some(e);
                        return;
                    }
                };
                self.parent_stack.push(idx);
                walk_state(self, s);
                self.parent_stack.pop();
                if self.error.is_some() {
                    return;
                }
                // After children are indexed, patch the initial child +
                // history pseudo if applicable.
                if let Some(region) = c.regions.first() {
                    if let Some(init_idx) = self.by_ir_id.get(&region.initial).copied() {
                        self.records[idx as usize].initial_child = Some(init_idx);
                    }
                }
                if let Some(h) = &c.history {
                    // The history pseudo-state is normally a member of the
                    // composite's inner region. Ensure it's indexed.
                    let h_idx = match self.by_ir_id.get(&h.id).copied() {
                        Some(i) => i,
                        None => match self.push(StateRecord {
                            c_name: c_ident(&format!("{}_History", c.name)),
                            ir_id: h.id.clone(),
                            dsl_name: format!("{}.History", c.name),
                            parent: idx,
                            kind: StateRecordKind::History,
                            initial_child: None,
                            history_pseudo: None,
                        }) {
                            Ok(i) => i,
                            Err(e) => {
                                self.error = Some(e);
                                return;
                            }
                        },
                    };
                    self.records[idx as usize].history_pseudo = Some(h_idx);
                }
            }
            StateNode::Parallel(p) => {
                let idx = match self.push(StateRecord {
                    c_name: c_ident(&p.name),
                    ir_id: p.id.clone(),
                    dsl_name: p.name.clone(),
                    parent,
                    kind: StateRecordKind::Parallel,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    Ok(idx) => idx,
                    Err(e) => {
                        self.error = Some(e);
                        return;
                    }
                };
                self.parent_stack.push(idx);
                walk_state(self, s);
                self.parent_stack.pop();
            }
            StateNode::Initial(i) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident("Initial"),
                    ir_id: i.id.clone(),
                    dsl_name: "<initial>".into(),
                    parent,
                    kind: StateRecordKind::Initial,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Final(f) => {
                // Prefer the DSL name (`final PaymentFinal` -> `PaymentFinal`).
                // Older fixtures that pre-date `FinalState.name` fall back to
                // a generic `Final` identifier, which is still unique-ified
                // by `IndexBuilder::push` (suffix `_N` on collision).
                let base = if f.name.is_empty() {
                    "Final"
                } else {
                    f.name.as_str()
                };
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident(base),
                    ir_id: f.id.clone(),
                    dsl_name: if f.name.is_empty() {
                        "<final>".into()
                    } else {
                        f.name.clone()
                    },
                    parent,
                    kind: StateRecordKind::Final,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Choice(c) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident("Choice"),
                    ir_id: c.id.clone(),
                    dsl_name: "<choice>".into(),
                    parent,
                    kind: StateRecordKind::Choice,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Junction(j) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident("Junction"),
                    ir_id: j.id.clone(),
                    dsl_name: "<junction>".into(),
                    parent,
                    kind: StateRecordKind::Junction,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::History(h) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident("History"),
                    ir_id: h.id.clone(),
                    dsl_name: "<history>".into(),
                    parent,
                    kind: StateRecordKind::History,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Fork(f) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident("Fork"),
                    ir_id: f.id.clone(),
                    dsl_name: "<fork>".into(),
                    parent,
                    kind: StateRecordKind::Fork,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Join(j) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident("Join"),
                    ir_id: j.id.clone(),
                    dsl_name: "<join>".into(),
                    parent,
                    kind: StateRecordKind::Join,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::Submachine(s) => {
                if let Err(e) = self.push(StateRecord {
                    c_name: c_ident(&s.name),
                    ir_id: s.id.clone(),
                    dsl_name: s.name.clone(),
                    parent,
                    kind: StateRecordKind::Submachine,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(e);
                }
            }
            StateNode::EntryPoint(e) => {
                if let Err(err) = self.push(StateRecord {
                    c_name: c_ident(&format!("EntryPoint_{}", e.name)),
                    ir_id: e.id.clone(),
                    dsl_name: e.name.clone(),
                    parent,
                    kind: StateRecordKind::EntryPoint,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(err);
                }
            }
            StateNode::ExitPoint(e) => {
                if let Err(err) = self.push(StateRecord {
                    c_name: c_ident(&format!("ExitPoint_{}", e.name)),
                    ir_id: e.id.clone(),
                    dsl_name: e.name.clone(),
                    parent,
                    kind: StateRecordKind::ExitPoint,
                    initial_child: None,
                    history_pseudo: None,
                }) {
                    self.error = Some(err);
                }
            }
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
        let idx = build_state_index(&flat_machine())
            .expect("build_state_index for flat machine should succeed");
        assert_eq!(idx.records[0].kind, StateRecordKind::Root);
        assert_eq!(idx.records[0].parent, ROOT_SENTINEL);
        // Initial pseudo of the root region populates `initial_child`.
        let init_idx = idx.lookup("ps-init").unwrap();
        assert_eq!(idx.records[0].initial_child, Some(init_idx));
    }

    #[test]
    fn simple_state_is_active_at_rest() {
        let idx = build_state_index(&flat_machine())
            .expect("build_state_index for flat machine should succeed");
        let s_idx = idx.lookup("s-idle").unwrap();
        assert_eq!(idx.get(s_idx).kind, StateRecordKind::Simple);
        assert!(idx.get(s_idx).kind.is_active_at_rest());
    }

    #[test]
    fn initial_pseudo_is_not_active_at_rest() {
        let idx = build_state_index(&flat_machine())
            .expect("build_state_index for flat machine should succeed");
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
        let idx =
            build_state_index(&m).expect("build_state_index for collision machine should succeed");
        let a = idx.lookup("s-idle-1").unwrap();
        let b = idx.lookup("s-idle-2").unwrap();
        assert_ne!(idx.get(a).c_name, idx.get(b).c_name);
    }

    /// R2.1 behaviour-equivalence guard (2026-05-15).
    ///
    /// Indexes a composite-with-inner-region fixture; verifies that
    /// (1) parents are threaded down by the IrVisitor `parent_stack`,
    /// (2) `initial_child` is patched after children index, and
    /// (3) document order is preserved.
    ///
    /// Pre-refactor this fixture exercised mutual recursion between
    /// `walk_region` and `walk_state`; post-refactor it exercises the
    /// IrVisitor's `walk_state` default and the stack-encoded parent.
    /// Any divergence in parent or ordering would be caught here.
    #[test]
    fn composite_initial_child_and_parent_thread_through_visitor() {
        use fsm_ir::{CompositeState, ContextSchema, QueueConfig, SimpleState};

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
                initial: "ps-root-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-root-init".into(),
                        target: "s-outer".into(),
                        loc: loc(),
                    }),
                    StateNode::Composite(CompositeState {
                        id: "s-outer".into(),
                        stable_id: "M:state:Outer".into(),
                        name: "Outer".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        regions: vec![RegionObject {
                            id: "r-outer".into(),
                            stable_id: None,
                            name: "__r_outer".into(),
                            initial: "ps-outer-init".into(),
                            states: vec![
                                StateNode::Initial(InitialPseudo {
                                    id: "ps-outer-init".into(),
                                    target: "s-inner".into(),
                                    loc: loc(),
                                }),
                                StateNode::Simple(SimpleState {
                                    id: "s-inner".into(),
                                    stable_id: "M:state:Inner".into(),
                                    name: "Inner".into(),
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
                        }],
                        history: None,
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
        let idx = build_state_index(&m).expect("composite fixture should index");
        // 5 records: root sentinel + ps-root-init + Outer + ps-outer-init + Inner
        assert_eq!(idx.count(), 5);
        let root = idx.get(ROOT_SENTINEL);
        assert_eq!(root.parent, ROOT_SENTINEL);
        let outer_idx = idx.lookup("s-outer").expect("outer indexed");
        let inner_idx = idx.lookup("s-inner").expect("inner indexed");
        // Outer's parent is the root sentinel.
        assert_eq!(idx.get(outer_idx).parent, ROOT_SENTINEL);
        // Inner's parent is Outer (parent threaded through visitor stack).
        assert_eq!(idx.get(inner_idx).parent, outer_idx);
        // The composite's initial_child is the inner-region's initial pseudo.
        let outer_init_idx = idx.lookup("ps-outer-init").unwrap();
        assert_eq!(idx.get(outer_idx).initial_child, Some(outer_init_idx));
        // Document order: ROOT, ps-root-init, Outer, ps-outer-init, Inner.
        assert_eq!(idx.records[0].kind, StateRecordKind::Root);
        assert_eq!(idx.records[1].kind, StateRecordKind::Initial);
        assert_eq!(idx.records[2].kind, StateRecordKind::Composite);
        assert_eq!(idx.records[3].kind, StateRecordKind::Initial);
        assert_eq!(idx.records[4].kind, StateRecordKind::Simple);
    }
}
