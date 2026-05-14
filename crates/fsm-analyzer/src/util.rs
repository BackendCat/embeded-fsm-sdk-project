//! Small utilities shared across the analyzer pipeline.

use fsm_diagnostics::{SourceLocation, Span};
use fsm_parser::cst::SyntaxNode;

/// Convert a rowan `TextRange` on `node` to the half-open byte [`Span`] used
/// by the diagnostic foundation.
pub fn span_of(node: &SyntaxNode) -> Span {
    let r = node.text_range();
    Span::new(u32::from(r.start()) as usize, u32::from(r.end()) as usize)
}

/// Build a coarse [`SourceLocation`] for `node` rooted at `file`. Line/column
/// values are not computed here — they require the original source bytes,
/// which the IR-lowering caller threads in via [`compute_line_col`].
pub fn loc_of(node: &SyntaxNode, file: &str) -> SourceLocation {
    SourceLocation::new(file.to_string(), span_of(node), 0, 0)
}

/// Compute a (line, column) pair for byte offset `pos` in `src`. Lines are
/// 1-indexed; columns are 1-indexed. Used by the lowerer to enrich the IR
/// `loc` fields without pulling in `fsm-lexer` at runtime.
pub fn compute_line_col(src: &str, pos: usize) -> (u32, u32) {
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for (i, b) in src.bytes().enumerate() {
        if i >= pos {
            break;
        }
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Same as [`loc_of`] but populates line/column using `src`.
pub fn loc_of_with_src(node: &SyntaxNode, file: &str, src: &str) -> SourceLocation {
    let span = span_of(node);
    let (line, column) = compute_line_col(src, span.start);
    SourceLocation::new(file.to_string(), span, line, column)
}
