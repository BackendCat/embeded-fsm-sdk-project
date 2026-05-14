//! `fsm-parser` — recursive-descent parser over a rowan CST with a typed
//! AST view on top.
//!
//! The grammar follows `docs/04-DSL-Specification.md` §§2–13 (the
//! "Normative Grammar Summary" in §13 is authoritative). Expressions use a
//! Pratt operator-precedence climber driven by the binding-power table in
//! §8.7.1 — per `docs/00-Decisions-And-Reconciliation.md` §B-05, the EBNF
//! in §8.7 is informational; §8.7.1 is the source of truth.
//!
//! The parser:
//! - Always produces a complete CST + diagnostic list — it never panics or
//!   aborts on malformed input. Recovery uses sync-set panic mode (Doc 20
//!   §4.4).
//! - Preserves trivia (whitespace, comments) so the formatter and LSP
//!   incremental reparse can round-trip the source.
//! - Validates security-sensitive constructs at parse time per Doc 00
//!   §7.12 (G-02): import path traversal and opaque-type-string regex.
//!
//! Public API:
//!
//! ```text
//! parse(src) -> ParseResult { green, errors }
//!     ParseResult::syntax()      -> SyntaxNode    (rowan CST view)
//!     ParseResult::ast()         -> ast::File     (typed AST view)
//!     ParseResult::reconstructed_text() -> String (byte-exact)
//! ```

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
// We surface the unused-must-use lint as an explicit warn so accidental
// dropped `Result`s in the security validators are caught.
#![warn(unused_must_use)]

pub mod ast;
pub mod cst;
pub mod expr;
mod grammar;
pub mod import_resolver;
pub mod limits;
pub mod opaque_type_validator;
mod parse;
mod parser;
mod token_set;

pub use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};

pub use crate::cst::{
    syntax_kind_from_token, FsmLanguage, GreenNode, GreenNodeBuilder, GreenToken, SyntaxElement,
    SyntaxKind, SyntaxNode, SyntaxToken,
};
pub use crate::limits::ParseLimits;
pub use crate::parse::{parse, parse_with_limits, parse_with_tokens, ParseResult};
pub use crate::parser::{DepthGuard, ExprContext, Parser};
pub use crate::token_set::TokenSet;

pub use crate::ast::AstNode;
