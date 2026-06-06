//! Timer arming — Doc 08 §13.1 (start condition on entry). Cancellation
//! lives in [`super::run::run_exit`] (Doc 08 §13.2: cancel-on-exit).

use fsm_ir::{TimerKind, Trigger};

use crate::runtime::{MachineIndex, Timer, TimerFire, TimerSet};

pub(super) fn arm_timers_on_entry(
    timers: &mut TimerSet,
    idx: &MachineIndex,
    state: &str,
    now: u64,
) {
    let Some(node) = idx.node(state) else { return };
    for t in &node.timers {
        let fire = match t.kind {
            TimerKind::After => TimerFire::OneShot,
            TimerKind::Every | TimerKind::EveryInternal => TimerFire::Periodic {
                period_ms: t.duration_ms,
            },
        };
        // P0-4: link timer to its transition by `timer_id` (set by analyzer
        // in `Trigger::After { timer_id }` / `Trigger::Every { timer_id }`).
        // Fall back to legacy (source + target) match for IR docs produced
        // by older test fixtures that don't supply timer_id.
        let transition_id = node.transitions.iter().find_map(|tr| match &tr.trigger {
            Some(Trigger::After { timer_id, .. }) | Some(Trigger::Every { timer_id, .. })
                if !timer_id.is_empty() =>
            {
                if timer_id == &t.id {
                    Some(tr.id.clone())
                } else {
                    None
                }
            }
            Some(Trigger::After { .. }) | Some(Trigger::Every { .. }) => {
                if tr.source == state && Some(tr.target.clone()) == t.target.clone() {
                    Some(tr.id.clone())
                } else {
                    None
                }
            }
            _ => None,
        });
        timers.arm(Timer {
            timer_id: t.id.clone(),
            source_state: state.to_string(),
            transition_id,
            expiry_ms: now + t.duration_ms as u64,
            fires: fire,
        });
    }
}
