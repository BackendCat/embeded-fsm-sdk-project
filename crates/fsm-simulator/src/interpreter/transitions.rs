//! Transition selection + execution — Doc 08 §4. Selection walks innermost-
//! first, one transition per region (B-11 collect-then-execute);
//! `execute_one_transition` drives the full exit-action-entry sequence
//! through the helpers in [`super::path`], [`super::run`], [`super::history`]
//! and [`super::timer`].

use std::collections::HashSet;

use fsm_ir::{TransitionKind, TransitionObject, Trigger};

use crate::eval::{eval_guard, execute_statements, EvalCtx, ExecOutcome, ExternRegistry, StmtCtx};
use crate::runtime::{effective_lca, EventKind, QueuedEvent, RuntimeState};

use super::history::record_history_before_exit;
use super::path::{entry_path, exit_set, expand_exit_with_parallel, expand_initial, is_leaflike};
use super::run::{resolve_target, run_entry, run_exit};
use super::timer::arm_timers_on_entry;
use super::StepError;

/// Doc 08 §4.1 — select transitions, innermost-first, one per region.
pub(super) fn select_transitions(
    rt: &RuntimeState,
    event: &QueuedEvent,
    externs: &ExternRegistry,
) -> Result<Vec<TransitionObject>, StepError> {
    let mut selected: Vec<TransitionObject> = Vec::new();
    let mut done: HashSet<String> = HashSet::new();

    // For timer fires, the transition is already chosen by id — find and
    // return it directly. (Skip the entire walk.)
    if let EventKind::TimerFire {
        transition_id,
        source_state,
        ..
    } = &event.kind
    {
        if let Some(node) = rt.machine.node(source_state) {
            if let Some(t) = node.transitions.iter().find(|t| &t.id == transition_id) {
                // Guard still must pass (timer transitions may have guards).
                // No guard ⇒ fire unconditionally; Ok(true) ⇒ fire; Ok(false)
                // ⇒ skip; Err ⇒ propagate as `StepError::GuardEval` (was
                // silently `false` pre-fix — audit P1-7 / D anti-pattern 1).
                let fire = match &t.guard {
                    None => true,
                    Some(g) => {
                        let evctx = EvalCtx {
                            context: &rt.context,
                            payload: rt.current_payload.as_ref(),
                            externs,
                        };
                        match eval_guard(g, &evctx) {
                            Ok(b) => b,
                            Err(e) => return Err(StepError::GuardEval(e)),
                        }
                    }
                };
                if fire {
                    selected.push(t.clone());
                }
            }
        }
        return Ok(selected);
    }

    // For Completion events, the trigger is `Completion { from }` — match
    // transitions on the just-completed state's ancestors.
    let event_target_id: Option<String> = match &event.kind {
        EventKind::Dispatched { event_id } | EventKind::Raised { event_id } => {
            Some(event_id.clone())
        }
        EventKind::Completion { .. } | EventKind::TimerFire { .. } => None,
    };
    let completion_from: Option<String> = match &event.kind {
        EventKind::Completion { state_id } => Some(state_id.clone()),
        _ => None,
    };

    // For each active leaf, walk ancestors innermost-first.
    let actives = rt.active_states.clone();
    for s in actives {
        // Skip if this leaf or any ancestor is already covered by a
        // previously-selected transition's exit set (B-11 collect phase —
        // one transition per region; parallel-state-level transitions cover
        // every region simultaneously).
        let ancestors_s = rt.machine.ancestors(&s);
        if ancestors_s.iter().any(|a| done.contains(a)) {
            continue;
        }
        let ancestors = ancestors_s;
        let mut chose: Option<TransitionObject> = None;
        for anc in &ancestors {
            let Some(node) = rt.machine.node(anc) else {
                continue;
            };
            // Collect candidate transitions whose trigger matches.
            let mut candidates: Vec<&TransitionObject> = Vec::new();
            for t in &node.transitions {
                if let Some(trig) = &t.trigger {
                    let matches_trigger = match (trig, &event_target_id, &completion_from) {
                        (Trigger::Event { event_id, .. }, Some(eid), _) => event_id == eid,
                        (Trigger::Completion { from }, _, Some(cid)) => {
                            from == cid || {
                                // Allow completion from any descendant of `from`.
                                let cid_ancestors = rt.machine.ancestors(cid);
                                cid_ancestors.iter().any(|a| a == from)
                            }
                        }
                        _ => false,
                    };
                    if !matches_trigger {
                        continue;
                    }
                } else {
                    // No trigger = completion transition. Doc 08 §4.4.
                    let Some(cid) = &completion_from else {
                        continue;
                    };
                    // Fire only when the source state matches the completed
                    // state (or its ancestor).
                    if &t.source != cid && {
                        let cid_anc = rt.machine.ancestors(cid);
                        !cid_anc.iter().any(|a| a == &t.source)
                    } {
                        continue;
                    }
                }
                if let Some(g) = &t.guard {
                    let evctx = EvalCtx {
                        context: &rt.context,
                        payload: rt.current_payload.as_ref(),
                        externs,
                    };
                    // Ok(true) ⇒ candidate; Ok(false) ⇒ skip; Err ⇒ propagate
                    // (was silently `false` pre-fix — audit P1-7).
                    match eval_guard(g, &evctx) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(e) => return Err(StepError::GuardEval(e)),
                    }
                }
                candidates.push(t);
            }
            if candidates.is_empty() {
                continue;
            }
            // Choose by (priority asc, document order asc).
            candidates.sort_by(|a, b| {
                a.priority
                    .cmp(&b.priority)
                    .then_with(|| std::cmp::Ordering::Equal)
            });
            chose = Some(candidates[0].clone());
            break;
        }
        if let Some(t) = chose {
            // Compute the exit set so other active leaves in the same region
            // don't double-select. For internal transitions, the exit set is
            // empty.
            if !matches!(t.kind, TransitionKind::Internal | TransitionKind::Local) {
                let lca = effective_lca(&t, &rt.machine);
                for ext in exit_set(&rt.machine, &t.source, &lca)? {
                    done.insert(ext);
                }
            }
            done.insert(t.source.clone());
            selected.push(t);
        }
    }
    Ok(selected)
}

/// Execute one transition's full exit / action / entry sequence.
pub(super) fn execute_one_transition(
    rt: &mut RuntimeState,
    t: &TransitionObject,
    exited_all: &mut Vec<String>,
    entered_all: &mut Vec<String>,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    // Internal transitions: just run actions, no exit / entry.
    if matches!(t.kind, TransitionKind::Internal) {
        let sctx = StmtCtx {
            machine: &rt.machine.clone(),
            externs,
            current_payload: rt.current_payload.clone(),
        };
        execute_statements(rt, &t.actions, &sctx, outcome)?;
        return Ok(());
    }

    let lca = effective_lca(t, &rt.machine);
    let exits = exit_set(&rt.machine, &t.source, &lca)?;

    // 1) Record history BEFORE running exit actions. Doc 08 §6.4.
    record_history_before_exit(rt, &exits);

    // 2) Compute the full set of states to exit, including any active
    //    descendants of `exits` (parallel children, composite leaves).
    //    Then order them innermost-first; for parallel-state exits the inner
    //    region leaves come BEFORE the parallel state itself, in reverse
    //    region-declaration order (Doc 08 §6.3).
    let mut full_exits: Vec<String> = Vec::new();
    let active_snapshot = rt.active_states.clone();
    for ex in &exits {
        // Include every active descendant of `ex` (active states whose
        // ancestor chain runs through `ex`).
        for a in &active_snapshot {
            if a == ex {
                continue;
            }
            let anc = rt.machine.ancestors(a);
            if anc.iter().any(|x| x == ex) && !full_exits.iter().any(|x| x == a) {
                full_exits.push(a.clone());
            }
        }
        if !full_exits.iter().any(|x| x == ex) {
            full_exits.push(ex.clone());
        }
    }

    for sid in expand_exit_with_parallel(&rt.machine.clone(), &full_exits) {
        run_exit(rt, &sid, outcome, externs)?;
        exited_all.push(sid);
    }
    // After exit, drop the affected active state(s).
    let exit_set_set: HashSet<&String> = exited_all.iter().collect();
    rt.active_states.retain(|s| !exit_set_set.contains(s));

    // 3) Transition action block.
    {
        let sctx = StmtCtx {
            machine: &rt.machine.clone(),
            externs,
            current_payload: rt.current_payload.clone(),
        };
        execute_statements(rt, &t.actions, &sctx, outcome)?;
    }

    // 4) Resolve the target (could be a pseudo-state — choice / junction /
    //    history / fork). Recurse through pseudo-states until we hit a basic /
    //    composite / parallel / final state.
    let resolved = resolve_target(rt, &t.target, externs, outcome)?;
    for tgt in resolved {
        // 5) Entry path from LCA down to `tgt`, then expand initial substates.
        let path = entry_path(&rt.machine.clone(), &lca, &tgt)?;
        for sid in &path {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        // 6) For composite / parallel / final targets, expand initial.
        let mut leaves = vec![tgt.clone()];
        let mut deeper: Vec<String> = Vec::new();
        expand_initial(&rt.machine.clone(), &tgt, &mut deeper);
        for sid in &deeper {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        leaves.extend(deeper);
        // 7) Update active_states with the leaf-most basic / final state.
        for leaf in &leaves {
            if is_leaflike(&rt.machine, leaf) {
                rt.active_states.push(leaf.clone());
            }
        }
        // 8) Re-arm timers in newly-entered states only (path + expanded).
        let now = rt.virtual_clock_ms;
        let mut seen_arm: HashSet<String> = HashSet::new();
        for sid in path.into_iter().chain(leaves.into_iter()) {
            if !seen_arm.insert(sid.clone()) {
                continue;
            }
            arm_timers_on_entry(&mut rt.timers, &rt.machine.clone(), &sid, now);
        }
    }
    Ok(())
}
