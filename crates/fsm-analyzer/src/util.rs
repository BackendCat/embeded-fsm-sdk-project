//! Small utilities shared across the analyzer pipeline.

use fsm_diagnostics::{LineColUnit, SourceLocation, Span};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-4 — the Archetype-C leave-and-explain residual).**
// This file legitimately retains `fsm_parser::cst` for the *positional*
// helpers: `span_of(&SyntaxNode) -> Span` is pure rowan-positional
// (`node.text_range()` — the byte-range bridge to `fsm-diagnostics`, used by
// 79 `span_of(x.syntax())` call sites crate-wide); `loc_of` builds on it;
// `submachine_ref_is_nested` is a `.parent()`-walk structural predicate
// (the W2a P1-2 defence-in-depth). There is **no typed-AST equivalent** for
// "the byte range of any node" / "is this ref nested" and inventing one
// would be a positional-API reimplementation. These `pub` fns have **zero
// cross-crate callers** (verified: the only cross-crate `fsm_analyzer::util`
// use is `compute_line_col` in `fsm-lsp/hover.rs`, unrelated to the CST
// seam) — de-facto crate-internal, not a public contract. Threading a typed
// wrapper through 79 call sites would be massive non-behavioural churn for
// zero clarity gain — explicitly the DRIFT-2 anti-pattern (Doc 00
// §11.44/§11.49, the `LineIndex` precedent). Left-and-explained.
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

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

/// Walk file-level `const NAME = <expr>` declarations and fold each value
/// into a `(name, i64)` pair, in source order, resolving later consts
/// against earlier ones. A const whose value cannot be folded is skipped
/// (it is reported elsewhere by the reference-resolution pass).
///
/// **F-1 single-source-of-truth (Finding F-1, audit §1.1 / §6 items 1-2).**
/// This is THE one file-consts-aware const-fold table builder, consumed by
/// BOTH the timer-duration *check* (`checks::timer`, the bounds check) AND
/// the *lowerer*'s timer-duration fold (`lower::state::duration_ms` →
/// [`eval_const_expr_value`]). Before F-1 the lowerer had its own
/// `eval_i64` that handled only `EXPR_LITERAL`/`EXPR_UNARY`/`EXPR_PAREN`
/// and **omitted `EXPR_NAME_REF`**, while the check had a *separate*
/// `resolve_expr_value` that *did* resolve `EXPR_NAME_REF` against the file
/// consts — so `after CONST ms` (the Doc 02 §6 / Doc 04 §12 *mandated*
/// idiom) passed the bounds check but the lowerer silently returned `None`
/// and dropped the whole timer (a #110-class silent miscompile). The fix is
/// not to paste `EXPR_NAME_REF` into the lowerer (that would re-create the
/// asymmetry latently); it is to make lower≡check *by construction* — one
/// resolver, one consts table builder, here. The submachine-`is`-nested
/// helper below is the prior in-tree precedent for this "shared so the two
/// sites can never drift apart" doctrine.
pub fn file_const_table(file: &ast::File) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = Vec::new();
    for c in file.consts() {
        if let (Some(name), Some(ce)) = (c.name(), c.value()) {
            // `ConstExpr` wraps a single expression child.
            if let Some(expr) = ce.syntax().children().next() {
                if let Some(v) = eval_const_expr_value(&expr, &out) {
                    out.push((name, v));
                }
            }
        }
    }
    out
}

/// Fold a single expression CST node to an `i64` compile-time constant, or
/// `None` if it is not const-foldable. Resolves `EXPR_NAME_REF` against the
/// `consts` table (built by [`file_const_table`]).
///
/// **F-1 single-source-of-truth.** This is THE shared const evaluator the
/// timer-duration check and the lowerer both call (see [`file_const_table`]
/// for the full rationale). The accepted forms are *exactly* the four the
/// deliberately-shallow expression CST admits — integer literal, unary
/// `+`/`-`, parenthesised, and a `const` name reference. A duration that is
/// still `None` after this (a `ctx.`/`payload.` ref or non-const-foldable
/// arithmetic) is a genuine runtime-variable duration: the explicitly-
/// post-v1.0-deferred form the §2.3 rejecting diagnostic (`FSM-E0411`)
/// hard-errors on, rather than the lowerer silently dropping it.
pub fn eval_const_expr_value(expr: &SyntaxNode, consts: &[(String, i64)]) -> Option<i64> {
    match expr.kind() {
        SyntaxKind::EXPR_LITERAL => {
            let tok = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::IntLiteral | SyntaxKind::FloatLiteral))?;
            parse_int_literal_i64(tok.text())
        }
        SyntaxKind::EXPR_UNARY => {
            let op_tok = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::Minus | SyntaxKind::Plus))?;
            let inner = expr.children().next()?;
            let v = eval_const_expr_value(&inner, consts)?;
            match op_tok.kind() {
                SyntaxKind::Minus => Some(-v),
                _ => Some(v),
            }
        }
        SyntaxKind::EXPR_PAREN => {
            let inner = expr.children().next()?;
            eval_const_expr_value(&inner, consts)
        }
        SyntaxKind::EXPR_NAME_REF => {
            let name = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)?
                .text()
                .to_string();
            consts.iter().find(|(n, _)| n == &name).map(|(_, v)| *v)
        }
        _ => None,
    }
}

/// Is this `SUBMACHINE_REF` nested inside a composite or parallel state
/// (i.e. NOT a direct child of the machine/submachine root region)?
///
/// **v1.1 phase-audit P1-2.** A top-level `state X is Sub` is fully
/// implemented end-to-end (W2a–W2d, gcc+sim≡codegen verified). A `state X
/// is Sub` that appears as a *descendant* of a composite/parallel state is
/// NOT yet implemented: W2d's `collect_sub_refs` walks only the root region,
/// so a nested ref reaches codegen as a leaf whose `entry_/exit_` are called
/// but whose prototypes are suppressed → dangling calls → C that fails the
/// project's own mandated `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`.
///
/// The CST shape (W2a) makes the test exact: a `SUBMACHINE_REF` is a child
/// of a `STATE_DECL`; that `STATE_DECL` is *top-level* iff its parent is
/// directly a `MACHINE_DECL` or `SUBMACHINE_DECL` (the root region body —
/// `MachineDecl::states()`/`SubmachineDecl::states()` are direct children,
/// no body wrapper node). It is *nested* iff any ancestor between it and the
/// enclosing machine/submachine is another `STATE_DECL` (composite) or a
/// `REGION_DECL` (parallel region).
///
/// Shared by [`crate::checks::submachine`] (which emits the rejecting
/// diagnostic) and the lowerer (which, as defence-in-depth, refuses to lower
/// a nested ref to `StateNode::Submachine`) so the reject-site and the
/// don't-lower-site can never drift apart.
pub fn submachine_ref_is_nested(sref_node: &SyntaxNode) -> bool {
    debug_assert_eq!(sref_node.kind(), SyntaxKind::SUBMACHINE_REF);
    let Some(state_decl) = sref_node.parent() else {
        return false;
    };
    // Walk strictly *above* the enclosing STATE_DECL. If we meet another
    // STATE_DECL or a REGION_DECL before the machine/submachine boundary,
    // the ref is a descendant of a composite/parallel state.
    let mut cur = state_decl.parent();
    while let Some(node) = cur {
        match node.kind() {
            SyntaxKind::STATE_DECL | SyntaxKind::REGION_DECL => return true,
            SyntaxKind::MACHINE_DECL | SyntaxKind::SUBMACHINE_DECL => return false,
            _ => cur = node.parent(),
        }
    }
    false
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

/// Compute a (line, column) pair for byte offset `pos` in `src`. Lines and
/// columns are 1-indexed; columns count **bytes** (a non-ASCII char advances
/// the column by its UTF-8 length). Used by the lowerer to enrich the IR
/// `loc` fields without pulling in `fsm-lexer` at runtime.
///
/// **DRIFT-2 convergence (Doc 00 §11.3x).** This was a hand-rolled per-byte
/// loop duplicated against `fsm_cli::cmd::check::line_col`'s per-scalar loop;
/// it now delegates to the single shared core
/// [`fsm_diagnostics::compute_line_col`] with [`LineColUnit::Byte`], which is
/// that exact loop generalised over the counting unit. The byte-counting,
/// 1-based contract baked into the IR `SourceLocation` (hence the
/// deterministic C / `lower_split_byte_identity` fingerprint) is **unchanged**
/// — proven byte-identical: at every char boundary (the only offsets
/// `span_of`'s rowan `TextRange` can produce) the converged core's
/// `Σ len_utf8` equals the old per-byte tally. The signature is preserved so
/// callers and the public `util` surface are untouched.
pub fn compute_line_col(src: &str, pos: usize) -> (u32, u32) {
    fsm_diagnostics::compute_line_col(src, pos, LineColUnit::Byte)
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
