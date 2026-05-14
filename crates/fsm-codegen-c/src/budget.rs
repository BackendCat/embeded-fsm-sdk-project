//! Memory-budget computation — Doc 00 §7.11, surfaced via
//! `fsm generate --report-memory` (Doc 18 §5.4).
//!
//! Walks the IR for a single machine and accumulates approximate sizes for
//! the context struct, event queue, history slots, timer slots, and a
//! conservative ROM-text estimate. These numbers are *advisory* — the
//! authoritative figure comes from `size` on the linked `.o` — but they
//! catch obvious overruns at compile time.

use fsm_ir::{ContextField, Ir, MachineObject, StateNode, Type};

use crate::config::CodegenConfig;
use crate::state_index::{build_state_index, StateIndex, StateRecordKind};

/// Approximate memory budget for a single machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryBudget {
    /// Total RAM for `Motor_t`. Sum of context, queue, internal bookkeeping.
    pub sizeof_machine: usize,
    /// Bytes contributed by user-declared context fields.
    pub sizeof_context: usize,
    /// Bytes contributed by the event queue (`capacity * sizeof(Event)`).
    pub queue_bytes: usize,
    /// Worst-case depth of the completion stack — used to bound the
    /// `_completion_depth` watchdog.
    pub max_completion_depth: usize,
    /// Bytes contributed by history pseudo-state storage. Two bytes per
    /// history pseudo-state (slot + write watermark) on machines without
    /// parallel history; codegen rounds up per region for parallel cases.
    pub history_bytes: usize,
    /// Bytes contributed by timer remaining counters.
    pub timer_bytes: usize,
    /// Total RAM. Same as `sizeof_machine` today; kept distinct so the
    /// emitter can later add ISR-safety overhead.
    pub total_ram_bytes: usize,
    /// Conservative ROM estimate (parent table + transition table + entry
    /// and exit function pointer tables).
    pub estimated_rom_bytes: usize,
}

/// Compute the memory budget for the first machine in an IR document.
///
/// IR documents may carry multiple machines; the budget is per-machine and
/// the CLI prints one block per machine. For codegen integration tests
/// that build a single `Motor` IR, taking the first machine is the right
/// behaviour.
pub fn compute_budget(ir: &Ir, config: &CodegenConfig) -> MemoryBudget {
    let machine = ir
        .machines
        .first()
        .expect("fsm-codegen-c: cannot compute budget for an empty IR document");
    compute_machine_budget(machine, config)
}

/// Per-machine budget computation. Exposed so tests can address each
/// machine in a multi-machine IR.
pub fn compute_machine_budget(machine: &MachineObject, config: &CodegenConfig) -> MemoryBudget {
    let index = build_state_index(machine);

    let sizeof_event = sizeof_event(machine);
    let queue_bytes = (config.queue_capacity as usize) * sizeof_event;
    let sizeof_context = sizeof_context(&machine.context.fields);
    let history_bytes = count_history_slots(&index); // u8 per history slot
    let timer_bytes = count_timers(machine) * 4; // uint32_t per timer
    let max_completion_depth = state_depth(&index);

    // Internal bookkeeping: `_active[]` (`MAX_PARALLEL_REGIONS` u8 slots
    // — one per simultaneously active leaf), `_active_count` (u8),
    // `_queue_head`, `_queue_tail`, `_queue_count`, `_completion_depth` —
    // four u8 fixed fields plus the variable-size active-leaf array.
    let bookkeeping = 5 + count_active_slots(machine);
    let sizeof_machine = sizeof_context + queue_bytes + history_bytes + timer_bytes + bookkeeping;

    let estimated_rom_bytes = estimate_rom(&index, machine);

    MemoryBudget {
        sizeof_machine,
        sizeof_context,
        queue_bytes,
        max_completion_depth,
        history_bytes,
        timer_bytes,
        total_ram_bytes: sizeof_machine,
        estimated_rom_bytes,
    }
}

fn sizeof_type(t: &Type) -> usize {
    match t {
        Type::Primitive { name } => match name.as_str() {
            "bool" | "u8" | "i8" => 1,
            "u16" | "i16" => 2,
            "u32" | "i32" | "f32" => 4,
            "u64" | "i64" | "f64" => 8,
            "string" => std::mem::size_of::<usize>(), // pointer
            _ => 4,
        },
        Type::Enum { .. } => 4,   // C enums default to int
        Type::Opaque { .. } => 4, // best-effort; user can tune
        Type::Array { element, size } => sizeof_type(element) * (*size as usize),
    }
}

fn sizeof_context(fields: &[ContextField]) -> usize {
    fields.iter().map(|f| sizeof_type(&f.ty)).sum()
}

fn sizeof_event(machine: &MachineObject) -> usize {
    // Event union: tag (u8) + worst-case payload. Walk every event payload
    // and take the maximum sum-of-fields.
    let tag = 1;
    let worst = machine
        .events
        .iter()
        .map(|e| e.payload.iter().map(|p| sizeof_type(&p.ty)).sum::<usize>())
        .max()
        .unwrap_or(0);
    tag + worst
}

fn count_history_slots(index: &StateIndex) -> usize {
    index
        .records
        .iter()
        .filter(|r| r.history_pseudo.is_some())
        .count()
}

fn count_timers(machine: &MachineObject) -> usize {
    fn count_state(s: &StateNode, acc: &mut usize) {
        match s {
            StateNode::Simple(ss) => *acc += ss.timers.len(),
            StateNode::Composite(cs) => {
                *acc += cs.timers.len();
                for r in &cs.regions {
                    for s in &r.states {
                        count_state(s, acc);
                    }
                }
            }
            StateNode::Parallel(ps) => {
                *acc += ps.timers.len();
                for r in &ps.regions {
                    for s in &r.states {
                        count_state(s, acc);
                    }
                }
            }
            _ => {}
        }
    }
    let mut acc = 0;
    for s in &machine.root.states {
        count_state(s, &mut acc);
    }
    acc
}

fn count_active_slots(machine: &MachineObject) -> usize {
    // Number of `_active[]` slots. Reuses the same conservative analysis
    // `region_layout::compute_max_active_leaves` performs.
    let idx = build_state_index(machine);
    crate::region_layout::build_region_layout(machine, &idx).max_parallel_regions as usize
}

fn state_depth(index: &StateIndex) -> usize {
    let mut max_depth = 0;
    for i in 0..index.records.len() {
        let mut depth = 0usize;
        let mut cur = i as u8;
        while cur != crate::state_index::ROOT_SENTINEL {
            let parent = index.records[cur as usize].parent;
            if parent == cur {
                break;
            }
            depth += 1;
            cur = parent;
        }
        if depth > max_depth {
            max_depth = depth;
        }
    }
    max_depth.max(1)
}

fn estimate_rom(index: &StateIndex, machine: &MachineObject) -> usize {
    let parent_table = index.records.len(); // u8 per state
                                            // Transition table: ~12 bytes per transition for switch strategy, ~16
                                            // for table strategy. Use the worst case to be conservative.
    let trans_count = count_transitions(machine);
    let trans_table = trans_count * 16;
    // Entry/exit function-pointer tables — pointer per state.
    let fn_tables = index
        .records
        .iter()
        .filter(|r| r.kind.is_active_at_rest())
        .count()
        * 2
        * std::mem::size_of::<fn()>();
    // Rough text-size guess: 64 bytes per transition + a fixed base.
    let text = 256 + trans_count * 64;
    parent_table + trans_table + fn_tables + text
}

fn count_transitions(machine: &MachineObject) -> usize {
    fn count_state(s: &StateNode, acc: &mut usize) {
        match s {
            StateNode::Simple(ss) => *acc += ss.transitions.len(),
            StateNode::Composite(cs) => {
                *acc += cs.transitions.len();
                for r in &cs.regions {
                    for s in &r.states {
                        count_state(s, acc);
                    }
                }
            }
            StateNode::Parallel(ps) => {
                *acc += ps.transitions.len();
                for r in &ps.regions {
                    for s in &r.states {
                        count_state(s, acc);
                    }
                }
            }
            _ => {}
        }
    }
    let mut acc = 0;
    for s in &machine.root.states {
        count_state(s, &mut acc);
    }
    acc
}

// `is_active_at_rest` is also used here. Re-import for clarity.
impl StateRecordKind {}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        ContextField, ContextSchema, EventObject, InitialPseudo, Ir, MachineObject, QueueConfig,
        RegionObject, SimpleState, StateNode, Type,
    };

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    fn motor_ir() -> Ir {
        Ir {
            ir_version: "1.0.0".into(),
            source_hash: "".into(),
            source_files: vec!["motor.fsm".into()],
            machines: vec![MachineObject {
                id: "m".into(),
                stable_id: "Motor".into(),
                name: "Motor".into(),
                context: ContextSchema {
                    fields: vec![
                        ContextField {
                            id: "f-speed".into(),
                            name: "speed".into(),
                            ty: Type::Primitive { name: "u16".into() },
                            default: None,
                            loc: loc(),
                        },
                        ContextField {
                            id: "f-running".into(),
                            name: "running".into(),
                            ty: Type::Primitive {
                                name: "bool".into(),
                            },
                            default: None,
                            loc: loc(),
                        },
                    ],
                },
                events: vec![EventObject {
                    id: "e-start".into(),
                    stable_id: "Motor:event:START".into(),
                    name: "START".into(),
                    payload: vec![],
                    loc: loc(),
                }],
                externs: vec![],
                root: RegionObject {
                    id: "r-root".into(),
                    stable_id: None,
                    name: "__root".into(),
                    initial: "ps-init".into(),
                    states: vec![
                        StateNode::Initial(InitialPseudo {
                            id: "ps-init".into(),
                            target: "s-idle".into(),
                            loc: loc(),
                        }),
                        StateNode::Simple(SimpleState {
                            id: "s-idle".into(),
                            stable_id: "Motor:state:Idle".into(),
                            name: "Idle".into(),
                            entry: vec![],
                            exit: vec![],
                            transitions: vec![],
                            timers: vec![],
                            defers: vec![],
                            loc: loc(),
                        }),
                    ],
                    priority: 0,
                    loc: loc(),
                },
                submachines: vec![],
                consts: vec![],
                imports: vec![],
                features: vec![],
                queue: QueueConfig::default(),
                targets: vec![],
                loc: loc(),
            }],
            diagnostics: vec![],
        }
    }

    #[test]
    fn budget_includes_context_fields() {
        let b = compute_budget(&motor_ir(), &CodegenConfig::default());
        // u16 (2) + bool (1) = 3
        assert_eq!(b.sizeof_context, 3);
    }

    #[test]
    fn total_ram_exceeds_context() {
        let b = compute_budget(&motor_ir(), &CodegenConfig::default());
        assert!(
            b.total_ram_bytes > b.sizeof_context,
            "got: {b:?} — queue + bookkeeping should push total above context"
        );
    }

    #[test]
    fn queue_bytes_scales_with_capacity() {
        let mut cfg = CodegenConfig::default();
        cfg.queue_capacity = 16;
        let b16 = compute_budget(&motor_ir(), &cfg);
        cfg.queue_capacity = 4;
        let b4 = compute_budget(&motor_ir(), &cfg);
        assert!(b16.queue_bytes > b4.queue_bytes);
    }

    #[test]
    fn rom_estimate_nonzero_even_for_empty_machine() {
        // Even a no-transition machine carries the parent table + function
        // tables in ROM.
        let b = compute_budget(&motor_ir(), &CodegenConfig::default());
        assert!(b.estimated_rom_bytes > 0);
    }
}
