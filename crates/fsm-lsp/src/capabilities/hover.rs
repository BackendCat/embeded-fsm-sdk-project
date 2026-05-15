//! `textDocument/hover` — Doc 26 §5 / §8 L3, GitHub-flavoured Markdown
//! per Doc 14 §5.
//!
//! ## Content source — verified seam (Doc 26 §5 `hover` row)
//!
//! Doc 26 §5 says hover reuses "`symbol_table` resolve_* + `Ir`
//! (transition counts, payload types, extern signatures, `@id`)". Verified
//! against the actual types:
//!
//! - The **kind / name** and the resolved declaration come from the shared
//!   [`crate::capabilities::resolve`] seam (the SAME `SymbolTable`
//!   resolution `definition` and `fsm check` use — hover can't disagree
//!   with a squiggle).
//! - The **structured detail** (event payload field *types*, extern
//!   signature with param/return *types*, context-field type + default
//!   value, a state's transitions-out count) is **not** in the
//!   `SymbolTable` — verified: `MachineSymbols` carries names + `Span`s and
//!   only a *stringised primitive* `context_field_types`; it has **no**
//!   payload schema, no extern param/return types, no transition
//!   structure. The lowered **`Ir`** is the sole in-tree source for those
//!   (`EventObject.payload`, `ExternObject.{pure,params,return_type}`,
//!   `ContextField.{ty,default}`, the per-state `transitions` vec). So
//!   hover threads `Analysis.ir` (the additive L3 field — same single
//!   analysis, no second lowering) and reads the IR for the detail. This
//!   matches Doc 26 §5's intent exactly; the row's shorthand is accurate
//!   here (unlike the L2 `StateEntry.shape` over-credit — §11.33(3)).
//!
//! ## The decl-site footer — a Doc-26/Doc-14 precision (no silent wrong)
//!
//! Doc 14 §5's state-hover example ends with `*Motor.fsm:12:3*`. The only
//! in-tree `line:col` for a decl is `fsm_ir::SourceLocation.{line,column}`,
//! which is produced by `fsm_analyzer::util::compute_line_col` — the
//! **byte-counted, 1-based** converter Doc 26 §4.1 explicitly names as
//! NON-LSP-correct (the §11.32 DRIFT-2 converter). Emitting that here would
//! print a subtly wrong column on any non-ASCII declaration line and would
//! disagree with the LSP's own ranges. Disciplined resolution: the footer
//! is **derived from L1's authoritative `LineIndex`** over the decl span
//! (the same converter every other LSP range uses), not from the IR's
//! `SourceLocation`. This meets Doc 14 §5's intent (show where the symbol
//! is declared) with the correct converter and keeps hover internally
//! consistent. Flagged precisely in Doc 00 §11.34 — it is a doc-vs-code
//! precision (Doc 14 §5's example implies the IR location is usable
//! verbatim; it is not LSP-correct), NOT a defect to overclaim.
//!
//! Whitespace / keyword / unknown symbol → `None` (no panic, no empty
//! tooltip — the spec-correct "no hover", Doc 26 §8 L3 negative case).

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Range};

use fsm_analyzer::symbol_table::{StateShape, SymbolTable};
use fsm_ir::{Ir, Literal, StateNode, Type};
use fsm_parser::cst::SyntaxNode;

use crate::capabilities::resolve::{resolve_at, Resolved};
use crate::position::{LineIndex, OffsetEncoding};

/// Build the hover for the identifier at `byte`, or `None` (cursor not on
/// a resolvable in-file symbol). `ir` is the threaded single-analysis IR
/// (`Analysis.ir`); `None` IR (catastrophic lowering failure) still yields
/// a name+kind hover from the symbol table — degraded, never absent or
/// wrong.
#[allow(clippy::too_many_arguments)]
pub fn hover(
    table: &SymbolTable,
    ir: Option<&Ir>,
    cst: &SyntaxNode,
    byte: u32,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<Hover> {
    let resolved = resolve_at(table, cst, byte)?;
    let decl_span = resolved.decl_span();
    let footer = decl_footer(&resolved, li, text, enc);
    let body = match &resolved {
        Resolved::State { machine, name, .. } => state_md(table, ir, *machine, name),
        Resolved::Event { machine, name, .. } => event_md(ir, *machine, name),
        Resolved::Extern {
            machine,
            name,
            is_pure,
            ..
        } => extern_md(ir, *machine, name, *is_pure),
        Resolved::ContextField { machine, name, .. } => context_field_md(ir, *machine, name),
        Resolved::Machine { name, .. } => format!("## machine `{name}`"),
    };
    let value = format!("{body}\n\n{footer}");
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        // Hover range = the resolved declaration's span projected through
        // the ONE authoritative LineIndex (so the client highlights the
        // hovered token's logical entity consistently with goto).
        range: Some(span_range(decl_span, li, text, enc)),
    })
}

fn span_range(
    span: fsm_diagnostics::Span,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Range {
    li.range(text, span, enc)
}

/// `*file:line:col*` derived from L1's authoritative `LineIndex` over the
/// decl span (NOT the IR's byte-1-based `SourceLocation` — see module
/// doc). Line/col are 1-based here (human-facing footer convention, Doc 14
/// §5 `*Motor.fsm:12:3*`), computed by adding 1 to the 0-based LSP
/// position the correct converter yields.
fn decl_footer(resolved: &Resolved, li: &LineIndex, text: &str, enc: OffsetEncoding) -> String {
    let span = resolved.decl_span();
    let pos = li.range(text, span, enc).start;
    format!("*{}:{}*", pos.line + 1, pos.character + 1)
}

// ---------------------------------------------------------------------------
// Per-kind Markdown. Structured content (Doc 14 §5) sourced from the IR.
// ---------------------------------------------------------------------------

/// Render an IR [`Type`] to its human form (the 4 IR type variants — total).
/// `Array`/`Enum`/`Opaque` are rendered faithfully so a `u8[16]` /
/// `opaque "T*"` payload is not silently flattened to a primitive.
fn ty(t: &Type) -> String {
    match t {
        Type::Primitive { name } => name.clone(),
        Type::Enum { enum_id } => enum_id.clone(),
        Type::Opaque { c_type } => format!("opaque \"{c_type}\""),
        Type::Array { element, size } => format!("{}[{size}]", ty(element)),
    }
}

fn literal(l: &Literal) -> String {
    match l {
        Literal::Int(i) => i.value.to_string(),
        Literal::Float(f) => f.value.to_string(),
        Literal::Bool(b) => b.value.to_string(),
        Literal::String(s) => format!("\"{}\"", s.value),
        Literal::EnumVariant(e) => format!("{}.{}", e.enum_name, e.variant_name),
    }
}

/// The IR machine for symbol-table index `mi` (the IR's machine order
/// mirrors the symbol table's — both come from the one analysis over the
/// same AST). Resolved by NAME to stay correct regardless of order.
fn ir_machine<'a>(
    ir: Option<&'a Ir>,
    table_name: Option<&str>,
) -> Option<&'a fsm_ir::MachineObject> {
    let ir = ir?;
    let name = table_name?;
    ir.machines.iter().find(|m| m.name == name)
}

fn state_md(table: &SymbolTable, ir: Option<&Ir>, mi: usize, name: &str) -> String {
    // Kind: derive composite/simple EXACTLY as L2's documentSymbol does
    // (the §11.33(3) derivation — `StateShape` alone cannot tell composite
    // from simple; the `container_path` graph does). Pseudo-state shapes
    // come straight off `StateShape`.
    let m = table.machines.get(mi);
    let shape = m
        .and_then(|m| {
            m.states
                .iter()
                .chain(m.regions.iter())
                .find(|s| s.name == name)
        })
        .map(|e| e.shape);
    let kind_word = match shape {
        Some(StateShape::Plain) | None => {
            // composite iff some state's container_path ends at this name.
            let is_composite = m
                .map(|m| {
                    let mut self_paths: Vec<Vec<String>> = Vec::new();
                    for s in m.states.iter().chain(m.regions.iter()) {
                        let mut p = s.container_path.clone();
                        p.push(s.name.clone());
                        self_paths.push(p);
                    }
                    // This state's own path = its container_path + name.
                    let me = m
                        .states
                        .iter()
                        .chain(m.regions.iter())
                        .find(|s| s.name == name);
                    if let Some(me) = me {
                        let mut my_path = me.container_path.clone();
                        my_path.push(me.name.clone());
                        m.states
                            .iter()
                            .chain(m.regions.iter())
                            .any(|s| s.container_path == my_path)
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            if is_composite {
                "composite"
            } else {
                "simple"
            }
        }
        Some(StateShape::Final) => "final",
        Some(StateShape::ShallowHistory) => "shallow history",
        Some(StateShape::DeepHistory) => "deep history",
        Some(StateShape::Choice) => "choice",
        Some(StateShape::Junction) => "junction",
        Some(StateShape::Fork) => "fork",
        Some(StateShape::Join) => "join",
        Some(StateShape::EntryPoint) => "entry point",
        Some(StateShape::ExitPoint) => "exit point",
    };
    let mut md = format!("## state `{name}` *({kind_word})*");

    // Transitions-out count + entry/exit actions from the IR (Doc 14 §5).
    if let Some(mo) = ir_machine(ir, table.machines.get(mi).map(|m| m.name.as_str())) {
        if let Some(sn) = find_state_node(&mo.root, name) {
            let (touts, has_entry, has_exit) = match sn {
                StateNode::Simple(s) => {
                    (s.transitions.len(), !s.entry.is_empty(), !s.exit.is_empty())
                }
                StateNode::Composite(c) => {
                    (c.transitions.len(), !c.entry.is_empty(), !c.exit.is_empty())
                }
                StateNode::Parallel(p) => {
                    (p.transitions.len(), !p.entry.is_empty(), !p.exit.is_empty())
                }
                _ => (0, false, false),
            };
            md.push_str(&format!("\n\n**Transitions out:** {touts}"));
            if has_entry {
                md.push_str("\n**Has entry actions:** yes");
            }
            if has_exit {
                md.push_str("\n**Has exit actions:** yes");
            }
        }
    }
    md
}

/// Depth-first search for the IR `StateNode` named `name` anywhere in the
/// machine's region tree (so a nested state's hover still finds its node).
fn find_state_node<'a>(region: &'a fsm_ir::RegionObject, name: &str) -> Option<&'a StateNode> {
    for sn in &region.states {
        if state_node_name(sn) == Some(name) {
            return Some(sn);
        }
        // Recurse into composites / parallels.
        let nested: &[fsm_ir::RegionObject] = match sn {
            StateNode::Composite(c) => &c.regions,
            StateNode::Parallel(p) => &p.regions,
            _ => &[],
        };
        for r in nested {
            if let Some(found) = find_state_node(r, name) {
                return Some(found);
            }
        }
    }
    None
}

fn state_node_name(sn: &StateNode) -> Option<&str> {
    match sn {
        StateNode::Simple(s) => Some(&s.name),
        StateNode::Composite(c) => Some(&c.name),
        StateNode::Parallel(p) => Some(&p.name),
        StateNode::Final(f) => Some(&f.name),
        StateNode::Submachine(s) => Some(&s.name),
        StateNode::EntryPoint(e) => Some(&e.name),
        StateNode::ExitPoint(e) => Some(&e.name),
        _ => None,
    }
}

fn event_md(ir: Option<&Ir>, _mi: usize, name: &str) -> String {
    let mut md = format!("## event `{name}`");
    // Payload field list — IR is the ONLY in-tree source (symbol_table has
    // no payload schema). Doc 14 §5 "Payload fields".
    let payload = ir.and_then(|ir| {
        ir.machines
            .iter()
            .flat_map(|m| m.events.iter())
            .find(|e| e.name == name)
            .map(|e| e.payload.clone())
    });
    match payload {
        Some(p) if !p.is_empty() => {
            md.push_str("\n\n**Payload fields:**");
            for field in &p {
                md.push_str(&format!("\n- `{}: {}`", field.name, ty(&field.ty)));
            }
        }
        Some(_) => md.push_str("\n\n*No payload.*"),
        None => {}
    }
    md
}

fn extern_md(ir: Option<&Ir>, _mi: usize, name: &str, is_pure: bool) -> String {
    let prefix = if is_pure { "`pure` extern" } else { "extern" };
    let mut md = format!("## {prefix} `{name}`");
    // Signature from the IR (params + return type). Doc 14 §5.
    if let Some(e) = ir.and_then(|ir| {
        ir.machines
            .iter()
            .flat_map(|m| m.externs.iter())
            .find(|e| e.name == name)
    }) {
        let params = e
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, ty(&p.ty)))
            .collect::<Vec<_>>()
            .join(", ");
        let ret = e
            .return_type
            .as_ref()
            .map(ty)
            .unwrap_or_else(|| "void".to_owned());
        md.push_str(&format!("\n\n**Signature:** `({params}) -> {ret}`"));
    }
    md
}

fn context_field_md(ir: Option<&Ir>, _mi: usize, name: &str) -> String {
    let mut md = format!("## context field `{name}`");
    // Type + default from the IR ContextField (richer + has the default
    // literal the stringised symbol_table type lacks). Doc 14 §5.
    if let Some(f) = ir.and_then(|ir| {
        ir.machines
            .iter()
            .flat_map(|m| m.context.fields.iter())
            .find(|f| f.name == name)
    }) {
        md.push_str(&format!("\n\n**Type:** `{}`", ty(&f.ty)));
        if let Some(d) = &f.default {
            md.push_str(&format!("\n**Default value:** `{}`", literal(d)));
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn setup(src: &str) -> (SymbolTable, Option<Ir>, SyntaxNode, LineIndex) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        let li = LineIndex::new(src);
        (a.symbol_table, a.ir, cst, li)
    }

    fn md_at(src: &str, needle: &str, plus: usize) -> Option<String> {
        let (t, ir, cst, li) = setup(src);
        let byte = (src.find(needle).unwrap() + plus) as u32;
        hover(&t, ir.as_ref(), &cst, byte, &li, src, OffsetEncoding::Utf8).map(|h| {
            match h.contents {
                HoverContents::Markup(m) => m.value,
                _ => panic!("expected markup"),
            }
        })
    }

    #[test]
    fn event_hover_lists_payload_fields_from_ir() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO(speed: u16, mode: u8) }\n  initial A\n  state A {\n    on GO -> A\n  }\n}\n";
        // Cursor on `GO` in `on GO`.
        let md = md_at(src, "on GO", 3).expect("event hover");
        assert!(md.contains("## event `GO`"), "got: {md}");
        assert!(md.contains("**Payload fields:**"), "got: {md}");
        assert!(md.contains("- `speed: u16`"), "got: {md}");
        assert!(md.contains("- `mode: u8`"), "got: {md}");
    }

    #[test]
    fn extern_hover_shows_signature_and_purity() {
        // Valid grammar: file-level `pure extern`, params `TYPE name`,
        // guard `[ok(1)]` BEFORE `->`.
        let src = "language fsm 2.0\n\npure extern ok(u8 x) : bool\n\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO [ok(1)] -> A\n  }\n}\n";
        let md = md_at(src, "ok(1)", 0).expect("extern hover");
        assert!(md.contains("## `pure` extern `ok`"), "got: {md}");
        assert!(md.contains("**Signature:** `(x: u8) -> bool`"), "got: {md}");
    }

    #[test]
    fn context_field_hover_shows_type_and_default() {
        // Valid grammar: guard `[ctx.count == 0]` BEFORE `->`.
        let src = "language fsm 2.0\nmachine M {\n  context { count: u16 = 7 }\n  events { GO }\n  initial A\n  state A {\n    on GO [ctx.count == 0] -> A\n  }\n}\n";
        let md = md_at(src, "ctx.count", 4).expect("ctx hover");
        assert!(md.contains("## context field `count`"), "got: {md}");
        assert!(md.contains("**Type:** `u16`"), "got: {md}");
        assert!(md.contains("**Default value:** `7`"), "got: {md}");
    }

    #[test]
    fn state_hover_shows_kind_and_transitions_out() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {}\n}\n";
        // Hover on the use-site `B` in `-> B` (resolves to the decl).
        let md = md_at(src, "-> B", 3).expect("state hover");
        assert!(md.contains("## state `B` *(simple)*"), "got: {md}");
        assert!(md.contains("**Transitions out:** 0"), "got: {md}");
        // Hover on `A` use-site (initial A) — A has 1 transition out.
        let md_a = md_at(src, "initial A", 8).expect("state A hover");
        assert!(md_a.contains("## state `A` *(simple)*"), "got: {md_a}");
        assert!(md_a.contains("**Transitions out:** 1"), "got: {md_a}");
    }

    #[test]
    fn hover_on_whitespace_is_none_no_panic() {
        let src = "language fsm 2.0\nmachine M {\n  initial A\n  state A {}\n}\n";
        assert!(
            md_at(src, "machine M", 7).is_none(),
            "whitespace -> no hover"
        );
        assert!(md_at(src, "state A", 1).is_none(), "keyword -> no hover");
    }
}
