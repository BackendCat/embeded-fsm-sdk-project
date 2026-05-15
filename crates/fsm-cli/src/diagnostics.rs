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

use fsm_diagnostics::deprecated::DeprecatedCode;
use fsm_diagnostics::{Diagnostic, DiagnosticCode, Severity};
use miette::NamedSource;

/// Render diagnostics for a single source file in human-readable form.
///
/// Output goes to stderr. `path` is the file label printed under each
/// `-->` pointer; `src` is the source text used to draw the snippet and
/// caret. Panic-safe — writer failures are silently discarded.
pub(crate) fn render_human(diags: &[Diagnostic], src: &str, path: &str) {
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
pub(crate) fn any_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// Promote every warning to error in-place. Used by `--warn-as-error`.
pub(crate) fn promote_warnings(diags: &mut [Diagnostic]) {
    for d in diags.iter_mut() {
        if d.severity == Severity::Warning {
            d.severity = Severity::Error;
        }
    }
}

/// A `fsm.toml [compiler]` lint-control entry that does not name any code
/// the toolchain knows (live OR retired). Surfaced as an exit-4 config
/// error rather than silently ignored — a key that looks honoured but is
/// not is the project's silent-data-loss cardinal sin (Doc 18 §3 exit-4,
/// the same clean-reject path `fsm.toml` type errors take).
#[derive(Debug)]
pub(crate) struct UnknownLintCode {
    /// `"allow"` or `"deny"` — the array the bad string came from.
    pub(crate) key: &'static str,
    /// The offending string exactly as written in `fsm.toml`.
    pub(crate) code: String,
}

impl std::fmt::Display for UnknownLintCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "fsm.toml [compiler] {} lists unknown diagnostic code {:?} \
             (expected a code like \"FSM-W0500\")",
            self.key, self.code
        )
    }
}

/// Validate a `[compiler] allow`/`deny` array against the diagnostic code
/// registry. A string is accepted IFF it parses as either a live
/// [`DiagnosticCode`] or a retired [`DeprecatedCode`] — Doc 10 §14 rule 2
/// requires retired codes to still be *silently accepted* in suppression
/// contexts (a project that pinned `allow = ["FSM-E0301"]` before that
/// code was retired must not start failing). Any other string is a clean
/// exit-4 config error (never a silent ignore — that is the same
/// silently-honoured-but-not sin the whole FU#67 wave exists to kill).
///
/// `key` is `"allow"` or `"deny"` purely for the error message. Returns
/// the first offending entry (one clear error beats a noisy dump; the
/// user fixes one typo and re-runs).
fn validate_codes(codes: &[String], key: &'static str) -> Result<(), UnknownLintCode> {
    for raw in codes {
        let known =
            DiagnosticCode::from_str(raw).is_some() || DeprecatedCode::from_str(raw).is_some();
        if !known {
            return Err(UnknownLintCode {
                key,
                code: raw.clone(),
            });
        }
    }
    Ok(())
}

/// Apply the merged `fsm.toml [compiler] allow`/`deny` lists to a finalized
/// diagnostic set, mirroring how `--warn-as-error` is applied here (a
/// post-analysis presentation transform — Doc 18 §6, the `[compiler]`
/// section). This is the SINGLE finalization point so `fsm check`'s
/// human + JSON renderers and its exit-code decision all observe the same
/// post-`allow`/`deny` set; it is intentionally NOT part of the
/// `fsm_analyzer` reuse seam the LSP shares (Doc 26 §3) — `allow`/`deny`
/// are a CLI/build-tool config concept (`fsm.toml`), exactly like
/// `warn_as_error`, which has always been a CLI-only finalization step and
/// is not in that seam. The editor surface has its own settings model
/// (`crates/fsm-lsp/src/config.rs`); extending the established CLI-only
/// finalization pattern is the non-forking choice.
///
/// Semantics, verbatim from Doc 18 (`fsm check` Options table + §6
/// example):
///   - `allow = [CODE, …]` — "Suppress specific diagnostic code globally":
///     every diagnostic whose code is listed is *removed* from the set, so
///     it is neither rendered nor able to set the exit code.
///   - `deny = [CODE, …]` — "Treat specific diagnostic code as error":
///     every *warning* whose code is listed is promoted to `Error` (so it
///     forces exit 1). A code already at `Error` is unaffected; `deny` does
///     not invent diagnostics, it only elevates ones that fired.
///
/// `deny` is applied AFTER `allow`: an `allow`ed code is gone before
/// `deny` runs, so listing the same code in both yields "suppressed"
/// (allow wins). That ordering is deterministic and the only sensible
/// reading of "globally suppress" — a suppressed diagnostic cannot also be
/// "an error".
///
/// `--warn-as-error` (if also set) runs separately at the call site; its
/// blanket promotion and `deny`'s targeted promotion compose without
/// conflict (both only ever raise severity).
///
/// Validation happens BEFORE any mutation: an unknown code anywhere in
/// either list aborts with [`UnknownLintCode`] and the diagnostics are
/// left untouched (the caller maps that to exit 4).
pub(crate) fn apply_allow_deny(
    diags: &mut Vec<Diagnostic>,
    allow: &[String],
    deny: &[String],
) -> Result<(), UnknownLintCode> {
    validate_codes(allow, "allow")?;
    validate_codes(deny, "deny")?;

    if !allow.is_empty() {
        diags.retain(|d| !allow.iter().any(|a| a == &format!("{}", d.code)));
    }
    if !deny.is_empty() {
        for d in diags.iter_mut() {
            if d.severity == Severity::Warning && deny.iter().any(|x| x == &format!("{}", d.code)) {
                d.severity = Severity::Error;
            }
        }
    }
    Ok(())
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

    // --- FU#67 allow/deny finalization (unit-level; the behavioural
    // end-to-end proof lives in tests/cli_check.rs driving the binary) ---

    fn w(c: DiagnosticCode) -> Diagnostic {
        Diagnostic::new(c, Span::new(0, 1))
    }

    #[test]
    fn allow_removes_only_the_listed_code() {
        let mut ds = vec![w(DiagnosticCode::W0100), w(DiagnosticCode::W0600)];
        apply_allow_deny(&mut ds, &["FSM-W0600".to_owned()], &[]).unwrap();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].code, DiagnosticCode::W0100);
    }

    #[test]
    fn deny_elevates_listed_warning_to_error_only() {
        let mut ds = vec![w(DiagnosticCode::W0100), w(DiagnosticCode::W0600)];
        apply_allow_deny(&mut ds, &[], &["FSM-W0600".to_owned()]).unwrap();
        assert_eq!(ds[0].severity, Severity::Warning, "W0100 untouched");
        assert_eq!(ds[1].severity, Severity::Error, "W0600 elevated");
    }

    #[test]
    fn deny_does_not_demote_an_existing_error() {
        // `deny` only ever RAISES severity; an Error stays an Error.
        let mut ds = vec![w(DiagnosticCode::E0001)];
        apply_allow_deny(&mut ds, &[], &["FSM-E0001".to_owned()]).unwrap();
        assert_eq!(ds[0].severity, Severity::Error);
    }

    #[test]
    fn allow_wins_over_deny_for_the_same_code() {
        let mut ds = vec![w(DiagnosticCode::W0600)];
        apply_allow_deny(
            &mut ds,
            &["FSM-W0600".to_owned()],
            &["FSM-W0600".to_owned()],
        )
        .unwrap();
        assert!(ds.is_empty(), "allow applied before deny -> suppressed");
    }

    #[test]
    fn unknown_code_is_an_error_and_leaves_diags_untouched() {
        let mut ds = vec![w(DiagnosticCode::W0600)];
        let err = apply_allow_deny(&mut ds, &["FSM-W9999".to_owned()], &[]).unwrap_err();
        assert_eq!(err.key, "allow");
        assert_eq!(err.code, "FSM-W9999");
        assert_eq!(ds.len(), 1, "validation aborts before any mutation");
        // The Display string must name the offending key + code clearly.
        let msg = format!("{err}");
        assert!(msg.contains("allow") && msg.contains("FSM-W9999"), "{msg}");
    }

    #[test]
    fn retired_code_is_accepted_not_an_error() {
        // Doc 10 §14 rule 2: retired codes still parse + are accepted.
        let mut ds = vec![w(DiagnosticCode::W0600)];
        apply_allow_deny(&mut ds, &["FSM-E0301".to_owned()], &[]).unwrap();
        assert_eq!(ds.len(), 1, "E0301 not present, nothing suppressed");
    }

    #[test]
    fn empty_lists_are_a_no_op() {
        let mut ds = vec![w(DiagnosticCode::W0600)];
        apply_allow_deny(&mut ds, &[], &[]).unwrap();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0].severity, Severity::Warning);
    }
}
