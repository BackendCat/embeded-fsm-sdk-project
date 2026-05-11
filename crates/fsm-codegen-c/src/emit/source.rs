//! `Motor.c` — translation unit. Wires every other emitter into a single
//! compilable source file.

use fsm_ir::StateNode;

use crate::config::DispatchStrategy;
use crate::state_index::StateRecordKind;

use super::license::header_block;
use super::{
    completion, defer, dispatch_switch, dispatch_table, history, queue, timer, EmittedFile,
    FileRole, MachineEmitCtx,
};

pub fn emit(ctx: &MachineEmitCtx<'_>) -> EmittedFile {
    let stem = ctx.file_stem();
    let macro_prefix = ctx.macro_prefix();
    let header = header_block(
        ctx.config,
        Some(&format!("{}.fsm (translation unit)", stem)),
    );

    let mut body = String::new();
    body.push_str(&format!("#include \"{}.h\"\n", stem));
    body.push_str(&format!("#include \"{}_impl.h\"\n", stem));
    body.push_str("#include <string.h> /* memset */\n\n");

    // Defer mask, history helpers, completion helpers, timer tick.
    body.push_str(&defer::emit_defer_mask(ctx));
    body.push_str("\n");
    body.push_str(&history::emit_history_helpers(ctx));
    body.push_str("\n");
    body.push_str(&completion::emit_all_regions_final_helper(ctx));
    body.push_str("\n");

    // Forward declarations for cross-references between the dispatcher and
    // the completion helper.
    body.push_str(&format!(
        "static void {prefix}_handle_completion({prefix}_t *m);\n",
        prefix = ctx.type_prefix(),
    ));
    body.push_str(&format!(
        "void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev);\n\n",
        prefix = ctx.type_prefix(),
    ));

    body.push_str(&queue::emit_queue(ctx));
    body.push_str("\n");

    // Dispatch — strategy-dependent.
    match ctx.strategy {
        DispatchStrategy::Switch | DispatchStrategy::Auto => {
            body.push_str(&dispatch_switch::emit_dispatch(ctx));
        }
        DispatchStrategy::Table => {
            body.push_str(&dispatch_table::emit_dispatch(ctx));
        }
    }
    body.push_str("\n");

    body.push_str(&completion::emit_handle_completion(ctx));
    body.push_str("\n");

    body.push_str(&timer::emit_advance_clock(ctx));
    body.push_str("\n");

    body.push_str(&emit_init(ctx));
    body.push_str("\n");
    body.push_str(&emit_current_state(ctx));

    // Reference the macro_prefix to silence unused warnings when no enums
    // referenced it directly.
    let _ = macro_prefix;

    EmittedFile {
        path: format!("{}.c", stem),
        role: FileRole::Source,
        content: format!("{}\n{}", header, body),
    }
}

fn emit_init(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "void {prefix}_init({prefix}_t *m) {{\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "    memset(m, 0, sizeof(*m));\n    m->_state = {macro}_STATE_ROOT;\n",
        macro = macro_prefix,
    ));
    // Determine the initial leaf for the root region — follow `initial`
    // through the chain of pseudo / composite states.
    if let Some(start_rec) = walk_initial(ctx) {
        s.push_str(&format!(
            "    m->_state = {macro}_STATE_{name};\n",
            macro = macro_prefix,
            name = start_rec.c_name,
        ));
        s.push_str(&format!(
            "    {prefix}_entry_{name}(m);\n",
            prefix = prefix,
            name = start_rec.c_name,
        ));
    }
    // Start any timers owned by the initial state (best-effort — full
    // start-on-entry happens through the user's entry handler if needed).
    for t in timer::collect_timers(ctx) {
        if Some(t.owner_state) == walk_initial(ctx).map(|r| ctx.index.lookup(&r.ir_id).unwrap()) {
            s.push_str(&format!(
                "    m->_timer_{field}_remaining_ms = {dur}u;\n",
                field = t.field_name,
                dur = t.duration_ms,
            ));
        }
    }
    s.push_str("}\n");
    s
}

fn emit_current_state(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    format!(
        "{prefix}_StateId_t {prefix}_current_state(const {prefix}_t *m) {{\n    return m->_state;\n}}\n",
        prefix = prefix,
    )
}

/// Follow the root region's initial chain to the first active-at-rest
/// state. Used by `Motor_init` to set the initial state.
fn walk_initial<'a>(ctx: &'a MachineEmitCtx<'a>) -> Option<&'a crate::state_index::StateRecord> {
    // Start at the root sentinel's initial child, then walk through
    // initial pseudos.
    let root_init = ctx.index.records[0].initial_child?;
    let mut cur = root_init;
    let mut bounce = 0;
    while bounce < 32 {
        bounce += 1;
        let rec = ctx.index.get(cur);
        match rec.kind {
            StateRecordKind::Simple
            | StateRecordKind::Composite
            | StateRecordKind::Parallel
            | StateRecordKind::Submachine => return Some(rec),
            StateRecordKind::Final => return Some(rec),
            StateRecordKind::Initial => {
                // Look up the actual InitialPseudo from the IR to follow
                // its target.
                if let Some(target_id) = find_initial_target(ctx.machine, &rec.ir_id) {
                    if let Some(next) = ctx.index.lookup(&target_id) {
                        cur = next;
                        continue;
                    }
                }
                return None;
            }
            _ => return Some(rec),
        }
    }
    None
}

fn find_initial_target(machine: &fsm_ir::MachineObject, ir_id: &str) -> Option<String> {
    fn walk(states: &[StateNode], id: &str) -> Option<String> {
        for s in states {
            match s {
                StateNode::Initial(i) => {
                    if i.id == id {
                        return Some(i.target.clone());
                    }
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&machine.root.states, ir_id)
}

/// Public helper: count parallel-region state slots needed in the machine
/// struct (each region of each parallel state stores one StateId_t).
pub fn count_parallel_region_slots(machine: &fsm_ir::MachineObject) -> usize {
    fn walk(states: &[StateNode], acc: &mut usize) {
        for s in states {
            if let StateNode::Parallel(p) = s {
                *acc += p.regions.len();
                for r in &p.regions {
                    walk(&r.states, acc);
                }
            } else if let StateNode::Composite(c) = s {
                for r in &c.regions {
                    walk(&r.states, acc);
                }
            }
        }
    }
    let mut acc = 0;
    walk(&machine.root.states, &mut acc);
    acc
}

/// Public helper: collect every timer's struct-field name in document
/// order. Used by header.rs to declare the storage slots.
pub fn collect_timer_names(machine: &fsm_ir::MachineObject) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(states: &[StateNode], machine_name: &str, out: &mut Vec<String>) {
        for s in states {
            let (state_name, timers, recurse) = match s {
                StateNode::Simple(s) => (&s.name, &s.timers, None),
                StateNode::Composite(c) => (&c.name, &c.timers, Some(c.regions.as_slice())),
                StateNode::Parallel(p) => (&p.name, &p.timers, Some(p.regions.as_slice())),
                _ => continue,
            };
            for t in timers {
                let safe = t.stable_id.replace([':', '-'], "_");
                let c_name = crate::state_index::c_ident(state_name);
                out.push(format!("{}_{}", c_name, safe));
            }
            if let Some(regions) = recurse {
                for r in regions {
                    walk(&r.states, machine_name, out);
                }
            }
        }
    }
    walk(&machine.root.states, &machine.name, &mut out);
    out
}
