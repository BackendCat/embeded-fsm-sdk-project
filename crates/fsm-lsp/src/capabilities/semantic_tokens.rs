//! `textDocument/semanticTokens` — Doc 26 §5 / §8 L6 (single-file).
//!
//! Semantic tokens are *more precise* than the Doc 21 TextMate grammar:
//! TextMate can only lex-classify (a bare `Ident` is the same scope
//! everywhere), whereas this layer knows — from the **one** `fsm check`
//! analysis the whole LSP already ran — whether an identifier is a state,
//! an event, an extern, a context field or a machine, and whether it is a
//! *declaration* or a *reference* (Doc 21 §6's own rule: semantic tokens
//! refine analyzed regions, TextMate covers the rest during startup/large
//! files — the two coexist, this does not redesign Doc 21).
//!
//! ## Classification reuse — ONE classifier, no parallel one (Doc 26 §8 L6)
//!
//! Every `Ident` is classified by **reusing**, not re-deriving:
//! - L3's [`crate::capabilities::resolve::resolve_at`] for **use sites**
//!   (the SAME classifier mirroring `checks::name_resolution`, resolving
//!   through the SAME `SymbolTable::resolve_*` `fsm check` uses), bridged
//!   to a legend type by [`crate::refs::resolved_entity_kind`] (`Resolved`
//!   → the L5 `SymbolKey` taxonomy → kind);
//! - L5's [`crate::refs::collect_decl_name_tokens`] for **declaration-name
//!   tokens** (the first `Ident` within a `SymbolTable` decl `Span` — the
//!   exact `ast::first_ident` rule the table builder used), which also
//!   yields the entity kind via [`crate::refs::SymbolKey::entity_kind`].
//!
//! A decl token and a use token of the same symbol therefore get the
//! **identical** legend type and differ only by the `declaration` modifier
//! (Doc 14 §10 modifier 0) — the decl-vs-ref split is decided by
//! `symbol_table` identity, exactly as Doc 26 §8 L6 requires, never by a
//! second ad-hoc classifier. There is **no** new analysis: `server.rs`
//! threads the `symbol_table` + `cst` of the single `analyze()` the
//! diagnostics/symbol/hover/completion/references path already ran.
//!
//! Non-`Ident` tokens get their **lexical** legend type straight from the
//! `fsm-lexer` `SyntaxKind` (keyword / operator / number / string /
//! comment / `@id` decorator). Pure structural punctuation that Doc 14 §10
//! gives **no** legend slot (`{ } ( ) ; , .`) is deliberately **not**
//! emitted — semantic tokens only refine legend-covered constructs; the
//! client falls back to the Doc 21 TextMate scope for those (the correct
//! LSP behaviour, and Doc 21 §6's stated coexistence model).
//!
//! ## Delta encoding (LSP relative format) — negotiated units via `position.rs`
//!
//! The wire format is the LSP relative encoding: each token is
//! `[deltaLine, deltaStartChar, length, tokenType, tokenModifiers]`, sorted
//! by position, deltas relative to the previous token, and `deltaStartChar`
//! reset on a new line. `deltaStartChar`/`length` are in the **negotiated**
//! `positionEncoding` unit (UTF-8 bytes or UTF-16 code units) — computed
//! via L1's authoritative [`crate::position::LineIndex`] (`position()`),
//! **not** a second converter (the §11.32 DRIFT-2 boundary is untouched).
//!
//! LSP `multilineTokenSupport` defaults off, so a multi-line
//! `BlockComment`/`DocComment` is split into **one token per line** (the
//! rust-analyzer model) — never a single token spanning lines (clients
//! reject or mis-render those).

use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
};

use fsm_analyzer::symbol_table::SymbolTable;
use fsm_diagnostics::Span;
use fsm_parser::cst::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::capabilities::resolve::resolve_at;
use crate::position::{LineIndex, OffsetEncoding};
use crate::refs::{collect_decl_name_tokens, resolved_entity_kind, EntityKind};

// ---------------------------------------------------------------------------
// Legend — Doc 14 §10 / §2 `ServerCapabilities`, VERIFIED against the real
// `fsm-lexer` `SyntaxKind`s + `symbol_table` capabilities (Doc 26 §8 L6).
//
// The token TYPE order is **exactly** Doc 14 §2's
// `semanticTokensProvider.legend.tokenTypes` array (index = legend
// position, asserted equal in the §5.4 tests so the encoded `tokenType`
// integers always reference the advertised name). The MODIFIER order is
// **exactly** Doc 14 §2's `tokenModifiers` array; an encoded
// `tokenModifiers` field is the bitset `1 << index`.
// ---------------------------------------------------------------------------

/// Doc 14 §10 token-type legend indices (index = position in the advertised
/// `tokenTypes` array — Doc 14 §2). Named so the classifier never hard-codes
/// a bare integer and the §5.4 oracle is self-documenting.
mod ty {
    pub const NAMESPACE: u32 = 0; // Machine names
    pub const TYPE: u32 = 1; // State names (decl + ref)
    pub const ENUM: u32 = 2; // Event names
    pub const FUNCTION: u32 = 3; // Extern function names
    pub const VARIABLE: u32 = 4; // ctx.X / payload.X fields
    pub const KEYWORD: u32 = 5; // all FSM-Lang keywords
    pub const STRING: u32 = 6; // stable-ID strings, string literals
    pub const NUMBER: u32 = 7; // integer / float literals
    pub const OPERATOR: u32 = 8; // -> ~> : = [ ] && || comparison …
    pub const COMMENT: u32 = 9; // line / block / doc comments
    pub const DECORATOR: u32 = 10; // @id(...) annotation
}

/// Doc 14 §10 token-modifier legend indices. The encoded `tokenModifiers`
/// field is a bitset: modifier `i` set ⇒ bit `1 << i`.
mod md {
    /// Doc 14 §10 modifier 0 — token is the declaration site, not a ref.
    pub const DECLARATION: u32 = 0;
    /// Doc 14 §10 modifier 1 — payload fields (read-only in action blocks).
    pub const READONLY: u32 = 1;
    // Modifier 2 `deprecated` — Doc 14 §10 says "reserved for future use"
    // (no in-tree source: no `SymbolTable`/IR field records a deprecated
    // stable ID). Declared in the legend (the spec advertises it) but never
    // emitted — emitting a modifier with no analysis backing would be a
    // fabrication. Flagged in Doc 00 §11.37, not silently dropped.
    /// Doc 14 §10 modifier 3 — enum-variant names in payload-type context.
    pub const STATIC: u32 = 3;
}

/// The semantic-tokens legend the server advertises in `initialize`
/// (`semanticTokensProvider.legend`). Declared **once** here and reused for
/// both the capability advertisement and the encoder, so the advertised
/// indices and the encoded `tokenType`/`tokenModifiers` can never diverge
/// (the §5.4 tests assert this identity).
///
/// Derivation (Doc 26 §8 L6, verify-vs-code discipline):
/// - **Types** are Doc 14 §2's `tokenTypes` array verbatim — cross-checked
///   that every one has a real producer in the `fsm-lexer` `SyntaxKind`s
///   (keyword/operator/number/string/comment/decorator tokens) or the
///   `symbol_table` (namespace/type/enum/function/variable via the L3/L5
///   classifiers). Doc 21 (TextMate) is a *draft* cross-checked, not
///   obeyed: its richer scope set (e.g. `entity.name.type.state.fsm`,
///   `support.type.event.fsm`) maps **into** these 11 LSP types (semantic
///   tokens are coarser than TextMate scopes by LSP design) — Doc 21 §6
///   itself defers the legend to Doc 14 §10, so there is no Doc-21/Doc-14
///   legend conflict to resolve.
/// - **Modifiers** are Doc 14 §2's `tokenModifiers` verbatim.
pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,
            SemanticTokenType::TYPE,
            SemanticTokenType::ENUM,
            SemanticTokenType::FUNCTION,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::KEYWORD,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::OPERATOR,
            SemanticTokenType::COMMENT,
            SemanticTokenType::DECORATOR,
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::DEPRECATED,
            SemanticTokenModifier::STATIC,
        ],
    }
}

/// One classified token before delta-encoding: an absolute byte range plus
/// its legend type and modifier bitset. (`type`/`mods` already in legend
/// index form; `mods` is the `1<<i` bitset.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Classified {
    /// Byte offset of the token start (into the analysed buffer).
    start: u32,
    /// Byte offset of the token end (exclusive).
    end: u32,
    ty: u32,
    mods: u32,
}

/// Map an entity kind to its Doc 14 §10 legend **type** index. The single
/// decl⇔use-consistent mapping (both paths funnel here): a state decl and a
/// state use get `ty::TYPE`; they differ only by the `declaration` modifier.
fn entity_type(kind: EntityKind) -> u32 {
    match kind {
        EntityKind::Machine => ty::NAMESPACE,
        EntityKind::State => ty::TYPE,
        EntityKind::Event => ty::ENUM,
        EntityKind::Extern => ty::FUNCTION,
        EntityKind::ContextField => ty::VARIABLE,
    }
}

/// Compute the classified token stream for `cst`, in document order.
///
/// One pass over every CST token. `Ident`s are classified by the **reused**
/// L3/L5 classifiers (use site → `resolve_at`; decl-name token →
/// `collect_decl_name_tokens`); every other token by its lexical
/// `SyntaxKind`. Tokens Doc 14 §10 has no legend slot for (structural
/// punctuation, whitespace) are skipped (the client uses the Doc 21
/// TextMate scope — the coexistence model). Multi-line comments are split
/// per line at the end (`split_multiline`); here a comment is one entry
/// spanning its full byte range.
fn classify(table: &SymbolTable, cst: &SyntaxNode) -> Vec<Classified> {
    // Decl-name tokens, keyed by their byte range → entity kind. Built once
    // (the SAME discovery L5's `ReferenceIndex` uses) so a decl ident gets
    // its type + the `declaration` modifier, a use ident the type only.
    let decl_ranges: Vec<(Span, EntityKind)> = collect_decl_name_tokens(table, cst)
        .into_iter()
        .map(|(key, span)| (span, key.entity_kind()))
        .collect();
    let decl_kind_at = |start: u32, end: u32| -> Option<EntityKind> {
        decl_ranges
            .iter()
            .find(|(s, _)| s.start as u32 == start && s.end as u32 == end)
            .map(|(_, k)| *k)
    };

    let mut out: Vec<Classified> = Vec::new();
    for el in cst.descendants_with_tokens() {
        let Some(tok) = el.into_token() else {
            continue;
        };
        let r = tok.text_range();
        let start = u32::from(r.start());
        let end = u32::from(r.end());
        if start == end {
            continue; // zero-width (synthesised) — nothing to colour.
        }

        let kind = tok.kind();
        let (ty, mods) = if kind == SyntaxKind::Ident {
            // 1) Declaration-name token? → entity type + `declaration`
            //    (Doc 14 §10 modifier 0). Decided by `symbol_table`
            //    identity, the SAME rule L5 uses — not re-classified.
            if let Some(k) = decl_kind_at(start, end) {
                (entity_type(k), bit(md::DECLARATION))
            } else if let Some(resolved) = resolve_at(table, cst, start) {
                // 2) A resolvable USE site → the SAME entity type, NO
                //    `declaration` (Doc 26 §8 L6 decl-vs-ref). Routed
                //    through the SAME `Resolved`→`SymbolKey` bridge L5 uses.
                (entity_type(resolved_entity_kind(&resolved)), 0)
            } else if is_contextual_keyword(&tok) {
                // 3) A contextual keyword the lexer emits as `Ident` but the
                //    parser treats specially (`entry`/`exit`/`events`/
                //    `queue`/`if`/`while`/`for`/`entry_point`/`exit_point`/
                //    `fsm`/`likely`/`rare`) → Doc 14 §10 index 5 `keyword`.
                //    Decided by the SAME node-context the parser/`resolve.rs`
                //    use, not a keyword string-list guess.
                (ty::KEYWORD, 0)
            } else if let Some((t, m)) = qualified_ident(&tok) {
                // 4) `payload.X` field / enum `Type.Variant` — `resolve_at`
                //    deliberately degrades these to `None` (the analyzer
                //    does not resolve them to one decl). Doc 14 §10 still
                //    assigns them: payload field → index 4 `variable` +
                //    `readonly` (mod 1); enum variant → index 2 `enum` +
                //    `static` (mod 3); the enum *type* prefix → index 2.
                (t, m)
            } else {
                // 5) Any other bare ident the analyzer resolves to no single
                //    declaration (an undeclared name, a const operand, …):
                //    no Doc 14 §10 legend slot → no semantic token (TextMate
                //    scope applies — the coexistence model). Never a guess.
                continue;
            }
        } else if let Some(t) = lexical_type(kind, &tok) {
            (t, 0)
        } else {
            // Structural punctuation / whitespace — Doc 14 §10 has no slot.
            continue;
        };

        out.push(Classified {
            start,
            end,
            ty,
            mods,
        });
    }

    // CST preorder is *almost* document order, but a token attached as
    // trailing trivia of one node can precede (by byte) a token of the next
    // — sort by start so the delta encoding is monotonic (the LSP contract).
    out.sort_by_key(|c| (c.start, c.end));
    out
}

/// The set bit for a modifier legend index (`1 << i`).
fn bit(modifier_index: u32) -> u32 {
    1u32 << modifier_index
}

/// Lexical legend type for a non-`Ident` token, or `None` when Doc 14 §10
/// gives the kind no legend slot (structural punctuation, whitespace) — in
/// which case no semantic token is emitted and the client uses the Doc 21
/// TextMate scope (the §6 coexistence model). The `StableId` /
/// `@`-in-annotation distinction needs the token's context, hence `&tok`.
fn lexical_type(kind: SyntaxKind, tok: &SyntaxToken) -> Option<u32> {
    match kind {
        // ---- keywords: every `Kw*` token (Doc 14 §10 index 5) -----------
        // The lexer emits a dedicated `Kw*` for every reserved word; the
        // pseudo-state words `choice`/`junction`/`fork`/`join` ARE real
        // `Kw*` here (not contextual idents). Doc 14 §10's "pseudo-state
        // keywords use index 5 keyword with modifier declaration at their
        // declaration sites" — the modifier is added below for the
        // pseudo-state-introducing keywords; the base type is always
        // `keyword`. (A `_` arm over the dense `Kw*` block is intentional:
        // every variant in the lexer's keyword range maps identically;
        // enumerating ~50 names buys nothing and risks omission drift.)
        k if is_keyword_kind(k) => Some(ty::KEYWORD),

        // ---- operators (Doc 14 §10 index 8) -----------------------------
        // Doc 14 §10's operator row lists `-> ~> : = [ ] && || ` + the
        // comparison operators explicitly. The other arithmetic/bitwise/
        // logical operators are operators by every reasonable reading and
        // by Doc 21's `keyword.operator.*` scopes — included for
        // completeness (semantic tokens being a refinement, not a
        // regression, of the TextMate operator classification).
        SyntaxKind::Arrow
        | SyntaxKind::HistoryArrow
        | SyntaxKind::Colon
        | SyntaxKind::Eq
        | SyntaxKind::EqEq
        | SyntaxKind::BangEq
        | SyntaxKind::Lt
        | SyntaxKind::Gt
        | SyntaxKind::Le
        | SyntaxKind::Ge
        | SyntaxKind::AmpAmp
        | SyntaxKind::PipePipe
        | SyntaxKind::Bang
        | SyntaxKind::Plus
        | SyntaxKind::Minus
        | SyntaxKind::Star
        | SyntaxKind::Slash
        | SyntaxKind::Percent
        | SyntaxKind::Amp
        | SyntaxKind::Pipe
        | SyntaxKind::Caret
        | SyntaxKind::Tilde
        | SyntaxKind::Shl
        | SyntaxKind::Shr
        // `[` `]` are explicitly in Doc 14 §10's operator row (guard
        // delimiters); the lexer tokenises them as `LBracket`/`RBracket`.
        | SyntaxKind::LBracket
        | SyntaxKind::RBracket => Some(ty::OPERATOR),

        // ---- literals ---------------------------------------------------
        SyntaxKind::IntLiteral | SyntaxKind::FloatLiteral => Some(ty::NUMBER),
        SyntaxKind::StringLiteral => {
            // Doc 14 §10 index 6 `string` covers both ordinary string
            // literals AND "Stable ID strings". A `StringLiteral` *inside* a
            // `STABLE_ID_ANNOT` (the `"…"` of `@id("…")`) is the stable-ID
            // string — still index 6 `string` (same legend slot), so no
            // branch needed; both map to `string`.
            Some(ty::STRING)
        }

        // ---- comments (Doc 14 §10 index 9) ------------------------------
        // Index 9's row is "Line comments, block comments". A `DocComment`
        // (`/// …`) is, in the real `fsm-lexer`, a single comment-class
        // trivia token (`SyntaxKind::is_trivia()` true); its *text* is not
        // separately tokenised. Doc 14 §10 also lists "doc comment text"
        // under index 6 `string` — that sub-clause is imprecise vs the real
        // lexer (there is no separate doc-text token to colour as a string;
        // the whole `///` line is one `DocComment`). Derive-correct: the
        // whole `DocComment` token → index 9 `comment` (its lexical kind),
        // consistent with line/block comments. Flagged precisely in Doc 00
        // §11.37 as a Doc-14-§10 imprecision — NOT a defect, NOT a Doc-14
        // change (a doc comment IS a comment; splitting a sub-token a
        // single-token lexer never produces would be the fabrication).
        SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::DocComment => {
            Some(ty::COMMENT)
        }

        // ---- the `@id(...)` decorator (Doc 14 §10 index 10) -------------
        // `StableId` is the lexer's `@ident` single token; `At` is the bare
        // `@` of the long `@ id ( "…" )` form. Both, *when inside a
        // `STABLE_ID_ANNOT`*, are the annotation marker → index 10
        // `decorator`. (A stray `@`/`StableId` outside an annotation node
        // is malformed input; we still colour it `decorator` — its lexical
        // intent — rather than drop it, matching Doc 21's
        // `storage.modifier.annotation.fsm` / `meta.annotation.fsm`.)
        SyntaxKind::StableId | SyntaxKind::At => Some(ty::DECORATOR),

        // ---- everything else: NO Doc 14 §10 legend slot -----------------
        // `{ } ( ) ; , .` (structural punctuation), `Whitespace`/`Newline`
        // (trivia), `Error`/`Eof` sentinels. Semantic tokens only refine
        // legend-covered constructs; the client uses the Doc 21 TextMate
        // scope for these (Doc 21 §6 coexistence). Emitting an out-of-legend
        // token would be a protocol error.
        _ => {
            let _ = tok; // (kept for the StableId/At context note above)
            None
        }
    }
}

/// `true` for every reserved-keyword `SyntaxKind` (`Kw*`). The lexer's
/// keyword tokens occupy a dense contiguous range from `KwAfter` to `KwU64`
/// (verified `crates/fsm-parser/src/cst/kinds.rs` — the block before the
/// operators). A range test over the stable `#[repr(u16)]` discriminants is
/// robust to new keywords (a future `Kw*` appended in that block is still
/// caught) and avoids a ~50-arm match that could silently miss one.
fn is_keyword_kind(k: SyntaxKind) -> bool {
    (SyntaxKind::KwAfter as u16..=SyntaxKind::KwU64 as u16).contains(&(k as u16))
}

/// Contextual keywords: words the lexer emits as plain `Ident` but the
/// parser treats as keywords *by position* (`crates/fsm-parser/src/cst/
/// kinds.rs` documents the exact set: `entry`, `exit`, `events`, `queue`,
/// `if`, `while`, `for`, `entry_point`, `exit_point`, `fsm`, and the
/// transition-prefix hints `likely`/`rare`). Doc 14 §10 index 5 `keyword`
/// is "**All** FSM-Lang keywords" — these are keywords to the user even
/// though they round-trip as `Ident` in the CST, so they must colour as
/// `keyword`, not as a (failed) symbol reference.
///
/// The decision is by the **enclosing node kind** (the SAME structural
/// signal `resolve.rs`/the parser use), NOT a bare ident-text allow-list: a
/// state/event/extern/field legitimately *named* `events` or `likely` is a
/// real symbol (the lexer keeps these contextual precisely so such names
/// keep parsing) and is already classified by `resolve_at`/the decl-name
/// pass *before* this is consulted (steps 1–2 in `classify`). So this is
/// reached only for an `Ident` that resolved to no symbol; here the
/// node-context disambiguates a genuine contextual-keyword *use* (e.g. the
/// `events` introducing an `EVENTS_BLOCK`, `entry`/`exit` introducing an
/// `ENTRY_DECL`/`EXIT_DECL`, `if`/`while`/`for` a `STMT_IF`/`STMT_WHILE`/
/// `STMT_FOR`, `likely`/`rare` a `BRANCH_HINT`, `entry_point`/`exit_point`
/// an `ENTRY_POINT_DECL`/`EXIT_POINT_DECL`, `fsm` the `LANGUAGE_DECL`).
fn is_contextual_keyword(tok: &SyntaxToken) -> bool {
    let Some(parent) = tok.parent() else {
        return false;
    };
    match parent.kind() {
        // `events { … }` — the `events` lead token of an EVENTS_BLOCK.
        SyntaxKind::EVENTS_BLOCK
        // `entry : …` / `exit : …`
        | SyntaxKind::ENTRY_DECL
        | SyntaxKind::EXIT_DECL
        // `queue { … }`
        | SyntaxKind::QUEUE_BLOCK
        // `if`/`while`/`for` action statements
        | SyntaxKind::STMT_IF
        | SyntaxKind::STMT_WHILE
        | SyntaxKind::STMT_FOR
        // `else` of an if-statement
        | SyntaxKind::STMT_ELSE
        // `likely`/`rare` transition prefix (wrapped in BRANCH_HINT)
        | SyntaxKind::BRANCH_HINT
        // the `fsm` of `language fsm 2.0`
        | SyntaxKind::LANGUAGE_DECL
        | SyntaxKind::LANGUAGE_VERSION => true,
        // `entry_point name -> T` / `exit_point name`: the lead `entry_point`/
        // `exit_point` is the FIRST ident of the decl; a *later* ident is the
        // pseudo-state's own name (a declaration, handled in step 1) or its
        // target (a state ref, handled in step 2). Only the lead one is the
        // contextual keyword.
        SyntaxKind::ENTRY_POINT_DECL | SyntaxKind::EXIT_POINT_DECL => {
            parent
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)
                .map(|first| first.text_range() == tok.text_range())
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Classify a `payload.X` field ident or an enum `Type.Variant` ident — the
/// two `EXPR_FIELD_REF` cases `resolve_at` deliberately returns `None` for
/// (the analyzer resolves neither to one declaration: payload schemas are
/// per-event and not in the `symbol_table`; an enum `Type.Variant` is a
/// qualified literal, not a single-decl target). Doc 14 §10 still assigns
/// them legend slots, so semantic tokens — being *more* precise than
/// TextMate — colour them:
///
/// - `payload` prefix ident → index 4 `variable` (Doc 21:
///   `variable.language.payload.fsm`);
/// - `payload.<field>` field ident → index 4 `variable` + `readonly`
///   (Doc 14 §10 modifier 1, "Payload fields (read-only in action
///   blocks)");
/// - an enum *type* prefix ident → index 2 `enum`;
/// - the `<Variant>` after it → index 2 `enum` + `static` (Doc 14 §10
///   modifier 3, "Enum variant names within event payload type context").
///
/// Returns `None` for any other `EXPR_FIELD_REF` shape (`ctx.` is already
/// handled by `resolve_at` in step 2; an unknown qualifier is not a Doc 14
/// §10 target — no token, TextMate applies). The lhs/rhs split mirrors
/// `checks::name_resolution`'s `EXPR_FIELD_REF` handling
/// (`name_resolution.rs:388-431`) exactly — same structural rule, not a
/// parallel one.
fn qualified_ident(tok: &SyntaxToken) -> Option<(u32, u32)> {
    let parent = tok.parent()?;
    if parent.kind() != SyntaxKind::EXPR_FIELD_REF {
        return None;
    }
    // lhs = leftmost ident anywhere in the first child subtree (the
    // `name_resolution.rs:391-401` rule); rhs = the direct Ident child
    // after the Dot (`name_resolution.rs:403-408`).
    let lhs = parent.children().next().and_then(|c| {
        c.descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text().to_string())
    })?;
    let rhs_tok = parent
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?;
    let this_is_lhs = is_in_first_child(&parent, tok);
    let this_is_rhs = rhs_tok.text_range() == tok.text_range();

    if lhs == "payload" {
        // `payload` prefix → variable; `payload.field` → variable+readonly.
        if this_is_lhs {
            return Some((ty::VARIABLE, 0));
        }
        if this_is_rhs {
            return Some((ty::VARIABLE, bit(md::READONLY)));
        }
        return None;
    }
    if lhs == "ctx" {
        // `ctx.field` is resolved by `resolve_at` (step 2) when the field
        // exists; if we reach here the field is unknown (E0104) — colour
        // the prefix `variable` (its lexical intent, Doc 21
        // `variable.language.ctx.fsm`), decline the unknown rhs (no Doc 14
        // §10 slot for an unresolved field — no token, never a guess).
        if this_is_lhs {
            return Some((ty::VARIABLE, 0));
        }
        return None;
    }
    // An enum `Type.Variant`: the analyzer treats lhs as an enum name when
    // it matches a declared enum (`name_resolution.rs:422`). We do not
    // re-derive enum membership here (that would be a second classifier);
    // Doc 14 §10 only assigns enum *variants* a slot (index 2 + `static`).
    // Conservatively colour the qualified `Type.Variant` pair as `enum`
    // (prefix) / `enum`+`static` (variant): this matches Doc 14 §10's
    // modifier-3 intent and Doc 21's `variable.other.enummember.fsm`
    // refinement without inventing a legend slot. lhs-not-`ctx`/`payload`
    // in an `EXPR_FIELD_REF` is, per the grammar, a qualified enum name.
    if this_is_lhs {
        return Some((ty::ENUM, 0));
    }
    if this_is_rhs {
        return Some((ty::ENUM, bit(md::STATIC)));
    }
    None
}

/// Whether `tok` lies anywhere within `parent`'s **first child node** (the
/// `EXPR_FIELD_REF` lhs subtree) — the `name_resolution.rs` lhs test.
fn is_in_first_child(parent: &SyntaxNode, tok: &SyntaxToken) -> bool {
    let Some(first) = parent.children().next() else {
        return false;
    };
    let tr = tok.text_range();
    first.text_range().contains_range(tr)
}

/// Encode the classified stream into the LSP relative delta format, with
/// `deltaStartChar`/`length` measured in the **negotiated** encoding via
/// L1's authoritative [`LineIndex`] (no second converter — §11.32 boundary
/// untouched). Multi-line comment tokens are split into one token per line
/// (LSP `multilineTokenSupport` is off by default — a token spanning lines
/// is rejected/mis-rendered by clients; rust-analyzer splits identically).
fn encode(
    classified: &[Classified],
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Vec<SemanticToken> {
    // Expand multi-line tokens into per-line single-line pieces FIRST, in
    // order, so the relative encoding below stays a simple monotone walk.
    let mut pieces: Vec<(
        /*line*/ u32,
        /*char*/ u32,
        /*len*/ u32,
        u32,
        u32,
    )> = Vec::new();
    for c in classified {
        let start_pos = li.position(text, c.start, enc);
        let end_pos = li.position(text, c.end, enc);
        if start_pos.line == end_pos.line {
            // Single-line: length is the negotiated-unit delta on the line.
            pieces.push((
                start_pos.line,
                start_pos.character,
                end_pos.character - start_pos.character,
                c.ty,
                c.mods,
            ));
            continue;
        }
        // Multi-line (only `BlockComment`/`DocComment` reach here): emit one
        // piece per covered line. Line L's piece spans [line_start_or_token_
        // start, line_end_or_token_end) in negotiated units. We re-walk the
        // byte range line by line using the LineIndex (no second converter).
        for line in start_pos.line..=end_pos.line {
            // Byte range of this line's slice of the token:
            //  - first line: from c.start to the line's end (the '\n');
            //  - last line: from the line start to c.end;
            //  - middle:    the whole line content.
            let line_start_byte = li.offset(
                text,
                tower_lsp::lsp_types::Position { line, character: 0 },
                enc,
            );
            // End of this line's *content* (byte index of the '\n', or
            // buffer end on the last line) — never includes the newline.
            let next_line_start = li.offset(
                text,
                tower_lsp::lsp_types::Position {
                    line: line + 1,
                    character: 0,
                },
                enc,
            );
            let line_content_end = if next_line_start > line_start_byte {
                // strip the '\n' that begins the next line
                next_line_start.saturating_sub(1)
            } else {
                next_line_start
            };
            let piece_start = c.start.max(line_start_byte);
            let piece_end = c.end.min(line_content_end);
            if piece_end <= piece_start {
                continue; // empty slice (e.g. a blank line inside `/* */`)
            }
            let sp = li.position(text, piece_start, enc);
            let ep = li.position(text, piece_end, enc);
            pieces.push((
                sp.line,
                sp.character,
                ep.character - sp.character,
                c.ty,
                c.mods,
            ));
        }
    }

    // Relative-encode: deltaLine vs previous token's line; deltaStartChar
    // vs previous token's start char, RESET when the line changes (the LSP
    // relative-encoding contract).
    let mut out: Vec<SemanticToken> = Vec::with_capacity(pieces.len());
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;
    for (line, ch, len, ty, mods) in pieces {
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { ch - prev_char } else { ch };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: len,
            token_type: ty,
            token_modifiers_bitset: mods,
        });
        prev_line = line;
        prev_char = ch;
    }
    out
}

/// `textDocument/semanticTokens/full` — every token in the buffer.
///
/// `table`+`cst` come from the SAME single `analyze()` the diagnostics/
/// symbol/hover/completion/references path runs (Doc 26 §3/§8 — no second
/// analysis, no parallel classifier). Ranges via L1's `LineIndex` in the
/// negotiated encoding (no second converter — §11.32 boundary intact).
pub fn semantic_tokens_full(
    table: &SymbolTable,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> SemanticTokens {
    let classified = classify(table, cst);
    SemanticTokens {
        result_id: None,
        data: encode(&classified, li, text, enc),
    }
}

/// `textDocument/semanticTokens/range` — Doc 26 §8 L6 explicitly specifies
/// `full + range`. Same classification (the one reused analysis), then the
/// stream is filtered to tokens that **overlap** the requested byte range
/// before delta-encoding (the encoding is recomputed for the subset so the
/// first token's deltas are relative to the start of the *response*, per
/// the LSP relative-encoding contract — a range response is a self-contained
/// token stream, not a slice of the full one).
pub fn semantic_tokens_range(
    table: &SymbolTable,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
    range_start_byte: u32,
    range_end_byte: u32,
) -> SemanticTokens {
    let classified: Vec<Classified> = classify(table, cst)
        .into_iter()
        // Overlap test: a token [s,e) is in range iff s < range_end AND
        // e > range_start (half-open, the usual interval-overlap rule).
        .filter(|c| c.start < range_end_byte && c.end > range_start_byte)
        .collect();
    SemanticTokens {
        result_id: None,
        data: encode(&classified, li, text, enc),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn setup(src: &str) -> (SymbolTable, SyntaxNode, LineIndex) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        let li = LineIndex::new(src);
        (a.symbol_table, cst, li)
    }

    /// Decode the relative stream back to absolute `(line, char, len, ty,
    /// mods)` tuples — the inverse of `encode`, used by the unit + the
    /// §5.4 client tests so assertions read against ABSOLUTE positions
    /// (a delta-math regression makes the decoded stream wrong → fails).
    fn decode(toks: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
        let mut out = Vec::new();
        let mut line = 0u32;
        let mut ch = 0u32;
        for t in toks {
            if t.delta_line == 0 {
                ch += t.delta_start;
            } else {
                line += t.delta_line;
                ch = t.delta_start;
            }
            out.push((line, ch, t.length, t.token_type, t.token_modifiers_bitset));
        }
        out
    }

    #[test]
    fn legend_indices_match_doc14_s10_order() {
        let l = legend();
        // Doc 14 §2/§10 order, position = index. A drift here would make
        // every encoded `tokenType` integer reference the wrong name.
        assert_eq!(
            l.token_types[ty::NAMESPACE as usize],
            SemanticTokenType::NAMESPACE
        );
        assert_eq!(l.token_types[ty::TYPE as usize], SemanticTokenType::TYPE);
        assert_eq!(l.token_types[ty::ENUM as usize], SemanticTokenType::ENUM);
        assert_eq!(
            l.token_types[ty::FUNCTION as usize],
            SemanticTokenType::FUNCTION
        );
        assert_eq!(
            l.token_types[ty::VARIABLE as usize],
            SemanticTokenType::VARIABLE
        );
        assert_eq!(
            l.token_types[ty::KEYWORD as usize],
            SemanticTokenType::KEYWORD
        );
        assert_eq!(
            l.token_types[ty::STRING as usize],
            SemanticTokenType::STRING
        );
        assert_eq!(
            l.token_types[ty::NUMBER as usize],
            SemanticTokenType::NUMBER
        );
        assert_eq!(
            l.token_types[ty::OPERATOR as usize],
            SemanticTokenType::OPERATOR
        );
        assert_eq!(
            l.token_types[ty::COMMENT as usize],
            SemanticTokenType::COMMENT
        );
        assert_eq!(
            l.token_types[ty::DECORATOR as usize],
            SemanticTokenType::DECORATOR
        );
        assert_eq!(
            l.token_modifiers[md::DECLARATION as usize],
            SemanticTokenModifier::DECLARATION
        );
        assert_eq!(
            l.token_modifiers[md::READONLY as usize],
            SemanticTokenModifier::READONLY
        );
        assert_eq!(
            l.token_modifiers[md::STATIC as usize],
            SemanticTokenModifier::STATIC
        );
        // The reserved (advertised, never-emitted) `deprecated` slot.
        assert_eq!(l.token_modifiers[2], SemanticTokenModifier::DEPRECATED);
    }

    #[test]
    fn state_decl_carries_declaration_modifier_use_does_not() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {}\n}\n";
        let (t, cst, li) = setup(src);
        let toks = decode(&semantic_tokens_full(&t, &cst, &li, src, OffsetEncoding::Utf8).data);

        // Absolute position of the `B` declaration name in `state B {}`.
        let decl_b = src.find("state B").unwrap() + 6;
        let dp = li.position(src, decl_b as u32, OffsetEncoding::Utf8);
        let dtok = toks
            .iter()
            .find(|(l, c, ..)| *l == dp.line && *c == dp.character)
            .expect("a token at the `B` declaration");
        assert_eq!(dtok.2, 1, "`B` length 1");
        assert_eq!(dtok.3, ty::TYPE, "a state name is legend type `type`");
        assert_eq!(
            dtok.4,
            bit(md::DECLARATION),
            "a state DECLARATION carries the `declaration` modifier"
        );

        // The `B` USED as a transition target in `on GO -> B`.
        let use_b = src.find("-> B").unwrap() + 3;
        let up = li.position(src, use_b as u32, OffsetEncoding::Utf8);
        let utok = toks
            .iter()
            .find(|(l, c, ..)| *l == up.line && *c == up.character)
            .expect("a token at the `B` use");
        assert_eq!(utok.3, ty::TYPE, "the same state name is still `type`");
        assert_eq!(
            utok.4, 0,
            "a state USE has NO `declaration` modifier (decl-vs-ref)"
        );
    }

    #[test]
    fn event_ctx_extern_keyword_comment_each_correct_type() {
        let src = "language fsm 2.0\nmachine M {\n  // a line comment\n  context { n: u8 = 0 }\n  events { GO }\n  pure extern ok(): bool\n  initial A\n  state A {\n    on GO [ctx.n == 0 && ok()] -> A\n  }\n}\n";
        let (t, cst, li) = setup(src);
        let toks = decode(&semantic_tokens_full(&t, &cst, &li, src, OffsetEncoding::Utf8).data);
        let kind_at = |needle: &str, plus: usize| -> (u32, u32) {
            let b = src.find(needle).unwrap() + plus;
            let p = li.position(src, b as u32, OffsetEncoding::Utf8);
            let tk = toks
                .iter()
                .find(|(l, c, ..)| *l == p.line && *c == p.character)
                .unwrap_or_else(|| panic!("token at {needle:?}+{plus}"));
            (tk.3, tk.4)
        };
        // Event use after `on `.
        assert_eq!(kind_at("on GO", 3).0, ty::ENUM, "event name → `enum`");
        // `ctx.n` field.
        assert_eq!(
            kind_at("ctx.n", 4).0,
            ty::VARIABLE,
            "ctx field → `variable`"
        );
        // extern call `ok()` in guard.
        assert_eq!(
            kind_at("ok()", 0).0,
            ty::FUNCTION,
            "extern name → `function`"
        );
        // The `state` keyword.
        assert_eq!(
            kind_at("state A", 0).0,
            ty::KEYWORD,
            "a reserved keyword → `keyword`"
        );
        // The `// a line comment` trivia.
        assert_eq!(
            kind_at("// a line", 0).0,
            ty::COMMENT,
            "a line comment → `comment`"
        );
        // The `->` operator.
        assert_eq!(kind_at("-> A", 0).0, ty::OPERATOR, "`->` → `operator`");
        // The `0` integer literal in the context default.
        assert_eq!(kind_at("= 0 }", 2).0, ty::NUMBER, "int literal → `number`");
    }

    #[test]
    fn deltas_are_monotone_and_decode_to_sorted_absolute_positions() {
        let src = "language fsm 2.0\nmachine M {\n  initial A\n  state A {}\n}\n";
        let (t, cst, li) = setup(src);
        let raw = semantic_tokens_full(&t, &cst, &li, src, OffsetEncoding::Utf8).data;
        // Every deltaLine ≥ 0 by construction; within a line deltaStartChar
        // is non-negative; decoded positions strictly non-decreasing.
        let dec = decode(&raw);
        for w in dec.windows(2) {
            let (al, ac, ..) = w[0];
            let (bl, bc, ..) = w[1];
            assert!(
                bl > al || (bl == al && bc >= ac),
                "decoded stream must be position-sorted: {:?} then {:?}",
                w[0],
                w[1]
            );
        }
        // Spot: the first emitted token is `language` (the leading `fsm`
        // keyword line) at line 0 char 0.
        assert_eq!(dec[0].0, 0, "first token on line 0");
    }

    #[test]
    fn structural_punctuation_emits_no_token() {
        let src = "language fsm 2.0\nmachine M {\n  initial A\n  state A {}\n}\n";
        let (t, cst, li) = setup(src);
        let toks = decode(&semantic_tokens_full(&t, &cst, &li, src, OffsetEncoding::Utf8).data);
        // The `{` after `machine M ` has NO legend slot (Doc 14 §10) → no
        // token at its position (TextMate handles it — coexistence).
        let brace = src.find("M {").unwrap() + 2;
        let bp = li.position(src, brace as u32, OffsetEncoding::Utf8);
        assert!(
            !toks
                .iter()
                .any(|(l, c, len, ..)| *l == bp.line && *c == bp.character && *len == 1),
            "structural `{{` must NOT get a semantic token"
        );
    }

    #[test]
    fn range_request_is_a_self_contained_substream() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> B\n  }\n  state B {}\n}\n";
        let (t, cst, li) = setup(src);
        // Range covering only `state B {}` (the last decl line).
        let s = src.find("state B").unwrap() as u32;
        let e = (src.find("state B").unwrap() + "state B {}".len()) as u32;
        let r = semantic_tokens_range(&t, &cst, &li, src, OffsetEncoding::Utf8, s, e);
        let dec = decode(&r.data);
        assert!(!dec.is_empty(), "range over `state B {{}}` has tokens");
        // First token's deltas are relative to the RESPONSE start (its
        // decoded absolute line is the `state B` line, not 0).
        let sb_line = li.position(src, s, OffsetEncoding::Utf8).line;
        assert_eq!(
            dec[0].0, sb_line,
            "range substream decodes to absolute positions (relative to response start)"
        );
        // Only tokens on the `state B {}` line are present (the `on GO ->
        // B` line is outside the range).
        for (l, ..) in &dec {
            assert_eq!(*l, sb_line, "no out-of-range token leaked in");
        }
    }

    #[test]
    fn multibyte_comment_splits_per_line_and_shifts_following_token() {
        // A 2-line block comment containing a 4-byte 🚀 and 2-byte Cyrillic
        // (lexer-valid trivia only — Cyrillic IDENTIFIERS explode the
        // ASCII-only lexer, the standing lesson). The state name AFTER the
        // comment on the SAME line must have a column that differs UTF-8 vs
        // UTF-16 (a byte/scalar shim fails this).
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    /* 🚀\n    ы */ on GO -> A\n  }\n}\n";
        let (t, cst, li) = setup(src);

        for enc in [OffsetEncoding::Utf8, OffsetEncoding::Utf16] {
            let toks = decode(&semantic_tokens_full(&t, &cst, &li, src, enc).data);
            // The block comment spans 2 lines → at least 2 comment pieces,
            // none spanning lines.
            let comment_lines: Vec<u32> = toks
                .iter()
                .filter(|(.., ty, _)| *ty == ty::COMMENT)
                .map(|(l, ..)| *l)
                .collect();
            assert!(
                comment_lines.len() >= 2,
                "[{enc:?}] multi-line /* */ split into per-line pieces, got {comment_lines:?}"
            );
            // The `A` use in `on GO -> A` on the comment's closing line:
            // its column is byte-based under UTF-8, code-unit under UTF-16.
            let use_a = src.find("-> A").unwrap() + 3;
            let p = li.position(src, use_a as u32, enc);
            let tk = toks
                .iter()
                .find(|(l, c, ..)| *l == p.line && *c == p.character)
                .unwrap_or_else(|| panic!("[{enc:?}] token at the post-comment `A` use"));
            assert_eq!(tk.3, ty::TYPE, "[{enc:?}] state use is `type`");
            assert_eq!(tk.4, 0, "[{enc:?}] a use has no `declaration` modifier");
            // Round-trip the decoded position back to the byte via the SAME
            // encoding's inverse — proves the negotiated-unit math.
            let back = li.offset(
                src,
                tower_lsp::lsp_types::Position {
                    line: tk.0,
                    character: tk.1,
                },
                enc,
            ) as usize;
            assert_eq!(
                &src[back..back + 1],
                "A",
                "[{enc:?}] decoded position round-trips to the bare `A` token"
            );
        }
    }
}
