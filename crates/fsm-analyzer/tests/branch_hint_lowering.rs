//! Analyzer/IR acceptance for the v1.1-W4 `likely`/`rare` branch hint.
//!
//! Proves the AST→IR lowering boundary for the new construct:
//!
//!  1. The analyzer lowers a `likely`/`rare` transition prefix into
//!     `TransitionObject.hint = Some(Likely|Rare)`, and an unhinted
//!     transition into `hint = None` — across every trigger flavour.
//!  2. The lowered IR is schema-valid (the W0 gate would otherwise have
//!     already panicked inside `analyze_with_source` for a debug build —
//!     here we additionally validate explicitly).
//!  3. The IR round-trips through the canonical `to_json` / `from_json`
//!     with the `hint` field preserved.
//!  4. serde back-compat: a pre-W4 IR JSON document (one with NO `hint`
//!     key on its transitions) still deserializes — `hint` defaults to
//!     `None` (`#[serde(default)]`), so old artifacts keep loading.
//!
//! FAIL-on-main: `likely on …` does not parse there (`FSM-E0010`) and
//! `TransitionObject` has no `hint` field, so neither the lowering
//! assertions nor this file's `hint:` references compile on `main`.

use fsm_analyzer::analyze_with_source;
use fsm_ir::{BranchHint, MachineObject, StateNode, TransitionObject};
use fsm_parser::parse;

fn lower(src: &str) -> MachineObject {
    let pr = parse(src);
    assert!(
        pr.errors.is_empty(),
        "parse errors for fixture:\n{src}\n{:#?}",
        pr.errors
    );
    let res = analyze_with_source(&pr, "hint.fsm", src);
    let errs: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
        .collect();
    assert!(errs.is_empty(), "analyzer errors: {errs:?}");
    res.ir.expect("ir produced").machines.remove(0)
}

fn all_transitions(m: &MachineObject) -> Vec<TransitionObject> {
    let mut out = Vec::new();
    fn walk(states: &[StateNode], out: &mut Vec<TransitionObject>) {
        for s in states {
            match s {
                StateNode::Simple(s) => out.extend(s.transitions.iter().cloned()),
                StateNode::Composite(c) => {
                    out.extend(c.transitions.iter().cloned());
                    for r in &c.regions {
                        walk(&r.states, out);
                    }
                }
                StateNode::Parallel(p) => {
                    out.extend(p.transitions.iter().cloned());
                    for r in &p.regions {
                        walk(&r.states, out);
                    }
                }
                StateNode::Submachine(sm) => out.extend(sm.transitions.iter().cloned()),
                _ => {}
            }
        }
    }
    walk(&m.root.states, &mut out);
    out
}

fn hint_for(trs: &[TransitionObject], src_substr: &str, tgt_substr: &str) -> Option<BranchHint> {
    trs.iter()
        .find(|t| t.source.contains(src_substr) && t.target.contains(tgt_substr))
        .unwrap_or_else(|| panic!("no transition {src_substr} -> {tgt_substr}"))
        .hint
}

#[test]
fn analyzer_lowers_likely_rare_and_none_across_flavours() {
    // Deterministic fixture: each transition keys off a DISTINCT event so
    // no E0300 nondeterminism — the point here is hint lowering across
    // flavours, not dispatch ambiguity. One of each: likely-external,
    // rare-external, unhinted-external, likely-local(`~>`), rare-internal,
    // rare-completion, likely-timed(`after`).
    let m = lower(
        r#"language fsm 2.0
feature timers
pure extern g() : bool
extern act()
machine M {
    events { GO PING DONE_EV LOOP TICK }
    initial Idle
    state Idle {
        likely on GO [g()] -> Running
    }
    state Running {
        rare   on PING -> Faulted
        on DONE_EV -> Idle
        likely on LOOP ~> Running
        rare   on TICK : act()
        rare   done -> Idle
        likely after 1000 ms -> Faulted
    }
    state Faulted {
        on GO -> Idle
    }
}"#,
    );
    let trs = all_transitions(&m);

    // External: likely / rare / unhinted.
    assert_eq!(
        hint_for(&trs, "Idle", "Running"),
        Some(BranchHint::Likely),
        "`likely on GO` external → hint Likely"
    );
    assert_eq!(
        hint_for(&trs, "Running", "Faulted"),
        Some(BranchHint::Rare),
        "`rare on PING` external → hint Rare"
    );
    assert_eq!(
        hint_for(&trs, "Running", "Idle"),
        None,
        "unhinted `on DONE_EV` external → None"
    );

    // The lowered set must contain at least one Likely, one Rare, one None
    // (covers local `~>`, internal, completion, timed forms too — they all
    // route through `build_transition`'s new `hint` arg).
    let likely = trs
        .iter()
        .filter(|t| t.hint == Some(BranchHint::Likely))
        .count();
    let rare = trs
        .iter()
        .filter(|t| t.hint == Some(BranchHint::Rare))
        .count();
    let none = trs.iter().filter(|t| t.hint.is_none()).count();
    assert!(
        likely >= 2 && rare >= 2 && none >= 1,
        "expected ≥2 Likely, ≥2 Rare, ≥1 None across flavours; \
         got likely={likely} rare={rare} none={none}"
    );
}

#[test]
fn lowered_ir_is_schema_valid_and_json_round_trips_with_hint() {
    let m = lower(
        r#"language fsm 2.0
pure extern g() : bool
machine M {
    events { GO STOP }
    initial Idle
    state Idle { likely on GO [g()] -> Running }
    state Running { rare on STOP [g()] -> Idle }
}"#,
    );
    // Rebuild a full Ir wrapper for serde + schema (analyze already
    // schema-gated it in debug; assert explicitly regardless of profile).
    let ir = fsm_ir::Ir {
        ir_version: "1.0.0".into(),
        source_hash: "sha256:test".into(),
        source_files: vec!["hint.fsm".into()],
        machines: vec![m],
        diagnostics: vec![],
    };

    // (2) schema-valid. Gated like `ir_schema_gate.rs` — the validator is
    // behind the (default-on) `schema-validate` feature; skip cleanly if a
    // build turns it off rather than failing to compile.
    #[cfg(feature = "schema-validate")]
    fsm_ir::validate_ir_against_schema(&ir)
        .expect("lowered IR with `hint` must validate against schema/ir/1.0.0");

    // (3) round-trip: hint survives canonical to_json → from_json.
    // `to_json` is `serde_json::to_string_pretty` (space after `:`), so
    // compare on a whitespace-stripped copy to stay format-agnostic.
    let json = fsm_ir::to_json(&ir).expect("serialize");
    let compact: String = json.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("\"hint\":\"likely\""),
        "serialized IR must carry the lowered likely hint; json:\n{json}"
    );
    assert!(
        compact.contains("\"hint\":\"rare\""),
        "serialized IR must carry the lowered rare hint"
    );
    let back = fsm_ir::from_json(&json).expect("deserialize");
    assert_eq!(back, ir, "IR must round-trip byte-for-byte through JSON");
}

#[test]
fn pre_w4_ir_json_without_hint_key_still_deserializes() {
    // A minimal IR document hand-written WITHOUT any `hint` key on the
    // transition — exactly the shape every pre-W4 artifact has. serde's
    // `#[serde(default)]` on `TransitionObject.hint` must accept it and
    // default to `None` (no breaking change to stored IR / downstream
    // consumers).
    let legacy = r#"{
      "irVersion": "1.0.0",
      "sourceHash": "sha256:legacy",
      "sourceFiles": ["old.fsm"],
      "machines": [{
        "id": "m", "stableId": "M", "name": "M",
        "context": { "fields": [] },
        "events": [{ "id": "e-go", "stableId": "M:event:GO", "name": "GO",
                     "payload": [],
                     "loc": { "file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1 } }],
        "externs": [],
        "root": {
          "id": "r", "name": "__root", "initial": "ps",
          "states": [
            { "kind":"initial","id":"ps","target":"s-a",
              "loc":{"file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1} },
            { "kind":"simple","id":"s-a","stableId":"M:state:A","name":"A",
              "entry":[],"exit":[],
              "transitions":[{
                "id":"t","stableId":"M:transition:t","source":"s-a","target":"s-a",
                "trigger":{"kind":"event","eventId":"e-go"},
                "actions":[],"priority":100,"kind":"external",
                "loc":{"file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1}
              }],
              "timers":[],"defers":[],
              "loc":{"file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1} }
          ],
          "priority":0,
          "loc":{"file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1}
        },
        "submachines":[],"consts":[],"imports":[],"features":[],
        "queue":{"capacity":8,"overflowPolicy":"assert",
                 "loc":{"file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1}},
        "targets":[],
        "loc":{"file":"old.fsm","span":{"start":0,"end":1},"line":1,"column":1}
      }],
      "diagnostics": []
    }"#;

    let ir =
        fsm_ir::from_json(legacy).expect("pre-W4 IR JSON (no `hint` key) must still deserialize");
    let t = match &ir.machines[0].root.states[1] {
        StateNode::Simple(s) => &s.transitions[0],
        _ => panic!("expected simple state"),
    };
    assert_eq!(
        t.hint, None,
        "a transition with no `hint` key must default to None (serde-default back-compat)"
    );
}
