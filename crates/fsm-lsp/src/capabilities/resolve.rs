//! Token-at-cursor → symbol resolution — the shared L3 seam (Doc 26 §5 /
//! §8 L3) feeding **both** `definition` and `hover`.
//!
//! ## Why one shared resolver
//!
//! `textDocument/definition` and `textDocument/hover` ask the *same*
//! question — "what declared entity is the identifier under the cursor?" —
//! and differ only in the answer's shape (a decl `Location` vs Markdown).
//! Doing the resolution once here keeps the two handlers byte-consistent
//! with each other and, crucially, byte-consistent with the diagnostics:
//! the classification mirrors the analyzer's own
//! `checks::name_resolution` dispatch (the SAME `SyntaxKind` arms) and the
//! resolution goes through the **same** `SymbolTable::resolve_*` methods
//! `fsm check` uses (`name_resolution.rs`). So a goto/hover can no more
//! disagree with `fsm check` than a squiggle can (Doc 26 §3 invariant
//! extended to L3) — there is no parallel resolver.
//!
//! ## Token-at-position mechanism (Doc 26 §5 "token-at-position via CST")
//!
//! The cursor `Position` is mapped to a byte via L1's `LineIndex` inverse
//! (`position::LineIndex::offset` — the reverse of the ONE authoritative
//! converter, not a second one), then rowan's `token_at_offset` yields the
//! `Ident` token covering that byte. The token's **node ancestry** (its
//! parent `SyntaxKind` chain) classifies it exactly as the analyzer
//! classifies the same token when emitting `FSM-E0100..E0104`: a direct
//! ident of a `TRANSITION_DECL`/`LOCAL_DECL` at index 1 is a state-ref
//! target; the ident inside `EXPR_FIELD_REF` after `ctx.` is a
//! context-field ref; an `on E`/`raise E`/`send E`/`defer E` ident is an
//! event ref; an `EXPR_CALL` callee is an extern ref; etc. **Single-file
//! only** (Doc 26 §4.6): every resolution is within the cursor's enclosing
//! machine via the per-file `SymbolTable`; there is no project index, so a
//! symbol that does not resolve in-file yields `None` (graceful
//! degradation per Doc 14 §8 — never a fabricated or cross-file location;
//! cross-file is explicitly v1.3, Doc 26 §4.6/§9).

use fsm_analyzer::scope::Scope;
use fsm_analyzer::symbol_table::{Entry, StateEntry, SymbolTable};
use fsm_diagnostics::Span;
use fsm_parser::cst::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::position::{LineIndex, OffsetEncoding};

/// What kind of declared entity the cursor identifier refers to, paired
/// with everything `definition` (decl `Span`) and `hover` (kind + name +
/// enrichment key) need. The variants mirror the analyzer's
/// name-resolution categories one-to-one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// A `state`/pseudo-state (transition target, `initial`, fork/join,
    /// history default, choice/junction branch, timer target, …).
    State {
        /// Index into `SymbolTable::machines` of the enclosing machine.
        machine: usize,
        name: String,
        /// The declaration's full byte span (`StateEntry::span`).
        decl_span: Span,
    },
    /// An `event` (trigger `on E`, `raise E`, `send E`, `defer E`).
    Event {
        machine: usize,
        name: String,
        decl_span: Span,
    },
    /// An `extern` (call in a guard or action block).
    Extern {
        machine: usize,
        name: String,
        decl_span: Span,
        /// `pure` externs are usable in guards (Doc 04 §2.5) — hover
        /// surfaces this exactly as `documentSymbol` does.
        is_pure: bool,
    },
    /// A `ctx.field` context field.
    ContextField {
        machine: usize,
        name: String,
        decl_span: Span,
    },
    /// A machine name (the `M` in `send E to M`).
    Machine { name: String, decl_span: Span },
}

impl Resolved {
    /// The declaration site to jump to (`textDocument/definition`).
    pub fn decl_span(&self) -> Span {
        match self {
            Resolved::State { decl_span, .. }
            | Resolved::Event { decl_span, .. }
            | Resolved::Extern { decl_span, .. }
            | Resolved::ContextField { decl_span, .. }
            | Resolved::Machine { decl_span, .. } => *decl_span,
        }
    }
}

/// Resolve the identifier at `byte` to its declaration, single-file.
///
/// Returns `None` when the cursor is not on a resolvable identifier
/// (whitespace, a keyword, punctuation, an unknown/cross-file name, or a
/// declaration site itself rather than a use site). `None` is the
/// spec-correct "no result" — never a panic, never a guessed location
/// (Doc 14 §8 single-file degradation).
pub fn resolve_at(table: &SymbolTable, cst: &SyntaxNode, byte: u32) -> Option<Resolved> {
    let tok = ident_token_at(cst, byte)?;
    let name = tok.text().to_string();

    // The enclosing machine (single-file scope, Doc 26 §4.6). A token
    // outside any `machine` (e.g. in a file-level `const`) has no machine
    // scope -> not resolvable as a state/event/extern/ctx ref here.
    let machine_node = enclosing(&tok, SyntaxKind::MACHINE_DECL);

    classify(table, &tok, &name, machine_node.as_ref())
}

/// The `Ident` token whose byte range covers `byte`.
///
/// `token_at_offset` can yield up to two tokens at a boundary (the cursor
/// sitting exactly between two tokens). We prefer an `Ident` (the only
/// resolvable token kind) over an adjacent keyword/punctuation so a cursor
/// at the very start of a name still resolves it. A non-`Ident` position
/// (whitespace, operator, keyword) yields `None`.
fn ident_token_at(cst: &SyntaxNode, byte: u32) -> Option<SyntaxToken> {
    let offset = rowan::TextSize::from(byte);
    match cst.token_at_offset(offset) {
        rowan::TokenAtOffset::None => None,
        rowan::TokenAtOffset::Single(t) => (t.kind() == SyntaxKind::Ident).then_some(t),
        rowan::TokenAtOffset::Between(a, b) => {
            if a.kind() == SyntaxKind::Ident {
                Some(a)
            } else if b.kind() == SyntaxKind::Ident {
                Some(b)
            } else {
                None
            }
        }
    }
}

/// First ancestor node (inclusive of the token's own parent) of `kind`.
fn enclosing(tok: &SyntaxToken, kind: SyntaxKind) -> Option<SyntaxNode> {
    tok.parent_ancestors().find(|n| n.kind() == kind)
}

/// Classify the ident by its CST context, mirroring `name_resolution.rs`.
fn classify(
    table: &SymbolTable,
    tok: &SyntaxToken,
    name: &str,
    machine_node: Option<&SyntaxNode>,
) -> Option<Resolved> {
    let parent = tok.parent()?;
    let pkind = parent.kind();

    // The cursor's machine index in the symbol table. Resolved by NAME via
    // the table's own `machine_index` (the SAME map `resolve_machine`
    // uses), so it stays correct regardless of declaration order.
    let machine_name = machine_node.and_then(machine_name_of);
    let machine_idx = machine_name
        .as_deref()
        .and_then(|n| table.machine_index.get(n).copied());

    // --- ctx.field / Machine.* qualified refs (EXPR_FIELD_REF) -----------
    // `EXPR_FIELD_REF` = <lhs node> Dot Ident. The analyzer treats the
    // trailing ident as a ctx-field ref iff the lhs ident is `ctx`
    // (`name_resolution.rs::check_expr`). We resolve the trailing ident the
    // same way; the `ctx`/`payload`/enum prefix ident itself is not a
    // resolvable declaration target (matches the analyzer not diagnosing
    // it). Only the field name (the direct Ident child after the Dot).
    if pkind == SyntaxKind::EXPR_FIELD_REF {
        let mi = machine_idx?;
        // Is THIS token the trailing field ident (the resolvable one)?
        let is_field_ident = field_ref_rhs_ident(&parent)
            .map(|t| t.text_range() == tok.text_range())
            .unwrap_or(false);
        if !is_field_ident {
            return None; // the `ctx`/prefix part — analyzer does not resolve it
        }
        let lhs = field_ref_lhs_ident(&parent)?;
        if lhs == "ctx" {
            let scope = Scope::for_machine(mi);
            let e = table.resolve_context_field(name, &scope)?;
            return Some(Resolved::ContextField {
                machine: mi,
                name: name.to_owned(),
                decl_span: e.span,
            });
        }
        // `payload.x` / `Enum.Variant` / submachine field — not a
        // single-decl jump target (the analyzer doesn't resolve these to a
        // declaration either). Degrade to None, never guess.
        return None;
    }

    // --- send E to M : the machine name (last ident of STMT_SEND) -------
    if pkind == SyntaxKind::STMT_SEND {
        // Grammar: `send EVENT [(args)] to MACHINE`. The analyzer resolves
        // the FIRST ident as the event and the LAST as the machine when
        // there are >=2 idents (`check_stmt_send`). Match that split.
        let idents: Vec<SyntaxToken> = direct_ident_tokens(&parent);
        let this = tok.text_range();
        let is_first = idents.first().map(|t| t.text_range()) == Some(this);
        let is_last = idents.last().map(|t| t.text_range()) == Some(this);
        if idents.len() >= 2 && is_last && !is_first {
            let m = table.resolve_machine(name)?;
            return Some(Resolved::Machine {
                name: name.to_owned(),
                decl_span: m.span,
            });
        }
        if is_first {
            let mi = machine_idx?;
            let scope = Scope::for_machine(mi);
            let e = table.resolve_event(name, &scope)?;
            return Some(Resolved::Event {
                machine: mi,
                name: name.to_owned(),
                decl_span: e.span,
            });
        }
        return None;
    }

    // --- raise E / defer E (statement + DEFER_DECL) : event ref ---------
    if pkind == SyntaxKind::STMT_RAISE
        || pkind == SyntaxKind::STMT_DEFER
        || pkind == SyntaxKind::DEFER_DECL
    {
        // The analyzer resolves only the FIRST ident here.
        if !is_first_direct_ident(&parent, tok) {
            return None;
        }
        let mi = machine_idx?;
        return resolve_event(table, mi, name);
    }

    // --- call f(...) statement : extern ref ----------------------------
    if pkind == SyntaxKind::STMT_CALL {
        if !is_first_direct_ident(&parent, tok) {
            return None;
        }
        let mi = machine_idx?;
        return resolve_extern(table, mi, name, in_guard(tok));
    }

    // --- EXPR_NAME_REF : an extern reference -------------------------
    // Two cases resolve to an extern declaration, both verified against
    // the lowering (`lower/expr.rs`):
    //  (a) the callee child of an `EXPR_CALL` — `f(args)` in a guard or
    //      action (`name_resolution.rs::call_name`); the analyzer
    //      diagnoses an unknown one as E0102.
    //  (b) a **bare** name-ref directly inside a `GUARD_CLAUSE` — e.g.
    //      `[can_start]`. `lower_guard_expr`'s `NameRef` arm lowers this
    //      to `GuardExpr::ExternCall { callee, args: [] }` — it *is* a
    //      zero-arg pure-extern reference. The analyzer emits no
    //      diagnostic for a valid one (it only E0102-checks EXPR_CALL),
    //      so resolving a *declared* extern here can never disagree with
    //      a squiggle, and Doc 14 §6 explicitly wants "extern name in
    //      guard … → extern declaration".
    // Any other bare name-ref (a const/enum/operand the analyzer does
    // not resolve to one declaration) degrades to `None` — never a guess.
    if pkind == SyntaxKind::EXPR_NAME_REF {
        let grand = parent.parent();
        let is_callee = grand
            .as_ref()
            .map(|g| g.kind() == SyntaxKind::EXPR_CALL && is_call_callee(g, &parent))
            .unwrap_or(false);
        // Bare guard name-ref: parent EXPR_NAME_REF whose own parent is
        // the GUARD_CLAUSE (not an EXPR_* — that would be a sub-expression
        // operand, e.g. `[ok && x > 0]`'s `ok`, which still lowers to an
        // ExternCall and is equally a valid extern ref).
        let is_bare_guard_ref = !is_callee
            && grand.as_ref().map(|g| g.kind()) != Some(SyntaxKind::EXPR_CALL)
            && in_guard(tok)
            // Exclude a field-ref LHS / qualified-name part (handled by the
            // EXPR_FIELD_REF arm; here parent is EXPR_NAME_REF so it is a
            // standalone name, not `a.b`).
            && grand.as_ref().map(|g| g.kind()) != Some(SyntaxKind::EXPR_FIELD_REF);
        if is_callee || is_bare_guard_ref {
            let mi = machine_idx?;
            return resolve_extern(table, mi, name, in_guard(tok));
        }
        return None;
    }

    // --- direct-ident decls: transition / timer / fork-join / branch /
    //     history default / initial / entry_point — all STATE refs at a
    //     specific ident index, exactly per `name_resolution.rs`. ---------
    let mi = machine_idx;
    match pkind {
        // `[likely] EV -> TARGET` / `[likely] EV ~> TARGET`: ident 0 =
        // event trigger, ident 1 = state target (BRANCH_HINT nests its
        // ident in a child node, so direct-ident indices are unaffected —
        // exactly the invariant `TransitionDecl::target` relies on).
        SyntaxKind::TRANSITION_DECL | SyntaxKind::LOCAL_DECL => {
            match direct_ident_index(&parent, tok) {
                Some(0) => resolve_event(table, mi?, name),
                Some(1) => resolve_state(table, mi?, name),
                _ => None,
            }
        }
        // `internal on EV :` — ident 0 = event, no target.
        SyntaxKind::INTERNAL_DECL => {
            if is_first_direct_ident(&parent, tok) {
                resolve_event(table, mi?, name)
            } else {
                None
            }
        }
        // `done [g] -> TARGET` — first direct ident is the state target.
        SyntaxKind::COMPLETION_DECL => {
            if is_first_direct_ident(&parent, tok) {
                resolve_state(table, mi?, name)
            } else {
                None
            }
        }
        // `after N ms -> TARGET` / `every N ms -> TARGET` — the only direct
        // ident is the target state (the duration is IntLiteral + `ms`).
        SyntaxKind::AFTER_DECL | SyntaxKind::EVERY_DECL => resolve_state(table, mi?, name),
        // `initial TARGET` / history `default -> TARGET` (INITIAL_DECL is
        // reused for the history default node) / entry_point `name ->
        // TARGET`. INITIAL_DECL: only ident is the target. ENTRY_POINT_DECL:
        // ident 0 = the pseudo-state's own NAME (a declaration, not a
        // resolvable ref — the analyzer doesn't resolve it), ident 1 = the
        // target state.
        SyntaxKind::INITIAL_DECL => resolve_state(table, mi?, name),
        SyntaxKind::ENTRY_POINT_DECL => match direct_ident_index(&parent, tok) {
            Some(0) => None, // the entry_point's own name = a decl site
            Some(1) => resolve_state(table, mi?, name),
            _ => None,
        },
        // fork targets `{ A, B }` / join sources `{ A, B }` — every direct
        // ident is a state ref.
        SyntaxKind::FORK_TARGETS | SyntaxKind::JOIN_SOURCES => resolve_state(table, mi?, name),
        // `join { ... } -> TARGET` — the target is the LAST direct ident at
        // JOIN_DECL level (sources live in the JOIN_SOURCES child node, so
        // they are NOT direct idents of JOIN_DECL).
        SyntaxKind::JOIN_DECL => {
            let idents = direct_ident_tokens(&parent);
            if idents.last().map(|t| t.text_range()) == Some(tok.text_range()) {
                resolve_state(table, mi?, name)
            } else {
                None
            }
        }
        // `choice/junction` branch `[g] -> TARGET` — the branch's first
        // direct ident is the target state.
        SyntaxKind::CHOICE_BRANCH | SyntaxKind::JUNCTION_BRANCH => {
            if is_first_direct_ident(&parent, tok) {
                resolve_state(table, mi?, name)
            } else {
                None
            }
        }
        // A `default -> X` history default parses as an INITIAL_DECL
        // nested under SHALLOW/DEEP_HISTORY_DECL — already handled by the
        // INITIAL_DECL arm above. Anything unclassified is deliberately
        // `None` (declaration sites, keywords, `payload.`/enum operands),
        // never a guessed location — the single-file graceful degradation.
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Resolution shims — each goes through the EXACT `SymbolTable::resolve_*`
// `fsm check` uses (`name_resolution.rs`), so a goto can never disagree
// with a diagnostic.
// ---------------------------------------------------------------------------

fn resolve_state(table: &SymbolTable, mi: usize, name: &str) -> Option<Resolved> {
    let scope = Scope::for_machine(mi);
    let e: &StateEntry = table.resolve_state(name, &scope)?;
    Some(Resolved::State {
        machine: mi,
        name: name.to_owned(),
        decl_span: e.span,
    })
}

fn resolve_event(table: &SymbolTable, mi: usize, name: &str) -> Option<Resolved> {
    let scope = Scope::for_machine(mi);
    let e: &Entry = table.resolve_event(name, &scope)?;
    Some(Resolved::Event {
        machine: mi,
        name: name.to_owned(),
        decl_span: e.span,
    })
}

fn resolve_extern(table: &SymbolTable, mi: usize, name: &str, _in_guard: bool) -> Option<Resolved> {
    let scope = Scope::for_machine(mi);
    let (e, is_pure) = table.resolve_extern(name, &scope)?;
    Some(Resolved::Extern {
        machine: mi,
        name: name.to_owned(),
        decl_span: e.span,
        is_pure,
    })
}

// ---------------------------------------------------------------------------
// CST helpers (token discovery — pure traversal, no analysis).
// ---------------------------------------------------------------------------

/// Machine name from a `MACHINE_DECL` node — its first `Ident` token (the
/// exact rule `MachineDecl::name`/the symbol-table builder used).
fn machine_name_of(m: &SyntaxNode) -> Option<String> {
    m.children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
}

/// Every **direct** `Ident` token of `node` (not descendants — direct
/// children only, matching how the analyzer's per-decl accessors count
/// idents; a `BRANCH_HINT`/`GUARD_CLAUSE` child's idents are nested and so
/// correctly excluded).
fn direct_ident_tokens(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .collect()
}

/// 0-based index of `tok` among `node`'s direct `Ident` tokens, or `None`
/// if `tok` is not a direct ident child of `node`.
fn direct_ident_index(node: &SyntaxNode, tok: &SyntaxToken) -> Option<usize> {
    direct_ident_tokens(node)
        .iter()
        .position(|t| t.text_range() == tok.text_range())
}

fn is_first_direct_ident(node: &SyntaxNode, tok: &SyntaxToken) -> bool {
    direct_ident_index(node, tok) == Some(0)
}

/// The trailing field ident of an `EXPR_FIELD_REF` (the direct `Ident`
/// child after the `Dot`) — the analyzer's `rhs` in `check_expr`.
fn field_ref_rhs_ident(field_ref: &SyntaxNode) -> Option<SyntaxToken> {
    field_ref
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
}

/// The leftmost ident of an `EXPR_FIELD_REF`'s lhs subtree (the analyzer's
/// `lhs` in `check_expr` — `"ctx"`/`"payload"`/an enum/machine name).
fn field_ref_lhs_ident(field_ref: &SyntaxNode) -> Option<String> {
    field_ref.children().next().and_then(|child| {
        child
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text().to_string())
    })
}

/// Whether `name_ref` is the callee child of `call` (the first
/// `EXPR_NAME_REF` child — `name_resolution.rs::call_name`).
fn is_call_callee(call: &SyntaxNode, name_ref: &SyntaxNode) -> bool {
    call.children()
        .find(|c| c.kind() == SyntaxKind::EXPR_NAME_REF)
        .map(|c| c.text_range() == name_ref.text_range())
        .unwrap_or(false)
}

/// Whether `tok` sits inside a `GUARD_CLAUSE` (so an extern call there must
/// be `pure` — the analyzer's `in_guard`). Used only to colour hover; the
/// decl jump is identical guard-or-not.
fn in_guard(tok: &SyntaxToken) -> bool {
    tok.parent_ancestors()
        .any(|n| n.kind() == SyntaxKind::GUARD_CLAUSE)
}

/// Project a declaration byte `Span` to an LSP range via L1's `LineIndex`
/// in the negotiated encoding (no second position converter — Doc 26
/// §4.1 / §11.32 DRIFT-2 boundary untouched).
pub fn decl_range(
    span: Span,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> tower_lsp::lsp_types::Range {
    li.range(text, span, enc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn tbl(src: &str) -> (SymbolTable, SyntaxNode) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        (a.symbol_table, cst)
    }

    /// Byte offset of the first occurrence of `needle` in `src`, +`plus`.
    fn at(src: &str, needle: &str, plus: usize) -> u32 {
        (src.find(needle).expect("needle in src") + plus) as u32
    }

    #[test]
    fn transition_target_resolves_to_state_decl() {
        let src =
            "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {}\n}\n";
        let (t, cst) = tbl(src);
        // Cursor on the `B` in `-> B`.
        let off = at(src, "-> B", 3);
        let r = resolve_at(&t, &cst, off).expect("B resolves");
        match r {
            Resolved::State {
                name, decl_span, ..
            } => {
                assert_eq!(name, "B");
                // decl_span must be the `state B {}` declaration.
                let decl = &src[decl_span.start..decl_span.end];
                assert!(decl.starts_with("state B"), "got decl {decl:?}");
            }
            other => panic!("expected State, got {other:?}"),
        }
    }

    #[test]
    fn trigger_event_resolves_to_event_decl_not_state() {
        let src =
            "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> A\n  }\n}\n";
        let (t, cst) = tbl(src);
        let off = at(src, "on GO", 3); // the `GO` after `on`
        match resolve_at(&t, &cst, off).expect("GO resolves") {
            Resolved::Event {
                name, decl_span, ..
            } => {
                assert_eq!(name, "GO");
                assert!(src[decl_span.start..decl_span.end].contains("GO"));
            }
            other => panic!("expected Event, got {other:?}"),
        }
    }

    #[test]
    fn ctx_field_resolves_only_on_field_not_on_ctx_prefix() {
        // Valid grammar: guard `[ctx.n == 0]` BEFORE `->` (Doc 04 §8.5).
        let src = "language fsm 2.0\nmachine M {\n  context { n: u8 = 0 }\n  events { GO }\n  initial A\n  state A {\n    on GO [ctx.n == 0] -> A\n  }\n}\n";
        let (t, cst) = tbl(src);
        // On `n` of `ctx.n` -> ContextField.
        let on_field = at(src, "ctx.n", 4);
        match resolve_at(&t, &cst, on_field).expect("n resolves") {
            Resolved::ContextField { name, .. } => assert_eq!(name, "n"),
            other => panic!("expected ContextField, got {other:?}"),
        }
        // On `ctx` itself -> None (analyzer does not resolve the prefix).
        let on_ctx = at(src, "ctx.n", 0);
        assert!(
            resolve_at(&t, &cst, on_ctx).is_none(),
            "the `ctx` prefix is not a decl jump target"
        );
    }

    #[test]
    fn unknown_state_ref_degrades_to_none_not_a_guess() {
        // `-> Nope` where Nope is undeclared: `fsm check` emits E0100; the
        // resolver must return None (no fabricated location), exactly the
        // single-file graceful degradation.
        let src =
            "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> Nope\n  }\n}\n";
        let (t, cst) = tbl(src);
        let off = at(src, "-> Nope", 3);
        assert!(
            resolve_at(&t, &cst, off).is_none(),
            "an unresolved ref must yield None, never a wrong location"
        );
    }

    #[test]
    fn cursor_on_whitespace_or_keyword_is_none() {
        let src = "language fsm 2.0\nmachine M {\n  initial A\n  state A {}\n}\n";
        let (t, cst) = tbl(src);
        // The `state` keyword.
        let kw = at(src, "state A", 1);
        assert!(resolve_at(&t, &cst, kw).is_none(), "keyword -> None");
        // A space.
        let ws = at(src, "machine M", 7);
        assert!(resolve_at(&t, &cst, ws).is_none(), "whitespace -> None");
    }

    #[test]
    fn declaration_site_is_not_a_use_site() {
        // Cursor on the `A` of `state A {}` (the declaration itself) — a
        // decl is not a goto target (you are already there); the analyzer
        // never resolves a decl ident as a ref. Must be None.
        let src = "language fsm 2.0\nmachine M {\n  initial A\n  state A {}\n}\n";
        let (t, cst) = tbl(src);
        let off = at(src, "state A", 6);
        assert!(
            resolve_at(&t, &cst, off).is_none(),
            "a declaration ident is not a use-site jump target"
        );
    }

    #[test]
    fn send_event_and_machine_split_like_the_analyzer() {
        // Valid grammar: single action after `:` (no braces — Doc 04 §9).
        let src = "language fsm 2.0\nmachine Other {\n  events { PING }\n  initial X\n  state X {}\n}\nmachine M {\n  events { PING }\n  initial A\n  state A {\n    on PING -> A : send PING to Other\n  }\n}\n";
        let (t, cst) = tbl(src);
        // The first `PING` after `send` = event (machine M's PING).
        let ev = at(src, "send PING to Other", 5);
        match resolve_at(&t, &cst, ev).expect("send event resolves") {
            Resolved::Event { name, machine, .. } => {
                assert_eq!(name, "PING");
                // Resolved in machine M (index 1), not Other.
                assert_eq!(machine, t.machine_index["M"]);
            }
            other => panic!("expected Event, got {other:?}"),
        }
        // `Other` after `to` = machine ref.
        let m = at(src, "to Other", 3);
        match resolve_at(&t, &cst, m).expect("send machine resolves") {
            Resolved::Machine { name, decl_span } => {
                assert_eq!(name, "Other");
                assert!(src[decl_span.start..decl_span.end].starts_with("machine Other"));
            }
            other => panic!("expected Machine, got {other:?}"),
        }
    }
}
