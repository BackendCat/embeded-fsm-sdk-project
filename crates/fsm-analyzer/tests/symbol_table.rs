//! Symbol-table integration tests — duplicate / missing name detection.

use fsm_analyzer::{symbol_table::SymbolTable, DiagnosticCode};
use fsm_parser::parse;

fn build(src: &str) -> (SymbolTable, Vec<fsm_diagnostics::Diagnostic>) {
    SymbolTable::build(&parse(src).ast())
}

#[test]
fn machine_indices_are_built() {
    let (st, _) = build("language fsm 2.0\nmachine A { }\nmachine B { }");
    assert!(st.machine_index.contains_key("A"));
    assert!(st.machine_index.contains_key("B"));
    assert_eq!(st.machines.len(), 2);
}

#[test]
fn events_resolve_in_machine_scope() {
    let (st, diags) =
        build("language fsm 2.0\nmachine M { events { START } initial S state S { } }");
    assert!(diags.is_empty(), "expected clean build: {diags:#?}");
    let scope = fsm_analyzer::scope::Scope::for_machine(0);
    assert!(st.resolve_event("START", &scope).is_some());
    assert!(st.resolve_event("STOP", &scope).is_none());
}

#[test]
fn duplicate_field_reports_related_info() {
    let (_, diags) = build(
        "language fsm 2.0\nmachine M { context { x : u8 = 0\nx : u16 = 0 } initial S state S { } }",
    );
    let d = diags
        .iter()
        .find(|d| d.code == DiagnosticCode::E0023)
        .unwrap();
    assert!(
        !d.related.is_empty(),
        "expected related-info for duplicate field"
    );
}

#[test]
fn state_lookup_by_name() {
    let (st, _) =
        build("language fsm 2.0\nmachine M { initial Idle\nstate Idle { } state Running { } }");
    let scope = fsm_analyzer::scope::Scope::for_machine(0);
    assert!(st.resolve_state("Idle", &scope).is_some());
    assert!(st.resolve_state("Running", &scope).is_some());
    assert!(st.resolve_state("Ghost", &scope).is_none());
}

#[test]
fn extern_purity_recorded() {
    let (st, _) = build(
        "language fsm 2.0\nmachine M {\npure extern is_ready() : bool\nextern do_step()\ninitial S\nstate S { }\n}",
    );
    let scope = fsm_analyzer::scope::Scope::for_machine(0);
    let (_, pure) = st.resolve_extern("is_ready", &scope).unwrap();
    assert!(pure);
    let (_, pure) = st.resolve_extern("do_step", &scope).unwrap();
    assert!(!pure);
}
