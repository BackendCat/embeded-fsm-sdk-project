//! `textDocument/prepareRename` + `textDocument/rename` — Doc 26 §5 / §8
//! L5 / risk-2 (the cardinal silent-data-loss risk in its most acute
//! form). Single-file only (cross-file is v1.3, Doc 26 §4.6/§9).
//!
//! ## Why every path here is conservative-by-construction
//!
//! `rename` rewrites the user's source. The whole safety argument is in
//! [`crate::refs`]: the edit set is **exactly** the
//! [`crate::refs::ReferenceIndex`] occurrences for the target — decl +
//! *semantically resolved* uses (each proven by the SAME L3 `resolve_at`
//! that mirrors `checks::name_resolution`), never a text match. A
//! same-spelled string-literal substring, comment word, or different-scope
//! symbol never entered the index, so it is **not** in the `WorkspaceEdit`
//! by construction (risk-2's headline guarantee).
//!
//! `prepareRename` is the up-front contract: it tells the client a token
//! is NOT renameable rather than letting a rename silently produce a
//! corrupting edit. It HARD-REJECTS machine names (codegen/ABI blast
//! radius, out of v1.2 scope), `@id`/state-id annotation strings, keywords
//! and contextual keywords, non-identifier cursors, and any position in a
//! string / comment / trivia — all via [`crate::refs::prepare_rename`],
//! whose resolution is purely semantic (a non-`Ident`, a keyword, a string
//! interior, a comment word, an `@id` literal is none of "a resolvable use
//! site or a decl-name token", so it is refused). `rename` additionally
//! rejects an invalid new identifier and an in-scope name collision with a
//! clear message and **no edit**.

use std::collections::HashMap;

use tower_lsp::lsp_types::{PrepareRenameResponse, TextEdit, Url, WorkspaceEdit};

use fsm_analyzer::symbol_table::SymbolTable;
use fsm_parser::cst::SyntaxNode;

use crate::position::{LineIndex, OffsetEncoding};
use crate::refs::{plan_rename, prepare_rename, ReferenceIndex};

/// `textDocument/prepareRename` — return the renameable **range** (the
/// bare identifier the editor will let the user edit) for a safely-
/// renameable user symbol, or `Err(message)` to tell the client up-front
/// it is not renameable (the LSP contract; never silent-allow → dangerous
/// edit — Doc 26 risk-2).
///
/// Returns:
/// - `Ok(Some(range))` — a state/event/extern/context-field name token;
/// - `Err(message)` — machine name / `@id` / keyword / string interior /
///   comment / trivia / non-identifier / unknown / cross-file-exposed.
pub fn prepare_rename_handler(
    table: &SymbolTable,
    cst: &SyntaxNode,
    byte: u32,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Result<Option<PrepareRenameResponse>, String> {
    match prepare_rename(table, cst, byte) {
        Ok((_key, name_span)) => {
            let range = li.range(text, name_span, enc);
            // `RangeWithPlaceholder` would also work; a bare `Range` is the
            // simplest spec-valid form and the placeholder defaults to the
            // current name (what every editor expects for an in-place
            // rename of an identifier).
            Ok(Some(PrepareRenameResponse::Range(range)))
        }
        Err(refusal) => Err(refusal.message()),
    }
}

/// `textDocument/rename` — build a [`WorkspaceEdit`] whose text edits are
/// **exactly** the [`ReferenceIndex`] occurrences (decl + semantic uses)
/// of the target, each rewritten to `new_name`. Rejects (returns
/// `Err(message)`, **no edit**) a non-renameable cursor, a machine, a
/// cross-file-exposed symbol, an invalid new identifier, or an in-scope
/// collision (Doc 26 §8 L5 / risk-2).
#[allow(clippy::too_many_arguments)]
pub fn rename_handler(
    table: &SymbolTable,
    index: &ReferenceIndex,
    cst: &SyntaxNode,
    uri: &Url,
    byte: u32,
    new_name: &str,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Result<WorkspaceEdit, String> {
    let spans = plan_rename(table, index, cst, byte, new_name).map_err(|e| e.message())?;
    let edits: Vec<TextEdit> = spans
        .iter()
        .map(|s| TextEdit {
            range: li.range(text, *s, enc),
            new_text: new_name.to_owned(),
        })
        .collect();
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(uri.clone(), edits);
    Ok(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn setup(src: &str) -> (SymbolTable, ReferenceIndex, SyntaxNode, LineIndex, Url) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        let idx = ReferenceIndex::build(&a.symbol_table, &cst);
        let li = LineIndex::new(src);
        let uri = Url::parse("file:///tmp/t.fsm").unwrap();
        (a.symbol_table, idx, cst, li, uri)
    }

    fn at(src: &str, needle: &str, plus: usize) -> u32 {
        (src.find(needle).expect("needle") + plus) as u32
    }

    #[test]
    fn prepare_rename_handler_range_for_state_reject_for_machine() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> A\n  }\n}\n";
        let (t, _idx, cst, li, _uri) = setup(src);
        let ok =
            prepare_rename_handler(&t, &cst, at(src, "-> A", 3), &li, src, OffsetEncoding::Utf8)
                .expect("state A renameable");
        match ok {
            Some(PrepareRenameResponse::Range(r)) => {
                // `state A {` decl-name `A` is on line 4 (0-based), col 8.
                assert_eq!((r.start.line, r.start.character), (4, 8));
                assert_eq!((r.end.line, r.end.character), (4, 9));
            }
            other => panic!("expected a Range, got {other:?}"),
        }
        let err = prepare_rename_handler(
            &t,
            &cst,
            at(src, "machine M", 8),
            &li,
            src,
            OffsetEncoding::Utf8,
        );
        assert!(err.is_err(), "machine name must be rejected up-front");
    }

    #[test]
    fn rename_handler_edits_are_exactly_the_semantic_set() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {\n    on GO -> B\n  }\n}\n";
        let (t, idx, cst, li, uri) = setup(src);
        let we = rename_handler(
            &t,
            &idx,
            &cst,
            &uri,
            at(src, "state B", 6),
            "Done",
            &li,
            src,
            OffsetEncoding::Utf8,
        )
        .expect("safe rename");
        let edits = &we.changes.as_ref().unwrap()[&uri];
        // decl + `-> B` (in A) + `-> B` (self in B) = 3.
        assert_eq!(edits.len(), 3, "exact edit count, got {edits:?}");
        for e in edits {
            assert_eq!(e.new_text, "Done");
        }
    }

    #[test]
    fn rename_handler_rejects_collision_with_no_edit() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {}\n}\n";
        let (t, idx, cst, li, uri) = setup(src);
        let res = rename_handler(
            &t,
            &idx,
            &cst,
            &uri,
            at(src, "-> B", 3),
            "A", // collides with the existing state `A`
            &li,
            src,
            OffsetEncoding::Utf8,
        );
        assert!(res.is_err(), "collision must be rejected");
        assert!(
            res.unwrap_err().contains("already exists"),
            "rejection message must explain the collision"
        );
    }
}
