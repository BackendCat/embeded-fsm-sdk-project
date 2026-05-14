//! Error type surfaced from [`crate::format_string`].
//!
//! Only `ParseFailed` is actually emittable today — `format` on a valid CST
//! never fails. We keep the variant set open so future additions (e.g.
//! "input exceeded internal depth limit") slot in without breaking the
//! public API.

use fsm_diagnostics::Diagnostic;
use std::fmt;

/// Failure modes for the formatter.
#[derive(Debug, Clone)]
pub enum FormatError {
    /// The input source did not parse cleanly. The wrapped diagnostics
    /// come straight from `fsm-parser`. Per Doc 19 §1.4 the formatter must
    /// be semantic-neutral; refusing to operate on broken input is the
    /// only safe option (we'd otherwise risk emitting code that re-parses
    /// to a different tree).
    ParseFailed(Vec<Diagnostic>),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::ParseFailed(d) => {
                write!(f, "input did not parse cleanly ({} diagnostic(s))", d.len())
            }
        }
    }
}

impl std::error::Error for FormatError {}
