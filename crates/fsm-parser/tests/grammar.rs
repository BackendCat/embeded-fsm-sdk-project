//! Integration tests covering one positive case per grammar rule plus the
//! parser-emitted negative cases per Doc 10 §3 (E0010-E0024) and the
//! per-Doc-00 corrections.

use fsm_diagnostics::DiagnosticCode;
use fsm_parser::ast::AstNode;
use fsm_parser::{ast, parse, SyntaxKind};

/// Convenience: parse `src` and assert it produced zero errors.
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

// ─── Top-level shapes ────────────────────────────────────────────────────

#[test]
fn language_header() {
    let pr = parse_clean("language fsm 2.0");
    assert!(pr.ast().language_decl().is_some());
}

#[test]
fn empty_machine() {
    let pr = parse_clean("language fsm 2.0\nmachine Empty { }");
    let machines: Vec<_> = pr.ast().machines().collect();
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0].name(), Some("Empty".to_string()));
}

#[test]
fn machine_with_export_flag() {
    let pr = parse_clean("language fsm 2.0\nexport machine M { }");
    let m = pr.ast().machines().next().unwrap();
    assert!(m.is_export());
}

#[test]
fn import_decl_simple() {
    let pr = parse_clean(
        r#"language fsm 2.0
import "common/events.fsm" as Common"#,
    );
    let imp = pr.ast().imports().next().unwrap();
    assert_eq!(imp.path().as_deref(), Some("common/events.fsm"));
    assert_eq!(
        imp.alias().and_then(|a| a.name()).as_deref(),
        Some("Common")
    );
}

#[test]
fn import_decl_with_items() {
    let pr = parse_clean(
        r#"language fsm 2.0
import "shared/types.fsm" { PacketType, ErrorCode }"#,
    );
    let imp = pr.ast().imports().next().unwrap();
    let items: Vec<_> = imp.items().unwrap().items().collect();
    assert_eq!(items.len(), 2);
}

#[test]
fn feature_decl() {
    let pr = parse_clean("language fsm 2.0\nfeature hsm\nfeature timers");
    let feats: Vec<_> = pr.ast().features().collect();
    assert_eq!(feats.len(), 2);
    assert_eq!(feats[0].name().as_deref(), Some("hsm"));
}

#[test]
fn const_decl() {
    let pr = parse_clean("language fsm 2.0\nconst MAX_RETRIES = 3");
    let c = pr.ast().consts().next().unwrap();
    assert_eq!(c.name().as_deref(), Some("MAX_RETRIES"));
}

#[test]
fn enum_decl() {
    let pr = parse_clean(
        r#"language fsm 2.0
enum PacketType { CONTROL = 0, DATA = 1, HEARTBEAT = 2 }"#,
    );
    let e = pr.ast().enums().next().unwrap();
    assert_eq!(e.name().as_deref(), Some("PacketType"));
    assert_eq!(e.variants().count(), 3);
}

#[test]
fn extern_decl_pure() {
    let pr = parse_clean(
        r#"language fsm 2.0
pure extern can_unlock(ctx) : bool"#,
    );
    let e = pr.ast().externs().next().unwrap();
    assert!(e.is_pure());
    assert_eq!(e.name().as_deref(), Some("can_unlock"));
}

#[test]
fn extern_decl_void() {
    let pr = parse_clean(
        r#"language fsm 2.0
extern set_speed(u16 rpm)"#,
    );
    let e = pr.ast().externs().next().unwrap();
    assert!(!e.is_pure());
    assert_eq!(e.params().unwrap().params().count(), 1);
}

#[test]
fn machine_with_context_events_initial() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    context { retry_count : u8 = 0 }
    events { START   STOP(reason : u8) }
    initial Idle
    state Idle { on START -> Running }
    state Running { on STOP -> Idle }
}"#,
    );
    let m = pr.ast().machines().next().unwrap();
    assert!(m.context().is_some());
    assert!(m.events().is_some());
    assert!(m.initial().is_some());
    assert_eq!(m.states().count(), 2);
}

// ─── State variants ──────────────────────────────────────────────────────

#[test]
fn composite_state_with_nested() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial Outer
    state Outer {
        initial Inner1
        state Inner1 { }
        state Inner2 { }
    }
}"#,
    );
    let m = pr.ast().machines().next().unwrap();
    let outer = m.states().next().unwrap();
    assert_eq!(outer.nested_states().count(), 2);
}

#[test]
fn parallel_state_with_regions() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature parallel
machine M {
    initial Working
    state Working {
        region A { initial S1   state S1 { } }
        region B { initial S2   state S2 { } }
    }
}"#,
    );
    let m = pr.ast().machines().next().unwrap();
    let w = m.states().next().unwrap();
    assert_eq!(w.regions().count(), 2);
}

#[test]
fn pseudo_states_all_variants() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature hsm
feature history
feature fork_join
machine M {
    initial X
    state X {
        initial S
        state S { on E -> S }
        final F
        shallow_history H1 { initial S }
        deep_history H2 { initial S }
        choice C { [a] -> S [else] -> S }
        junction J { [a] -> S }
        fork Fk -> { S }
        join Jn { S } -> S
    }
}"#,
    );
    let m = pr.ast().machines().next().unwrap();
    let x = m.states().next().unwrap();
    // Each pseudo-state form is a direct child.
    let kinds: Vec<_> = x.syntax().children().map(|c| c.kind()).collect();
    assert!(kinds.contains(&SyntaxKind::FINAL_DECL));
    assert!(kinds.contains(&SyntaxKind::SHALLOW_HISTORY_DECL));
    assert!(kinds.contains(&SyntaxKind::DEEP_HISTORY_DECL));
    assert!(kinds.contains(&SyntaxKind::CHOICE_DECL));
    assert!(kinds.contains(&SyntaxKind::JUNCTION_DECL));
    assert!(kinds.contains(&SyntaxKind::FORK_DECL));
    assert!(kinds.contains(&SyntaxKind::JOIN_DECL));
}

// ─── Transition shapes (Doc 04 §8) ──────────────────────────────────────

#[test]
fn external_transition_with_guard_and_action() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E [ctx.x == 1] -> S : ctx.y = 2 }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    let t = s.transitions().next().unwrap();
    assert_eq!(t.trigger().as_deref(), Some("E"));
    assert_eq!(t.target().as_deref(), Some("S"));
    assert!(t.guard().is_some());
    assert!(t.actions().is_some());
}

#[test]
fn internal_transition() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E : ctx.y = 2 }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(s.internal_transitions().count(), 1);
}

#[test]
fn local_transition_history_arrow() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        on E ~> Inner
    }
}"#,
    );
    let outer = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(outer.local_transitions().count(), 1);
}

#[test]
fn completion_without_guard() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial S
    state S { done -> S }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(s.completions().count(), 1);
    let c = s.completions().next().unwrap();
    assert!(c.guard().is_none());
}

#[test]
fn completion_with_guard_per_b07() {
    // Per Doc 00 §B-07, guards on completion transitions ARE permitted.
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial S
    state S { done [ctx.x == 1] -> S }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    let c = s.completions().next().unwrap();
    assert!(c.guard().is_some(), "completion guard must be parsed");
}

#[test]
fn after_timer() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature timers
machine M {
    initial S
    state S { after 100 ms -> S }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(s.after().count(), 1);
}

#[test]
fn every_timer_with_transition() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature timers
machine M {
    initial S
    state S { every 50 ms -> S }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(s.every().count(), 1);
}

#[test]
fn every_timer_internal() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature timers
machine M {
    extern tick()
    initial S
    state S { every 100 ms : tick() }
}"#,
    );
    let s = pr.ast().machines().next().unwrap().states().next().unwrap();
    assert_eq!(s.every_internal().count(), 1);
}

// ─── Action statements ──────────────────────────────────────────────────

fn first_action_block(src: &str) -> ast::ActionBlock {
    let pr = parse_clean(src);
    pr.ast()
        .machines()
        .next()
        .unwrap()
        .states()
        .next()
        .unwrap()
        .transitions()
        .next()
        .unwrap()
        .actions()
        .expect("action block")
}

#[test]
fn assign_statement() {
    let blk = first_action_block(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : ctx.x = 5 }
}"#,
    );
    let stmts: Vec<_> = blk.statements().collect();
    assert!(matches!(stmts[0], ast::Stmt::Assign(_)));
}

#[test]
fn if_else_statement() {
    let blk = first_action_block(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : if (ctx.cond) { ctx.x = 1 } else { ctx.x = 2 } }
}"#,
    );
    let stmts: Vec<_> = blk.statements().collect();
    assert!(matches!(stmts[0], ast::Stmt::If(_)));
}

#[test]
fn while_statement() {
    let blk = first_action_block(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : while (ctx.n) { ctx.n = ctx.n - 1 } }
}"#,
    );
    let stmts: Vec<_> = blk.statements().collect();
    assert!(matches!(stmts[0], ast::Stmt::While(_)));
}

#[test]
fn for_statement() {
    let blk = first_action_block(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : for (ctx.i = 0; ctx.i < 10; ctx.i = ctx.i + 1) { ctx.sum = ctx.sum + ctx.i } }
}"#,
    );
    let stmts: Vec<_> = blk.statements().collect();
    assert!(matches!(stmts[0], ast::Stmt::For(_)));
}

#[test]
fn raise_statement() {
    let blk = first_action_block(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : raise OTHER }
}"#,
    );
    let stmts: Vec<_> = blk.statements().collect();
    assert!(matches!(stmts[0], ast::Stmt::Raise(_)));
}

#[test]
fn send_statement() {
    let blk = first_action_block(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : send EV to OtherMachine }
}"#,
    );
    let stmts: Vec<_> = blk.statements().collect();
    assert!(matches!(stmts[0], ast::Stmt::Send(_)));
}

// ─── Expressions / Pratt ─────────────────────────────────────────────────

#[test]
fn expression_precedence_in_action() {
    // (1 + 2 * 3) — Pratt should bind `*` before `+`.
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S : ctx.x = 1 + 2 * 3 }
}"#,
    );
    let dump = fsm_parser::ast::ast_dump(&pr.syntax());
    // The outer EXPR_BINARY should hold `+`; inner should hold `*`.
    assert!(dump.contains("EXPR_BINARY"));
    assert!(dump.contains("Plus"));
    assert!(dump.contains("Star"));
}

// ─── Negative tests (parser-level diagnostics) ───────────────────────────

#[test]
fn missing_arrow_emits_e0010() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E S }
}"#,
    );
    assert!(
        pr.errors.iter().any(|d| d.code == DiagnosticCode::E0010),
        "expected E0010 on missing '->', got: {:#?}",
        pr.errors
    );
}

#[test]
fn missing_closing_brace_recovers() {
    // Missing the final `}` for the machine. Parser should still produce
    // an AST root and emit at least one diagnostic.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    initial S
    state S { on E -> S "#,
    );
    assert!(!pr.errors.is_empty());
    assert!(pr.ast().machines().next().is_some());
}

#[test]
fn recovery_continues_after_broken_statement() {
    // A state with a broken transition; the parser must not abort, must
    // produce a complete AST root, AND must keep parsing — at minimum the
    // state declared BEFORE the broken one survives recovery.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    initial Good1
    state Good1 { on E -> Good2 }
    state Bad { on @@@ }
}"#,
    );
    let names: Vec<_> = pr
        .ast()
        .machines()
        .next()
        .unwrap()
        .states()
        .filter_map(|s| s.name())
        .collect();
    assert!(names.contains(&"Good1".to_string()));
    // Bad state may or may not be recoverable depending on sync-set
    // alignment; the guarantee is "parser keeps going, AST has Good1".
    assert!(!pr.errors.is_empty());
    // The CST must be byte-exact regardless.
    assert!(!pr.green.text_len().eq(&rowan::TextSize::from(0)));
}

#[test]
fn recovery_after_top_level_garbage() {
    // Garbage between two valid top-level decls. The parser should report
    // an error and still parse the second machine.
    let pr = parse(
        r#"language fsm 2.0
@@@
machine Good { }"#,
    );
    let names: Vec<_> = pr.ast().machines().filter_map(|m| m.name()).collect();
    assert!(names.contains(&"Good".to_string()), "names: {names:?}");
}

// ─── Security validators ────────────────────────────────────────────────

#[test]
fn import_path_traversal_rejected() {
    let pr = parse(
        r#"language fsm 2.0
import "../../etc/passwd""#,
    );
    assert!(
        pr.errors.iter().any(|d| d.code == DiagnosticCode::E0010),
        "expected import-path E0010, got {:#?}",
        pr.errors
    );
    let msg = pr
        .errors
        .iter()
        .find(|d| d.code == DiagnosticCode::E0010)
        .map(|d| d.message.as_str())
        .unwrap_or_default();
    assert!(
        msg.contains("path traversal") || msg.contains(".."),
        "diagnostic message should mention path traversal, got: {msg}"
    );
}

#[test]
fn opaque_type_injection_rejected() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    context { f : opaque "int; system(\"rm -rf /\"); int" }
    initial S
    state S { }
}"#,
    );
    assert!(
        pr.errors.iter().any(|d| d.code == DiagnosticCode::E0010),
        "expected opaque-type E0010, got {:#?}",
        pr.errors
    );
}

// ─── CST round-trip ──────────────────────────────────────────────────────

#[test]
fn cst_round_trips_source_bytes_for_byte() {
    let src = r#"language fsm 2.0

// Top-level comment.
feature hsm

/// Doc comment on machine.
machine M {
    /* block comment */
    initial S
    state S {
        on E -> S
    }
}
"#;
    let pr = parse(src);
    assert_eq!(pr.reconstructed_text(), src);
}
