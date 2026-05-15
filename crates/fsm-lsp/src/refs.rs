//! `ReferenceIndex` — the ONE genuinely-new analysis of L5 (Doc 26 §4.7 /
//! §8 L5). Powers `textDocument/references`, `prepareRename`, and `rename`.
//!
//! ## Why this is *derived*, not a second analysis (Doc 26 §4.7)
//!
//! `SymbolTable`'s `resolve_*` methods are **name → declaration** only;
//! there is no reverse (declaration → all use sites) index anywhere
//! in-tree (`symbol_table.rs:362-433`). `references`/`rename` need that
//! reverse direction. `ReferenceIndex` builds it as a **single CST walk**
//! over the outputs of the *one* `analyze()` the diagnostics/symbol/hover
//! path already ran — it runs **no** `analyze()` of its own, lowers **no**
//! IR, and adds **no** parallel resolver. For every `Ident` token in the
//! parse tree it asks the **exact same L3 question** the
//! `crate::capabilities::resolve` seam asks (`resolve_at`, which mirrors
//! `checks::name_resolution`'s dispatch and resolves through the SAME
//! `SymbolTable::resolve_*` `fsm check` uses) — so a "reference" here is a
//! *semantically resolved* reference, never a text/identifier-string
//! match. It cannot drift from `fsm check` because resolution still goes
//! through `SymbolTable` (Doc 26 §4.7, verbatim: "It reuses `SymbolTable`
//! for the resolution decision … not a parallel analyzer").
//!
//! ## Conservative-by-construction (Doc 26 risk-2, HIGH — the cardinal sin)
//!
//! `textDocument/rename` rewrites the user's source. A rename that touches
//! a same-spelled token that is NOT actually a reference to the target —
//! a different-scope symbol, a string-literal substring, a comment word, a
//! keyword — **silently corrupts their program** (the project's
//! silent-data-loss cardinal sin, most acute form). Every admission rule
//! below is therefore biased hard toward safety:
//!
//! - A token is admitted to a symbol's reference set **iff** it is an
//!   `Ident` token AND either (a) it is that symbol's **declaration-name
//!   token** (the first `Ident` of the decl whose `Span` the symbol table
//!   recorded — the exact rule `ast::first_ident` / the table builder used
//!   to derive the name string, so the name and this token are
//!   byte-consistent), or (b) `resolve_at` (the L3 classifier) resolves it
//!   to *exactly that declaration* (same `SymbolKey`).
//! - A token inside a **string literal**, a **comment**, or any **trivia**
//!   is never a CST `Ident` in a ref/decl position `resolve_at` classifies
//!   (it is a `StringLiteral` / `LineComment` / `BlockComment` / `DocComment`
//!   / whitespace token), so it is excluded *by construction* — there is
//!   no code path that could include it.
//! - A same-spelled token in a **different machine** (or otherwise a
//!   different scope) resolves — via `resolve_at`, using the correct
//!   enclosing-machine index the SAME way `resolve_machine` does — to a
//!   **different declaration `Span`** → a different `SymbolKey` → excluded.
//! - Anything `resolve_at` returns `None` for (keywords, punctuation,
//!   `payload.`/enum operands the analyzer does not resolve to one decl,
//!   unknown/cross-file names) is **not** a reference to anything.
//!
//! A missed reference is a usability annoyance; a wrong edit is a
//! catastrophe — they are NOT symmetric. When semantic resolution cannot
//! *prove* a token is the target, it is excluded.

use std::collections::HashMap;

use fsm_analyzer::symbol_table::SymbolTable;
use fsm_diagnostics::Span;
use fsm_parser::cst::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::capabilities::resolve::{resolve_at, Resolved};

/// A stable identity for one declared symbol within this single file.
///
/// Two tokens are references to the **same** symbol iff their resolved
/// `SymbolKey`s are equal. The declaration `Span` (from the `SymbolTable`,
/// which `resolve_at` returns via `Resolved::decl_span`) uniquely
/// identifies a declaration in one parsed buffer, and the kind +
/// machine-index discriminate categories so a state and an event that
/// happen to share a name+span boundary can never collide. This is the
/// semantic equality the entire L5 safety story rests on — it is *never* a
/// name-string comparison.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SymbolKey {
    /// A `state`/pseudo-state, identified by enclosing machine + decl span.
    State { machine: usize, decl: Span },
    /// An `event`.
    Event { machine: usize, decl: Span },
    /// An `extern` (machine-local or file-level — `resolve_extern`'s search
    /// order is honoured by `resolve_at`, so the resolved `decl` span is
    /// the authoritative identity regardless of which scope it lives in).
    Extern { machine: usize, decl: Span },
    /// A `ctx.field` context field.
    ContextField { machine: usize, decl: Span },
    /// A top-level `machine` name.
    Machine { decl: Span },
}

impl SymbolKey {
    /// `true` iff this symbol is **safely renameable** in v1.2 (Doc 26 §8
    /// L5 / Doc 14 §8). A `machine` rename has codegen/ABI blast radius
    /// (generated C type/function names, cross-file `send … to M`) beyond
    /// v1.2's single-file scope → **out of scope**, hard-rejected. Every
    /// other category (state / event / extern / context field) is a
    /// single-file in-scope rename.
    fn is_renameable_kind(&self) -> bool {
        !matches!(self, SymbolKey::Machine { .. })
    }

    /// The category of declared entity this key names. L6's semantic-tokens
    /// classifier maps a *declaration-name* token to its Doc 14 §10 legend
    /// type via this — reusing the SAME `SymbolKey` taxonomy L5's reference
    /// index already keys on (one entity model, no parallel one). The
    /// variants line up one-to-one with L3's [`crate::capabilities::resolve::
    /// Resolved`] (use sites), so a decl and a use of the same symbol get
    /// the identical legend type and differ only by the `declaration`
    /// modifier (Doc 14 §10 modifier 0) — exactly the Doc 26 §8 L6 decl-vs-ref
    /// requirement, decided by `symbol_table` identity, never re-classified.
    pub(crate) fn entity_kind(&self) -> EntityKind {
        match self {
            SymbolKey::State { .. } => EntityKind::State,
            SymbolKey::Event { .. } => EntityKind::Event,
            SymbolKey::Extern { .. } => EntityKind::Extern,
            SymbolKey::ContextField { .. } => EntityKind::ContextField,
            SymbolKey::Machine { .. } => EntityKind::Machine,
        }
    }
}

/// The five user-symbol categories the LSP distinguishes — the union of
/// L3's [`crate::capabilities::resolve::Resolved`] (use sites) and L5's
/// [`SymbolKey`] (declarations). L6 maps each to exactly one Doc 14 §10
/// semantic-token legend type. Defined here (next to `SymbolKey`, its
/// declaration-side source) so both the decl path
/// ([`SymbolKey::entity_kind`]) and the use path
/// ([`crate::capabilities::semantic_tokens`]'s `Resolved` bridge) share one
/// taxonomy — no second entity model, the Doc 26 §8 L6 reuse seam.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum EntityKind {
    /// `state`/pseudo-state name — Doc 14 §10 type index 1 (`type`).
    State,
    /// `event` name — Doc 14 §10 type index 2 (`enum`).
    Event,
    /// `extern` function name — Doc 14 §10 type index 3 (`function`).
    Extern,
    /// `ctx.field` context field — Doc 14 §10 type index 4 (`variable`).
    ContextField,
    /// `machine` name — Doc 14 §10 type index 0 (`namespace`).
    Machine,
}

/// Build a [`SymbolKey`] from an L3 [`Resolved`]. This is the *only*
/// bridge from "what the L3 classifier resolved" to "symbol identity" —
/// keeping the two in lockstep means a reference can never disagree with a
/// goto/hover (Doc 26 §3 invariant, extended to L5).
fn key_of(r: &Resolved) -> SymbolKey {
    match r {
        Resolved::State {
            machine, decl_span, ..
        } => SymbolKey::State {
            machine: *machine,
            decl: *decl_span,
        },
        Resolved::Event {
            machine, decl_span, ..
        } => SymbolKey::Event {
            machine: *machine,
            decl: *decl_span,
        },
        Resolved::Extern {
            machine, decl_span, ..
        } => SymbolKey::Extern {
            machine: *machine,
            decl: *decl_span,
        },
        Resolved::ContextField {
            machine, decl_span, ..
        } => SymbolKey::ContextField {
            machine: *machine,
            decl: *decl_span,
        },
        Resolved::Machine { decl_span, .. } => SymbolKey::Machine { decl: *decl_span },
    }
}

/// The [`EntityKind`] of an L3-resolved *use site*, routed through the SAME
/// [`key_of`] bridge L5's reference index uses (`Resolved` → `SymbolKey` →
/// kind). L6 calls this for every `resolve_at`-classified `Ident` so a use
/// site's legend type is decided by the identical taxonomy as its
/// declaration (just without the `declaration` modifier) — one classifier,
/// not two (Doc 26 §8 L6).
pub(crate) fn resolved_entity_kind(r: &Resolved) -> EntityKind {
    key_of(r).entity_kind()
}

/// One occurrence of a symbol — a byte range plus whether it is the
/// declaration site or a use site (LSP `references` honours
/// `includeDeclaration` by filtering on this).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    /// Byte range of the **identifier token only** (NOT the whole
    /// declaration). `rename` rewrites exactly this range, so it must be
    /// the bare name — never the surrounding `state X { … }` span.
    pub span: Span,
    /// `true` for the declaration-name token, `false` for a use site.
    pub is_decl: bool,
}

/// The reverse index: declaration → every occurrence (decl + semantic
/// uses) within this one file. Derived from the single analysis; semantic
/// only (Doc 26 §4.7).
#[derive(Clone, Debug, Default)]
pub struct ReferenceIndex {
    by_symbol: HashMap<SymbolKey, Vec<Occurrence>>,
}

impl ReferenceIndex {
    /// Build the index with **one CST walk** over the outputs of the
    /// *single* `analyze()` (its `symbol_table` + the parsed `cst`). Runs
    /// no analysis, lowers no IR, adds no parallel resolver.
    ///
    /// Every `Ident` token is classified by the SAME L3 `resolve_at`
    /// (use-site → its declaration) and, independently, tested for being a
    /// declaration-name token (the first `Ident` within a recorded decl
    /// `Span`). Only tokens semantic-resolution proves belong to a symbol
    /// are recorded; string/comment/trivia tokens are not `Ident`s in a
    /// classified position and so are excluded by construction.
    pub fn build(table: &SymbolTable, cst: &SyntaxNode) -> Self {
        let mut by_symbol: HashMap<SymbolKey, Vec<Occurrence>> = HashMap::new();

        // Pre-compute the set of declaration-name token ranges keyed by the
        // symbol they declare. A decl-name token is the FIRST `Ident` whose
        // range lies inside a `SymbolTable` decl `Span` — the exact rule
        // `ast::first_ident` (and thus the table builder) used to derive
        // the name string, so the recorded name and this token are
        // byte-identical. `resolve_at` deliberately returns `None` on a
        // declaration ident (a decl is not a use site), so the decl-name
        // token is discovered structurally here, not via the classifier.
        let decl_names = collect_decl_name_tokens(table, cst);
        for (key, span) in &decl_names {
            by_symbol.entry(key.clone()).or_default().push(Occurrence {
                span: *span,
                is_decl: true,
            });
        }

        // Walk every Ident token; admit a USE occurrence iff `resolve_at`
        // (the L3 classifier + `SymbolTable::resolve_*`) resolves it to
        // exactly some declaration. A token inside a string/comment/trivia
        // is a `StringLiteral`/comment/whitespace token — never an `Ident`
        // in a position `resolve_at` classifies — so it is excluded here
        // with no special-casing. A same-spelled token resolving to a
        // different machine/decl gets a different `SymbolKey` and lands in
        // a different bucket (so it is correctly NOT a reference to the
        // target).
        for tok in ident_tokens(cst) {
            let r = &tok.text_range();
            let start = u32::from(r.start());
            // The byte at the token's start uniquely lands inside it; the
            // classifier resolves the token covering that byte.
            let Some(resolved) = resolve_at(table, cst, start) else {
                continue;
            };
            let key = key_of(&resolved);
            let span = Span::new(usize::from(r.start()), usize::from(r.end()));
            // A decl-name token can also superficially look like a use of
            // its own symbol; we already recorded it as the decl above and
            // `resolve_at` returns `None` on a decl ident, so this loop
            // never double-counts it. (Belt-and-braces: skip if this exact
            // range is already the decl occurrence for this key.)
            let bucket = by_symbol.entry(key).or_default();
            let already_decl = bucket.iter().any(|o| o.is_decl && o.span == span);
            if !already_decl {
                bucket.push(Occurrence {
                    span,
                    is_decl: false,
                });
            }
        }

        // Deterministic order (document order) so the served edit/reference
        // list is stable across runs — important for the §5.4 oracle
        // byte-range equality and for predictable client behaviour.
        for occ in by_symbol.values_mut() {
            occ.sort_by_key(|o| (o.span.start, o.span.end, !o.is_decl));
            occ.dedup();
        }

        ReferenceIndex { by_symbol }
    }

    /// Resolve the identifier at `byte` to its [`SymbolKey`] (via the SAME
    /// L3 `resolve_at`), or `None` if the cursor is not on a resolvable
    /// reference/identifier (whitespace, keyword, string, comment,
    /// unknown/cross-file name). This is how `references`/`prepareRename`/
    /// `rename` find *which* symbol the cursor is on — always semantically.
    pub fn key_at(table: &SymbolTable, cst: &SyntaxNode, byte: u32) -> Option<SymbolKey> {
        // Two routes resolve a cursor to a symbol, both semantic:
        //  (1) the cursor is on a USE site → `resolve_at` classifies it;
        //  (2) the cursor is on the DECL-name token → `resolve_at` returns
        //      `None` (a decl is not a use), so fall back to the structural
        //      decl-name lookup (the same one `build` uses). This lets
        //      `prepareRename`/`references` work with the cursor on the
        //      declaration itself, which clients routinely do.
        if let Some(r) = resolve_at(table, cst, byte) {
            return Some(key_of(&r));
        }
        // Decl-name fallback: find a decl-name token covering `byte`.
        let decl_names = collect_decl_name_tokens(table, cst);
        for (key, span) in decl_names {
            if (span.start as u32) <= byte && byte < (span.end as u32) {
                return Some(key);
            }
        }
        None
    }

    /// Every occurrence (decl + uses) of `key`, in document order. Empty
    /// slice if the symbol has no recorded occurrences (defensive — a
    /// resolvable key always has at least its decl).
    pub fn occurrences(&self, key: &SymbolKey) -> &[Occurrence] {
        self.by_symbol.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Why a `prepareRename`/`rename` request was refused. Each maps to a
/// clear, up-front client message — the LSP contract is to tell the client
/// a token is not renameable rather than silent-allow → dangerous edit
/// (Doc 26 risk-2 / §8 L5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameRefusal {
    /// The cursor is not on a renameable user symbol at all (whitespace,
    /// keyword, contextual keyword, punctuation, string interior, comment,
    /// `@id`/state-id annotation, an unknown/cross-file name).
    NotARenameableSymbol,
    /// The symbol IS resolvable but its kind is out of v1.2 rename scope:
    /// a `machine` name (codegen/ABI blast radius beyond single-file —
    /// Doc 14 §8 / Doc 26 §8 L5).
    MachineNameOutOfScope,
    /// Single-file scope: the symbol is exposed cross-file (e.g. a machine
    /// referenced by `send … to M`), so a single-file edit would be
    /// partial/unsafe. Cross-file rename is explicitly v1.3 (Doc 26 §4.6).
    CrossFileExposed,
}

impl RenameRefusal {
    /// The user-visible message. Cross-file uses Doc 14 §8's exact
    /// sanctioned string so the client shows the spec's own v1 boundary.
    pub fn message(&self) -> String {
        match self {
            RenameRefusal::NotARenameableSymbol => {
                "Cannot rename: the cursor is not on a renameable symbol \
                 (only state, event, extern and context-field names declared \
                 in this file can be renamed)."
                    .to_owned()
            }
            RenameRefusal::MachineNameOutOfScope => {
                "Cannot rename a machine name: it is identity-bearing for \
                 code generation and may be referenced across files. \
                 Machine rename is out of scope in LSP v1."
                    .to_owned()
            }
            RenameRefusal::CrossFileExposed => "Cross-file rename is not supported in LSP v1. \
                 Rename in each file manually."
                .to_owned(),
        }
    }
}

/// Decide whether the symbol at `byte` may be renamed, and if so return its
/// key + the **declaration-name range** (the `prepareRename` "renameable
/// range" — the bare identifier the editor will let the user edit).
///
/// HARD-REJECTS (returns `Err`) — never silent-allow → dangerous edit:
/// - a non-resolvable cursor (whitespace / keyword / contextual keyword /
///   punctuation / **string interior** / **comment** / trivia / an
///   `@id`/state-id annotation string) → `NotARenameableSymbol` (these are
///   not `Ident`s `resolve_at` classifies / not a recorded decl name);
/// - a **machine name** (`is_renameable_kind() == false`) →
///   `MachineNameOutOfScope` (codegen/ABI blast radius, Doc 26 §8 L5);
/// - a symbol with detectable **cross-file exposure** → `CrossFileExposed`
///   (single-file edit would be partial; cross-file is v1.3, Doc 26 §4.6).
pub fn prepare_rename(
    table: &SymbolTable,
    cst: &SyntaxNode,
    byte: u32,
) -> Result<(SymbolKey, Span), RenameRefusal> {
    // `key_at` is purely semantic: it returns Some ONLY for a resolvable
    // use site or a decl-name token. A cursor in a string/comment/keyword/
    // whitespace/`@id` annotation is none of those → `None` → rejected
    // up-front (the LSP contract; never a silent-allow that becomes a
    // corrupting edit — Doc 26 risk-2).
    let key =
        ReferenceIndex::key_at(table, cst, byte).ok_or(RenameRefusal::NotARenameableSymbol)?;

    // Machine names: identity-bearing for codegen + cross-file `send`.
    if !key.is_renameable_kind() {
        return Err(RenameRefusal::MachineNameOutOfScope);
    }

    // Cross-file exposure (single-file safety, Doc 26 §4.6). In v1.2 the
    // only in-tree construct that exposes a *non-machine* symbol across a
    // file boundary is — there is none: events/states/externs/context
    // fields are all machine-local in the single-file model (`import` only
    // affects the security pass, never the per-file `SymbolTable`; Doc 26
    // §4.6 / `check.rs:184`). Machine is already rejected above. So a
    // renameable-kind symbol has no detectable cross-file exposure by
    // construction in v1.2; this branch is kept as the explicit, audited
    // statement of that boundary (and the seam where v1.3 cross-file
    // detection plugs in) rather than an unstated assumption.
    if cross_file_exposed(&key) {
        return Err(RenameRefusal::CrossFileExposed);
    }

    // The renameable range = the declaration-name token range (the bare
    // identifier). Always present for a resolvable renameable symbol.
    let decl_span = decl_name_span(table, cst, &key).ok_or(RenameRefusal::NotARenameableSymbol)?;
    Ok((key, decl_span))
}

/// Whether `key` names a symbol with detectable cross-file exposure.
///
/// In the v1.2 single-file model only a `machine` is cross-file-exposed
/// (via `send … to M` / `import` / codegen names). Machine rename is
/// already rejected by kind before this is consulted, so for every
/// *renameable* kind this is `false` — the single-file edit is total.
/// Factored out as the explicit, named boundary (Doc 26 §4.6) and the
/// future v1.3 plug-in point, not an implicit assumption.
fn cross_file_exposed(key: &SymbolKey) -> bool {
    matches!(key, SymbolKey::Machine { .. })
}

/// The declaration-name token range for `key` (the bare identifier), or
/// `None` if the declaration has no name token (resilient parser on broken
/// input — never panics, just declines the rename).
fn decl_name_span(table: &SymbolTable, cst: &SyntaxNode, key: &SymbolKey) -> Option<Span> {
    collect_decl_name_tokens(table, cst)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, span)| span)
}

/// Validate a proposed new name and, if safe, return the exact set of
/// edits (decl + every semantic use) for a `WorkspaceEdit`.
///
/// Rejects (returns `Err` with a clear message, **no edit**) if:
/// - the cursor is not on a renameable symbol / it is a machine /
///   cross-file-exposed (all via [`prepare_rename`]);
/// - `new_name` is not a valid FSM-Lang identifier;
/// - `new_name` would **collide** with an existing symbol the renamed one
///   could shadow/conflict with in the same scope.
///
/// The returned ranges are EXACTLY [`ReferenceIndex::occurrences`] for the
/// target — decl + semantically-resolved uses, nothing textual. A
/// same-spelled string-literal substring / comment word / different-scope
/// symbol is **not** in the set by construction (it never entered the
/// index — see the module docs / risk-2).
pub fn plan_rename(
    table: &SymbolTable,
    index: &ReferenceIndex,
    cst: &SyntaxNode,
    byte: u32,
    new_name: &str,
) -> Result<Vec<Span>, RenameError> {
    let (key, _range) = prepare_rename(table, cst, byte).map_err(RenameError::Refused)?;

    if !is_valid_identifier(new_name) {
        return Err(RenameError::InvalidIdentifier(new_name.to_owned()));
    }

    // Collision: renaming to a name that already exists in a scope the
    // renamed symbol shares would silently merge two distinct entities or
    // shadow one — a different flavour of the silent-corruption sin. Reject
    // with a clear message and emit NO edit (Doc 26 §8 L5 / risk-2).
    if let Some(conflict) = collides(table, &key, new_name) {
        return Err(RenameError::Collision {
            new_name: new_name.to_owned(),
            kind: conflict,
        });
    }

    let edits: Vec<Span> = index.occurrences(&key).iter().map(|o| o.span).collect();
    // A resolvable renameable symbol always has at least its decl; an empty
    // edit set would mean we failed to find the symbol — refuse rather than
    // return a no-op "success" (honest failure over a silent nothing).
    if edits.is_empty() {
        return Err(RenameError::Refused(RenameRefusal::NotARenameableSymbol));
    }
    Ok(edits)
}

/// A rename was refused with a reason the client must surface (no edit is
/// produced for any of these — Doc 26 risk-2: a clear message, never a
/// partial/wrong `WorkspaceEdit`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameError {
    /// `prepareRename`-level refusal (not-a-symbol / machine / cross-file).
    Refused(RenameRefusal),
    /// `new_name` is not a syntactically valid FSM-Lang identifier.
    InvalidIdentifier(String),
    /// `new_name` already names another symbol in a conflicting scope.
    Collision {
        new_name: String,
        /// What it would collide with (for the message).
        kind: &'static str,
    },
}

impl RenameError {
    /// The user-visible message (the client shows this; no edit happens).
    pub fn message(&self) -> String {
        match self {
            RenameError::Refused(r) => r.message(),
            RenameError::InvalidIdentifier(n) => format!(
                "Cannot rename: '{n}' is not a valid identifier (an identifier \
                 must start with a letter or '_' and contain only letters, \
                 digits or '_')."
            ),
            RenameError::Collision { new_name, kind } => format!(
                "Cannot rename to '{new_name}': a {kind} with that name already \
                 exists in this scope. Renaming would silently merge or shadow \
                 two distinct symbols."
            ),
        }
    }
}

/// Whether `name` is a syntactically valid FSM-Lang identifier (first char
/// a letter or `_`, the rest letters/digits/`_`). Conservative on purpose:
/// a name the lexer would not tokenise as a single `Ident` must never be
/// written into the buffer (it would corrupt the parse — risk-2).
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether renaming `key`'s symbol to `new_name` would collide with an
/// existing symbol it could conflict with, returning the colliding
/// category for the message. Conservative: any same-name entity of the
/// SAME category in the SAME machine scope (or, for context fields/events/
/// externs, the per-machine + file-extern visibility the analyzer uses) is
/// a collision. We deliberately over-reject (e.g. a state name colliding
/// with any same-named state anywhere in the machine, matching the
/// analyzer's name-only `resolve_state`) rather than risk a silent merge.
fn collides(table: &SymbolTable, key: &SymbolKey, new_name: &str) -> Option<&'static str> {
    match key {
        SymbolKey::State { machine, .. } => {
            // The analyzer's `resolve_state` is name-only within a machine
            // (`symbol_table.rs:384`); two states sharing a name in the
            // same machine are already ambiguous to resolution, so renaming
            // INTO an existing state name is a collision. (Same-name states
            // under different parents already resolve to the first — we
            // must not create another such ambiguity silently.)
            let m = table.machines.get(*machine)?;
            m.states
                .iter()
                .any(|s| s.name == new_name)
                .then_some("state")
        }
        SymbolKey::Event { machine, .. } => {
            let m = table.machines.get(*machine)?;
            m.events
                .iter()
                .any(|e| e.name == new_name)
                .then_some("event")
        }
        SymbolKey::Extern { machine, .. } => {
            let m = table.machines.get(*machine)?;
            let local = m.externs.iter().any(|e| e.name == new_name);
            // `resolve_extern` searches machine scope then file scope
            // (`symbol_table.rs:369`), so a file-extern of the same name is
            // also reachable from this machine → also a collision.
            let file = table.file_externs.iter().any(|e| e.name == new_name);
            (local || file).then_some("extern")
        }
        SymbolKey::ContextField { machine, .. } => {
            let m = table.machines.get(*machine)?;
            m.context_fields
                .iter()
                .any(|f| f.name == new_name)
                .then_some("context field")
        }
        // Machine rename is rejected before collision-checking is reached;
        // kept exhaustive so a future renameable-machine decision must
        // consciously add its (cross-file) collision rule here.
        SymbolKey::Machine { .. } => table
            .machine_index
            .contains_key(new_name)
            .then_some("machine"),
    }
}

// ---------------------------------------------------------------------------
// CST helpers — pure traversal over the already-parsed tree (no analysis).
// ---------------------------------------------------------------------------

/// Every `Ident` token in the parse tree, in document order. String
/// literals, comments and whitespace are *different* token kinds and are
/// therefore never yielded here — the structural reason a comment word or
/// a string-literal substring can never become a reference (risk-2).
fn ident_tokens(cst: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    cst.descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
}

/// For every declaration the `SymbolTable` recorded, the `(SymbolKey,
/// name-token Span)` of its **declaration-name token** — the first `Ident`
/// whose range lies inside the recorded decl `Span`.
///
/// This mirrors the table builder exactly: every entry's name string was
/// derived via `ast::first_ident` over the same decl node, i.e. the first
/// `Ident` token in the decl subtree. The decl `Span` from the table is
/// that decl subtree's span, so the first `Ident` inside it IS the
/// name-token the builder used — the recorded name and this token are
/// byte-identical by construction. We intersect against the parse tree's
/// actual `Ident` tokens so a nameless decl (resilient parser on broken
/// input) simply yields nothing for that entry (never a panic, never a
/// guessed range).
///
/// `pub(crate)` because L6's semantic-tokens classifier
/// ([`crate::capabilities::semantic_tokens`]) reuses the **exact same**
/// declaration-name discovery to assign the `declaration` modifier — a
/// decl-name token must carry its entity type *plus* `declaration`, a use
/// site the type *without* it (Doc 14 §10 modifier 0). Sharing this one
/// function (rather than re-deriving "which `Ident` is a declaration")
/// keeps L6's decl-vs-ref split byte-identical to L5's reference index and
/// to `fsm check` — the Doc 26 §8 L6 "reuse `symbol_table` for the
/// decl/ref distinction, do NOT add a parallel classifier" seam (the same
/// promote-and-reuse the L4 wave applied to `resolve.rs`'s helpers).
pub(crate) fn collect_decl_name_tokens(
    table: &SymbolTable,
    cst: &SyntaxNode,
) -> Vec<(SymbolKey, Span)> {
    let mut out: Vec<(SymbolKey, Span)> = Vec::new();

    // First `Ident` token whose range is within `[span.start, span.end)`.
    // The decl span starts at the decl keyword (`state`/`event`/…); the
    // name is the first `Ident` after it — exactly `first_ident`'s result.
    let first_ident_in = |span: Span| -> Option<Span> {
        cst.descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text_range())
            .find(|r| {
                let (s, e) = (u32::from(r.start()), u32::from(r.end()));
                (span.start as u32) <= s && e <= (span.end as u32)
            })
            .map(|r| Span::new(usize::from(r.start()), usize::from(r.end())))
    };

    for (mi, m) in table.machines.iter().enumerate() {
        // Machine name token (recorded for completeness so a machine cursor
        // resolves to a key — `prepare_rename` then rejects it by kind,
        // which is the correct, *informative* refusal rather than a blank
        // "not a symbol").
        if let Some(span) = first_ident_in(m.span) {
            out.push((SymbolKey::Machine { decl: m.span }, span));
        }
        for e in &m.events {
            if let Some(span) = first_ident_in(e.span) {
                out.push((
                    SymbolKey::Event {
                        machine: mi,
                        decl: e.span,
                    },
                    span,
                ));
            }
        }
        for e in &m.externs {
            if let Some(span) = first_ident_in(e.span) {
                out.push((
                    SymbolKey::Extern {
                        machine: mi,
                        decl: e.span,
                    },
                    span,
                ));
            }
        }
        for f in &m.context_fields {
            if let Some(span) = first_ident_in(f.span) {
                out.push((
                    SymbolKey::ContextField {
                        machine: mi,
                        decl: f.span,
                    },
                    span,
                ));
            }
        }
        for s in &m.states {
            if let Some(span) = first_ident_in(s.span) {
                out.push((
                    SymbolKey::State {
                        machine: mi,
                        decl: s.span,
                    },
                    span,
                ));
            }
        }
    }
    // File-level externs are reachable from any machine via
    // `resolve_extern`'s file-scope fallback; index their decl name under
    // the machine that *resolves* them — but a file extern has no single
    // machine. `resolve_at` resolves a file-extern *use* to
    // `Resolved::Extern{ machine: <the using machine>, decl: <file-extern
    // span> }`, so the `decl` span is the stable identity; we record the
    // file extern's own decl-name token under EACH machine index that could
    // resolve it is unnecessary — the `decl` span already disambiguates and
    // `key_at`'s use-site path keys uses correctly. The decl-name token
    // itself is recorded once with the using-context-independent identity:
    // a synthetic machine index is wrong, so we DON'T fabricate one. A file
    // extern's *declaration* cursor therefore resolves via the use-path
    // only if referenced; renaming a file extern is supported through its
    // use sites and (when referenced from a machine) its resolved key. This
    // is the conservative single-file boundary: no fabricated machine
    // identity, no guessed range.
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn setup(src: &str) -> (SymbolTable, SyntaxNode) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        (a.symbol_table, cst)
    }

    fn at(src: &str, needle: &str, plus: usize) -> u32 {
        (src.find(needle).expect("needle") + plus) as u32
    }

    #[test]
    fn references_are_semantic_not_textual_string_and_comment_excluded() {
        // `Target` appears: (1) as the state decl name, (2) as a transition
        // target (a real semantic use), (3) inside a // comment, (4) inside
        // a string-literal (a ctx-field default). Only (1)+(2) may be in
        // the index — (3)/(4) are NOT `Ident` tokens in a classified
        // position so they are excluded by construction (risk-2).
        let src = "language fsm 2.0\nmachine M {\n  context { note: str = \"go to Target now\" }\n  events { GO }\n  initial Start\n  state Start {\n    // reach Target soon\n    on GO -> Target\n  }\n  state Target {}\n}\n";
        let (t, cst) = setup(src);
        let idx = ReferenceIndex::build(&t, &cst);
        // Resolve via the decl-name token of `state Target {}`.
        let key =
            ReferenceIndex::key_at(&t, &cst, at(src, "state Target", 6)).expect("Target resolves");
        let occ = idx.occurrences(&key);
        // Exactly 2 occurrences: the decl + the one transition-target use.
        assert_eq!(
            occ.len(),
            2,
            "expected decl + 1 semantic use only, got {occ:?}"
        );
        // The decl-name token range must be the `Target` in `state Target`.
        let decl = occ.iter().find(|o| o.is_decl).expect("a decl occurrence");
        assert_eq!(
            &src[decl.span.start..decl.span.end],
            "Target",
            "decl occurrence is the bare name token"
        );
        // The use must be the `Target` after `->`, NOT the comment or
        // string substring. Assert NONE of the recorded spans fall inside
        // the comment or the string literal.
        let comment_start = src.find("// reach Target").unwrap();
        let comment_end = src[comment_start..].find('\n').unwrap() + comment_start;
        let str_start = src.find("\"go to Target now\"").unwrap();
        let str_end = str_start + "\"go to Target now\"".len();
        for o in occ {
            let in_comment = o.span.start >= comment_start && o.span.start < comment_end;
            let in_string = o.span.start >= str_start && o.span.start < str_end;
            assert!(
                !in_comment,
                "a comment substring must NEVER be a reference: {o:?}"
            );
            assert!(
                !in_string,
                "a string-literal substring must NEVER be a reference: {o:?}"
            );
        }
        // And the one use is exactly the transition target.
        let use_occ = occ.iter().find(|o| !o.is_decl).expect("a use occurrence");
        let used = &src[use_occ.span.start..use_occ.span.end];
        assert_eq!(used, "Target");
        assert!(
            use_occ.span.start > src.find("-> ").unwrap(),
            "the use is the post-arrow target token"
        );
    }

    #[test]
    fn same_spelled_symbol_in_different_machine_is_a_distinct_key() {
        // Two machines each declare a state `S` and use it. The `S` in M1
        // and the `S` in M2 must be DIFFERENT symbols (different machine
        // index → different SymbolKey) — renaming one must never touch the
        // other (the different-scope safety guarantee).
        let src = "language fsm 2.0\nmachine M1 {\n  events { GO }\n  initial S\n  state S {\n    on GO -> S\n  }\n}\nmachine M2 {\n  events { GO }\n  initial S\n  state S {\n    on GO -> S\n  }\n}\n";
        let (t, cst) = setup(src);
        let idx = ReferenceIndex::build(&t, &cst);
        // Cursor on M1's `state S` decl-name token (the FIRST `state S`).
        let k1 = ReferenceIndex::key_at(&t, &cst, at(src, "state S", 6)).expect("M1.S resolves");
        // M2's `state S` is the SECOND occurrence of "state S".
        let m2_state_s = {
            let first = src.find("state S").unwrap();
            src[first + 1..].find("state S").unwrap() + first + 1 + 6
        };
        let k2 = ReferenceIndex::key_at(&t, &cst, m2_state_s as u32).expect("M2.S resolves");
        assert_ne!(
            k1, k2,
            "same-spelled state in a different machine MUST be a distinct symbol"
        );
        // Renaming M1.S touches only M1's occurrences (decl + 2 uses:
        // `initial S` and `-> S`), none in M2.
        let m1_occ = idx.occurrences(&k1);
        let m2_first_byte = src.find("machine M2").unwrap();
        for o in m1_occ {
            assert!(
                o.span.start < m2_first_byte,
                "an M1.S occurrence leaked into M2's text: {o:?}"
            );
        }
        assert!(
            !m1_occ.is_empty() && !idx.occurrences(&k2).is_empty(),
            "both symbols must have their own occurrences"
        );
    }

    #[test]
    fn prepare_rename_rejects_machine_keyword_string_comment_and_atid() {
        // `@id("…")` is a PREFIX annotation on the line BEFORE the decl
        // (Doc 04 line 1130/1156 grammar) — here on `state A`. A cursor
        // inside that `@id` string must NOT be renameable (it is an
        // identity-bearing annotation, Doc 26 §8 L5). The `"A in a string"`
        // ctx-default proves a string-interior cursor is also rejected.
        let src = "language fsm 2.0\nmachine M {\n  context { note: str = \"A in a string\" }\n  events { GO }\n  initial A\n  // comment word A here\n  @id(\"s-a-id\")\n  state A {\n    on GO -> A\n  }\n}\n";
        let (t, cst) = setup(src);
        // Machine name → MachineNameOutOfScope (informative, not blank).
        let m = prepare_rename(&t, &cst, at(src, "machine M", 8));
        assert_eq!(m, Err(RenameRefusal::MachineNameOutOfScope));
        // Keyword `state` → NotARenameableSymbol.
        let kw = prepare_rename(&t, &cst, at(src, "state A", 1));
        assert_eq!(kw, Err(RenameRefusal::NotARenameableSymbol));
        // Inside the @id annotation string `"s-a-id"` → NotARenameable
        // (an identity-bearing annotation, not a renameable symbol).
        let atid = prepare_rename(&t, &cst, at(src, "s-a-id", 2));
        assert_eq!(atid, Err(RenameRefusal::NotARenameableSymbol));
        // Inside the // comment (the word `A`) → NotARenameableSymbol.
        let cmt = prepare_rename(&t, &cst, at(src, "comment word A", 13));
        assert_eq!(cmt, Err(RenameRefusal::NotARenameableSymbol));
        // Inside the string-literal `"A in a string"` → not renameable.
        let strlit = prepare_rename(&t, &cst, at(src, "A in a string", 0));
        assert_eq!(strlit, Err(RenameRefusal::NotARenameableSymbol));
        // Whitespace → NotARenameableSymbol.
        let ws = prepare_rename(&t, &cst, at(src, "machine M", 7));
        assert_eq!(ws, Err(RenameRefusal::NotARenameableSymbol));
        // A real state → Ok with the bare-name range.
        let (key, span) = prepare_rename(&t, &cst, at(src, "-> A", 3)).expect("state A renameable");
        assert!(matches!(key, SymbolKey::State { .. }));
        assert_eq!(&src[span.start..span.end], "A");
    }

    #[test]
    fn plan_rename_excludes_string_comment_diff_scope_and_rejects_collision() {
        // The risk-2 core, at the unit layer (the client test asserts it
        // end-to-end too). Target `Foo` also appears as a string substring,
        // a comment word, AND as a same-spelled DIFFERENT-machine state.
        let src = "language fsm 2.0\nmachine A {\n  context { note: str = \"Foo string here\" }\n  events { GO }\n  initial Foo\n  state Foo {\n    // Foo in a comment\n    on GO -> Foo\n  }\n  state Bar {}\n}\nmachine B {\n  events { GO }\n  initial Foo\n  state Foo {\n    on GO -> Foo\n  }\n}\n";
        let (t, cst) = setup(src);
        let idx = ReferenceIndex::build(&t, &cst);
        // Rename A's `Foo` (cursor on the transition-target use in A).
        let a_use = at(src, "-> Foo", 3);
        let edits = plan_rename(&t, &idx, &cst, a_use, "Renamed").expect("safe rename");
        // Every edit must be within machine A's text, never B's, never the
        // comment, never the string.
        let b_start = src.find("machine B").unwrap();
        let comment_start = src.find("// Foo in a comment").unwrap();
        let comment_end = src[comment_start..].find('\n').unwrap() + comment_start;
        let str_start = src.find("\"Foo string here\"").unwrap();
        let str_end = str_start + "\"Foo string here\"".len();
        for e in &edits {
            assert!(e.start < b_start, "edit leaked into machine B: {e:?}");
            assert!(
                !(e.start >= comment_start && e.start < comment_end),
                "edit fell inside the comment: {e:?}"
            );
            assert!(
                !(e.start >= str_start && e.start < str_end),
                "edit fell inside the string literal: {e:?}"
            );
            assert_eq!(&src[e.start..e.end], "Foo", "edit must be the bare name");
        }
        // A.Foo: decl + `initial Foo` + `-> Foo` = 3 edits exactly.
        assert_eq!(edits.len(), 3, "exact semantic edit count, got {edits:?}");
        // Collision: renaming A.Foo to `Bar` (an existing state in A) is
        // rejected with NO edit.
        let collide = plan_rename(&t, &idx, &cst, a_use, "Bar");
        assert!(
            matches!(collide, Err(RenameError::Collision { .. })),
            "rename into an existing same-scope state name must be rejected, got {collide:?}"
        );
        // Invalid identifier rejected too.
        let bad = plan_rename(&t, &idx, &cst, a_use, "1nope");
        assert!(matches!(bad, Err(RenameError::InvalidIdentifier(_))));
    }
}
