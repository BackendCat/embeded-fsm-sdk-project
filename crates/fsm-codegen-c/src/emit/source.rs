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
    s.push_str("    memset(m, 0, sizeof(*m));\n");
    s.push_str("    m->_active_count = 1;\n");
    s.push_str(&format!(
        "    m->_active[0] = {macro}_STATE_ROOT;\n",
        macro = macro_prefix,
    ));

    // Expand the root region's initial pseudo into a list of leaves the
    // runtime starts out in. For a non-parallel path this is a single
    // leaf; if the path crosses a parallel state, every region of that
    // parallel contributes one leaf (Doc 08 §2.3).
    let mut entries: Vec<EntryRec> = Vec::new();
    if let Some(start_idx) = ctx.index.records[0].initial_child {
        let chain = resolve_initial_chain(ctx, start_idx);
        for chain_state in &chain {
            expand_initial_to_leaves(ctx, *chain_state, &mut entries);
        }
    }

    // Emit entry actions for every state along the initial path (composites
    // / parallels first, then the leaf), and write each leaf into its
    // assigned `_active[]` slot. We dedupe via `entered` to avoid emitting
    // the same entry function twice when the same composite is on multiple
    // chain paths.
    let mut entered: std::collections::BTreeSet<u8> = std::collections::BTreeSet::new();
    let mut leaf_count: u8 = 0;
    for e in &entries {
        for anc in &e.ancestors {
            if !entered.insert(*anc) {
                continue;
            }
            let rec = ctx.index.get(*anc);
            if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
                s.push_str(&format!(
                    "    {prefix}_entry_{name}(m);\n",
                    prefix = prefix,
                    name = rec.c_name,
                ));
            }
        }
        let leaf = ctx.index.get(e.leaf);
        let slot = ctx.layout.slot(e.leaf);
        s.push_str(&format!(
            "    m->_active[{slot}] = {macro}_STATE_{name};\n",
            macro = macro_prefix,
            name = leaf.c_name,
            slot = slot,
        ));
        if leaf.kind.is_active_at_rest() && leaf.kind != StateRecordKind::Final {
            s.push_str(&format!(
                "    {prefix}_entry_{name}(m);\n",
                prefix = prefix,
                name = leaf.c_name,
            ));
        }
        leaf_count += 1;
    }

    if leaf_count > 1 {
        s.push_str(&format!("    m->_active_count = {};\n", leaf_count));
    }

    // Start any timers owned by states entered during init.
    for t in timer::collect_timers(ctx) {
        let owned = entries
            .iter()
            .any(|e| e.leaf == t.owner_state || e.ancestors.contains(&t.owner_state));
        if owned {
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

/// One leaf entered during init. `ancestors` lists the path of composites /
/// parallels traversed on the way down (root-first, NOT including the
/// leaf itself).
struct EntryRec {
    leaf: u8,
    ancestors: Vec<u8>,
}

/// Resolve a chain of Initial pseudo-states down to the first active-at-
/// rest state ID. Returns the resulting chain (length 1 unless the IR
/// has a degenerate Initial→Initial path, which the analyzer rejects).
fn resolve_initial_chain(ctx: &MachineEmitCtx<'_>, mut cur: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bounce = 0;
    while bounce < 32 {
        bounce += 1;
        let rec = ctx.index.get(cur);
        match rec.kind {
            StateRecordKind::Initial => {
                if let Some(target_id) = find_initial_target(ctx.machine, &rec.ir_id) {
                    if let Some(next) = ctx.index.lookup(&target_id) {
                        cur = next;
                        continue;
                    }
                }
                break;
            }
            _ => {
                out.push(cur);
                break;
            }
        }
    }
    out
}

/// Expand a state into the list of leaf-active states the runtime is in at
/// rest. A simple/final state yields itself; a composite yields its
/// initial-chain's leaves (recursive); a parallel yields one leaf per
/// region (recursive). Each emitted `EntryRec` carries the chain of
/// composites / parallels that needed to be entered to reach the leaf.
fn expand_initial_to_leaves(ctx: &MachineEmitCtx<'_>, state_idx: u8, out: &mut Vec<EntryRec>) {
    expand_recurse(ctx, state_idx, &mut Vec::new(), out);
}

fn expand_recurse(
    ctx: &MachineEmitCtx<'_>,
    state_idx: u8,
    ancestors: &mut Vec<u8>,
    out: &mut Vec<EntryRec>,
) {
    let rec = ctx.index.get(state_idx);
    match rec.kind {
        StateRecordKind::Simple | StateRecordKind::Final | StateRecordKind::Submachine => {
            out.push(EntryRec {
                leaf: state_idx,
                ancestors: ancestors.clone(),
            });
        }
        StateRecordKind::Composite => {
            ancestors.push(state_idx);
            // A composite has at most one region in v1.0; follow its
            // initial chain into the deeper structure.
            let composite_node = find_composite(ctx.machine, &rec.ir_id);
            if let Some(c) = composite_node {
                if let Some(region) = c.regions.first() {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        let chain = resolve_initial_chain(ctx, init_idx);
                        for child in chain {
                            expand_recurse(ctx, child, ancestors, out);
                        }
                    }
                }
            }
            ancestors.pop();
        }
        StateRecordKind::Parallel => {
            ancestors.push(state_idx);
            let parallel_node = find_parallel(ctx.machine, &rec.ir_id);
            if let Some(p) = parallel_node {
                for region in &p.regions {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        let chain = resolve_initial_chain(ctx, init_idx);
                        for child in chain {
                            expand_recurse(ctx, child, ancestors, out);
                        }
                    }
                }
            }
            ancestors.pop();
        }
        _ => {
            // Pseudo-state — analyzer should have lowered this to a real
            // target by now.
        }
    }
}

fn find_composite<'a>(
    m: &'a fsm_ir::MachineObject,
    id: &str,
) -> Option<&'a fsm_ir::CompositeState> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a fsm_ir::CompositeState> {
        for s in states {
            match s {
                StateNode::Composite(c) => {
                    if c.id == id {
                        return Some(c);
                    }
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
    walk(&m.root.states, id)
}

fn find_parallel<'a>(m: &'a fsm_ir::MachineObject, id: &str) -> Option<&'a fsm_ir::ParallelState> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a fsm_ir::ParallelState> {
        for s in states {
            match s {
                StateNode::Parallel(p) => {
                    if p.id == id {
                        return Some(p);
                    }
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
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
    walk(&m.root.states, id)
}

fn emit_current_state(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    format!(
        "{prefix}_StateId_t {prefix}_current_state(const {prefix}_t *m) {{\n    return m->_active[0];\n}}\n",
        prefix = prefix,
    )
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
