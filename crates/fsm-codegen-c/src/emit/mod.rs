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
pub mod queue;
pub mod source;
pub mod timer;
pub mod transition;

/// Bundle of generated files. Extensible Vec form per Doc 00 §5.6 NIT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmittedFiles {
    pub files: Vec<EmittedFile>,
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
    QueueCapacityNotPowerOfTwo(u8),
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
    // Pre-flight: queue capacity must be a power of two (Doc 11 §6 / §12).
    if !is_power_of_two(config.queue_capacity) {
        return Err(EmitError::QueueCapacityNotPowerOfTwo(config.queue_capacity));
    }

    // v1.1 (2026-05-15): `defer EVENT` is a real UML 2.5.1 §14.2.3.9.1
    // feature. The pre-v1.1 audit P0-5 option-b stopgap had the analyzer
    // reject every `defer` with FSM-E0903 and a debug_assert here as a
    // belt-and-braces guard against a defer-bearing IR reaching codegen.
    // Both are retired: a defer-bearing IR is now CORRECT input, and the
    // dispatch strategy emits the deferred-event runtime for it (see
    // `emit::defer` + `dispatch_switch` / `dispatch_table`).

    let mut files = Vec::new();
    // Single HAL header shared by every emitted machine (Doc 16).
    files.push(hal::emit_hal_header(config));

    // Reserved for future cross-machine event de-duplication.
    let _event_seen: BTreeSet<String> = BTreeSet::new();

    for machine in &ir.machines {
        if machine.root.states.is_empty() {
            return Err(EmitError::EmptyMachine(machine.name.clone()));
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
        let resolved_strategy = config.strategy.resolve(index.count());

        let ctx = MachineEmitCtx {
            machine,
            index: &index,
            parents: &parents,
            layout: &layout,
            config,
            strategy: resolved_strategy,
        };

        files.push(conf_header::emit(&ctx));
        files.push(impl_header::emit(&ctx));
        files.push(header::emit(&ctx));
        files.push(source::emit(&ctx));
    }

    Ok(EmittedFiles { files })
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

fn is_power_of_two(n: u8) -> bool {
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
