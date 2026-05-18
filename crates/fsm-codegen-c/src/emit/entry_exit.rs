//! Entry / exit sequencing — Doc 08 §6, §7 and Doc 00 §7.7 (`effective_lca`).
//!
//! For a transition `t` with `source`, `target`, and `kind`:
//!
//! ```text
//! effective_lca(t) =
//!     if t.kind == External and t.source == t.target: parent_of(t.source)
//!     else:                                           lca_inclusive(source, target)
//!
//! exit_set(t)  = path(t.source ← effective_lca)   minus the effective_lca itself
//! entry_set(t) = path(effective_lca → t.target)   minus the effective_lca itself
//! ```
//!
//! Codegen emits an inline sequence of `Motor_exit_X(m); ...; m->_state = T;
//! Motor_entry_T(m);` lines that walks the exit set leaf-first and the
//! entry set root-first.
//!
//! ## Transition-kind LCA semantics (Doc 00 §7.7 B-09 / Doc 08 §5.1, §6.1)
//!
//! `effective_lca` is the SHARED transition-LCA algorithm consumed by every
//! kind. Per the authoritative reconciliation (Doc 00 §7.7 B-09, mirrored
//! verbatim by the unforked `fsm_ir::effective_lca` the shipped
//! `fsm_simulator` uses) it is `lca_inclusive(source, target)` with **one**
//! exception: an `External` self-transition (`source == target`) lifts to
//! `parent(source)` so the leaf still exits + re-enters. There is **no**
//! `Local`/`Internal` LCA special-case — the B-09 table's
//! `Local (~>): LCA(S,S) = S` row is the *self* case, which falls straight
//! out of `lca_inclusive(S, S) = S` (S is its own ancestor); a *non-self*
//! local transition uses the genuine common ancestor exactly like every
//! other kind. The exit/entry walk then naturally yields the correct set
//! for BOTH a within-subtree local (`Outer ~> Inner`, Inner a descendant of
//! Outer: LCA = Outer ⇒ source not exited, only Outer→Inner entered) and a
//! sibling/cousin local (`Inner1 ~> Inner2`: LCA = the common composite ⇒
//! exit the source leaf-chain, enter the target leaf-chain, the common
//! composite NOT re-entered). This is leaf-symmetric and byte-identical in
//! observable behaviour to the shipped simulator's `effective_lca` /
//! `exit_set` / `entry_path` for every kind (the carriers differ only in
//! whether *region* ids appear in the chain — both stop at the LCA and emit
//! STATE ids only, so the externally-observable exit/entry STATE sequences
//! agree by construction).
//!
//! `Internal` keeps a no-exit / no-entry contract, but enforced where the
//! shipped simulator enforces it — `execute_one_transition`'s Internal
//! early-return, mirrored by the `Internal` short-circuits in `exit_path` /
//! `entry_path` below — NOT by collapsing the LCA (an Internal transition
//! never moves state, so its LCA value is unobservable anyway).
//!
//! ### FW110-FU-C judgment call — the sibling-local fixture vs `FSM-E0110`
//!
//! Doc 04 §8.3 states a local (`~>`) target MUST be a *proper descendant* of
//! the source (`FSM-E0110` otherwise). That validation is **unimplemented**
//! (a phantom code in Doc 10; no analyzer enforcement) and is an
//! analyzer-side concern explicitly out of this codegen wave's scope. The
//! shipped `fsm_simulator` (the unforked differential oracle) does NOT gate
//! on E0110 either — it lowers a sibling-targeted local via the genuine LCA
//! above. The defined behaviour the differential measures against is
//! therefore the genuine-LCA lowering; converging codegen onto it (rather
//! than rejecting the corpus fixture) is the correct, well-defined fix here.
//! Even if E0110 were enforced, the only legal local would be the
//! within-subtree case — for which this same algorithm is also correct.

use fsm_ir::{ParentResolver, TransitionKind, TransitionObject};

use crate::parent_table::ParentTable;
use crate::state_index::{StateIndex, ROOT_SENTINEL};

/// `ParentTable` is the `u8`-index carrier for the shared LCA algorithm.
///
/// AD-1 (2026-05-15): the walk is the shared [`fsm_ir`] generic; this impl
/// only supplies the per-step parent + the `ROOT_SENTINEL` terminator.
/// Unlike the analyzer/simulator `String` carriers, regions are *not* in
/// this chain — `parents[i]` is the enclosing composite/parallel state
/// index (or `ROOT_SENTINEL`). `ROOT_SENTINEL` is its own parent in the
/// table, so termination is via `is_root`, not via `parent` returning
/// `None`; the shared walk's repeat-guard would also stop a malformed
/// self-parent cycle. This reproduces the deleted bespoke chain walk
/// exactly (proven by this module's tests).
impl ParentResolver for ParentTable {
    type Id = u8;

    fn parent(&self, id: &u8) -> Option<u8> {
        self.parents.get(*id as usize).copied()
    }

    fn is_root(&self, id: &u8) -> bool {
        *id == ROOT_SENTINEL
    }
}

/// Compute the effective LCA index for a transition, per Doc 00 §7.7 B-09.
///
/// `effective_lca = lca_inclusive(source, target)`, with the SOLE kind
/// exception being an `External` self-transition lifting to `parent(source)`
/// (so the leaf still fires exit + re-entry — UML 2.5.1 §14.2.3.9.6). This
/// is the shared transition-LCA algorithm for EVERY kind and is identical to
/// the authoritative Doc 00 §7.7 B-09 encoding and the unforked
/// [`fsm_ir::effective_lca`] the shipped `fsm_simulator` consumes (the
/// `String` simulator carrier and this `u8` codegen carrier differ only in
/// whether region ids appear in the parent chain — the LCA *index* and the
/// resulting exit/entry STATE sequences are observably identical).
///
/// FW110-FU-C: the prior `Local | Internal => source` collapse was WRONG for
/// a non-self local transition whose target is a sibling/cousin (it re-ran
/// the common composite's entry action and skipped the source leaf's exit
/// action). `Local` now uses the genuine common ancestor exactly like every
/// other kind; `Internal`'s no-exit/no-entry contract is enforced (as the
/// simulator does) by the `Internal` early-return in `exit_path`/`entry_path`
/// below, NOT by collapsing the unobservable LCA of a state-preserving
/// transition. See this module's doc comment for the full re-derivation and
/// the within-subtree-vs-sibling discriminator.
pub fn effective_lca(t: &TransitionObject, index: &StateIndex, parents: &ParentTable) -> u8 {
    let source = index.must_lookup(&t.source);
    let target = index.must_lookup(&t.target);
    let base = lca_inclusive(source, target, parents);

    match t.kind {
        TransitionKind::External if source == target => parents.parents[source as usize],
        _ => base,
    }
}

/// Standard inclusive LCA — a state is its own ancestor. Doc 08 §5.1.
///
/// Thin adapter over the shared [`fsm_ir::lca_inclusive`] generic (AD-1).
/// The `u8` `ParentTable` carrier above makes the shared walk produce a
/// chain `[a, …, ROOT_SENTINEL]` identical to the deleted bespoke loop
/// (which pushed `ROOT_SENTINEL` exactly once before stopping), so the
/// returned index is byte-identical for every input.
pub fn lca_inclusive(a: u8, b: u8, parents: &ParentTable) -> u8 {
    fsm_ir::lca_inclusive(&a, &b, parents)
}

/// Compute the ordered exit set (innermost first) for a transition.
pub fn exit_path(t: &TransitionObject, index: &StateIndex, parents: &ParentTable) -> Vec<u8> {
    let source = index.must_lookup(&t.source);
    let lca = effective_lca(t, index, parents);
    let mut path = Vec::new();

    // For an internal transition there is no exit at all — mirrors the
    // shipped `fsm_simulator::execute_one_transition` Internal early-return.
    if matches!(t.kind, TransitionKind::Internal) {
        return path;
    }

    // Every other kind (External / Local / Completion / pseudostate): walk
    // the source upward until we hit (but do NOT include) the effective LCA.
    // For a within-subtree local (`Outer ~> Inner`) the effective LCA *is*
    // the source, so this loop runs zero times and the source is correctly
    // NOT exited (the descendant-only invariant — B-09 / Doc 08 §6.1 — falls
    // out of the genuine LCA, no Local special-case needed). For a
    // sibling/cousin local the LCA is the common composite, so the source
    // leaf-chain up to (excluding) that composite IS exited — byte-identical
    // to the shipped simulator's `exit_set(source, effective_lca)`.
    let mut cur = source;
    while cur != lca && cur != ROOT_SENTINEL {
        path.push(cur);
        cur = parents.parents[cur as usize];
    }
    path
}

/// Compute the ordered entry set (root-first) for a transition.
pub fn entry_path(t: &TransitionObject, index: &StateIndex, parents: &ParentTable) -> Vec<u8> {
    let target = index.must_lookup(&t.target);
    let lca = effective_lca(t, index, parents);

    // Internal transitions never enter anything — mirrors the shipped
    // `fsm_simulator::execute_one_transition` Internal early-return.
    if matches!(t.kind, TransitionKind::Internal) {
        return Vec::new();
    }
    // Every other kind: walk the target upward to (but NOT including) the
    // effective LCA, then reverse to root-first. For a within-subtree local
    // (`Outer ~> Inner`) the LCA is `Outer`, so this yields exactly
    // `[Inner]` — `Outer` is NOT re-entered. For a sibling/cousin local the
    // LCA is the common composite, so only the target leaf-chain is entered
    // and the common composite is NOT re-entered — byte-identical to the
    // shipped simulator's `entry_path(effective_lca, target)`.
    let mut chain = Vec::new();
    let mut cur = target;
    while cur != lca && cur != ROOT_SENTINEL {
        chain.push(cur);
        cur = parents.parents[cur as usize];
    }
    chain.reverse();
    chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        CompositeState, ContextSchema, InitialPseudo, MachineObject, QueueConfig, RegionObject,
        SimpleState, StateNode, TransitionKind, TransitionObject, Trigger,
    };

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    #[allow(deprecated)]
    fn t(source: &str, target: &str, kind: TransitionKind) -> TransitionObject {
        TransitionObject {
            id: format!("t-{source}-{target}"),
            stable_id: "T".into(),
            source: source.into(),
            target: target.into(),
            trigger: Some(Trigger::Event {
                event_id: "e".into(),
                payload_binding: None,
            }),
            guard: None,
            actions: vec![],
            priority: 100,
            kind,
            internal: false,
            hint: None,
            loc: loc(),
        }
    }

    fn nested_index() -> (
        crate::state_index::StateIndex,
        crate::parent_table::ParentTable,
    ) {
        // root → Op (composite) → Running (simple)
        let m = MachineObject {
            id: "m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-init".into(),
                        target: "s-op".into(),
                        loc: loc(),
                    }),
                    StateNode::Composite(CompositeState {
                        id: "s-op".into(),
                        stable_id: "M:Op".into(),
                        name: "Op".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        regions: vec![RegionObject {
                            id: "r-op".into(),
                            stable_id: None,
                            name: "Op".into(),
                            initial: "ps-op-init".into(),
                            states: vec![
                                StateNode::Initial(InitialPseudo {
                                    id: "ps-op-init".into(),
                                    target: "s-running".into(),
                                    loc: loc(),
                                }),
                                StateNode::Simple(SimpleState {
                                    id: "s-running".into(),
                                    stable_id: "M:Running".into(),
                                    name: "Running".into(),
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
                        }],
                        history: None,
                        loc: loc(),
                    }),
                    StateNode::Simple(SimpleState {
                        id: "s-faulted".into(),
                        stable_id: "M:Faulted".into(),
                        name: "Faulted".into(),
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
        };
        let index = crate::state_index::build_state_index(&m).expect("build_state_index");
        let parents = crate::parent_table::build_parent_table(&index);
        (index, parents)
    }

    #[test]
    fn external_self_transition_lca_is_parent() {
        // B-09: external `s -> s` must exit and re-enter; LCA = parent.
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-running", TransitionKind::External);
        let lca = effective_lca(&tt, &index, &parents);
        let parent = index.lookup("s-op").unwrap();
        assert_eq!(lca, parent);
        // Exit path is [Running]; entry path is [Running].
        assert_eq!(
            exit_path(&tt, &index, &parents),
            vec![index.lookup("s-running").unwrap()]
        );
        assert_eq!(
            entry_path(&tt, &index, &parents),
            vec![index.lookup("s-running").unwrap()]
        );
    }

    #[test]
    fn local_self_transition_has_empty_exit_set() {
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-running", TransitionKind::Local);
        assert_eq!(exit_path(&tt, &index, &parents).len(), 0);
        assert_eq!(entry_path(&tt, &index, &parents).len(), 0);
    }

    #[test]
    fn internal_transition_has_no_entry_or_exit() {
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-running", TransitionKind::Internal);
        assert!(exit_path(&tt, &index, &parents).is_empty());
        assert!(entry_path(&tt, &index, &parents).is_empty());
    }

    /// AD-1 guard: the shared `u8` LCA must agree with a from-scratch
    /// transcription of the deleted bespoke chain walk on the canonical
    /// pairs — sibling, nested, external-self, local-self. (No parallel
    /// here: codegen's `u8` carrier collapses regions so cross-parallel
    /// reduces to the same parent-index walk; the `fsm-ir`
    /// `lca_parent_resolver` suite covers the parallel `u8` case.)
    fn oracle_lca(a: u8, b: u8, parents: &crate::parent_table::ParentTable) -> u8 {
        let mut chain = Vec::with_capacity(8);
        let mut cur = a;
        loop {
            chain.push(cur);
            if cur == ROOT_SENTINEL {
                break;
            }
            cur = parents.parents[cur as usize];
            if cur == ROOT_SENTINEL {
                chain.push(ROOT_SENTINEL);
                break;
            }
        }
        let mut cur = b;
        loop {
            if chain.contains(&cur) {
                return cur;
            }
            if cur == ROOT_SENTINEL {
                return ROOT_SENTINEL;
            }
            cur = parents.parents[cur as usize];
        }
    }

    #[test]
    fn shared_u8_lca_matches_handrolled_oracle() {
        let (index, parents) = nested_index();
        let running = index.lookup("s-running").unwrap();
        let op = index.lookup("s-op").unwrap();
        let faulted = index.lookup("s-faulted").unwrap();
        for (a, b) in [
            (running, faulted), // nested across composite → ROOT
            (faulted, running),
            (running, op),       // ancestor → op
            (running, running),  // self → running
            (op, ROOT_SENTINEL), // → ROOT
        ] {
            assert_eq!(
                lca_inclusive(a, b, &parents),
                oracle_lca(a, b, &parents),
                "u8 lca mismatch for ({a},{b})"
            );
        }
        // Concrete: Running/Faulted across the Op composite → ROOT.
        assert_eq!(lca_inclusive(running, faulted, &parents), ROOT_SENTINEL);
        assert_eq!(lca_inclusive(running, op, &parents), op);
    }

    #[test]
    fn nested_transition_exits_through_composite() {
        // Running -> Faulted should exit Running AND Op (since Faulted is
        // outside Op), then enter Faulted.
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-faulted", TransitionKind::External);
        let exits = exit_path(&tt, &index, &parents);
        let running = index.lookup("s-running").unwrap();
        let op = index.lookup("s-op").unwrap();
        let faulted = index.lookup("s-faulted").unwrap();
        assert_eq!(exits, vec![running, op]);
        assert_eq!(entry_path(&tt, &index, &parents), vec![faulted]);
    }

    /// Topology mirroring the `stress-self-transitions` corpus fixture:
    /// `root → Box(composite) → {Inner1, Inner2}` (two SIBLING leaves inside
    /// one composite). The FW110-FU-C regression vehicle.
    fn box_two_inner_index() -> (
        crate::state_index::StateIndex,
        crate::parent_table::ParentTable,
    ) {
        let inner1 = StateNode::Simple(SimpleState {
            id: "s-inner1".into(),
            stable_id: "M:Inner1".into(),
            name: "Inner1".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            loc: loc(),
        });
        let inner2 = StateNode::Simple(SimpleState {
            id: "s-inner2".into(),
            stable_id: "M:Inner2".into(),
            name: "Inner2".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            loc: loc(),
        });
        let m = MachineObject {
            id: "m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-init".into(),
                        target: "s-box".into(),
                        loc: loc(),
                    }),
                    StateNode::Composite(CompositeState {
                        id: "s-box".into(),
                        stable_id: "M:Box".into(),
                        name: "Box".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        regions: vec![RegionObject {
                            id: "r-box".into(),
                            stable_id: None,
                            name: "Box".into(),
                            initial: "ps-box-init".into(),
                            states: vec![
                                StateNode::Initial(InitialPseudo {
                                    id: "ps-box-init".into(),
                                    target: "s-inner1".into(),
                                    loc: loc(),
                                }),
                                inner1,
                                inner2,
                            ],
                            priority: 0,
                            loc: loc(),
                        }],
                        history: None,
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
        };
        let index = crate::state_index::build_state_index(&m).expect("build_state_index");
        let parents = crate::parent_table::build_parent_table(&index);
        (index, parents)
    }

    /// FW110-FU-C — the core regression. A SIBLING-targeted local transition
    /// (`Inner1 ~> Inner2`, both inside composite `Box`) MUST exit the
    /// source leaf `Inner1`, enter the target leaf `Inner2`, and MUST NOT
    /// re-enter (or exit) the common composite `Box`. The prior
    /// `Local => source` LCA collapse produced LCA=Inner1, an empty exit
    /// set (skipping `exit_Inner1`) and an entry path that walked PAST `Box`
    /// (wrongly re-running `entry_Box`). Now LCA = the genuine common
    /// ancestor (`Box`'s index — regions are collapsed in the codegen `u8`
    /// carrier) so the sets are leaf-symmetric and `Box` is excluded from
    /// both — byte-identical to the shipped simulator.
    #[test]
    fn sibling_local_exits_source_leaf_enters_target_leaf_not_composite() {
        let (index, parents) = box_two_inner_index();
        let tt = t("s-inner1", "s-inner2", TransitionKind::Local);
        let box_idx = index.lookup("s-box").unwrap();
        let inner1 = index.lookup("s-inner1").unwrap();
        let inner2 = index.lookup("s-inner2").unwrap();

        // Genuine common ancestor — NOT the source (the FW110-FU-C fix).
        assert_eq!(
            effective_lca(&tt, &index, &parents),
            box_idx,
            "sibling-local effective_lca must be the common composite, not the source"
        );
        // Source leaf IS exited (the prior bug skipped this entirely).
        assert_eq!(
            exit_path(&tt, &index, &parents),
            vec![inner1],
            "sibling-local must exit exactly the source leaf — Box NOT exited"
        );
        // Only the target leaf is entered — Box is NOT re-entered (the prior
        // bug walked past Box and wrongly included it).
        assert_eq!(
            entry_path(&tt, &index, &parents),
            vec![inner2],
            "sibling-local must enter exactly the target leaf — Box NOT re-entered"
        );
        assert!(
            !exit_path(&tt, &index, &parents).contains(&box_idx)
                && !entry_path(&tt, &index, &parents).contains(&box_idx),
            "the common composite must never appear in either set for a sibling-local"
        );
    }

    /// FW110-FU-C — the within-subtree local must STILL not exit the source
    /// (the descendant-only invariant, B-09 / Doc 08 §6.1). `Box ~> Inner2`
    /// (Inner2 a descendant of Box): LCA = Box ⇒ empty exit (Box not
    /// exited), entry = `[Inner2]` only (Box not re-entered). Proves the fix
    /// did not regress the case the old collapse handled.
    #[test]
    fn within_subtree_local_does_not_exit_source() {
        let (index, parents) = box_two_inner_index();
        let tt = t("s-box", "s-inner2", TransitionKind::Local);
        let box_idx = index.lookup("s-box").unwrap();
        let inner2 = index.lookup("s-inner2").unwrap();
        assert_eq!(effective_lca(&tt, &index, &parents), box_idx);
        assert_eq!(
            exit_path(&tt, &index, &parents),
            Vec::<u8>::new(),
            "within-subtree local must NOT exit the source composite"
        );
        assert_eq!(
            entry_path(&tt, &index, &parents),
            vec![inner2],
            "within-subtree local enters only the path below the source, source NOT re-entered"
        );
    }

    /// FW110-FU-C guard — `Internal` keeps the no-exit / no-entry contract
    /// even though its LCA is no longer collapsed (it is now the genuine
    /// `lca_inclusive`, which is unobservable for a state-preserving
    /// transition — the contract is enforced by the `Internal`
    /// short-circuits, exactly as the shipped simulator does).
    #[test]
    fn internal_non_self_still_has_no_entry_or_exit() {
        let (index, parents) = box_two_inner_index();
        let tt = t("s-inner1", "s-inner2", TransitionKind::Internal);
        assert!(exit_path(&tt, &index, &parents).is_empty());
        assert!(entry_path(&tt, &index, &parents).is_empty());
    }
}
