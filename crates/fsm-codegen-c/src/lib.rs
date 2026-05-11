//! `fsm-codegen-c` — C99 code generator for FSM Studio.
//!
//! Implements the IR → C99 emit specified by Doc 11 with the TL-normative
//! reconciliations from Doc 00:
//!
//! - B-08: parallel completion fires only when ALL regions reach Final.
//! - B-09: external self-transition LCA is `source.parent`, local/internal is
//!   `source` itself.
//! - B-10: switch-based dispatch walks leaf-to-root using a static
//!   `Motor_parent_table[]`.
//! - B-11: table-driven dispatch is two-phase — collect at most one
//!   transition per region, then execute all.
//! - B-14: history pseudo-states require a default target (analyzer enforced;
//!   codegen still emits the default-target restore path).
//! - §10.3 (Doc 00): HAL is mandatory; emit `#include "fsm_hal.h"` and call
//!   `fsm_hal_clock_now_ms()` for timer logic.
//! - §10.4 (Doc 00): generated files carry an `SPDX-License-Identifier`
//!   header, MIT default, overridable via `CodegenConfig.license_spdx`.
//!
//! Public surface follows Doc 00 §5.6 NIT: `EmittedFiles` is an extensible
//! `Vec<EmittedFile>` so future additions (e.g. CMakeLists, simulator hook
//! source) do not require new struct fields.
//!
//! Dependency rule: this crate depends on `fsm-ir` and `fsm-diagnostics`
//! ONLY (Doc 23 §4). It MUST NOT depend on `fsm-parser` or `fsm-analyzer`.

#![forbid(unsafe_code)]
// Codegen emits a lot of string templates. The push_str("\n") /
// push_str(&format!(...)) patterns are deliberate — they keep emit code
// reading like the C they produce. The micro-optimisations clippy
// suggests (`push('\n')`, `write!()`) bury that structure.
#![allow(
    clippy::single_char_add_str,
    clippy::useless_format,
    clippy::needless_lifetimes,
    clippy::no_effect,
    clippy::only_used_in_recursion,
    clippy::field_reassign_with_default
)]

pub use fsm_ir::{
    BinaryOp, CastExpr, ChoiceBranch, ChoiceState, CmpOp, CompositeState, ConstDecl, ContextField,
    ContextSchema, DeferDecl, EnumVariantLit, EventObject, Expr, ExternObject, FieldRef,
    FinalState, ForkPseudo, GuardExpr, GuardOperand, HistoryKind, HistoryObject, InitialPseudo, Ir,
    JoinPseudo, JunctionState, Literal, MachineObject, OverflowPolicy as IrOverflowPolicy,
    ParallelState, Param, QueueConfig, RegionObject, SimpleState, SimpleState as SimpleStateObj,
    StateNode, Statement, SubmachineRef, TargetConfig, TimerKind, TimerObject, TransitionKind,
    TransitionObject, Trigger, Type, UnaryOp,
};

pub mod budget;
pub mod config;
pub mod emit;
pub mod expr;
pub mod parent_table;
pub mod state_index;
pub mod stmt;

pub use budget::{compute_budget, MemoryBudget};
pub use config::{CodegenConfig, DispatchStrategy, OverflowPolicy, TargetProfile};
pub use emit::{emit, EmitError, EmittedFile, EmittedFiles, FileRole};
pub use parent_table::{build_parent_table, ParentTable};
pub use state_index::{build_state_index, StateIndex, ROOT_SENTINEL};
