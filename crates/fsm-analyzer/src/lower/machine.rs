//! Machine-level lowering — `lower_machine` and the per-machine pieces
//! (context fields, events, externs, consts, queue, targets, types,
//! literals).
//!
//! AD-3 (2026-05-15): these were `LoweringCtx` methods. Each is a verbatim
//! transcription with the implicit `self` replaced by explicit
//! `&mut IdMinter` / `&LocCtx` parameters — same traversal order, same
//! `format!` templates, same counter sequence ⇒ byte-identical IR.

use fsm_ir::{
    BoolLit, ConstDecl as IrConstDecl, ContextField, ContextSchema, EnumVariantLit, EventObject,
    ExternObject, FeatureDecl as IrFeatureDecl, FloatLit, ImportDecl as IrImportDecl, IntLit,
    Literal, MachineObject, OverflowPolicy, Param, QueueConfig, RegionObject, StringLit,
    TargetConfig, Type,
};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-1 + R-3 leave-and-explain).** Retains
// `fsm_parser::cst` for: (R-1) `lower_type_ref_node` — the OPAQUE-BUG-1
// fix. The typed `Param::ty()`/`FieldDecl::ty()`/`ExternDecl::return_type()`
// cast only to `TypeRef` and *cannot see* an `OPAQUE_TYPE_REF` sibling, so
// they returned `None` and the param/field silently vanished with no
// diagnostic (a P0-1-class silent-data-loss bug — documented at
// `lower_type_ref_node`). Resolving from the *parent* node is *more
// correct* than the typed accessor; adding a `TypeOrOpaque` enum to
// `fsm_parser::ast` is defensible but is a **new public type on the most
// behaviourally-critical seam** in a non-behavioural wave — the
// conservative call is leave-and-explain (the option is recorded for a
// later dedicated parser-API wave; removing the coupling here would risk
// re-introducing the exact P0-1-class regression W0 is forbidden from
// causing — C-2); (R-3) `lower_literal`'s shallow-expr literal walks.
// Folding either worsens clarity / risks the P0-1 regression class for
// zero behaviour gain — Doc 00 §11.44/§11.49, the DRIFT-2 `LineIndex`
// precedent. Left-and-explained.
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::symbol_table::SymbolTable;
use crate::util::{file_const_table, parse_int_literal_i64};

use super::ids::IdMinter;
use super::loc::LocCtx;
use super::state::lower_state_children;

/// The accessors `lower_machine` needs, shared by `machine Name { … }` and
/// `submachine Name { … }` (Doc 04 §15). W2a deliberately reuses
/// `parse_machine_body` for both, so the two AST nodes expose an identical
/// surface — this trait lets one lowerer serve both without duplicating the
/// ~100-line body (and without risking byte-drift between two copies; the
/// snapshot guard pins the machine path).
pub(super) trait MachineLike {
    fn syntax(&self) -> &SyntaxNode;
    fn name(&self) -> Option<String>;
    fn stable_id_text(&self) -> Option<String>;
    fn context(&self) -> Option<ast::ContextBlock>;
    fn events(&self) -> Option<ast::EventsBlock>;
    fn externs(&self) -> ast::AstChildren<ast::ExternDecl>;
    fn queue(&self) -> Option<ast::QueueBlock>;
    fn target(&self) -> Option<ast::TargetBlock>;
}

impl MachineLike for ast::MachineDecl {
    fn syntax(&self) -> &SyntaxNode {
        AstNode::syntax(self)
    }
    fn name(&self) -> Option<String> {
        ast::MachineDecl::name(self)
    }
    fn stable_id_text(&self) -> Option<String> {
        self.stable_id().and_then(|s| s.id())
    }
    fn context(&self) -> Option<ast::ContextBlock> {
        ast::MachineDecl::context(self)
    }
    fn events(&self) -> Option<ast::EventsBlock> {
        ast::MachineDecl::events(self)
    }
    fn externs(&self) -> ast::AstChildren<ast::ExternDecl> {
        ast::MachineDecl::externs(self)
    }
    fn queue(&self) -> Option<ast::QueueBlock> {
        ast::MachineDecl::queue(self)
    }
    fn target(&self) -> Option<ast::TargetBlock> {
        ast::MachineDecl::target(self)
    }
}

impl MachineLike for ast::SubmachineDecl {
    fn syntax(&self) -> &SyntaxNode {
        AstNode::syntax(self)
    }
    fn name(&self) -> Option<String> {
        ast::SubmachineDecl::name(self)
    }
    fn stable_id_text(&self) -> Option<String> {
        self.stable_id().and_then(|s| s.id())
    }
    fn context(&self) -> Option<ast::ContextBlock> {
        ast::SubmachineDecl::context(self)
    }
    fn events(&self) -> Option<ast::EventsBlock> {
        ast::SubmachineDecl::events(self)
    }
    fn externs(&self) -> ast::AstChildren<ast::ExternDecl> {
        ast::SubmachineDecl::externs(self)
    }
    fn queue(&self) -> Option<ast::QueueBlock> {
        ast::SubmachineDecl::queue(self)
    }
    fn target(&self) -> Option<ast::TargetBlock> {
        ast::SubmachineDecl::target(self)
    }
}

pub(super) fn lower_machine<M: MachineLike>(
    machine: &M,
    m_idx: usize,
    st: &SymbolTable,
    file: &str,
    src: &str,
    ast_file: &ast::File,
) -> Option<MachineObject> {
    // Top-level machines carry the flat submachine-template set; a
    // submachine's *own* `submachines` stays empty (the IR models templates
    // as a flat per-machine list, not a recursive tree — recursing here
    // would not terminate for a submachine that references another).
    lower_machine_inner(machine, m_idx, st, file, src, ast_file, true)
}

#[allow(clippy::too_many_arguments)]
fn lower_machine_inner<M: MachineLike>(
    machine: &M,
    m_idx: usize,
    st: &SymbolTable,
    file: &str,
    src: &str,
    ast_file: &ast::File,
    include_submachines: bool,
) -> Option<MachineObject> {
    let name = machine
        .name()
        .unwrap_or_else(|| format!("__machine_{m_idx}"));
    let stable_id = machine
        .stable_id_text()
        .unwrap_or_else(|| format!("M:{name}"));
    // The ONE shared file-consts fold table (F-1 single source of truth).
    // `ast_file` is the whole-file AST (consts are file-level, mirrored
    // onto every machine — same convention as file-level externs/consts
    // below); the timer-duration fold resolves `after CONST ms` against
    // this exactly as `checks::timer` does, so check≡lower by construction.
    let file_consts = file_const_table(ast_file);
    let mut ids = IdMinter::new(&name, m_idx, file_consts);
    let locs = LocCtx::new(file, src);

    // Context fields.
    let context = if let Some(cb) = machine.context() {
        ContextSchema {
            fields: cb
                .fields()
                .filter_map(|f| lower_field(&mut ids, &locs, &f))
                .collect(),
        }
    } else {
        ContextSchema::default()
    };

    // Events.
    let events: Vec<EventObject> = match machine.events() {
        Some(eb) => eb
            .events()
            .filter_map(|e| lower_event(&mut ids, &locs, &e))
            .collect(),
        None => Vec::new(),
    };

    // Externs (machine-local + file-level mirrored onto every machine so
    // codegen / simulator have a single source of declared externs).
    let mut externs: Vec<ExternObject> = machine
        .externs()
        .filter_map(|e| lower_extern(&mut ids, &locs, &e))
        .collect();
    for ext in ast_file.externs() {
        if let Some(e) = lower_extern(&mut ids, &locs, &ext) {
            if !externs.iter().any(|x| x.name == e.name) {
                externs.push(e);
            }
        }
    }

    // Consts (file-level mirrored onto each machine for codegen convenience).
    let consts: Vec<IrConstDecl> = lower_consts_for_machine(&ids, &locs, st);

    // Queue config.
    let queue = lower_queue(&locs, machine.queue().as_ref());

    // Target blocks.
    let targets: Vec<TargetConfig> = lower_targets(&locs, machine);

    // Imports / features — file-level, mirrored on every machine.
    let imports = lower_imports();
    let features = lower_features();

    // Submachine templates (Doc 04 §15 / Doc 09 §3). Every top-level
    // `submachine Name { … }` is mirrored onto each machine (the same
    // convention as file-level consts/externs) so a `MachineObject` is a
    // self-contained codegen/simulator unit — `StateNode::Submachine`
    // resolves its `submachine_id` within the *same* machine's
    // `submachines`. A submachine lowers structurally exactly like a
    // machine (`SubmachineDecl: MachineLike`) — same root region, same
    // state-tree lowerer — recursively, so a submachine that itself
    // references another submachine still carries the full template set.
    let submachines: Vec<MachineObject> = if include_submachines {
        ast_file
            .submachines()
            .enumerate()
            .filter_map(|(sub_idx, sub)| {
                // Distinct m_idx space; `include_submachines: false` so the
                // recursion bottoms out (a submachine carries no nested
                // template set — see `lower_machine`).
                lower_machine_inner(&sub, m_idx + 1 + sub_idx, st, file, src, ast_file, false)
            })
            .collect()
    } else {
        Vec::new()
    };

    // Root region.
    //
    // Per Doc 09 §5, `region.initial` MUST be the ID of an Initial
    // pseudo-state node that lives inside `region.states` (Doc 09 §4.4).
    // `lower_state_children` emits that pseudo-state on the way through and
    // returns its id so we can wire it up here.
    let (root_states, root_initial_pseudo_id) =
        lower_state_children(&mut ids, &locs, machine.syntax());
    let root_loc = locs.loc(machine.syntax());
    let root = RegionObject {
        id: format!("r-{name}-root"),
        stable_id: None,
        name: format!("{name}__root"),
        initial: root_initial_pseudo_id.unwrap_or_default(),
        states: root_states,
        priority: 0,
        loc: root_loc.clone(),
    };

    Some(MachineObject {
        id: format!("m-{name}"),
        stable_id,
        name,
        context,
        events,
        externs,
        root,
        submachines,
        consts,
        imports,
        features,
        queue,
        targets,
        loc: locs.loc(machine.syntax()),
    })
}

// -- machine-level pieces ---------------------------------------------------

fn lower_field(ids: &mut IdMinter, locs: &LocCtx, f: &ast::FieldDecl) -> Option<ContextField> {
    let name = f.name()?;
    // OPAQUE-BUG-1: resolve from the FIELD_DECL node, not `f.ty()` (which
    // only casts to TYPE_REF and cannot see an OPAQUE_TYPE_REF sibling).
    let ty = lower_type_ref_node(f.syntax())?;
    let default = f
        .default()
        .and_then(|d| lower_literal(ids, locs, d.syntax()));
    Some(ContextField {
        id: format!("f-{}-{name}", ids.machine_name),
        name,
        ty,
        default,
        loc: locs.loc(f.syntax()),
    })
}

fn lower_event(ids: &mut IdMinter, locs: &LocCtx, e: &ast::EventDecl) -> Option<EventObject> {
    let name = e.name()?;
    let payload: Vec<Param> = if let Some(pl) = e.payload() {
        pl.fields()
            .filter_map(|f| {
                let n = f.name()?;
                let ty = lower_type_ref_node(f.syntax())?;
                Some(Param {
                    name: n,
                    ty,
                    id: None,
                    loc: Some(locs.loc(f.syntax())),
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    Some(EventObject {
        id: format!("ev-{}-{name}", ids.machine_name),
        stable_id: format!("M:{}:event:{name}", ids.machine_name),
        name,
        payload,
        loc: locs.loc(e.syntax()),
    })
}

fn lower_extern(ids: &mut IdMinter, locs: &LocCtx, e: &ast::ExternDecl) -> Option<ExternObject> {
    let name = e.name()?;
    let pure = e.is_pure();
    let params: Vec<Param> = if let Some(pl) = e.params() {
        pl.params()
            .filter_map(|p| {
                let n = p.name()?;
                // OPAQUE-BUG-1: resolve from the PARAM node so an
                // `opaque "T *"` parameter is not silently dropped.
                let ty = lower_type_ref_node(p.syntax())?;
                Some(Param {
                    name: n,
                    ty,
                    id: None,
                    loc: Some(locs.loc(p.syntax())),
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    // OPAQUE-BUG-1: resolve the return type from the EXTERN_DECL node so an
    // `: opaque "T"` return is carried through instead of silently dropped.
    let return_type = lower_type_ref_node(e.syntax());
    Some(ExternObject {
        id: format!("ex-{}-{name}", ids.machine_name),
        stable_id: format!("M:{}:extern:{name}", ids.machine_name),
        name,
        pure,
        params,
        return_type,
        loc: locs.loc(e.syntax()),
    })
}

fn lower_consts_for_machine(ids: &IdMinter, locs: &LocCtx, st: &SymbolTable) -> Vec<IrConstDecl> {
    // file-level consts only for now; per-machine consts are not in the
    // current grammar.
    st.file_consts
        .iter()
        .map(|entry| IrConstDecl {
            id: format!("c-{}-{}", ids.machine_name, entry.name),
            stable_id: format!("file:const:{}", entry.name),
            name: entry.name.clone(),
            ty: Type::Primitive { name: "i64".into() },
            value: Literal::Int(IntLit {
                value: 0,
                loc: None,
            }),
            loc: locs.loc_span(entry.span),
        })
        .collect()
}

fn lower_queue(locs: &LocCtx, qb: Option<&ast::QueueBlock>) -> QueueConfig {
    let Some(qb) = qb else {
        return QueueConfig::default();
    };
    // A present-but-key-omitting `queue {}` block falls back to the IR
    // canonical default (`QueueConfig::default()` — capacity 16 / Assert),
    // NOT a second hardcoded literal that could drift from it.
    let defaults = QueueConfig::default();
    let mut capacity = defaults.capacity;
    let mut overflow = defaults.overflow_policy;
    for entry in qb.entries() {
        let Some(key) = entry.key() else { continue };
        // F-2 fix (#110/F-1 silent-misconfig class): take the RHS value via
        // the single typed `ConfigEntry::value()` accessor, which skips the
        // key `Ident` and the `=`. The prior "first value-shaped token"
        // scan returned the *key* `Ident` (it is itself value-shaped), so
        // `capacity = 32` parsed `"capacity"` → `u32::from_str` failed →
        // the value silently fell back to the default 16, and
        // `overflow = drop_newest` matched the catch-all → `Assert`. A
        // user's explicit in-source queue config was silently miscompiled.
        match (key.as_str(), entry.value()) {
            ("capacity", Some(v)) => {
                if let Ok(n) = v.parse::<u32>() {
                    capacity = n;
                }
            }
            ("overflow", Some(v)) => {
                overflow = match v.as_str() {
                    "drop_oldest" | "drop-oldest" => OverflowPolicy::DropOldest,
                    "drop_newest" | "drop-newest" => OverflowPolicy::DropNewest,
                    "error" => OverflowPolicy::Error,
                    _ => OverflowPolicy::Assert,
                };
            }
            _ => {}
        }
    }
    QueueConfig {
        capacity,
        overflow_policy: overflow,
        loc: locs.loc(qb.syntax()),
    }
}

fn lower_targets<M: MachineLike>(locs: &LocCtx, machine: &M) -> Vec<TargetConfig> {
    // The grammar emits a single TARGET_BLOCK; we currently surface one.
    let Some(tb) = machine.target() else {
        return Vec::new();
    };
    let name = tb.name().unwrap_or_else(|| "default".to_string());
    let mut options = Vec::new();
    let mut profile = String::new();
    for entry in tb.entries() {
        let Some(key) = entry.key() else { continue };
        let value = entry
            .syntax()
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| {
                matches!(
                    t.kind(),
                    SyntaxKind::Ident
                        | SyntaxKind::IntLiteral
                        | SyntaxKind::KwTrue
                        | SyntaxKind::KwFalse
                )
            });
        if key == "profile" {
            if let Some(t) = value {
                profile = t.text().to_string();
            }
            continue;
        }
        if let Some(t) = value {
            let txt = t.text();
            let opt = if t.kind() == SyntaxKind::IntLiteral {
                fsm_ir::TargetOptionValue::Int(txt.parse().unwrap_or(0))
            } else if t.kind() == SyntaxKind::KwTrue {
                fsm_ir::TargetOptionValue::Bool(true)
            } else if t.kind() == SyntaxKind::KwFalse {
                fsm_ir::TargetOptionValue::Bool(false)
            } else {
                fsm_ir::TargetOptionValue::Ident(txt.to_string())
            };
            options.push((key, opt));
        }
    }
    if profile.is_empty() {
        profile = "c99".to_string();
    }
    vec![TargetConfig {
        name,
        profile,
        options,
        loc: locs.loc(tb.syntax()),
    }]
}

fn lower_imports() -> Vec<IrImportDecl> {
    Vec::new()
}

fn lower_features() -> Vec<IrFeatureDecl> {
    Vec::new()
}

// -- types ------------------------------------------------------------------

/// Lower the type-reference *child* of `parent` (a PARAM, FIELD_DECL,
/// EXTERN_DECL, EVENT payload field, or CAST_EXPR) to an IR [`Type`].
///
/// OPAQUE-BUG-1 fix: `opaque "C_type"` parses into a distinct
/// `OPAQUE_TYPE_REF` node, *not* a `TYPE_REF`. The typed accessors
/// (`Param::ty`, `FieldDecl::ty`, `ExternDecl::return_type`) cast only to
/// `ast::TypeRef`, so they returned `None` for an opaque type and the
/// caller's `?`/`filter_map` silently dropped the whole param/field/return
/// — a documented Doc 04 construct vanishing with no diagnostic (P0-1-class
/// silent data loss). Resolving from the *parent* node lets us see either
/// type-ref kind, so opaque types now round-trip into the IR's existing
/// [`Type::Opaque`] variant (already emitted verbatim by codegen).
pub(super) fn lower_type_ref_node(parent: &SyntaxNode) -> Option<Type> {
    let child = parent
        .children()
        .find(|n| matches!(n.kind(), SyntaxKind::TYPE_REF | SyntaxKind::OPAQUE_TYPE_REF))?;
    if child.kind() == SyntaxKind::OPAQUE_TYPE_REF {
        // The grammar already validated the body against the G-02 charset
        // at parse time; emit it verbatim.
        let c_type = ast::OpaqueTypeRef::cast(child)?.c_type()?;
        return Some(Type::Opaque { c_type });
    }
    let tok = child
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .next()?;
    Some(match tok.kind() {
        SyntaxKind::KwBool => Type::Primitive {
            name: "bool".into(),
        },
        SyntaxKind::KwU8 => Type::Primitive { name: "u8".into() },
        SyntaxKind::KwU16 => Type::Primitive { name: "u16".into() },
        SyntaxKind::KwU32 => Type::Primitive { name: "u32".into() },
        SyntaxKind::KwU64 => Type::Primitive { name: "u64".into() },
        SyntaxKind::KwI8 => Type::Primitive { name: "i8".into() },
        SyntaxKind::KwI16 => Type::Primitive { name: "i16".into() },
        SyntaxKind::KwI32 => Type::Primitive { name: "i32".into() },
        SyntaxKind::KwI64 => Type::Primitive { name: "i64".into() },
        SyntaxKind::KwF32 => Type::Primitive { name: "f32".into() },
        SyntaxKind::KwF64 => Type::Primitive { name: "f64".into() },
        SyntaxKind::Ident => Type::Enum {
            enum_id: tok.text().to_string(),
        },
        _ => return None,
    })
}

#[allow(clippy::only_used_in_recursion)]
pub(super) fn lower_literal(
    ids: &mut IdMinter,
    locs: &LocCtx,
    node: &SyntaxNode,
) -> Option<Literal> {
    // CONST_EXPR wraps the actual expression.
    let inner = if node.kind() == SyntaxKind::CONST_EXPR {
        node.children().next()?
    } else {
        node.clone()
    };
    match inner.kind() {
        SyntaxKind::EXPR_LITERAL => {
            for el in inner.children_with_tokens() {
                let Some(t) = el.into_token() else { continue };
                match t.kind() {
                    SyntaxKind::IntLiteral => {
                        let parsed = parse_int_literal_i64(t.text())?;
                        return Some(Literal::Int(IntLit {
                            value: parsed,
                            loc: None,
                        }));
                    }
                    SyntaxKind::FloatLiteral => {
                        let v: f64 = t.text().parse().ok()?;
                        return Some(Literal::Float(FloatLit {
                            value: v,
                            loc: None,
                        }));
                    }
                    SyntaxKind::StringLiteral => {
                        let raw = t.text();
                        let body = raw
                            .strip_prefix('"')
                            .and_then(|s| s.strip_suffix('"'))
                            .unwrap_or(raw);
                        return Some(Literal::String(StringLit {
                            value: body.to_string(),
                            loc: None,
                        }));
                    }
                    SyntaxKind::KwTrue => {
                        return Some(Literal::Bool(BoolLit {
                            value: true,
                            loc: None,
                        }));
                    }
                    SyntaxKind::KwFalse => {
                        return Some(Literal::Bool(BoolLit {
                            value: false,
                            loc: None,
                        }));
                    }
                    _ => {}
                }
            }
            None
        }
        SyntaxKind::EXPR_UNARY => {
            let op = inner
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::Minus | SyntaxKind::Plus))?;
            let inner_expr = inner.children().next()?;
            let v = lower_literal(ids, locs, &inner_expr)?;
            match (op.kind(), v) {
                (SyntaxKind::Minus, Literal::Int(IntLit { value, loc })) => {
                    Some(Literal::Int(IntLit { value: -value, loc }))
                }
                (SyntaxKind::Minus, Literal::Float(FloatLit { value, loc })) => {
                    Some(Literal::Float(FloatLit { value: -value, loc }))
                }
                (_, lit) => Some(lit),
            }
        }
        SyntaxKind::EXPR_PAREN => {
            let inner_expr = inner.children().next()?;
            lower_literal(ids, locs, &inner_expr)
        }
        SyntaxKind::EXPR_FIELD_REF => {
            // EnumName.Variant form.
            let idents: Vec<String> = inner
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .filter(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
                .collect();
            if idents.len() >= 2 {
                return Some(Literal::EnumVariant(EnumVariantLit {
                    enum_name: idents[0].clone(),
                    variant_name: idents[1].clone(),
                    loc: None,
                }));
            }
            None
        }
        _ => None,
    }
}
