//! W2 behavioural-acceptance gate (Doc 30 §5.4 / §4.2-W2).
//!
//! The W2 wave extends the W1 explorer to composite / parallel / history /
//! **timer** / **submachine** machines. Per Doc 30 §5.4 + the P0-1 lesson,
//! **symbol-presence / "it compiles / handles more shapes" is NOT
//! acceptance** — every verdict here is **Interpreter-oracle
//! cross-checked**: a direct `fsm_simulator::Interpreter` is driven by hand
//! to establish ground truth, and the verifier is asserted to AGREE
//! (reachable-set byte-equality / the deadlock witness replayed through a
//! fresh interpreter lands in the reported config). A forked
//! transition-selection / timer-firing / completion semantics would make
//! the verifier disagree with the real interpreter and fail these tests —
//! that is the W2 keystone proof extended to hierarchy/parallel/timer/
//! submachine.
//!
//! Coverage map (the brief's W2 acceptance (3)–(8)):
//!   (3) `composite_parallel_*`        — composite/parallel verdict, oracle-checked
//!   (4) `timer_wait_*` / `timer_*deadlock` — timer NOT-deadlock + real timer-deadlock+witness
//!   (5) `submachine_*`                — sub-instance configs explored, correct verdict
//!   (8) `*_reachable_set_equals_interpreter_bfs` — keystone oracle-agreement on hierarchy

use std::collections::{BTreeSet, VecDeque};

use fsm_simulator::{InitOptions, Interpreter, InterpreterSnapshot};
use fsm_verify::{verify, Verdict, VerifyOptions};

const COMPOSITE_PARALLEL_SOUND: &str = include_str!("fixtures/composite_parallel_sound.fsm");
const PARALLEL_JOIN_DEADLOCK: &str = include_str!("fixtures/parallel_join_deadlock.fsm");
const TIMER_WAIT_THEN_DONE: &str = include_str!("fixtures/timer_wait_then_done.fsm");
const TIMER_HEARTBEAT_DEADLOCK: &str = include_str!("fixtures/timer_heartbeat_deadlock.fsm");
const SUBMACHINE_COMPLETES: &str = include_str!("fixtures/submachine_completes.fsm");

fn ir_of(src: &str) -> fsm_ir::Ir {
    let pr = fsm_parser::parse(src);
    let res = fsm_analyzer::analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == fsm_diagnostics::Severity::Error),
        "fixture must analyze clean; got: {:?}",
        res.diagnostics
    );
    res.ir.expect("analyzer produced IR")
}

fn fresh(ir: &fsm_ir::Ir) -> Interpreter {
    let mut i = Interpreter::new(ir).unwrap();
    i.init(InitOptions {
        machine_name: ir.machines[0].name.clone(),
        initial_context: None,
        virtual_clock_start_ms: 0,
    })
    .unwrap();
    i
}

/// An **independent** interpreter-driven BFS over the reachable
/// configuration space, exploring the SAME edge families the verifier
/// does (every declared event + the timer-fire `advance_clock` edge),
/// keyed by the public `InterpreterSnapshot`'s active states + relative
/// timer/sub-instance state. This is a *separate* implementation from
/// `fsm-verify`'s engine (it lives in the test, drives the interpreter
/// directly), so byte-equality of its reachable set with the verifier's
/// is the keystone proof: the verifier did not fork the semantics.
fn interpreter_reachable_set(ir: &fsm_ir::Ir) -> BTreeSet<String> {
    fn key(s: &InterpreterSnapshot) -> String {
        // Active states + relative timer phase + recursive sub leaves —
        // a clock-origin-normalised key that ALSO zeroes the (book-keeping,
        // non-configuration) `next_trace_id` — recursively for nested
        // sub-instances. This independently MIRRORS the engine's
        // `digest::canonical_bytes` normalisation (clock origin + trace-id
        // excluded, recursively): two behaviourally-identical configs
        // reached via different-length paths (incl. submachine RECONNECT
        // teardown/re-create) must share a key, or neither this hand-BFS
        // NOR the engine converges. Deterministic via BTreeMap/Vec order.
        fn sub_key(sub: &fsm_simulator::SubmachineSnapshot) -> String {
            let origin = sub.virtual_clock_ms;
            let mut t: Vec<String> = sub
                .timers
                .iter()
                .map(|tm| {
                    format!(
                        "{}:{}",
                        tm.source_state,
                        tm.expiry_ms.saturating_sub(origin)
                    )
                })
                .collect();
            t.sort();
            let nested: Vec<String> = sub
                .submachines
                .iter()
                .map(|(k, v)| format!("{k}=>{}", sub_key(v)))
                .collect();
            // `virtual_clock_ms` -> 0, `next_trace_id` excluded entirely;
            // `history`/`defer_set`/`context` kept (observable).
            format!(
                "[{:?};h{:?};d{:?};c{:?};t{};s{{{}}}]",
                sub.active_states,
                sub.history,
                sub.defer_set,
                sub.context,
                t.join(","),
                nested.join(",")
            )
        }
        let origin = s.virtual_clock_ms;
        let mut t: Vec<String> = s
            .timers
            .iter()
            .map(|tm| {
                format!(
                    "{}:{}",
                    tm.source_state,
                    tm.expiry_ms.saturating_sub(origin)
                )
            })
            .collect();
        t.sort();
        let subs: Vec<String> = s
            .submachines
            .iter()
            .map(|(k, v)| format!("{k}=>{}", sub_key(v)))
            .collect();
        format!(
            "{:?}|h{:?}|d{:?}|c{:?}|{}|s{{{}}}",
            s.active_states,
            s.history,
            s.defer_set,
            s.context,
            t.join(","),
            subs.join(",")
        )
    }

    let mut interp = fresh(ir);
    let events: Vec<String> = ir.machines[0]
        .events
        .iter()
        .map(|e| e.name.clone())
        .collect();

    let start = interp.snapshot().unwrap();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    visited.insert(key(&start));
    let mut reachable: BTreeSet<String> = interp.current_states().into_iter().collect();
    let mut q: VecDeque<InterpreterSnapshot> = VecDeque::new();
    q.push_back(start);

    let mut guard = 0;
    while let Some(snap) = q.pop_front() {
        guard += 1;
        assert!(
            guard < 100_000,
            "hand-BFS did not converge — fixture too big"
        );

        // Declared-event edges.
        for ev in &events {
            interp.restore(snap.clone()).unwrap();
            interp.dispatch(ev).unwrap();
            let succ = interp.snapshot().unwrap();
            for s in interp.current_states() {
                reachable.insert(s);
            }
            if visited.insert(key(&succ)) {
                q.push_back(succ);
            }
        }
        // Timer-fire edge: advance to the soonest armed expiry (recursing
        // into nested subs for the global min — same rule the engine uses).
        let min_expiry = {
            fn sub_min(s: &fsm_simulator::SubmachineSnapshot) -> Option<u64> {
                let h = s.timers.iter().map(|t| t.expiry_ms).min();
                let n = s.submachines.values().filter_map(sub_min).min();
                match (h, n) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                }
            }
            let h = snap.timers.iter().map(|t| t.expiry_ms).min();
            let n = snap.submachines.values().filter_map(sub_min).min();
            match (h, n) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            }
        };
        if let Some(exp) = min_expiry {
            interp.restore(snap.clone()).unwrap();
            let now = interp.virtual_clock_ms();
            interp.advance_clock(exp.saturating_sub(now)).unwrap();
            let succ = interp.snapshot().unwrap();
            for s in interp.current_states() {
                reachable.insert(s);
            }
            if visited.insert(key(&succ)) {
                q.push_back(succ);
            }
        }
    }
    reachable
}

// ───────────────────────────────────────────────────────────────────────
// (3) COMPOSITE + PARALLEL — correct ProvenNoDeadlock + reachability,
//     cross-checked against an independent interpreter-driven hand BFS
//     (the keystone proof on a hierarchical/parallel fixture — brief (8)).
// ───────────────────────────────────────────────────────────────────────
#[test]
fn composite_parallel_sound_is_proven_and_matches_interpreter_bfs() {
    let ir = ir_of(COMPOSITE_PARALLEL_SOUND);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs on a parallel machine");
    assert_eq!(
        out.verdict,
        Verdict::ProvenNoDeadlock,
        "the parallel machine has progress from every config ⇒ no deadlock"
    );

    // KEYSTONE (brief (8)): the verifier's reachable set must EQUAL an
    // independent interpreter-driven BFS. A forked LCA/region/completion
    // semantics would diverge here.
    let oracle = interpreter_reachable_set(&ir);
    assert_eq!(
        out.reachability.reachable, oracle,
        "verifier's reachable set must byte-equal an independent \
         interpreter-driven BFS over the parallel machine (no forked semantics)"
    );
    // Sanity: it actually explored the parallel interior (both regions'
    // leaves are present), not just the top level.
    assert!(
        oracle.iter().any(|s| s.contains("Motor")) && oracle.iter().any(|s| s.contains("Pump")),
        "both parallel regions must have been explored; got {oracle:?}"
    );
}

// ───────────────────────────────────────────────────────────────────────
// (3) PARALLEL-JOIN DEADLOCK — a region that can never reach its final
//     wedges the all-regions-done join. Detected as Deadlock; the witness
//     replayed through a FRESH interpreter lands in the reported config.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn parallel_join_deadlock_is_detected_with_oracle_checked_witness() {
    let ir = ir_of(PARALLEL_JOIN_DEADLOCK);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs");
    let (report, witness) = match &out.verdict {
        Verdict::Deadlock { report, witness } => (report, witness),
        other => panic!("a wedged parallel join is a deadlock; got {other:?}"),
    };

    // The deadlocked config must be non-final (Left at LFinal, Right wedged
    // at RB, Working not completed). It must NOT contain `Finished`.
    assert!(
        !report.config.iter().any(|s| s.contains("Finished")),
        "the join can never fire ⇒ `Finished` is unreachable; deadlock \
         config = {:?}",
        report.config
    );

    // Oracle cross-check: replay the witness (declared events only — the
    // join deadlock involves no timer) through a FRESH interpreter and
    // assert it lands in EXACTLY the reported deadlocked configuration,
    // and that NO declared event progresses from there.
    let mut interp = fresh(&ir);
    for ev in witness {
        assert!(
            !ev.starts_with("<timer-fire"),
            "this deadlock is event-only; witness must not contain a timer-fire"
        );
        interp.dispatch(ev).unwrap();
    }
    let landed = interp.current_states();
    let mut landed_sorted = landed.clone();
    landed_sorted.sort();
    let mut config_sorted = report.config.clone();
    config_sorted.sort();
    assert_eq!(
        landed_sorted, config_sorted,
        "replaying the witness through a fresh interpreter must land in the \
         reported deadlocked configuration (oracle-checked witness)"
    );
    // And from there the interpreter agrees nothing progresses.
    let snap = interp.snapshot().unwrap();
    for ev in ir.machines[0].events.iter().map(|e| &e.name) {
        interp.restore(snap.clone()).unwrap();
        interp.dispatch(ev).unwrap();
        assert_eq!(
            interp.current_states(),
            landed,
            "oracle: no declared event ({ev}) progresses from the deadlock — \
             confirms the verifier's verdict tracks the real interpreter"
        );
    }
}

// ───────────────────────────────────────────────────────────────────────
// (4a) TIMER WAIT THAT FIRES — must NOT be flagged a deadlock (the Doc 08
//      §13 / §4.1 foot-gun). The timer-fire edge (driven via the real
//      `Interpreter::advance_clock`) progresses the wait state.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn timer_wait_state_is_not_a_false_deadlock() {
    let ir = ir_of(TIMER_WAIT_THEN_DONE);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs on a timer machine");
    assert_eq!(
        out.verdict,
        Verdict::ProvenNoDeadlock,
        "a state waiting on a timer that FIRES is NOT a deadlock — the \
         timer-fire edge progresses it (the Doc 30 §4.1 foot-gun guard)"
    );

    // Oracle cross-check: a direct interpreter `advance_clock` past the
    // 2000 ms timer actually leaves `Warming` (so the verifier's
    // not-a-deadlock verdict tracks the real timer semantics, not a
    // re-implemented clock).
    let mut interp = fresh(&ir);
    let warming = interp.current_states();
    assert!(
        warming.iter().any(|s| s.ends_with("-Warming")),
        "init lands in Warming; got {warming:?}"
    );
    interp.advance_clock(2001).unwrap();
    let after = interp.current_states();
    assert!(
        after != warming && after.iter().any(|s| s.ends_with("-Ready")),
        "oracle: the real interpreter's timer-fire moves Warming -> Ready; \
         got {after:?}"
    );
    // The whole machine reaches `Done` (final) — the reachable set proves
    // the timer chain was followed end-to-end.
    assert!(
        out.reachability
            .reachable
            .iter()
            .any(|s| s.ends_with("-Done")),
        "the timer chain must reach the final Done state; reachable = {:?}",
        out.reachability.reachable
    );
}

// ───────────────────────────────────────────────────────────────────────
// (4b) GENUINE TIMER-DEADLOCK — an `every_internal` heartbeat whose
//      configuration never changes. Detected as Deadlock WITH a witness;
//      the witness replayed through a fresh interpreter lands in the
//      wedge, and the interpreter confirms no edge (event OR timer-fire)
//      progresses. This is the brief's "real timer-deadlock IS detected".
// ───────────────────────────────────────────────────────────────────────
#[test]
fn genuine_timer_deadlock_is_detected_with_oracle_checked_witness() {
    let ir = ir_of(TIMER_HEARTBEAT_DEADLOCK);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs");
    let (report, witness) = match &out.verdict {
        Verdict::Deadlock { report, witness } => (report, witness),
        other => panic!(
            "an every_internal-only state is a genuine timer-deadlock \
             (config never changes); got {other:?}"
        ),
    };
    assert!(
        report.config.iter().any(|s| s.ends_with("-Wedged")),
        "the deadlocked config is {{Wedged}}; got {:?}",
        report.config
    );
    // Witness is [START] (the only event reaching the wedge).
    assert_eq!(
        witness,
        &vec!["START".to_string()],
        "witness must be the exact event sequence reaching the wedge"
    );

    // Oracle cross-check: replay [START] through a fresh interpreter →
    // lands in Wedged; then NO declared event AND NO timer-fire produces a
    // *different* configuration (the heartbeat ticks forever but the
    // config is invariant — that IS the deadlock).
    let mut interp = fresh(&ir);
    interp.dispatch("START").unwrap();
    let wedged = interp.current_states();
    let mut a = wedged.clone();
    a.sort();
    let mut b = report.config.clone();
    b.sort();
    assert_eq!(a, b, "witness replay lands in the reported deadlock config");

    let snap = interp.snapshot().unwrap();
    for ev in ir.machines[0].events.iter().map(|e| &e.name) {
        interp.restore(snap.clone()).unwrap();
        interp.dispatch(ev).unwrap();
        assert_eq!(
            interp.current_states(),
            wedged,
            "oracle: event {ev} does not progress the wedge"
        );
    }
    // Timer-fire also does not change the active configuration.
    interp.restore(snap.clone()).unwrap();
    let now = interp.virtual_clock_ms();
    let exp = interp
        .snapshot()
        .unwrap()
        .timers
        .iter()
        .map(|t| t.expiry_ms)
        .min()
        .expect("Wedged has the every-1000ms timer armed");
    interp.advance_clock(exp.saturating_sub(now)).unwrap();
    assert_eq!(
        interp.current_states(),
        wedged,
        "oracle: the every_internal timer fires (runs tick()) but the \
         CONFIGURATION is invariant — confirms the genuine deadlock the \
         verifier detected (and that it is detected, not Inconclusive)"
    );
}

// ───────────────────────────────────────────────────────────────────────
// (5) SUBMACHINE — sub-instance configs are explored, completion is taken
//     by the interpreter (no forked `done`), verdict is correct, and the
//     reachable set EQUALS an independent interpreter-driven BFS (the
//     keystone proof, submachine axis).
// ───────────────────────────────────────────────────────────────────────
#[test]
fn submachine_machine_is_proven_and_subinstances_explored() {
    let ir = ir_of(SUBMACHINE_COMPLETES);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs on a submachine model");
    assert_eq!(
        out.verdict,
        Verdict::ProvenNoDeadlock,
        "every config has progress (delegated events advance the sub; sub \
         completion drives the parent done) ⇒ no deadlock"
    );

    // PROOF the explorer descended into the sub-instance (not "leaves in
    // the reachable set" — `reachable` tracks the *parent machine's*
    // declared states via `current_states()`, which by design does not
    // surface a sub-template's leaves; that is correct W1-inherited
    // reachability semantics). The sound proof is *behavioural*:
    //
    // (a) `s-Device-Online` IS reachable. `Online` is reachable ONLY via
    //     `Connecting`'s `done -> Online` completion edge, which fires ONLY
    //     when the embedded `Connection` sub-instance runs all the way to
    //     its `final` (Idle -> Handshake -> Established -> Done). So
    //     reaching `Online` is itself proof the explorer drove the
    //     sub-instance through every nested leaf via delegated events +
    //     observed the interpreter's completion — impossible unless the
    //     W2-P0 lossless snapshot distinguished the nested configs (else
    //     the digest would have conflated Idle/Handshake/… and pruned).
    let r = &out.reachability.reachable;
    assert!(
        r.iter().any(|s| s.ends_with("-Online")),
        "Online must be reachable — it is gated on the sub-instance running \
         to its final + the interpreter's `done -> Online` completion, so \
         this proves the explorer traversed the nested sub-instance; \
         reachable = {r:?}"
    );

    // (b) The visited-config count reflects the SUB-INCLUSIVE space: a
    //     flat 2-state parent would be ~2 configs; the sub-instance's
    //     Idle/Handshake/Established/Done distinct nested configs multiply
    //     it. (> a small constant proves distinct sub-instance configs were
    //     visited, i.e. the digest did NOT conflate them — the P0 point.)
    assert!(
        out.stats.configs_visited >= 4,
        "the sub-inclusive reachable space must have >= 4 distinct configs \
         (the nested Idle/Handshake/Established/Done are distinguished — the \
         W2-P0 digest-soundness); got {}",
        out.stats.configs_visited
    );

    // (c) KEYSTONE (brief (8), submachine axis): the verifier's reachable
    //     set EQUALS an independent interpreter-driven BFS that itself
    //     drives the real interpreter (delegated events + completion). A
    //     forked submachine-completion / delegation semantics in fsm-verify
    //     would make these diverge.
    let oracle = interpreter_reachable_set(&ir);
    assert_eq!(
        out.reachability.reachable, oracle,
        "verifier's reachable set must byte-equal an independent \
         interpreter-driven BFS over the submachine model (no forked \
         submachine semantics — the keystone, submachine axis)"
    );
}
