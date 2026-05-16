//! `fsm verify` CLI end-to-end acceptance (Doc 30 §5.4 / §4.3 — the W1
//! gate's CLI half). Behavioural, on real `.fsm` fixtures driven through
//! the *real binary* — symbol-presence is explicitly NOT acceptance.
//!
//! Covers the (a)–(e) acceptance + the documented exit-code contract +
//! the `--json` schema + determinism (byte-equal `--json` over two runs)
//! + the full `verify → generate → check` headless pipeline (the owner
//! pipeline-before-UI bar, Doc 30 §5.2 added gate).
//!
//! Fixtures live in the `fsm-verify` crate (the canonical W1 fixtures);
//! the CLI tests reuse them by absolute path.

use assert_cmd::Command;
use serde_json::Value;

fn fixture(name: &str) -> String {
    format!(
        "{}/../fsm-verify/tests/fixtures/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    )
}

fn run_verify(extra: &[&str]) -> (i32, String, String) {
    let out = Command::cargo_bin("fsm")
        .unwrap()
        .arg("verify")
        .args(extra)
        .output()
        .expect("spawn fsm");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// (c) sound machine ⇒ exit 0, "verified".
#[test]
fn sound_machine_exits_zero_verified() {
    let (code, stdout, _err) = run_verify(&[&fixture("motor_sound.fsm")]);
    assert_eq!(
        code, 0,
        "a deadlock-free machine must exit 0; stdout: {stdout}"
    );
    assert!(
        stdout.contains("verified"),
        "human output must say verified; got: {stdout}"
    );
}

// (b) genuine deadlock ⇒ exit 1, with the concrete witness in --json.
#[test]
fn deadlock_exits_one_with_witness_in_json() {
    let (code, stdout, _err) = run_verify(&["--json", &fixture("deadlock_guard_trap.fsm")]);
    assert_eq!(
        code, 1,
        "a reachable deadlock must exit 1; stdout: {stdout}"
    );

    let v: Value = serde_json::from_str(&stdout).expect("--json emits valid JSON");
    assert_eq!(v["schema"], "fsm-verify/v1");
    assert_eq!(v["verdict"], "property-violated");
    assert_eq!(v["exitCode"], 1);
    assert_eq!(v["properties"]["deadlockFree"]["result"], "violated");
    // The counterexample carries the concrete reaching event sequence.
    let witness = &v["properties"]["deadlockFree"]["counterexample"]["witness"];
    assert_eq!(
        witness,
        &serde_json::json!(["GO"]),
        "the JSON counterexample must carry the concrete witness trace; got {witness}"
    );
    let cfg = &v["properties"]["deadlockFree"]["counterexample"]["config"];
    assert!(
        cfg.as_array().map(|a| a.len() == 1).unwrap_or(false),
        "the deadlocked config must be the single Trap state; got {cfg}"
    );
}

// (a) unreachable state ⇒ FSM-E0400 in --json, exit 1 (property-violated:
//     a proven-unreachable state is a violation of the reachability
//     property, per the documented contract).
#[test]
fn unreachable_state_reports_fsm_e0400_and_exits_one() {
    let (code, stdout, _err) = run_verify(&["--json", &fixture("unreachable_island.fsm")]);
    assert_eq!(
        code, 1,
        "a proven-unreachable state violates the reachability property ⇒ exit 1; stdout: {stdout}"
    );
    let v: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["verdict"], "property-violated");
    assert_eq!(v["properties"]["reachability"]["result"], "violated");
    // The unreachable set carries the island; a diagnostic with code
    // FSM-E0400 is present.
    let unreachable = v["properties"]["reachability"]["unreachableStates"]
        .as_array()
        .expect("array");
    assert!(
        !unreachable.is_empty(),
        "the island state must appear in unreachableStates; got {unreachable:?}"
    );
    let diags = v["properties"]["reachability"]["diagnostics"]
        .as_array()
        .expect("array");
    assert!(
        diags
            .iter()
            .any(|d| d["code"] == "FSM-E0400" && d["severity"] == "error"),
        "an FSM-E0400 error diagnostic must be emitted; got {diags:?}"
    );
    // Deadlock property still holds (reachable subgraph is sound).
    assert_eq!(v["properties"]["deadlockFree"]["result"], "holds");
}

// (d) bound exceeded ⇒ INCONCLUSIVE, exit 2, explicitly NOT "verified".
#[test]
fn too_small_bound_exits_two_inconclusive_not_verified() {
    let (code, stdout, _err) =
        run_verify(&["--json", "--max-states", "1", &fixture("motor_sound.fsm")]);
    assert_eq!(
        code, 2,
        "hitting a bound must exit 2 (inconclusive), NOT 0; stdout: {stdout}"
    );
    let v: Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(
        v["verdict"], "inconclusive",
        "the verdict must be inconclusive, never verified, on a bound hit (false-proven guard)"
    );
    assert_ne!(v["verdict"], "verified");
    assert_eq!(v["bound"]["hit"], true);
    assert_eq!(v["bound"]["stopReason"], "max-states-hit");
}

// (e) determinism: byte-equal --json over two runs.
#[test]
fn json_is_byte_equal_across_two_runs() {
    let (_c1, a, _e1) = run_verify(&["--json", &fixture("deadlock_guard_trap.fsm")]);
    let (_c2, b, _e2) = run_verify(&["--json", &fixture("deadlock_guard_trap.fsm")]);
    assert_eq!(a, b, "--json output must be byte-identical across runs");

    // And the sound machine (exercises sorted reachable-set ordering).
    let (_c3, c, _e3) = run_verify(&["--json", &fixture("motor_sound.fsm")]);
    let (_c4, d, _e4) = run_verify(&["--json", &fixture("motor_sound.fsm")]);
    assert_eq!(
        c, d,
        "sound-machine --json must be byte-identical across runs"
    );
}

// Exit-code contract: a non-existent file ⇒ exit 3 (IO bucket), distinct
// from any verification verdict.
#[test]
fn missing_file_exits_three() {
    let (code, _o, err) = run_verify(&["/no/such/path/missing.fsm"]);
    assert_eq!(code, 3, "missing input must exit 3; stderr: {err}");
}

// Exit-code contract: a model that does not analyze ⇒ exit 4 (cannot
// verify what won't compile), distinct from a verdict.
#[test]
fn unanalyzable_model_exits_four() {
    let td = tempfile::tempdir().unwrap();
    let p = td.path().join("broken.fsm");
    std::fs::write(&p, "language fsm 2.0\nmachine Broken {\n  state Idle {\n").unwrap();
    let (code, _o, err) = run_verify(&[p.to_str().unwrap()]);
    assert_eq!(
        code, 4,
        "a model that does not compile cannot be verified ⇒ exit 4; stderr: {err}"
    );
}

// W2 relaxation (Doc 30 §4.2-W2 — supersedes the W1 `out_of_w1_scope`
// exit-4 rejection this test previously asserted). A composite/parallel/
// timer/submachine model is now VERIFIED end-to-end through the CLI, not
// rejected. (The durable STOP-not-wrong exit-4 path is still exercised by
// `unanalyzable_model_exits_four` above.) Here: a composite machine that
// is deadlock-free verifies (exit 0); a timer machine with a genuine
// every_internal deadlock is detected (exit 1) — both through the real
// binary with the documented exit-code contract intact.
#[test]
fn composite_and_timer_models_verify_end_to_end_in_w2() {
    let td = tempfile::tempdir().unwrap();

    // (i) A deadlock-free composite/parallel machine ⇒ exit 0 "verified".
    let parallel = td.path().join("parallel_sound.fsm");
    std::fs::write(
        &parallel,
        "language fsm 2.0\nfeature parallel\nfeature hsm\n\
         machine P {\n  events { T S }\n  initial Idle\n  \
         state Idle { on S -> Run }\n  \
         state Run {\n    on S -> Idle\n    \
         region A { initial A0 state A0 { on T -> A1 } state A1 { on T -> A0 } }\n    \
         region B { initial B0 state B0 { on T -> B1 } state B1 { on T -> B0 } }\n  }\n}\n",
    )
    .unwrap();
    let (code, stdout, err) = run_verify(&[parallel.to_str().unwrap()]);
    assert_eq!(
        code, 0,
        "a deadlock-free composite/parallel model now VERIFIES (W2) — \
         exit 0; stdout: {stdout}; stderr: {err}"
    );
    assert!(
        stdout.contains("verified"),
        "composite/parallel sound model must say verified; got: {stdout}"
    );

    // (ii) A genuine timer-deadlock (every_internal — config never
    //      changes) ⇒ exit 1 "property-violated" with a witness, detected
    //      end-to-end (the W2 timer-fire edge + extended deadlock def).
    let timer_dl = td.path().join("timer_dl.fsm");
    std::fs::write(
        &timer_dl,
        "language fsm 2.0\nfeature timers\nextern tick()\n\
         machine HB {\n  events { START }\n  initial Ok\n  \
         state Ok { on START -> Wedged }\n  \
         state Wedged { every 1000 ms : tick() }\n}\n",
    )
    .unwrap();
    let (code, stdout, err) = run_verify(&["--json", timer_dl.to_str().unwrap()]);
    assert_eq!(
        code, 1,
        "a genuine timer-deadlock is detected end-to-end ⇒ exit 1; \
         stdout: {stdout}; stderr: {err}"
    );
    let v: Value = serde_json::from_str(&stdout).expect("--json parses");
    assert_eq!(v["verdict"], "property-violated");
    assert_eq!(
        v["schema"], "fsm-verify/v1",
        "the schema stays additive fsm-verify/v1 across W2"
    );
}

// The owner pipeline-before-UI bar (Doc 30 §5.2 added gate): the full
// `fsm verify → fsm generate → fsm check` loop is driveable headless by a
// CI/factory script with the documented machine-readable contract and zero
// UI. This proves the closed loop, not just one step.
#[test]
fn full_verify_generate_check_pipeline_is_headless_driveable() {
    let motor = fixture("motor_sound.fsm");

    // 1. verify --json → parse the verdict programmatically (a CI step).
    let (vcode, vout, _ve) = run_verify(&["--json", &motor]);
    assert_eq!(vcode, 0, "verify must pass for the sound motor");
    let vj: Value = serde_json::from_str(&vout).expect("verify --json is machine-readable");
    assert_eq!(
        vj["verdict"], "verified",
        "a CI script keys on this field; it must be 'verified'"
    );

    // 2. Only if verified, generate C (the gate a factory enforces).
    let outdir = tempfile::tempdir().unwrap();
    Command::cargo_bin("fsm")
        .unwrap()
        .args([
            "generate",
            "--target",
            "c99",
            "--out",
            outdir.path().to_str().unwrap(),
            &motor,
        ])
        .assert()
        .success();

    // 3. check still passes (the model is consistent end-to-end).
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["check", &motor])
        .assert()
        .success();

    // The whole loop ran with zero UI, machine-readable output, documented
    // exit codes — the pipeline-before-UI bar is met.
}
