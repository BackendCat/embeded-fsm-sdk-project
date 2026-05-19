//! C99 file emission engine.
//!
//! Public entry: [`emit`]. For each machine in the IR, produces a tuple of
//! generated files (header, source, impl header, conf header) plus a single
//! shared HAL header per call.
//!
//! File ordering: HAL first (other generated headers include it), then per
//! machine: conf header, impl header, public header, source.

use std::collections::BTreeSet;

use fsm_ir::Ir;
use thiserror::Error;

use crate::config::CodegenConfig;
use crate::expr::primitive_to_c;

pub mod completion;
pub mod conf_header;
pub mod defer;
pub mod dispatch_switch;
pub mod dispatch_table;
pub mod entry_exit;
pub mod hal;
pub mod header;
pub mod history;
pub mod impl_header;
pub mod license;
pub mod pseudostate;
pub mod queue;
pub mod source;
pub mod submachine;
pub mod timer;
pub mod trace_hook;
pub mod transition;

/// Bundle of generated files. Extensible Vec form per Doc 00 §5.6 NIT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmittedFiles {
    pub files: Vec<EmittedFile>,
    /// F-2 (F-1 doctrine: never a SILENT override). Non-fatal build notes
    /// the CLI must surface — currently the "an integrator `--queue-size` /
    /// `fsm.toml` shadowed an explicit in-source `queue {}`" disclosure.
    /// Empty on the common path → no behaviour/output change when unused.
    pub notes: Vec<String>,
}

impl EmittedFiles {
    /// Lookup a file by exact path. Returns `None` if not present — useful
    /// in tests that assert specific files were emitted.
    pub fn find(&self, path: &str) -> Option<&EmittedFile> {
        self.files.iter().find(|f| f.path == path)
    }
}

/// One generated file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmittedFile {
    pub path: String,
    pub role: FileRole,
    pub content: String,
}

/// Role tag for an emitted file. Drives test assertions and CLI logging.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileRole {
    Header,
    Source,
    ImplHeader,
    ConfHeader,
    Hal,
}

/// Emission errors. Distinct from `Diagnostic` because these are codegen
/// invariant violations — the IR was malformed in a way the analyzer should
/// have rejected. Surfacing as a hard error keeps the CLI from silently
/// emitting broken C.
#[derive(Debug, Error)]
pub enum EmitError {
    #[error("fsm-codegen-c: machine `{0}` has no states; nothing to emit")]
    EmptyMachine(String),
    #[error(
        "fsm-codegen-c: queue capacity {0} is not a power of two; \
         the C99 runtime uses bitwise-AND modulo and requires 2^N values"
    )]
    QueueCapacityNotPowerOfTwo(u32),
    #[error(
        "fsm-codegen-c: state index overflowed u8 — codegen does not support > 255 states. \
         Split the machine, raise the index type, or audit the IR for accidental state duplication."
    )]
    TooManyStates,
    /// A transition source/target (or other id reference) points at a state
    /// id that wasn't indexed for the machine. The analyzer is responsible
    /// for rejecting unresolved names (FSM-E0002) before codegen runs; this
    /// variant catches the case where an IR slipped through with an
    /// unresolved reference and lets `fsm generate` exit 2 with a clear
    /// diagnostic instead of aborting via `panic!` in
    /// `state_index::must_lookup` (audit P1-8).
    #[error("fsm-codegen-c: unindexed state id `{0}` referenced from a transition or trigger")]
    UnknownStateId(String),
}

/// Run the full emit pipeline for an IR document.
pub fn emit(ir: &Ir, config: &CodegenConfig) -> Result<EmittedFiles, EmitError> {
    // Pre-flight: the *fallback* queue capacity must be a power of two
    // (Doc 11 §6 / §12). F-2: this only validates the bottom-tier default;
    // the EFFECTIVE per-machine capacity (in-source `queue {}` may differ)
    // is the ring-mask invariant that actually matters and is re-checked
    // per machine in `emit_machine_recursive` after `resolve_queue`. The
    // analyzer (FSM-E0412) is the authoritative user-facing gate for a
    // non-power-of-2 in-source `capacity = N`; this is belt-and-braces so
    // a regression there becomes a clean exit-2, not broken C (P1-8
    // defense-in-depth pattern).
    if !is_power_of_two(config.queue_capacity as u32) {
        return Err(EmitError::QueueCapacityNotPowerOfTwo(
            config.queue_capacity as u32,
        ));
    }

    // v1.1 (2026-05-15): `defer EVENT` is a real UML 2.5.1 §14.2.3.9.1
    // feature. The pre-v1.1 audit P0-5 option-b stopgap had the analyzer
    // reject every `defer` with FSM-E0903 and a debug_assert here as a
    // belt-and-braces guard against a defer-bearing IR reaching codegen.
    // Both are retired: a defer-bearing IR is now CORRECT input, and the
    // dispatch strategy emits the deferred-event runtime for it (see
    // `emit::defer` + `dispatch_switch` / `dispatch_table`).

    let mut files = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    // Single HAL header shared by every emitted machine (Doc 16).
    files.push(hal::emit_hal_header(config));

    // Reserved for future cross-machine event de-duplication.
    let _event_seen: BTreeSet<String> = BTreeSet::new();

    for machine in &ir.machines {
        // v1.1-W2d: a `state X is Sub` ref-state owns a nested value-member
        // sub-instance of `Sub`'s machine struct (heap-free G2). The
        // template is its own self-contained codegen unit, carried in
        // `machine.submachines` (Doc 09 §3 / §4.11). Emit every submachine
        // template BEFORE the parent so the parent header's
        // `#include "Sub.h"` resolves and the nested `Sub_t` member is a
        // complete type. W2b sets `include_submachines:false` on the nested
        // lower, so `machine.submachines` of a submachine is empty — the
        // recursion bottoms out and `Sub_t` is a finite struct (no runtime
        // cycle guard needed; FSM-E0502 already rejects static template
        // cycles at analysis).
        //
        // v1.1-W7 (Doc 00 §11.25): the *unresolved* effective strategy for
        // this top-level machine is `strategy_for(name)` — its per-machine
        // override if registered, else the global strategy. It is threaded
        // down so every submachine template of this logical machine shares
        // the SAME dispatch family (a submachine has no user-facing
        // `[machine.X]` name to target separately; mixing dispatch *within*
        // one logical machine would be surprising). `Auto` is still resolved
        // per emitted unit against that unit's own state count below.
        let effective = config.strategy_for(&machine.name);
        emit_machine_recursive(machine, config, effective, &mut files, &mut notes)?;
    }

    Ok(EmittedFiles { files, notes })
}

/// Emit one machine unit plus, depth-first, every submachine template it
/// references. Submachines are emitted first so a parent that nests a
/// `Sub_t` value member has the complete type available via
/// `#include "Sub.h"`.
///
/// `effective` is the unresolved dispatch strategy for the *logical*
/// machine being emitted (after per-machine-override resolution at the
/// top-level call site, W7). It is constant across the submachine
/// recursion so one logical machine emits one consistent dispatch family;
/// `Auto` is resolved here against each emitted unit's own state count so
/// the small-machine-readability heuristic still applies per unit.
fn emit_machine_recursive(
    machine: &fsm_ir::MachineObject,
    config: &CodegenConfig,
    effective: crate::config::DispatchStrategy,
    files: &mut Vec<EmittedFile>,
    notes: &mut Vec<String>,
) -> Result<(), EmitError> {
    if machine.root.states.is_empty() {
        return Err(EmitError::EmptyMachine(machine.name.clone()));
    }

    // Emit referenced submachine templates first (post-order): the parent's
    // generated header includes theirs, and the nested value member needs
    // their full struct definition. They inherit the parent's `effective`
    // strategy (W7) — same logical machine, same dispatch family.
    for sub in &machine.submachines {
        emit_machine_recursive(sub, config, effective, files, notes)?;
    }

    let index = crate::state_index::build_state_index(machine)?;
    // Audit P1-8 (2026-05-14): walk every transition source / target
    // through the index BEFORE any emitter dereferences via
    // `must_lookup`. The analyzer is the authoritative gate (FSM-E0002
    // unresolved-name), but defending in codegen turns "analyzer
    // regression slipped through" from a `panic!` + backtrace into a
    // clean `EmitError::UnknownStateId` propagated to CLI exit code 2.
    pre_flight_validate(machine, &index)?;
    let parents = crate::parent_table::build_parent_table(&index);
    let layout = crate::region_layout::build_region_layout(machine, &index);
    let resolved_strategy = effective.resolve(index.count());

    // F-2: resolve the EFFECTIVE queue config for THIS machine (in-source
    // `queue {}` > integrator `--queue-size`/`fsm.toml` override > default).
    // `resolve_queue` is the single precedence point; the optional note is
    // the F-1-doctrine disclosure that an integrator override shadowed an
    // explicit in-source block (never a SILENT override).
    let (queue_capacity, queue_overflow, queue_note) = config.resolve_queue(&machine.queue);
    if let Some(n) = queue_note {
        let mut parts = Vec::new();
        if n.capacity_overridden {
            parts.push(format!(
                "capacity {} (in-source) overridden to {} by --queue-size / fsm.toml",
                n.in_source_capacity, n.effective_capacity
            ));
        }
        if n.overflow_overridden {
            parts.push(format!(
                "overflow {:?} (in-source) overridden to {:?} by --queue-overflow / fsm.toml",
                n.in_source_overflow, n.effective_overflow
            ));
        }
        notes.push(format!(
            "machine `{}`: an integrator override shadowed its in-source `queue {{}}` \
             block — {}. The generated firmware uses the override; the in-source value \
             is NOT in effect (this disclosure exists so the override is never silent).",
            machine.name,
            parts.join("; ")
        ));
    }
    // The ring buffer indexes with `& (CAP-1)` (emit::queue) — the EFFECTIVE
    // capacity (which may be the in-source `queue { capacity = N }`, not the
    // fallback) MUST be a power of two. The analyzer (FSM-E0412) is the
    // authoritative user-facing reject for an in-source non-2^N; this is the
    // belt-and-braces guard so a slip there is a clean exit-2, not a
    // corrupt-modulo miscompile (P1-8 defense-in-depth).
    if !is_power_of_two(queue_capacity) {
        return Err(EmitError::QueueCapacityNotPowerOfTwo(queue_capacity));
    }

    let ctx = MachineEmitCtx {
        machine,
        index: &index,
        parents: &parents,
        layout: &layout,
        config,
        strategy: resolved_strategy,
        queue_capacity,
        queue_overflow,
    };

    files.push(conf_header::emit(&ctx));
    files.push(impl_header::emit(&ctx));
    files.push(header::emit(&ctx));
    files.push(source::emit(&ctx));
    Ok(())
}

/// Verify that every state id referenced from a transition, trigger, or
/// timer in `machine` is present in `index`. Surfaces missing ids as
/// [`EmitError::UnknownStateId`] so callers can map the error to a
/// non-zero CLI exit rather than crashing inside an emit pass.
///
/// The analyzer is supposed to reject unresolved names at parse time
/// (FSM-E0002); this is a belt-and-braces check that lets `fsm generate`
/// exit cleanly even when an analyzer regression lets a malformed IR
/// reach codegen. Audit P1-8.
fn pre_flight_validate(
    machine: &fsm_ir::MachineObject,
    index: &crate::state_index::StateIndex,
) -> Result<(), EmitError> {
    fn check(idx: &crate::state_index::StateIndex, id: &str) -> Result<(), EmitError> {
        if id.is_empty() {
            // Empty ids appear from lowering when a target name failed to
            // resolve at analysis time; the diagnostic was already emitted
            // and this codegen path simply skips them rather than calling
            // `must_lookup` on `""`.
            return Ok(());
        }
        if idx.lookup(id).is_none() {
            return Err(EmitError::UnknownStateId(id.to_owned()));
        }
        Ok(())
    }
    fn walk(
        idx: &crate::state_index::StateIndex,
        states: &[fsm_ir::StateNode],
    ) -> Result<(), EmitError> {
        for s in states {
            match s {
                fsm_ir::StateNode::Simple(ss) => {
                    for t in &ss.transitions {
                        check(idx, &t.source)?;
                        check(idx, &t.target)?;
                    }
                    for t in &ss.timers {
                        if let Some(tgt) = &t.target {
                            check(idx, tgt)?;
                        }
                    }
                }
                fsm_ir::StateNode::Composite(c) => {
                    for t in &c.transitions {
                        check(idx, &t.source)?;
                        check(idx, &t.target)?;
                    }
                    for t in &c.timers {
                        if let Some(tgt) = &t.target {
                            check(idx, tgt)?;
                        }
                    }
                    for r in &c.regions {
                        walk(idx, &r.states)?;
                    }
                }
                fsm_ir::StateNode::Parallel(p) => {
                    for t in &p.transitions {
                        check(idx, &t.source)?;
                        check(idx, &t.target)?;
                    }
                    for r in &p.regions {
                        walk(idx, &r.states)?;
                    }
                }
                fsm_ir::StateNode::Submachine(sm) => {
                    for t in &sm.transitions {
                        check(idx, &t.source)?;
                        check(idx, &t.target)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    walk(index, &machine.root.states)
}

/// Per-machine emit context, threaded through the file emitters so they
/// don't each have to re-resolve the dispatch strategy / build the index.
pub struct MachineEmitCtx<'a> {
    pub machine: &'a fsm_ir::MachineObject,
    pub index: &'a crate::state_index::StateIndex,
    pub parents: &'a crate::parent_table::ParentTable,
    pub layout: &'a crate::region_layout::RegionLayout,
    pub config: &'a CodegenConfig,
    pub strategy: crate::config::DispatchStrategy,
    /// F-2: the effective queue capacity for THIS machine, already resolved
    /// through [`CodegenConfig::resolve_queue`] (in-source `queue {}` >
    /// integrator override > default). Emitters MUST read this, never
    /// `config.queue_capacity` directly — the latter is only the bottom-tier
    /// fallback and ignoring the IR `queue {}` is precisely the F-2 defect.
    pub queue_capacity: u32,
    /// F-2: the effective overflow policy for THIS machine (same resolution
    /// as [`MachineEmitCtx::queue_capacity`]).
    pub queue_overflow: crate::config::OverflowPolicy,
}

impl<'a> MachineEmitCtx<'a> {
    /// Uppercased machine prefix used for #include guards, e.g. `MOTOR`.
    pub fn macro_prefix(&self) -> String {
        crate::state_index::c_ident(&self.machine.name)
    }

    /// Mixed-case machine prefix used for function names, e.g. `Motor`.
    pub fn type_prefix(&self) -> &str {
        &self.machine.name
    }

    /// Canonical filename stem (e.g. `Motor`).
    pub fn file_stem(&self) -> &str {
        &self.machine.name
    }

    /// Resolve an IR event id to its C enum value, e.g. `MOTOR_EVENT_START`.
    pub fn event_c_enum(&self, event_id: &str) -> String {
        let prefix = self.macro_prefix();
        let event = self
            .machine
            .events
            .iter()
            .find(|e| e.id == event_id)
            .map(|e| crate::state_index::c_ident(&e.name))
            .unwrap_or_else(|| crate::state_index::c_ident(event_id));
        format!("{}_EVENT_{}", prefix, event)
    }

    /// Resolve a context field name to a C member: `m->context.field`.
    pub fn ctx_field_c_type(&self, field_name: &str) -> &'static str {
        self.machine
            .context
            .fields
            .iter()
            .find(|f| f.name == field_name)
            .map(|f| match &f.ty {
                fsm_ir::Type::Primitive { name } => primitive_to_c(name),
                _ => "int",
            })
            .unwrap_or("int")
    }
}

fn is_power_of_two(n: u32) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_of_two_detected() {
        assert!(is_power_of_two(1));
        assert!(is_power_of_two(2));
        assert!(is_power_of_two(4));
        assert!(is_power_of_two(64));
        assert!(is_power_of_two(128));
        assert!(!is_power_of_two(0));
        assert!(!is_power_of_two(3));
        assert!(!is_power_of_two(10));
    }
}
