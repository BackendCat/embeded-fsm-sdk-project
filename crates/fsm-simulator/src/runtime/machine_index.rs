//! Rich, simulator-specific machine index — extends `fsm_analyzer`'s
//! parent-only `MachineIndex` with the extra topology the RTC step needs.
//!
//! Beyond LCA, the interpreter must answer:
//! - "what is the active leaf of region R?" (Doc 08 §6.2, §11)
//! - "what is the kind / actions / regions of state X?" (every step)
//! - "what is the depth of X for innermost-first sorting?" (§6)
//! - "what initial state should a composite expand into?" (§7.3)
//! - "what timers belong to this state?" (§13)
//! - "what defers / transitions apply on this state?" (§4, §10)
//!
//! We flatten the IR machine tree into a `HashMap<StateId, Arc<NodeRef>>`
//! at construction time so every RTC step is O(1) lookups + O(depth) walks.

use std::collections::HashMap;
use std::sync::Arc;

use fsm_ir::{
    walk_state, DeferDecl, HistoryObject, IrVisitor, MachineObject, RegionObject, StateNode,
    TimerObject, TransitionObject,
};

/// One node in the flattened machine tree.
///
/// `parent` is the direct lexical parent — either a state ID (when the node
/// is a state-in-region whose region's parent state we surface) or a region
/// ID. For LCA computation we walk through `parent_state_or_region` which
/// alternates state-id ↔ region-id ↔ state-id ↔ ... up to the root region.
#[derive(Clone, Debug)]
pub struct NodeRef {
    pub id: String,
    pub stable_id: Option<String>,
    pub name: String,
    pub kind: NodeKind,
    /// Parent state ID. None for top-level states under the root region.
    pub parent_state: Option<String>,
    /// Containing region. None only for the root region's regional "parent",
    /// which does not exist (root region has no enclosing state).
    pub parent_region: Option<String>,
    /// Direct subtree information — populated only for composite / parallel.
    pub regions: Vec<String>,
    /// History pseudo-state on a composite (Doc 09 §4.2). None for other kinds.
    pub history: Option<HistoryObject>,
    /// Transitions declared on this state (every kind that has them).
    pub transitions: Vec<TransitionObject>,
    /// Timers belonging to this state.
    pub timers: Vec<TimerObject>,
    /// `defer EVENT` declarations on this state.
    pub defers: Vec<DeferDecl>,
    /// Entry / exit / completion actions (statement list).
    pub entry: Vec<fsm_ir::Statement>,
    pub exit: Vec<fsm_ir::Statement>,
}

/// State kind tag — discriminator over the IR's `StateNode` cases. Carries
/// per-kind data either inline (Initial, Choice, etc.) or via fields on
/// [`NodeRef`].
#[derive(Clone, Debug)]
pub enum NodeKind {
    Simple,
    Composite,
    Parallel,
    Initial {
        target: String,
    },
    Final,
    Choice {
        branches: Vec<fsm_ir::ChoiceBranch>,
    },
    Junction {
        branches: Vec<fsm_ir::ChoiceBranch>,
    },
    History {
        history: HistoryObject,
    },
    Fork {
        targets: Vec<String>,
    },
    Join {
        sources: Vec<String>,
        target: String,
        actions: Vec<fsm_ir::Statement>,
    },
    SubmachineRef,
    EntryPoint,
    ExitPoint,
}

#[derive(Clone, Debug)]
pub struct RegionRef {
    pub id: String,
    pub name: String,
    pub initial_pseudo: String,
    pub priority: u32,
    pub parent_state: Option<String>,
    pub child_state_ids: Vec<String>,
}

/// Flattened index over a single `MachineObject`. Built once per
/// `Interpreter::new`.
#[derive(Clone, Debug)]
pub struct MachineIndex {
    pub machine: Arc<MachineObject>,
    pub root_region_id: String,
    pub nodes: HashMap<String, Arc<NodeRef>>,
    pub regions: HashMap<String, Arc<RegionRef>>,
    /// Index of events declared on the machine, by id.
    pub events_by_id: HashMap<String, Arc<fsm_ir::EventObject>>,
    pub events_by_name: HashMap<String, String>, // name → id
}

/// R2.1 (2026-05-15): visitor that flattens a machine's region/state tree
/// into the simulator's lookup maps. Traversal lives in
/// `IrVisitor::walk_state` (fsm-ir); per-variant data extraction stays here.
///
/// The two parent-context fields (`parent_state_stack`,
/// `parent_region_stack`) are pushed/popped around region and state
/// descents respectively so any newly-recorded node sees the right
/// lexical ancestors. The previous shape passed them as plain arguments
/// to mutually-recursive free functions — the stacks encode the same
/// invariant on the visitor struct so we can ride the trait's recursion.
struct IndexBuilder<'a> {
    nodes: &'a mut HashMap<String, Arc<NodeRef>>,
    regions: &'a mut HashMap<String, Arc<RegionRef>>,
    /// Current containing-region id while walking states. Top entry is the
    /// region that owns the next `visit_state` call.
    parent_region_stack: Vec<String>,
    /// Current containing-state id while walking nested regions. `None` at
    /// the root region's level (root has no enclosing state); on entering
    /// a composite/parallel's regions we push that state's id.
    parent_state_stack: Vec<Option<String>>,
}

impl<'a> IrVisitor for IndexBuilder<'a> {
    fn visit_region(&mut self, r: &RegionObject) {
        let parent_state = self.parent_state_stack.last().cloned().unwrap_or(None);
        let child_ids: Vec<String> = r.states.iter().map(state_id).collect();
        let rref = RegionRef {
            id: r.id.clone(),
            name: r.name.clone(),
            initial_pseudo: r.initial.clone(),
            priority: r.priority,
            parent_state: parent_state.clone(),
            child_state_ids: child_ids,
        };
        self.regions.insert(r.id.clone(), Arc::new(rref));
        self.parent_region_stack.push(r.id.clone());
        for s in &r.states {
            self.visit_state(s);
        }
        self.parent_region_stack.pop();
    }

    fn visit_state(&mut self, s: &StateNode) {
        let parent_state = self.parent_state_stack.last().cloned().unwrap_or(None);
        let parent_region = self.parent_region_stack.last().cloned().unwrap_or_default();
        self.record_state(s, parent_state, &parent_region);
        // Descend into composite/parallel regions: push self as the
        // parent_state, recurse via the trait, pop. For non-region-bearing
        // variants `walk_state` is a no-op so the push/pop is cheap.
        if state_has_regions(s) {
            self.parent_state_stack.push(Some(state_id(s)));
            walk_state(self, s);
            self.parent_state_stack.pop();
        } else {
            walk_state(self, s);
        }
    }
}

fn state_has_regions(s: &StateNode) -> bool {
    matches!(s, StateNode::Composite(_) | StateNode::Parallel(_))
}

impl MachineIndex {
    pub fn build(m: Arc<MachineObject>) -> Self {
        let root_region_id = m.root.id.clone();
        let mut nodes: HashMap<String, Arc<NodeRef>> = HashMap::new();
        let mut regions: HashMap<String, Arc<RegionRef>> = HashMap::new();
        let mut builder = IndexBuilder {
            nodes: &mut nodes,
            regions: &mut regions,
            parent_region_stack: Vec::new(),
            parent_state_stack: vec![None],
        };
        builder.visit_region(&m.root);
        let events_by_id: HashMap<_, _> = m
            .events
            .iter()
            .map(|e| (e.id.clone(), Arc::new(e.clone())))
            .collect();
        let events_by_name: HashMap<_, _> = m
            .events
            .iter()
            .map(|e| (e.name.clone(), e.id.clone()))
            .collect();
        Self {
            machine: m,
            root_region_id,
            nodes,
            regions,
            events_by_id,
            events_by_name,
        }
    }
}

impl<'a> IndexBuilder<'a> {
    fn record_state(&mut self, s: &StateNode, parent_state: Option<String>, parent_region: &str) {
        let (id, name, stable, kind, regs, hist, trans, timers, defers, entry, exit) = match s {
            StateNode::Simple(s) => (
                s.id.clone(),
                s.name.clone(),
                Some(s.stable_id.clone()),
                NodeKind::Simple,
                vec![],
                None,
                s.transitions.clone(),
                s.timers.clone(),
                s.defers.clone(),
                s.entry.clone(),
                s.exit.clone(),
            ),
            StateNode::Composite(s) => (
                s.id.clone(),
                s.name.clone(),
                Some(s.stable_id.clone()),
                NodeKind::Composite,
                s.regions.iter().map(|r| r.id.clone()).collect(),
                s.history.clone(),
                s.transitions.clone(),
                s.timers.clone(),
                s.defers.clone(),
                s.entry.clone(),
                s.exit.clone(),
            ),
            StateNode::Parallel(s) => (
                s.id.clone(),
                s.name.clone(),
                Some(s.stable_id.clone()),
                NodeKind::Parallel,
                s.regions.iter().map(|r| r.id.clone()).collect(),
                None,
                s.transitions.clone(),
                s.timers.clone(),
                s.defers.clone(),
                s.entry.clone(),
                s.exit.clone(),
            ),
            StateNode::Initial(s) => (
                s.id.clone(),
                String::from("__initial__"),
                None,
                NodeKind::Initial {
                    target: s.target.clone(),
                },
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::Final(s) => (
                s.id.clone(),
                String::from("__final__"),
                Some(s.stable_id.clone()),
                NodeKind::Final,
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::Choice(s) => (
                s.id.clone(),
                String::from("__choice__"),
                Some(s.stable_id.clone()),
                NodeKind::Choice {
                    branches: s.branches.clone(),
                },
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::Junction(s) => (
                s.id.clone(),
                String::from("__junction__"),
                Some(s.stable_id.clone()),
                NodeKind::Junction {
                    branches: s.branches.clone(),
                },
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::History(s) => (
                s.id.clone(),
                String::from("__history__"),
                Some(s.stable_id.clone()),
                NodeKind::History { history: s.clone() },
                vec![],
                Some(s.clone()),
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::Fork(s) => (
                s.id.clone(),
                String::from("__fork__"),
                Some(s.stable_id.clone()),
                NodeKind::Fork {
                    targets: s.targets.clone(),
                },
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::Join(s) => (
                s.id.clone(),
                String::from("__join__"),
                Some(s.stable_id.clone()),
                NodeKind::Join {
                    sources: s.sources.clone(),
                    target: s.target.clone(),
                    actions: s.actions.clone(),
                },
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::Submachine(s) => (
                s.id.clone(),
                s.name.clone(),
                Some(s.stable_id.clone()),
                NodeKind::SubmachineRef,
                vec![],
                None,
                s.transitions.clone(),
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::EntryPoint(s) => (
                s.id.clone(),
                s.name.clone(),
                None,
                NodeKind::EntryPoint,
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            StateNode::ExitPoint(s) => (
                s.id.clone(),
                s.name.clone(),
                None,
                NodeKind::ExitPoint,
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
        };
        let nref = NodeRef {
            id: id.clone(),
            stable_id: stable,
            name,
            kind,
            parent_state,
            parent_region: Some(parent_region.to_string()),
            regions: regs,
            history: hist,
            transitions: trans,
            timers,
            defers,
            entry,
            exit,
        };
        self.nodes.insert(id, Arc::new(nref));
        // Region descent is now driven by `walk_state` via the IrVisitor
        // recursion — the visitor's `visit_region` is invoked for every
        // child region with the right parent-state pushed on the stack.
    }
}

impl MachineIndex {
    pub fn node(&self, id: &str) -> Option<&NodeRef> {
        self.nodes.get(id).map(|a| a.as_ref())
    }

    pub fn region(&self, id: &str) -> Option<&RegionRef> {
        self.regions.get(id).map(|a| a.as_ref())
    }

    /// Ordered ancestors of a state ID. The state itself is the first
    /// element. Walks state-parent links; intermediate regions appear as
    /// their own IDs. Terminates at the root region.
    pub fn ancestors(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = id.to_string();
        // Defensive cap — should never reach this on well-formed IR.
        for _ in 0..1024 {
            if out.iter().any(|x: &String| x == &cur) {
                break;
            }
            out.push(cur.clone());
            // If `cur` is a state, step to its parent_region; if it's a
            // region, step to its parent_state. Stop at root region.
            if let Some(node) = self.nodes.get(&cur) {
                if let Some(region) = &node.parent_region {
                    cur = region.clone();
                } else {
                    break;
                }
            } else if let Some(region) = self.regions.get(&cur) {
                if cur == self.root_region_id {
                    break;
                }
                match &region.parent_state {
                    Some(s) => cur = s.clone(),
                    None => break,
                }
            } else {
                break;
            }
        }
        out
    }

    /// Direct parent state of a state. Returns the containing composite /
    /// parallel state, skipping the intermediate region. For top-level
    /// states the result is `None` (they sit under the root region).
    pub fn parent_state(&self, id: &str) -> Option<&str> {
        self.nodes.get(id).and_then(|n| n.parent_state.as_deref())
    }

    /// Region that contains `state_id` directly.
    pub fn containing_region(&self, state_id: &str) -> Option<&str> {
        self.nodes
            .get(state_id)
            .and_then(|n| n.parent_region.as_deref())
    }

    /// Lookup an event by DSL-level name.
    pub fn event_id_by_name(&self, name: &str) -> Option<&str> {
        self.events_by_name.get(name).map(String::as_str)
    }

    pub fn event_name_by_id(&self, id: &str) -> Option<&str> {
        self.events_by_id.get(id).map(|e| e.name.as_str())
    }
}

fn state_id(s: &StateNode) -> String {
    match s {
        StateNode::Simple(s) => s.id.clone(),
        StateNode::Composite(s) => s.id.clone(),
        StateNode::Parallel(s) => s.id.clone(),
        StateNode::Initial(s) => s.id.clone(),
        StateNode::Final(s) => s.id.clone(),
        StateNode::Choice(s) => s.id.clone(),
        StateNode::Junction(s) => s.id.clone(),
        StateNode::History(s) => s.id.clone(),
        StateNode::Fork(s) => s.id.clone(),
        StateNode::Join(s) => s.id.clone(),
        StateNode::Submachine(s) => s.id.clone(),
        StateNode::EntryPoint(s) => s.id.clone(),
        StateNode::ExitPoint(s) => s.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    //! R2.1 behaviour-equivalence guards (2026-05-15).
    //!
    //! The IrVisitor-driven `MachineIndex::build` replaced the prior
    //! pair of mutually-recursive `record_region` / `record_state`
    //! free-function calls. These tests pin down the externally
    //! observable behaviour — parent-region wiring, parent-state
    //! threading across composite descents, region child-state ids —
    //! so any future regression in the visitor's stack discipline
    //! surfaces at unit-test time rather than via interpreter traces.
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{CompositeState, ContextSchema, InitialPseudo, QueueConfig, SimpleState};

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    /// One composite parent containing one inner region with one simple
    /// child — the smallest topology that exercises:
    /// - `parent_state_stack` push on composite descent (inner's
    ///   `parent_state` must be `s-outer`),
    /// - `parent_region_stack` push (inner's `parent_region` must be the
    ///   inner-region id),
    /// - region's `child_state_ids` order preserved (initial pseudo first,
    ///   then the simple state).
    #[test]
    fn composite_parent_state_and_region_threaded_through_visitor() {
        let machine = MachineObject {
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
        let idx = MachineIndex::build(Arc::new(machine));
        // Root region recorded; parent_state must be None.
        let r_root = idx.region("r-root").expect("root region registered");
        assert!(r_root.parent_state.is_none());
        // Inner region recorded with parent_state = Outer.
        let r_outer = idx.region("r-outer").expect("inner region registered");
        assert_eq!(r_outer.parent_state.as_deref(), Some("s-outer"));
        // Outer state's parent_state is None (under root), parent_region is r-root.
        let outer = idx.node("s-outer").expect("outer node registered");
        assert!(outer.parent_state.is_none());
        assert_eq!(outer.parent_region.as_deref(), Some("r-root"));
        // Inner state's parent_state is Outer, parent_region is r-outer.
        let inner = idx.node("s-inner").expect("inner node registered");
        assert_eq!(inner.parent_state.as_deref(), Some("s-outer"));
        assert_eq!(inner.parent_region.as_deref(), Some("r-outer"));
        // Region's child-state ids preserve document order.
        assert_eq!(
            r_outer.child_state_ids,
            vec!["ps-outer-init".to_string(), "s-inner".to_string()],
        );
    }
}
