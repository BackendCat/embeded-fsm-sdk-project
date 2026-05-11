//! JSON I/O helpers for the canonical IR.
//!
//! `to_json` / `from_json` are the canonical entry points. `from_json`
//! enforces the major-version compatibility rule from Doc 09 §17: a
//! consumer MUST reject documents whose `irVersion` major differs from
//! its own — for v1.0 that means anything outside `1.x.y`.

use std::io::{Read, Write};

use serde::Deserialize;
use thiserror::Error;

use crate::model::{Ir, CURRENT_IR_VERSION};

/// Error type for all `fsm-ir` JSON operations.
#[derive(Debug, Error)]
pub enum IrJsonError {
    /// `serde_json` failed during parse or serialise.
    #[error("JSON error: {0}")]
    Serde(#[from] serde_json::Error),

    /// I/O error from a reader / writer.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// `irVersion` field is missing entirely. Doc 09 §2 requires it.
    #[error("IR document is missing required field `irVersion`")]
    MissingIrVersion,

    /// `irVersion` exists but is not a valid `MAJOR.MINOR.PATCH` triple.
    #[error("IR document has unparsable `irVersion`: {got:?}")]
    UnparsableIrVersion { got: String },

    /// `irVersion` major component differs from what this consumer supports.
    /// Per Doc 09 §17 consumers MUST reject mismatched majors.
    #[error(
        "IR document has incompatible major version: got {got:?}, this consumer expects {expected_major}.x.y"
    )]
    IncompatibleMajorVersion { got: String, expected_major: u32 },
}

/// Pretty-print an [`Ir`] to JSON.
pub fn to_json(ir: &Ir) -> Result<String, IrJsonError> {
    Ok(serde_json::to_string_pretty(ir)?)
}

/// Parse JSON into an [`Ir`], enforcing major-version compatibility.
///
/// Per Doc 09 §17, consumers MUST reject documents whose major version
/// differs from the consumer's. This implementation supports `1.x.y` and
/// rejects anything else with [`IrJsonError::IncompatibleMajorVersion`].
pub fn from_json(s: &str) -> Result<Ir, IrJsonError> {
    // Peek at irVersion BEFORE full deserialisation. A higher-major
    // document may contain new required fields whose absence in the
    // current Rust struct would cause a confusing "missing field" error
    // rather than the precise "incompatible major" error we want.
    let raw: serde_json::Value = serde_json::from_str(s)?;
    check_ir_version(&raw)?;
    let ir = Ir::deserialize(raw)?;
    Ok(ir)
}

/// Write an [`Ir`] as pretty-printed JSON to `w`.
pub fn to_writer<W: Write>(mut w: W, ir: &Ir) -> Result<(), IrJsonError> {
    let s = to_json(ir)?;
    w.write_all(s.as_bytes())?;
    Ok(())
}

/// Read an [`Ir`] from `r`, enforcing major-version compatibility.
pub fn from_reader<R: Read>(mut r: R) -> Result<Ir, IrJsonError> {
    let mut buf = String::new();
    r.read_to_string(&mut buf)?;
    from_json(&buf)
}

/// Validate the `irVersion` field on a raw JSON value before full parsing.
fn check_ir_version(raw: &serde_json::Value) -> Result<(), IrJsonError> {
    let v = raw
        .get("irVersion")
        .ok_or(IrJsonError::MissingIrVersion)?
        .as_str()
        .ok_or_else(|| IrJsonError::UnparsableIrVersion {
            got: raw
                .get("irVersion")
                .map(|x| x.to_string())
                .unwrap_or_default(),
        })?;
    let major = parse_major(v).ok_or_else(|| IrJsonError::UnparsableIrVersion { got: v.into() })?;
    let expected_major =
        parse_major(CURRENT_IR_VERSION).expect("CURRENT_IR_VERSION is well-formed");
    if major != expected_major {
        return Err(IrJsonError::IncompatibleMajorVersion {
            got: v.into(),
            expected_major,
        });
    }
    Ok(())
}

fn parse_major(v: &str) -> Option<u32> {
    v.split('.').next()?.parse().ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use fsm_diagnostics::{SourceLocation, Span};

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    fn empty_machine(name: &str) -> MachineObject {
        MachineObject {
            id: format!("m-{name}"),
            stable_id: name.into(),
            name: name.into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial-0".into(),
                states: vec![],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig {
                capacity: 16,
                overflow_policy: OverflowPolicy::Assert,
                loc: loc(),
            },
            targets: vec![],
            loc: loc(),
        }
    }

    #[test]
    fn empty_ir_round_trips() {
        let ir = Ir::default();
        let s = to_json(&ir).unwrap();
        let back = from_json(&s).unwrap();
        assert_eq!(ir, back);
    }

    #[test]
    fn from_json_rejects_v2() {
        let s = r#"{"irVersion":"2.0.0","sourceHash":"sha256:","sourceFiles":[],"machines":[],"diagnostics":[]}"#;
        match from_json(s) {
            Err(IrJsonError::IncompatibleMajorVersion {
                got,
                expected_major,
            }) => {
                assert_eq!(got, "2.0.0");
                assert_eq!(expected_major, 1);
            }
            other => panic!("expected IncompatibleMajorVersion, got {other:?}"),
        }
    }

    #[test]
    fn from_json_accepts_higher_minor_and_patch() {
        // Forward-compatible per Doc 09 §17: same major, higher minor/patch
        // is accepted (consumer ignores unknown fields).
        let s = r#"{"irVersion":"1.99.999","sourceHash":"","sourceFiles":[],"machines":[],"diagnostics":[]}"#;
        let ir = from_json(s).unwrap();
        assert_eq!(ir.ir_version, "1.99.999");
    }

    #[test]
    fn from_json_rejects_missing_version() {
        let s = r#"{"sourceHash":"","sourceFiles":[],"machines":[],"diagnostics":[]}"#;
        assert!(matches!(from_json(s), Err(IrJsonError::MissingIrVersion)));
    }

    #[test]
    fn from_json_rejects_unparsable_version() {
        let s =
            r#"{"irVersion":"v1","sourceHash":"","sourceFiles":[],"machines":[],"diagnostics":[]}"#;
        assert!(matches!(
            from_json(s),
            Err(IrJsonError::UnparsableIrVersion { .. })
        ));
    }

    #[test]
    fn writer_and_reader_round_trip() {
        let ir = Ir {
            source_hash: "sha256:abc".into(),
            source_files: vec!["m.fsm".into()],
            machines: vec![empty_machine("Motor")],
            ..Ir::default()
        };

        let mut buf: Vec<u8> = Vec::new();
        to_writer(&mut buf, &ir).unwrap();
        let back = from_reader(buf.as_slice()).unwrap();
        assert_eq!(ir, back);
    }
}
