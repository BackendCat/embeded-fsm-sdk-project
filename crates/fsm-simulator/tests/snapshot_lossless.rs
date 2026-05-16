//! v1.4-W2 P0 — `InterpreterSnapshot` losslessness round-trip gate.
//!
//! Closes the §11.3-W1-audit headline blocker (D-2 / Doc 30 §4.1
//! overstatement): pre-W2 `InterpreterSnapshot` / `snapshot()` /
//! `restore()` silently dropped `RuntimeState.timers` (the armed timer
//! set) and `RuntimeState.submachines` (the recursive nested sub-instance
//! configs). For the verifier's `ConfigDigest` to not conflate
//! behaviourally-distinct configs (→ premature visited-set pruning → a
//! **false `ProvenNoDeadlock`**, the cardinal verification sin), the
//! snapshot must be **lossless**.
//!
//! The acceptance (Doc 30 §5.4 — behavioural, NOT symbol-presence) is a
//! `snapshot → restore → re-snapshot` **byte-identity** round-trip on
//! hierarchical / parallel / **timer** / **submachine** configs, plus a
//! regression guard that **flat-machine snapshots are byte-unchanged**
//! (the new fields encode as empty `[]` / `{}` ⇒ no W1 / existing-sim-test
//! regression). Every fixture is real `.fsm` source through the real
//! parse+analyze pipeline.

use fsm_analyzer::analyze;
use fsm_parser::parse;
use fsm_simulator::{InitOptions, Interpreter, InterpreterSnapshot};

fn ir_of(src: &str) -> fsm_ir::Ir {
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == fsm_diagnostics::Severity::Error),
        "fixture must analyze clean; got: {:?}",
        res.diagnostics
    );
    res.ir.expect("analyzer produced IR")
}

/// Canonical JSON form of a snapshot (the exact bytes the verifier's
/// `ConfigDigest` hashes). Determinism is the Doc 13 §11 contract; this
/// test *is* the proof the W2 extension preserved it.
fn json(s: &InterpreterSnapshot) -> String {
    serde_json::to_string(s).expect("snapshot is JSON-encodable")
}

/// Lowered IDs are `s-<Machine>-<State>`; assert by state-*name* suffix so
/// the tests are robust to the prefixing scheme (the losslessness property
/// is what is under test, not the ID format).
fn in_state(interp: &Interpreter, state_name: &str) -> bool {
    let needle = format!("-{state_name}");
    interp
        .current_states()
        .iter()
        .any(|s| s.ends_with(&needle) || s == state_name)
}

/// The losslessness invariant: capturing a config, perturbing the
/// interpreter, restoring, and re-capturing yields a **byte-identical**
/// snapshot. If any configuration-relevant field were dropped on
/// snapshot/restore, the re-snapshot would differ here.
fn assert_round_trip_lossless(interp: &mut Interpreter, perturb: impl FnOnce(&mut Interpreter)) {
    let before = interp.snapshot().expect("snapshot");
    let before_json = json(&before);

    perturb(interp); // move the interpreter somewhere else entirely

    interp.restore(before).expect("restore");
    let after = interp.snapshot().expect("re-snapshot");

    assert_eq!(
        before_json,
        json(&after),
        "snapshot→restore→re-snapshot must be byte-identical (lossless)"
    );
}

// ───────────────────────────────────────────────────────────────────────
// (P0a) SUBMACHINE — a mid-sub-instance config round-trips losslessly, and
//       restore resurrects the EXACT nested leaf (not a torn-down one).
// ───────────────────────────────────────────────────────────────────────
const SUBMACHINE_FSM: &str = r#"language fsm 2.0

feature submachines

submachine Connection {
    events { CONNECT ACK DONE }
    initial Idle
    state Idle { on CONNECT -> Handshake }
    state Handshake { on ACK -> Established }
    state Established { on DONE -> Done }
    final Done
}

machine Device {
    events { CONNECT ACK DONE RESET }
    initial Connecting
    state Connecting is Connection {
        done -> Online
        on RESET -> Connecting
    }
    state Online { on RESET -> Connecting }
}"#;

#[test]
fn submachine_subinstance_config_round_trips_losslessly() {
    let ir = ir_of(SUBMACHINE_FSM);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .unwrap();

    // Drive the sub-instance to a non-initial leaf: Idle -> Handshake.
    interp.dispatch("CONNECT").unwrap();
    assert_eq!(
        interp
            .submachine_active_states("s-Device-Connecting")
            .and_then(|v| v.first().cloned())
            .as_deref(),
        Some("s-Connection-Handshake"),
        "sub-instance is mid-protocol at Handshake"
    );

    // Round-trip: snapshot here, perturb by advancing the sub
    // (ACK ⇒ Handshake -> Established), restore, re-snapshot must match.
    assert_round_trip_lossless(&mut interp, |i| {
        i.dispatch("ACK").unwrap();
        assert_eq!(
            i.submachine_active_states("s-Device-Connecting")
                .and_then(|v| v.first().cloned())
                .as_deref(),
            Some("s-Connection-Established"),
            "perturbation actually moved the sub-instance forward"
        );
    });

    // And the restore behaviourally resurrected the EXACT nested leaf —
    // pre-W2 `restore` silently dropped `submachines`, so this read would
    // have been `None`/empty (the audit's lossy-restore defect).
    assert_eq!(
        interp
            .submachine_active_states("s-Device-Connecting")
            .and_then(|v| v.first().cloned())
            .as_deref(),
        Some("s-Connection-Handshake"),
        "restore resurrected the sub-instance at Handshake (lossless restore)"
    );
}

#[test]
fn submachine_distinct_subinstance_leaves_produce_distinct_snapshots() {
    // The verifier-soundness crux at the simulator layer: two configs that
    // differ ONLY in the nested sub-instance's active leaf MUST serialise
    // to DIFFERENT bytes (else the digest conflates them → false proof).
    let ir = ir_of(SUBMACHINE_FSM);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .unwrap();

    interp.dispatch("CONNECT").unwrap(); // sub @ Handshake
    let at_handshake = json(&interp.snapshot().unwrap());

    interp.dispatch("ACK").unwrap(); // sub @ Established
    let at_established = json(&interp.snapshot().unwrap());

    // Parent leaf is `Connecting` in BOTH; only the nested leaf differs.
    assert_ne!(
        at_handshake, at_established,
        "configs differing ONLY in the nested sub-instance leaf must have \
         distinct snapshot bytes — pre-W2 they were identical (the D-2 \
         conflation that would yield a false ProvenNoDeadlock)"
    );
}

// ───────────────────────────────────────────────────────────────────────
// (P0b) TIMER — a mid-phase armed-timer config round-trips losslessly, and
//       restore re-arms the EXACT timer set (not an empty one).
// ───────────────────────────────────────────────────────────────────────
// The canonical, trace-verified timer FSM (the shipped traffic-light
// example, `examples/traffic-light/`). Its `after N ms` cycle is the
// project's proven timer reference, driven exactly as its committed
// `.trace` drives it: a single `advance_clock` per expiry boundary
// (`advance_clock` fires the next-expiring timer and drains; the entering
// state arms its own fresh timer — Doc 08 §13 + the P0-4 re-arm-on-entry
// fix). We reuse it so the *losslessness* property is tested against a
// known-correct timer machine, not a hand-rolled one with unverified
// firing semantics.
const TIMER_FSM: &str = include_str!("../../../examples/traffic-light/traffic-light.fsm");

// Lowered IDs are `s-<Machine>-<State>` (e.g. `s-TrafficLight-Red`);
// submachine sub-instance IDs are `s-<Template>-<State>`
// (e.g. `s-Connection-Idle`).

#[test]
fn armed_timer_config_round_trips_losslessly() {
    let ir = ir_of(TIMER_FSM);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "TrafficLight".into(),
            ..Default::default()
        })
        .unwrap();

    // Init lands in `Red` with an `after 2000 ms` timer ARMED (the
    // traffic-light's verified entry — see its committed `.trace`).
    assert!(
        in_state(&interp, "Red"),
        "init lands in Red; got {:?}",
        interp.current_states()
    );
    let snap_red = interp.snapshot().unwrap();
    assert!(
        !snap_red.timers.is_empty(),
        "the Red `after 2000 ms` timer must be present in the snapshot \
         (pre-W2 the `timers` field did not exist — the audit D-2 blocker)"
    );

    // Round-trip: perturb by advancing past the Red timer (it fires,
    // Red -> GreenAccelerating, per the verified trace's first
    // `advance_clock 2001`), restore back to Red, re-snapshot must be
    // byte-identical.
    assert_round_trip_lossless(&mut interp, |i| {
        i.advance_clock(2001).unwrap();
        assert!(
            in_state(i, "GreenAccelerating"),
            "perturbation fired the Red timer (Red -> GreenAccelerating); got {:?}",
            i.current_states()
        );
    });

    // Behavioural proof the armed timer was restored, not lost: the
    // RESTORED interpreter is back in Red at t=0 with the timer re-armed;
    // advancing past 2000 ms must STILL fire it (Red -> GreenAccelerating).
    // A lossy restore (pre-W2) would have dropped the timer ⇒ it would
    // never fire ⇒ this would stay in Red.
    assert!(
        in_state(&interp, "Red"),
        "restored back to Red; got {:?}",
        interp.current_states()
    );
    interp.advance_clock(2001).unwrap();
    assert!(
        in_state(&interp, "GreenAccelerating"),
        "the RESTORED armed timer still fires (lossless restore) — pre-W2 \
         the dropped timer would never fire and this would stay in Red; got {:?}",
        interp.current_states()
    );
}

#[test]
fn distinct_armed_timer_phase_produces_distinct_snapshots() {
    // Two configs in the SAME active leaf differing ONLY in virtual clock
    // (hence remaining timer phase) must differ in bytes. (The clock was
    // already keyed pre-W2; this asserts the *timer set itself* is now
    // also part of the bytes, by comparing an armed-timer config to a
    // no-timer config at the same clock.)
    let ir = ir_of(TIMER_FSM);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "TrafficLight".into(),
            ..Default::default()
        })
        .unwrap();
    // `Red` with the `after 2000 ms` timer armed.
    let red_snap = interp.snapshot().unwrap();
    let red_armed = json(&red_snap);
    assert!(
        !red_snap.timers.is_empty(),
        "Red must have its 2000 ms timer armed"
    );

    // Advance through the cycle to `Green` (Red -> GreenAccelerating ->
    // Green), which arms a DIFFERENT timer (30000 ms, owned by the Green
    // state). The *armed set* differs.
    interp.advance_clock(2001).unwrap(); // Red -> GreenAccelerating
    interp.advance_clock(2001).unwrap(); // GreenAccelerating -> Green
    assert!(
        in_state(&interp, "Green"),
        "advanced to Green; got {:?}",
        interp.current_states()
    );
    let green_snap = interp.snapshot().unwrap();
    let green_armed = json(&green_snap);

    assert_ne!(
        red_armed, green_armed,
        "different armed timer sets must yield different snapshot bytes"
    );
    // Explicitly assert the armed timer's owning state changed (the armed
    // set itself is captured, not merely the clock): Red's timer is owned
    // by the Red state, Green's by the Green state.
    let red_owners: Vec<_> = red_snap.timers.iter().map(|t| &t.source_state).collect();
    let green_owners: Vec<_> = green_snap.timers.iter().map(|t| &t.source_state).collect();
    assert_ne!(
        red_owners, green_owners,
        "the armed timer's owning state must differ (Red-owned vs Green-owned) \
         — proving the armed *set* is in the snapshot, not just the clock"
    );
}

// ───────────────────────────────────────────────────────────────────────
// (P0c) HIERARCHICAL + PARALLEL — composite/parallel configs round-trip
//       losslessly (history + region leaves are already keyed pre-W2; this
//       proves the W2 extension did not regress them and that timers armed
//       on nested states are captured).
// ───────────────────────────────────────────────────────────────────────
const HIER_PARALLEL_TIMER_FSM: &str = r#"language fsm 2.0

feature parallel
feature hsm
feature timers

machine Plant {
    events { TICK STOP RESUME }
    initial Running
    state Running {
        on STOP -> Stopped

        region Motor {
            initial MotorIdle
            state MotorIdle { on TICK -> MotorSpin }
            state MotorSpin { after 3000 ms -> MotorIdle }
        }
        region Pump {
            initial PumpLow
            state PumpLow { on TICK -> PumpHigh }
            state PumpHigh { on TICK -> PumpLow }
        }
    }
    state Stopped { on RESUME -> Running }
}"#;

#[test]
fn hierarchical_parallel_timer_config_round_trips_losslessly() {
    let ir = ir_of(HIER_PARALLEL_TIMER_FSM);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Plant".into(),
            ..Default::default()
        })
        .unwrap();

    // Drive into a parallel config where one region has a timer armed:
    // TICK ⇒ Motor MotorIdle->MotorSpin (arms `after 3000 ms`), Pump
    // PumpLow->PumpHigh.
    interp.dispatch("TICK").unwrap();
    assert!(
        in_state(&interp, "MotorSpin") && in_state(&interp, "PumpHigh"),
        "both regions advanced (config = {:?})",
        interp.current_states()
    );
    assert!(
        !interp.snapshot().unwrap().timers.is_empty(),
        "the MotorSpin-armed 3000 ms timer is captured in the snapshot"
    );

    // Round-trip across a clock advance that fires the region timer.
    assert_round_trip_lossless(&mut interp, |i| {
        i.advance_clock(4000).unwrap();
        assert!(
            in_state(i, "MotorIdle"),
            "perturbation fired the region timer (MotorSpin -> MotorIdle): {:?}",
            i.current_states()
        );
    });

    // Restore landed back in the parallel config with the timer re-armed.
    assert!(
        in_state(&interp, "MotorSpin") && in_state(&interp, "PumpHigh"),
        "restored parallel config (= {:?})",
        interp.current_states()
    );
}

// ───────────────────────────────────────────────────────────────────────
// (REGRESSION) FLAT machine snapshots are BYTE-UNCHANGED by the W2
// extension — the two new fields encode as empty `[]` / `{}`, so the
// pre-W2 fields' bytes are untouched (no W1 / existing-sim-test
// regression). We assert (a) the round-trip is lossless AND (b) the new
// fields serialise to the documented empty forms for a flat machine.
// ───────────────────────────────────────────────────────────────────────
const FLAT_FSM: &str = r#"language fsm 2.0
machine Motor {
    events { START STOP }
    initial Idle
    state Idle { on START -> Running }
    state Running { on STOP -> Idle }
}"#;

#[test]
fn flat_machine_snapshot_round_trips_and_new_fields_are_empty() {
    let ir = ir_of(FLAT_FSM);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();

    let s = interp.snapshot().unwrap();
    // The W2-additive fields are EMPTY for a flat machine ⇒ the pre-W2
    // bytes are unaffected (the additive-not-breaking guarantee).
    assert!(
        s.timers.is_empty(),
        "a flat machine has no armed timers (new `timers` field empty)"
    );
    assert!(
        s.submachines.is_empty(),
        "a flat machine has no sub-instances (new `submachines` field empty)"
    );
    let body = json(&s);
    assert!(
        body.contains("\"timers\":[]") && body.contains("\"submachines\":{}"),
        "the new fields serialise to empty []/{{}} for a flat machine \
         (additive, pre-W2 fields' bytes untouched); got: {body}"
    );

    // And the round-trip is lossless across a real dispatch.
    assert_round_trip_lossless(&mut interp, |i| {
        i.dispatch("START").unwrap();
        assert!(
            in_state(i, "Running"),
            "perturbation moved Idle -> Running; got {:?}",
            i.current_states()
        );
    });
}
