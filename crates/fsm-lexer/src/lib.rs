//! `fsm-lexer` — tokenizer for FSM-Lang.
//!
//! Per Doc 20 §3, this crate is zero-copy and `no_std`-friendly. It produces
//! `Token { kind, span }` pairs over a borrowed `&str`. No allocation per
//! token, no analysis, no recovery beyond emitting `TokenKind::Error(code)`
//! and advancing by the full UTF-8 width of the offending character.
//!
//! The lexical grammar follows `docs/04-DSL-Specification.md` §1:
//!
//! - §1.1 source encoding — UTF-8, BOM consumed silently.
//! - §1.2 identifiers — ASCII letter + `_` start, ASCII alnum + `_` continue.
//! - §1.3 literals — decimal, hex (`0x…`), binary (`0b…`), float, string.
//! - §1.4 comments — `//`, `/// (doc)`, `/* */` (does not nest).
//! - §1.5 keywords — 53 reserved words, lifted into `Kw*` token kinds.
//! - §1.6 operators and delimiters — including 2-char operators `->`, `~>`,
//!   `==`, `!=`, `<=`, `>=`, `&&`, `||`, `<<`, `>>`.
//! - §14 stable IDs — `@ident` form lexes as a single `StableId` token.
//!
//! Edge-case resolutions for items the spec left silent (per
//! `docs/00-Decisions-And-Reconciliation.md` §B fix to VALIDATION_REPORT 2.1
//! and 2.2) are documented at the top of `lexer.rs`.
//!
//! Diagnostic codes raised at the lexer level (FSM-E0001..FSM-E0004) come
//! from `fsm-diagnostics` and follow `docs/10-Diagnostic-Code-Catalog.md` §3.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

mod lexer;
mod token;

pub use fsm_diagnostics::{DiagnosticCode, SourceLocation, Span};
pub use lexer::{tokenize, Lexer};
pub use token::{Token, TokenKind};
