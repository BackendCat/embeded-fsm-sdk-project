//! Negative integration tests — one per diagnostic code the analyzer is
//! responsible for emitting. Each test:
//! 1. parses a tiny source designed to trigger exactly one analyzer-level
//!    diagnostic;
//! 2. runs `analyze`;
//! 3. asserts the expected code appears in the diagnostic list.
//!
//! Test numbering follows Doc 00 §B / §G test plan plus the new codes
//! (E0410, E0111). E0903 ("defer not supported") was retired in v1.1 when
//! the defer runtime shipped; only E0310 (defer-vs-transition conflict)
//! remains for defer. PARSE-NEG-* corrections from Doc 00 G-11 are
//! reflected here.

use fsm_analyzer::{analyze, DiagnosticCode};
use fsm_parser::parse;

fn has_code(d: &[fsm_diagnostics::Diagnostic], code: DiagnosticCode) -> bool {
    d.iter().any(|x| x.code == code)
}

// ---------------------------------------------------------------------------
// Duplicate-name codes
// ---------------------------------------------------------------------------

#[test]
fn e0020_duplicate_machine() {
    let pr = parse("language fsm 2.0\nmachine M { }\nmachine M { }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0020));
}

#[test]
fn e0021_duplicate_state() {
    let pr = parse("language fsm 2.0\nmachine M { initial A state A { } state A { } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0021));
}

#[test]
fn e0022_duplicate_event() {
    let pr = parse("language fsm 2.0\nmachine M { events { START START } initial S state S { } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0022));
}

#[test]
fn e0023_duplicate_context_field() {
    let pr = parse(
        "language fsm 2.0\nmachine M { context { x : u8 = 0\nx : u16 = 0 } initial S state S { } }",
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0023));
}

#[test]
fn e0024_duplicate_extern() {
    let pr = parse(
        "language fsm 2.0\nmachine M { extern foo() : bool\nextern foo() : bool\ninitial S state S { } }",
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0024));
}

// ---------------------------------------------------------------------------
// Name resolution
// ---------------------------------------------------------------------------

#[test]
fn e0100_unknown_state_in_transition_target() {
    let pr =
        parse("language fsm 2.0\nmachine M { events { E } initial S state S { on E -> Unknown } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0100));
}

#[test]
fn e0100_unknown_state_in_initial() {
    let pr = parse("language fsm 2.0\nmachine M { initial Ghost\nstate S { } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0100));
}

#[test]
fn e0101_unknown_event_in_trigger() {
    let pr = parse("language fsm 2.0\nmachine M { initial S state S { on E -> S } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0101));
}

#[test]
fn e0102_unknown_extern_in_call() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial S
    state S { on E -> S : missing() }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0102));
}

#[test]
fn e0103_unknown_machine_in_send() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial S
    state S { on E -> S : send E to Ghost }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0103));
}

#[test]
fn e0104_unknown_context_field() {
    // Per Doc 00 NEG-008 correction.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    context { speed : u16 = 0 }
    initial S
    state S { on E [ctx.unknown == 1] -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0104));
}

#[test]
fn e0106_non_pure_extern_in_guard() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    extern impure() : bool
    initial S
    state S { on E [impure()] -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0106));
}

#[test]
fn e0107_missing_initial_declaration() {
    let pr = parse("language fsm 2.0\nmachine M { state S { } state T { } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0107));
}

#[test]
fn e0108_multiple_initial_declarations() {
    let pr = parse("language fsm 2.0\nmachine M { initial A\ninitial B\nstate A { } state B { } }");
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0108));
}

#[test]
fn e0109_history_default_unknown_state() {
    let pr = parse(
        r#"language fsm 2.0
feature history
machine M {
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        shallow_history H { initial Phantom }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0109));
}

// ---------------------------------------------------------------------------
// Type / semantic errors
// ---------------------------------------------------------------------------

#[test]
fn e0200_guard_numeric_literal() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial S
    state S { on E [42] -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0200));
}

#[test]
fn e0201_assign_negative_to_unsigned() {
    // Per Doc 00 NEG-010 correction (E0200 → E0201).
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    context { speed : u16 = 0 }
    initial S
    state S { on E -> S : ctx.speed = -1 }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0201));
}

#[test]
fn e0204_assign_to_payload_field() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E(value : u8) }
    initial S
    state S { on E -> S : payload.value = 5 }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0204));
}

#[test]
fn e0208_negative_default_in_unsigned() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    context { count : u16 = -1 }
    initial S
    state S { }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0208));
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn e0300_multiple_unguarded_transitions_same_event() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial A
    state A { on E -> A   on E -> B }
    state B { }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0300));
}

#[test]
fn e0300_overlapping_field_guards() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    context { x : u16 = 0 }
    initial S
    state S {
        on E [ctx.x > 5] -> S
        on E [ctx.x > 10] -> S
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0300));
}

#[test]
fn w0300_priority_resolved_conflict() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    context { x : u16 = 0 }
    initial S
    state S {
        on E [ctx.x > 5] priority 1 -> S
        on E [ctx.x > 10] priority 2 -> S
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::W0300));
}

// ---------------------------------------------------------------------------
// Reachability / timers
// ---------------------------------------------------------------------------

#[test]
fn e0410_timer_zero_ms() {
    // Per Doc 00 §B-13.
    let pr = parse(
        r#"language fsm 2.0
feature timers
machine M {
    initial S
    state S { after 0 ms -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0410));
}

#[test]
fn e0410_timer_negative_ms() {
    let pr = parse(
        r#"language fsm 2.0
feature timers
machine M {
    initial S
    state S { after -5 ms -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0410));
}

#[test]
fn w0601_timer_exceeds_24_hours() {
    let pr = parse(
        r#"language fsm 2.0
feature timers
machine M {
    initial S
    state S { after 100000000 ms -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::W0601));
}

// ---------------------------------------------------------------------------
// Parallel / submachine
// ---------------------------------------------------------------------------

#[test]
fn e0600_region_missing_initial() {
    let pr = parse(
        r#"language fsm 2.0
feature parallel
machine M {
    initial Working
    state Working {
        region A { state S1 { } state S2 { } }
        region B { initial T1 state T1 { } }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0600));
}

#[test]
fn h0004_single_region_parallel() {
    let pr = parse(
        r#"language fsm 2.0
feature parallel
machine M {
    initial Working
    state Working {
        region A { initial S1 state S1 { } state S2 { } }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::H0004));
}

#[test]
fn w0600_region_with_one_state() {
    let pr = parse(
        r#"language fsm 2.0
feature parallel
machine M {
    initial Working
    state Working {
        region A { initial S1 state S1 { } }
        region B { initial T1 state T1 { } state T2 { } }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::W0600));
}

// ---------------------------------------------------------------------------
// History — B-14
// ---------------------------------------------------------------------------

#[test]
fn e0111_history_missing_default() {
    // Per Doc 00 §B-14.
    let pr = parse(
        r#"language fsm 2.0
feature history
machine M {
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        shallow_history H { }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0111));
}

#[test]
fn e0111_deep_history_missing_default() {
    let pr = parse(
        r#"language fsm 2.0
feature history
machine M {
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        deep_history H { }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0111));
}

// ---------------------------------------------------------------------------
// Defer
// ---------------------------------------------------------------------------

#[test]
fn e0310_defer_conflicts_with_transition() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { DATA }
    initial S
    state S {
        defer DATA
        on DATA -> S
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0310));
}

// ---------------------------------------------------------------------------
// Reachability E0401 — external self-transition on composite
// ---------------------------------------------------------------------------

#[test]
fn e0401_external_self_on_composite_state() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        on E -> Outer
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0401));
}

// ---------------------------------------------------------------------------
// W0603 — constant guard
// ---------------------------------------------------------------------------

#[test]
fn w0603_constant_guard_true() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial S
    state S { on E [true] -> S }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::W0603));
}

// ---------------------------------------------------------------------------
// W0101 — dead transition (completion after [else])
// ---------------------------------------------------------------------------

#[test]
fn w0101_completion_after_else() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    context { x : u8 = 0 }
    initial S
    state S {
        done [else] -> S
        done [ctx.x == 1] -> S
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::W0101));
}

// ---------------------------------------------------------------------------
// E0205 — invalid LHS
// ---------------------------------------------------------------------------

#[test]
fn e0205_invalid_assignment_lhs() {
    // Bare identifier on the LHS (not ctx./payload.field) triggers E0205.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { E }
    initial S
    state S { on E -> S : bare_var = 1 }
}"#,
    );
    let res = analyze(&pr);
    assert!(has_code(&res.diagnostics, DiagnosticCode::E0205));
}

// ---------------------------------------------------------------------------
// W0200 — loop in action block (Doc 02 §9.2 / Doc 04 §8.7.2 / Doc 10
// §FSM-W0200). IMPLEMENTed by v1.2-FU-DEAD-CODES: the corpus mandates the
// compiler emit it ("The compiler emits FSM-W0200 … when a loop appears in
// an action block"), so the analyzer SHOULD — and now does. §5.4: a real
// `analyze()` over a triggering fixture must surface W0200 (Warning, not
// blocking) with the exact catalog message; a control with NO loop must NOT.
// ---------------------------------------------------------------------------

#[test]
fn w0200_while_loop_in_transition_action_block() {
    // `while` in a transition `:` action block — the canonical W0200 case
    // (Doc 04 §8.7.2 "Loops: while, for (emit FSM-W0200 …)").
    let pr = parse(
        r#"language fsm 2.0
machine M {
    context { n : u8 = 3 }
    events { E }
    initial S
    state S { on E -> S : while (ctx.n) { ctx.n = ctx.n - 1 } }
}"#,
    );
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::W0200),
        "while-in-action must emit FSM-W0200; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
    // It is a Warning (compilation continues; code is still generated —
    // Doc 11 §18), and carries the exact Doc 10 §FSM-W0200 "Message".
    let d = res
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::W0200)
        .unwrap();
    assert_eq!(d.severity, fsm_diagnostics::Severity::Warning);
    assert!(
        d.message.contains("Loop in action block")
            && d.message.contains("bounded-execution analysis"),
        "unexpected W0200 message: {:?}",
        d.message
    );
}

#[test]
fn w0200_for_loop_in_entry_action_block() {
    // `for` in a state `entry:` action block — exercises the entry/exit
    // ACTION_BLOCK path (not just transition `:` actions).
    let pr = parse(
        r#"language fsm 2.0
machine M {
    context { i : u8 = 0, sum : u8 = 0 }
    initial S
    state S {
        entry: for (ctx.i = 0; ctx.i < 10; ctx.i = ctx.i + 1) { ctx.sum = ctx.sum + ctx.i }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::W0200),
        "for-in-entry-action must emit FSM-W0200; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w0200_not_emitted_for_loop_free_action_block() {
    // CONTROL: an action block with assignments + an `if` but NO loop must
    // NOT emit W0200 (proves the check is loop-specific, not "any action").
    let pr = parse(
        r#"language fsm 2.0
machine M {
    context { x : u8 = 0 }
    events { E }
    initial S
    state S {
        entry: ctx.x = 1
        on E -> S : if (ctx.x > 0) { ctx.x = ctx.x - 1 }
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(
        !has_code(&res.diagnostics, DiagnosticCode::W0200),
        "loop-free action must NOT emit FSM-W0200; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// W0500 — "extern declared but never used" was RETIRED by
// v1.2-FU-DEAD-CODES (vestigial: a bare catalog title, no normative "the
// compiler emits" statement, zero emitters, the one module that considered
// it declined it, its whole "unused declaration" family unbuilt). §5.4
// (RETIRE arm): the previously-"would-trigger" input — an `extern` declared
// but never referenced — must now produce the CORRECT behaviour, i.e.
// *nothing* (no W0500 and no other spurious diagnostic; not a silent
// change to some other code). The catalog-absence + still-parses-in-
// suppression assertions live in fsm-diagnostics' own unit tests.
// ---------------------------------------------------------------------------

#[test]
fn unused_extern_is_clean_w0500_retired_no_substitute_diagnostic() {
    // Two externs declared, neither referenced anywhere — the exact shape
    // the never-implemented W0500 "unused extern" check would have flagged.
    // Post-retirement the correct behaviour is a fully clean analysis.
    let pr = parse(
        "language fsm 2.0\nmachine M {\npure extern is_ready() : bool\nextern do_step()\ninitial S\nstate S { }\n}",
    );
    let res = analyze(&pr);
    assert!(
        res.diagnostics.is_empty(),
        "unused-extern source must analyze cleanly after W0500 retirement \
         (no W0500, no silent substitute diagnostic); got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}
