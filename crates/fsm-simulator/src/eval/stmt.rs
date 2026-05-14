//! Execute one [`Statement`] block — Doc 09 §8.
//!
//! Statements mutate the runtime context, enqueue events, or invoke externs.
//! The action emitter MUST be deterministic: the implementation does not
//! introspect iteration order of `HashMap`, all loops over statements use
//! `Vec`.

use std::collections::HashMap;

use fsm_ir::{Expr, FieldRef, Statement};
use thiserror::Error;

use super::expr::{eval_expr, EvalCtx};
use crate::runtime::event::{EventKind, QueuedEvent};
use crate::runtime::queue::QueueError;
use crate::runtime::state::RuntimeState;
use crate::runtime::value::Value;
use crate::runtime::MachineIndex;

#[derive(Debug, Error)]
pub enum StmtError {
    #[error("evaluation: {0}")]
    Eval(#[from] super::expr::EvalError),
    #[error("queue: {0}")]
    Queue(#[from] QueueError),
    #[error("send: cross-machine target not supported in v1.0")]
    SendUnsupported,
    #[error("payload mutation is not permitted (payload is read-only)")]
    PayloadAssign,
    #[error("loop iteration limit reached")]
    LoopLimit,
    /// Defer statement encountered outside a state context.
    #[error("`defer` outside a stateful context")]
    DeferOutsideState,
    /// Trace of executed action names, for [`StepRecord.actionsExecuted`].
    /// Not really an error — kept for symmetry; statement execution returns
    /// an `Ok(Vec<String>)` with the list of action names.
    #[error("internal: not an error sentinel")]
    Sentinel,
}

/// Hard cap on iterations of a single `while` / `for` to keep an ill-formed
/// IR from hanging the interpreter.
const LOOP_ITER_LIMIT: usize = 10_000;

/// Record of side-effects produced by executing a statement block — used to
/// populate [`crate::trace::StepRecord`].
#[derive(Debug, Default, Clone)]
pub struct ExecOutcome {
    /// Names of every `call(extern, args)` invoked. Matches Doc 13 §11
    /// `actionsExecuted: ["Motor_action_startMotor"]`.
    pub actions_executed: Vec<String>,
}

/// Mutable evaluator context — holds both shared (`externs`) and mutable
/// state (`context`). Implements `EvalCtx` accessor on demand.
pub struct StmtCtx<'a> {
    pub machine: &'a MachineIndex,
    pub externs: &'a super::extern_registry::ExternRegistry,
    pub current_payload: Option<HashMap<String, Value>>,
}

/// Run a sequence of statements in order. The interpreter passes the live
/// [`RuntimeState`] so each statement can mutate `context`, push to the
/// queue, or record a defer.
pub fn execute_statements(
    rt: &mut RuntimeState,
    stmts: &[Statement],
    sctx: &StmtCtx,
    outcome: &mut ExecOutcome,
) -> Result<(), StmtError> {
    for s in stmts {
        execute_statement(rt, s, sctx, outcome)?;
    }
    Ok(())
}

pub fn execute_statement(
    rt: &mut RuntimeState,
    stmt: &Statement,
    sctx: &StmtCtx,
    outcome: &mut ExecOutcome,
) -> Result<(), StmtError> {
    match stmt {
        Statement::Assign { target, value } => {
            let v = eval_with(value, rt, sctx)?;
            match target {
                FieldRef::Ctx { field } => {
                    // Coerce to the declared field type so context values
                    // preserve their declared width (mirrors C99 store).
                    let coerced = coerce_to_field(rt, field, v);
                    rt.context.insert(field.clone(), coerced);
                }
                FieldRef::Payload { .. } => return Err(StmtError::PayloadAssign),
            }
        }
        Statement::If {
            condition,
            then,
            else_,
        } => {
            let v = eval_with(condition, rt, sctx)?;
            let branch = if v.as_bool() { then } else { else_ };
            execute_statements(rt, branch, sctx, outcome)?;
        }
        Statement::While { condition, body } => {
            for i in 0.. {
                if i >= LOOP_ITER_LIMIT {
                    return Err(StmtError::LoopLimit);
                }
                let v = eval_with(condition, rt, sctx)?;
                if !v.as_bool() {
                    break;
                }
                execute_statements(rt, body, sctx, outcome)?;
            }
        }
        Statement::For {
            init,
            condition,
            update,
            body,
        } => {
            execute_statement(rt, init, sctx, outcome)?;
            for i in 0.. {
                if i >= LOOP_ITER_LIMIT {
                    return Err(StmtError::LoopLimit);
                }
                let v = eval_with(condition, rt, sctx)?;
                if !v.as_bool() {
                    break;
                }
                execute_statements(rt, body, sctx, outcome)?;
                execute_statement(rt, update, sctx, outcome)?;
            }
        }
        Statement::Call { callee, args } => {
            let mut argv = Vec::with_capacity(args.len());
            for a in args {
                argv.push(eval_with(a, rt, sctx)?);
            }
            let _ = sctx.externs.invoke(callee, &argv, false);
            outcome.actions_executed.push(callee.clone());
        }
        Statement::Raise { event_id, args } => {
            // Action `raise` events are enqueued at the FRONT of the queue
            // per Doc 08 §3.2 and §14 — they are processed before the next
            // external event. We capture argument values keyed by parameter
            // name so guard expressions can read `payload.field`.
            let payload = build_event_payload(rt, sctx, event_id, args)?;
            rt.queue.push_front(QueuedEvent {
                kind: EventKind::Raised {
                    event_id: event_id.clone(),
                },
                payload,
            })?;
        }
        Statement::Send { .. } => {
            // Cross-machine routing requires a multi-instance simulator. For
            // v1.0 we treat `send` to the same machine as `raise`; routing to
            // a different machine is unsupported.
            return Err(StmtError::SendUnsupported);
        }
        Statement::Defer { event_id } => {
            if !rt.defer_set.iter().any(|e| e == event_id) {
                rt.defer_set.push(event_id.clone());
            }
        }
    }
    Ok(())
}

/// Coerce an arithmetic result to the declared type of `field` on the
/// machine. Falls back to the value as-is if the field is unknown or has a
/// non-primitive type.
fn coerce_to_field(rt: &RuntimeState, field: &str, v: Value) -> Value {
    let declared = rt
        .machine
        .machine
        .context
        .fields
        .iter()
        .find(|f| f.name == field)
        .map(|f| &f.ty);
    let Some(fsm_ir::Type::Primitive { name }) = declared else {
        return v;
    };
    v.cast_to(name.as_str()).unwrap_or(v)
}

fn eval_with(e: &Expr, rt: &RuntimeState, sctx: &StmtCtx) -> Result<Value, StmtError> {
    let evctx = EvalCtx {
        context: &rt.context,
        payload: sctx.current_payload.as_ref(),
        externs: sctx.externs,
    };
    Ok(eval_expr(e, &evctx)?)
}

/// Build an event-payload map from a `raise` / `send` call's argument list.
/// We need the event's payload-parameter names to key the map; if the event
/// is unknown the arguments are silently dropped (the simulator is forgiving
/// — analyzer would have caught the error at compile time).
fn build_event_payload(
    rt: &RuntimeState,
    sctx: &StmtCtx,
    event_id: &str,
    args: &[Expr],
) -> Result<Option<HashMap<String, Value>>, StmtError> {
    let evctx = EvalCtx {
        context: &rt.context,
        payload: sctx.current_payload.as_ref(),
        externs: sctx.externs,
    };
    let event = sctx.machine.events_by_id.get(event_id);
    let mut map = HashMap::new();
    if let Some(ev) = event {
        for (i, arg) in args.iter().enumerate() {
            let v = eval_expr(arg, &evctx)?;
            if let Some(p) = ev.payload.get(i) {
                map.insert(p.name.clone(), v);
            }
        }
    }
    Ok(if map.is_empty() { None } else { Some(map) })
}
