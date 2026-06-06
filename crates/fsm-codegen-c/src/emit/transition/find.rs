//! Pure IR-tree lookups used by the entry-chain emitter and the active-
//! descendant exit-set collector. Every function here is a recursive walk
//! of the IR `StateNode` tree returning a borrowed reference (or owned
//! `String` target id) — no codegen, no side effects.

use fsm_ir::StateNode;

pub(super) fn find_initial_target(machine: &fsm_ir::MachineObject, ir_id: &str) -> Option<String> {
    fn walk(states: &[StateNode], id: &str) -> Option<String> {
        for s in states {
            match s {
                StateNode::Initial(i) => {
                    if i.id == id {
                        return Some(i.target.clone());
                    }
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&machine.root.states, ir_id)
}

pub(super) fn find_composite<'a>(
    m: &'a fsm_ir::MachineObject,
    id: &str,
) -> Option<&'a fsm_ir::CompositeState> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a fsm_ir::CompositeState> {
        for s in states {
            match s {
                StateNode::Composite(c) => {
                    if c.id == id {
                        return Some(c);
                    }
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&m.root.states, id)
}

pub(super) fn find_parallel<'a>(
    m: &'a fsm_ir::MachineObject,
    id: &str,
) -> Option<&'a fsm_ir::ParallelState> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a fsm_ir::ParallelState> {
        for s in states {
            match s {
                StateNode::Parallel(p) => {
                    if p.id == id {
                        return Some(p);
                    }
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&m.root.states, id)
}
