//! Parser-layer acceptance for the v1.1-W4 `likely` / `rare` transition
//! branch-hint prefix (Doc 04 §8.8).
//!
//! Scope is the parse layer only: the optional `likely`/`rare` prefix on a
//! transition must (a) parse onto the right transition flavour, (b) be
//! readable via the typed `branch_hint()` accessor, (c) be `None` when
//! absent (the unhinted form is byte-identical to pre-W4), and (d) NOT
//! break back-compat — an FSM that uses `likely`/`rare` as an ordinary
//! identifier (state / event / extern name) elsewhere must still parse,
//! because the keywords are *contextual* (only special in transition-prefix
//! position).
//!
//! Every positive test here FAILS on `main` (`likely on …` →
//! `FSM-E0010 unexpected 'likely' in state body`) and PASSES after the W4
//! grammar lands, per FSM-PROC-SUBAGENT §5.1.

use fsm_parser::ast::BranchHint;
use fsm_parser::parse;

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

/// `likely on EVT -> T` / `rare on EVT -> T` parse and the hint is exposed
/// on the external-transition AST node.
#[test]
fn likely_and_rare_prefix_external_transition() {
    let pr = parse_clean(
        r#"language fsm 2.0
machine M {
    events { TICK FAULT STOP }
    initial Idle
    state Idle { on TICK -> Running }
    state Running {
        likely on TICK  -> Running
        rare   on FAULT -> Faulted
        on STOP -> Idle
    }
    state Faulted { on STOP -> Idle }
}"#,
    );

    let running = pr
        .ast()
        .machines()
        .next()
        .unwrap()
        .states()
        .find(|s| s.name().as_deref() == Some("Running"))
        .expect("Running state");

    let trs: Vec<_> = running.transitions().collect();
    assert_eq!(trs.len(), 3, "TICK + FAULT + STOP");

    // Order is document order.
    assert_eq!(trs[0].trigger().as_deref(), Some("TICK"));
    assert_eq!(trs[0].target().as_deref(), Some("Running"));
    assert_eq!(trs[0].branch_hint(), Some(BranchHint::Likely));

    assert_eq!(trs[1].trigger().as_deref(), Some("FAULT"));
    assert_eq!(trs[1].target().as_deref(), Some("Faulted"));
    assert_eq!(trs[1].branch_hint(), Some(BranchHint::Rare));

    // Unhinted transition → None (the trigger/target accessors are
    // unaffected — the hint, when present, nests in a BRANCH_HINT child
    // node, not a direct token).
    assert_eq!(trs[2].trigger().as_deref(), Some("STOP"));
    assert_eq!(trs[2].target().as_deref(), Some("Idle"));
    assert_eq!(trs[2].branch_hint(), None);
}

/// Absent prefix ⇒ `branch_hint() == None` for every transition flavour,
/// and trigger/target still resolve (no index shift).
#[test]
fn absent_prefix_is_none_and_accessors_unaffected() {
    let pr = parse_clean(
        r#"language fsm 2.0
extern noop()
machine M {
    events { GO }
    initial A
    state A { on GO -> B }
    state B {
        on GO ~> A
        on GO : noop()
        done -> A
    }
}"#,
    );
    let m = pr.ast().machines().next().unwrap();
    let a = m
        .states()
        .find(|s| s.name().as_deref() == Some("A"))
        .unwrap();
    let b = m
        .states()
        .find(|s| s.name().as_deref() == Some("B"))
        .unwrap();

    let ext = a.transitions().next().unwrap();
    assert_eq!(ext.trigger().as_deref(), Some("GO"));
    assert_eq!(ext.target().as_deref(), Some("B"));
    assert_eq!(ext.branch_hint(), None);

    let local = b.local_transitions().next().unwrap();
    assert_eq!(local.trigger().as_deref(), Some("GO"));
    assert_eq!(local.target().as_deref(), Some("A"));
    assert_eq!(local.branch_hint(), None);

    let internal = b.internal_transitions().next().unwrap();
    assert_eq!(internal.trigger().as_deref(), Some("GO"));
    assert_eq!(internal.branch_hint(), None);

    let completion = b.completions().next().unwrap();
    assert_eq!(completion.target().as_deref(), Some("A"));
    assert_eq!(completion.branch_hint(), None);
}

/// The hint is admitted on every trigger form: local (`~>`), internal
/// (`on E :`), completion (`done`), timed (`after`/`every`).
#[test]
fn hint_on_local_internal_completion_and_timed_forms() {
    let pr = parse_clean(
        r#"language fsm 2.0
feature timers
extern beep()
machine M {
    events { GO PING }
    initial A
    state A { on GO -> B }
    state B {
        likely on GO ~> A
        rare   on PING : beep()
        rare   done -> A
        likely after 1000 ms -> A
        rare   every 500 ms -> A
    }
}"#,
    );
    let b = pr
        .ast()
        .machines()
        .next()
        .unwrap()
        .states()
        .find(|s| s.name().as_deref() == Some("B"))
        .unwrap();

    assert_eq!(
        b.local_transitions().next().unwrap().branch_hint(),
        Some(BranchHint::Likely),
        "likely on a `~>` local transition"
    );
    assert_eq!(
        b.internal_transitions().next().unwrap().branch_hint(),
        Some(BranchHint::Rare),
        "rare on an internal `on E :` transition"
    );
    assert_eq!(
        b.completions().next().unwrap().branch_hint(),
        Some(BranchHint::Rare),
        "rare on a `done ->` completion"
    );
    assert_eq!(
        b.after().next().unwrap().branch_hint(),
        Some(BranchHint::Likely),
        "likely on an `after N ms ->` timed transition"
    );
    assert_eq!(
        b.every().next().unwrap().branch_hint(),
        Some(BranchHint::Rare),
        "rare on an `every N ms ->` timed transition"
    );
}

/// Back-compat: `likely` / `rare` used as ordinary identifiers (state name,
/// event name, extern name, target name) anywhere that is NOT a
/// transition-prefix position must STILL parse cleanly. This is the whole
/// reason the keywords are contextual rather than reserved — a pre-W4
/// `.fsm` that happens to use these words keeps working.
#[test]
fn likely_rare_as_plain_identifiers_still_parse() {
    let pr = parse_clean(
        r#"language fsm 2.0
pure extern likely() : bool
machine M {
    events { rare go }
    initial likely
    state likely {
        on rare [likely()] -> rare
    }
    state rare {
        on go -> likely
    }
}"#,
    );
    let m = pr.ast().machines().next().unwrap();
    // State named `likely` exists; its transition triggers on event `rare`
    // and targets state `rare` — none of these are hints.
    let st_likely = m
        .states()
        .find(|s| s.name().as_deref() == Some("likely"))
        .expect("state literally named `likely`");
    let t = st_likely.transitions().next().unwrap();
    assert_eq!(t.trigger().as_deref(), Some("rare"));
    assert_eq!(t.target().as_deref(), Some("rare"));
    assert_eq!(
        t.branch_hint(),
        None,
        "`on rare -> rare` — `rare` here is an event/state name, NOT a hint"
    );

    let st_rare = m
        .states()
        .find(|s| s.name().as_deref() == Some("rare"))
        .expect("state literally named `rare`");
    let t2 = st_rare.transitions().next().unwrap();
    assert_eq!(t2.trigger().as_deref(), Some("go"));
    assert_eq!(t2.target().as_deref(), Some("likely"));
    assert_eq!(t2.branch_hint(), None);
}

/// `likely`/`rare` immediately followed by another identifier (a state body
/// starting with a bare ident that is NOT a transition keyword) is still
/// the pre-W4 "unexpected ident" error — proving the contextual gate only
/// triggers the hint in genuine transition-prefix position and does not
/// silently swallow malformed input.
#[test]
fn likely_not_followed_by_transition_is_unexpected_ident_error() {
    let pr = parse(
        r#"language fsm 2.0
machine M {
    initial A
    state A { likely wibble }
}"#,
    );
    assert!(
        !pr.errors.is_empty(),
        "`likely wibble` (not a transition prefix) must still be a parse error"
    );
    let joined = format!("{:#?}", pr.errors);
    assert!(
        joined.contains("likely") || joined.contains("wibble") || joined.contains("state body"),
        "error should point at the unexpected token in the state body; got: {joined}"
    );
}
