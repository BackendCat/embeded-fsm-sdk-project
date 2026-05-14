//! Compile-time validation of `import "path"` declarations.
//!
//! Per Doc 00 §7.12 (G-02) the parser MUST reject obviously malicious import
//! paths so a `.fsm` source posted to a shared build host cannot exfiltrate
//! arbitrary files. The module provides **two** layers of defence:
//!
//! 1. [`validate_import_path`] — a **no-I/O, shape-only** check. Rejects
//!    absolute paths, `..` segments, Windows drive letters, UNC prefixes,
//!    NUL bytes, and the empty string. This is the form the parser itself
//!    uses at parse time, when the workspace root is not yet known (e.g.,
//!    a `parse(src)` call from the LSP that does not carry a file context).
//!
//! 2. [`resolve_import`] — the **full security check** that requires
//!    filesystem context. After the shape check passes, the path is joined
//!    onto the importing file's directory, then [`Path::canonicalize`]'d
//!    (which follows symlinks). The result must start with the workspace
//!    root; otherwise the import is rejected as a containment escape. This
//!    is the form the CLI / build driver MUST call before opening an
//!    imported `.fsm` file.
//!
//! The shape-only path is *necessary but not sufficient*: it cannot detect
//! a workspace-relative symlink that points outside the workspace (the
//! attacker controls the imported `.fsm` filename; they may not control
//! the filesystem, but they can craft sources that exploit symlinks that
//! already exist). Only the canonicalize-plus-prefix check closes that gap.
//!
//! Rejected forms (shape):
//! - Absolute Unix paths (`/etc/passwd`).
//! - Windows-style absolute paths (`C:\…`, `\\server\share`).
//! - Any path containing a `..` segment ("parent directory" escape).
//! - Paths with `\0` (NUL) bytes — defensive, the lexer normally rejects
//!   these but defence in depth is cheap.
//! - Empty path strings.
//!
//! Rejected forms (resolve):
//! - All of the above, plus
//! - Paths that, after canonicalization, fall outside the workspace root
//!   (e.g., via a symlink pointing to `/etc/`).
//! - Paths that do not resolve to an existing filesystem entry.
//!
//! Accepted forms:
//! - Workspace-relative paths: `common/events.fsm`, `./types.fsm`.
//! - For [`validate_import_path`], existence is not checked.
//! - For [`resolve_import`], the path must point to a real file/dir whose
//!   canonical form is inside the workspace.

use std::path::{Path, PathBuf};

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};
use thiserror::Error;

/// Outcome of the path-shape check. The OK variant carries no payload — the
/// parser stores the raw string slice in the CST regardless.
pub type ImportPathResult = Result<(), Diagnostic>;

/// Resolution-time failure modes for [`resolve_import`]. Mapped to
/// `FSM-E0010` diagnostics by callers (the parser-level catch-all). The
/// variant carries enough context for a build driver to surface a
/// human-readable message without re-classifying.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImportError {
    /// Shape check (no-I/O) failed. `reason` is the human-readable cause
    /// emitted by [`validate_import_path`].
    #[error("invalid import path shape: {reason}")]
    BadShape { reason: String },
    /// `canonicalize()` failed — the candidate path does not resolve to an
    /// existing filesystem entry. This is also returned when the importing
    /// file has no parent directory (an unusual but possible state for
    /// `parse(src)` calls fed a synthesized path).
    #[error("import path could not be resolved")]
    Unresolved,
    /// After canonicalization the resolved path is outside the workspace
    /// root. This is the symlink-escape rejection.
    #[error("import path resolves outside the workspace root")]
    OutsideWorkspace,
}

impl ImportError {
    /// Convert into a `FSM-E0010` diagnostic anchored at `span`. The parser
    /// keeps the diagnostic catalogue narrow by routing every import-path
    /// failure through E0010 (per Doc 10 §3 and the existing convention in
    /// [`validate_import_path`]).
    pub fn into_diagnostic(self, span: Span) -> Diagnostic {
        let msg = match &self {
            ImportError::BadShape { reason } => format!("invalid import path: {reason}"),
            ImportError::Unresolved => {
                "invalid import path: could not be resolved on the filesystem".to_string()
            }
            ImportError::OutsideWorkspace => {
                "invalid import path: resolves outside the workspace root".to_string()
            }
        };
        Diagnostic::new(DiagnosticCode::E0010, span).with_message(msg)
    }
}

/// Validate the path *content* of an `import "…"` declaration — **no I/O**.
///
/// `raw` is the **unquoted** path content (the lexer already stripped the
/// surrounding `"`). `span` is the source span of the original string
/// literal — diagnostics anchor here so the editor highlights the right
/// region.
///
/// This is the shape-only form callable without a workspace root, suited
/// to the parser's parse-time path. The full security check
/// ([`resolve_import`]) is the parser-side contract for callers that hold
/// filesystem context.
///
/// The diagnostic code emitted on failure is `FSM-E0010` ("expected token")
/// with a parameterised message; Doc 10 §3 already covers parser-level
/// syntactic errors under E0010, and treating an unsafe path as a syntactic
/// error keeps the catalogue narrow. The full G-02 wording is preserved in
/// the diagnostic's message so the LSP can surface it.
pub fn validate_import_path(raw: &str, span: Span) -> ImportPathResult {
    match validate_shape(raw) {
        Ok(()) => Ok(()),
        Err(reason) => Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message(format!("invalid import path: {reason}"))),
    }
}

/// Resolve an `import "path"` to its **canonical** filesystem location,
/// enforcing workspace-root containment. This is the **strict** form per
/// Doc 00 §7.12 G-02 and MUST be called by any build driver that actually
/// opens an imported `.fsm` file.
///
/// Parameters:
/// - `workspace_root` — the canonical absolute path of the workspace root
///   (the directory containing the nearest `fsm.toml`). The caller is
///   expected to have already canonicalized this; we re-canonicalize
///   defensively in case the caller passed a relative or symlinked form.
/// - `from_file` — the path of the importing `.fsm` source. Relative
///   imports resolve against this file's parent directory. The file itself
///   does not need to exist on disk yet; only its parent's existence
///   matters for canonicalization.
/// - `import_str` — the **unquoted** import-path content (the lexer
///   already stripped the surrounding `"`).
///
/// Behaviour:
/// 1. Shape check — rejects `..`, absolute paths, Windows drive prefixes,
///    NUL bytes, empty string. Returns [`ImportError::BadShape`].
/// 2. Resolve — `from_file.parent().join(import_str)`, then
///    [`Path::canonicalize`]. This follows symlinks. Returns
///    [`ImportError::Unresolved`] if either canonicalization fails or
///    `from_file` has no parent.
/// 3. Containment — `canonical.starts_with(canonical_workspace_root)`. If
///    false, returns [`ImportError::OutsideWorkspace`] — this is the
///    symlink-escape rejection.
///
/// On success, returns the canonicalized [`PathBuf`] suitable for opening.
pub fn resolve_import(
    workspace_root: &Path,
    from_file: &Path,
    import_str: &str,
) -> Result<PathBuf, ImportError> {
    // Step 1 — shape check (fast path, no I/O).
    if let Err(reason) = validate_shape(import_str) {
        return Err(ImportError::BadShape { reason });
    }

    // Step 2 — resolve the candidate path relative to the importing file's
    // parent directory. If `from_file` has no parent (e.g., a bare filename
    // with no directory component), fall back to the workspace root as the
    // resolution base; this keeps `parse(synthetic_path)` callers from
    // landing on a hard error before the real path can be applied.
    let base = from_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| workspace_root.to_path_buf());
    let candidate = base.join(import_str);
    let canonical = candidate
        .canonicalize()
        .map_err(|_| ImportError::Unresolved)?;

    // Step 3 — canonicalize the workspace root so the containment check
    // is symlink-stable on both sides. If the caller already canonicalized
    // the root, this is an idempotent re-canonicalization.
    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|_| ImportError::Unresolved)?;

    if !canonical.starts_with(canonical_root) {
        return Err(ImportError::OutsideWorkspace);
    }
    Ok(canonical)
}

/// Internal shape check returning the failure reason as a `String`. Shared
/// by [`validate_import_path`] (which wraps it in a `Diagnostic`) and
/// [`resolve_import`] (which wraps it in [`ImportError::BadShape`]).
fn validate_shape(raw: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err("empty string".to_string());
    }
    if raw.as_bytes().contains(&0) {
        return Err("contains NUL byte".to_string());
    }
    if is_absolute_unix(raw) {
        return Err("absolute paths are not permitted".to_string());
    }
    if is_absolute_windows(raw) {
        return Err("drive-prefixed paths are not permitted".to_string());
    }
    if has_parent_segment(raw) {
        return Err("`..` segments are not permitted (path traversal)".to_string());
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

    #[test]
    fn import_error_into_diagnostic_uses_e0010() {
        let d = ImportError::OutsideWorkspace.into_diagnostic(Span::new(0, 4));
        assert_eq!(d.code, DiagnosticCode::E0010);
        assert!(d.message.contains("outside"));
    }
}
