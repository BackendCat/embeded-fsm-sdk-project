//! Symbol table — first pass over the AST collects every named entity per
//! machine; the analyzer then resolves references against it.
//!
//! Layout: a flat `MachineSymbols` per top-level `machine` declaration, each
//! holding ordered tables for events, externs, consts, enums, states, regions
//! and context fields. Each table is a `Vec<Entry>` plus a parallel name
//! index so duplicate-name detection (FSM-E0020..E0024) can report the
//! prior declaration via [`fsm_diagnostics::RelatedInfo`].
//!
//! Doc 04 §5/§7 — every `state` introduces a new container scope; the same
//! state name may appear under different parents but not within the same
//! parent scope (Doc 10 FSM-E0021).

use std::collections::HashMap;

use fsm_diagnostics::{Diagnostic, DiagnosticCode, RelatedInfo, Span};
use fsm_parser::ast::{self, AstNode};

use crate::scope::Scope;
use crate::util::span_of;

/// One named entity within a machine scope. Common fields shared by every
/// table; specialised tables wrap this in a typed alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub span: Span,
    /// Stable ID (`@id("...")`), or auto-generated `M:kind:name` when the
    /// declaration omits an explicit one. Populated only for those entities
    /// that carry stable IDs (states, transitions, events, externs).
    pub stable_id: Option<String>,
}

/// State-table entry — adds the parent container path so the analyzer can
/// disambiguate same-named states under different parents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateEntry {
    pub name: String,
    pub span: Span,
    pub stable_id: Option<String>,
    /// Dotted path of containing state / region IDs (machine-root is empty).
    pub container_path: Vec<String>,
    /// Discriminator — Simple/Composite cannot be known without inspecting
    /// children, so we set a coarse [`StateShape`] hint based on the AST node
    /// type for the few checks that need it (history default existence,
    /// parallel-region rules, fork/join validation).
    pub shape: StateShape,
}

/// Coarse classification of a state-like declaration. Composite vs Simple is
/// resolved at lowering time by inspecting nested AST children.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StateShape {
    /// `state X { ... }` — Simple if no nested states/regions, Composite
    /// otherwise. The distinction is computed at lowering time.
    Plain,
    /// `final X`
    Final,
    /// `shallow_history X { default -> ... }`
    ShallowHistory,
    /// `deep_history X { default -> ... }`
    DeepHistory,
    /// `choice X { ... }`
    Choice,
    /// `junction X { ... }`
    Junction,
    /// `fork X -> { ... }`
    Fork,
    /// `join X { ... } -> Y`
    Join,
    /// `entry_point name -> Target`
    EntryPoint,
    /// `exit_point name`
    ExitPoint,
}

/// Symbol bag for a single machine. All declarations live in machine scope
/// per Doc 04 §2 — events, externs, consts (file-level consts are mirrored
/// here so guard / action expressions can resolve them through one table).
#[derive(Clone, Debug)]
pub struct MachineSymbols {
    pub name: String,
    pub span: Span,
    pub stable_id: Option<String>,
    pub events: Vec<Entry>,
    pub externs: Vec<Entry>,
    /// `is_pure` parallel to `externs`.
    pub extern_pure: Vec<bool>,
    pub consts: Vec<Entry>,
    pub enums: Vec<EnumEntry>,
    pub context_fields: Vec<Entry>,
    /// Type text parallel to `context_fields` — primitive name (`"u8"`,
    /// `"bool"`, …) or enum/opaque identifier when present in source.
    pub context_field_types: Vec<Option<String>>,
    pub states: Vec<StateEntry>,
    /// Every region introduces a region "scope" — its container path includes
    /// the parent parallel/composite state.
    pub regions: Vec<StateEntry>,
}

impl Default for MachineSymbols {
    fn default() -> Self {
        Self {
            name: String::new(),
            span: Span::new(0, 0),
            stable_id: None,
            events: Vec::new(),
            externs: Vec::new(),
            extern_pure: Vec::new(),
            consts: Vec::new(),
            enums: Vec::new(),
            context_fields: Vec::new(),
            context_field_types: Vec::new(),
            states: Vec::new(),
            regions: Vec::new(),
        }
    }
}

/// Enum table entry — records the enum name plus its declared variants for
/// `EnumName.Variant` resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumEntry {
    pub name: String,
    pub span: Span,
    pub variants: Vec<String>,
}

/// Submachine-template registry entry — Doc 04 §15. Submachines are
/// **templates** referenced by `state X is Sub`, not instantiable top-level
/// machines, so they live in their own namespace (W2a kept
/// [`ast::File::submachines`] disjoint from [`ast::File::machines`] for
/// exactly this reason). The analyzer records just enough to (a) resolve a
/// `is Sub` reference, (b) decide FSM-E0500 (has an entry point: an
/// `initial` or an `entry_point`) and FSM-E0501 (has an exit point: a
/// `final` state or an `exit_point`), and (c) build the submachine
/// instantiation graph for FSM-E0502 cycle detection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmachineEntry {
    pub name: String,
    pub span: Span,
    /// `true` when the template declares an `initial` or an `entry_point`
    /// pseudo-state — the two entry forms per Doc 08 §12.2.
    pub has_entry_point: bool,
    /// `true` when the template declares a `final` state or an `exit_point`
    /// pseudo-state — the exit forms per Doc 08 §12.3.
    pub has_exit_point: bool,
    /// Names of submachines this template references via `state Y is Z`
    /// (its outgoing edges in the instantiation graph — FSM-E0502).
    pub references: Vec<String>,
}

/// Top-level symbol table — one [`MachineSymbols`] per declared machine.
#[derive(Clone, Debug, Default)]
pub struct SymbolTable {
    pub machines: Vec<MachineSymbols>,
    /// File-level consts — declared outside any `machine` block.
    pub file_consts: Vec<Entry>,
    /// File-level enums.
    pub file_enums: Vec<EnumEntry>,
    /// File-level externs.
    pub file_externs: Vec<Entry>,
    /// File-level extern purity (parallel to `file_externs`).
    pub file_extern_pure: Vec<bool>,
    /// Quick lookup machine_name -> index in `machines`.
    pub machine_index: HashMap<String, usize>,
    /// Top-level `submachine Name { … }` templates (Doc 04 §15). Distinct
    /// from `machines` — a submachine is a referenced template, never a
    /// top-level instantiable machine, so `is Sub` resolves here and never
    /// pollutes machine resolution.
    pub submachines: Vec<SubmachineEntry>,
    /// Quick lookup submachine_name -> index in `submachines`.
    pub submachine_index: HashMap<String, usize>,
}

impl SymbolTable {
    /// Walk the AST and populate the table. Emits FSM-E0020..E0024 / E0025
    /// for duplicate names and surfaces them as the second return value.
    pub fn build(file: &ast::File) -> (Self, Vec<Diagnostic>) {
        let mut diags = Vec::new();
        let mut st = SymbolTable::default();

        // File-scope decls — collected first so machines can shadow them
        // without losing the earlier instance for E0024 reporting.
        for c in file.consts() {
            if let Some(name) = c.name() {
                push_name_unique(
                    &mut st.file_consts,
                    name,
                    span_of(c.syntax()),
                    None,
                    |_, _| DiagnosticCode::E0023,
                    &mut diags,
                );
            }
        }
        for e in file.enums() {
            if let Some(name) = e.name() {
                let variants = e.variants().filter_map(|v| v.name()).collect::<Vec<_>>();
                if let Some(prev) = st.file_enums.iter().find(|x| x.name == name) {
                    diags.push(
                        Diagnostic::new(DiagnosticCode::E0023, span_of(e.syntax())).with_related(
                            RelatedInfo {
                                message: format!("previous enum '{}' here", prev.name),
                                span: prev.span,
                            },
                        ),
                    );
                } else {
                    st.file_enums.push(EnumEntry {
                        name,
                        span: span_of(e.syntax()),
                        variants,
                    });
                }
            }
        }
        for ext in file.externs() {
            if let Some(name) = ext.name() {
                let is_pure = ext.is_pure();
                if let Some(idx) = st.file_externs.iter().position(|x| x.name == name) {
                    let prev = &st.file_externs[idx];
                    diags.push(
                        Diagnostic::new(DiagnosticCode::E0024, span_of(ext.syntax())).with_related(
                            RelatedInfo {
                                message: format!("previous extern '{}' here", prev.name),
                                span: prev.span,
                            },
                        ),
                    );
                } else {
                    st.file_externs.push(Entry {
                        name: name.clone(),
                        span: span_of(ext.syntax()),
                        stable_id: None,
                    });
                    st.file_extern_pure.push(is_pure);
                }
            }
        }

        // Submachine templates (Doc 04 §15). Registered before machines so a
        // machine's `state X is Sub` can resolve `Sub`. Duplicate template
        // names reuse the machine-redefinition code (FSM-E0020) — a
        // submachine occupies the same "named top-level construct" space.
        for sm in file.submachines() {
            let sname = sm.name().unwrap_or_default();
            if sname.is_empty() {
                continue;
            }
            let sspan = span_of(sm.syntax());
            if let Some(&prev_idx) = st.submachine_index.get(&sname) {
                let prev = &st.submachines[prev_idx];
                diags.push(Diagnostic::new(DiagnosticCode::E0020, sspan).with_related(
                    RelatedInfo {
                        message: format!("previous submachine '{}' here", prev.name),
                        span: prev.span,
                    },
                ));
                continue;
            }
            let entry = build_submachine_entry(&sm, sname.clone(), sspan);
            st.submachine_index.insert(sname, st.submachines.len());
            st.submachines.push(entry);
        }

        // Machines.
        for m in file.machines() {
            let mname = m.name().unwrap_or_default();
            let mspan = span_of(m.syntax());

            if let Some(&prev_idx) = st.machine_index.get(&mname) {
                let prev = &st.machines[prev_idx];
                diags.push(Diagnostic::new(DiagnosticCode::E0020, mspan).with_related(
                    RelatedInfo {
                        message: format!("previous machine '{}' here", prev.name),
                        span: prev.span,
                    },
                ));
                continue;
            }

            let stable_id = m.stable_id().and_then(|s| s.id());
            let mut sym = MachineSymbols {
                name: mname.clone(),
                span: mspan,
                stable_id,
                ..Default::default()
            };

            // events ---------------------------------------------------------
            if let Some(ev_block) = m.events() {
                for ev in ev_block.events() {
                    if let Some(name) = ev.name() {
                        push_entry(
                            &mut sym.events,
                            name,
                            span_of(ev.syntax()),
                            None,
                            DiagnosticCode::E0022,
                            "event",
                            &mut diags,
                        );
                    }
                }
            }

            // externs (machine-local) ---------------------------------------
            for ext in m.externs() {
                if let Some(name) = ext.name() {
                    let is_pure = ext.is_pure();
                    if push_entry(
                        &mut sym.externs,
                        name,
                        span_of(ext.syntax()),
                        None,
                        DiagnosticCode::E0024,
                        "extern",
                        &mut diags,
                    ) {
                        sym.extern_pure.push(is_pure);
                    }
                }
            }

            // context fields ------------------------------------------------
            if let Some(ctx) = m.context() {
                for f in ctx.fields() {
                    if let Some(name) = f.name() {
                        let ty_text = f.ty().and_then(|t| primitive_type_text(t.syntax()));
                        if push_entry(
                            &mut sym.context_fields,
                            name,
                            span_of(f.syntax()),
                            None,
                            DiagnosticCode::E0023,
                            "context field",
                            &mut diags,
                        ) {
                            sym.context_field_types.push(ty_text);
                        }
                    }
                }
            }

            // states (recursive walk) ---------------------------------------
            collect_states(
                &m,
                &Scope::for_machine(st.machines.len()),
                &mut sym,
                &mut diags,
            );

            st.machine_index.insert(mname, st.machines.len());
            st.machines.push(sym);
        }

        (st, diags)
    }

    /// Resolve an event by name within the current scope's machine.
    pub fn resolve_event(&self, name: &str, ctx: &Scope) -> Option<&Entry> {
        let m = ctx.machine_idx?;
        self.machines.get(m)?.events.iter().find(|e| e.name == name)
    }

    /// Resolve an extern by name — searches machine-scope first, then file
    /// scope per Doc 04 §2.5.
    pub fn resolve_extern(&self, name: &str, ctx: &Scope) -> Option<(&Entry, bool)> {
        if let Some(mi) = ctx.machine_idx {
            if let Some(m) = self.machines.get(mi) {
                if let Some(idx) = m.externs.iter().position(|e| e.name == name) {
                    return Some((&m.externs[idx], m.extern_pure[idx]));
                }
            }
        }
        if let Some(idx) = self.file_externs.iter().position(|e| e.name == name) {
            return Some((&self.file_externs[idx], self.file_extern_pure[idx]));
        }
        None
    }

    /// Resolve a state by name within the machine's full state list.
    pub fn resolve_state(&self, name: &str, ctx: &Scope) -> Option<&StateEntry> {
        let m = ctx.machine_idx?;
        self.machines.get(m)?.states.iter().find(|s| s.name == name)
    }

    /// Resolve a context field — only valid inside a machine scope.
    pub fn resolve_context_field(&self, name: &str, ctx: &Scope) -> Option<&Entry> {
        let m = ctx.machine_idx?;
        self.machines
            .get(m)?
            .context_fields
            .iter()
            .find(|f| f.name == name)
    }

    /// Primitive type text associated with a context field (or enum/opaque
    /// name). Returns `None` when the field is not declared or the type was
    /// unresolvable.
    pub fn context_field_type(&self, name: &str, ctx: &Scope) -> Option<&str> {
        let m_idx = ctx.machine_idx?;
        let m = self.machines.get(m_idx)?;
        let idx = m.context_fields.iter().position(|f| f.name == name)?;
        m.context_field_types.get(idx).and_then(|s| s.as_deref())
    }

    /// Resolve a machine by name.
    pub fn resolve_machine(&self, name: &str) -> Option<&MachineSymbols> {
        let idx = *self.machine_index.get(name)?;
        self.machines.get(idx)
    }

    /// Resolve a submachine template by name (Doc 04 §15). Returns `None`
    /// when `state X is <name>` names something that is not a declared
    /// `submachine` — the FSM-E0103 trigger.
    pub fn resolve_submachine(&self, name: &str) -> Option<&SubmachineEntry> {
        let idx = *self.submachine_index.get(name)?;
        self.submachines.get(idx)
    }

    /// Resolve a qualified name `Enum.Variant` — returns the matching enum
    /// entry if the variant exists.
    pub fn resolve_enum_variant(&self, enum_name: &str, variant: &str) -> Option<&EnumEntry> {
        // Check file-scope enums.
        if let Some(e) = self.file_enums.iter().find(|e| e.name == enum_name) {
            if e.variants.iter().any(|v| v == variant) {
                return Some(e);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Pull the first primitive-keyword or identifier token under a `TYPE_REF`
/// node and return it as a string. Used to populate the parallel
/// `context_field_types` table.
fn primitive_type_text(ty_ref: &fsm_parser::cst::SyntaxNode) -> Option<String> {
    use fsm_parser::cst::SyntaxKind as K;
    ty_ref
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| {
            matches!(
                t.kind(),
                K::KwBool
                    | K::KwU8
                    | K::KwU16
                    | K::KwU32
                    | K::KwU64
                    | K::KwI8
                    | K::KwI16
                    | K::KwI32
                    | K::KwI64
                    | K::KwF32
                    | K::KwF64
                    | K::Ident
            )
        })
        .map(|t| t.text().to_string())
}

/// Build the [`SubmachineEntry`] for a `submachine Name { … }` template.
///
/// Walks the template's CST once collecting the three facts the analyzer
/// needs downstream:
///  - `has_entry_point`: an `initial` decl or an `entry_point` pseudo-state
///    (the two entry forms, Doc 08 §12.2) — drives FSM-E0500;
///  - `has_exit_point`: a `final` state or an `exit_point` pseudo-state
///    (the exit forms, Doc 08 §12.3) — drives FSM-E0501;
///  - `references`: every `state Y is Z` nested in the template — its
///    outgoing edges for the FSM-E0502 instantiation-cycle graph.
fn build_submachine_entry(sm: &ast::SubmachineDecl, name: String, span: Span) -> SubmachineEntry {
    use fsm_parser::cst::SyntaxKind as K;
    let mut has_entry_point = sm.initial().is_some();
    let mut has_exit_point = false;
    let mut references = Vec::new();
    for node in sm.syntax().descendants() {
        match node.kind() {
            K::INITIAL_DECL => has_entry_point = true,
            K::ENTRY_POINT_DECL => has_entry_point = true,
            K::FINAL_DECL => has_exit_point = true,
            K::EXIT_POINT_DECL => has_exit_point = true,
            K::SUBMACHINE_REF => {
                if let Some(r) = ast::SubmachineRef::cast(node.clone()).and_then(|r| r.name()) {
                    references.push(r);
                }
            }
            _ => {}
        }
    }
    SubmachineEntry {
        name,
        span,
        has_entry_point,
        has_exit_point,
        references,
    }
}

/// Push `Entry` to `tab` if the name is fresh. On collision, emit a diagnostic
/// of kind `code` with a `RelatedInfo` pointing at the previous declaration.
/// Returns `true` on successful push.
fn push_entry(
    tab: &mut Vec<Entry>,
    name: String,
    span: Span,
    stable_id: Option<String>,
    code: DiagnosticCode,
    label: &str,
    diags: &mut Vec<Diagnostic>,
) -> bool {
    if let Some(prev) = tab.iter().find(|e| e.name == name) {
        diags.push(Diagnostic::new(code, span).with_related(RelatedInfo {
            message: format!("previous {label} '{}' here", prev.name),
            span: prev.span,
        }));
        false
    } else {
        tab.push(Entry {
            name,
            span,
            stable_id,
        });
        true
    }
}

/// Internal helper used by file-level const collection — same semantics as
/// `push_entry` minus the parallel `is_pure` table.
fn push_name_unique(
    tab: &mut Vec<Entry>,
    name: String,
    span: Span,
    stable_id: Option<String>,
    classify: impl Fn(&str, &str) -> DiagnosticCode,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(prev) = tab.iter().find(|e| e.name == name) {
        diags.push(
            Diagnostic::new(classify(&prev.name, &name), span).with_related(RelatedInfo {
                message: format!("previous '{}' here", prev.name),
                span: prev.span,
            }),
        );
    } else {
        tab.push(Entry {
            name,
            span,
            stable_id,
        });
    }
}

/// Depth-first traversal collecting every state-like declaration under a
/// machine. Region containment is tracked via the running `Scope`.
fn collect_states(
    m: &ast::MachineDecl,
    base: &Scope,
    sym: &mut MachineSymbols,
    diags: &mut Vec<Diagnostic>,
) {
    for st in m.states() {
        visit_state(&st, base, sym, diags);
    }
    for region in m.regions() {
        visit_region(&region, base, sym, diags);
    }
    // Pseudo-state forms can also appear at machine top-level (final F,
    // choice C, etc.). The AST only exposes them via descendant traversal —
    // collect them recursively via the rowan tree.
    for child in m.syntax().children() {
        collect_pseudo_states(&child, base, sym, diags);
    }
}

fn visit_state(
    st: &ast::StateDecl,
    parent: &Scope,
    sym: &mut MachineSymbols,
    diags: &mut Vec<Diagnostic>,
) {
    let name = st.name().unwrap_or_default();
    let span = span_of(st.syntax());
    let shape = StateShape::Plain;
    insert_state_entry(sym, diags, name.clone(), span, parent, shape);

    let inner = parent.push(name.clone());
    for nested in st.nested_states() {
        visit_state(&nested, &inner, sym, diags);
    }
    for region in st.regions() {
        visit_region(&region, &inner, sym, diags);
    }
    for child in st.syntax().children() {
        collect_pseudo_states(&child, &inner, sym, diags);
    }
}

fn visit_region(
    region: &ast::RegionDecl,
    parent: &Scope,
    sym: &mut MachineSymbols,
    diags: &mut Vec<Diagnostic>,
) {
    let name = region.name().unwrap_or_default();
    let span = span_of(region.syntax());
    // Regions live in a separate table so checks can iterate just regions.
    sym.regions.push(StateEntry {
        name: name.clone(),
        span,
        stable_id: None,
        container_path: parent.container_path.clone(),
        shape: StateShape::Plain,
    });

    let inner = parent.push(name);
    for nested in region.states() {
        visit_state(&nested, &inner, sym, diags);
    }
    for child in region.syntax().children() {
        collect_pseudo_states(&child, &inner, sym, diags);
    }
}

/// Walk a CST subtree and add `final` / `history` / `choice` / `junction` /
/// `fork` / `join` / `entry_point` / `exit_point` pseudo-states to the symbol
/// table. The AST exposes typed accessors for nested states/regions only —
/// other forms must be discovered via the CST.
fn collect_pseudo_states(
    node: &fsm_parser::cst::SyntaxNode,
    container: &Scope,
    sym: &mut MachineSymbols,
    diags: &mut Vec<Diagnostic>,
) {
    use fsm_parser::cst::SyntaxKind as K;
    let (name_opt, shape_opt): (Option<String>, Option<StateShape>) = match node.kind() {
        K::FINAL_DECL => (
            ast::FinalDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::Final),
        ),
        K::SHALLOW_HISTORY_DECL => (
            ast::ShallowHistoryDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::ShallowHistory),
        ),
        K::DEEP_HISTORY_DECL => (
            ast::DeepHistoryDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::DeepHistory),
        ),
        K::CHOICE_DECL => (
            ast::ChoiceDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::Choice),
        ),
        K::JUNCTION_DECL => (
            ast::JunctionDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::Junction),
        ),
        K::FORK_DECL => (
            ast::ForkDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::Fork),
        ),
        K::JOIN_DECL => (
            ast::JoinDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::Join),
        ),
        K::ENTRY_POINT_DECL => (
            ast::EntryPointDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::EntryPoint),
        ),
        K::EXIT_POINT_DECL => (
            ast::ExitPointDecl::cast(node.clone()).and_then(|n| n.name()),
            Some(StateShape::ExitPoint),
        ),
        _ => (None, None),
    };
    if let (Some(name), Some(shape)) = (name_opt, shape_opt) {
        insert_state_entry(sym, diags, name, span_of(node), container, shape);
    }
}

fn insert_state_entry(
    sym: &mut MachineSymbols,
    diags: &mut Vec<Diagnostic>,
    name: String,
    span: Span,
    container: &Scope,
    shape: StateShape,
) {
    if name.is_empty() {
        return;
    }
    // Duplicate-name check is scoped to the same `container_path` — different
    // parents may declare same-named children per Doc 04 §5.
    if let Some(prev) = sym
        .states
        .iter()
        .find(|s| s.name == name && s.container_path == container.container_path)
    {
        diags.push(
            Diagnostic::new(DiagnosticCode::E0021, span).with_related(RelatedInfo {
                message: format!("previous state '{}' here", prev.name),
                span: prev.span,
            }),
        );
        return;
    }
    sym.states.push(StateEntry {
        name,
        span,
        stable_id: None,
        container_path: container.container_path.clone(),
        shape,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ast(src: &str) -> fsm_parser::ParseResult {
        fsm_parser::parse(src)
    }

    #[test]
    fn empty_file_yields_empty_table() {
        let pr = parse_ast("language fsm 2.0");
        let (st, diags) = SymbolTable::build(&pr.ast());
        assert!(st.machines.is_empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn duplicate_machine_name_emits_e0020() {
        let pr = parse_ast("language fsm 2.0\nmachine M { }\nmachine M { }");
        let (_st, diags) = SymbolTable::build(&pr.ast());
        assert!(diags.iter().any(|d| d.code == DiagnosticCode::E0020));
    }

    #[test]
    fn duplicate_event_emits_e0022() {
        let src = "language fsm 2.0\nmachine M { events { START   START } initial S state S { } }";
        let pr = parse_ast(src);
        let (_st, diags) = SymbolTable::build(&pr.ast());
        assert!(diags.iter().any(|d| d.code == DiagnosticCode::E0022));
    }

    #[test]
    fn duplicate_state_emits_e0021() {
        let src = "language fsm 2.0\nmachine M { initial A state A { } state A { } }";
        let pr = parse_ast(src);
        let (_st, diags) = SymbolTable::build(&pr.ast());
        assert!(diags.iter().any(|d| d.code == DiagnosticCode::E0021));
    }

    #[test]
    fn duplicate_extern_emits_e0024() {
        let src = "language fsm 2.0\nmachine M { extern foo() : bool\nextern foo() : bool\ninitial S state S { } }";
        let pr = parse_ast(src);
        let (_st, diags) = SymbolTable::build(&pr.ast());
        assert!(diags.iter().any(|d| d.code == DiagnosticCode::E0024));
    }
}
