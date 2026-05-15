//! `fsm-ir` — canonical Intermediate Representation for FSM Studio.
//!
//! Per Doc 20 §6, this crate is pure data: structs, enums, JSON I/O, and a
//! tree-walking visitor. No business logic. Codegen and the simulator
//! consume this IR. Foundation types — `Span`, `SourceLocation`,
//! `Diagnostic`, `Severity`, `DiagnosticCode` — are re-exported from
//! [`fsm_diagnostics`] (Doc 00 §7.1) so downstream crates have one import.
//!
//! Wire format is documented in `docs/09-Canonical-IR-Schema.md` and the
//! companion JSON Schema at `schema/ir/1.0.0/model.json`. Reconciler
//! additions (`const`, `import`, `feature`, `queue`, `target`, `cast`,
//! `enum_variant`, `kind` on transitions) come from `docs/00-Decisions-
//! And-Reconciliation.md` §7.4.

#![forbid(unsafe_code)]

pub use fsm_diagnostics::{Diagnostic, DiagnosticCode, Severity, SourceLocation, Span};

pub mod json;
pub mod lca;
pub mod model;
pub mod visitor;

pub use json::{from_json, from_reader, to_json, to_writer, IrJsonError};
#[cfg(feature = "schema-validate")]
pub use json::{validate_ir_against_schema, IR_SCHEMA_JSON};
pub use lca::{ancestors, effective_lca, lca_inclusive, ParentResolver};
pub use model::*;
pub use visitor::{walk_ir, walk_machine, walk_region, walk_state, IrVisitor};
