//! `textDocument/definition` — Doc 26 §5 / §8 L3 (single-file).
//!
//! Pure projection over the shared [`crate::capabilities::resolve`] seam:
//! resolve the identifier under the cursor to its declaration (via the
//! exact `SymbolTable::resolve_*` `fsm check` uses), then project the
//! declaration's byte `Span` to an LSP [`Location`] through L1's
//! `LineIndex` in the negotiated encoding. **No analysis here, no second
//! position converter** (Doc 26 §4.1 / §11.32 boundary intact).
//!
//! Single-file only (Doc 26 §4.6): the per-file `SymbolTable` has no
//! cross-file/project index, so a symbol that does not resolve in this
//! buffer yields `None` — the spec-correct "no result" (Doc 14 §8 graceful
//! single-file degradation). The handler **never** fabricates a location
//! and **never** points outside this file: cross-file goto is explicitly
//! v1.3 (Doc 26 §4.6/§9), and a wrong location is the silent-data-loss
//! cardinal sin. The decl `Span` always lies in *this* document (it is a
//! span of the same parsed buffer), so the returned `Location.uri` is the
//! request URI by construction.

use tower_lsp::lsp_types::{Location, Url};

use fsm_analyzer::symbol_table::SymbolTable;
use fsm_parser::cst::SyntaxNode;

use crate::capabilities::resolve::{decl_range, resolve_at};
use crate::position::{LineIndex, OffsetEncoding};

/// Resolve the identifier at `byte` to its declaration `Location`, or
/// `None` (cursor not on a resolvable in-file identifier — whitespace,
/// keyword, declaration site, unknown name, or a cross-file/`payload`
/// symbol the single-file index cannot resolve; Doc 14 §8 degradation).
pub fn goto_definition(
    table: &SymbolTable,
    cst: &SyntaxNode,
    uri: &Url,
    byte: u32,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<Location> {
    let resolved = resolve_at(table, cst, byte)?;
    // The decl span is a byte range of THIS buffer (the same parse the
    // symbol table was built from), so the declaration is always in the
    // requested document — never a synthesised cross-file URI.
    let range = decl_range(resolved.decl_span(), li, text, enc);
    Some(Location {
        uri: uri.clone(),
        range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn setup(src: &str) -> (SymbolTable, SyntaxNode, LineIndex, Url) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        let li = LineIndex::new(src);
        let uri = Url::parse("file:///tmp/t.fsm").unwrap();
        (a.symbol_table, cst, li, uri)
    }

    #[test]
    fn definition_returns_state_decl_range() {
        let src =
            "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {}\n}\n";
        let (t, cst, li, uri) = setup(src);
        let byte = (src.find("-> B").unwrap() + 3) as u32;
        let loc = goto_definition(&t, &cst, &uri, byte, &li, src, OffsetEncoding::Utf8)
            .expect("B resolves");
        assert_eq!(loc.uri, uri, "single-file: decl is in the same document");
        // The range must cover `state B {}` (line 7, 0-based).
        // src layout: line 7 = "  state B {}" -> decl starts col 2.
        assert_eq!(loc.range.start.line, 7);
        assert_eq!(loc.range.start.character, 2);
        let span_text = {
            let li2 = LineIndex::new(src);
            // Reconstruct the byte span the range maps to and confirm it is
            // the `state B {}` declaration (independent of the range math).
            let off = li2.offset(src, loc.range.start, OffsetEncoding::Utf8) as usize;
            &src[off..off + "state B {}".len()]
        };
        assert_eq!(span_text, "state B {}");
    }

    #[test]
    fn definition_none_for_unresolved_does_not_point_anywhere() {
        let src =
            "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> Ghost\n  }\n}\n";
        let (t, cst, li, uri) = setup(src);
        let byte = (src.find("-> Ghost").unwrap() + 3) as u32;
        assert!(
            goto_definition(&t, &cst, &uri, byte, &li, src, OffsetEncoding::Utf8).is_none(),
            "an undeclared ref must return None — never a fabricated Location"
        );
    }
}
