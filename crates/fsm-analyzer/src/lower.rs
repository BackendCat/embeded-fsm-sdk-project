//! AST → IR lowering — the analyzer's main entry point.
//!
//! Per Doc 09 §1 design principle 5, the IR MAY be partial when errors are
//! present; we lower as much as we structurally can and surface diagnostics
//! cumulatively. The lowerer:
//!
//! - assigns deterministic IDs to every machine, state, region, event,
//!   extern, and transition;
//! - records `kind` on every transition (External / Local / Internal /
//!   Completion);
//! - resolves timer durations to a `u32` and rejects 0-duration entries via
//!   the [`crate::checks::timer`] pass before lowering;
//! - applies Doc 09 §1 defaults: `priority: 0`, `queue.capacity: 16`,
//!   `overflow: Assert`;
//! - computes `effective_lca` on each transition via the [`crate::lca`]
//!   helpers (the value is not stored in the IR — codegen recomputes it from
//!   `(source, target, kind)`).

use fsm_diagnostics::{Diagnostic, Span};
use fsm_ir::{
    BinaryOp, BoolLit, CastExpr, ChoiceBranch as IrChoiceBranch, ChoiceState, CmpOp,
    CompositeState, ConstDecl as IrConstDecl, ContextField, ContextSchema,
    DeferDecl as IrDeferDecl, DiagnosticObject, EnumVariantLit, EventObject, Expr as IrExpr,
    ExternObject, FeatureDecl as IrFeatureDecl, FieldRef, FinalState, FloatLit, ForkPseudo,
    GuardExpr, GuardOperand, HistoryKind, HistoryObject, ImportDecl as IrImportDecl, InitialPseudo,
    IntLit, Ir, JoinPseudo, JunctionState, Literal, MachineObject, OverflowPolicy, ParallelState,
    Param, QueueConfig, RegionObject, SimpleState, SourceLocation, StateNode, Statement, StringLit,
    TargetConfig, TimerKind, TimerObject, TransitionKind, TransitionObject, Trigger, Type, UnaryOp,
    CURRENT_IR_VERSION,
};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::{SyntaxKind, SyntaxNode};
use fsm_parser::ParseResult;

use crate::checks;
use crate::symbol_table::SymbolTable;
use crate::util::{compute_line_col, span_of};

/// Result of running the full analyzer pipeline.
#[derive(Clone, Debug)]
pub struct AnalysisResult {
    /// Lowered IR. `None` only on catastrophic errors that block any
    /// meaningful structural output; partial IR is preferred.
    pub ir: Option<Ir>,
    /// Cumulative diagnostics — analyzer + lowerer + parser diagnostics
    /// merged in source-emission order.
    pub diagnostics: Vec<Diagnostic>,
    /// Symbol table — exposed so downstream tools (LSP completion,
    /// documentation generators) can re-use it without rebuilding.
    pub symbol_table: SymbolTable,
}

/// Run the analyzer end-to-end. Parser-level diagnostics are preserved.
pub fn analyze(parse_result: &ParseResult) -> AnalysisResult {
    analyze_with_source(parse_result, "<source>", "")
}

/// Same as [`analyze`] but takes a source path and the raw source text so
/// `SourceLocation` values carry line/column data and the IR's `sourceHash`
/// is content-derived.
pub fn analyze_with_source(parse_result: &ParseResult, file: &str, src: &str) -> AnalysisResult {
    let mut diagnostics = parse_result.errors.clone();
    let ast = parse_result.ast();
    let (st, sym_diags) = SymbolTable::build(&ast);
    diagnostics.extend(sym_diags);

    let mut check_diags = Vec::new();
    checks::run_all(&ast, &st, &mut check_diags);
    diagnostics.extend(check_diags);

    let ir = lower_file(&ast, &st, file, src, &diagnostics);
    AnalysisResult {
        ir: Some(ir),
        diagnostics,
        symbol_table: st,
    }
}

// ---------------------------------------------------------------------------
// Top-level lowering
// ---------------------------------------------------------------------------

fn lower_file(
    file: &ast::File,
    st: &SymbolTable,
    file_path: &str,
    src: &str,
    diagnostics: &[Diagnostic],
) -> Ir {
    let mut machines = Vec::new();
    for (idx, machine) in file.machines().enumerate() {
        if let Some(m_ir) = lower_machine(&machine, idx, st, file_path, src, file) {
            machines.push(m_ir);
        }
    }
    Ir {
        ir_version: CURRENT_IR_VERSION.to_string(),
        source_hash: source_hash(src),
        source_files: vec![file_path.to_string()],
        machines,
        diagnostics: diagnostics.iter().cloned().map(DiagnosticObject).collect(),
    }
}

fn lower_machine(
    machine: &ast::MachineDecl,
    m_idx: usize,
    st: &SymbolTable,
    file: &str,
    src: &str,
    ast_file: &ast::File,
) -> Option<MachineObject> {
    let name = machine
        .name()
        .unwrap_or_else(|| format!("__machine_{m_idx}"));
    let stable_id = machine
        .stable_id()
        .and_then(|s| s.id())
        .unwrap_or_else(|| format!("M:{name}"));
    let mut ctx = LoweringCtx::new(file, src, &name, m_idx);

    // Context fields.
    let context = if let Some(cb) = machine.context() {
        ContextSchema {
            fields: cb.fields().filter_map(|f| ctx.lower_field(&f)).collect(),
        }
    } else {
        ContextSchema::default()
    };

    // Events.
    let events: Vec<EventObject> = match machine.events() {
        Some(eb) => eb.events().filter_map(|e| ctx.lower_event(&e)).collect(),
        None => Vec::new(),
    };

    // Externs (machine-local + file-level mirrored onto every machine so
    // codegen / simulator have a single source of declared externs).
    let mut externs: Vec<ExternObject> = machine
        .externs()
        .filter_map(|e| ctx.lower_extern(&e))
        .collect();
    for ext in ast_file.externs() {
        if let Some(e) = ctx.lower_extern(&ext) {
            if !externs.iter().any(|x| x.name == e.name) {
                externs.push(e);
            }
        }
    }

    // Consts (file-level mirrored onto each machine for codegen convenience).
    let consts: Vec<IrConstDecl> = ctx.lower_consts_for_machine(st);

    // Queue config.
    let queue = ctx.lower_queue(machine.queue().as_ref());

    // Target blocks.
    let targets: Vec<TargetConfig> = ctx.lower_targets(machine);

    // Imports / features — file-level, mirrored on every machine.
    let imports = ctx.lower_imports();
    let features = ctx.lower_features();

    // Root region.
    //
    // Per Doc 09 §5, `region.initial` MUST be the ID of an Initial
    // pseudo-state node that lives inside `region.states` (Doc 09 §4.4).
    // `lower_state_children` emits that pseudo-state on the way through and
    // returns its id so we can wire it up here.
    let (root_states, root_initial_pseudo_id) = ctx.lower_state_children(machine.syntax());
    let root_loc = ctx.loc(machine.syntax());
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
        submachines: Vec::new(),
        consts,
        imports,
        features,
        queue,
        targets,
        loc: ctx.loc(machine.syntax()),
    })
}

// ---------------------------------------------------------------------------
// Lowering context — owns counters and the per-machine ID factory.
// ---------------------------------------------------------------------------

struct LoweringCtx<'a> {
    file: &'a str,
    src: &'a str,
    machine_name: String,
    /// Reserved for future cross-machine resolution; carry the machine's
    /// position in the symbol table for submachine wiring.
    #[allow(dead_code)]
    m_idx: usize,
    /// Counter for auto-generated transition IDs.
    transition_counter: usize,
    /// Counter for auto-generated pseudo-state IDs.
    pseudo_counter: usize,
    /// Counter for fallback state IDs when the AST is incomplete.
    state_counter: usize,
}

impl<'a> LoweringCtx<'a> {
    fn new(file: &'a str, src: &'a str, machine_name: &str, m_idx: usize) -> Self {
        Self {
            file,
            src,
            machine_name: machine_name.to_string(),
            m_idx,
            transition_counter: 0,
            pseudo_counter: 0,
            state_counter: 0,
        }
    }

    fn loc(&self, n: &SyntaxNode) -> SourceLocation {
        let span = span_of(n);
        let (line, column) = compute_line_col(self.src, span.start);
        SourceLocation::new(self.file.to_string(), span, line, column)
    }

    fn loc_span(&self, span: Span) -> SourceLocation {
        let (line, column) = compute_line_col(self.src, span.start);
        SourceLocation::new(self.file.to_string(), span, line, column)
    }

    fn next_transition_id(&mut self) -> String {
        let id = format!("t-{}-{}", self.machine_name, self.transition_counter);
        self.transition_counter += 1;
        id
    }

    fn next_pseudo_id(&mut self, kind: &str) -> String {
        let id = format!("ps-{}-{}-{}", kind, self.machine_name, self.pseudo_counter);
        self.pseudo_counter += 1;
        id
    }

    fn state_id(&mut self, name: &str) -> String {
        if name.is_empty() {
            let id = format!("s-{}-anon-{}", self.machine_name, self.state_counter);
            self.state_counter += 1;
            id
        } else {
            format!("s-{}-{}", self.machine_name, name)
        }
    }

    // -- machine-level pieces -------------------------------------------------

    fn lower_field(&mut self, f: &ast::FieldDecl) -> Option<ContextField> {
        let name = f.name()?;
        let ty = self.lower_type_ref(f.ty().as_ref())?;
        let default = f.default().and_then(|d| self.lower_literal(d.syntax()));
        Some(ContextField {
            id: format!("f-{}-{name}", self.machine_name),
            name,
            ty,
            default,
            loc: self.loc(f.syntax()),
        })
    }

    fn lower_event(&mut self, e: &ast::EventDecl) -> Option<EventObject> {
        let name = e.name()?;
        let payload: Vec<Param> = if let Some(pl) = e.payload() {
            pl.fields()
                .filter_map(|f| {
                    let n = f.name()?;
                    let ty = self.lower_type_ref(f.ty().as_ref())?;
                    Some(Param {
                        name: n,
                        ty,
                        id: None,
                        loc: Some(self.loc(f.syntax())),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        Some(EventObject {
            id: format!("ev-{}-{name}", self.machine_name),
            stable_id: format!("M:{}:event:{name}", self.machine_name),
            name,
            payload,
            loc: self.loc(e.syntax()),
        })
    }

    fn lower_extern(&mut self, e: &ast::ExternDecl) -> Option<ExternObject> {
        let name = e.name()?;
        let pure = e.is_pure();
        let params: Vec<Param> = if let Some(pl) = e.params() {
            pl.params()
                .filter_map(|p| {
                    let n = p.name()?;
                    let ty = self.lower_type_ref(p.ty().as_ref())?;
                    Some(Param {
                        name: n,
                        ty,
                        id: None,
                        loc: Some(self.loc(p.syntax())),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let return_type = e.return_type().and_then(|t| self.lower_type_ref(Some(&t)));
        Some(ExternObject {
            id: format!("ex-{}-{name}", self.machine_name),
            stable_id: format!("M:{}:extern:{name}", self.machine_name),
            name,
            pure,
            params,
            return_type,
            loc: self.loc(e.syntax()),
        })
    }

    fn lower_consts_for_machine(&mut self, st: &SymbolTable) -> Vec<IrConstDecl> {
        // file-level consts only for now; per-machine consts are not in the
        // current grammar.
        st.file_consts
            .iter()
            .map(|entry| IrConstDecl {
                id: format!("c-{}-{}", self.machine_name, entry.name),
                stable_id: format!("file:const:{}", entry.name),
                name: entry.name.clone(),
                ty: Type::Primitive { name: "i64".into() },
                value: Literal::Int(IntLit {
                    value: 0,
                    loc: None,
                }),
                loc: self.loc_span(entry.span),
            })
            .collect()
    }

    fn lower_queue(&mut self, qb: Option<&ast::QueueBlock>) -> QueueConfig {
        let Some(qb) = qb else {
            return QueueConfig::default();
        };
        let mut capacity = 16u32;
        let mut overflow = OverflowPolicy::Assert;
        for entry in qb.entries() {
            let Some(key) = entry.key() else { continue };
            // Value is the first IntLiteral / Ident token after the `=`.
            let val_tok = entry
                .syntax()
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| {
                    matches!(
                        t.kind(),
                        SyntaxKind::IntLiteral
                            | SyntaxKind::Ident
                            | SyntaxKind::KwTrue
                            | SyntaxKind::KwFalse
                    )
                });
            match (key.as_str(), val_tok.as_ref().map(|t| t.text().to_string())) {
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
            loc: self.loc(qb.syntax()),
        }
    }

    fn lower_targets(&mut self, machine: &ast::MachineDecl) -> Vec<TargetConfig> {
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
            loc: self.loc(tb.syntax()),
        }]
    }

    fn lower_imports(&mut self) -> Vec<IrImportDecl> {
        Vec::new()
    }

    fn lower_features(&mut self) -> Vec<IrFeatureDecl> {
        Vec::new()
    }

    // -- types ----------------------------------------------------------------

    fn lower_type_ref(&self, ty: Option<&ast::TypeRef>) -> Option<Type> {
        let ty = ty?;
        // Type variations: a primitive keyword token, an opaque-string, or an
        // identifier (enum or named type).
        let tok = ty
            .syntax()
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
    fn lower_literal(&self, node: &SyntaxNode) -> Option<Literal> {
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
                            let v: String = t.text().chars().filter(|c| *c != '_').collect();
                            let parsed = if let Some(rest) =
                                v.strip_prefix("0x").or_else(|| v.strip_prefix("0X"))
                            {
                                i64::from_str_radix(rest, 16).ok()?
                            } else if let Some(rest) =
                                v.strip_prefix("0b").or_else(|| v.strip_prefix("0B"))
                            {
                                i64::from_str_radix(rest, 2).ok()?
                            } else {
                                v.parse().ok()?
                            };
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
                let v = self.lower_literal(&inner_expr)?;
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
                self.lower_literal(&inner_expr)
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

    // -- state lowering ------------------------------------------------------

    /// Lower the direct children of `parent` (a machine or a state) into a
    /// list of [`StateNode`]s. Returns the produced state list and the
    /// pseudo-state ID of the **first** Initial pseudo-state emitted (so the
    /// caller can plumb it into [`RegionObject::initial`] per Doc 09 §5).
    /// `None` when the source declared no `initial` keyword in this scope.
    fn lower_state_children(&mut self, parent: &SyntaxNode) -> (Vec<StateNode>, Option<String>) {
        let mut out = Vec::new();
        let mut initial_pseudo_id: Option<String> = None;
        // INITIAL_DECL becomes an InitialPseudo state. Its `target` MUST be a
        // state ID per Doc 09 §4.4, so we translate the AST-level name
        // ("Idle") into the canonical state ID form ("s-<machine>-Idle")
        // produced by [`state_target_id`] — the same form transitions use.
        for child in parent.children() {
            match child.kind() {
                SyntaxKind::INITIAL_DECL => {
                    if let Some(init) = ast::InitialDecl::cast(child.clone()) {
                        let target_name = init.target().unwrap_or_default();
                        let target_id = state_target_id(self, &target_name);
                        let id = self.next_pseudo_id("initial");
                        if initial_pseudo_id.is_none() {
                            initial_pseudo_id = Some(id.clone());
                        }
                        out.push(StateNode::Initial(InitialPseudo {
                            id,
                            target: target_id,
                            loc: self.loc(&child),
                        }));
                    }
                }
                SyntaxKind::STATE_DECL => {
                    if let Some(state) = ast::StateDecl::cast(child.clone()) {
                        out.push(self.lower_state(&state));
                    }
                }
                SyntaxKind::REGION_DECL => {
                    // top-level region: the parent must be a parallel state —
                    // we surface it as a ParallelState with one region. The
                    // surrounding caller orchestrates multi-region grouping.
                    // For lowering simplicity, regions inside a state become
                    // children of a `ParallelState` (see lower_state).
                }
                SyntaxKind::FINAL_DECL => {
                    if let Some(f) = ast::FinalDecl::cast(child.clone()) {
                        let name = f.name().unwrap_or_default();
                        out.push(StateNode::Final(FinalState {
                            id: self.state_id(&name),
                            stable_id: format!("M:{}:final:{name}", self.machine_name),
                            name: name.clone(),
                            loc: self.loc(&child),
                        }));
                    }
                }
                SyntaxKind::SHALLOW_HISTORY_DECL => {
                    out.push(self.lower_history(&child, HistoryKind::Shallow));
                }
                SyntaxKind::DEEP_HISTORY_DECL => {
                    out.push(self.lower_history(&child, HistoryKind::Deep));
                }
                SyntaxKind::CHOICE_DECL => out.push(self.lower_choice(&child)),
                SyntaxKind::JUNCTION_DECL => out.push(self.lower_junction(&child)),
                SyntaxKind::FORK_DECL => out.push(self.lower_fork(&child)),
                SyntaxKind::JOIN_DECL => out.push(self.lower_join(&child)),
                _ => {}
            }
        }
        (out, initial_pseudo_id)
    }

    fn lower_state(&mut self, state: &ast::StateDecl) -> StateNode {
        let name = state.name().unwrap_or_default();
        let id = self.state_id(&name);
        let stable_id = format!("M:{}:state:{name}", self.machine_name);

        let entry = state
            .entry()
            .and_then(|e| e.action_block())
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        let exit = state
            .exit()
            .and_then(|e| e.action_block())
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        let mut transitions = self.lower_transitions(state, &id, &name);
        let (timers, timer_transitions) = self.lower_timers(state, &id);
        transitions.extend(timer_transitions);
        let defers = self.lower_defers(state);

        let regions: Vec<_> = state.regions().collect();
        let nested_states: Vec<_> = state.nested_states().collect();

        if !regions.is_empty() {
            // Parallel or composite-with-regions: build region objects from
            // each REGION_DECL child.
            let region_objs: Vec<RegionObject> =
                regions.iter().map(|r| self.lower_region(r)).collect();
            if region_objs.len() >= 2 {
                return StateNode::Parallel(ParallelState {
                    id,
                    stable_id,
                    name: name.clone(),
                    entry,
                    exit,
                    transitions,
                    timers,
                    defers,
                    regions: region_objs,
                    loc: self.loc(state.syntax()),
                });
            }
            return StateNode::Composite(CompositeState {
                id,
                stable_id,
                name: name.clone(),
                entry,
                exit,
                transitions,
                timers,
                defers,
                regions: region_objs,
                history: None,
                loc: self.loc(state.syntax()),
            });
        }
        if !nested_states.is_empty() {
            // Composite with implicit region — collect children into one
            // synthetic region. Doc 09 §5 requires `region.initial` to be the
            // ID of an Initial pseudo-state; `lower_state_children` emits it
            // and returns its id.
            let (states, inner_initial_pseudo_id) = self.lower_state_children(state.syntax());
            let region = RegionObject {
                id: format!("r-{}-{name}", self.machine_name),
                stable_id: None,
                name: format!("{name}__r"),
                initial: inner_initial_pseudo_id.unwrap_or_default(),
                states,
                priority: 0,
                loc: self.loc(state.syntax()),
            };
            return StateNode::Composite(CompositeState {
                id,
                stable_id,
                name: name.clone(),
                entry,
                exit,
                transitions,
                timers,
                defers,
                regions: vec![region],
                history: extract_history(state, self),
                loc: self.loc(state.syntax()),
            });
        }
        StateNode::Simple(SimpleState {
            id,
            stable_id,
            name,
            entry,
            exit,
            transitions,
            timers,
            defers,
            loc: self.loc(state.syntax()),
        })
    }

    fn lower_region(&mut self, r: &ast::RegionDecl) -> RegionObject {
        let name = r
            .name()
            .unwrap_or_else(|| format!("__region_{}", self.pseudo_counter));
        // `region.initial` holds the ID of the Initial pseudo-state (Doc 09
        // §5). `lower_state_children` emits it inside `states` and gives us
        // back its id.
        let (states, initial_pseudo_id) = self.lower_state_children(r.syntax());
        RegionObject {
            id: format!("r-{}-{name}", self.machine_name),
            stable_id: None,
            name,
            initial: initial_pseudo_id.unwrap_or_default(),
            states,
            priority: 0,
            loc: self.loc(r.syntax()),
        }
    }

    fn lower_history(&mut self, node: &SyntaxNode, kind: HistoryKind) -> StateNode {
        let (name, default) = match kind {
            HistoryKind::Shallow => {
                let h = ast::ShallowHistoryDecl::cast(node.clone());
                (
                    h.as_ref().and_then(|n| n.name()).unwrap_or_default(),
                    h.and_then(|n| n.default()).and_then(|d| d.target()),
                )
            }
            HistoryKind::Deep => {
                let h = ast::DeepHistoryDecl::cast(node.clone());
                (
                    h.as_ref().and_then(|n| n.name()).unwrap_or_default(),
                    h.and_then(|n| n.default()).and_then(|d| d.target()),
                )
            }
        };
        // Missing default is rejected by [`crate::checks::history`] — we
        // still produce an IR entry with empty default_target so codegen
        // never sees `Option::None` (matching the type-system embodiment of
        // B-14). Normalize the raw `default` name to the canonical state IR
        // id so consumers (codegen, simulator) can look it up via the same
        // `s-<Machine>-<Name>` key family used for all other transition
        // targets.
        let default_target = default.map(|n| self.state_id(&n)).unwrap_or_default();
        StateNode::History(HistoryObject {
            id: self.state_id(&name),
            stable_id: format!("M:{}:history:{name}", self.machine_name),
            history_kind: kind,
            default_target,
            loc: self.loc(node),
        })
    }

    fn lower_choice(&mut self, node: &SyntaxNode) -> StateNode {
        let c = ast::ChoiceDecl::cast(node.clone());
        let name = c.as_ref().and_then(|n| n.name()).unwrap_or_default();
        let mut branches = Vec::new();
        if let Some(c) = c {
            for branch in c.branches() {
                let target = branch.target().unwrap_or_default();
                branches.push(IrChoiceBranch {
                    guard: GuardExpr::Else,
                    target,
                    actions: Vec::new(),
                    loc: self.loc(branch.syntax()),
                });
            }
        }
        StateNode::Choice(ChoiceState {
            id: self.state_id(&name),
            stable_id: format!("M:{}:choice:{name}", self.machine_name),
            branches,
            loc: self.loc(node),
        })
    }

    fn lower_junction(&mut self, node: &SyntaxNode) -> StateNode {
        let j = ast::JunctionDecl::cast(node.clone());
        let name = j.as_ref().and_then(|n| n.name()).unwrap_or_default();
        let mut branches = Vec::new();
        if let Some(j) = j {
            for branch in j.branches() {
                let target = branch.target().unwrap_or_default();
                branches.push(IrChoiceBranch {
                    guard: GuardExpr::Else,
                    target,
                    actions: Vec::new(),
                    loc: self.loc(branch.syntax()),
                });
            }
        }
        StateNode::Junction(JunctionState {
            id: self.state_id(&name),
            stable_id: format!("M:{}:junction:{name}", self.machine_name),
            branches,
            loc: self.loc(node),
        })
    }

    fn lower_fork(&mut self, node: &SyntaxNode) -> StateNode {
        let f = ast::ForkDecl::cast(node.clone());
        let name = f.as_ref().and_then(|n| n.name()).unwrap_or_default();
        let targets = f
            .and_then(|n| n.targets())
            .map(|t| t.names())
            .unwrap_or_default();
        StateNode::Fork(ForkPseudo {
            id: self.state_id(&name),
            stable_id: format!("M:{}:fork:{name}", self.machine_name),
            targets,
            loc: self.loc(node),
        })
    }

    fn lower_join(&mut self, node: &SyntaxNode) -> StateNode {
        let j = ast::JoinDecl::cast(node.clone());
        let name = j.as_ref().and_then(|n| n.name()).unwrap_or_default();
        let sources = j
            .as_ref()
            .and_then(|n| n.sources())
            .map(|t| t.names())
            .unwrap_or_default();
        let target = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| t.kind() == SyntaxKind::Ident)
            .last()
            .map(|t| t.text().to_string())
            .unwrap_or_default();
        StateNode::Join(JoinPseudo {
            id: self.state_id(&name),
            stable_id: format!("M:{}:join:{name}", self.machine_name),
            sources,
            target,
            actions: Vec::new(),
            loc: self.loc(node),
        })
    }

    fn lower_transitions(
        &mut self,
        state: &ast::StateDecl,
        source_id: &str,
        source_name: &str,
    ) -> Vec<TransitionObject> {
        let mut out = Vec::new();
        for t in state.transitions() {
            if let Some(tr) = self.lower_external(&t, source_id, source_name) {
                out.push(tr);
            }
        }
        for t in state.internal_transitions() {
            if let Some(tr) = self.lower_internal(&t, source_id) {
                out.push(tr);
            }
        }
        for t in state.local_transitions() {
            if let Some(tr) = self.lower_local(&t, source_id) {
                out.push(tr);
            }
        }
        for c in state.completions() {
            if let Some(tr) = self.lower_completion(&c, source_id) {
                out.push(tr);
            }
        }
        out
    }

    fn lower_external(
        &mut self,
        t: &ast::TransitionDecl,
        source_id: &str,
        source_name: &str,
    ) -> Option<TransitionObject> {
        let trigger_name = t.trigger()?;
        let payload_binding = extract_trigger_payload_binding(t.syntax());
        let target_name = t.target().unwrap_or_default();
        let priority = t
            .priority()
            .and_then(|p| extract_priority(p.syntax()))
            .unwrap_or(0);
        let guard = t.guard().map(|g| self.lower_guard_clause(&g));
        let actions = t
            .actions()
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        Some(self.build_transition(
            source_id,
            &state_target_id(self, &target_name),
            TransitionKind::External,
            Some(Trigger::Event {
                event_id: format!("ev-{}-{trigger_name}", self.machine_name),
                payload_binding,
            }),
            priority,
            guard,
            actions,
            t.syntax(),
            source_name,
        ))
    }

    fn lower_internal(
        &mut self,
        t: &ast::InternalDecl,
        source_id: &str,
    ) -> Option<TransitionObject> {
        let trigger_name = t.trigger()?;
        let payload_binding = extract_trigger_payload_binding(t.syntax());
        let priority = t
            .priority()
            .and_then(|p| extract_priority(p.syntax()))
            .unwrap_or(0);
        let guard = t.guard().map(|g| self.lower_guard_clause(&g));
        let actions = t
            .actions()
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        Some(self.build_transition(
            source_id,
            source_id,
            TransitionKind::Internal,
            Some(Trigger::Event {
                event_id: format!("ev-{}-{trigger_name}", self.machine_name),
                payload_binding,
            }),
            priority,
            guard,
            actions,
            t.syntax(),
            "",
        ))
    }

    fn lower_local(&mut self, t: &ast::LocalDecl, source_id: &str) -> Option<TransitionObject> {
        let trigger_name = t.trigger()?;
        let payload_binding = extract_trigger_payload_binding(t.syntax());
        let target_name = t.target().unwrap_or_default();
        let priority = t
            .priority()
            .and_then(|p| extract_priority(p.syntax()))
            .unwrap_or(0);
        let guard = t.guard().map(|g| self.lower_guard_clause(&g));
        let actions = t
            .actions()
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        Some(self.build_transition(
            source_id,
            &state_target_id(self, &target_name),
            TransitionKind::Local,
            Some(Trigger::Event {
                event_id: format!("ev-{}-{trigger_name}", self.machine_name),
                payload_binding,
            }),
            priority,
            guard,
            actions,
            t.syntax(),
            "",
        ))
    }

    fn lower_completion(
        &mut self,
        c: &ast::CompletionDecl,
        source_id: &str,
    ) -> Option<TransitionObject> {
        let target_name = c.target().unwrap_or_default();
        let priority = c
            .priority()
            .and_then(|p| extract_priority(p.syntax()))
            .unwrap_or(0);
        let guard = c.guard().map(|g| self.lower_guard_clause(&g));
        let actions = c
            .actions()
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        Some(self.build_transition(
            source_id,
            &state_target_id(self, &target_name),
            TransitionKind::Completion,
            None,
            priority,
            guard,
            actions,
            c.syntax(),
            "",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_transition(
        &mut self,
        source_id: &str,
        target_id: &str,
        kind: TransitionKind,
        trigger: Option<Trigger>,
        priority: i64,
        guard: Option<GuardExpr>,
        actions: Vec<Statement>,
        node: &SyntaxNode,
        _source_name: &str,
    ) -> TransitionObject {
        let id = self.next_transition_id();
        #[allow(deprecated)]
        TransitionObject {
            id: id.clone(),
            stable_id: format!("M:{}:transition:{id}", self.machine_name),
            source: source_id.to_string(),
            target: target_id.to_string(),
            trigger,
            guard,
            actions,
            priority: priority.clamp(0, u16::MAX as i64) as u16,
            kind,
            internal: matches!(kind, TransitionKind::Internal),
            loc: self.loc(node),
        }
    }

    /// Lower `after` / `every` / `every_internal` declarations into the
    /// owning state's [`TimerObject`] list AND, for the targeted forms,
    /// matching [`TransitionObject`]s with `Trigger::After` / `Trigger::Every`
    /// carrying the timer's stable id (P0-4 fix). Without the timer-bound
    /// trigger, downstream codegen had no way to dispatch the timer event
    /// independently from `done` completion and would collapse both into
    /// `EVENT__COMPLETION`.
    fn lower_timers(
        &mut self,
        state: &ast::StateDecl,
        owner: &str,
    ) -> (Vec<TimerObject>, Vec<TransitionObject>) {
        let mut timers = Vec::new();
        let mut transitions = Vec::new();
        for (kind_idx, a) in state.after().enumerate() {
            if let Some(ms) = duration_ms(a.syntax()) {
                let target = a.target();
                let actions = action_block_under(a.syntax())
                    .map(|ab| self.lower_action_block(&ab))
                    .unwrap_or_default();
                let timer_id = self.next_pseudo_id("timer");
                let stable_id =
                    format!("M:{}:timer:after:{}:{}", self.machine_name, owner, kind_idx);
                if let Some(t) = target.as_ref() {
                    transitions.push(self.build_transition(
                        owner,
                        &state_target_id(self, t),
                        TransitionKind::External,
                        Some(Trigger::After {
                            duration_ms: ms,
                            timer_id: timer_id.clone(),
                        }),
                        0,
                        None,
                        actions.clone(),
                        a.syntax(),
                        "",
                    ));
                }
                timers.push(TimerObject {
                    id: timer_id,
                    stable_id,
                    kind: TimerKind::After,
                    duration_ms: ms,
                    owner_state_id: owner.to_string(),
                    target: target.map(|s| state_target_id(self, &s)),
                    actions,
                    loc: self.loc(a.syntax()),
                });
            }
        }
        for (kind_idx, e) in state.every().enumerate() {
            if let Some(ms) = duration_ms(e.syntax()) {
                let target = e.target();
                let actions = action_block_under(e.syntax())
                    .map(|ab| self.lower_action_block(&ab))
                    .unwrap_or_default();
                let timer_id = self.next_pseudo_id("timer");
                let stable_id =
                    format!("M:{}:timer:every:{}:{}", self.machine_name, owner, kind_idx);
                if let Some(t) = target.as_ref() {
                    transitions.push(self.build_transition(
                        owner,
                        &state_target_id(self, t),
                        TransitionKind::External,
                        Some(Trigger::Every {
                            period_ms: ms,
                            timer_id: timer_id.clone(),
                        }),
                        0,
                        None,
                        actions.clone(),
                        e.syntax(),
                        "",
                    ));
                }
                timers.push(TimerObject {
                    id: timer_id,
                    stable_id,
                    kind: TimerKind::Every,
                    duration_ms: ms,
                    owner_state_id: owner.to_string(),
                    target: target.map(|s| state_target_id(self, &s)),
                    actions,
                    loc: self.loc(e.syntax()),
                });
            }
        }
        for (kind_idx, e) in state.every_internal().enumerate() {
            if let Some(ms) = duration_ms(e.syntax()) {
                let actions = action_block_under(e.syntax())
                    .map(|ab| self.lower_action_block(&ab))
                    .unwrap_or_default();
                let timer_id = self.next_pseudo_id("timer");
                let stable_id = format!(
                    "M:{}:timer:every_internal:{}:{}",
                    self.machine_name, owner, kind_idx
                );
                // `every_internal` has no target; codegen runs the actions
                // on each fire without an exit/entry sequence. We still emit
                // an internal-kind transition so the timer trigger is
                // distinguishable from `done` completion in dispatch.
                transitions.push(self.build_transition(
                    owner,
                    owner,
                    TransitionKind::Internal,
                    Some(Trigger::Every {
                        period_ms: ms,
                        timer_id: timer_id.clone(),
                    }),
                    0,
                    None,
                    actions.clone(),
                    e.syntax(),
                    "",
                ));
                timers.push(TimerObject {
                    id: timer_id,
                    stable_id,
                    kind: TimerKind::EveryInternal,
                    duration_ms: ms,
                    owner_state_id: owner.to_string(),
                    target: None,
                    actions,
                    loc: self.loc(e.syntax()),
                });
            }
        }
        (timers, transitions)
    }

    fn lower_defers(&mut self, state: &ast::StateDecl) -> Vec<IrDeferDecl> {
        state
            .defers()
            .filter_map(|d| {
                let event = d.event()?;
                Some(IrDeferDecl {
                    event_id: format!("ev-{}-{event}", self.machine_name),
                    loc: self.loc(d.syntax()),
                })
            })
            .collect()
    }

    // -- action / guard sublanguage lowering ---------------------------------
    //
    // Doc 04 §8.5 + §8.7 + §9 / Doc 09 §7-§9. Walks the typed AST produced
    // by the Pratt expression parser and reshapes it into IR
    // [`Statement`] / [`GuardExpr`] / [`IrExpr`] trees that the simulator
    // and codegen consume directly. On a malformed sub-expression we emit
    // the most-recoverable IR fallback (see [`Self::guard_recovery`]) rather
    // than panic — the parser has already flagged the syntactic error.

    fn lower_action_block(&mut self, ab: &ast::ActionBlock) -> Vec<Statement> {
        let mut out = Vec::new();
        for stmt in ab.statements() {
            if let Some(s) = self.lower_stmt(&stmt) {
                out.push(s);
            }
        }
        out
    }

    fn lower_stmt(&mut self, stmt: &ast::Stmt) -> Option<Statement> {
        match stmt {
            ast::Stmt::Assign(a) => self.lower_stmt_assign(a),
            ast::Stmt::If(i) => self.lower_stmt_if(i),
            ast::Stmt::While(w) => self.lower_stmt_while(w),
            ast::Stmt::For(f) => self.lower_stmt_for(f),
            ast::Stmt::Call(c) => self.lower_stmt_call(c),
            ast::Stmt::Raise(r) => self.lower_stmt_raise(r),
            ast::Stmt::Send(s) => self.lower_stmt_send(s),
            ast::Stmt::Defer(d) => self.lower_stmt_defer(d),
        }
    }

    fn lower_stmt_assign(&mut self, a: &ast::StmtAssign) -> Option<Statement> {
        // STMT_ASSIGN children: lhs Expr, RHS Expr.
        let mut exprs = a
            .syntax()
            .children()
            .filter_map(|n| ast::Expr::cast(n.clone()));
        let lhs_ast = exprs.next()?;
        let rhs_ast = exprs.next()?;
        let target = expr_to_field_ref(&lhs_ast)?;
        let value = self.lower_action_expr(&rhs_ast);
        Some(Statement::Assign { target, value })
    }

    fn lower_stmt_if(&mut self, i: &ast::StmtIf) -> Option<Statement> {
        // STMT_IF children (in order): condition Expr, then-ACTION_BLOCK,
        // optional STMT_ELSE wrapping either another STMT_IF or an
        // ACTION_BLOCK.
        let mut iter = i.syntax().children();
        let cond_node = iter.next()?;
        let condition = ast::Expr::cast(cond_node.clone())
            .map(|e| self.lower_action_expr(&e))
            .unwrap_or_else(literal_false_expr);
        let then_block = iter.next()?;
        let then = if then_block.kind() == SyntaxKind::ACTION_BLOCK {
            ast::ActionBlock::cast(then_block.clone())
                .map(|ab| self.lower_action_block(&ab))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let else_ = iter
            .find(|n| n.kind() == SyntaxKind::STMT_ELSE)
            .map(|n| self.lower_stmt_else_body(&n))
            .unwrap_or_default();
        Some(Statement::If {
            condition,
            then,
            else_,
        })
    }

    fn lower_stmt_else_body(&mut self, n: &SyntaxNode) -> Vec<Statement> {
        // STMT_ELSE wraps either an `if` chain (else-if) or an ACTION_BLOCK.
        for child in n.children() {
            match child.kind() {
                SyntaxKind::STMT_IF => {
                    if let Some(if_stmt) = ast::StmtIf::cast(child.clone()) {
                        if let Some(s) = self.lower_stmt_if(&if_stmt) {
                            return vec![s];
                        }
                    }
                }
                SyntaxKind::ACTION_BLOCK => {
                    if let Some(ab) = ast::ActionBlock::cast(child.clone()) {
                        return self.lower_action_block(&ab);
                    }
                }
                _ => {}
            }
        }
        Vec::new()
    }

    fn lower_stmt_while(&mut self, w: &ast::StmtWhile) -> Option<Statement> {
        // STMT_WHILE children: condition Expr, body ACTION_BLOCK.
        let mut iter = w.syntax().children();
        let cond_node = iter.next()?;
        let condition = ast::Expr::cast(cond_node.clone())
            .map(|e| self.lower_action_expr(&e))
            .unwrap_or_else(literal_false_expr);
        let body = iter
            .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
            .and_then(ast::ActionBlock::cast)
            .map(|ab| self.lower_action_block(&ab))
            .unwrap_or_default();
        Some(Statement::While { condition, body })
    }

    fn lower_stmt_for(&mut self, f: &ast::StmtFor) -> Option<Statement> {
        // STMT_FOR children (Pratt-ordered): init STMT_ASSIGN, condition Expr,
        // update STMT_ASSIGN, body ACTION_BLOCK.
        let children: Vec<SyntaxNode> = f.syntax().children().collect();
        let init_node = children
            .iter()
            .find(|n| n.kind() == SyntaxKind::STMT_ASSIGN)?;
        let init =
            ast::StmtAssign::cast(init_node.clone()).and_then(|a| self.lower_stmt_assign(&a))?;
        let mut assigns_seen = 0usize;
        let mut update: Option<Statement> = None;
        let mut condition: Option<IrExpr> = None;
        let mut body: Vec<Statement> = Vec::new();
        for n in &children {
            match n.kind() {
                SyntaxKind::STMT_ASSIGN => {
                    assigns_seen += 1;
                    if assigns_seen == 2 {
                        update = ast::StmtAssign::cast(n.clone())
                            .and_then(|a| self.lower_stmt_assign(&a));
                    }
                }
                SyntaxKind::ACTION_BLOCK => {
                    if let Some(ab) = ast::ActionBlock::cast(n.clone()) {
                        body = self.lower_action_block(&ab);
                    }
                }
                _ => {
                    if condition.is_none() {
                        if let Some(e) = ast::Expr::cast(n.clone()) {
                            condition = Some(self.lower_action_expr(&e));
                        }
                    }
                }
            }
        }
        Some(Statement::For {
            init: Box::new(init),
            condition: condition.unwrap_or_else(literal_false_expr),
            update: Box::new(update.unwrap_or(Statement::Call {
                callee: String::new(),
                args: Vec::new(),
            })),
            body,
        })
    }

    fn lower_stmt_call(&mut self, c: &ast::StmtCall) -> Option<Statement> {
        // STMT_CALL wraps a single Expr (a call or bare-ident). Allow the
        // bare-ident form to lower to a zero-arg call so users may write
        // `reset_link` as a no-arg extern invocation.
        let expr = c.syntax().children().find_map(ast::Expr::cast)?;
        match expr {
            ast::Expr::Call(call_node) => {
                let (callee, args) = self.lower_call_form(call_node.syntax())?;
                Some(Statement::Call { callee, args })
            }
            ast::Expr::NameRef(_) => {
                let callee = first_ident_text(expr.syntax())?;
                Some(Statement::Call {
                    callee,
                    args: Vec::new(),
                })
            }
            _ => None,
        }
    }

    fn lower_stmt_raise(&mut self, r: &ast::StmtRaise) -> Option<Statement> {
        // `raise EVT(args?)` — first ident is the event name.
        let event = first_ident_text(r.syntax())?;
        let args = self.lower_arg_list_under(r.syntax());
        Some(Statement::Raise {
            event_id: format!("ev-{}-{event}", self.machine_name),
            args,
        })
    }

    fn lower_stmt_send(&mut self, s: &ast::StmtSend) -> Option<Statement> {
        // `send EVT(args?) to TARGET` — two ident tokens (event then target).
        let idents = ident_token_texts(s.syntax());
        let event = idents.first()?.clone();
        let machine_target = idents.get(1).cloned().unwrap_or_default();
        let args = self.lower_arg_list_under(s.syntax());
        Some(Statement::Send {
            event_id: format!("ev-{}-{event}", self.machine_name),
            args,
            machine_id: machine_target,
        })
    }

    fn lower_stmt_defer(&mut self, d: &ast::StmtDefer) -> Option<Statement> {
        let event = first_ident_text(d.syntax())?;
        Some(Statement::Defer {
            event_id: format!("ev-{}-{event}", self.machine_name),
        })
    }

    fn lower_arg_list_under(&mut self, parent: &SyntaxNode) -> Vec<IrExpr> {
        parent
            .children()
            .find(|n| n.kind() == SyntaxKind::ARG_LIST)
            .map(|al| self.lower_arg_list(&al))
            .unwrap_or_default()
    }

    fn lower_arg_list(&mut self, node: &SyntaxNode) -> Vec<IrExpr> {
        node.children()
            .filter_map(|c| ast::Expr::cast(c.clone()))
            .map(|e| self.lower_action_expr(&e))
            .collect()
    }

    fn lower_action_expr(&mut self, e: &ast::Expr) -> IrExpr {
        match e {
            ast::Expr::Binary(b) => self.lower_binary_expr(b.syntax()),
            ast::Expr::Unary(u) => self.lower_unary_expr(u.syntax()),
            ast::Expr::Cast(c) => self.lower_cast_expr(c.syntax()),
            ast::Expr::Call(c) => {
                let (callee, args) = self
                    .lower_call_form(c.syntax())
                    .unwrap_or_else(|| (String::new(), Vec::new()));
                IrExpr::Call { callee, args }
            }
            ast::Expr::FieldRef(f) => match field_ref_from_node(f.syntax()) {
                Some(field_ref) => IrExpr::FieldRef { field_ref },
                None => {
                    // Treat `EnumName.Variant` as an enum-variant literal
                    // when it isn't a `ctx.x` / `payload.x` reference.
                    if let Some(lit) = enum_variant_from_field_ref(f.syntax()) {
                        IrExpr::Literal(Literal::EnumVariant(lit))
                    } else {
                        IrExpr::Literal(Literal::Bool(BoolLit {
                            value: false,
                            loc: None,
                        }))
                    }
                }
            },
            ast::Expr::Literal(l) => IrExpr::Literal(self.lower_literal(l.syntax()).unwrap_or(
                Literal::Bool(BoolLit {
                    value: false,
                    loc: None,
                }),
            )),
            ast::Expr::NameRef(n) => {
                // A bare identifier in an action-expression position is most
                // commonly a zero-arg extern call (e.g. `reset_link`). The
                // analyzer's scope check determines whether the name resolves
                // to an extern; here we conservatively model it as a Call
                // with no args so codegen/simulator can use it.
                let callee = first_ident_text(n.syntax()).unwrap_or_default();
                IrExpr::Call {
                    callee,
                    args: Vec::new(),
                }
            }
            ast::Expr::QualifiedName(q) => {
                // Pratt parser routes most qualified names through FieldRef,
                // but the EXPR_QUALIFIED_NAME shape may appear for explicit
                // enum-variant literals.
                if let Some(lit) = enum_variant_from_field_ref(q.syntax()) {
                    IrExpr::Literal(Literal::EnumVariant(lit))
                } else {
                    IrExpr::Literal(Literal::Bool(BoolLit {
                        value: false,
                        loc: None,
                    }))
                }
            }
            ast::Expr::Paren(p) => p
                .syntax()
                .children()
                .find_map(ast::Expr::cast)
                .map(|inner| self.lower_action_expr(&inner))
                .unwrap_or_else(literal_false_expr),
            ast::Expr::GuardElse(_) => {
                // `else` is a guard-only marker; in action position fall back
                // to a constant false so the parser-emitted diagnostic
                // remains the sole source of error reporting.
                literal_false_expr()
            }
        }
    }

    fn lower_binary_expr(&mut self, node: &SyntaxNode) -> IrExpr {
        let mut children = node.children();
        let lhs = children
            .next()
            .and_then(ast::Expr::cast)
            .map(|e| self.lower_action_expr(&e))
            .unwrap_or_else(literal_false_expr);
        let rhs = children
            .next()
            .and_then(ast::Expr::cast)
            .map(|e| self.lower_action_expr(&e))
            .unwrap_or_else(literal_false_expr);
        let op = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find_map(|t| binary_op_from_kind(t.kind()))
            .unwrap_or(BinaryOp::Eq);
        IrExpr::Binary {
            op,
            left: Box::new(lhs),
            right: Box::new(rhs),
        }
    }

    fn lower_unary_expr(&mut self, node: &SyntaxNode) -> IrExpr {
        let operand = node
            .children()
            .find_map(ast::Expr::cast)
            .map(|e| self.lower_action_expr(&e))
            .unwrap_or_else(literal_false_expr);
        let op = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find_map(|t| unary_op_from_kind(t.kind()))
            .unwrap_or(UnaryOp::Not);
        IrExpr::Unary {
            op,
            operand: Box::new(operand),
        }
    }

    fn lower_cast_expr(&mut self, node: &SyntaxNode) -> IrExpr {
        let operand = node
            .children()
            .find_map(ast::Expr::cast)
            .map(|e| self.lower_action_expr(&e))
            .unwrap_or_else(literal_false_expr);
        let target_type = node
            .children()
            .find_map(ast::TypeRef::cast)
            .and_then(|t| self.lower_type_ref(Some(&t)))
            .unwrap_or(Type::Primitive { name: "i64".into() });
        IrExpr::Cast(CastExpr {
            operand: Box::new(operand),
            target_type,
            loc: self.loc(node),
        })
    }

    fn lower_call_form(&mut self, node: &SyntaxNode) -> Option<(String, Vec<IrExpr>)> {
        // EXPR_CALL children: callee Expr (typically EXPR_NAME_REF), ARG_LIST.
        let callee_node = node.children().next()?;
        let callee = first_ident_text(&callee_node)?;
        let args = self.lower_arg_list_under(node);
        Some((callee, args))
    }

    // ----- Guard expressions ------------------------------------------------

    fn lower_guard_clause(&mut self, g: &ast::GuardClause) -> GuardExpr {
        match g.expr() {
            Some(expr) => self.lower_guard_expr(&expr),
            // Empty `[]` recovers to a falsy guard so the transition stays
            // inert until the user authors a valid guard.
            None => GuardExpr::Not {
                operand: Box::new(GuardExpr::Else),
            },
        }
    }

    fn lower_guard_expr(&mut self, e: &ast::Expr) -> GuardExpr {
        match e {
            ast::Expr::GuardElse(_) => GuardExpr::Else,
            ast::Expr::Literal(l) => match self.lower_literal(l.syntax()) {
                Some(Literal::Bool(b)) => {
                    if b.value {
                        // `[true]` is exact-equal to "no guard"; emit a
                        // tautological field-cmp (1 == 1) so the IR remains
                        // serializable without an extra discriminant.
                        GuardExpr::ExternCall {
                            callee: "__true".into(),
                            args: vec![IrExpr::Literal(Literal::Bool(BoolLit {
                                value: true,
                                loc: None,
                            }))],
                        }
                    } else {
                        GuardExpr::Not {
                            operand: Box::new(GuardExpr::Else),
                        }
                    }
                }
                _ => self.guard_recovery(),
            },
            ast::Expr::Unary(u) => {
                // Only `!` is meaningful in guard context.
                let inner = u
                    .syntax()
                    .children()
                    .find_map(ast::Expr::cast)
                    .map(|e| self.lower_guard_expr(&e))
                    .unwrap_or_else(|| self.guard_recovery());
                let is_bang = u
                    .syntax()
                    .children_with_tokens()
                    .filter_map(|el| el.into_token())
                    .any(|t| t.kind() == SyntaxKind::Bang);
                if is_bang {
                    GuardExpr::Not {
                        operand: Box::new(inner),
                    }
                } else {
                    inner
                }
            }
            ast::Expr::Binary(b) => self.lower_guard_binary(b.syntax()),
            ast::Expr::Paren(p) => p
                .syntax()
                .children()
                .find_map(ast::Expr::cast)
                .map(|inner| self.lower_guard_expr(&inner))
                .unwrap_or_else(|| self.guard_recovery()),
            ast::Expr::Call(c) => {
                let (callee, args) = self
                    .lower_call_form(c.syntax())
                    .unwrap_or_else(|| (String::new(), Vec::new()));
                GuardExpr::ExternCall { callee, args }
            }
            ast::Expr::NameRef(n) => {
                // A bare identifier in guard position is a zero-arg pure
                // extern call — the common case (`[can_start]`).
                let callee = first_ident_text(n.syntax()).unwrap_or_default();
                GuardExpr::ExternCall {
                    callee,
                    args: Vec::new(),
                }
            }
            ast::Expr::FieldRef(_) | ast::Expr::QualifiedName(_) | ast::Expr::Cast(_) => {
                self.guard_recovery()
            }
        }
    }

    fn lower_guard_binary(&mut self, node: &SyntaxNode) -> GuardExpr {
        let op_tok = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| guard_op_kind(t.kind()).is_some());
        let kind = op_tok
            .as_ref()
            .map(|t| t.kind())
            .unwrap_or(SyntaxKind::EqEq);
        let mut child_exprs = node.children().filter_map(|c| ast::Expr::cast(c.clone()));
        let lhs = child_exprs.next();
        let rhs = child_exprs.next();
        match (kind, lhs, rhs) {
            (SyntaxKind::AmpAmp, Some(l), Some(r)) => GuardExpr::And {
                left: Box::new(self.lower_guard_expr(&l)),
                right: Box::new(self.lower_guard_expr(&r)),
            },
            (SyntaxKind::PipePipe, Some(l), Some(r)) => GuardExpr::Or {
                left: Box::new(self.lower_guard_expr(&l)),
                right: Box::new(self.lower_guard_expr(&r)),
            },
            (kind, Some(l), Some(r)) => {
                let op = match cmp_op_from_kind(kind) {
                    Some(o) => o,
                    None => return self.guard_recovery(),
                };
                let Some(lhs_ref) = expr_to_field_ref(&l) else {
                    return self.guard_recovery();
                };
                let rhs_operand = match guard_operand_from_expr(&r) {
                    Some(o) => o,
                    None => match self.lower_literal(r.syntax()) {
                        Some(lit) => GuardOperand::Literal(lit),
                        None => return self.guard_recovery(),
                    },
                };
                GuardExpr::FieldCmp {
                    lhs: lhs_ref,
                    op,
                    rhs: rhs_operand,
                }
            }
            _ => self.guard_recovery(),
        }
    }

    /// Recovery sentinel for malformed guards: never-true so the transition
    /// stays inert and the parser's diagnostic remains the surfaced error.
    fn guard_recovery(&self) -> GuardExpr {
        GuardExpr::Not {
            operand: Box::new(GuardExpr::Else),
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers for action/guard lowering — kept outside the impl block
// because they're pure tree-rewrites that don't need the LoweringCtx state.
// ---------------------------------------------------------------------------

/// Best-effort scan of a transition AST node for an `IDENT '(' IDENT ')'`
/// pattern immediately after the leading `on`. Captures the second ident as
/// the payload-binding name. The current grammar does not produce a typed
/// node for the binding, so we walk the green-tree tokens directly.
fn extract_trigger_payload_binding(node: &SyntaxNode) -> Option<String> {
    let mut tokens = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| !t.kind().is_trivia());
    // Token sequence we expect: KwOn IDENT LParen IDENT RParen ...
    let _on = tokens.next()?;
    let _trigger = tokens.next()?;
    let lparen = tokens.next()?;
    if lparen.kind() != SyntaxKind::LParen {
        return None;
    }
    let binding = tokens.next()?;
    if binding.kind() != SyntaxKind::Ident {
        return None;
    }
    let rparen = tokens.next()?;
    if rparen.kind() != SyntaxKind::RParen {
        return None;
    }
    Some(binding.text().to_string())
}

/// Locate the ACTION_BLOCK child of a timer / completion node (`after`,
/// `every`, etc.) — these AST nodes do not yet expose a typed accessor.
fn action_block_under(node: &SyntaxNode) -> Option<ast::ActionBlock> {
    node.children()
        .find(|c| c.kind() == SyntaxKind::ACTION_BLOCK)
        .and_then(ast::ActionBlock::cast)
}

/// Translate an action-language `Expr::FieldRef` into the IR's `FieldRef`,
/// which uses `Ctx { field } | Payload { field }`. Returns `None` for any
/// other shape (e.g. `EnumName.Variant`).
fn expr_to_field_ref(e: &ast::Expr) -> Option<FieldRef> {
    let node = e.syntax();
    if node.kind() != SyntaxKind::EXPR_FIELD_REF {
        return None;
    }
    field_ref_from_node(node)
}

/// Same as [`expr_to_field_ref`] but operating on a raw syntax node. Returns
/// `None` if the leading qualifier isn't `ctx` / `payload`.
fn field_ref_from_node(node: &SyntaxNode) -> Option<FieldRef> {
    let head = node.children().next().and_then(|c| first_ident_text(&c))?;
    let field = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();
    match head.as_str() {
        "ctx" => Some(FieldRef::Ctx { field }),
        "payload" => Some(FieldRef::Payload { field }),
        _ => None,
    }
}

/// Translate an `EnumName.Variant` field-ref into an enum-variant literal.
/// Only fires when the leading qualifier is not `ctx` / `payload`.
fn enum_variant_from_field_ref(node: &SyntaxNode) -> Option<EnumVariantLit> {
    let enum_name = node.children().next().and_then(|c| first_ident_text(&c))?;
    if enum_name == "ctx" || enum_name == "payload" {
        return None;
    }
    let variant_name = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();
    Some(EnumVariantLit {
        enum_name,
        variant_name,
        loc: None,
    })
}

/// First Ident text descending into the typed expression subtree.
fn first_ident_text(node: &SyntaxNode) -> Option<String> {
    node.descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
}

fn ident_token_texts(node: &SyntaxNode) -> Vec<String> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .collect()
}

fn guard_operand_from_expr(e: &ast::Expr) -> Option<GuardOperand> {
    match e {
        ast::Expr::FieldRef(_) => {
            let f = expr_to_field_ref(e)?;
            Some(GuardOperand::FieldRef(f))
        }
        _ => None,
    }
}

fn binary_op_from_kind(k: SyntaxKind) -> Option<BinaryOp> {
    Some(match k {
        SyntaxKind::Plus => BinaryOp::Add,
        SyntaxKind::Minus => BinaryOp::Sub,
        SyntaxKind::Star => BinaryOp::Mul,
        SyntaxKind::Slash => BinaryOp::Div,
        SyntaxKind::Percent => BinaryOp::Mod,
        SyntaxKind::Amp => BinaryOp::BitAnd,
        SyntaxKind::Pipe => BinaryOp::BitOr,
        SyntaxKind::Caret => BinaryOp::BitXor,
        SyntaxKind::Shl => BinaryOp::Shl,
        SyntaxKind::Shr => BinaryOp::Shr,
        SyntaxKind::AmpAmp => BinaryOp::LogAnd,
        SyntaxKind::PipePipe => BinaryOp::LogOr,
        SyntaxKind::EqEq => BinaryOp::Eq,
        SyntaxKind::BangEq => BinaryOp::NotEq,
        SyntaxKind::Lt => BinaryOp::Lt,
        SyntaxKind::Gt => BinaryOp::Gt,
        SyntaxKind::Le => BinaryOp::LtEq,
        SyntaxKind::Ge => BinaryOp::GtEq,
        _ => return None,
    })
}

fn unary_op_from_kind(k: SyntaxKind) -> Option<UnaryOp> {
    Some(match k {
        SyntaxKind::Bang => UnaryOp::Not,
        SyntaxKind::Minus => UnaryOp::Neg,
        SyntaxKind::Tilde => UnaryOp::BitNot,
        _ => return None,
    })
}

fn guard_op_kind(k: SyntaxKind) -> Option<()> {
    matches!(
        k,
        SyntaxKind::EqEq
            | SyntaxKind::BangEq
            | SyntaxKind::Lt
            | SyntaxKind::Gt
            | SyntaxKind::Le
            | SyntaxKind::Ge
            | SyntaxKind::AmpAmp
            | SyntaxKind::PipePipe
    )
    .then_some(())
}

fn cmp_op_from_kind(k: SyntaxKind) -> Option<CmpOp> {
    Some(match k {
        SyntaxKind::EqEq => CmpOp::Eq,
        SyntaxKind::BangEq => CmpOp::NotEq,
        SyntaxKind::Lt => CmpOp::Lt,
        SyntaxKind::Gt => CmpOp::Gt,
        SyntaxKind::Le => CmpOp::LtEq,
        SyntaxKind::Ge => CmpOp::GtEq,
        _ => return None,
    })
}

fn literal_false_expr() -> IrExpr {
    IrExpr::Literal(Literal::Bool(BoolLit {
        value: false,
        loc: None,
    }))
}

fn duration_ms(node: &SyntaxNode) -> Option<u32> {
    let ce = node
        .children()
        .find(|c| c.kind() == SyntaxKind::CONST_EXPR)?;
    let expr = ce.children().next()?;
    let v = eval_i64(&expr)?;
    if v < 0 {
        return None;
    }
    u32::try_from(v).ok()
}

fn eval_i64(node: &SyntaxNode) -> Option<i64> {
    match node.kind() {
        SyntaxKind::EXPR_LITERAL => {
            let tok = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::IntLiteral)?;
            let s: String = tok.text().chars().filter(|c| *c != '_').collect();
            if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                i64::from_str_radix(rest, 16).ok()
            } else if let Some(rest) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
                i64::from_str_radix(rest, 2).ok()
            } else {
                s.parse().ok()
            }
        }
        SyntaxKind::EXPR_UNARY => {
            let op = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::Minus | SyntaxKind::Plus))?;
            let inner = node.children().next()?;
            let v = eval_i64(&inner)?;
            match op.kind() {
                SyntaxKind::Minus => Some(-v),
                _ => Some(v),
            }
        }
        SyntaxKind::EXPR_PAREN => {
            let inner = node.children().next()?;
            eval_i64(&inner)
        }
        _ => None,
    }
}

fn extract_priority(node: &SyntaxNode) -> Option<i64> {
    let tok = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::IntLiteral)?;
    let s: String = tok.text().chars().filter(|c| *c != '_').collect();
    s.parse().ok()
}

fn state_target_id(ctx: &LoweringCtx, name: &str) -> String {
    if name.is_empty() {
        format!("s-{}-unknown", ctx.machine_name)
    } else {
        format!("s-{}-{}", ctx.machine_name, name)
    }
}

fn extract_history(state: &ast::StateDecl, ctx: &mut LoweringCtx) -> Option<HistoryObject> {
    let mut found = None;
    for child in state.syntax().children() {
        let kind = child.kind();
        if matches!(
            kind,
            SyntaxKind::SHALLOW_HISTORY_DECL | SyntaxKind::DEEP_HISTORY_DECL
        ) {
            let history_kind = if kind == SyntaxKind::SHALLOW_HISTORY_DECL {
                HistoryKind::Shallow
            } else {
                HistoryKind::Deep
            };
            let StateNode::History(h) = ctx.lower_history(&child, history_kind) else {
                continue;
            };
            found = Some(h);
            break;
        }
    }
    found
}

/// `sha256:` content hash of `src` — Doc 09 §2 `sourceHash`. Implementation
/// uses a small in-tree SHA-256 to avoid a workspace-level dep.
fn source_hash(src: &str) -> String {
    let bytes = sha256(src.as_bytes());
    let mut out = String::from("sha256:");
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Tiny SHA-256 implementation — no external dep.
// Reference: RFC 6234. Used exclusively for source-hash labels in the IR.
// ---------------------------------------------------------------------------

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded: Vec<u8> = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, c) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> fsm_parser::ParseResult {
        fsm_parser::parse(src)
    }

    #[test]
    fn lower_empty_machine_produces_ir() {
        let pr = parse("language fsm 2.0\nmachine M { }");
        let res = analyze(&pr);
        let ir = res.ir.unwrap();
        assert_eq!(ir.ir_version, "1.0.0");
        assert_eq!(ir.machines.len(), 1);
        assert_eq!(ir.machines[0].name, "M");
    }

    #[test]
    fn source_hash_changes_with_source() {
        let a = source_hash("language fsm 2.0");
        let b = source_hash("language fsm 2.1");
        assert_ne!(a, b);
        assert!(a.starts_with("sha256:"));
    }

    #[test]
    fn external_self_transition_marks_kind_external() {
        let pr =
            parse("language fsm 2.0\nmachine M { events { E } initial S state S { on E -> S } }");
        let res = analyze(&pr);
        let ir = res.ir.unwrap();
        let m = &ir.machines[0];
        let root_state = &m
            .root
            .states
            .iter()
            .find_map(|s| match s {
                StateNode::Simple(s) => Some(s),
                _ => None,
            })
            .unwrap();
        assert_eq!(root_state.transitions.len(), 1);
        assert!(matches!(
            root_state.transitions[0].kind,
            TransitionKind::External
        ));
    }
}
