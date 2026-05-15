//! Switch-based dispatch — Doc 11 §8, Doc 00 §7.8 (B-10 + B-11).
//!
//! Pattern:
//!  - Emit `Motor_parent_table[]: static const M_StateId_t[]`.
//!  - Emit per-state helper `Motor_try_transitions_in_state` returning
//!    `bool` (true if a transition fired).
//!  - Emit the outer `Motor_dispatch` that, per active region, walks
//!    leaf-to-root using the parent table (collect-then-execute over
//!    `_active[]`).

use fsm_ir::{walk_state, IrVisitor, StateNode, TransitionObject};

use super::MachineEmitCtx;

/// Emit the parent table + per-state helpers + outer dispatch loop.
pub fn emit_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let mut s = String::new();
    s.push_str(&emit_parent_table(ctx));
    s.push_str("\n");
    // v1.1: the deferred-event apparatus is emitted right after the parent
    // table (the defer-membership query walks it) and before the outer
    // dispatch that calls into it. Gated on `machine_has_defer` so a
    // machine with no `defer` declaration emits byte-identical code to
    // before this wave (no dead helpers, no `-Werror=unused-function`).
    if super::defer::machine_has_defer(ctx) {
        s.push_str(&super::defer::emit_defer_table(ctx));
        s.push_str("\n");
        s.push_str(&super::defer::emit_defer_runtime(ctx));
        s.push_str("\n");
    }
    s.push_str(&emit_per_state_helpers(ctx));
    s.push_str("\n");
    s.push_str(&emit_outer_dispatch(ctx));
    s
}

fn emit_parent_table(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* B-10 hierarchical dispatch: parent-pointer table indexed by StateId. */\n",
    ));
    s.push_str(&format!(
        "static const {prefix}_StateId_t {prefix}_parent_table[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    for (i, rec) in ctx.index.records.iter().enumerate() {
        let parent_rec = ctx.index.get(rec.parent);
        s.push_str(&format!(
            "    [{}] = {macro}_STATE_{name}, /* {} */\n",
            i,
            rec.dsl_name,
            macro = macro_prefix,
            name = parent_rec.c_name,
        ));
    }
    s.push_str("};\n");
    s
}

/// Whether any state in the machine declares at least one outgoing
/// transition. When false, the per-state switch / table dispatch bodies
/// reference none of their pointer parameters (a zero-transition machine —
/// states with only entry/exit actions, or a placeholder skeleton — is
/// valid DSL, Doc 04 §3), so the emitters must `(void)`-cast the genuinely-
/// unused params or the generated C fails `gcc -std=c99 -Wall -Wextra
/// -Wpedantic -Werror` with `-Werror=unused-parameter` (TD-BUG-1). Pseudo-
/// states (initial/final/history) host no transitions, matching the
/// dispatch visitors' own traversal.
pub fn machine_has_transitions(ctx: &MachineEmitCtx<'_>) -> bool {
    fn walk(states: &[StateNode]) -> bool {
        states.iter().any(|s| match s {
            StateNode::Simple(ss) => !ss.transitions.is_empty(),
            StateNode::Composite(c) => {
                !c.transitions.is_empty() || c.regions.iter().any(|r| walk(&r.states))
            }
            StateNode::Parallel(p) => {
                !p.transitions.is_empty() || p.regions.iter().any(|r| walk(&r.states))
            }
            // v1.1-W2d: a submachine ref-state carries the parent's own
            // `on EVT` / `done ->` transitions (Doc 09 §4.11). They emit a
            // per-state case like any other state's, so a machine whose
            // only transitions live on a ref-state is NOT transition-free
            // (otherwise the spurious `(void)m;(void)ev;` cast would shadow
            // a used param and the ref-state cases would be unreachable).
            StateNode::Submachine(sm) => !sm.transitions.is_empty(),
            _ => false,
        })
    }
    walk(&ctx.machine.root.states)
}

fn emit_per_state_helpers(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* Per-state transition try function. Returns true if a transition fired. */\n",
    ));
    s.push_str(&format!(
        "static bool {prefix}_try_transitions_in_state({prefix}_t *m, {prefix}_StateId_t s, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    // TD-BUG-1: a valid zero-transition machine emits no per-state case, so
    // the `switch (s)` below references `s` but neither `m` nor `ev`. Cast
    // the genuinely-unused params so the body stays `-Werror=unused-
    // parameter`-clean. `s` is always used (it is the switch discriminant),
    // so it is NOT cast — blanket-casting a used param would itself warn
    // under some toolchains and is misleading.
    if !machine_has_transitions(ctx) {
        s.push_str("    (void)m; (void)ev;\n");
    }
    s.push_str("    switch (s) {\n");

    // R2.1 (2026-05-15): traversal delegated to `IrVisitor::walk_state` so
    // adding a new pseudo-state variant only requires editing
    // `fsm-ir/src/visitor.rs`. Per-variant emit logic stays here. We start
    // at the root region (not the machine) because the switch dispatch
    // emits cases for *this* machine only; submachines (Doc 09 §4.11)
    // emit their own switch.
    let mut emitter = SwitchEmitter { ctx, out: &mut s };
    emitter.visit_region(&ctx.machine.root);
    s.push_str("    default: break;\n");
    s.push_str("    }\n");
    s.push_str("    return false;\n");
    s.push_str("}\n");
    s
}

/// Per-state-case emitter for the switch-strategy dispatch. Implemented as
/// an `IrVisitor` so traversal lives in `fsm-ir` (R2.1). The traversal-only
/// concern (visit every state recursively) is the trait default; this struct
/// supplies the per-state emit body.
struct SwitchEmitter<'a, 'm> {
    ctx: &'a MachineEmitCtx<'m>,
    out: &'a mut String,
}

impl<'a, 'm> IrVisitor for SwitchEmitter<'a, 'm> {
    fn visit_state(&mut self, s: &StateNode) {
        let (id, transitions) = match s {
            StateNode::Simple(ss) => (&ss.id, &ss.transitions),
            StateNode::Composite(c) => (&c.id, &c.transitions),
            StateNode::Parallel(p) => (&p.id, &p.transitions),
            // v1.1-W2d: a `state X is Sub` ref-state's own transitions
            // (`SubmachineRef.transitions` — parent-level `on EVT` /
            // `done -> Target`, Doc 09 §4.11) dispatch exactly like any
            // other state's. Emitting them as a normal per-state case is
            // what makes transition-wins observable (a ref-state `on EVT`
            // beats delegation) and what fires `done -> Target` when the
            // sub-completion synthesises `EVENT__COMPLETION`.
            StateNode::Submachine(sm) => (&sm.id, &sm.transitions),
            _ => {
                // Pseudo-states have no per-state switch case. Still descend
                // so any future regions inside (currently none) are visited.
                walk_state(self, s);
                return;
            }
        };
        if !transitions.is_empty() {
            let idx = self.ctx.index.must_lookup(id);
            let rec = self.ctx.index.get(idx);
            self.out.push_str(&format!(
                "    case {macro}_STATE_{name}: /* {dsl} */\n",
                macro = self.ctx.macro_prefix(),
                name = rec.c_name,
                dsl = rec.dsl_name,
            ));
            // W7-FU-1 (Doc 08 §4.1/§4.2): a state may declare two or more
            // transitions on the SAME event disambiguated by guards
            // (`on E [g1] -> A` / `on E [g2] -> B` / optional `on E -> C`
            // fallback). The candidate set for one (source,event) must
            // collapse to a SINGLE C `case <EVENT>:` whose body evaluates the
            // candidates' guards in (priority asc, document order) and takes
            // the FIRST enabled one (an unguarded / `[else]` candidate is
            // always-enabled = catch-all). Emitting one `case` per transition
            // (the pre-fix bug) produced duplicate C `case` labels (gcc hard
            // error) and made the 2nd+ guarded transition unreachable.
            //
            // Sort by priority ascending (lower number wins, Doc 08 §4.2);
            // `sort_by` is STABLE so equal-priority candidates keep their IR
            // Vec order = document order (Doc 08 §4.2 tiebreak), matching the
            // simulator's `select_transitions` exactly (sim is the oracle).
            let mut sorted: Vec<&TransitionObject> = transitions.iter().collect();
            sorted.sort_by(|a, b| a.priority.cmp(&b.priority));
            // Group the (now priority/doc-order-sorted) candidates by their
            // resolved C event enum, PRESERVING first-appearance order of the
            // events and the within-event candidate order. One emitted `case`
            // per distinct event; its body chains every candidate.
            let mut event_order: Vec<String> = Vec::new();
            let mut by_event: std::collections::HashMap<String, Vec<&TransitionObject>> =
                std::collections::HashMap::new();
            for t in &sorted {
                let key = trigger_event_c(t, self.ctx);
                if !by_event.contains_key(&key) {
                    event_order.push(key.clone());
                }
                by_event.entry(key).or_default().push(t);
            }
            self.out.push_str("        switch (ev->id) {\n");
            for event_c in &event_order {
                emit_event_case(event_c, &by_event[event_c], self.ctx, self.out);
            }
            self.out.push_str("        default: break;\n");
            self.out.push_str("        }\n");
            self.out.push_str("        break;\n");
        }
        walk_state(self, s);
    }
}

/// Resolve a transition's trigger to the C event-enum identifier its
/// `switch (ev->id)` case is keyed by. Factored out of the old
/// `emit_one_case` so the per-state emitter can GROUP transitions by event
/// (W7-FU-1) before emitting one case per event.
fn trigger_event_c(t: &TransitionObject, ctx: &MachineEmitCtx<'_>) -> String {
    let trigger_id = trigger_id_of(t, ctx);
    if trigger_id.starts_with(&ctx.macro_prefix()) {
        trigger_id
    } else {
        ctx.event_c_enum(&trigger_id)
    }
}

fn trigger_id_of(t: &TransitionObject, ctx: &MachineEmitCtx<'_>) -> String {
    match &t.trigger {
        Some(fsm_ir::Trigger::Event { event_id, .. }) => event_id.clone(),
        // `done -> Y` / pre-P0-4 timer triggers come in with no trigger;
        // map to the reserved completion event id.
        Some(fsm_ir::Trigger::Completion { .. }) | None => {
            format!("{}_EVENT__COMPLETION", ctx.macro_prefix())
        }
        // P0-4: timer triggers carry the IR timer id; resolve to the
        // distinct per-timer event variant so this transition is
        // dispatched only on its own timer's fire, not on EVENT__COMPLETION.
        Some(fsm_ir::Trigger::After { timer_id, .. })
        | Some(fsm_ir::Trigger::Every { timer_id, .. }) => {
            super::timer::timer_event_c(ctx, timer_id)
                .unwrap_or_else(|| format!("{}_EVENT__COMPLETION", ctx.macro_prefix()))
        }
    }
}

/// Emit ONE `case <EVENT>:` whose body evaluates `candidates` (already in
/// priority/document order) and fires the FIRST enabled one — Doc 08 §4.1
/// (`min(candidates, key=(priority, document_order))`) restricted to one
/// (source,event) group. This is the W7-FU-1 fix: prior code emitted a
/// separate `case` per transition, yielding a duplicate C `case` label
/// (gcc hard error) and an unreachable 2nd+ guarded transition.
///
/// Each candidate is wrapped in `do { ... } while (0)`. `emit_transition_body`
/// emits `if (!guard) <on_guard_fail>` first; with `on_guard_fail = "break;"`
/// a failed guard `break`s out of THAT candidate's `do/while(0)` and control
/// falls to the next candidate. A candidate with no guard (or `[else]`,
/// which lowers to the always-true `1`) emits no guard check, so it always
/// runs its body and `return true`s — the correct catch-all/fallback that
/// terminates the chain (Doc 08 §4.2: an unguarded transition is an
/// always-enabled candidate; in document order it wins only once every
/// earlier guard has failed). If every candidate's guard is false and there
/// is no unguarded fallback, the `case` falls through to its `break;` (the
/// outer per-state `break;`), i.e. the event is not consumed in this state
/// and the leaf-to-root walk continues to the parent (Doc 08 §4.1) — exactly
/// the simulator's behaviour (no candidate enabled => no transition selected
/// for this state, bubble up).
fn emit_event_case(
    event_c: &str,
    candidates: &[&TransitionObject],
    ctx: &MachineEmitCtx<'_>,
    out: &mut String,
) {
    out.push_str(&format!("        case {}: {{\n", event_c));
    for t in candidates {
        let trigger_id = trigger_id_of(t, ctx);
        // Resolve the event-specific payload root. The payload union is
        // keyed by event name (`ev->__payload.FAULT`), so `payload.code`
        // in the DSL must lower to `ev->__payload.FAULT.code` inside
        // FAULT's case body. Falling back to the bare union root keeps the
        // code shape sane for events without a declared payload.
        let payload_prefix = trigger_event_payload_prefix(ctx, &trigger_id);
        // `do { ... } while (0)` scopes one candidate: a failed guard
        // `break`s this candidate only and falls to the next.
        out.push_str("            do {\n");
        super::transition::emit_transition_body(
            t,
            ctx,
            &payload_prefix,
            /*indent_spaces=*/ 16,
            /*on_guard_fail=*/ "break;",
            out,
        );
        out.push_str("                return true;\n");
        out.push_str("            } while (0);\n");
    }
    out.push_str("            break;\n");
    out.push_str("        }\n");
}

fn emit_outer_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let has_defer = super::defer::machine_has_defer(ctx);
    let sub_refs = super::submachine::collect_sub_refs(ctx);

    // v1.1-W2d: submachine delegation + completion sweep, emitted AFTER the
    // parent leaf-to-root walk (so a parent-level ref-state transition wins
    // — transition-wins, mirroring W2c's `select_transitions` before
    // `try_delegate_to_submachine`) and BEFORE the defer hook (a delegated
    // event was consumed by the sub; it is neither held nor discarded —
    // W2c's `try_delegate_to_submachine` returns the consumed records). The
    // completion sweep recursively re-enters `..._dispatch` with a
    // synthetic `EVENT__COMPLETION`, where the parent walk fires the
    // ref-state `done -> Target` through the EXISTING machinery — exactly
    // W2c enqueuing `Completion(ref_id)` and letting the existing R1 path
    // fire it (the `_completion_depth` watchdog already bounds §9.4).
    let delegation = if sub_refs.is_empty() {
        String::new()
    } else {
        let mut d = String::new();
        // Event routing — `__delegated_any` write is only consumed by the
        // defer hook, so emit it only when this machine declares `defer`
        // (else an unread bool trips -Werror=unused-but-set-variable —
        // TD-BUG-1). Then the completion sweep (sub Final → synthetic
        // `EVENT__COMPLETION` → existing `done ->` machinery). The switch
        // strategy executes transitions inline during the parent walk, so
        // sweeping here (post-walk) is recursion-safe — a fired `done`
        // already exited the ref-state.
        super::submachine::emit_delegation_block(ctx, &sub_refs, "    ", has_defer, &mut d);
        super::submachine::emit_completion_sweep(ctx, &sub_refs, "    ", &mut d);
        d
    };

    // v1.1 deferral hook A: an unconsumed event whose id is deferred by the
    // active configuration is HELD (not discarded). Checked AFTER the
    // leaf-to-root search fails for every region, so an enabled transition
    // always wins (transition-wins, UML 2.5.1 §14.2.3.9.1 / Doc 08 §10.1).
    // `fired_in_region[r]` is also set by a delegated-and-consumed event, so
    // gating on "no region fired" keeps a delegated event from being held
    // (W2c treats a delegated event as consumed, not deferred).
    let defer_hold = if has_defer {
        let sub_guard = if sub_refs.is_empty() {
            String::new()
        } else {
            // A delegated-and-consumed event marks its region fired; do not
            // also defer it (it was handled inside the sub).
            String::from(" && !__delegated_any")
        };
        format!(
            "    if (!fired_any{sub_guard} && {prefix}_active_config_defers(m, ev->id)) {{\n\
             \x20       {prefix}_defer_push(m, ev);\n\
             \x20       return;\n\
             \x20   }}\n",
            prefix = prefix,
            sub_guard = sub_guard,
        )
    } else {
        String::new()
    };

    // v1.1 deferral hook B: a fired transition may have exited a deferring
    // state. Mirror the simulator's `run_step` ordering exactly:
    //
    //   sim step 2: execute transition
    //   sim step 3: release_deferred  → released events prepended to queue
    //   sim step 4: check_completion  → completion event push_front'd
    //   (return; drain pops completion FIRST, then the released events)
    //
    // So in C: release the buffer to the queue front (Doc 08 §10.2), then
    // run completion synchronously (the simulator processes completion
    // before the released deferred events because completion is push_front'd
    // *after* the deferred prepend), THEN drain the released events. Draining
    // re-runs the same RTC machinery the simulator's `drain_internal_queue`
    // does for prepended events (Doc 08 §10.3). A redispatched event that
    // finds no consuming transition and is no longer deferred discards
    // normally — identical to the simulator.
    let release_call = if has_defer {
        format!("        {prefix}_release_deferred(m);\n", prefix = prefix)
    } else {
        String::new()
    };
    let drain_released = if has_defer {
        format!(
            "        {{\n\
             \x20           {prefix}_Event_t __rd;\n\
             \x20           while ({prefix}_dequeue(m, &__rd)) {{\n\
             \x20               {prefix}_dispatch(m, &__rd);\n\
             \x20           }}\n\
             \x20       }}\n",
            prefix = prefix,
        )
    } else {
        String::new()
    };

    format!(
        r#"void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev) {{
    /* B-10 + B-11: per-region ancestor walk. For each active leaf in
     * `_active[]`, walk leaf-to-root via parent_table; the first ancestor
     * with a matching transition fires it. Regions iterate independently,
     * so a single event can drive every region in a parallel state in the
     * same RTC step.
     *
     * v1.1 (2026-05-15): deferred-event runtime. The audit P0-5 option-b
     * stopgap (analyzer rejected every `defer` with FSM-E0903) is retired;
     * `defer` is now a real UML 2.5.1 §14.2.3.9.1 feature. Hook A holds an
     * unconsumed-but-deferred event; hook B releases held events to the
     * queue front on a configuration-changing transition and drains them.
     * Both gated on `machine_has_defer`, so non-defer machines emit the
     * exact code they did before. */
    bool fired_any = false;
    bool fired_in_region[{macro}_MAX_PARALLEL_REGIONS] = {{ false }};
{delegated_decl}    /* Snapshot active region count up front so transition side effects
     * that change `_active_count` (e.g. cross-out-of-parallel) do not
     * shrink the iteration mid-walk. Doc 08 §4.1: process innermost
     * leaves first (slots 1..N are nested below slot 0), so iterate
     * from high to low. */
    uint8_t initial_active = m->_active_count;
    for (int8_t r = (int8_t)initial_active - 1; r >= 0; r--) {{
        /* Slot may have been cleared by a sibling-region transition
         * (cross-out-of-parallel). */
        if ((uint8_t)r >= m->_active_count) continue;
        /* Skip slots whose region already fired in this RTC step. */
        if (fired_in_region[r]) continue;
        {prefix}_StateId_t s = m->_active[r];
        while (1) {{
            if ({prefix}_try_transitions_in_state(m, s, ev)) {{
                fired_any = true;
                fired_in_region[r] = true;
                break;
            }}
            if (s == {macro}_STATE_ROOT) break;
            s = {prefix}_parent_table[s];
        }}
    }}
{delegation}{defer_hold}    if (fired_any) {{
{release_call}        {prefix}_handle_completion(m);
{drain_released}    }}
    /* Otherwise: no ancestor handled the event — discard per Doc 08 §3.1. */
}}
"#,
        prefix = prefix,
        macro = macro_prefix,
        delegated_decl = if sub_refs.is_empty() || !has_defer {
            // Only the defer hook reads `__delegated_any`; with no `defer`
            // it would be an unread bool (-Werror=unused-but-set-variable).
            String::new()
        } else {
            String::from(
                "    /* v1.1-W2d: set by a delegated-and-consumed event so the\n\
                 \x20    * defer hook does not also hold it (W2c treats a delegated\n\
                 \x20    * event as consumed). */\n\
                 \x20   bool __delegated_any = false;\n",
            )
        },
        delegation = delegation,
        defer_hold = defer_hold,
        release_call = release_call,
        drain_released = drain_released,
    )
}

/// Build the per-transition payload prefix. Each event with a non-empty
/// payload schema gets its own member inside the `__payload` union (see
/// `header.rs::emit_payload_structs`), so `payload.X` must address through
/// that member. When the event has no payload schema, fall back to the
/// bare union root — the codegen still emits the dereference but the user
/// guard/action shouldn't reference it.
fn trigger_event_payload_prefix(ctx: &MachineEmitCtx<'_>, trigger_id: &str) -> String {
    let event = ctx
        .machine
        .events
        .iter()
        .find(|e| e.id == trigger_id || e.stable_id == trigger_id);
    match event {
        Some(ev) if !ev.payload.is_empty() => format!("ev->__payload.{}", ev.name),
        _ => "ev->__payload".to_string(),
    }
}
