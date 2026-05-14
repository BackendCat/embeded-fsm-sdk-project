//! Region layout — assigns each state to an `_active[]` slot.
//!
//! Doc 00 §7.8 (B-11 collect-then-execute) and Doc 08 §2.3 (active
//! configuration representation) require the C runtime to carry one
//! `StateId_t` per active region. We replace the legacy single `_state`
//! field with a uniform `_active[MOTOR_MAX_PARALLEL_REGIONS]` array; slot 0
//! is the singleton/non-parallel path, slots 1..N are per-region leaves
//! inside parallel states.
//!
//! `MAX_PARALLEL_REGIONS` is computed conservatively as the worst-case
//! number of simultaneously active leaves at rest: a static IR analysis
//! that returns `1` for flat/composite-only machines and the sum-of-regions
//! for any parallel state on the way to a leaf.
//!
//! The mapping is total: every state index (including pseudo-states) has a
//! slot. For pseudo-states the value is informational — they never appear
//! at rest in `_active[]` — but the codegen still consults the slot during
//! transition target resolution.

use fsm_ir::{MachineObject, RegionObject, StateNode};

use crate::state_index::StateIndex;

/// Per-state region assignment metadata.
#[derive(Clone, Debug)]
pub struct RegionLayout {
    /// One slot per state index. `slot_of[i]` is the `_active[]` index a
    /// leaf-active occurrence of state `i` lands in.
    /// - Slot 0 holds the singleton non-parallel leaf.
    /// - Slots 1..N hold per-region leaves inside parallel states.
    pub slot_of: Vec<u8>,
    /// Number of `_active[]` slots the machine needs. Always >= 1.
    pub max_parallel_regions: u8,
}

impl RegionLayout {
    /// Slot the live leaf for `state_idx` is stored in.
    pub fn slot(&self, state_idx: u8) -> u8 {
        self.slot_of.get(state_idx as usize).copied().unwrap_or(0)
    }
}

/// Build a [`RegionLayout`] for a machine.
///
/// Algorithm: depth-first walk of the IR. The initial "primary" slot is 0.
/// When entering a parallel state, region 0 shares its parent's slot; the
/// remaining regions take fresh slots from a running counter (1, 2, …).
/// Nested parallels recurse with their own running counter so every
/// region in the whole IR has a unique slot. The size of `_active[]` is
/// the maximum number of simultaneously active leaves at rest, which is
/// also the maximum slot-index + 1 (see [`compute_max_active_leaves`]).
pub fn build_region_layout(machine: &MachineObject, index: &StateIndex) -> RegionLayout {
    let mut slot_of = vec![0u8; index.count()];
    let mut next_slot: u8 = 1;

    fn walk_region(
        region: &RegionObject,
        my_slot: u8,
        slot_of: &mut [u8],
        next_slot: &mut u8,
        index: &StateIndex,
    ) {
        for state in &region.states {
            walk_state(state, my_slot, slot_of, next_slot, index);
        }
    }

    fn walk_state(
        state: &StateNode,
        my_slot: u8,
        slot_of: &mut [u8],
        next_slot: &mut u8,
        index: &StateIndex,
    ) {
        if let Some(idx) = state_ir_id(state).and_then(|s| index.lookup(s)) {
            slot_of[idx as usize] = my_slot;
        }
        match state {
            StateNode::Composite(c) => {
                for r in &c.regions {
                    walk_region(r, my_slot, slot_of, next_slot, index);
                }
            }
            StateNode::Parallel(p) => {
                // Region 0 of the parallel SHARES its parent's slot (the
                // parallel's own slot). Subsequent regions take fresh
                // slots from the running counter.
                for (region_idx, region) in p.regions.iter().enumerate() {
                    let region_slot = if region_idx == 0 {
                        my_slot
                    } else {
                        let s = *next_slot;
                        *next_slot = next_slot.checked_add(1).expect(
                            "fsm-codegen-c: parallel region count exceeds u8; v1.0 does not support that many",
                        );
                        s
                    };
                    walk_region(region, region_slot, slot_of, next_slot, index);
                }
            }
            _ => {}
        }
    }

    for state in &machine.root.states {
        walk_state(state, 0, &mut slot_of, &mut next_slot, index);
    }

    let max_parallel_regions = compute_max_active_leaves(&machine.root).max(1);

    RegionLayout {
        slot_of,
        max_parallel_regions,
    }
}

fn state_ir_id(state: &StateNode) -> Option<&str> {
    match state {
        StateNode::Simple(s) => Some(&s.id),
        StateNode::Composite(c) => Some(&c.id),
        StateNode::Parallel(p) => Some(&p.id),
        StateNode::Initial(i) => Some(&i.id),
        StateNode::Final(f) => Some(&f.id),
        StateNode::Choice(c) => Some(&c.id),
        StateNode::Junction(j) => Some(&j.id),
        StateNode::History(h) => Some(&h.id),
        StateNode::Fork(f) => Some(&f.id),
        StateNode::Join(j) => Some(&j.id),
        StateNode::Submachine(s) => Some(&s.id),
        StateNode::EntryPoint(e) => Some(&e.id),
        StateNode::ExitPoint(e) => Some(&e.id),
    }
}

/// Compute the maximum number of simultaneously active leaves a region can
/// hold. Used to size `MAX_PARALLEL_REGIONS`. Definition:
/// - A region's count = max over its child states' counts (only one state
///   in a region is active at a time at the region's leaf level).
/// - A simple/final/leaf state contributes 1.
/// - A composite contributes its inner region's count.
/// - A parallel contributes the SUM of its regions' counts.
fn compute_max_active_leaves(region: &RegionObject) -> u8 {
    let mut best: u8 = 1;
    for s in &region.states {
        let n = state_max_active_leaves(s);
        if n > best {
            best = n;
        }
    }
    best
}

fn state_max_active_leaves(state: &StateNode) -> u8 {
    match state {
        StateNode::Simple(_) | StateNode::Final(_) | StateNode::Submachine(_) => 1,
        StateNode::Composite(c) => c
            .regions
            .iter()
            .map(compute_max_active_leaves)
            .max()
            .unwrap_or(1),
        StateNode::Parallel(p) => p
            .regions
            .iter()
            .map(compute_max_active_leaves)
            .sum::<u8>()
            .max(1),
        // Pseudo-states never appear at rest.
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        ContextSchema, FinalState, InitialPseudo, MachineObject, ParallelState, QueueConfig,
        RegionObject, SimpleState, StateNode,
    };

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    fn flat() -> MachineObject {
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
                        stable_id: "M:Idle".into(),
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

    fn parallel_two_regions() -> MachineObject {
        let region_a = RegionObject {
            id: "r-a".into(),
            stable_id: None,
            name: "RA".into(),
            initial: "ps-a-init".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-a-init".into(),
                    target: "s-a".into(),
                    loc: loc(),
                }),
                StateNode::Simple(SimpleState {
                    id: "s-a".into(),
                    stable_id: "M:A".into(),
                    name: "A".into(),
                    entry: vec![],
                    exit: vec![],
                    transitions: vec![],
                    timers: vec![],
                    defers: vec![],
                    loc: loc(),
                }),
                StateNode::Final(FinalState {
                    id: "s-a-final".into(),
                    stable_id: "M:AFinal".into(),
                    name: "AFinal".into(),
                    loc: loc(),
                }),
            ],
            priority: 0,
            loc: loc(),
        };
        let region_b = RegionObject {
            id: "r-b".into(),
            stable_id: None,
            name: "RB".into(),
            initial: "ps-b-init".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-b-init".into(),
                    target: "s-b".into(),
                    loc: loc(),
                }),
                StateNode::Simple(SimpleState {
                    id: "s-b".into(),
                    stable_id: "M:B".into(),
                    name: "B".into(),
                    entry: vec![],
                    exit: vec![],
                    transitions: vec![],
                    timers: vec![],
                    defers: vec![],
                    loc: loc(),
                }),
                StateNode::Final(FinalState {
                    id: "s-b-final".into(),
                    stable_id: "M:BFinal".into(),
                    name: "BFinal".into(),
                    loc: loc(),
                }),
            ],
            priority: 1,
            loc: loc(),
        };
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
                        target: "s-par".into(),
                        loc: loc(),
                    }),
                    StateNode::Parallel(ParallelState {
                        id: "s-par".into(),
                        stable_id: "M:Par".into(),
                        name: "Par".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        regions: vec![region_a, region_b],
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
    fn flat_machine_max_regions_is_one() {
        let m = flat();
        let idx = crate::state_index::build_state_index(&m);
        let layout = build_region_layout(&m, &idx);
        assert_eq!(layout.max_parallel_regions, 1);
        let idle = idx.lookup("s-idle").unwrap();
        assert_eq!(layout.slot(idle), 0);
    }

    #[test]
    fn parallel_two_regions_gives_two_slots() {
        let m = parallel_two_regions();
        let idx = crate::state_index::build_state_index(&m);
        let layout = build_region_layout(&m, &idx);
        // Two regions → two slots total (region 0 shares the parent's
        // slot 0, region 1 takes slot 1).
        assert_eq!(layout.max_parallel_regions, 2);
        let a = idx.lookup("s-a").unwrap();
        let b = idx.lookup("s-b").unwrap();
        let par = idx.lookup("s-par").unwrap();
        // Region 0's leaf shares slot 0 with the parallel; region 1 has
        // its own slot.
        assert_eq!(layout.slot(par), 0);
        assert_eq!(layout.slot(a), 0);
        assert_eq!(layout.slot(b), 1);
    }
}
