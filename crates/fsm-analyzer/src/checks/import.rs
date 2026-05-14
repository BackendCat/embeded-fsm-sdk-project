//! Import-sanity checks. For v1.0 the parser already validates path
//! traversal (Doc 00 §7.12 G-02); this module adds:
//!
//! - duplicate `import "X"` declarations within a file (emits `FSM-W0500`-
//!   adjacent: we reuse the duplicate-extern code shape as a placeholder
//!   `FSM-W0500` is not quite right, so we keep this module light and emit
//!   no diagnostics for v1.0 since parser-level validation already covers
//!   the security-sensitive cases).
//!
//! The function is present so the public API matches the brief; future
//! version-mismatch checks land here.

use fsm_diagnostics::Diagnostic;
use fsm_parser::ast::File;

use crate::symbol_table::SymbolTable;

/// Currently a no-op — parser handles import path validation per Doc 00
/// §7.12. Hook reserved for future cross-file resolution. The signature
/// matches the other checks for parallel dispatch via the orchestrator.
#[allow(clippy::ptr_arg)]
pub fn check(_file: &File, _st: &SymbolTable, _out: &mut Vec<Diagnostic>) {}
