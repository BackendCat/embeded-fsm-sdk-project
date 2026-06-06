//! Entry / exit execution sequencing — Doc 08 §6.2 / §7.2 — and target
//! resolution through pseudo-states (choice / junction / history / fork).
//!
//! The structural queries (entry path, exit set, expand initial) live in
//! [`super::path`]; this module *runs* the entry / exit actions and threads
//! pseudo-state evaluation into a concrete target list.

use std::collections::HashSet;

use crate::eval::{eval_guard, execute_statements, EvalCtx, ExecOutcome, ExternRegistry, StmtCtx};
use crate::runtime::{NodeKind, RuntimeState};

use super::path::{entry_path, expand_initial, is_leaflike};
use super::timer::arm_timers_on_entry;
use super::StepError;

pub(super) fn run_entry(
    rt: &mut RuntimeState,
    state_id: &str,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    let machine = rt.machine.clone();
    let stmts = machine
        .node(state_id)
        .map(|n| n.entry.clone())
        .unwrap_or_default();
    let sctx = StmtCtx {
        machine: &machine,
        externs,
        current_payload: rt.current_payload.clone(),
    };
    execute_statements(rt, &stmts, &sctx, outcome)?;
    Ok(())
}

pub(super) fn run_exit(
    rt: &mut RuntimeState,
    state_id: &str,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    let machine = rt.machine.clone();
    let stmts = machine
        .node(state_id)
        .map(|n| n.exit.clone())
        .unwrap_or_default();
    let sctx = StmtCtx {
        machine: &machine,
        externs,
        current_payload: rt.current_payload.clone(),
    };
    execute_statements(rt, &stmts, &sctx, outcome)?;
    // Cancel timers owned by this state per Doc 08 §13.2.
    rt.timers.cancel_owned_by(state_id);
    // Doc 08 §12.3 — parent-triggered exit of a submachine-ref state fully
    // exits (drops) its sub-instance. Done here, at the exit point, so a
    // *self-transition* on the ref-state (exit then re-enter in one step)
    // tears the stale sub down; the post-step `sync_submachines` then
    // re-instantiates a FRESH one on the re-entry (no stale sub-state
    // across an exit/enter cycle). Deterministic drop, no leak.
    if matches!(
        machine.node(state_id).map(|n| &n.kind),
        Some(NodeKind::SubmachineRef)
    ) {
        rt.submachines.remove(state_id);
    }
    Ok(())
}

/// Resolve a transition target through pseudo-states. Choice / Junction
/// branches are evaluated; History returns the recorded state or its default;
/// Fork returns multiple targets.
pub(super) fn resolve_target(
    rt: &mut RuntimeState,
    target: &str,
    externs: &ExternRegistry,
    outcome: &mut ExecOutcome,
) -> Result<Vec<String>, StepError> {
    let Some(node) = rt.machine.node(target) else {
        return Ok(vec![target.to_string()]);
    };
    match &node.kind {
        NodeKind::Choice { branches } | NodeKind::Junction { branches } => {
            // Pick the first branch whose guard evaluates true; `Else` wins
            // if no other branch matched.
            let mut pick: Option<fsm_ir::ChoiceBranch> = None;
            let evctx = EvalCtx {
                context: &rt.context,
                payload: rt.current_payload.as_ref(),
                externs,
            };
            let mut else_branch: Option<fsm_ir::ChoiceBranch> = None;
            for b in branches {
                if matches!(b.guard, fsm_ir::GuardExpr::Else) {
                    else_branch = Some(b.clone());
                    continue;
                }
                // Ok(true) ⇒ pick; Ok(false) ⇒ try next; Err ⇒ propagate
                // (was silently `false` pre-fix — audit P1-7).
                match eval_guard(&b.guard, &evctx) {
                    Ok(true) => {
                        pick = Some(b.clone());
                        break;
                    }
                    Ok(false) => continue,
                    Err(e) => return Err(StepError::GuardEval(e)),
                }
            }
            let branch = pick.or(else_branch).ok_or_else(|| {
                StepError::Internal(format!("choice {target} has no matching branch"))
            })?;
            // Run branch actions.
            let sctx = StmtCtx {
                machine: &rt.machine.clone(),
                externs,
                current_payload: rt.current_payload.clone(),
            };
            execute_statements(rt, &branch.actions, &sctx, outcome)?;
            resolve_target(rt, &branch.target, externs, outcome)
        }
        NodeKind::History { history } => {
            let saved = rt.history.get(&history.id).cloned();
            match saved {
                Some(path) if !path.is_empty() => Ok(path),
                _ => Ok(vec![history.default_target.clone()]),
            }
        }
        NodeKind::Fork { targets } => Ok(targets.clone()),
        NodeKind::Initial { target } => resolve_target(rt, &target.clone(), externs, outcome),
        _ => Ok(vec![target.to_string()]),
    }
}

/// Entry into one state at init time, following the entry sequence from
/// `lca` down to `target`. Used by [`super::Interpreter::init`].
pub(super) fn enter_state_path(
    rt: &mut RuntimeState,
    lca: &str,
    target: &str,
    entered_all: &mut Vec<String>,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    // Resolve pseudo-state targets (initial transitions can point at
    // composite states whose own initial expansion runs).
    let resolved = resolve_target(rt, target, externs, outcome)?;
    for tgt in resolved {
        let path = entry_path(&rt.machine.clone(), lca, &tgt)?;
        for sid in &path {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        let mut deeper: Vec<String> = Vec::new();
        expand_initial(&rt.machine.clone(), &tgt, &mut deeper);
        for sid in &deeper {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        let mut leaves = vec![tgt.clone()];
        leaves.extend(deeper);
        for leaf in &leaves {
            if is_leaflike(&rt.machine, leaf) {
                rt.active_states.push(leaf.clone());
            }
        }
        // Re-arm timers in the newly-entered states only.
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
