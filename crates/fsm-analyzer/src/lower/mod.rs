//! AST → IR lowering — the analyzer's main entry point.
//!
//! Per Doc 09 §1 design principle 5, the IR MAY be partial when errors are
//! present; we lower as much as we structurally can and surface diagnostics
//! cumulatively. The lowerer:
//!
//! - assigns deterministic IDs to every machine, state, region, event,
//!   extern, and transition;
//! - records `kind` on every transition (External / Local / Internal /
//!   Completion);
//! - resolves timer durations to a `u32` and rejects 0-duration entries via
//!   the [`crate::checks::timer`] pass before lowering;
//! - applies spec defaults for clauses the source omits: a transition with
//!   no `priority` clause lowers to [`fsm_ir::DEFAULT_TRANSITION_PRIORITY`]
//!   (100, per Doc 04 §8.6 / Doc 09 §6 — *not* 0; see Doc 00 §11.27 /
//!   W7-FU-2), `queue.capacity: 16`, `overflow: Assert`. (`RegionObject`'s
//!   own `priority` field is a distinct concern — region dispatch order for
//!   parallel states, Doc 09 §5 — and defaults to 0 separately.);
//! - computes `effective_lca` on each transition via the [`crate::lca`]
//!   helpers (the value is not stored in the IR — codegen recomputes it from
//!   `(source, target, kind)`).
//!
//! ## Structure (AD-3, 2026-05-15)
//!
//! This used to be one 2200-line file built around a `LoweringCtx`
//! god-object: 32 methods, 27 of them `&mut self` purely to bump three ID
//! counters (Audit C LCOM cluster A + B + C, mutation density ~18%). It is
//! now split into focused, low-mutation units — **a pure structure
//! refactor, behaviour byte-identical** (the four example IRs and the full
//! suite are unchanged pre/post; a snapshot guard pins them):
//!
//! - [`ids::IdMinter`] — the only mutable participant: the three monotonic
//!   counters + `machine_name`.
//! - [`loc::LocCtx`] — immutable file/src → `SourceLocation` helpers.
//! - [`machine`] / [`state`] / [`expr`] — free `lower_*` functions that
//!   thread `&mut IdMinter` + `&LocCtx` explicitly. No god-object `self`.
//! - [`hash`] — the in-tree SHA-256 + `sourceHash` label.

mod expr;
mod hash;
mod ids;
mod loc;
mod machine;
mod state;

use fsm_diagnostics::Diagnostic;
use fsm_ir::{DiagnosticObject, Ir, CURRENT_IR_VERSION};
use fsm_parser::ast::{self};
use fsm_parser::ParseResult;

use crate::checks;
use crate::symbol_table::SymbolTable;

use self::hash::source_hash;
use self::machine::lower_machine;

/// Result of running the full analyzer pipeline.
#[derive(Clone, Debug)]
pub struct AnalysisResult {
    /// Lowered IR. `None` only on catastrophic errors that block any
    /// meaningful structural output; partial IR is preferred.
    pub ir: Option<Ir>,
    /// Cumulative diagnostics — analyzer + lowerer + parser diagnostics
    /// merged in source-emission order.
    pub diagnostics: Vec<Diagnostic>,
    /// Symbol table — exposed so downstream tools (LSP completion,
    /// documentation generators) can re-use it without rebuilding.
    pub symbol_table: SymbolTable,
}

/// Run the analyzer end-to-end. Parser-level diagnostics are preserved.
pub fn analyze(parse_result: &ParseResult) -> AnalysisResult {
    analyze_with_source(parse_result, "<source>", "")
}

/// Same as [`analyze`] but takes a source path and the raw source text so
/// `SourceLocation` values carry line/column data and the IR's `sourceHash`
/// is content-derived.
pub fn analyze_with_source(parse_result: &ParseResult, file: &str, src: &str) -> AnalysisResult {
    let mut diagnostics = parse_result.errors.clone();
    let ast = parse_result.ast();
    let (st, sym_diags) = SymbolTable::build(&ast);
    diagnostics.extend(sym_diags);

    let mut check_diags = Vec::new();
    checks::run_all(&ast, &st, &mut check_diags);
    diagnostics.extend(check_diags);

    let ir = lower_file(&ast, &st, file, src, &diagnostics);
    debug_assert_ir_schema(&ir);
    AnalysisResult {
        ir: Some(ir),
        diagnostics,
        symbol_table: st,
    }
}

/// Internal-invariant gate (PD-3): the IR the analyzer just produced MUST
/// satisfy the canonical wire-format schema (`schema/ir/1.0.0/model.json`).
///
/// A violation is a **lowering bug**, never user error — the analyzer
/// already rejected malformed *source* with diagnostics; reaching here with
/// schema-invalid *IR* means a producer defect. We `debug_assert!` with the
/// concrete schema error path so a malformed-IR bug fails loudly at the
/// analyzer boundary instead of corrupting codegen / the simulator three
/// crates downstream — the exact failure mode of the Wave-1.9
/// `region.initial` contract bug.
///
/// `#[cfg(debug_assertions)]`: dev + `cargo test` builds enforce it; an
/// optimized `--release` build compiles this to nothing (zero cost), the
/// same discipline `debug_assert!` itself uses. Feature-gated on
/// `schema-validate` so a `--no-default-features` build drops the
/// `jsonschema` dependency.
#[cfg(all(debug_assertions, feature = "schema-validate"))]
fn debug_assert_ir_schema(ir: &Ir) {
    if let Err(errors) = fsm_ir::validate_ir_against_schema(ir) {
        // Cap the rendered list so a structurally-broken IR doesn't dump
        // thousands of lines; the first few violations localize the bug.
        let shown: Vec<&String> = errors.iter().take(8).collect();
        let extra = errors.len().saturating_sub(shown.len());
        let suffix = if extra > 0 {
            format!("\n  … and {extra} more violation(s)")
        } else {
            String::new()
        };
        debug_assert!(
            false,
            "INTERNAL: analyzer produced IR that violates \
             schema/ir/1.0.0/model.json — this is a lowering bug, not user \
             error. Schema violations:\n  {}{}",
            shown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n  "),
            suffix,
        );
    }
}

/// Release / `--no-default-features` no-op counterpart — see the
/// `cfg(debug_assertions)` variant above for the rationale. Kept as a
/// separate item (rather than an inline `cfg!`) so the gate adds literally
/// zero instructions to an optimized build.
#[cfg(not(all(debug_assertions, feature = "schema-validate")))]
#[inline(always)]
fn debug_assert_ir_schema(_ir: &Ir) {}

// ---------------------------------------------------------------------------
// Top-level lowering
// ---------------------------------------------------------------------------

fn lower_file(
    file: &ast::File,
    st: &SymbolTable,
    file_path: &str,
    src: &str,
    diagnostics: &[Diagnostic],
) -> Ir {
    let mut machines = Vec::new();
    for (idx, machine) in file.machines().enumerate() {
        if let Some(m_ir) = lower_machine(&machine, idx, st, file_path, src, file) {
            machines.push(m_ir);
        }
    }
    Ir {
        ir_version: CURRENT_IR_VERSION.to_string(),
        source_hash: source_hash(src),
        source_files: vec![file_path.to_string()],
        machines,
        diagnostics: diagnostics.iter().cloned().map(DiagnosticObject).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_ir::{StateNode, TransitionKind};

    fn parse(src: &str) -> fsm_parser::ParseResult {
        fsm_parser::parse(src)
    }

    #[test]
    fn lower_empty_machine_produces_ir() {
        let pr = parse("language fsm 2.0\nmachine M { }");
        let res = analyze(&pr);
        let ir = res.ir.unwrap();
        assert_eq!(ir.ir_version, "1.0.0");
        assert_eq!(ir.machines.len(), 1);
        assert_eq!(ir.machines[0].name, "M");
    }

    #[test]
    fn source_hash_changes_with_source() {
        let a = source_hash("language fsm 2.0");
        let b = source_hash("language fsm 2.1");
        assert_ne!(a, b);
        assert!(a.starts_with("sha256:"));
    }

    #[test]
    fn external_self_transition_marks_kind_external() {
        let pr =
            parse("language fsm 2.0\nmachine M { events { E } initial S state S { on E -> S } }");
        let res = analyze(&pr);
        let ir = res.ir.unwrap();
        let m = &ir.machines[0];
        let root_state = &m
            .root
            .states
            .iter()
            .find_map(|s| match s {
                StateNode::Simple(s) => Some(s),
                _ => None,
            })
            .unwrap();
        assert_eq!(root_state.transitions.len(), 1);
        assert!(matches!(
            root_state.transitions[0].kind,
            TransitionKind::External
        ));
    }
}
