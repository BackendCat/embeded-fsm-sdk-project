//! The keystone reuse seam — Doc 26 §3.
//!
//! `fsm-lsp` MUST NOT re-implement analysis (Doc 26 §2.1, Doc 20 §9.4). It
//! calls the **exact same** functions `fsm check` calls, in the same order,
//! so an editor squiggle can never disagree with `fsm check --json`:
//!
//! | step | `fsm check` (`crates/fsm-cli/src/cmd/check.rs`) | here |
//! |---|---|---|
//! | parse | `fsm_parser::parse(&src)` (`check.rs:46`) | `analyze` |
//! | workspace root | `crate::safe_io::workspace_root_for(path)` (`check.rs:59`) | [`workspace_root_for`] (CLI-internal `pub(crate)` fn re-implemented identically — see note) |
//! | import security | `security_check_imports` -> `resolve_import` (`check.rs:60,178`) | [`security_check_imports`] |
//! | semantic analysis | `fsm_analyzer::analyze_with_source(&pr,&label,&src)` (`check.rs:61`) | `analyze` |
//! | merge | `diags.append(&mut import_diags)` (`check.rs:65`) | `analyze` |
//!
//! The only behavioural difference from `fsm check`: the CLI reads the
//! file from disk; the LSP analyses the *in-memory editor buffer* (the
//! authoritative content while a document is open). The URI's filesystem
//! path is still used for the import-security containment check so an
//! `OutsideWorkspace` import squiggles in-editor identically to the CLI.
//!
//! ### Why `workspace_root_for` is re-implemented, not called
//!
//! `fsm_cli::safe_io::workspace_root_for` is `pub(crate)` *inside the
//! `fsm-cli` binary crate* — it is not, and should not become, public API
//! (the LSP must not depend on the CLI binary; Doc 26 §6 lists the
//! intended-public seams and the CLI is not among them). The function is
//! an 11-line `fsm.toml`-walker with no behavioural subtlety; Doc 26 §3
//! sanctions the LSP "SHOULD run it too so its diagnostics match the CLI
//! exactly". This re-implementation is byte-identical in behaviour to
//! `check.rs`'s use (walk parents for `fsm.toml`; fall back to the file's
//! own directory — the most permissive position that still rejects every
//! escape path). It is NOT a #64-narrowed *public-API* regression: nothing
//! cross-crate-public was narrowed; the CLI's private helper was never
//! public. Tracked as the intended boundary, not a defect.

use std::path::{Path, PathBuf};

use fsm_analyzer::{analyze_with_source, SymbolTable};
use fsm_diagnostics::{Diagnostic, Span};
use fsm_parser::ast::{AstNode, File as AstFile};
use fsm_parser::import_resolver::{resolve_import, ImportError};
use fsm_parser::{parse, ParseResult, SyntaxKind};

/// The result of **one** `fsm check` pipeline run over a buffer.
///
/// Doc 26 §3 / §8 L2: there is exactly **one** analysis per buffer and it
/// feeds *both* consumers — diagnostics (L1) **and** the symbol table (L2
/// `documentSymbol`). L1 discarded `result.symbol_table`; L2 threads it
/// through here so `documentSymbol` reuses the SAME analysis the debounced
/// `publishDiagnostics` already ran — no second analysis pass (Doc 26 §8
/// L2: "One analysis feeds both"), no parallel symbol extraction.
///
/// This is an **additive, behaviour-neutral** change: `analyze()` makes the
/// identical `analyze_with_source` call in the identical order and merges
/// import diagnostics identically — it merely *keeps* the `symbol_table`
/// field L1 dropped on the floor. The `.diagnostics` projection is
/// byte-identical to L1 (the §11.32 reuse-seam invariant is intact).
#[derive(Clone, Debug)]
pub struct Analysis {
    /// Diagnostics in `fsm check` order: parse + symbol + checks (from
    /// `analyze_with_source`) then appended import-security diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// The analyzer's per-machine ordered symbol tables (events / externs /
    /// consts / enums / context fields / states / regions, each with
    /// `Span`s; `StateEntry` additionally carries `container_path` + a
    /// coarse `shape`). Doc 26 §5's `documentSymbol` seam — the analyzer's
    /// own doc-comment (`lower/mod.rs:66`) declares the LSP a sanctioned
    /// consumer. Carried straight off the single `analyze_with_source`
    /// result; the LSP re-runs no symbol extraction (Doc 26 §8 L2).
    pub symbol_table: SymbolTable,
}

/// Run the **exact** `fsm check` pipeline over an in-memory buffer.
///
/// `uri_path` is the filesystem path the document URI maps to (used only
/// for the import-security containment boundary, exactly as `check.rs`
/// uses the CLI argument path). `src` is the authoritative editor buffer.
pub fn analyze(src: &str, uri_path: &Path) -> Analysis {
    // 1. parse — `check.rs:46`
    let pr = parse(src);

    // 2. workspace root — `check.rs:59` (re-implemented identically; see
    //    module note on why the CLI's `pub(crate)` helper is not called).
    let workspace_root = workspace_root_for(uri_path);

    // 3. import-security pass — `check.rs:60` (must run BEFORE analyze so
    //    the merge order matches the CLI exactly).
    let mut import_diags = security_check_imports(&pr, uri_path, &workspace_root);

    // 4. semantic analysis — `check.rs:61`. L2: we KEEP `result.symbol_table`
    //    (L1 discarded it). `documentSymbol` consumes it from THIS single
    //    run — there is no second analysis pass and no parallel symbol
    //    extraction (Doc 26 §8 L2: "One analysis feeds both"). `result.ir`
    //    is still discarded — it is an L3+ (hover/inlay) seam, not L2.
    let result = analyze_with_source(&pr, &uri_path.to_string_lossy(), src);
    let mut diagnostics = result.diagnostics;
    let symbol_table = result.symbol_table;

    // 5. merge — `check.rs:65`: import diagnostics appended after the
    //    parse/analyze diagnostics, sharing the same downstream renderer
    //    (here: the LSP Span->Range projection). This merge is byte-identical
    //    to L1 — keeping `symbol_table` above does not perturb it.
    diagnostics.append(&mut import_diags);

    Analysis {
        diagnostics,
        symbol_table,
    }
}

/// Discover the workspace root for `path` by walking parent directories
/// for an `fsm.toml`, falling back to the file's own directory.
///
/// Behaviourally identical to `fsm_cli::safe_io::workspace_root_for` as
/// used at `check.rs:59` — the most permissive root that still rejects
/// every out-of-workspace import (everything outside the returned
/// directory is still rejected by [`resolve_import`]). Re-implemented
/// rather than called because the CLI helper is `pub(crate)` inside the
/// binary crate and is correctly NOT public API (module note).
fn workspace_root_for(path: &Path) -> PathBuf {
    let start = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut dir: &Path = &start;
    loop {
        if dir.join("fsm.toml").is_file() {
            return dir.to_path_buf();
        }
        match dir.parent() {
            Some(p) if !p.as_os_str().is_empty() => dir = p,
            _ => return start.clone(),
        }
    }
}

/// Run every `import "..."` declaration through [`resolve_import`] so a
/// workspace-relative symlink that escapes the workspace squiggles
/// in-editor identically to `fsm check`.
///
/// This is a line-for-line behavioural mirror of
/// `fsm_cli::cmd::check::security_check_imports` (`check.rs:157-195`):
/// `OutsideWorkspace` is the only hard error surfaced here; `BadShape`
/// (already diagnosed by the parser's shape check) and `Unresolved` (a
/// sibling `.fsm` may not exist yet in author workflows) are intentionally
/// silent — matching the CLI exactly so the LSP can never disagree with
/// `fsm check`.
pub fn security_check_imports(
    pr: &ParseResult,
    file_path: &Path,
    workspace_root: &Path,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let file = AstFile::cast(pr.syntax()).expect("root node is always FILE");
    for imp in file.imports() {
        let Some(raw) = imp.path() else { continue };
        // Recover the path span via the IMPORT_DECL subtree — first
        // StringLiteral token (identical recovery to `check.rs:166-177`).
        let span = imp
            .syntax()
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::StringLiteral)
            .map(|t| {
                let r = t.text_range();
                Span::new(usize::from(r.start()), usize::from(r.end()))
            })
            .unwrap_or_else(|| Span::new(0, 0));
        match resolve_import(workspace_root, file_path, &raw) {
            Ok(_) => {}
            Err(ImportError::BadShape { .. }) => {
                // Already diagnosed by the parser's shape check — silent
                // here to avoid duplicate noise (matches `check.rs:180`).
            }
            Err(ImportError::Unresolved) => {
                // Not flagged — sibling file may not exist yet
                // (matches `check.rs:184`).
            }
            Err(err @ ImportError::OutsideWorkspace) => {
                out.push(err.into_diagnostic(span));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_source_yields_no_diagnostics() {
        let src = "language fsm 2.0\n\nmachine M {\n  initial S\n  state S {}\n}\n";
        let a = analyze(src, Path::new("/tmp/m.fsm"));
        assert!(
            a.diagnostics.is_empty(),
            "expected clean parse, got {:?}",
            a.diagnostics
        );
    }

    #[test]
    fn missing_initial_surfaces_e0107_same_as_cli() {
        // `machine` with a state but no `initial` -> the analyzer emits
        // FSM-E0107. This is the SAME `analyze_with_source` call the CLI
        // makes, so the code must match `fsm check` byte-for-byte.
        let src = "language fsm 2.0\n\nmachine M {\n  state S {}\n}\n";
        let a = analyze(src, Path::new("/tmp/m.fsm"));
        assert!(
            a.diagnostics
                .iter()
                .any(|d| format!("{}", d.code) == "FSM-E0107"),
            "expected FSM-E0107, got {:?}",
            a.diagnostics
                .iter()
                .map(|d| format!("{}", d.code))
                .collect::<Vec<_>>()
        );
    }
}
