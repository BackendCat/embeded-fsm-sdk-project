//! Parent-pointer table — Doc 00 §7.8 (B-10).
//!
//! Emitted as a `static const Motor_StateId_t Motor_parent_table[]`
//! indexed by `M_StateId_t`. The dispatch loop walks this table leaf-to-root
//! while attempting transitions in each ancestor.
//!
//! Build cost is O(N) where N is the state count. Lookup cost is O(1).

use crate::state_index::StateIndex;

/// Wrapper carrying the per-index parent values.
#[derive(Clone, Debug)]
pub struct ParentTable {
    /// One entry per state index in the [`StateIndex`]. `parents[0]` is
    /// always `ROOT_SENTINEL` (the root sentinel is its own parent — the
    /// ancestor walk in dispatch terminates by comparing against
    /// `ROOT_SENTINEL`, not by walking past it).
    pub parents: Vec<u8>,
}

/// Build the parent-pointer table for a machine's [`StateIndex`].
pub fn build_parent_table(index: &StateIndex) -> ParentTable {
    let parents = index.records.iter().map(|r| r.parent).collect();
    ParentTable { parents }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_index::build_state_index;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        CompositeState, ContextSchema, InitialPseudo, MachineObject, QueueConfig, RegionObject,
        SimpleState, StateNode,
    };

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    fn nested_machine() -> MachineObject {
        // Root → Operational (composite) → Running (simple)
        let running = StateNode::Simple(SimpleState {
            id: "s-running".into(),
            stable_id: "M:state:Running".into(),
            name: "Running".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            loc: loc(),
        });
        let operational = StateNode::Composite(CompositeState {
            id: "s-op".into(),
            stable_id: "M:state:Operational".into(),
            name: "Operational".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            regions: vec![RegionObject {
                id: "r-op".into(),
                stable_id: None,
                name: "Op".into(),
                initial: "ps-op-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-op-init".into(),
                        target: "s-running".into(),
                        loc: loc(),
                    }),
                    running,
                ],
                priority: 0,
                loc: loc(),
            }],
            history: None,
            loc: loc(),
        });
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
                initial: "ps-root-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-root-init".into(),
                        target: "s-op".into(),
                        loc: loc(),
                    }),
                    operational,
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
    fn parent_chain_walks_to_root() {
        let m = nested_machine();
        let idx = build_state_index(&m).expect("build_state_index");
        let pt = build_parent_table(&idx);
        let running_idx = idx.lookup("s-running").unwrap();
        let op_idx = idx.lookup("s-op").unwrap();
        // Running.parent == Operational
        assert_eq!(pt.parents[running_idx as usize], op_idx);
        // Operational.parent == ROOT
        assert_eq!(pt.parents[op_idx as usize], super::super::ROOT_SENTINEL);
    }
}
