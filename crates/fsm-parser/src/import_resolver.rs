//! Compile-time validation of `import "path"` declarations.
//!
//! Per Doc 00 §7.12 (G-02) the parser MUST reject obviously malicious import
//! paths so a `.fsm` source posted to a shared build host cannot exfiltrate
//! arbitrary files. The full canonicalisation check (resolve under workspace
//! root, follow symlinks before comparison) depends on the workspace context
//! which the parser does not have. The parser performs the **path-shape**
//! checks here; the workspace-relative canonicalisation step is the resolver's
//! responsibility downstream.
//!
//! Rejected forms:
//! - Absolute Unix paths (`/etc/passwd`).
//! - Windows-style absolute paths (`C:\…`, `\\server\share`).
//! - Any path containing a `..` segment ("parent directory" escape).
//! - Paths with `\0` (NUL) bytes — defensive, the lexer normally rejects
//!   these but defence in depth is cheap.
//! - Empty path strings.
//!
//! Accepted forms:
//! - Workspace-relative paths: `common/events.fsm`, `./types.fsm`.
//! - Whether the file exists is the resolver's problem; the parser only
//!   refuses paths that would be unsafe to even try.

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};

/// Outcome of the path-shape check. The OK variant carries no payload — the
/// parser stores the raw string slice in the CST regardless.
pub type ImportPathResult = Result<(), Diagnostic>;

/// Validate the path *content* of an `import "…"` declaration.
///
/// `raw` is the **unquoted** path content (the lexer already stripped the
/// surrounding `"`). `span` is the source span of the original string
/// literal — diagnostics anchor here so the editor highlights the right
/// region.
///
/// The diagnostic code emitted on failure is `FSM-E0010` ("expected token")
/// with a parameterised message; Doc 10 §3 already covers parser-level
/// syntactic errors under E0010, and treating an unsafe path as a syntactic
/// error keeps the catalogue narrow. The full G-02 wording is preserved in
/// the diagnostic's message so the LSP can surface it.
pub fn validate_import_path(raw: &str, span: Span) -> ImportPathResult {
    if raw.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message("invalid import path: empty string"));
    }
    if raw.as_bytes().contains(&0) {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message("invalid import path: contains NUL byte"));
    }
    if is_absolute_unix(raw) {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message("invalid import path: absolute paths are not permitted"));
    }
    if is_absolute_windows(raw) {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message("invalid import path: drive-prefixed paths are not permitted"));
    }
    if has_parent_segment(raw) {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span).with_message(
            "invalid import path: `..` segments are not permitted (path traversal)",
        ));
    }
    Ok(())
}

fn is_absolute_unix(p: &str) -> bool {
    p.starts_with('/')
}

fn is_absolute_windows(p: &str) -> bool {
    // `C:\foo`, `C:/foo`, or UNC `\\server\share`.
    let bytes = p.as_bytes();
    if bytes.len() >= 2 {
        let drive = bytes[0];
        let colon = bytes[1];
        if drive.is_ascii_alphabetic() && colon == b':' {
            return true;
        }
    }
    p.starts_with('\\') || p.starts_with("//")
}

/// True iff any path segment (separated by `/` or `\`) is exactly `..`.
fn has_parent_segment(p: &str) -> bool {
    p.split(|c| c == '/' || c == '\\').any(|seg| seg == "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) {
        assert!(
            validate_import_path(s, Span::new(0, s.len())).is_ok(),
            "expected {s:?} to be accepted"
        );
    }
    fn bad(s: &str) {
        let r = validate_import_path(s, Span::new(0, s.len()));
        assert!(r.is_err(), "expected {s:?} to be rejected");
        assert_eq!(r.unwrap_err().code, DiagnosticCode::E0010);
    }

    #[test]
    fn rejects_empty() {
        bad("");
    }

    #[test]
    fn rejects_absolute() {
        bad("/etc/passwd");
        bad("/tmp/foo.fsm");
    }

    #[test]
    fn rejects_windows_absolute() {
        bad("C:\\windows\\system32\\foo.fsm");
        bad("D:/projects/foo.fsm");
        bad("\\\\server\\share\\foo.fsm");
    }

    #[test]
    fn rejects_parent_segment() {
        bad("../foo.fsm");
        bad("a/../../etc/passwd");
        bad("a\\..\\..\\etc\\passwd");
    }

    #[test]
    fn accepts_normal_paths() {
        ok("foo.fsm");
        ok("common/events.fsm");
        ok("./types.fsm");
        ok("a/b/c.fsm");
        ok("a.b/c.fsm");
    }

    #[test]
    fn rejects_nul_byte() {
        bad("foo\0bar.fsm");
    }
}
