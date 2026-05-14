//! Guard / expression / statement evaluator.
//!
//! Split into three layers so that the central RTC step in
//! [`crate::interpreter`] can call them independently:
//!
//! 1. [`expr`] — pure expression evaluation, including guard evaluation.
//! 2. [`stmt`] — statement execution (mutates context, enqueues events).
//! 3. [`extern_registry`] — host-side extern stubs.

pub mod arith;
pub mod expr;
pub mod extern_registry;
pub mod stmt;

pub use expr::{eval_expr, eval_field_ref, eval_guard, EvalCtx, EvalError};
pub use extern_registry::{ExternFn, ExternRegistry};
pub use stmt::{execute_statement, execute_statements, ExecOutcome, StmtCtx, StmtError};
