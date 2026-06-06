//! History pseudo-state recording — Doc 08 §6.4 (record before exit) and
//! §8 (shallow / deep semantics). Restoring a saved history value happens
//! during target resolution in [`super::run::resolve_target`].

use fsm_ir::HistoryKind;

use crate::runtime::RuntimeState;

use super::path::{direct_child_of, find_active_in_subtree};

/// Doc 08 §6.4 — record history just before exit. For each exiting state,
/// if any ancestor composite has a `history` pseudo-state, record the leaf
/// (shallow) or the deeper path (deep).
pub(super) fn record_history_before_exit(rt: &mut RuntimeState, exits: &[String]) {
    // Walk every composite that is being exited and check whether it has a
    // history pseudo-state.
    for ex in exits {
        if let Some(node) = rt.machine.node(ex) {
            if let Some(h) = &node.history {
                let active_in_subtree = find_active_in_subtree(rt, ex);
                match h.history_kind {
                    HistoryKind::Shallow => {
                        // Direct child of `ex`.
                        if let Some(leaf) = active_in_subtree.first() {
                            // Walk back up from leaf to the direct child of `ex`.
                            let direct = direct_child_of(&rt.machine, ex, leaf);
                            rt.history.insert(h.id.clone(), vec![direct]);
                        }
                    }
                    HistoryKind::Deep => {
                        // Full active descendant set.
                        rt.history.insert(h.id.clone(), active_in_subtree);
                    }
                }
            }
        }
    }
}
