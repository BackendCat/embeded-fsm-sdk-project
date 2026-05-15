//! `textDocument/references` — Doc 26 §5 / §8 L5 (single-file).
//!
//! Pure projection over the L5 [`crate::refs::ReferenceIndex`] (the ONE
//! new analysis, itself derived from the single `analyze()` — no second
//! pass, semantic-only). The cursor is resolved to a [`crate::refs::
//! SymbolKey`] via the SAME L3 `resolve_at` the index is built with, then
//! every recorded occurrence (decl + semantic uses) is projected to an LSP
//! [`Location`] through L1's authoritative `LineIndex` in the negotiated
//! encoding — **no second position converter** (the §11.32 DRIFT-2
//! boundary is untouched).
//!
//! `includeDeclaration` is honoured exactly: when `false`, the
//! declaration-name occurrence is filtered out and only use sites are
//! returned. Cursor not on a resolvable symbol (whitespace / keyword /
//! string / comment / unknown / cross-file name) → `None` (the
//! spec-correct "no references", never a panic, never a textual guess).
//! Every returned range lies in *this* buffer (occurrences are byte ranges
//! of the same parse), so each `Location.uri` is the request URI by
//! construction — single-file only, cross-file is v1.3 (Doc 26 §4.6/§9).

use tower_lsp::lsp_types::{Location, Url};

use fsm_analyzer::symbol_table::SymbolTable;
use fsm_parser::cst::SyntaxNode;

use crate::position::{LineIndex, OffsetEncoding};
use crate::refs::ReferenceIndex;

/// Resolve the identifier at `byte` and return every reference `Location`
/// (decl + semantic uses), honouring `include_declaration`. `None` when
/// the cursor is not on a resolvable in-file symbol.
#[allow(clippy::too_many_arguments)]
pub fn references(
    table: &SymbolTable,
    index: &ReferenceIndex,
    cst: &SyntaxNode,
    uri: &Url,
    byte: u32,
    include_declaration: bool,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<Vec<Location>> {
    // Semantic resolution only — `key_at` returns `Some` ONLY for a
    // resolvable use site or a decl-name token (never a string/comment/
    // keyword/whitespace position).
    let key = ReferenceIndex::key_at(table, cst, byte)?;
    let occ = index.occurrences(&key);
    if occ.is_empty() {
        return None;
    }
    let locations: Vec<Location> = occ
        .iter()
        .filter(|o| include_declaration || !o.is_decl)
        .map(|o| Location {
            uri: uri.clone(),
            range: li.range(text, o.span, enc),
        })
        .collect();
    Some(locations)
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
    fn references_returns_decl_plus_all_semantic_uses() {
        // `B` declared once, used in `initial`? no — used in 3 transitions.
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {\n    on GO -> B\n  }\n  state C {\n    on GO -> B\n  }\n}\n";
        let (t, idx, cst, li, uri) = setup(src);
        // Cursor on the `B` decl name.
        let cur = at(src, "state B", 6);
        let with_decl = references(
            &t,
            &idx,
            &cst,
            &uri,
            cur,
            true,
            &li,
            src,
            OffsetEncoding::Utf8,
        )
        .expect("B has references");
        // decl + 3 transition-target uses = 4.
        assert_eq!(with_decl.len(), 4, "decl + 3 uses, got {with_decl:?}");
        for l in &with_decl {
            assert_eq!(l.uri, uri, "single-file");
        }
        // includeDeclaration=false drops exactly the decl → 3.
        let no_decl = references(
            &t,
            &idx,
            &cst,
            &uri,
            cur,
            false,
            &li,
            src,
            OffsetEncoding::Utf8,
        )
        .expect("B uses");
        assert_eq!(no_decl.len(), 3, "uses only, got {no_decl:?}");
    }

    #[test]
    fn references_none_on_keyword_or_whitespace() {
        let src = "language fsm 2.0\nmachine M {\n  initial A\n  state A {}\n}\n";
        let (t, idx, cst, li, uri) = setup(src);
        let kw = references(
            &t,
            &idx,
            &cst,
            &uri,
            at(src, "state A", 1),
            true,
            &li,
            src,
            OffsetEncoding::Utf8,
        );
        assert!(kw.is_none(), "keyword → None");
    }
}
