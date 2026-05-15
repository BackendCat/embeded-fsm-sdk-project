//! `textDocument/completion` — Doc 26 §5 / §8 L4 (single-file).
//!
//! ## The reuse seam (Doc 26 §8 L4, verified-vs-code per §11.33(3))
//!
//! L4 does **not** build a parallel context detector. It reuses L3's
//! [`crate::capabilities::resolve`] CST-traversal substrate — the SAME
//! `enclosing` node-ancestry walk and `in_guard` predicate the L3 resolver
//! uses to classify a cursor — plus one shared
//! [`resolve::prev_significant_token`] primitive (added to `resolve.rs`
//! alongside its sibling token-discovery helpers, not duplicated here).
//! L3's resolver answers "what declared entity *is* the identifier under
//! the cursor?"; completion answers the dual "what may legally be typed
//! *here*?" The discriminator is the **preceding non-trivia token** plus
//! the **enclosing-node `SyntaxKind`** — empirically verified against the
//! resilient parser's real output at each cursor context (an incomplete
//! `on |` / `-> |` / `[|` still yields a usable tree: the parser was
//! written LSP-resilience-aware, Doc 26 §4.4). This is the rust-analyzer
//! completion model and mirrors `resolve.rs::classify`'s philosophy (same
//! ancestry kinds, same `parent_ancestors` traversal) rather than a new
//! ad-hoc detector.
//!
//! ## Candidate sourcing (verified vs `symbol_table.rs`)
//!
//! Every name candidate comes from the **one** `Analysis.symbol_table`
//! threaded since L2 (no second analysis, Doc 26 §3/§8). Verified against
//! `crates/fsm-analyzer/src/symbol_table.rs`: `MachineSymbols` exposes
//! ordered `events` / `externs`(+`extern_pure`) / `consts` / `enums`(with
//! `variants`) / `context_fields`(+`context_field_types`) / `states`, plus
//! file-level `file_consts` / `file_enums` / `file_externs`. That is
//! exactly the set the §5 `completion` row needs. **Doc 26 §5 precision
//! (the recurring discipline):** the §5 row's shorthand lists
//! "events/states/externs/`ctx.`/`payload.`" — the `symbol_table` genuinely
//! provides the first four (verified above), but it carries **no per-event
//! payload schema** (`payload.<field>` is event-specific and lives only in
//! the lowered `Ir::EventObject.payload`, exactly as §11.34(4) already
//! established for hover). Payload-member completion is therefore an
//! `Ir`-sourced concern (L7-adjacent, Doc 26 §5 lists `payload.` only for
//! the future inlay/snippet seam); L4 scopes it OUT rather than half-wiring
//! it from a table that does not hold it — the §5 row is shorthand-imprecise
//! on `payload.` exactly as it was on L2's `StateEntry.shape`, NOT a code
//! defect (flagged precisely, not papered over, not overclaimed; the four
//! `symbol_table`-backed candidate classes ARE accurate as written).
//!
//! ## Keyword source
//!
//! The DSL keyword list is **Doc 04 §1.5** — the single normative registry
//! (Doc 04 §1.5 header: "All other documents … MUST cross-reference this
//! section and MUST NOT redefine the list inline"; Doc 14 §4 forbids
//! inlining). [`DSL_KEYWORDS`] is a verbatim transcription with a test that
//! pins it to the spec list so a future keyword addition that misses this
//! file fails CI.
//!
//! ## Wrong-context exclusion (as load-bearing as the positive cases)
//!
//! The classifier returns exactly one [`CompletionContext`]; each context
//! emits ONLY its context-correct candidate classes. An event name can
//! never appear in a transition-target completion, a state name never in a
//! trigger completion, keywords never inside a guard expression, etc. —
//! offering wrong-context garbage is silent-mislead-adjacent (a
//! cardinal-sin-adjacent UX failure, Doc 26 §8 L4). An unclassifiable
//! cursor yields **no** completion ([`CompletionContext::Unknown`] → empty),
//! never a dump of every symbol (the spec-correct "nothing meaningful
//! here", never noise).
//!
//! ## One position converter
//!
//! The cursor `Position` → byte mapping is done by the caller
//! (`server.rs`) via the ONE authoritative `position::LineIndex::offset`
//! (the L3 inverse) before calling in here — no second converter (Doc 26
//! §4.1 / §11.32 DRIFT-2 boundary untouched). L4 emits no `TextEdit`
//! (plain `label`/`insert_text` items, client-anchored at the cursor), so
//! no range math is introduced; if a future wave adds replace ranges they
//! MUST go through that same `LineIndex` (asserted by the §5.4 non-ASCII
//! both-encoding test, which exercises the cursor-mapping path).

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind,
};

use fsm_analyzer::scope::Scope;
use fsm_analyzer::symbol_table::SymbolTable;
use fsm_parser::cst::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::capabilities::resolve::{enclosing, in_guard, prev_significant_token};

/// The DSL keyword set — **verbatim** from Doc 04 §1.5 (the single
/// normative keyword registry; Doc 14 §4 forbids redefining it inline).
///
/// Order is the spec's reading order. Contextual keywords (`entry`,
/// `exit`, `events`, `queue`, `if`, `while`, `for`, `entry_point`,
/// `exit_point`, `likely`, `rare`) are deliberately **excluded** — Doc 04
/// §1.5 keeps them out of the reserved list because they are ordinary
/// identifiers outside their syntactic position; suggesting them as
/// reserved keywords would misrepresent the grammar. (`fsm` is the
/// language-tag contextual word, likewise excluded.)
///
/// Pinned to the spec by [`tests::dsl_keywords_match_doc04_s1_5`].
pub const DSL_KEYWORDS: &[&str] = &[
    "after",
    "as",
    "bool",
    "cancel",
    "choice",
    "composite",
    "const",
    "context",
    "deep_history",
    "defer",
    "done",
    "else",
    "enum",
    "every",
    "export",
    "extern",
    "f32",
    "f64",
    "false",
    "feature",
    "final",
    "fork",
    "i8",
    "i16",
    "i32",
    "i64",
    "import",
    "initial",
    "is",
    "join",
    "junction",
    "language",
    "machine",
    "ms",
    "on",
    "opaque",
    "parallel",
    "priority",
    "pure",
    "raise",
    "region",
    "schedule",
    "send",
    "shallow_history",
    "state",
    "submachine",
    "target",
    "to",
    "true",
    "u8",
    "u16",
    "u32",
    "u64",
];

/// What may legally be completed at the cursor. One variant per Doc 26 §8
/// L4 context; the classifier returns exactly one so wrong-context
/// candidates are *structurally* impossible (the exclusion guarantee).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionContext {
    /// Transition target after `->` / `~>` (or fork/join/initial/timer
    /// target, `done ->`, choice/junction branch) → **state names**.
    TransitionTarget,
    /// Trigger position after `on` → **event names**.
    Trigger,
    /// `raise` / `defer` event position → **event names** (same candidate
    /// class as Trigger; distinct context for clarity/extension).
    RaiseDeferEvent,
    /// `send EVENT to` machine position → **machine names**.
    SendMachine,
    /// Guard expression (inside `[ … ]`) → **context fields + `pure`
    /// externs + consts + enum types** (the values legal in a guard, Doc
    /// 04 §8.5 / §2.5: a guard may call only `pure` externs).
    GuardExpr,
    /// Action expression (after `:` on an external/local transition, or in
    /// an `entry`/`exit` block) → **context fields + all externs + consts
    /// + enum types + the action statement keywords** (`raise`/`send`/
    /// `defer`/`cancel`).
    ActionExpr,
    /// Just after `ctx.` → **context field names only** (no kind/keyword
    /// noise — exactly the analyzer's `ctx`-prefixed field-ref space).
    CtxField,
    /// Statement/declaration start inside a `state` body → the state-item
    /// keywords (`on`/`entry`/`exit`/`after`/`every`/`initial`/`final`/
    /// nested `state`/…) — sourced from the one [`DSL_KEYWORDS`] list.
    StateBodyStart,
    /// Declaration start inside a `machine` body → machine-item keywords
    /// (`events`/`context`/`extern`/`initial`/`state`/`region`/…).
    MachineBodyStart,
    /// File top level (after the `language` decl, outside any machine) →
    /// top-level keywords (`machine`/`const`/`enum`/`extern`/`import`/
    /// `feature`/`submachine`).
    FileTopLevel,
    /// Cursor is not at a position where a meaningful, context-correct
    /// completion exists (inside a comment, on punctuation mid-token, an
    /// unclassifiable spot). → **no items** (never a dump-everything).
    Unknown,
}

/// Classify the cursor at `byte` into exactly one [`CompletionContext`].
///
/// Reuses L3's [`prev_significant_token`] + [`enclosing`] + [`in_guard`]
/// (the SAME traversal substrate the L3 resolver classifies with — Doc 26
/// §8 L4 reuse seam). The empirically-verified discriminator is the
/// preceding non-trivia token's `SyntaxKind`, refined by the enclosing
/// node's kind; an incomplete edit still yields a usable tree (resilient
/// parser, Doc 26 §4.4) so this is robust on partial input.
pub fn context_at(cst: &SyntaxNode, byte: u32) -> CompletionContext {
    let Some(prev) = prev_significant_token(cst, byte) else {
        return CompletionContext::Unknown;
    };
    // A cursor *inside* a comment/trivia token never completes DSL
    // candidates (its prev-significant is found by skipping trivia, but if
    // the cursor token itself is a comment we must not classify it). The
    // `token_at_offset` left token is what the user is in.
    if cursor_is_in_trivia(cst, byte) {
        return CompletionContext::Unknown;
    }

    let pkind = prev.kind();

    // --- ctx. field: the token immediately before is a `.` whose LHS
    //     ident is `ctx` (exactly the analyzer's ctx-field-ref rule,
    //     `name_resolution.rs::check_expr`; mirrored in resolve.rs's
    //     EXPR_FIELD_REF arm). Only then is it the field-name space. -----
    if pkind == SyntaxKind::Dot {
        return if dot_lhs_is_ctx(&prev) {
            CompletionContext::CtxField
        } else {
            // `payload.` / `Enum.` / submachine field — NOT a
            // symbol_table-backed completion (payload schema is IR-only,
            // §11.34(4); enum-member completion is the GuardExpr/ActionExpr
            // enum-type path, not a trailing-field path). No guess.
            CompletionContext::Unknown
        };
    }

    // --- transition target: prev token is the transition arrow. --------
    if pkind == SyntaxKind::Arrow || pkind == SyntaxKind::HistoryArrow {
        return CompletionContext::TransitionTarget;
    }

    // --- trigger: prev token is `on` (the event-trigger keyword). ------
    if pkind == SyntaxKind::KwOn {
        return CompletionContext::Trigger;
    }
    // Partial trigger ident `on G|`: prev is an Ident whose own
    // prev-significant is `on` AND it sits directly under a transition
    // node at ident-index 0 (mirrors resolve.rs's TRANSITION_DECL ident-0
    // = event rule). This makes `on G|` still a Trigger context.
    if pkind == SyntaxKind::Ident {
        if let Some(c) = ident_partial_context(&prev) {
            return c;
        }
    }

    // --- raise / defer event position. ---------------------------------
    if pkind == SyntaxKind::KwRaise || pkind == SyntaxKind::KwDefer {
        return CompletionContext::RaiseDeferEvent;
    }

    // --- send EVENT to MACHINE: prev is `to` inside a STMT_SEND. -------
    if pkind == SyntaxKind::KwTo && enclosing(&prev, SyntaxKind::STMT_SEND).is_some() {
        return CompletionContext::SendMachine;
    }

    // --- initial / fork / join / timer / branch target: these are all
    //     state-target positions, exactly the resolve.rs state-ref arms.
    //     The empirically-verified anchor is the keyword/arrow that
    //     introduces the target.
    if pkind == SyntaxKind::KwInitial {
        return CompletionContext::TransitionTarget;
    }

    // --- guard expression: cursor is within a GUARD_CLAUSE (the SAME
    //     in_guard predicate L3 uses). Covers `[|`, `[c|`, `[ctx.n == |`.
    if in_guard(&prev) || prev_token_opens_guard(&prev) {
        return CompletionContext::GuardExpr;
    }

    // --- action expression: after `:` on an external/local transition,
    //     or inside an entry/exit ACTION_BLOCK. -------------------------
    if is_action_position(&prev) {
        return CompletionContext::ActionExpr;
    }

    // --- structural starts: prev token is an opening `{` (or the `}` /
    //     end of a previous sibling decl) and the enclosing node tells us
    //     which body we are in. Verified ancestry:
    //       STATE_DECL    -> state-item keywords
    //       MACHINE_DECL  -> machine-item keywords
    //       FILE          -> top-level keywords
    if pkind == SyntaxKind::LBrace || pkind == SyntaxKind::RBrace || pkind == SyntaxKind::Semicolon
    {
        return body_start_context(&prev);
    }

    // After a language version literal at file scope → top-level start.
    if enclosing(&prev, SyntaxKind::MACHINE_DECL).is_none()
        && enclosing(&prev, SyntaxKind::LANGUAGE_DECL).is_some()
    {
        return CompletionContext::FileTopLevel;
    }

    CompletionContext::Unknown
}

/// Build the context-correct completion items for the cursor at `byte`.
///
/// Sources every name candidate from the single threaded
/// `symbol_table` (no second analysis), keywords from [`DSL_KEYWORDS`]
/// (Doc 04 §1.5). Returns an **empty** Vec for
/// [`CompletionContext::Unknown`] — the spec-correct "nothing meaningful
/// here", never a dump of all symbols (the wrong-context-noise sin).
pub fn completions(table: &SymbolTable, cst: &SyntaxNode, byte: u32) -> Vec<CompletionItem> {
    let ctx = context_at(cst, byte);
    // The enclosing machine (single-file scope, Doc 26 §4.6) — resolved by
    // NAME via the table's own `machine_index`, exactly as resolve.rs does,
    // so candidate lists are the cursor machine's declared symbols.
    let machine_idx = enclosing_machine_idx(table, cst, byte);

    match ctx {
        CompletionContext::TransitionTarget => state_items(table, machine_idx),
        CompletionContext::Trigger | CompletionContext::RaiseDeferEvent => {
            event_items(table, machine_idx)
        }
        CompletionContext::SendMachine => machine_items(table),
        CompletionContext::CtxField => ctx_field_items(table, machine_idx),
        CompletionContext::GuardExpr => guard_value_items(table, machine_idx),
        CompletionContext::ActionExpr => action_value_items(table, machine_idx),
        CompletionContext::StateBodyStart => {
            // Doc 14 §4: state-body position offers BOTH the state-item
            // keywords AND the empty-line editor snippets (both are
            // context-correct here, and ONLY here).
            let mut v = keyword_items(STATE_BODY_KEYWORDS);
            v.extend(state_body_snippets());
            v
        }
        CompletionContext::MachineBodyStart => keyword_items(MACHINE_BODY_KEYWORDS),
        CompletionContext::FileTopLevel => keyword_items(TOP_LEVEL_KEYWORDS),
        CompletionContext::Unknown => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Candidate builders — each draws ONLY from the verified `symbol_table`
// fields (Doc 26 §3/§8: one analysis, no second pass) for exactly one
// context, so a wrong-context class is structurally unreachable.
// ---------------------------------------------------------------------------

fn state_items(table: &SymbolTable, mi: Option<usize>) -> Vec<CompletionItem> {
    let Some(mi) = mi else { return Vec::new() };
    let Some(m) = table.machines.get(mi) else {
        return Vec::new();
    };
    m.states
        .iter()
        .map(|s| CompletionItem {
            label: s.name.clone(),
            // Doc 14 §4: state names = kind 7 (Class).
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(state_detail(m, s)),
            ..Default::default()
        })
        .collect()
}

/// Human detail for a state candidate (Doc 14 §4 shows `"composite
/// state"`). Composite-vs-simple for a `StateShape::Plain` is **derived
/// from the `container_path` graph the SAME way L2's `documentSymbol`
/// derives it** (`document_symbol.rs` §: a Plain state with ≥1 child ⇒
/// composite, 0 ⇒ simple — the documented seam, NOT re-invented); a
/// pseudo-state uses its shape name. No new analysis — pure graph read of
/// the already-built table.
fn state_detail(
    m: &fsm_analyzer::symbol_table::MachineSymbols,
    s: &fsm_analyzer::symbol_table::StateEntry,
) -> String {
    use fsm_analyzer::symbol_table::StateShape;
    match s.shape {
        StateShape::Plain => {
            // Its own dotted path = container_path + name; any state whose
            // container_path equals that is a child (the L2 rule).
            let mut self_path = s.container_path.clone();
            self_path.push(s.name.clone());
            let has_child = m.states.iter().any(|o| o.container_path == self_path)
                || m.regions.iter().any(|r| r.container_path == self_path);
            if has_child {
                "composite state".to_owned()
            } else {
                "simple state".to_owned()
            }
        }
        StateShape::Final => "final state".to_owned(),
        StateShape::ShallowHistory => "shallow history".to_owned(),
        StateShape::DeepHistory => "deep history".to_owned(),
        StateShape::Choice => "choice pseudo-state".to_owned(),
        StateShape::Junction => "junction pseudo-state".to_owned(),
        StateShape::Fork => "fork pseudo-state".to_owned(),
        StateShape::Join => "join pseudo-state".to_owned(),
        StateShape::EntryPoint => "entry point".to_owned(),
        StateShape::ExitPoint => "exit point".to_owned(),
    }
}

fn event_items(table: &SymbolTable, mi: Option<usize>) -> Vec<CompletionItem> {
    let Some(mi) = mi else { return Vec::new() };
    let Some(m) = table.machines.get(mi) else {
        return Vec::new();
    };
    m.events
        .iter()
        .map(|e| CompletionItem {
            label: e.name.clone(),
            kind: Some(CompletionItemKind::EVENT),
            detail: Some("event".to_owned()),
            ..Default::default()
        })
        .collect()
}

fn machine_items(table: &SymbolTable) -> Vec<CompletionItem> {
    table
        .machines
        .iter()
        .map(|m| CompletionItem {
            label: m.name.clone(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("machine".to_owned()),
            ..Default::default()
        })
        .collect()
}

fn ctx_field_items(table: &SymbolTable, mi: Option<usize>) -> Vec<CompletionItem> {
    let Some(mi) = mi else { return Vec::new() };
    let Some(m) = table.machines.get(mi) else {
        return Vec::new();
    };
    m.context_fields
        .iter()
        .enumerate()
        .map(|(i, f)| context_field_item(f, m.context_field_types.get(i)))
        .collect()
}

/// A context-field completion item in the exact Doc 14 §4 shape: kind 5
/// (Field), `detail` = the field's type text, `labelDetails.description`
/// = `"context field"`. Type comes from the parallel `context_field_types`
/// table (the only type info `symbol_table` carries — verified vs
/// `symbol_table.rs`; a full schema would be IR, out of L4 scope).
fn context_field_item(
    f: &fsm_analyzer::symbol_table::Entry,
    ty: Option<&Option<String>>,
) -> CompletionItem {
    let ty_text = ty.and_then(|t| t.as_deref()).unwrap_or("?").to_owned();
    CompletionItem {
        label: f.name.clone(),
        kind: Some(CompletionItemKind::FIELD),
        detail: Some(ty_text),
        label_details: Some(CompletionItemLabelDetails {
            detail: None,
            description: Some("context field".to_owned()),
        }),
        ..Default::default()
    }
}

/// Guard-legal values: context fields + **`pure`** externs only (Doc 04
/// §2.5 — a guard may call only `pure` externs) + consts + enum types.
fn guard_value_items(table: &SymbolTable, mi: Option<usize>) -> Vec<CompletionItem> {
    let mut out = Vec::new();
    if let Some(m) = mi.and_then(|i| table.machines.get(i)) {
        out.extend(context_field_value_items(m));
        // pure externs only.
        for (idx, e) in m.externs.iter().enumerate() {
            if m.extern_pure.get(idx).copied().unwrap_or(false) {
                out.push(extern_item(&e.name, true));
            }
        }
        out.extend(const_items(&m.consts));
        out.extend(enum_type_items(&m.enums));
    }
    // file-scope pure externs / consts / enums are in scope too.
    for (idx, e) in table.file_externs.iter().enumerate() {
        if table.file_extern_pure.get(idx).copied().unwrap_or(false) {
            out.push(extern_item(&e.name, true));
        }
    }
    out.extend(const_items(&table.file_consts));
    out.extend(enum_type_items(&table.file_enums));
    out
}

/// Action-legal values: context fields + **all** externs (pure or not) +
/// consts + enum types + the action-statement keywords.
fn action_value_items(table: &SymbolTable, mi: Option<usize>) -> Vec<CompletionItem> {
    let mut out = Vec::new();
    if let Some(m) = mi.and_then(|i| table.machines.get(i)) {
        out.extend(context_field_value_items(m));
        for (idx, e) in m.externs.iter().enumerate() {
            let pure = m.extern_pure.get(idx).copied().unwrap_or(false);
            out.push(extern_item(&e.name, pure));
        }
        out.extend(const_items(&m.consts));
        out.extend(enum_type_items(&m.enums));
    }
    for (idx, e) in table.file_externs.iter().enumerate() {
        let pure = table.file_extern_pure.get(idx).copied().unwrap_or(false);
        out.push(extern_item(&e.name, pure));
    }
    out.extend(const_items(&table.file_consts));
    out.extend(enum_type_items(&table.file_enums));
    // Action-statement keywords (Doc 04 §1.5 subset legal to *start* an
    // action) — drawn from the one DSL_KEYWORDS list (no inline redefine).
    out.extend(keyword_items(ACTION_KEYWORDS));
    out
}

fn context_field_value_items(
    m: &fsm_analyzer::symbol_table::MachineSymbols,
) -> impl Iterator<Item = CompletionItem> + '_ {
    m.context_fields
        .iter()
        .enumerate()
        .map(move |(i, f)| context_field_item(f, m.context_field_types.get(i)))
}

fn extern_item(name: &str, pure: bool) -> CompletionItem {
    CompletionItem {
        label: name.to_owned(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(if pure {
            "pure extern".to_owned()
        } else {
            "extern".to_owned()
        }),
        ..Default::default()
    }
}

fn const_items(consts: &[fsm_analyzer::symbol_table::Entry]) -> Vec<CompletionItem> {
    consts
        .iter()
        .map(|c| CompletionItem {
            label: c.name.clone(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some("const".to_owned()),
            ..Default::default()
        })
        .collect()
}

fn enum_type_items(enums: &[fsm_analyzer::symbol_table::EnumEntry]) -> Vec<CompletionItem> {
    enums
        .iter()
        .map(|e| CompletionItem {
            label: e.name.clone(),
            kind: Some(CompletionItemKind::ENUM),
            detail: Some(format!("enum {{ {} }}", e.variants.join(", "))),
            ..Default::default()
        })
        .collect()
}

fn keyword_items(keywords: &[&str]) -> Vec<CompletionItem> {
    keywords.iter().map(|kw| keyword_item(kw)).collect()
}

/// One keyword completion item in the Doc 14 §4 shape: kind 14 (Keyword),
/// `detail: "(keyword)"`. For the three structural-block keywords Doc 14
/// §4 gives an explicit snippet body (`machine`/`composite`/`parallel`,
/// `insertTextFormat: 2`/Snippet) the `insertText` is supplied **verbatim
/// from Doc 14 §4** — Doc 26 §5's `completion` row defers the snippet
/// table to Doc 22 §9 / Doc 14 §4; only the three the spec writes out are
/// emitted here (the rest are plain keyword inserts — NOT inventing
/// snippets the spec does not define).
fn keyword_item(kw: &str) -> CompletionItem {
    let snippet = match kw {
        "machine" => Some("machine ${1:Name} {\n    $0\n}"),
        "composite" => Some(
            "composite ${1:Name} {\n    initial ${2:Sub}\n\n    state ${2:Sub} {\n        $0\n    }\n}",
        ),
        "parallel" => Some(
            "parallel ${1:Name} {\n    region ${2:First} {\n        initial ${3:Sub}\n        state ${3:Sub} { $0 }\n    }\n}",
        ),
        _ => None,
    };
    CompletionItem {
        label: kw.to_owned(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some("(keyword)".to_owned()),
        insert_text: snippet.map(str::to_owned),
        insert_text_format: snippet.map(|_| InsertTextFormat::SNIPPET),
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!("FSM-Lang keyword `{kw}` (Doc 04 §1.5)"),
        })),
        ..Default::default()
    }
}

/// The empty-state-body editor snippets — **verbatim from Doc 14 §4**
/// ("Snippets — offered on empty lines within state bodies", kind 15).
/// These are distinct from keyword items: they are full transition/handler
/// templates. Emitted ONLY in `StateBodyStart` (their Doc-14-specified
/// position) alongside the state-body keywords, never elsewhere.
fn state_body_snippets() -> Vec<CompletionItem> {
    const SNIPPETS: &[(&str, &str)] = &[
        ("on EVENT -> TARGET", "on ${1:EVENT} -> ${2:Target}"),
        ("after Xms -> TARGET", "after ${1:1000} ms -> ${2:Timeout}"),
        ("entry action", "entry : ${1:action}"),
        ("exit action", "exit : ${1:action}"),
    ];
    SNIPPETS
        .iter()
        .map(|(label, body)| CompletionItem {
            label: (*label).to_owned(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some((*body).to_owned()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        })
        .collect()
}

// Keyword subsets — every entry is also in `DSL_KEYWORDS` (Doc 04 §1.5 is
// the single source; these are *position-filtered views* of it, asserted
// by `tests::keyword_subsets_are_subsets_of_doc04`). They are NOT a
// re-definition of the keyword list — they pick which of the §1.5 keywords
// can legally *start* a construct in each structural position.
const TOP_LEVEL_KEYWORDS: &[&str] = &[
    "machine",
    "submachine",
    "const",
    "enum",
    "extern",
    "import",
    "feature",
];
const MACHINE_BODY_KEYWORDS: &[&str] = &[
    "context", "extern", "const", "enum", "initial", "state", "region", "final", "choice",
    "junction", "fork", "join",
];
const STATE_BODY_KEYWORDS: &[&str] = &[
    "on", "after", "every", "initial", "state", "region", "final", "done", "defer",
];
const ACTION_KEYWORDS: &[&str] = &["raise", "send", "defer", "cancel"];

// ---------------------------------------------------------------------------
// Classification helpers — pure CST traversal, reusing the resolve.rs
// substrate's philosophy (ancestry + sibling-position), no analysis.
// ---------------------------------------------------------------------------

/// True when the cursor token itself is a trivia (comment/whitespace) the
/// user is editing *inside* — completion of DSL candidates there is wrong
/// (you are typing prose). Whitespace alone is fine (it is the gap between
/// tokens); only an actual comment body suppresses.
fn cursor_is_in_trivia(cst: &SyntaxNode, byte: u32) -> bool {
    let offset = rowan::TextSize::from(byte);
    let t = match cst.token_at_offset(offset) {
        rowan::TokenAtOffset::None => return false,
        rowan::TokenAtOffset::Single(t) => t,
        // At a boundary the cursor is between tokens — not "inside" a
        // comment; classify normally.
        rowan::TokenAtOffset::Between(_, _) => return false,
    };
    matches!(
        t.kind(),
        SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::DocComment
    ) && t.text_range().start() < offset
        && offset < t.text_range().end()
}

/// The LHS ident of the `EXPR_FIELD_REF` (or recovery `ERROR_NODE`) that a
/// trailing `.` belongs to — mirrors `resolve.rs::field_ref_lhs_ident`'s
/// rule (`ctx`/`payload`/enum prefix). Here we only need to know if it is
/// literally `ctx` (the analyzer's context-field-ref discriminator).
fn dot_lhs_is_ctx(dot: &SyntaxToken) -> bool {
    // The ident token immediately preceding the `.` (skipping trivia) is
    // the field-ref LHS in every well-formed AND every resilient-recovery
    // shape we verified (`[ctx.|` recovers as ERROR_NODE wrapping
    // `ctx` `.`; the EXPR_FIELD_REF shape has `ctx` as the prior ident).
    let mut prev = dot.prev_token();
    while let Some(t) = prev {
        if t.kind().is_trivia() {
            prev = t.prev_token();
            continue;
        }
        return t.kind() == SyntaxKind::Ident && t.text() == "ctx";
    }
    false
}

/// A partial identifier the user is mid-typing: decide its context from
/// where that ident node sits, reusing resolve.rs's ident-position rules
/// (TRANSITION_DECL ident-0 = trigger event; ident-1 = state target;
/// inside GUARD_CLAUSE = guard value). Returns `None` to fall through to
/// the prev-token rules for any shape this does not own.
fn ident_partial_context(ident: &SyntaxToken) -> Option<CompletionContext> {
    let parent = ident.parent()?;
    let pkind = parent.kind();
    // `on G|` — Ident directly under a transition node. Index 0 = the
    // trigger event (exactly resolve.rs's TRANSITION_DECL/LOCAL_DECL/
    // INTERNAL_DECL ident-0 = event arm).
    if matches!(
        pkind,
        SyntaxKind::TRANSITION_DECL
            | SyntaxKind::LOCAL_DECL
            | SyntaxKind::INTERNAL_DECL
            | SyntaxKind::COMPLETION_DECL
    ) {
        let idents: Vec<SyntaxToken> = parent
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| t.kind() == SyntaxKind::Ident)
            .collect();
        let this = ident.text_range();
        if idents.first().map(|t| t.text_range()) == Some(this) {
            return Some(CompletionContext::Trigger);
        }
        if idents.get(1).map(|t| t.text_range()) == Some(this) {
            return Some(CompletionContext::TransitionTarget);
        }
    }
    // Partial ident inside a guard clause (e.g. `[ca|`) → guard values.
    if in_guard(ident) {
        return Some(CompletionContext::GuardExpr);
    }
    None
}

/// Whether `prev` is the `[` that opens a guard clause (covers the very
/// first keystroke `[|` before any guard EXPR node exists).
fn prev_token_opens_guard(prev: &SyntaxToken) -> bool {
    prev.kind() == SyntaxKind::LBracket
        && (enclosing(prev, SyntaxKind::GUARD_CLAUSE).is_some()
            || enclosing(prev, SyntaxKind::TRANSITION_DECL).is_some()
            || enclosing(prev, SyntaxKind::LOCAL_DECL).is_some()
            || enclosing(prev, SyntaxKind::INTERNAL_DECL).is_some())
}

/// Whether the cursor is in an action position: prev token is the `:` of an
/// external/local transition (the action separator), or the cursor is
/// inside an `ACTION_BLOCK` / `ENTRY_DECL` / `EXIT_DECL`.
fn is_action_position(prev: &SyntaxToken) -> bool {
    if enclosing(prev, SyntaxKind::ACTION_BLOCK).is_some()
        || enclosing(prev, SyntaxKind::ENTRY_DECL).is_some()
        || enclosing(prev, SyntaxKind::EXIT_DECL).is_some()
    {
        return true;
    }
    // `… -> T : |` — the `:` directly under a transition node opens the
    // single-action form (Doc 04 §9). GUARD_CLAUSE uses `[ ]`, not `:`, so
    // a `:` here is unambiguously the action separator.
    if prev.kind() == SyntaxKind::Colon {
        return enclosing(prev, SyntaxKind::TRANSITION_DECL).is_some()
            || enclosing(prev, SyntaxKind::LOCAL_DECL).is_some()
            || enclosing(prev, SyntaxKind::INTERNAL_DECL).is_some()
            || enclosing(prev, SyntaxKind::COMPLETION_DECL).is_some()
            || enclosing(prev, SyntaxKind::AFTER_DECL).is_some()
            || enclosing(prev, SyntaxKind::EVERY_DECL).is_some();
    }
    false
}

/// Given a structural anchor token (`{` / `}` / `;`), decide which body's
/// declaration-start keywords apply from the enclosing-node ancestry —
/// exactly the verified ancestry chains (STATE_DECL ⊂ MACHINE_DECL ⊂ FILE).
/// Innermost wins (a state body inside a machine body is a state context).
fn body_start_context(anchor: &SyntaxToken) -> CompletionContext {
    // Walk ancestors nearest-first; the first body kind we hit is the
    // active scope (resolve.rs uses the same nearest-ancestor philosophy).
    for n in anchor.parent_ancestors() {
        match n.kind() {
            SyntaxKind::STATE_DECL | SyntaxKind::REGION_DECL => {
                return CompletionContext::StateBodyStart
            }
            SyntaxKind::MACHINE_DECL | SyntaxKind::SUBMACHINE_DECL => {
                return CompletionContext::MachineBodyStart
            }
            SyntaxKind::FILE => return CompletionContext::FileTopLevel,
            _ => {}
        }
    }
    CompletionContext::Unknown
}

/// The cursor's enclosing-machine index in the symbol table, resolved by
/// NAME via `machine_index` — the SAME map `resolve.rs` and the analyzer's
/// `resolve_machine` use, so candidate lists are correct regardless of
/// declaration order. `None` when the cursor is outside any machine.
fn enclosing_machine_idx(table: &SymbolTable, cst: &SyntaxNode, byte: u32) -> Option<usize> {
    let anchor = prev_significant_token(cst, byte)?;
    let machine = enclosing(&anchor, SyntaxKind::MACHINE_DECL)
        .or_else(|| enclosing(&anchor, SyntaxKind::SUBMACHINE_DECL))?;
    let name = machine
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();
    table.machine_index.get(&name).copied()
}

/// A `Scope` rooted in the cursor's machine — kept for symmetry with
/// `resolve.rs` (the analyzer resolves names through a `Scope`); completion
/// iterates the machine's tables directly (it lists *all* candidates, not
/// resolve-one), but a future scoped filter would key off this.
#[allow(dead_code)]
fn machine_scope(mi: usize) -> Scope {
    Scope::for_machine(mi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::analysis::analyze;

    fn setup(src: &str) -> (SymbolTable, SyntaxNode) {
        let a = analyze(src, Path::new("/tmp/t.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        (a.symbol_table, cst)
    }

    fn at(src: &str, needle: &str, plus: usize) -> u32 {
        (src.find(needle).expect("needle in src") + plus) as u32
    }

    fn labels(items: &[CompletionItem]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
        v.sort();
        v
    }

    /// The DSL_KEYWORDS list MUST equal Doc 04 §1.5 exactly — if a future
    /// keyword is added to the spec but not here (or vice-versa) this
    /// fails, enforcing the single-source rule (Doc 04 §1.5 / Doc 14 §4).
    #[test]
    fn dsl_keywords_match_doc04_s1_5() {
        // Verbatim from docs/04-DSL-Specification.md §1.5 (the AUTHORITATIVE
        // LIST block), excluding the contextual keywords §1.5 explicitly
        // keeps OUT of the reserved set.
        let mut spec: Vec<&str> = vec![
            "after",
            "as",
            "bool",
            "cancel",
            "choice",
            "composite",
            "const",
            "context",
            "deep_history",
            "defer",
            "done",
            "else",
            "enum",
            "every",
            "export",
            "extern",
            "f32",
            "f64",
            "false",
            "feature",
            "final",
            "fork",
            "i8",
            "i16",
            "i32",
            "i64",
            "import",
            "initial",
            "is",
            "join",
            "junction",
            "language",
            "machine",
            "ms",
            "on",
            "opaque",
            "parallel",
            "priority",
            "pure",
            "raise",
            "region",
            "schedule",
            "send",
            "shallow_history",
            "state",
            "submachine",
            "target",
            "to",
            "true",
            "u8",
            "u16",
            "u32",
            "u64",
        ];
        spec.sort();
        let mut got: Vec<&str> = DSL_KEYWORDS.to_vec();
        got.sort();
        assert_eq!(
            got, spec,
            "DSL_KEYWORDS must be a verbatim transcription of Doc 04 §1.5 \
             (the single normative keyword registry — Doc 14 §4 forbids \
             inlining a divergent list)"
        );
        // Contextual keywords MUST NOT be present (Doc 04 §1.5 keeps them
        // out of the reserved list deliberately — they are identifiers
        // outside their position; suggesting them as keywords misleads).
        for ck in [
            "entry",
            "exit",
            "events",
            "queue",
            "if",
            "while",
            "for",
            "entry_point",
            "exit_point",
            "likely",
            "rare",
            "fsm",
        ] {
            assert!(
                !DSL_KEYWORDS.contains(&ck),
                "contextual keyword {ck:?} must NOT be in the reserved list"
            );
        }
    }

    #[test]
    fn keyword_subsets_are_subsets_of_doc04() {
        for (name, set) in [
            ("TOP_LEVEL", TOP_LEVEL_KEYWORDS),
            ("MACHINE_BODY", MACHINE_BODY_KEYWORDS),
            ("STATE_BODY", STATE_BODY_KEYWORDS),
            ("ACTION", ACTION_KEYWORDS),
        ] {
            for kw in set {
                assert!(
                    DSL_KEYWORDS.contains(kw),
                    "{name} keyword {kw:?} is not in the Doc 04 §1.5 list — \
                     position views must be SUBSETS of the single registry, \
                     never a redefinition"
                );
            }
        }
    }

    #[test]
    fn transition_target_offers_states_not_events() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> \n  }\n  state B {}\n}\n";
        let (t, cst) = setup(src);
        let off = at(src, "-> \n", 3);
        assert_eq!(context_at(&cst, off), CompletionContext::TransitionTarget);
        let items = completions(&t, &cst, off);
        let ls = labels(&items);
        assert_eq!(ls, vec!["A", "B"], "transition target → state names only");
        for it in &items {
            assert_eq!(it.kind, Some(CompletionItemKind::CLASS));
        }
        // NEGATIVE exclusion: the event `GO` (which exists in scope) MUST
        // NOT appear in a transition-target completion.
        assert!(
            !ls.contains(&"GO".to_string()),
            "event `GO` must be ABSENT from a transition-target completion"
        );
    }

    #[test]
    fn trigger_offers_events_not_states() {
        let src = "language fsm 2.0\nmachine M {\n  events { GO STOP }\n  initial A\n  state A {\n    on \n  }\n  state B {}\n}\n";
        let (t, cst) = setup(src);
        let off = at(src, "on \n", 3);
        assert_eq!(context_at(&cst, off), CompletionContext::Trigger);
        let ls = labels(&completions(&t, &cst, off));
        assert_eq!(ls, vec!["GO", "STOP"], "trigger → event names only");
        // NEGATIVE: states `A`/`B` exist in scope but a trigger position
        // must NOT offer them.
        assert!(
            !ls.contains(&"A".to_string()) && !ls.contains(&"B".to_string()),
            "state names must be ABSENT from a trigger completion"
        );
    }

    #[test]
    fn ctx_dot_offers_only_context_fields() {
        let src = "language fsm 2.0\nmachine M {\n  context { n: u8 = 0\n  flag: bool = false }\n  events { GO }\n  initial A\n  state A {\n    on GO [ctx.\n  }\n}\n";
        let (t, cst) = setup(src);
        let off = at(src, "ctx.\n", 4);
        assert_eq!(context_at(&cst, off), CompletionContext::CtxField);
        let items = completions(&t, &cst, off);
        let ls = labels(&items);
        assert_eq!(ls, vec!["flag", "n"], "ctx. → context fields only");
        for it in &items {
            assert_eq!(it.kind, Some(CompletionItemKind::FIELD));
        }
        // NEGATIVE: event `GO` is in scope but `ctx.` is purely the
        // field-name space — it must be absent.
        assert!(!ls.contains(&"GO".to_string()));
    }

    #[test]
    fn guard_offers_pure_externs_and_fields_not_impure_externs() {
        let src = "language fsm 2.0\npure extern can_go(u8 n) : bool\nextern do_side() : bool\nmachine M {\n  context { n: u8 = 0 }\n  events { GO }\n  initial A\n  state A {\n    on GO [ \n  }\n}\n";
        let (t, cst) = setup(src);
        let off = at(src, "[ \n", 2);
        assert_eq!(context_at(&cst, off), CompletionContext::GuardExpr);
        let ls = labels(&completions(&t, &cst, off));
        assert!(ls.contains(&"can_go".to_string()), "pure extern offered");
        assert!(ls.contains(&"n".to_string()), "context field offered");
        // NEGATIVE: an IMPURE extern is illegal in a guard (Doc 04 §2.5) →
        // must be absent; a state name must be absent.
        assert!(
            !ls.contains(&"do_side".to_string()),
            "impure extern `do_side` must be ABSENT from a guard completion"
        );
        assert!(
            !ls.contains(&"A".to_string()),
            "state name must be ABSENT from a guard completion"
        );
    }

    #[test]
    fn statement_start_offers_keywords_not_symbols() {
        let src =
            "language fsm 2.0\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    \n  }\n}\n";
        let (t, cst) = setup(src);
        // A space in the empty state body.
        let off = at(src, "state A {\n    \n", 12);
        assert_eq!(context_at(&cst, off), CompletionContext::StateBodyStart);
        let ls = labels(&completions(&t, &cst, off));
        assert!(ls.contains(&"on".to_string()), "state-body keyword `on`");
        assert!(ls.contains(&"after".to_string()), "state-body keyword");
        // NEGATIVE: the event `GO` / state `A` are symbols, not statement
        // starters — they must be absent here.
        assert!(!ls.contains(&"GO".to_string()) && !ls.contains(&"A".to_string()));
    }

    #[test]
    fn unknown_context_yields_no_items_never_a_dump() {
        // Cursor inside a line comment — completing DSL candidates there is
        // wrong; must be empty, NOT every symbol.
        let src = "language fsm 2.0\nmachine M {\n  // a comment here\n  events { GO }\n  initial A\n  state A {}\n}\n";
        let (t, cst) = setup(src);
        let off = at(src, "comment here", 3);
        assert_eq!(context_at(&cst, off), CompletionContext::Unknown);
        assert!(
            completions(&t, &cst, off).is_empty(),
            "an unclassifiable cursor must yield NO completion (never a \
             dump of all symbols — that is the wrong-context-noise sin)"
        );
    }

    #[test]
    fn send_to_offers_machine_names() {
        let src = "language fsm 2.0\nmachine Other { events { P } initial X state X {} }\nmachine M {\n  events { P }\n  initial A\n  state A {\n    on P -> A : send P to \n  }\n}\n";
        let (t, cst) = setup(src);
        let off = at(src, "to \n", 3);
        assert_eq!(context_at(&cst, off), CompletionContext::SendMachine);
        let ls = labels(&completions(&t, &cst, off));
        assert!(
            ls.contains(&"Other".to_string()) && ls.contains(&"M".to_string()),
            "send … to → machine names, got {ls:?}"
        );
    }
}
