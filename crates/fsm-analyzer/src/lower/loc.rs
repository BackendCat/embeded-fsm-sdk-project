//! Source-location helpers — immutable for the whole lowering pass.
//!
//! AD-3 (2026-05-15): extracted from the former `LoweringCtx` god-object
//! (Audit C LCOM cluster B). These two methods only ever read `file` /
//! `src`; isolating them removes the last reason most lowerers held
//! `&mut self`. Bodies are a verbatim transcription of the old
//! `LoweringCtx::{loc, loc_span}` — identical `SourceLocation` output.

use fsm_diagnostics::Span;
use fsm_ir::SourceLocation;
// **W0 / Doc 29 §3.4 (R-4 leave-and-explain — transitive).** `LocCtx::loc`
// takes a `&SyntaxNode` purely to feed `util::span_of` (the pure positional
// byte-range bridge — R-4). There is no typed-AST equivalent for "the byte
// range of any node"; this is the parser's *public* CST type used for its
// intended positional purpose. Folding it would mean threading a typed
// wrapper through every lowerer's `loc(x.syntax())` call for zero clarity
// gain — the DRIFT-2 anti-pattern (Doc 00 §11.44/§11.49). Left-and-explained.
use fsm_parser::cst::SyntaxNode;

use crate::util::{compute_line_col, span_of};

/// Carries the file path + raw source so every emitted `SourceLocation`
/// gets line/column data. Constructed once per machine and shared
/// immutably by every lowerer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocCtx<'a> {
    file: &'a str,
    src: &'a str,
}

impl<'a> LocCtx<'a> {
    pub(crate) fn new(file: &'a str, src: &'a str) -> Self {
        Self { file, src }
    }

    pub(crate) fn loc(&self, n: &SyntaxNode) -> SourceLocation {
        let span = span_of(n);
        let (line, column) = compute_line_col(self.src, span.start);
        SourceLocation::new(self.file.to_string(), span, line, column)
    }

    pub(crate) fn loc_span(&self, span: Span) -> SourceLocation {
        let (line, column) = compute_line_col(self.src, span.start);
        SourceLocation::new(self.file.to_string(), span, line, column)
    }
}
