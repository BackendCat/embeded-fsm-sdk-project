//! Diagnostic rendering for the CLI.
//!
//! Two output modes are supported:
//!
//! - **human** (default) — Rust-style `error[FSM-Exxxx]: …` block with a
//!   `--> file:line:col` pointer (Doc 18 §3). Rendered via miette's
//!   GraphicalReportHandler so the source snippet + caret are free.
//! - **JSON** (`--json` flag on `check`) — `serde_json` array of diagnostic
//!   objects per Doc 18 §3, emitted from `cmd::check` directly so the
//!   aggregate array spans every input file.
//!
//! Human-mode output goes to stderr per Doc 18 §4. The JSON formatter
//! writes to stdout because the JSON IS the command's structured output.

use std::io::{self, Write};

use fsm_diagnostics::{Diagnostic, Severity};
use miette::NamedSource;

/// Render diagnostics for a single source file in human-readable form.
///
/// Output goes to stderr. `path` is the file label printed under each
/// `-->` pointer; `src` is the source text used to draw the snippet and
/// caret. Panic-safe — writer failures are silently discarded.
pub fn render_human(diags: &[Diagnostic], src: &str, path: &str) {
    // unicode_nocolor keeps escapes out of test golden outputs. A real
    // user-facing color path can route through `--no-color` in Doc 18 §2;
    // staying nocolor here avoids accidental escape leaks until we wire
    // that flag.
    let handler =
        miette::GraphicalReportHandler::new().with_theme(miette::GraphicalTheme::unicode_nocolor());
    let mut stderr = io::stderr().lock();
    for d in diags {
        let mut out = String::new();
        let report = miette::Report::new(d.clone())
            .with_source_code(NamedSource::new(path, src.to_string()));
        let _ = handler.render_report(&mut out, report.as_ref());
        let _ = stderr.write_all(out.as_bytes());
    }
    let _ = stderr.flush();
}

/// Returns `true` if any diagnostic in the slice carries error severity.
/// Used by every subcommand to choose between exit-0 and exit-1.
pub fn any_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// Promote every warning to error in-place. Used by `--warn-as-error`.
pub fn promote_warnings(diags: &mut [Diagnostic]) {
    for d in diags.iter_mut() {
        if d.severity == Severity::Warning {
            d.severity = Severity::Error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{DiagnosticCode, Span};

    #[test]
    fn any_errors_distinguishes_warning_from_error() {
        let err = Diagnostic::new(DiagnosticCode::E0001, Span::new(0, 1));
        let warn = Diagnostic::new(DiagnosticCode::W0100, Span::new(0, 1));
        assert!(any_errors(&[err.clone()]));
        assert!(!any_errors(&[warn.clone()]));
        assert!(any_errors(&[warn, err]));
    }

    #[test]
    fn promote_warnings_flips_severity() {
        let mut ds = vec![Diagnostic::new(DiagnosticCode::W0100, Span::new(0, 1))];
        promote_warnings(&mut ds);
        assert_eq!(ds[0].severity, Severity::Error);
    }
}
