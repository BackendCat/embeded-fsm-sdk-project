//! OPAQUE-BUG-1 — `opaque "C_type"` in extern param/return + context-field
//! position must round-trip into the IR's `Type::Opaque`, not be silently
//! dropped.
//!
//! The defect (pre-fix): `opaque "T *"` parses into an `OPAQUE_TYPE_REF`
//! node, but the analyzer's `Param::ty`/`FieldDecl::ty`/
//! `ExternDecl::return_type` accessors cast only to `ast::TypeRef`. They
//! returned `None` for opaque, and `lower_extern`/`lower_field`'s
//! `?`/`filter_map` then silently discarded the whole parameter, field, and
//! return type — a documented Doc 04 construct vanishing with NO diagnostic
//! (`fsm check` exit 0). That is a P0-1-class silent-data-loss: core
//! embedded HAL handle-passing produced a behaviourally wrong IR.
//!
//! FAIL-on-main proof (SUBAGENT_CONVENTIONS §5.1): on `main` the opaque
//! param/return/field are absent from the lowered IR — `set_handle` has
//! zero params, `is_ready` has no return type, the `dev` context field is
//! missing. Every assertion below fails there. After the fix the opaque
//! types are carried verbatim as `Type::Opaque { c_type }`.

use fsm_analyzer::analyze_with_source;
use fsm_ir::{MachineObject, Type};
use fsm_parser::parse;

const SRC: &str = r#"language fsm 2.0
extern set_handle(opaque "struct dev *" h)
pure extern is_ready(opaque "struct dev *" h) : opaque "struct dev *"
machine M {
  context { dev: opaque "struct dev *" = 0 }
  events { GO }
  initial S
  state S { on GO [is_ready(ctx.dev)] -> S : set_handle(ctx.dev) }
}
"#;

fn lower(src: &str) -> MachineObject {
    let pr = parse(src);
    assert!(
        pr.errors.is_empty(),
        "parse errors for fixture:\n{src}\n{:#?}",
        pr.errors
    );
    let res = analyze_with_source(&pr, "opaque.fsm", src);
    let errs: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
        .collect();
    assert!(
        errs.is_empty(),
        "analyzer errors for valid opaque fixture: {errs:?}"
    );
    res.ir.expect("ir produced").machines.remove(0)
}

#[test]
fn opaque_extern_param_round_trips_into_ir_not_dropped() {
    let m = lower(SRC);

    let set_handle = m
        .externs
        .iter()
        .find(|e| e.name == "set_handle")
        .expect("set_handle extern present");

    // The defect: param silently dropped → params.len() == 0 on main.
    assert_eq!(
        set_handle.params.len(),
        1,
        "opaque-typed extern param was dropped (silent data loss); params={:?}",
        set_handle.params
    );
    let p = &set_handle.params[0];
    assert_eq!(p.name, "h");
    assert_eq!(
        p.ty,
        Type::Opaque {
            c_type: "struct dev *".into()
        },
        "opaque param type must round-trip verbatim, got {:?}",
        p.ty
    );
}

#[test]
fn opaque_extern_return_type_round_trips_into_ir_not_dropped() {
    let m = lower(SRC);

    let is_ready = m
        .externs
        .iter()
        .find(|e| e.name == "is_ready")
        .expect("is_ready extern present");

    // The defect: `: opaque "..."` return silently lowered to None on main.
    assert_eq!(
        is_ready.return_type,
        Some(Type::Opaque {
            c_type: "struct dev *".into()
        }),
        "opaque extern return type must round-trip, got {:?}",
        is_ready.return_type
    );
}

#[test]
fn opaque_context_field_round_trips_into_ir_not_dropped() {
    let m = lower(SRC);

    // The defect: the whole `dev` field silently dropped on main, leaving
    // an empty context schema (and a dangling `ctx.dev` reference in the
    // emitted C).
    let dev = m
        .context
        .fields
        .iter()
        .find(|f| f.name == "dev")
        .expect("opaque context field `dev` must survive lowering");
    assert_eq!(
        dev.ty,
        Type::Opaque {
            c_type: "struct dev *".into()
        },
        "opaque context field type must round-trip, got {:?}",
        dev.ty
    );
}
