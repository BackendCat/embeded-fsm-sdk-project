//! Trace recording and replay — `StepRecord` per Doc 13 §11 plus a small
//! `TraceFile` format used by the conformance test runner.
//!
//! ## Trace file format
//!
//! Doc 15 §7 specifies trace files in YAML. For v1.0 we ship a JSON variant
//! (line-for-line equivalent shape — keys and structure unchanged) because
//! adding `serde_yaml` to the workspace would violate the disk-conscious
//! "minimum-deps" guidance and break the prior decision to depend on serde
//! only. The runtime trace records are JSON anyway per Doc 13 §11, so the
//! choice unifies the two formats. Switching to YAML in v1.1 is a one-line
//! Cargo.toml change plus a parser swap.
//!
//! ## Shape
//!
//! ```json
//! {
//!   "machineFile": "../machines/motor.fsm",
//!   "description": "Motor positive flow",
//!   "init": { "context": {} },
//!   "steps": [
//!     { "action": "dispatch", "event": "START" },
//!     { "action": "advance_clock", "deltaMs": 100 },
//!     { "action": "raise", "event": "FAULT" }
//!   ],
//!   "expected": [ /* StepRecord ... */ ]
//! }
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::interpreter::{Interpreter, StepError};
use crate::runtime::value::Value;
use fsm_ir::Ir;

/// One emitted step record. Doc 13 §11 normative shape — kept identical to
/// the JSON example there for trace correlation across tools.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRecord {
    pub trace_id: u64,
    /// `kind` is not in Doc 13 §11 but harmless — every consumer of the JSON
    /// can ignore unknown fields per Doc 09 §17, and it gives the simulator
    /// a deterministic discriminator for asserting expected step types.
    pub kind: StepKind,
    /// Virtual clock at the moment the step started. `timestampMs` in Doc 13
    /// §11 is the same value — renamed here to make the meaning explicit and
    /// avoid the "wall vs. virtual" question.
    pub virtual_clock_ms: u64,
    /// Original dispatch event, if any. None for completion / timer steps
    /// that have no DSL-level event name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_received: Option<EventReceivedRecord>,
    /// Transition that fired (matches Doc 13 §11 `transitionTaken`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_taken: Option<TransitionTakenRecord>,
    /// State IDs that exited, in innermost-first order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exited_states: Vec<String>,
    /// State IDs that entered, in outermost-first order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entered_states: Vec<String>,
    /// Names of extern callees invoked from the transition / entry / exit
    /// actions. Doc 13 §11 `actionsExecuted` field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions_executed: Vec<String>,
    /// Active config before the step.
    pub config_before: Vec<String>,
    /// Active config after the step.
    pub config_after: Vec<String>,
}

/// Discriminator for the kind of step recorded. Set by the interpreter as a
/// convenience for trace consumers; the structural fields above are
/// authoritative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Initialization step — populated by [`Interpreter::init`]. No event,
    /// only the entry path from root.
    Init,
    /// An external event was dispatched.
    Dispatched,
    /// An internal `raise` was processed.
    Raised,
    /// A timer fired.
    TimerFired,
    /// A synthetic completion event was processed.
    Completion,
    /// The event was discarded (no enabled transition).
    Discarded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventReceivedRecord {
    pub name: String,
    /// Stable event ID — only present when distinct from `name`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<HashMap<String, Value>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionTakenRecord {
    /// Transition's stable_id (Doc 13 §11).
    pub stable_id: String,
    pub source: String,
    pub target: String,
}

// ---------------------------------------------------------------------------
// Trace file — driver input for the conformance test runner.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceFile {
    /// Optional pointer back to the source machine file (Doc 15 §7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub init: InitTrace,
    pub steps: Vec<TraceCommand>,
    /// Optional pre-recorded expected step output. When present,
    /// [`execute_trace`] returns a per-step diff against this list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected: Vec<StepRecord>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitTrace {
    /// Initial context values keyed by field name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<HashMap<String, Value>>,
    /// Initial virtual clock — defaults to 0 if omitted.
    #[serde(default)]
    pub virtual_clock_start_ms: u64,
    /// Machine name to load. Defaults to the first machine in the IR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TraceCommand {
    Dispatch {
        event: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<HashMap<String, Value>>,
    },
    AdvanceClock {
        #[serde(rename = "deltaMs")]
        delta_ms: u64,
    },
    Raise {
        event: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<HashMap<String, Value>>,
    },
}

/// Aggregate result of executing one trace file.
#[derive(Clone, Debug)]
pub struct TraceResult {
    /// Actual step records produced by the interpreter.
    pub actual: Vec<StepRecord>,
    /// `true` when `actual == trace.expected` (or `expected` was empty,
    /// meaning the caller wanted to capture rather than assert).
    pub matches_expected: bool,
    /// Index of the first mismatching record, or `None` if all match.
    pub first_mismatch: Option<usize>,
}

#[derive(Debug, Error)]
pub enum TraceParseError {
    #[error("json parse: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("interpreter: {0}")]
    Interp(#[from] StepError),
    #[error("event '{0}' not declared on machine")]
    UnknownEvent(String),
}

pub fn parse_trace_yaml(s: &str) -> Result<TraceFile, TraceParseError> {
    // JSON in v1.0; same shape as the eventual YAML format. See module docs.
    Ok(serde_json::from_str(s)?)
}

pub fn write_trace_yaml(t: &TraceFile) -> Result<String, TraceParseError> {
    Ok(serde_json::to_string_pretty(t)?)
}

/// Execute a [`TraceFile`] against the given IR, returning the actual step
/// records and a diff verdict against `trace.expected`.
pub fn execute_trace(ir: &Ir, trace: &TraceFile) -> Result<TraceResult, ExecError> {
    let mut interp = Interpreter::new(ir)?;
    let machine_name = trace
        .init
        .machine_name
        .clone()
        .or_else(|| ir.machines.first().map(|m| m.name.clone()))
        .ok_or_else(|| ExecError::UnknownEvent("(no machines in IR)".into()))?;
    let opts = crate::interpreter::InitOptions {
        machine_name,
        initial_context: trace.init.context.clone(),
        virtual_clock_start_ms: trace.init.virtual_clock_start_ms,
    };
    let mut actual = interp.init(opts)?;
    for cmd in &trace.steps {
        match cmd {
            TraceCommand::Dispatch { event, payload } => {
                let recs = interp.dispatch_with_payload(event, payload.clone())?;
                actual.extend(recs);
            }
            TraceCommand::AdvanceClock { delta_ms } => {
                let recs = interp.advance_clock(*delta_ms)?;
                actual.extend(recs);
            }
            TraceCommand::Raise { event, payload } => {
                let recs = interp.raise_with_payload(event, payload.clone())?;
                actual.extend(recs);
            }
        }
    }
    let mut first_mismatch: Option<usize> = None;
    if !trace.expected.is_empty() {
        for (i, (got, want)) in actual.iter().zip(trace.expected.iter()).enumerate() {
            if got != want {
                first_mismatch = Some(i);
                break;
            }
        }
        if first_mismatch.is_none() && actual.len() != trace.expected.len() {
            first_mismatch = Some(actual.len().min(trace.expected.len()));
        }
    }
    Ok(TraceResult {
        actual,
        matches_expected: trace.expected.is_empty() || first_mismatch.is_none(),
        first_mismatch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_dispatch_command() {
        let t = TraceFile {
            machine_file: None,
            description: Some("test".into()),
            init: InitTrace::default(),
            steps: vec![
                TraceCommand::Dispatch {
                    event: "START".into(),
                    payload: None,
                },
                TraceCommand::AdvanceClock { delta_ms: 100 },
                TraceCommand::Raise {
                    event: "STOP".into(),
                    payload: None,
                },
            ],
            expected: vec![],
        };
        let s = write_trace_yaml(&t).unwrap();
        let back = parse_trace_yaml(&s).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn step_record_kind_round_trips() {
        let rec = StepRecord {
            trace_id: 1,
            kind: StepKind::Dispatched,
            virtual_clock_ms: 0,
            event_received: None,
            transition_taken: None,
            exited_states: vec![],
            entered_states: vec![],
            actions_executed: vec![],
            config_before: vec![],
            config_after: vec![],
        };
        let s = serde_json::to_string(&rec).unwrap();
        let back: StepRecord = serde_json::from_str(&s).unwrap();
        assert_eq!(rec, back);
    }
}
