//! `fsm-analyzer` — semantic analysis + AST→IR lowering.
//!
//! Pipeline per Doc 20 §5: parse → symbol-table-build → semantic-checks →
//! lowering. The crate consumes [`fsm_parser::ParseResult`] (typed AST view
//! over a rowan CST) and produces [`fsm_ir::Ir`] plus a
//! [`Vec<fsm_diagnostics::Diagnostic>`].
//!
//! Public entry point is [`analyze`] (path-agnostic) or
//! [`analyze_with_source`] (carries file path + raw source so the IR's
//! `sourceHash`, `sourceFiles`, and per-node `loc.{line, column}` populate).
//!
//! Reconciler decisions encoded here:
//! - **B-06** transition `kind` discriminator on every transition.
//! - **B-07** completion guards ALLOWED; no E0301 emitted.
//! - **B-09** [`effective_lca`] for self-transitions.
//! - **B-13** `FSM-E0410` for `after 0 ms`.
//! - **B-14** `FSM-E0111` when a history pseudo-state has no `default ->`.
//! - **G-08 / v1.1** `defer EVENT` lowered to the IR defer set; runtime
//!   support shipped in codegen-c + simulator (Doc 08 §10). `FSM-E0903`
//!   (the v1.0 "not supported" stopgap) is retired; only `FSM-E0310`
//!   (defer-vs-explicit-transition conflict) remains for defer.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub use fsm_diagnostics::{Diagnostic, DiagnosticCode, Severity, Span};
pub use fsm_ir::{Ir, MachineObject};

pub mod checks;
pub mod lca;
pub mod lower;
pub mod scope;
pub mod symbol_table;
pub mod util;

pub use lca::{effective_lca, lca_inclusive, LcaIndex};
pub use lower::{analyze, analyze_with_source, AnalysisResult};
pub use symbol_table::SymbolTable;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_language_decl_yields_no_machines() {
        let pr = fsm_parser::parse("language fsm 2.0");
        let res = analyze(&pr);
        assert!(res.ir.unwrap().machines.is_empty());
    }
}
