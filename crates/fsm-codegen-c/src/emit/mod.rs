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
    #[error("fsm-codegen-c: state index overflowed u8 — codegen does not support > 255 states")]
    TooManyStates,
}

/// Run the full emit pipeline for an IR document.
pub fn emit(ir: &Ir, config: &CodegenConfig) -> Result<EmittedFiles, EmitError> {
    // Pre-flight: queue capacity must be a power of two (Doc 11 §6 / §12).
    if !is_power_of_two(config.queue_capacity) {
        return Err(EmitError::QueueCapacityNotPowerOfTwo(config.queue_capacity));
    }

    // Audit P0-5 option-b (2026-05-14): the analyzer rejects every
    // `defer EVENT` with FSM-E0903 in v1.0, so a defer-bearing IR should
    // never reach codegen. Belt-and-braces: assert it in debug builds.
    // If this fires, an analyzer regression has let a non-conforming IR
    // through and we'd otherwise silently emit broken C.
    #[cfg(debug_assertions)]
    for machine in &ir.machines {
        debug_assert!(
            !any_state_has_defer(&machine.root.states),
            "fsm-codegen-c: machine `{}` reached codegen with a `defer EVENT` \
             declaration — the analyzer must have rejected it with FSM-E0903 \
             first (audit P0-5; defer not supported in v1.0)",
            machine.name,
        );
    }

    let mut files = Vec::new();
    // Single HAL header shared by every emitted machine (Doc 16).
    files.push(hal::emit_hal_header(config));

    // Reserved for future cross-machine event de-duplication.
    let _event_seen: BTreeSet<String> = BTreeSet::new();

    for machine in &ir.machines {
        if machine.root.states.is_empty() {
            return Err(EmitError::EmptyMachine(machine.name.clone()));
        }

        let index = crate::state_index::build_state_index(machine);
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

/// Recursively check whether any state (top-level or nested through
/// composite / parallel regions) carries a non-empty `defers` list.
/// Used by the `debug_assertions` guard at the top of `emit` to catch
/// analyzer regressions before they produce broken C (audit P0-5).
#[cfg(debug_assertions)]
fn any_state_has_defer(states: &[fsm_ir::StateNode]) -> bool {
    use fsm_ir::StateNode;
    states.iter().any(|s| match s {
        StateNode::Simple(ss) => !ss.defers.is_empty(),
        StateNode::Composite(c) => {
            !c.defers.is_empty() || c.regions.iter().any(|r| any_state_has_defer(&r.states))
        }
        StateNode::Parallel(p) => {
            !p.defers.is_empty() || p.regions.iter().any(|r| any_state_has_defer(&r.states))
        }
        _ => false,
    })
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
