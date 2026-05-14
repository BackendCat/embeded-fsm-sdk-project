//! Determinism integration tests — overlapping guards, dead transitions,
//! priority resolution.

use fsm_analyzer::{analyze, DiagnosticCode};
use fsm_parser::parse;

fn diagnostics(src: &str) -> Vec<fsm_diagnostics::Diagnostic> {
    analyze(&parse(src)).diagnostics
}

#[test]
fn disjoint_eq_guards_are_accepted() {
    let src = r#"language fsm 2.0
machine M {
    events { E }
    context { mode : u8 = 0 }
    initial S
    state S {
        on E [ctx.mode == 1] -> S
        on E [ctx.mode == 2] -> S
    }
}"#;
    let diags = diagnostics(src);
    assert!(
        !diags.iter().any(|d| d.code == DiagnosticCode::E0300),
        "diagnostics: {diags:#?}"
    );
}

#[test]
fn disjoint_interval_guards_are_accepted() {
    let src = r#"language fsm 2.0
machine M {
    events { E }
    context { x : u16 = 0 }
    initial S
    state S {
        on E [ctx.x < 10] -> S
        on E [ctx.x >= 10] -> S
    }
}"#;
    let diags = diagnostics(src);
    assert!(!diags.iter().any(|d| d.code == DiagnosticCode::E0300));
}

#[test]
fn overlapping_interval_guards_emit_e0300() {
    let src = r#"language fsm 2.0
machine M {
    events { E }
    context { x : u16 = 0 }
    initial S
    state S {
        on E [ctx.x > 5] -> S
        on E [ctx.x > 10] -> S
    }
}"#;
    let diags = diagnostics(src);
    assert!(diags.iter().any(|d| d.code == DiagnosticCode::E0300));
}

#[test]
fn else_branch_makes_residual_unguarded() {
    let src = r#"language fsm 2.0
machine M {
    events { E }
    context { x : u16 = 0 }
    initial S
    state S {
        on E [ctx.x > 5] -> S
        on E [else] -> S
    }
}"#;
    let diags = diagnostics(src);
    assert!(!diags.iter().any(|d| d.code == DiagnosticCode::E0300));
}

#[test]
fn equal_priorities_still_overlap() {
    let src = r#"language fsm 2.0
machine M {
    events { E }
    context { x : u16 = 0 }
    initial S
    state S {
        on E [ctx.x > 5] priority 1 -> S
        on E [ctx.x > 10] priority 1 -> S
    }
}"#;
    let diags = diagnostics(src);
    // Same priorities cannot resolve a conflict — E0300 should fire.
    assert!(diags.iter().any(|d| d.code == DiagnosticCode::E0300));
}
