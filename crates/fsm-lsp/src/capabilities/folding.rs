//! `textDocument/foldingRange` projection — Doc 26 §5 / §8 L2.
//!
//! Doc 14 §12 folding regions. **No analysis, no symbol table** — folding
//! is purely structural, so this is a single CST descendant walk over the
//! parse tree (Doc 26 §5 `foldingRange` row: "CST node ranges … pure tree
//! walk"). Every line number goes through L1's [`LineIndex`] in the
//! negotiated encoding — no second position converter (Doc 26 §4.1).
//!
//! ## Folded constructs (Doc 14 §12)
//!
//! | CST node / token | Doc 14 §12 row |
//! |---|---|
//! | `MACHINE_DECL` | "Full machine body" |
//! | `STATE_DECL` | "State body" / "Composite state body" |
//! | `REGION_DECL` | "Region body" |
//! | `CONTEXT_BLOCK` | "Context block" |
//! | `EVENTS_BLOCK` | events block (the analogous declaration block) |
//! | `BlockComment` / `DocComment` token | "Block comment" (`/* … */`) |
//!
//! Doc 14 §12 lists `composite NAME {` / `parallel NAME {` as distinct
//! rows; in this DSL there is **no** `composite`/`parallel` keyword — a
//! composite/parallel state *is* a `state NAME { … }` (resp. with nested
//! `region`s), i.e. a `STATE_DECL` whose body holds nested
//! `STATE_DECL`/`REGION_DECL` children (the grammar models all three as
//! `STATE_DECL`; Doc 04). Folding the `STATE_DECL`/`REGION_DECL` bodies
//! therefore covers the composite/parallel rows exactly — there is no
//! separate node kind to miss. The Doc 14 §12 "transition group" row is a
//! *semantic* grouping (contiguous `on E` runs) — it is **not** an L2
//! deliverable (Doc 26 §8 L2 scopes folding to composite/parallel/region
//! + multi-line constructs from CST ranges); it is deliberately left for a
//! later wave rather than half-implemented.
//!
//! A region is emitted only when it spans **more than one line** (a
//! single-line `state A {}` has nothing to fold; LSP clients ignore
//! degenerate one-line ranges and emitting them is noise).

use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind};

use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::position::{LineIndex, OffsetEncoding};

/// Compute the Doc 14 §12 folding ranges for one buffer.
///
/// `cst` is the parse-tree root; `line_index`/`text`/`encoding` are L1's
/// position boundary (same `LineIndex` the diagnostics path uses). The
/// returned ranges are in document order (the CST is walked preorder).
pub fn folding_ranges(
    cst: &SyntaxNode,
    line_index: &LineIndex,
    text: &str,
    encoding: OffsetEncoding,
) -> Vec<FoldingRange> {
    let mut out = Vec::new();
    for el in cst.descendants_with_tokens() {
        if let Some(n) = el.as_node() {
            if is_foldable_node(n.kind()) {
                push_fold(&mut out, n, None, line_index, text, encoding);
            }
        } else if let Some(t) = el.as_token() {
            if matches!(t.kind(), SyntaxKind::BlockComment | SyntaxKind::DocComment) {
                // A token has no `.children`, but the same byte-range →
                // line projection applies (a `/* … */` over ≥2 lines).
                let tr = t.text_range();
                push_range(
                    &mut out,
                    u32::from(tr.start()),
                    u32::from(tr.end()),
                    Some(FoldingRangeKind::Comment),
                    line_index,
                    text,
                    encoding,
                );
            }
        }
    }
    out
}

/// The structural node kinds whose body is foldable (Doc 14 §12).
fn is_foldable_node(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::MACHINE_DECL
            | SyntaxKind::STATE_DECL
            | SyntaxKind::REGION_DECL
            | SyntaxKind::CONTEXT_BLOCK
            | SyntaxKind::EVENTS_BLOCK
    )
}

/// Emit a fold for a structural node's full extent.
fn push_fold(
    out: &mut Vec<FoldingRange>,
    node: &SyntaxNode,
    kind: Option<FoldingRangeKind>,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) {
    let tr = node.text_range();
    push_range(
        out,
        u32::from(tr.start()),
        u32::from(tr.end()),
        kind,
        li,
        text,
        enc,
    );
}

/// Project a byte range `[start, end)` to a `FoldingRange`, skipping it
/// when it does not span at least two lines (LSP folding is line-based —
/// a single-line construct has nothing to collapse, and emitting a
/// `start_line == end_line` range is noise clients discard).
fn push_range(
    out: &mut Vec<FoldingRange>,
    start: u32,
    end: u32,
    kind: Option<FoldingRangeKind>,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) {
    // `end` is exclusive; the last *content* byte is `end - 1`. Folding to
    // the line of the closing token (`}` / `*/`) is the LSP convention
    // (the client keeps the end line visible, folding the lines between).
    let last = end.saturating_sub(1).max(start);
    let s = li.position(text, start, enc).line;
    let e = li.position(text, last, enc).line;
    if e <= s {
        return; // single-line — nothing to fold.
    }
    out.push(FoldingRange {
        start_line: s,
        start_character: None,
        end_line: e,
        end_character: None,
        kind,
        collapsed_text: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folds(src: &str) -> Vec<FoldingRange> {
        let pr = fsm_parser::parse(src);
        let li = LineIndex::new(src);
        folding_ranges(&pr.syntax(), &li, src, OffsetEncoding::Utf8)
    }

    #[test]
    fn machine_and_state_bodies_fold() {
        // machine spans lines 1..6 (0-based); state S lines 4..5.
        let src = "language fsm 2.0\n\nmachine M {\n  initial S\n  state S {\n  }\n}\n";
        let f = folds(src);
        // The machine body fold: starts on the `machine` line, ends on `}`.
        assert!(
            f.iter().any(|r| r.start_line == 2 && r.end_line == 6),
            "machine body fold 2..6 missing in {f:?}"
        );
        // The multi-line `state S { \n }` fold (lines 4..5).
        assert!(
            f.iter().any(|r| r.start_line == 4 && r.end_line == 5),
            "state S body fold 4..5 missing in {f:?}"
        );
    }

    #[test]
    fn single_line_state_is_not_folded() {
        let src = "language fsm 2.0\nmachine M {\n  initial S\n  state S {}\n}\n";
        let f = folds(src);
        // `state S {}` is entirely on line 3 → no fold for it. Only the
        // machine body (lines 1..4) folds.
        assert!(f.iter().all(|r| !(r.start_line == 3 && r.end_line == 3)));
        assert!(f.iter().any(|r| r.start_line == 1 && r.end_line == 4));
    }

    #[test]
    fn block_comment_folds_as_comment_kind() {
        let src = "language fsm 2.0\n/*\n multi\n line\n*/\nmachine M {}\n";
        let f = folds(src);
        let c = f
            .iter()
            .find(|r| r.kind == Some(FoldingRangeKind::Comment))
            .expect("block comment fold present");
        assert_eq!(c.start_line, 1, "/* on line 1");
        assert_eq!(c.end_line, 4, "*/ on line 4");
    }
}
