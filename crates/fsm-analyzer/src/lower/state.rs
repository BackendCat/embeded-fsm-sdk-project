//! State-tree lowering — states, regions, pseudo-states, transitions,
//! timers, defers, and the transition builder.
//!
//! AD-3 (2026-05-15): these were the bulk of the `LoweringCtx` `impl`
//! (Audit C LCOM cluster C). Each is a verbatim transcription with `self`
//! replaced by explicit `&mut IdMinter` / `&LocCtx`. The traversal order,
//! the counter increment order, every `format!` template, and the
//! `state_target_id` calls are unchanged ⇒ the lowered IR (ids, stable
//! ids, locs, transition order) is byte-identical pre/post.

use fsm_ir::{
    BranchHint, ChoiceBranch as IrChoiceBranch, ChoiceState, CompositeState,
    DeferDecl as IrDeferDecl, FinalState, ForkPseudo, GuardExpr, HistoryKind, HistoryObject,
    InitialPseudo, JoinPseudo, JunctionState, ParallelState, RegionObject, SimpleState, StateNode,
    Statement, SubmachineRef, TimerKind, TimerObject, TransitionKind, TransitionObject, Trigger,
    DEFAULT_TRANSITION_PRIORITY,
};
use fsm_parser::ast::{self, AstNode, BranchHint as AstBranchHint};
// **W0 / Doc 29 §3.4 (R-2 + R-3 leave-and-explain).** This file
// deliberately retains `fsm_parser::cst` for: (R-2) `lower_state_children`'s
// `children()+kind()` dispatch over INITIAL/STATE/REGION/FINAL/HISTORY/
// CHOICE/JUNCTION/FORK/JOIN children in **exact source order across
// heterogeneous kinds** — the IR id-minter is order-sensitive
// (`lower_split_byte_identity.rs` pins it byte-for-byte); the typed AST has
// per-kind iterators but no ordered heterogeneous-child iterator, and adding
// a typed `enum StateChild` ordered iterator whose only consumer is this one
// loop is the refactor-to-number trap; (R-3) the `eval_i64` timer-duration
// const-fold over the deliberately-shallow expression CST. Folding either
// would worsen clarity / risk the P0-1 byte-identity regression class for
// zero behaviour gain — Doc 00 §11.44/§11.49, the DRIFT-2 `LineIndex`
// precedent. The single-construct scans were relocated to typed parser
// accessors (see the free-helpers section below); only the order-critical /
// shallow-AST residuals stay.
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use super::expr::{lower_action_block, lower_guard_clause};
use super::ids::IdMinter;
use super::loc::LocCtx;
use crate::util::submachine_ref_is_nested;

/// Lower the direct children of `parent` (a machine or a state) into a
/// list of [`StateNode`]s. Returns the produced state list and the
/// pseudo-state ID of the **first** Initial pseudo-state emitted (so the
/// caller can plumb it into [`RegionObject::initial`] per Doc 09 §5).
/// `None` when the source declared no `initial` keyword in this scope.
pub(super) fn lower_state_children(
    ids: &mut IdMinter,
    locs: &LocCtx,
    parent: &SyntaxNode,
) -> (Vec<StateNode>, Option<String>) {
    let mut out = Vec::new();
    let mut initial_pseudo_id: Option<String> = None;
    // INITIAL_DECL becomes an InitialPseudo state. Its `target` MUST be a
    // state ID per Doc 09 §4.4, so we translate the AST-level name
    // ("Idle") into the canonical state ID form ("s-<machine>-Idle")
    // produced by [`IdMinter::state_target_id`] — the same form
    // transitions use.
    for child in parent.children() {
        match child.kind() {
            SyntaxKind::INITIAL_DECL => {
                if let Some(init) = ast::InitialDecl::cast(child.clone()) {
                    let target_name = init.target().unwrap_or_default();
                    let target_id = ids.state_target_id(&target_name);
                    let id = ids.next_pseudo_id("initial");
                    if initial_pseudo_id.is_none() {
                        initial_pseudo_id = Some(id.clone());
                    }
                    out.push(StateNode::Initial(InitialPseudo {
                        id,
                        target: target_id,
                        loc: locs.loc(&child),
                    }));
                }
            }
            SyntaxKind::STATE_DECL => {
                if let Some(state) = ast::StateDecl::cast(child.clone()) {
                    out.push(lower_state(ids, locs, &state));
                }
            }
            SyntaxKind::REGION_DECL => {
                // top-level region: the parent must be a parallel state —
                // we surface it as a ParallelState with one region. The
                // surrounding caller orchestrates multi-region grouping.
                // For lowering simplicity, regions inside a state become
                // children of a `ParallelState` (see lower_state).
            }
            SyntaxKind::FINAL_DECL => {
                if let Some(f) = ast::FinalDecl::cast(child.clone()) {
                    let name = f.name().unwrap_or_default();
                    out.push(StateNode::Final(FinalState {
                        id: ids.state_id(&name),
                        stable_id: format!("M:{}:final:{name}", ids.machine_name),
                        name: name.clone(),
                        loc: locs.loc(&child),
                    }));
                }
            }
            SyntaxKind::SHALLOW_HISTORY_DECL => {
                out.push(lower_history(ids, locs, &child, HistoryKind::Shallow));
            }
            SyntaxKind::DEEP_HISTORY_DECL => {
                out.push(lower_history(ids, locs, &child, HistoryKind::Deep));
            }
            SyntaxKind::CHOICE_DECL => out.push(lower_choice(ids, locs, &child)),
            SyntaxKind::JUNCTION_DECL => out.push(lower_junction(ids, locs, &child)),
            SyntaxKind::FORK_DECL => out.push(lower_fork(ids, locs, &child)),
            SyntaxKind::JOIN_DECL => out.push(lower_join(ids, locs, &child)),
            _ => {}
        }
    }
    (out, initial_pseudo_id)
}

fn lower_state(ids: &mut IdMinter, locs: &LocCtx, state: &ast::StateDecl) -> StateNode {
    let name = state.name().unwrap_or_default();
    let id = ids.state_id(&name);
    let stable_id = format!("M:{}:state:{name}", ids.machine_name);

    let entry = state
        .entry()
        .and_then(|e| e.action_block())
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    let exit = state
        .exit()
        .and_then(|e| e.action_block())
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    let mut transitions = lower_transitions(ids, locs, state, &id, &name);
    let (timers, timer_transitions) = lower_timers(ids, locs, state, &id);
    transitions.extend(timer_transitions);
    let defers = lower_defers(ids, locs, state);

    // `state X is Sub { … }` (Doc 04 §15 / Doc 09 §4.11). The `is` binding
    // takes precedence over the state's structural shape: the state IS an
    // instance of the named submachine template, so it lowers to a
    // `StateNode::Submachine`, not Simple/Composite. The state's own
    // transitions (external/local/internal + `done ->` completion + timer
    // edges) are carried on the ref per §4.11 — the parent dispatches them
    // around the sub-instance (running the sub-instance itself is W2c).
    // `submachine_id` resolves to the template's MachineObject id
    // (`m-<SubName>`); FSM-E0103 (unknown ref) / FSM-E0500 / FSM-E0501 /
    // FSM-E0502 are emitted by `checks::submachine` against the same name —
    // when unresolved we still emit a structurally-valid SubmachineRef
    // (partial-IR principle, Doc 09 §1) so downstream stays schema-valid.
    //
    // **P1-2 defence-in-depth:** ONLY a *top-level* ref lowers to
    // `StateNode::Submachine`. A ref nested inside a composite/parallel
    // state is rejected at analysis (`checks::submachine` emits an
    // error-severity E0502, so `fsm check`/`fsm generate` already abort
    // before codegen). Belt-and-suspenders, the lowerer additionally
    // refuses to produce a `StateNode::Submachine` for a nested ref —
    // codegen's `collect_sub_refs` walks only the root region, so a nested
    // `StateNode::Submachine` would be emitted as a leaf whose
    // entry_/exit_ are called but whose prototypes are suppressed →
    // non-compilable C. By falling through to the normal structural
    // lowering (Simple/Composite/Parallel from the state's own children) we
    // *guarantee* the IR can never carry a nested `StateNode::Submachine`,
    // independent of whether the diagnostic gate is bypassed (e.g. a
    // future caller that lowers despite errors). Nested-submachine *support*
    // is the tracked SUB-FU-2 follow-up.
    //
    // Guard-clause form: enter the submachine path ONLY for a ref that is
    // both present AND top-level. A nested ref falls through (the `is`
    // binding is treated as absent for the rejected position) to the normal
    // structural lowering below — its user-visible error is the E0502
    // diagnostic from `checks::submachine`; this branch only keeps the
    // partial IR structurally safe for any downstream that lowers despite
    // errors.
    if let Some(sref) = state
        .submachine_ref()
        .filter(|s| !submachine_ref_is_nested(s.syntax()))
    {
        let sub_name = sref.name().unwrap_or_default();
        return StateNode::Submachine(SubmachineRef {
            id,
            stable_id,
            name,
            submachine_id: format!("m-{sub_name}"),
            // Named entry/exit-point *mappings* (`entry_point` /
            // `exit_point` wiring, Doc 09 §4.12) are not expressible in the
            // current `is Sub { … }` grammar — W2a's SUBMACHINE_REF carries
            // only the submachine name. Implicit-initial entry + final-state
            // exit (Doc 08 §12.2/§12.3) need no mapping table. Left empty;
            // populated if/when the grammar gains explicit point bindings.
            entry_points: Vec::new(),
            exit_points: Vec::new(),
            transitions,
            loc: locs.loc(state.syntax()),
        });
    }

    let regions: Vec<_> = state.regions().collect();
    let nested_states: Vec<_> = state.nested_states().collect();

    if !regions.is_empty() {
        // Parallel or composite-with-regions: build region objects from
        // each REGION_DECL child.
        let region_objs: Vec<RegionObject> =
            regions.iter().map(|r| lower_region(ids, locs, r)).collect();
        if region_objs.len() >= 2 {
            return StateNode::Parallel(ParallelState {
                id,
                stable_id,
                name: name.clone(),
                entry,
                exit,
                transitions,
                timers,
                defers,
                regions: region_objs,
                loc: locs.loc(state.syntax()),
            });
        }
        return StateNode::Composite(CompositeState {
            id,
            stable_id,
            name: name.clone(),
            entry,
            exit,
            transitions,
            timers,
            defers,
            regions: region_objs,
            history: None,
            loc: locs.loc(state.syntax()),
        });
    }
    if !nested_states.is_empty() {
        // Composite with implicit region — collect children into one
        // synthetic region. Doc 09 §5 requires `region.initial` to be the
        // ID of an Initial pseudo-state; `lower_state_children` emits it
        // and returns its id.
        let (states, inner_initial_pseudo_id) = lower_state_children(ids, locs, state.syntax());
        let region = RegionObject {
            id: format!("r-{}-{name}", ids.machine_name),
            stable_id: None,
            name: format!("{name}__r"),
            initial: inner_initial_pseudo_id.unwrap_or_default(),
            states,
            priority: 0,
            loc: locs.loc(state.syntax()),
        };
        return StateNode::Composite(CompositeState {
            id,
            stable_id,
            name: name.clone(),
            entry,
            exit,
            transitions,
            timers,
            defers,
            regions: vec![region],
            history: extract_history(state, ids, locs),
            loc: locs.loc(state.syntax()),
        });
    }
    StateNode::Simple(SimpleState {
        id,
        stable_id,
        name,
        entry,
        exit,
        transitions,
        timers,
        defers,
        loc: locs.loc(state.syntax()),
    })
}

fn lower_region(ids: &mut IdMinter, locs: &LocCtx, r: &ast::RegionDecl) -> RegionObject {
    let name = r
        .name()
        .unwrap_or_else(|| format!("__region_{}", ids.pseudo_counter()));
    // `region.initial` holds the ID of the Initial pseudo-state (Doc 09
    // §5). `lower_state_children` emits it inside `states` and gives us
    // back its id.
    let (states, initial_pseudo_id) = lower_state_children(ids, locs, r.syntax());
    RegionObject {
        id: format!("r-{}-{name}", ids.machine_name),
        stable_id: None,
        name,
        initial: initial_pseudo_id.unwrap_or_default(),
        states,
        priority: 0,
        loc: locs.loc(r.syntax()),
    }
}

fn lower_history(
    ids: &mut IdMinter,
    locs: &LocCtx,
    node: &SyntaxNode,
    kind: HistoryKind,
) -> StateNode {
    let (name, default) = match kind {
        HistoryKind::Shallow => {
            let h = ast::ShallowHistoryDecl::cast(node.clone());
            (
                h.as_ref().and_then(|n| n.name()).unwrap_or_default(),
                h.and_then(|n| n.default()).and_then(|d| d.target()),
            )
        }
        HistoryKind::Deep => {
            let h = ast::DeepHistoryDecl::cast(node.clone());
            (
                h.as_ref().and_then(|n| n.name()).unwrap_or_default(),
                h.and_then(|n| n.default()).and_then(|d| d.target()),
            )
        }
    };
    // Missing default is rejected by [`crate::checks::history`] — we
    // still produce an IR entry with empty default_target so codegen
    // never sees `Option::None` (matching the type-system embodiment of
    // B-14). Normalize the raw `default` name to the canonical state IR
    // id so consumers (codegen, simulator) can look it up via the same
    // `s-<Machine>-<Name>` key family used for all other transition
    // targets.
    let default_target = default.map(|n| ids.state_id(&n)).unwrap_or_default();
    StateNode::History(HistoryObject {
        id: ids.state_id(&name),
        stable_id: format!("M:{}:history:{name}", ids.machine_name),
        history_kind: kind,
        default_target,
        loc: locs.loc(node),
    })
}

// FW110-FU-A2: a `choice`/`junction` branch carries the SAME three IR
// fields a transition does — a guard, an action list, and a *resolved*
// target id — and the shipped, unforked `fsm_simulator::resolve_target`
// (interpreter.rs ≈1572-1612) is the correctness oracle: it walks the
// branches top-to-bottom, takes the first whose `eval_guard` is `Ok(true)`,
// falls back to the `GuardExpr::Else` branch, errors if none, then runs
// `branch.actions` and recurses on `branch.target`. The pre-fix lowering
// hardcoded every branch `guard: Else, actions: []` and left `target` as
// the raw DSL name (`"High"`), so `resolve_target`'s `rt.machine.node()`
// lookup missed and the engines silently produced an empty config. The
// guard/action sub-language and the target-id form are lowered EXACTLY as
// every ordinary transition does (`lower_guard_clause` /
// `lower_action_block` / `IdMinter::state_target_id` — see `lower_external`
// et al.). `[else]` needs no special-case: the grammar emits a
// `GUARD_CLAUSE` whose expr is `Expr::GuardElse`, and `lower_guard_clause`
// → `lower_guard_expr` already maps that to `GuardExpr::Else` (the exact
// discriminant `resolve_target` matches on). The `unwrap_or(Else)` is the
// defensive parse-error fallback (a branch with no `GUARD_CLAUSE` child at
// all): treating it as the else branch is the most-recoverable choice and
// is consistent with how `resolve_target` handles a non-`true` guard.
fn lower_choice_branch<B>(ids: &mut IdMinter, locs: &LocCtx, branch: &B) -> IrChoiceBranch
where
    B: ChoiceBranchAst,
{
    let guard = branch
        .branch_guard()
        .map(|g| lower_guard_clause(ids, locs, &g))
        .unwrap_or(GuardExpr::Else);
    let actions = branch
        .branch_actions()
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    IrChoiceBranch {
        guard,
        // A branch target is a *reference* to a declared state, never a
        // declaration — so `state_target_id` (the non-mutating
        // `s-<machine>-<name>` form every transition target uses), NOT
        // `state_id` (which auto-numbers anonymous declarations).
        target: ids.state_target_id(&branch.branch_target().unwrap_or_default()),
        actions,
        loc: locs.loc(branch.syntax()),
    }
}

/// The choice/junction branch shape `lower_choice_branch` needs. `ChoiceBranch`
/// and `JunctionBranch` are distinct AST node kinds with identical accessors
/// (the grammar builds the same `GUARD_CLAUSE`/`ACTION_BLOCK`/target shape for
/// both); this trait lets one lowering serve both without duplicating it.
trait ChoiceBranchAst {
    fn branch_guard(&self) -> Option<ast::GuardClause>;
    fn branch_actions(&self) -> Option<ast::ActionBlock>;
    fn branch_target(&self) -> Option<String>;
    fn syntax(&self) -> &SyntaxNode;
}

impl ChoiceBranchAst for ast::ChoiceBranch {
    fn branch_guard(&self) -> Option<ast::GuardClause> {
        self.guard()
    }
    fn branch_actions(&self) -> Option<ast::ActionBlock> {
        self.actions()
    }
    fn branch_target(&self) -> Option<String> {
        self.target()
    }
    fn syntax(&self) -> &SyntaxNode {
        AstNode::syntax(self)
    }
}

impl ChoiceBranchAst for ast::JunctionBranch {
    fn branch_guard(&self) -> Option<ast::GuardClause> {
        self.guard()
    }
    fn branch_actions(&self) -> Option<ast::ActionBlock> {
        self.actions()
    }
    fn branch_target(&self) -> Option<String> {
        self.target()
    }
    fn syntax(&self) -> &SyntaxNode {
        AstNode::syntax(self)
    }
}

fn lower_choice(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> StateNode {
    let c = ast::ChoiceDecl::cast(node.clone());
    let name = c.as_ref().and_then(|n| n.name()).unwrap_or_default();
    let mut branches = Vec::new();
    if let Some(c) = c {
        for branch in c.branches() {
            branches.push(lower_choice_branch(ids, locs, &branch));
        }
    }
    StateNode::Choice(ChoiceState {
        id: ids.state_id(&name),
        stable_id: format!("M:{}:choice:{name}", ids.machine_name),
        branches,
        loc: locs.loc(node),
    })
}

fn lower_junction(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> StateNode {
    let j = ast::JunctionDecl::cast(node.clone());
    let name = j.as_ref().and_then(|n| n.name()).unwrap_or_default();
    let mut branches = Vec::new();
    if let Some(j) = j {
        for branch in j.branches() {
            branches.push(lower_choice_branch(ids, locs, &branch));
        }
    }
    StateNode::Junction(JunctionState {
        id: ids.state_id(&name),
        stable_id: format!("M:{}:junction:{name}", ids.machine_name),
        branches,
        loc: locs.loc(node),
    })
}

fn lower_fork(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> StateNode {
    let f = ast::ForkDecl::cast(node.clone());
    let name = f.as_ref().and_then(|n| n.name()).unwrap_or_default();
    let targets = f
        .and_then(|n| n.targets())
        .map(|t| t.names())
        .unwrap_or_default();
    StateNode::Fork(ForkPseudo {
        id: ids.state_id(&name),
        stable_id: format!("M:{}:fork:{name}", ids.machine_name),
        targets,
        loc: locs.loc(node),
    })
}

fn lower_join(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> StateNode {
    let j = ast::JoinDecl::cast(node.clone());
    let name = j.as_ref().and_then(|n| n.name()).unwrap_or_default();
    let sources = j
        .as_ref()
        .and_then(|n| n.sources())
        .map(|t| t.names())
        .unwrap_or_default();
    let target = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .last()
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    StateNode::Join(JoinPseudo {
        id: ids.state_id(&name),
        stable_id: format!("M:{}:join:{name}", ids.machine_name),
        sources,
        target,
        actions: Vec::new(),
        loc: locs.loc(node),
    })
}

fn lower_transitions(
    ids: &mut IdMinter,
    locs: &LocCtx,
    state: &ast::StateDecl,
    source_id: &str,
    source_name: &str,
) -> Vec<TransitionObject> {
    let mut out = Vec::new();
    for t in state.transitions() {
        if let Some(tr) = lower_external(ids, locs, &t, source_id, source_name) {
            out.push(tr);
        }
    }
    for t in state.internal_transitions() {
        if let Some(tr) = lower_internal(ids, locs, &t, source_id) {
            out.push(tr);
        }
    }
    for t in state.local_transitions() {
        if let Some(tr) = lower_local(ids, locs, &t, source_id) {
            out.push(tr);
        }
    }
    for c in state.completions() {
        if let Some(tr) = lower_completion(ids, locs, &c, source_id) {
            out.push(tr);
        }
    }
    out
}

fn lower_external(
    ids: &mut IdMinter,
    locs: &LocCtx,
    t: &ast::TransitionDecl,
    source_id: &str,
    source_name: &str,
) -> Option<TransitionObject> {
    let trigger_name = t.trigger()?;
    let payload_binding = t.payload_binding();
    let target_name = t.target().unwrap_or_default();
    let priority = t
        .priority()
        .and_then(|p| p.value())
        .unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY));
    let guard = t.guard().map(|g| lower_guard_clause(ids, locs, &g));
    let actions = t
        .actions()
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    Some(build_transition(
        ids,
        locs,
        source_id,
        &ids.state_target_id(&target_name),
        TransitionKind::External,
        Some(Trigger::Event {
            event_id: format!("ev-{}-{trigger_name}", ids.machine_name),
            payload_binding,
        }),
        priority,
        guard,
        actions,
        lower_branch_hint(t.branch_hint()),
        t.syntax(),
        source_name,
    ))
}

fn lower_internal(
    ids: &mut IdMinter,
    locs: &LocCtx,
    t: &ast::InternalDecl,
    source_id: &str,
) -> Option<TransitionObject> {
    let trigger_name = t.trigger()?;
    let payload_binding = t.payload_binding();
    let priority = t
        .priority()
        .and_then(|p| p.value())
        .unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY));
    let guard = t.guard().map(|g| lower_guard_clause(ids, locs, &g));
    let actions = t
        .actions()
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    Some(build_transition(
        ids,
        locs,
        source_id,
        source_id,
        TransitionKind::Internal,
        Some(Trigger::Event {
            event_id: format!("ev-{}-{trigger_name}", ids.machine_name),
            payload_binding,
        }),
        priority,
        guard,
        actions,
        lower_branch_hint(t.branch_hint()),
        t.syntax(),
        "",
    ))
}

fn lower_local(
    ids: &mut IdMinter,
    locs: &LocCtx,
    t: &ast::LocalDecl,
    source_id: &str,
) -> Option<TransitionObject> {
    let trigger_name = t.trigger()?;
    let payload_binding = t.payload_binding();
    let target_name = t.target().unwrap_or_default();
    let priority = t
        .priority()
        .and_then(|p| p.value())
        .unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY));
    let guard = t.guard().map(|g| lower_guard_clause(ids, locs, &g));
    let actions = t
        .actions()
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    Some(build_transition(
        ids,
        locs,
        source_id,
        &ids.state_target_id(&target_name),
        TransitionKind::Local,
        Some(Trigger::Event {
            event_id: format!("ev-{}-{trigger_name}", ids.machine_name),
            payload_binding,
        }),
        priority,
        guard,
        actions,
        lower_branch_hint(t.branch_hint()),
        t.syntax(),
        "",
    ))
}

fn lower_completion(
    ids: &mut IdMinter,
    locs: &LocCtx,
    c: &ast::CompletionDecl,
    source_id: &str,
) -> Option<TransitionObject> {
    let target_name = c.target().unwrap_or_default();
    let priority = c
        .priority()
        .and_then(|p| p.value())
        .unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY));
    let guard = c.guard().map(|g| lower_guard_clause(ids, locs, &g));
    let actions = c
        .actions()
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    Some(build_transition(
        ids,
        locs,
        source_id,
        &ids.state_target_id(&target_name),
        TransitionKind::Completion,
        None,
        priority,
        guard,
        actions,
        lower_branch_hint(c.branch_hint()),
        c.syntax(),
        "",
    ))
}

/// v1.1-W4: map the parser-side AST hint enum to the IR enum. Kept here
/// (in the analyzer, the AST→IR boundary) so `fsm-parser` need not depend on
/// `fsm-ir`. A no-op lift — there is no diagnostic: a hint is always
/// syntactically valid and the grammar admits at most one prefix per
/// transition, so `Likely`/`Rare` are the only inputs.
fn lower_branch_hint(h: Option<AstBranchHint>) -> Option<BranchHint> {
    h.map(|h| match h {
        AstBranchHint::Likely => BranchHint::Likely,
        AstBranchHint::Rare => BranchHint::Rare,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_transition(
    ids: &mut IdMinter,
    locs: &LocCtx,
    source_id: &str,
    target_id: &str,
    kind: TransitionKind,
    trigger: Option<Trigger>,
    priority: i64,
    guard: Option<GuardExpr>,
    actions: Vec<Statement>,
    hint: Option<BranchHint>,
    node: &SyntaxNode,
    _source_name: &str,
) -> TransitionObject {
    let id = ids.next_transition_id();
    #[allow(deprecated)]
    TransitionObject {
        id: id.clone(),
        stable_id: format!("M:{}:transition:{id}", ids.machine_name),
        source: source_id.to_string(),
        target: target_id.to_string(),
        trigger,
        guard,
        actions,
        priority: priority.clamp(0, u16::MAX as i64) as u16,
        kind,
        internal: matches!(kind, TransitionKind::Internal),
        hint,
        loc: locs.loc(node),
    }
}

/// Lower `after` / `every` / `every_internal` declarations into the
/// owning state's [`TimerObject`] list AND, for the targeted forms,
/// matching [`TransitionObject`]s with `Trigger::After` / `Trigger::Every`
/// carrying the timer's stable id (P0-4 fix). Without the timer-bound
/// trigger, downstream codegen had no way to dispatch the timer event
/// independently from `done` completion and would collapse both into
/// `EVENT__COMPLETION`.
fn lower_timers(
    ids: &mut IdMinter,
    locs: &LocCtx,
    state: &ast::StateDecl,
    owner: &str,
) -> (Vec<TimerObject>, Vec<TransitionObject>) {
    let mut timers = Vec::new();
    let mut transitions = Vec::new();
    for (kind_idx, a) in state.after().enumerate() {
        if let Some(ms) = a.duration().and_then(|ce| duration_ms(&ce)) {
            let target = a.target();
            let actions = a
                .action_block()
                .map(|ab| lower_action_block(ids, locs, &ab))
                .unwrap_or_default();
            let timer_id = ids.next_pseudo_id("timer");
            let stable_id = format!("M:{}:timer:after:{}:{}", ids.machine_name, owner, kind_idx);
            if let Some(t) = target.as_ref() {
                transitions.push(build_transition(
                    ids,
                    locs,
                    owner,
                    &ids.state_target_id(t),
                    TransitionKind::External,
                    Some(Trigger::After {
                        duration_ms: ms,
                        timer_id: timer_id.clone(),
                    }),
                    0,
                    None,
                    actions.clone(),
                    lower_branch_hint(a.branch_hint()),
                    a.syntax(),
                    "",
                ));
            }
            timers.push(TimerObject {
                id: timer_id,
                stable_id,
                kind: TimerKind::After,
                duration_ms: ms,
                owner_state_id: owner.to_string(),
                target: target.map(|s| ids.state_target_id(&s)),
                actions,
                loc: locs.loc(a.syntax()),
            });
        }
    }
    for (kind_idx, e) in state.every().enumerate() {
        if let Some(ms) = e.duration().and_then(|ce| duration_ms(&ce)) {
            let target = e.target();
            let actions = e
                .action_block()
                .map(|ab| lower_action_block(ids, locs, &ab))
                .unwrap_or_default();
            let timer_id = ids.next_pseudo_id("timer");
            let stable_id = format!("M:{}:timer:every:{}:{}", ids.machine_name, owner, kind_idx);
            if let Some(t) = target.as_ref() {
                transitions.push(build_transition(
                    ids,
                    locs,
                    owner,
                    &ids.state_target_id(t),
                    TransitionKind::External,
                    Some(Trigger::Every {
                        period_ms: ms,
                        timer_id: timer_id.clone(),
                    }),
                    0,
                    None,
                    actions.clone(),
                    lower_branch_hint(e.branch_hint()),
                    e.syntax(),
                    "",
                ));
            }
            timers.push(TimerObject {
                id: timer_id,
                stable_id,
                kind: TimerKind::Every,
                duration_ms: ms,
                owner_state_id: owner.to_string(),
                target: target.map(|s| ids.state_target_id(&s)),
                actions,
                loc: locs.loc(e.syntax()),
            });
        }
    }
    for (kind_idx, e) in state.every_internal().enumerate() {
        if let Some(ms) = e.duration().and_then(|ce| duration_ms(&ce)) {
            let actions = e
                .action_block()
                .map(|ab| lower_action_block(ids, locs, &ab))
                .unwrap_or_default();
            let timer_id = ids.next_pseudo_id("timer");
            let stable_id = format!(
                "M:{}:timer:every_internal:{}:{}",
                ids.machine_name, owner, kind_idx
            );
            // `every_internal` has no target; codegen runs the actions
            // on each fire without an exit/entry sequence. We still emit
            // an internal-kind transition so the timer trigger is
            // distinguishable from `done` completion in dispatch.
            transitions.push(build_transition(
                ids,
                locs,
                owner,
                owner,
                TransitionKind::Internal,
                Some(Trigger::Every {
                    period_ms: ms,
                    timer_id: timer_id.clone(),
                }),
                0,
                None,
                actions.clone(),
                // `every_internal` is action-only — no `->` target, no
                // condition to wrap, so a hint is meaningless here. The
                // grammar still admits `likely every N ms : ...` (the
                // dispatcher gates on `KwEvery`); we deliberately drop the
                // hint for the no-target internal form (nothing to lower
                // it onto). EVERY_DECL (targeted) carries it; this does
                // not.
                None,
                e.syntax(),
                "",
            ));
            timers.push(TimerObject {
                id: timer_id,
                stable_id,
                kind: TimerKind::EveryInternal,
                duration_ms: ms,
                owner_state_id: owner.to_string(),
                target: None,
                actions,
                loc: locs.loc(e.syntax()),
            });
        }
    }
    (timers, transitions)
}

fn lower_defers(ids: &mut IdMinter, locs: &LocCtx, state: &ast::StateDecl) -> Vec<IrDeferDecl> {
    state
        .defers()
        .filter_map(|d| {
            let event = d.event()?;
            Some(IrDeferDecl {
                event_id: format!("ev-{}-{event}", ids.machine_name),
                loc: locs.loc(d.syntax()),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Free helpers — pure tree scans, no IdMinter / LocCtx state needed.
//
// **W0 (Doc 29 §3.2/§6):** the single-construct CST scans (`extract_priority`,
// `extract_trigger_payload_binding`, `action_block_under`, and the
// CONST_EXPR-find of `duration_ms`) were RELOCATED to their correct home as
// behaviour-inert typed accessors on `fsm_parser::ast` (`PriorityClause::value`,
// `{Transition,Internal,Local}Decl::payload_binding`,
// `{After,Every,EveryInternal}Decl::{action_block,duration}`). What remains
// here is the **R-2/R-3 leave-and-explain residual** (Doc 00 §11.44/§11.49,
// the DRIFT-2 `LineIndex` precedent — left-and-explained, NOT folded to hit a
// zero-`cst` count).
// ---------------------------------------------------------------------------

/// **R-2 residual (Doc 29 §3.4).** This deliberately retains the
/// `children()+kind()` CST dispatch over the *heterogeneous*
/// SHALLOW/DEEP_HISTORY_DECL children. The typed AST exposes per-kind
/// iterators but no single ordered heterogeneous-child iterator, and the IR
/// id-minter (`lower_state_children`) is order-sensitive — `lower_history`
/// shares the minting path, so the *first* history child in **source order**
/// is the one bound to the composite. A typed `enum StateChild` ordered
/// iterator would be a substantial new `fsm-parser` API whose only consumer
/// is this one helper (the refactor-to-number trap SUBAGENT §10 / Doc 00
/// §11.44 forbid); folding it would obscure the order-critical intent for
/// zero behaviour gain. Left-and-explained.
fn extract_history(
    state: &ast::StateDecl,
    ids: &mut IdMinter,
    locs: &LocCtx,
) -> Option<HistoryObject> {
    let mut found = None;
    for child in state.syntax().children() {
        let kind = child.kind();
        if matches!(
            kind,
            SyntaxKind::SHALLOW_HISTORY_DECL | SyntaxKind::DEEP_HISTORY_DECL
        ) {
            let history_kind = if kind == SyntaxKind::SHALLOW_HISTORY_DECL {
                HistoryKind::Shallow
            } else {
                HistoryKind::Deep
            };
            let StateNode::History(h) = lower_history(ids, locs, &child, history_kind) else {
                continue;
            };
            found = Some(h);
            break;
        }
    }
    found
}

/// Const-fold a timer's typed `CONST_EXPR` node (from the W0
/// `{After,Every,EveryInternal}Decl::duration()` accessor) to a `u32`
/// milliseconds value, or `None` if it is negative / non-foldable.
///
/// **R-3 residual (Doc 29 §3.4).** `eval_i64` walks the *deliberately
/// shallow* expression CST (`EXPR_LITERAL`/`EXPR_UNARY`/`EXPR_PAREN`). The
/// `Expr` typed AST is `cast`/`syntax`-only by design (no structural
/// accessors); a full typed accessor layer would relocate — not eliminate —
/// this walk into `fsm-parser` (the analyzer would still depend on the
/// shape, just via more indirection) and add a large parser public surface
/// in a 0-new-API-intended wave. This is internal-to-analyzer const folding
/// over the parser's *public* CST type used for its intended purpose; it is
/// left-and-explained, the canonical SUBAGENT §10 / Doc 00 §11.44 case.
fn duration_ms(ce: &ast::ConstExpr) -> Option<u32> {
    let expr = ce.syntax().children().next()?;
    let v = eval_i64(&expr)?;
    if v < 0 {
        return None;
    }
    u32::try_from(v).ok()
}

fn eval_i64(node: &SyntaxNode) -> Option<i64> {
    match node.kind() {
        SyntaxKind::EXPR_LITERAL => {
            let tok = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::IntLiteral)?;
            crate::util::parse_int_literal_i64(tok.text())
        }
        SyntaxKind::EXPR_UNARY => {
            let op = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::Minus | SyntaxKind::Plus))?;
            let inner = node.children().next()?;
            let v = eval_i64(&inner)?;
            match op.kind() {
                SyntaxKind::Minus => Some(-v),
                _ => Some(v),
            }
        }
        SyntaxKind::EXPR_PAREN => {
            let inner = node.children().next()?;
            eval_i64(&inner)
        }
        _ => None,
    }
}
