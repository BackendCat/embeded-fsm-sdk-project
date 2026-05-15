//! Parser-layer acceptance for the submachine surface syntax (Doc 04 §15).
//!
//! Scope is the parse layer only (v1.1-W2a): the new `submachine Name { … }`
//! top-level form and the `state Name is SubName { … }` reference form must
//! produce the correct CST/AST shape and round-trip the source byte-for-byte.
//! Semantic lowering / codegen are later waves (W2b/c/d).
//!
//! Every test here FAILS on `main` (submachine → FSM-E0010 "expected
//! top-level declaration" / "unexpected 'is'") and PASSES after the W2a
//! grammar lands, per FSM-PROC-SUBAGENT §5.1.

use fsm_parser::ast::SubmachineRef;
use fsm_parser::{parse, SyntaxKind};

/// Convenience: parse `src` and assert it produced zero diagnostics.
fn parse_clean(src: &str) -> fsm_parser::ParseResult {
    let pr = parse(src);
    assert!(
        pr.errors.is_empty(),
        "expected clean parse for:\n---\n{}\n---\nerrors: {:#?}",
        src,
        pr.errors
    );
    pr
}

// ─── Positive: top-level submachine declaration ──────────────────────────

#[test]
fn top_level_submachine_decl_parses_with_zero_errors() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature submachines

submachine Connection {
    initial Idle
    state Idle { on START -> Online }
    state Online { on STOP -> Idle }
}

machine Parent {
    initial Connecting
    state Connecting { on GO -> Connecting }
}"#,
    );

    // The AST exposes the submachine as a first-class declaration, disjoint
    // from machines.
    let subs: Vec<_> = pr.ast().submachines().collect();
    assert_eq!(subs.len(), 1, "exactly one SubmachineDecl expected");
    assert_eq!(subs[0].name().as_deref(), Some("Connection"));

    // Its body reuses the machine-item accessors (initial + two states).
    assert_eq!(
        subs[0].initial().and_then(|i| i.target()).as_deref(),
        Some("Idle")
    );
    assert_eq!(subs[0].states().count(), 2);

    // The top-level `machine` and `submachine` views are disjoint.
    let machines: Vec<_> = pr.ast().machines().collect();
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].name().as_deref(), Some("Parent"));
}

#[test]
fn submachine_decl_accepts_stable_id_and_doc_prefix() {
    // Doc 04 §15: `submachine_decl = [doc_comment] , [stable_id] ,
    // "submachine" , identifier , "{" , machine_body , "}" ;`
    //
    // The `@id(...)` / `///` prefix is consumed by the shared top-level
    // doc-or-id dispatch path (same path as `machine_decl`), so a prefixed
    // submachine must parse with zero diagnostics. (Per the codebase's
    // stable-id model the STABLE_ID_ANNOT is a FILE-level sibling of the
    // decl on this path — identical to `machine`; this wave does not change
    // that shared wiring, only the submachine grammar/CST/AST.)
    let pr = parse_clean(
        r#"language fsm 2.0
feature submachines

@id("sm-conn")
/// A reusable connection handler.
submachine Connection {
    initial Idle
    state Idle { }
}"#,
    );
    let sub = pr.ast().submachines().next().unwrap();
    assert_eq!(sub.name().as_deref(), Some("Connection"));
    // The stable-id annotation survives in the CST (round-trip-preserving)
    // as a sibling node, mirroring the existing machine-decl behaviour.
    let saw_stable_id = pr
        .syntax()
        .descendants()
        .any(|n| n.kind() == SyntaxKind::STABLE_ID_ANNOT);
    assert!(
        saw_stable_id,
        "the @id(...) prefix is preserved as a STABLE_ID_ANNOT node"
    );
}

// ─── Positive: `state Name is SubName { … }` reference form ──────────────

#[test]
fn state_is_submachine_reference_exposes_name_and_transitions() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature submachines

submachine Connection {
    initial Idle
    state Idle { }
}

machine Parent {
    initial Connecting
    state Connecting is Connection {
        on T -> Online
        done -> Online
    }
    state Online { }
}"#,
    );

    let parent = pr.ast().machines().next().unwrap();
    let connecting = parent
        .states()
        .find(|s| s.name().as_deref() == Some("Connecting"))
        .expect("Connecting state present");

    // The `is Connection` binding is exposed as an optional typed child.
    let sref: SubmachineRef = connecting
        .submachine_ref()
        .expect("state exposes submachine_ref()");
    assert_eq!(sref.name().as_deref(), Some("Connection"));

    // The state STILL exposes its own transitions + completion edge.
    assert_eq!(
        connecting.transitions().count(),
        1,
        "the `on T -> Online` transition survives alongside `is`"
    );
    assert_eq!(
        connecting.completions().count(),
        1,
        "the `done -> Online` completion edge survives alongside `is`"
    );

    // An ordinary state has no submachine reference.
    let online = parent
        .states()
        .find(|s| s.name().as_deref() == Some("Online"))
        .unwrap();
    assert!(online.submachine_ref().is_none());
}

#[test]
fn state_is_submachine_reference_with_entry_exit() {
    // §15: the reference body may carry entry/exit alongside `done ->`.
    let pr = parse_clean(
        r#"language fsm 2.0
feature submachines

submachine Connection { initial Idle  state Idle { } }

machine Parent {
    initial Link
    state Link is Connection {
        entry : noop()
        exit  : noop()
        done -> Done
    }
    state Done { }
}"#,
    );
    let link = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(
        link.submachine_ref().and_then(|r| r.name()).as_deref(),
        Some("Connection")
    );
    assert!(link.entry().is_some(), "entry survives alongside `is`");
    assert!(link.exit().is_some(), "exit survives alongside `is`");
    assert_eq!(link.completions().count(), 1);
}

// ─── Negative + recovery ─────────────────────────────────────────────────

#[test]
fn submachine_without_name_emits_diagnostic_and_recovers() {
    // `submachine { }` — missing name. The parser must emit a diagnostic
    // and keep going so a subsequent valid machine still parses.
    let pr = parse(
        r#"language fsm 2.0
submachine { }
machine Good { }"#,
    );
    assert!(
        !pr.errors.is_empty(),
        "missing submachine name must produce a diagnostic"
    );
    let names: Vec<_> = pr.ast().machines().filter_map(|m| m.name()).collect();
    assert!(
        names.contains(&"Good".to_string()),
        "parser recovered and saw the later machine; names: {names:?}"
    );
}

#[test]
fn state_is_without_submachine_name_emits_diagnostic_and_recovers() {
    // `state X is { }` — `is` with no submachine name.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    initial X
    state X is { }
    state Y { }
}"#,
    );
    assert!(
        !pr.errors.is_empty(),
        "missing submachine name after `is` must produce a diagnostic"
    );
    // Recovery: the sibling state Y still parses.
    let names: Vec<_> = pr
        .ast()
        .machines()
        .next()
        .unwrap()
        .states()
        .filter_map(|s| s.name())
        .collect();
    assert!(
        names.contains(&"Y".to_string()),
        "parser recovered past the malformed `is`; names: {names:?}"
    );
}

#[test]
fn state_is_with_junk_after_name_emits_diagnostic_and_recovers() {
    // `state X is Y Z {}` — junk token `Z` between the submachine name and
    // the body. The parser must flag it and still parse the body + siblings.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    initial X
    state X is Y Z { on E -> X }
    state W { }
}"#,
    );
    assert!(
        !pr.errors.is_empty(),
        "junk after submachine reference name must produce a diagnostic"
    );
    let m = pr.ast().machines().next().unwrap();
    let names: Vec<_> = m.states().filter_map(|s| s.name()).collect();
    assert!(
        names.contains(&"W".to_string()),
        "parser resynced to the brace and kept parsing; names: {names:?}"
    );
    // The valid reference name was still captured before the junk.
    let x = m
        .states()
        .find(|s| s.name().as_deref() == Some("X"))
        .unwrap();
    assert_eq!(
        x.submachine_ref().and_then(|r| r.name()).as_deref(),
        Some("Y")
    );
}

// ─── CST round-trip (rowan lossless invariant) ───────────────────────────

#[test]
fn submachine_cst_round_trips_source_byte_for_byte() {
    // Exercises both new forms plus interleaved trivia (comments, blank
    // lines, doc comments, stable IDs) — the green tree must reproduce the
    // input exactly.
    let src = r#"language fsm 2.0

feature submachines

/// A reusable connection handler.
@id("sm-conn")
submachine Connection {
    initial Idle
    // inner comment
    state Idle { on START -> Online }
    state Online { on STOP -> Idle }
}

machine Parent {
    initial Connecting

    /* the link state is a submachine instance */
    state Connecting is Connection {
        on TIMEOUT -> Connecting
        done -> Online
    }
    state Online { }
}
"#;
    let pr = parse(src);
    assert_eq!(
        pr.reconstructed_text(),
        src,
        "rowan green tree must reproduce the source byte-for-byte"
    );
}

#[test]
fn submachine_decl_produces_distinct_cst_node_kind() {
    // The top-level form yields a SUBMACHINE_DECL node and the reference
    // form yields a SUBMACHINE_REF node nested under STATE_DECL — neither
    // exists on `main` (the syntax fails to parse there entirely).
    let pr = parse_clean(
        r#"language fsm 2.0
feature submachines

submachine S { initial A  state A { } }

machine M {
    initial P
    state P is S { }
}"#,
    );
    let root = pr.syntax();
    let mut saw_submachine_decl = false;
    let mut saw_submachine_ref = false;
    for n in root.descendants() {
        match n.kind() {
            SyntaxKind::SUBMACHINE_DECL => saw_submachine_decl = true,
            SyntaxKind::SUBMACHINE_REF => {
                saw_submachine_ref = true;
                // The SUBMACHINE_REF is a child of a STATE_DECL.
                assert_eq!(
                    n.parent().map(|p| p.kind()),
                    Some(SyntaxKind::STATE_DECL),
                    "SUBMACHINE_REF must nest under STATE_DECL"
                );
            }
            _ => {}
        }
    }
    assert!(saw_submachine_decl, "expected a SUBMACHINE_DECL node");
    assert!(saw_submachine_ref, "expected a SUBMACHINE_REF node");
}
