//! Small utilities shared across the analyzer pipeline.

use fsm_diagnostics::{SourceLocation, Span};
use fsm_parser::ast;
use fsm_parser::cst::SyntaxNode;

/// Yield every [`ast::StateDecl`] reachable from `m` — top-level states,
/// region states, and every transitive nested state — in pre-order
/// (parent before children).
///
/// **R2.1 dedup (2026-05-15)**: this previously lived as three byte-for-byte
/// identical implementations in `checks/completion.rs`, `checks/defer.rs`,
/// `checks/determinism.rs`. Per `docs/AUDIT_C_QUALITY_2026_05_14.md`.
/// Behavior is identical — the existing analyzer tests verify pre/post
/// equivalence by passing unchanged.
pub fn walk_all_states(m: &ast::MachineDecl) -> Vec<ast::StateDecl> {
    let mut out = Vec::new();
    for s in m.states() {
        out.push(s.clone());
        collect_nested_states(&s, &mut out);
    }
    for r in m.regions() {
        for s in r.states() {
            out.push(s.clone());
            collect_nested_states(&s, &mut out);
        }
    }
    out
}

/// Inner recursion for [`walk_all_states`]. Collects nested states and
/// region states under `s` into `out`.
pub fn collect_nested_states(s: &ast::StateDecl, out: &mut Vec<ast::StateDecl>) {
    for child in s.nested_states() {
        out.push(child.clone());
        collect_nested_states(&child, out);
    }
    for r in s.regions() {
        for nested in r.states() {
            out.push(nested.clone());
            collect_nested_states(&nested, out);
        }
    }
}

/// Parse a DSL integer literal token text into an `i64`, accepting:
///
/// - decimal: `42`, `-1`
/// - hex: `0x2A`, `0X2a`
/// - binary: `0b1010`, `0B1010`
/// - underscore separators anywhere (`1_000_000`, `0x_FF`)
/// - surrounding ASCII whitespace
///
/// Returns `None` for malformed input or overflow. The mirrored
/// [`parse_int_literal_i128`] uses the same syntax and is the right choice
/// where range checks need to detect overflow above the `i64` range
/// (notably the type-check unsigned-bound logic).
///
/// **R2.2 dedup (2026-05-15)**: this previously lived as five copy-paste
/// implementations across `lower.rs` (×2), `checks/timer.rs`,
/// `checks/type_check.rs`, `checks/determinism.rs`. Per
/// `docs/AUDIT_C_QUALITY_2026_05_14.md`. Behavior is identical to the
/// inlined versions, proven by the existing analyzer tests staying green.
pub fn parse_int_literal_i64(text: &str) -> Option<i64> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let cleaned = cleaned.trim();
    if let Some(rest) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        return i64::from_str_radix(rest, 16).ok();
    }
    if let Some(rest) = cleaned
        .strip_prefix("0b")
        .or_else(|| cleaned.strip_prefix("0B"))
    {
        return i64::from_str_radix(rest, 2).ok();
    }
    cleaned.parse::<i64>().ok()
}

/// `i128` flavor of [`parse_int_literal_i64`] — same syntax, wider range.
/// Used by the type-check unsigned-bound logic that needs to detect
/// out-of-range values for `u64` literals (which an `i64` parser can't
/// represent without ambiguity).
pub fn parse_int_literal_i128(text: &str) -> Option<i128> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let cleaned = cleaned.trim();
    if let Some(rest) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        return i128::from_str_radix(rest, 16).ok();
    }
    if let Some(rest) = cleaned
        .strip_prefix("0b")
        .or_else(|| cleaned.strip_prefix("0B"))
    {
        return i128::from_str_radix(rest, 2).ok();
    }
    cleaned.parse::<i128>().ok()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// R2.2 micro-test: every behavior the 5 inlined copies had — decimal,
    /// hex (lower/upper), binary (lower/upper), underscores, leading/trailing
    /// whitespace, malformed input — must be preserved.
    #[test]
    fn parse_int_literal_i64_covers_all_inlined_call_sites() {
        // Decimal
        assert_eq!(parse_int_literal_i64("0"), Some(0));
        assert_eq!(parse_int_literal_i64("42"), Some(42));
        assert_eq!(parse_int_literal_i64("-1"), Some(-1));
        // Hex (both cases)
        assert_eq!(parse_int_literal_i64("0x2A"), Some(42));
        assert_eq!(parse_int_literal_i64("0X2a"), Some(42));
        assert_eq!(parse_int_literal_i64("0xff"), Some(255));
        // Binary (both cases)
        assert_eq!(parse_int_literal_i64("0b1010"), Some(10));
        assert_eq!(parse_int_literal_i64("0B1010"), Some(10));
        // Underscore separators
        assert_eq!(parse_int_literal_i64("1_000_000"), Some(1_000_000));
        assert_eq!(parse_int_literal_i64("0xFF_FF"), Some(0xFFFF));
        // Whitespace tolerance
        assert_eq!(parse_int_literal_i64("  42 "), Some(42));
        // Malformed → None
        assert_eq!(parse_int_literal_i64("0xZZ"), None);
        assert_eq!(parse_int_literal_i64("not a number"), None);
        assert_eq!(parse_int_literal_i64(""), None);
        // Overflow → None
        assert_eq!(parse_int_literal_i64("99999999999999999999"), None);
    }

    #[test]
    fn parse_int_literal_i128_extends_range_with_same_syntax() {
        // Same syntax cases as i64.
        assert_eq!(parse_int_literal_i128("0xFF"), Some(255));
        assert_eq!(parse_int_literal_i128("0b101"), Some(5));
        assert_eq!(parse_int_literal_i128("1_234"), Some(1_234));
        // i64::MAX + 1 fits in i128.
        assert_eq!(
            parse_int_literal_i128("9223372036854775808"),
            Some(9_223_372_036_854_775_808_i128),
        );
        // u64::MAX fits as i128.
        assert_eq!(
            parse_int_literal_i128("18446744073709551615"),
            Some(18_446_744_073_709_551_615_i128),
        );
        assert_eq!(parse_int_literal_i128("bad"), None);
    }
}
