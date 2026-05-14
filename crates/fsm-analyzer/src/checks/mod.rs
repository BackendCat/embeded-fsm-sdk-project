//! Semantic-check orchestrator.
//!
//! Each `pub mod` underneath this file owns one check family — name
//! resolution, type checking, determinism analysis, completion semantics,
//! history defaults, timer literals, parallel-region structure, submachine
//! references, defer bitmask limits, import sanity.
//!
//! The orchestrator runs them in dependency order: name resolution first
//! (so subsequent passes know whether a reference resolves), then type
//! checking and structural checks, then determinism — which depends on
//! resolved guard expressions.

use fsm_diagnostics::Diagnostic;
use fsm_parser::ast::File;

use crate::symbol_table::SymbolTable;

pub mod completion;
pub mod defer;
pub mod determinism;
pub mod history;
pub mod import;
pub mod name_resolution;
pub mod parallel;
pub mod submachine;
pub mod timer;
pub mod type_check;

/// Run every semantic check against `file` using `st`. Appends diagnostics to
/// `out`. The function does **not** stop on the first error — diagnostics are
/// surfaced cumulatively per Doc 09 §1 design principle 5.
pub fn run_all(file: &File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    name_resolution::check(file, st, out);
    history::check(file, st, out);
    timer::check(file, st, out);
    parallel::check(file, st, out);
    submachine::check(file, st, out);
    defer::check(file, st, out);
    type_check::check(file, st, out);
    completion::check(file, st, out);
    determinism::check(file, st, out);
    import::check(file, st, out);
}
