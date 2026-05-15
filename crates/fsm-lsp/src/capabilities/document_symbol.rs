//! `textDocument/documentSymbol` projection — Doc 26 §5 / §8 L2.
//!
//! Builds the Doc 14 §13 hierarchical symbol tree. **No analysis happens
//! here** — this module is a pure projection of the analyzer's
//! [`SymbolTable`] (the *same* one the debounced `publishDiagnostics`
//! produced, threaded through [`crate::analysis::Analysis`]; Doc 26 §8 L2:
//! "one analysis feeds both"). It re-runs no symbol extraction and adds no
//! second position converter — every range goes through L1's
//! [`LineIndex`] in the negotiated encoding (Doc 26 §4.1).
//!
//! ## The seam, exactly (Doc 26 §5 `documentSymbol` row)
//!
//! Per machine, the analyzer hands us ordered tables: `events` /
//! `externs` / `consts` / `enums` / `context_fields` / `states` /
//! `regions`. Each carries a full-declaration byte [`Span`]; `StateEntry`
//! additionally carries `container_path` (the dotted list of containing
//! state/region *names*, machine-root = empty) and a coarse
//! [`StateShape`]. The Doc 14 §13 tree is assembled from those three
//! facts:
//!
//! - the machine is the root [`SymbolKind::MODULE`] node;
//! - category group nodes (`context`/`events`/`externs`/`consts`/`enums`)
//!   mirror Doc 14 §13's grouping verbatim;
//! - the state hierarchy is reconstructed from `container_path`: an entry
//!   whose `container_path` is `P` is a child of the entry at path `P`
//!   (a region is itself such an entry, so a region nests under its
//!   composite and the region's states nest under the region — exactly
//!   the statechart shape);
//! - **composite vs simple** is *derived*, not read: `StateShape::Plain`
//!   with ≥1 child ⇒ composite, with 0 children ⇒ simple. The
//!   `SymbolTable` deliberately does not store this (its own doc-comment
//!   says Composite/Simple "is resolved at lowering time"); deriving it
//!   from the already-built `container_path` graph is the documented seam
//!   and adds no analysis. Pseudo-state shapes (`Final`/`Choice`/`Fork`/…)
//!   come straight off `StateShape`.
//!
//! ## `range` vs `selectionRange`
//!
//! `range` = the full declaration span (`Entry::span`/`StateEntry::span`,
//! which is `span_of(decl_node)` — the whole `state X { … }`). LSP
//! requires `selectionRange` (the name) ⊆ `range`. The `SymbolTable` does
//! not store a separate name span, so the name token is located by the
//! **same rule the table itself used to derive the name**: the first
//! `Ident` token within the declaration span (mirrors
//! `fsm_parser::ast::first_ident`). This is the standard LSP
//! name-locating step, not symbol re-extraction — Doc 26 §5 scopes
//! `documentSymbol` as "tree assembly + Span→Range only" and the
//! name-token lookup is part of producing the `selectionRange`. If the
//! name token cannot be found (malformed input — the resilient parser can
//! yield a nameless decl, Doc 26 §4.4/risk-3) `selectionRange` falls back
//! to `range`, which is always valid and never panics.

use tower_lsp::lsp_types::{DocumentSymbol, Range, SymbolKind};

use fsm_analyzer::symbol_table::{Entry, MachineSymbols, StateEntry, StateShape, SymbolTable};
use fsm_diagnostics::Span;
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::position::{LineIndex, OffsetEncoding};

/// Build the Doc 14 §13 hierarchical `DocumentSymbol` tree for one buffer.
///
/// `cst` is the parse tree root (used only to locate name tokens for
/// `selectionRange`); `line_index`/`text`/`encoding` are L1's position
/// boundary. Returns one root [`DocumentSymbol`] per declared machine.
pub fn document_symbols(
    table: &SymbolTable,
    cst: &SyntaxNode,
    line_index: &LineIndex,
    text: &str,
    encoding: OffsetEncoding,
) -> Vec<DocumentSymbol> {
    table
        .machines
        .iter()
        .map(|m| machine_symbol(m, cst, line_index, text, encoding))
        .collect()
}

/// One machine → its Doc 14 §13 subtree.
fn machine_symbol(
    m: &MachineSymbols,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> DocumentSymbol {
    let range = li.range(text, m.span, enc);
    let selection = name_range(cst, m.span, li, text, enc).unwrap_or(range);

    let mut children: Vec<DocumentSymbol> = Vec::new();

    // --- context fields group (Doc 14 §13 "context") ---------------------
    if !m.context_fields.is_empty() {
        let kids: Vec<DocumentSymbol> = m
            .context_fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let detail = m
                    .context_field_types
                    .get(i)
                    .and_then(|t| t.clone())
                    .map(|ty| format!("{}: {ty}", f.name));
                leaf(f, SymbolKind::FIELD, detail, cst, li, text, enc)
            })
            .collect();
        children.push(group("context", SymbolKind::NAMESPACE, range, kids));
    }

    // --- events group ----------------------------------------------------
    if !m.events.is_empty() {
        let kids = simple_leaves(&m.events, SymbolKind::EVENT, cst, li, text, enc);
        children.push(group("events", SymbolKind::NAMESPACE, range, kids));
    }

    // --- externs group ---------------------------------------------------
    if !m.externs.is_empty() {
        let kids: Vec<DocumentSymbol> = m
            .externs
            .iter()
            .enumerate()
            .map(|(i, e)| {
                // Doc 14 §13 annotates pure externs "(pure)".
                let detail = if *m.extern_pure.get(i).unwrap_or(&false) {
                    Some(format!("{} (pure)", e.name))
                } else {
                    None
                };
                leaf(e, SymbolKind::FUNCTION, detail, cst, li, text, enc)
            })
            .collect();
        children.push(group("externs", SymbolKind::NAMESPACE, range, kids));
    }

    // --- consts group ----------------------------------------------------
    if !m.consts.is_empty() {
        let kids = simple_leaves(&m.consts, SymbolKind::CONSTANT, cst, li, text, enc);
        children.push(group("consts", SymbolKind::NAMESPACE, range, kids));
    }

    // --- enums group -----------------------------------------------------
    if !m.enums.is_empty() {
        let kids: Vec<DocumentSymbol> = m
            .enums
            .iter()
            .map(|e| {
                let r = li.range(text, e.span, enc);
                let sel = name_range(cst, e.span, li, text, enc).unwrap_or(r);
                #[allow(deprecated)]
                DocumentSymbol {
                    name: e.name.clone(),
                    detail: None,
                    kind: SymbolKind::ENUM,
                    tags: None,
                    deprecated: None,
                    range: r,
                    selection_range: sel,
                    children: None,
                }
            })
            .collect();
        children.push(group("enums", SymbolKind::NAMESPACE, range, kids));
    }

    // --- states + pseudo-states (the hierarchy, Doc 14 §13) --------------
    let forest = build_state_forest(m, cst, li, text, enc);
    if !forest.real_roots.is_empty() {
        children.push(group(
            "states",
            SymbolKind::NAMESPACE,
            range,
            forest.real_roots,
        ));
    }
    if !forest.pseudo_roots.is_empty() {
        children.push(group(
            "pseudo-states",
            SymbolKind::NAMESPACE,
            range,
            forest.pseudo_roots,
        ));
    }

    #[allow(deprecated)]
    DocumentSymbol {
        name: m.name.clone(),
        detail: None,
        kind: SymbolKind::MODULE,
        tags: None,
        deprecated: None,
        range,
        selection_range: selection,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

/// A category group node (`context`/`events`/…). Doc 14 §13 renders these
/// as intermediate tree nodes; LSP has no "group" kind so [`NAMESPACE`] is
/// used (the conventional choice for synthetic grouping). Its `range`/
/// `selectionRange` is the enclosing machine span — it has no own source
/// span, and a group must still carry a valid (machine-contained) range.
fn group(
    name: &str,
    kind: SymbolKind,
    enclosing: Range,
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    #[allow(deprecated)]
    DocumentSymbol {
        name: name.to_owned(),
        detail: None,
        kind,
        tags: None,
        deprecated: None,
        range: enclosing,
        selection_range: enclosing,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

/// Project a flat `Entry` table to leaf `DocumentSymbol`s of one kind.
fn simple_leaves(
    entries: &[Entry],
    kind: SymbolKind,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Vec<DocumentSymbol> {
    entries
        .iter()
        .map(|e| leaf(e, kind, None, cst, li, text, enc))
        .collect()
}

/// One leaf (childless) symbol from a generic `Entry`.
fn leaf(
    e: &Entry,
    kind: SymbolKind,
    detail: Option<String>,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> DocumentSymbol {
    let r = li.range(text, e.span, enc);
    let sel = name_range(cst, e.span, li, text, enc).unwrap_or(r);
    #[allow(deprecated)]
    DocumentSymbol {
        name: e.name.clone(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: r,
        selection_range: sel,
        children: None,
    }
}

/// Result of reconstructing the state hierarchy from `container_path`:
/// the two Doc 14 §13 top-level buckets (`states` and `pseudo-states`).
struct StateForest {
    real_roots: Vec<DocumentSymbol>,
    pseudo_roots: Vec<DocumentSymbol>,
}

/// A node in the working hierarchy before it is split into the two Doc 14
/// §13 buckets. Built bottom-up from the `container_path` graph.
struct Node<'a> {
    entry: &'a StateEntry,
    /// Dotted path *to this node* (its own `container_path` + its name).
    self_path: Vec<String>,
    children: Vec<usize>,
}

/// Reconstruct the Doc 14 §13 state tree from the analyzer's `states` +
/// `regions` tables via `container_path`. Pure graph assembly — **no
/// analysis** (the parent/child edges already exist implicitly in the
/// table the single analysis produced).
///
/// Algorithm: every `states`/`regions` entry becomes a node keyed by its
/// own dotted path; an entry whose `container_path` equals some node's
/// `self_path` is that node's child; entries with empty `container_path`
/// are machine-root. A real-state root with children is composite, without
/// is simple; pseudo-state roots go to the `pseudo-states` bucket. Nested
/// pseudo-states stay under their structural container (most-natural and
/// matches the hierarchy the statechart actually has).
fn build_state_forest(
    m: &MachineSymbols,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> StateForest {
    // All state-like entries, regions included (a region is a container in
    // the path graph exactly like a composite state).
    let all: Vec<&StateEntry> = m.states.iter().chain(m.regions.iter()).collect();

    let mut nodes: Vec<Node> = all
        .iter()
        .map(|e| {
            let mut self_path = e.container_path.clone();
            self_path.push(e.name.clone());
            Node {
                entry: e,
                self_path,
                children: Vec::new(),
            }
        })
        .collect();

    // Wire parent → child edges. A node N's parent is the (first) node P
    // whose `self_path == N.entry.container_path`. O(n²) — fine for the
    // hundreds-of-states embedded files this targets (Doc 26 §4.4).
    let mut roots: Vec<usize> = Vec::new();
    for i in 0..nodes.len() {
        let cpath = nodes[i].entry.container_path.clone();
        if cpath.is_empty() {
            roots.push(i);
            continue;
        }
        match nodes.iter().position(|p| p.self_path == cpath) {
            Some(parent) => nodes[parent].children.push(i),
            // A container_path that resolves to nothing (only on broken
            // input — every well-formed nesting has a real container in
            // the same table) is surfaced at root rather than dropped, so
            // the symbol is never silently lost (Doc 26 cardinal sin).
            None => roots.push(i),
        }
    }

    let mut real_roots = Vec::new();
    let mut pseudo_roots = Vec::new();
    for &r in &roots {
        let sym = node_to_symbol(r, &nodes, cst, li, text, enc);
        if is_pseudo(nodes[r].entry.shape) {
            pseudo_roots.push(sym);
        } else {
            real_roots.push(sym);
        }
    }
    StateForest {
        real_roots,
        pseudo_roots,
    }
}

/// Recursively turn a hierarchy node into a `DocumentSymbol`, deriving
/// composite-vs-simple from child count (the documented seam — the
/// `SymbolTable` does not store it).
fn node_to_symbol(
    idx: usize,
    nodes: &[Node],
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> DocumentSymbol {
    let n = &nodes[idx];
    let e = n.entry;
    let child_syms: Vec<DocumentSymbol> = n
        .children
        .iter()
        .map(|&c| node_to_symbol(c, nodes, cst, li, text, enc))
        .collect();

    let has_children = !child_syms.is_empty();
    let (kind, detail) = classify_state(e.shape, has_children, &e.name);

    let r = li.range(text, e.span, enc);
    let sel = name_range(cst, e.span, li, text, enc).unwrap_or(r);
    #[allow(deprecated)]
    DocumentSymbol {
        name: e.name.clone(),
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: r,
        selection_range: sel,
        children: if child_syms.is_empty() {
            None
        } else {
            Some(child_syms)
        },
    }
}

/// `true` for the pseudo-state shapes (everything except a `Plain` real
/// state). Doc 14 §13 splits these into the separate `pseudo-states`
/// bucket *at machine root*; nested ones keep their structural parent.
fn is_pseudo(shape: StateShape) -> bool {
    !matches!(shape, StateShape::Plain)
}

/// Map a (`StateShape`, has-children) pair to its LSP `SymbolKind` + the
/// Doc 14 §13 parenthetical detail (`(simple)`/`(composite)`/`(choice)`/…).
/// Composite-vs-simple is derived from `has_children` (the seam — see the
/// module doc-comment); pseudo shapes come straight off `StateShape`.
fn classify_state(
    shape: StateShape,
    has_children: bool,
    name: &str,
) -> (SymbolKind, Option<String>) {
    match shape {
        StateShape::Plain => {
            if has_children {
                // A composite state is a namespace-like container.
                (SymbolKind::CLASS, Some(format!("{name} (composite)")))
            } else {
                (SymbolKind::CLASS, Some(format!("{name} (simple)")))
            }
        }
        StateShape::Final => (SymbolKind::ENUM_MEMBER, Some(format!("{name} (final)"))),
        StateShape::ShallowHistory => (
            SymbolKind::OPERATOR,
            Some(format!("{name} (shallow history)")),
        ),
        StateShape::DeepHistory => (SymbolKind::OPERATOR, Some(format!("{name} (deep history)"))),
        StateShape::Choice => (SymbolKind::OPERATOR, Some(format!("{name} (choice)"))),
        StateShape::Junction => (SymbolKind::OPERATOR, Some(format!("{name} (junction)"))),
        StateShape::Fork => (SymbolKind::OPERATOR, Some(format!("{name} (fork)"))),
        StateShape::Join => (SymbolKind::OPERATOR, Some(format!("{name} (join)"))),
        StateShape::EntryPoint => (SymbolKind::OPERATOR, Some(format!("{name} (entry point)"))),
        StateShape::ExitPoint => (SymbolKind::OPERATOR, Some(format!("{name} (exit point)"))),
    }
}

/// Locate the **name token** within a declaration's byte `Span` and return
/// its LSP `Range` (the `selectionRange`).
///
/// The name is the first `Ident` token whose start is inside `decl_span`
/// — the *exact* rule `fsm_parser::ast::first_ident` (and therefore the
/// `SymbolTable` builder) used to derive the name string, so the
/// `selectionRange` is byte-consistent with the symbol's `name`. Walking
/// tokens of the covering declaration node (not a fresh parse) keeps this
/// a pure projection. `None` (→ caller falls back to the full `range`,
/// always valid) iff no `Ident` is found — only possible on the resilient
/// parser's malformed-input output (Doc 26 §4.4/risk-3); never panics.
fn name_range(
    cst: &SyntaxNode,
    decl_span: Span,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<Range> {
    let lo = decl_span.start as u32;
    let hi = decl_span.end as u32;
    // The first Ident token whose start byte lies within the declaration
    // span. Tokens are visited in source order, so this is the *name*
    // (the first identifier of `machine M`, `state X`, `event E`, …) —
    // keyword tokens (`machine`/`state`/…) are not `Ident`, so they are
    // correctly skipped exactly as `first_ident` skips them.
    for el in cst.descendants_with_tokens() {
        if let Some(tok) = el.as_token() {
            if tok.kind() != SyntaxKind::Ident {
                continue;
            }
            let tr = tok.text_range();
            let start = u32::from(tr.start());
            if start >= lo && start < hi {
                let span = Span::new(u32::from(tr.start()) as usize, u32::from(tr.end()) as usize);
                return Some(li.range(text, span, enc));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    /// Resolve a symbol path like `["VendingMachine","states","Operational"]`
    /// down the nested tree, returning the leaf node. Test-only navigation
    /// helper (so assertions read declaratively).
    fn find<'a>(roots: &'a [DocumentSymbol], path: &[&str]) -> Option<&'a DocumentSymbol> {
        let mut cur: Option<&DocumentSymbol> = None;
        let mut level = roots;
        for seg in path {
            let found = level.iter().find(|s| s.name == *seg)?;
            cur = Some(found);
            level = found.children.as_deref().unwrap_or(&[]);
        }
        cur
    }

    fn build(src: &str) -> Vec<DocumentSymbol> {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let pr = fsm_parser::parse(src);
        let li = LineIndex::new(src);
        document_symbols(
            &a.symbol_table,
            &pr.syntax(),
            &li,
            src,
            OffsetEncoding::Utf8,
        )
    }

    #[test]
    fn flat_machine_groups_and_kinds() {
        let src = "language fsm 2.0\nmachine M {\n  events { E }\n  initial S\n  state S {}\n}\n";
        let syms = build(src);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "M");
        assert_eq!(syms[0].kind, SymbolKind::MODULE);
        // events group present with one EVENT leaf.
        let e = find(&syms, &["M", "events", "E"]).expect("event E in tree");
        assert_eq!(e.kind, SymbolKind::EVENT);
        // S is a simple state under the `states` group.
        let s = find(&syms, &["M", "states", "S"]).expect("state S in tree");
        assert_eq!(s.kind, SymbolKind::CLASS);
        assert_eq!(s.detail.as_deref(), Some("S (simple)"));
    }

    #[test]
    fn composite_is_derived_from_children_not_shape() {
        let src = "language fsm 2.0\nmachine M {\n  initial Outer\n  state Outer {\n    initial Inner\n    state Inner {}\n  }\n}\n";
        let syms = build(src);
        let outer = find(&syms, &["M", "states", "Outer"]).expect("Outer");
        assert_eq!(outer.detail.as_deref(), Some("Outer (composite)"));
        let inner = find(&syms, &["M", "states", "Outer", "Inner"]).expect("Inner nested");
        assert_eq!(inner.detail.as_deref(), Some("Inner (simple)"));
    }

    #[test]
    fn selection_range_is_the_name_not_the_whole_decl() {
        let src = "language fsm 2.0\nmachine Mm {\n  initial S\n  state S {}\n}\n";
        let syms = build(src);
        let m = &syms[0];
        // Full range spans the whole `machine Mm { … }`; the selection
        // range is just "Mm" (4 chars wide on its line) and is contained.
        assert!(m.selection_range.start.line >= m.range.start.line);
        assert!(m.selection_range.end.line <= m.range.end.line);
        // "Mm" starts at byte 25 on line 1 col 8 (after "machine ").
        assert_eq!(m.selection_range.start.line, 1);
        assert_eq!(m.selection_range.start.character, 8);
        assert_eq!(m.selection_range.end.character, 10);
        assert_ne!(m.selection_range, m.range);
    }
}
