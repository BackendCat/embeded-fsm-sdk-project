//! `textDocument/codeAction` — Doc 14 §9 / Doc 26 §5/§8 L7.
//!
//! `codeAction` is an **edit-producing** capability: a returned
//! `WorkspaceEdit` rewrites the user's source. A wrong quick-fix silently
//! corrupts their program — the SAME cardinal silent-data-loss class as
//! `rename` (Doc 26 risk-2). The whole design here is therefore
//! **bias-hard-to-safety**: a quick-fix is offered ONLY when the fix is
//! unambiguous AND the edit is provably mechanical (a single insert or a
//! single deletion at a span the analysis already proved, span math via
//! the one [`crate::position::LineIndex`] — never string munging). A
//! missing quick-fix is a minor UX gap; a wrong edit is a catastrophe.
//!
//! ## Which Doc 14 §9 fixes ship, and why the rest are scoped OUT
//!
//! Doc 26 §8 L7 says "Doc 14 §9 quick-fix table"; the brief mandates
//! verifying each listed fix against the **real** `DiagnosticCode`s and
//! their actual emission sites, implementing ONLY those whose edit is
//! provably mechanical, and scoping-out-and-flagging the rest (Doc 00
//! §11.38) rather than shipping a possibly-corrupting edit to hit a list.
//! Verified at this HEAD:
//!
//! | Doc 14 §9 row | Emitted? | Diagnostic span | Verdict |
//! |---|---|---|---|
//! | **FSM-E0107** no initial | yes (`name_resolution.rs:43`) | whole `MACHINE_DECL` | **SHIP.** Single insert of `initial <FirstState>` after the machine's `{`; `<FirstState>` is `m.states().next()` (the SAME first-state the E0107 check counts). One inserted line, name from the parsed CST, position from the CST `{` token → provably mechanical; re-analysis clears E0107 with no new diagnostic. |
//! | **FSM-E0022** dup event | yes (`symbol_table.rs:300`) | the **duplicate** `EVENT_DECL` node | **SHIP (guarded).** The diagnostic span *is exactly* the second declaration; events are whitespace-separated (grammar `{ event_decl }`, no separator), so deleting that exact node span is a provably-correct deletion (the kept first decl + every `on EVENT` ref are untouched; re-analysis clears E0022). **Guard:** if the duplicate's preceding non-trivia sibling is a `STABLE_ID_ANNOT` (`@id(...)` authored on it), deleting only the `EVENT_DECL` would orphan a dangling `@id` — NOT mechanical, so the action is withheld for that case (bias to safety). |
//! | FSM-E0100 unknown state | yes (14 sites) | varies (transition / branch / `initial` / enum operand) | **SCOPE OUT.** "Create state X": the name must be lifted from a *heterogeneous* spanned expression (14 distinct emission contexts) and a placement chosen — neither is a single unambiguous mechanical edit. Flagged Doc 00 §11.38. |
//! | FSM-E0106 non-pure extern in guard | yes (`name_resolution.rs:445`) | the **call site**, not the extern decl | **SCOPE OUT.** "Add `pure`" mutates the *extern declaration* elsewhere, and — decisively — flipping an extern to `pure` is a **semantic assertion about the user's foreign code** the tool cannot prove: a genuinely-impure extern wrongly marked `pure` silently corrupts generated guard evaluation. Not a mechanical syntax fix. Flagged Doc 00 §11.38. |
//! | FSM-W0200 loop in action | **NEVER emitted** anywhere in the toolchain | n/a | **SCOPE OUT (dead).** Catalog-reserved only — no emission site exists, so no diagnostic can carry it and the §5.4 apply-and-verify test could not even be written. Flagged Doc 00 §11.38. |
//! | FSM-W0500 unused extern | **NEVER emitted** anywhere | n/a | **SCOPE OUT (dead).** Same as W0200 — reserved-only. Flagged Doc 00 §11.38. |
//! | FSM-E0300 nondeterminism | yes (`determinism.rs:340,359`) | ONE transition (`b`) | **SCOPE OUT.** "Add priority 1/2 to conflicting transitions" is a *multi-transition coordinated* edit, but the diagnostic spans only the second transition; producing the correct set requires re-deriving the determinism grouping — a parallel re-analysis the brief forbids — and the priority-insertion position is non-trivial. Not provably mechanical. Flagged Doc 00 §11.38. |
//! | refactor.extract ×2 (Doc 14 §9) | n/a (cursor-triggered) | n/a | **SCOPE OUT.** "Extract block/loop to extern" is a non-mechanical semantic refactor (extern signature inference, parameter capture, ABI); Doc 26 §5 sources it as "AST for refactor.extract" with no extraction substrate in-tree. Not provably safe. Flagged Doc 00 §11.38. |
//!
//! Net: **2 provably-mechanical quick-fixes** (E0107, E0022-guarded); 5
//! Doc-14-§9 codes + the 2 refactor.extract actions scoped out with a
//! documented safety reason. Per the brief, every scoped-out action IS a
//! reported scope-out, not a silent omission.
//!
//! ## One analysis, one position converter, no parallel anything
//!
//! `server.rs::code_action` runs the SAME single `analyze()` the
//! diagnostics path runs (the reuse seam, Doc 26 §3) and projects its
//! diagnostics through the SAME `to_lsp_diagnostics`. The fix is matched on
//! the **server-authoritative native `DiagnosticCode`** (not the
//! client-supplied `context.diagnostics`, which a client controls) and
//! tied back to the LSP `Diagnostic` it resolves (`action.diagnostics`).
//! Every edit `Range` goes through L1's one `LineIndex` (no string
//! slicing, no second converter — the §11.32 DRIFT-2 boundary intact).

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionResponse, Position, Range, TextEdit,
    Url, WorkspaceEdit,
};

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};
use fsm_parser::ast::{AstNode, MachineDecl, StateDecl};
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::capabilities::diagnostics::to_lsp_diagnostic;
use crate::position::{LineIndex, OffsetEncoding};

/// The two `CodeActionKind`s the server may return. Advertised verbatim in
/// `initialize` (Doc 14 §2 `codeActionProvider.codeActionKinds`) and used
/// to honour a client's `context.only` filter. Only `quickfix` is ever
/// *produced* (the two Doc 14 §9 `refactor.extract` actions are scoped
/// out, see module docs); `refactor` is advertised because Doc 14 §2's
/// `ServerCapabilities` block lists it and a client may filter on it —
/// advertising the documented set while honestly producing only the safe
/// subset is the established advertise-the-spec / ship-only-the-safe
/// discipline (the L6 `deprecated`-modifier precedent, Doc 00 §11.37(4)).
pub fn code_action_kinds() -> Vec<CodeActionKind> {
    vec![CodeActionKind::QUICKFIX, CodeActionKind::REFACTOR]
}

/// Build the `quickfix` actions whose `WorkspaceEdit` is provably
/// mechanical for the diagnostics overlapping the request `range`.
///
/// Returns `None` (no actions) when nothing safe applies — never a
/// best-effort or speculative action (the brief's "a diagnostic with NO
/// safe mechanical fix offers NO action, not a bogus one"). The set is a
/// pure projection of the SAME single `analyze()` diagnostics +
/// the parsed `cst`; no second analysis, no second position converter.
#[allow(clippy::too_many_arguments)]
pub fn code_actions(
    diagnostics: &[Diagnostic],
    cst: &SyntaxNode,
    uri: &Url,
    req_range: Range,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<CodeActionResponse> {
    let mut out: CodeActionResponse = Vec::new();
    for d in diagnostics {
        // The diagnostic must overlap the range the client asked about
        // (LSP: code actions are offered for the invocation range). The
        // overlap test is on LSP ranges via the ONE LineIndex — never a
        // byte/text heuristic.
        let d_range = li.range(text, d.span, enc);
        if !ranges_overlap(d_range, req_range) {
            continue;
        }
        let edit = match d.code {
            DiagnosticCode::E0107 => fix_e0107(d, cst, li, text, enc),
            DiagnosticCode::E0022 => fix_e0022_guarded(d, cst, li, text, enc),
            // Every other code — including the 5 other Doc 14 §9 rows and
            // the 2 refactor.extract actions — is deliberately not
            // produced (see module docs / Doc 00 §11.38). No bogus action.
            _ => None,
        };
        if let Some((title, edit)) = edit {
            let mut changes = std::collections::HashMap::new();
            changes.insert(uri.clone(), edit);
            out.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                // Tie the action to the exact LSP Diagnostic it resolves
                // (the LSP contract: a quick-fix names its diagnostic).
                diagnostics: Some(vec![to_lsp_diagnostic(d, uri, text, li, enc)]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                // A quick-fix that fully addresses the error is `preferred`
                // (the LSP "auto fix" hint) — both of ours fully resolve
                // their diagnostic by construction.
                is_preferred: Some(true),
                disabled: None,
                data: None,
            }));
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// FSM-E0107 (no `initial`) → insert `initial <FirstState>` after the
/// machine's opening `{`.
///
/// Provably mechanical: the E0107 diagnostic span is **exactly**
/// `span_of(MACHINE_DECL)` (verified `name_resolution.rs:43`), so the
/// machine node is the unique CST node with that span. The check only
/// fires when the machine has ≥1 `state` (`name_resolution.rs:41`
/// `m.states().count() > 0`), so `m.states().next()` is the same
/// first-state the analyzer counted; an `initial` naming a *declared*
/// state resolves cleanly (no E0100), and there is exactly one (no
/// E0108) — re-analysis clears E0107 with no new diagnostic. The insert
/// point is the byte just after the machine's `{` token (an `initial`
/// declaration is grammatically valid anywhere in the machine body, Doc
/// 04 §6; right after `{` is always valid). Withheld (no fix) if the
/// parse is too broken to recover the machine node, its `{`, or a named
/// first state — a conservative decline, never a guess.
fn fix_e0107(
    d: &Diagnostic,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<(String, Vec<TextEdit>)> {
    let machine_node = node_with_exact_span(cst, SyntaxKind::MACHINE_DECL, d.span)?;
    let machine = MachineDecl::cast(machine_node.clone())?;
    // First `state` declaration in source order — the SAME the E0107
    // check counts (`m.states()`); its name is what `initial` must point
    // at for the fix to actually resolve the error.
    let first_state: StateDecl = machine.states().next()?;
    let state_name = first_state.name()?;
    if state_name.is_empty() {
        return None;
    }
    // The machine's opening brace — a direct child token of MACHINE_DECL.
    let lbrace = machine_node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::LBrace)?;
    let insert_at = u32::from(lbrace.text_range().end());
    // Indent with the Doc 22 §8 `fsmLang.format.indentSize` default (4
    // spaces) on its own line — the canonical machine-body layout every
    // in-tree fixture uses. A leading '\n' puts the new decl on the line
    // after `{`; no trailing '\n' (the existing first line keeps its own
    // newline, so the body is not double-spaced).
    let new_text = format!("\n    initial {state_name}");
    let pos: Position = li.position(text, insert_at, enc);
    Some((
        format!("Add `initial {state_name}` declaration"),
        vec![TextEdit {
            // Zero-width range = a pure insertion at `insert_at`.
            range: Range {
                start: pos,
                end: pos,
            },
            new_text,
        }],
    ))
}

/// FSM-E0022 (duplicate event) → delete the duplicate declaration.
///
/// Provably mechanical **only** when the duplicate has no `@id`
/// annotation authored on it. The E0022 diagnostic span is *exactly*
/// `span_of(EVENT_DECL)` of the **second** declaration (verified
/// `symbol_table.rs:300` via `push_entry`; the first is the one kept).
/// Events are whitespace-separated (`events_block = "{" , { event_decl }
/// , "}"` — no separator token, verified `grammar/machine.rs:151`), and
/// the `EVENT_DECL` node's range already subsumes its own trailing
/// trivia, so deleting exactly that span yields valid syntax with the
/// first declaration and every `on EVENT` reference untouched →
/// re-analysis clears E0022 with no new diagnostic.
///
/// **Guard (bias to safety):** if the duplicate's nearest preceding
/// non-trivia sibling is a `STABLE_ID_ANNOT` (an `@id("…")` written on
/// the duplicate — the grammar parses it as a *separate sibling* before
/// the `EVENT_DECL`, verified), deleting only the `EVENT_DECL` would
/// leave a dangling `@id` that would then mis-bind. That edit is NOT
/// mechanically safe, so the fix is withheld for that case — a missing
/// quick-fix is a minor UX gap; a corrupting one is the cardinal sin.
fn fix_e0022_guarded(
    d: &Diagnostic,
    cst: &SyntaxNode,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Option<(String, Vec<TextEdit>)> {
    let event_node = node_with_exact_span(cst, SyntaxKind::EVENT_DECL, d.span)?;
    // SAFETY GUARD: a STABLE_ID_ANNOT immediately before this EVENT_DECL
    // means an `@id(...)` was authored on the duplicate; deleting only
    // the EVENT_DECL would orphan it. Decline (no fix) — never a
    // possibly-corrupting edit.
    if let Some(prev) = prev_significant_sibling_node(&event_node) {
        if prev.kind() == SyntaxKind::STABLE_ID_ANNOT {
            return None;
        }
    }
    let event_name = fsm_parser::ast::first_ident(&event_node).unwrap_or_default();
    let range: Range = li.range(text, d.span, enc);
    let title = if event_name.is_empty() {
        "Remove duplicate event declaration".to_owned()
    } else {
        format!("Remove duplicate event `{event_name}`")
    };
    Some((
        title,
        vec![TextEdit {
            // Delete exactly the duplicate EVENT_DECL span (incl. its own
            // trailing trivia, which the node range subsumes).
            range,
            new_text: String::new(),
        }],
    ))
}

/// The unique CST node of `kind` whose byte range is **exactly** `span`.
///
/// The analyzer emits E0107/E0022 with `span_of(node)` for a specific
/// node kind, so the node carrying the fix is the one whose range equals
/// the diagnostic span — an exact match, never a "closest"/"contains"
/// heuristic (which could pick the wrong node and corrupt the edit).
/// `None` if no node matches (a broken parse) — a conservative decline.
fn node_with_exact_span(root: &SyntaxNode, kind: SyntaxKind, span: Span) -> Option<SyntaxNode> {
    root.descendants().find(|n| {
        n.kind() == kind && {
            let r = n.text_range();
            usize::from(r.start()) == span.start && usize::from(r.end()) == span.end
        }
    })
}

/// Nearest preceding sibling that is a *node* (skipping trivia tokens and
/// any inter-element whitespace/comment tokens). Used only by the E0022
/// guard to detect a `STABLE_ID_ANNOT` authored on the duplicate event.
fn prev_significant_sibling_node(n: &SyntaxNode) -> Option<SyntaxNode> {
    let mut e = n.prev_sibling_or_token();
    while let Some(el) = e {
        if let Some(node) = el.as_node() {
            return Some(node.clone());
        }
        e = el.prev_sibling_or_token();
    }
    None
}

/// Half-open LSP-range overlap (line/character ordered). Used to decide
/// whether a diagnostic is "within the invocation range" (Doc 14 §9). A
/// pure ordering test on positions the ONE `LineIndex` produced.
fn ranges_overlap(a: Range, b: Range) -> bool {
    fn le(p: Position, q: Position) -> bool {
        (p.line, p.character) <= (q.line, q.character)
    }
    // Overlap unless one ends strictly before the other starts. Equality
    // counts as overlap so a zero-width invocation cursor exactly at a
    // diagnostic boundary still surfaces the fix.
    le(a.start, b.end) && le(b.start, a.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn setup(src: &str) -> (Vec<Diagnostic>, SyntaxNode, LineIndex, Url) {
        let a = analyze(src, Path::new("/tmp/ca.fsm"));
        let cst = fsm_parser::parse(src).syntax();
        let li = LineIndex::new(src);
        let uri = Url::parse("file:///tmp/ca.fsm").unwrap();
        (a.diagnostics, cst, li, uri)
    }

    fn full_range(li: &LineIndex, src: &str) -> Range {
        li.range(src, Span::new(0, src.len()), OffsetEncoding::Utf8)
    }

    #[test]
    fn e0107_inserts_initial_first_state_and_resolves() {
        let src =
            "language fsm 2.0\nmachine M {\n    events { GO }\n    state Idle {\n        on GO -> Idle\n    }\n}\n";
        let (diags, cst, li, uri) = setup(src);
        let r = code_actions(
            &diags,
            &cst,
            &uri,
            full_range(&li, src),
            &li,
            src,
            OffsetEncoding::Utf8,
        )
        .expect("E0107 yields a quick-fix");
        assert_eq!(r.len(), 1, "exactly one safe action");
        let CodeActionOrCommand::CodeAction(a) = &r[0] else {
            panic!("expected a CodeAction")
        };
        assert_eq!(a.kind, Some(CodeActionKind::QUICKFIX));
        let edits = &a.edit.as_ref().unwrap().changes.as_ref().unwrap()[&uri];
        assert_eq!(edits.len(), 1);
        // Apply and re-analyse: E0107 must be gone, nothing else broken.
        let e = &edits[0];
        let s = li.offset(src, e.range.start, OffsetEncoding::Utf8) as usize;
        let en = li.offset(src, e.range.end, OffsetEncoding::Utf8) as usize;
        assert_eq!(s, en, "E0107 fix is a pure insertion (zero-width range)");
        let mut buf = src.to_string();
        buf.replace_range(s..en, &e.new_text);
        assert!(
            buf.contains("initial Idle"),
            "fix inserts `initial Idle`, got {buf:?}"
        );
        let re = analyze(&buf, Path::new("/tmp/ca.fsm"));
        assert!(
            !re.diagnostics
                .iter()
                .any(|x| x.code == DiagnosticCode::E0107),
            "E0107 must be resolved, got {:?}",
            re.diagnostics.iter().map(|x| x.code).collect::<Vec<_>>()
        );
        assert!(
            re.diagnostics.is_empty(),
            "no NEW diagnostic may be introduced, got {:?}",
            re.diagnostics
        );
    }

    #[test]
    fn e0022_deletes_duplicate_and_resolves_no_stable_id() {
        let src = "language fsm 2.0\nmachine M {\n    events { GO STOP GO }\n    initial Idle\n    state Idle {\n        on GO -> Idle\n    }\n}\n";
        let (diags, cst, li, uri) = setup(src);
        let r = code_actions(
            &diags,
            &cst,
            &uri,
            full_range(&li, src),
            &li,
            src,
            OffsetEncoding::Utf8,
        )
        .expect("E0022 yields a quick-fix");
        let CodeActionOrCommand::CodeAction(a) = &r[0] else {
            panic!("expected a CodeAction")
        };
        let edits = &a.edit.as_ref().unwrap().changes.as_ref().unwrap()[&uri];
        let e = &edits[0];
        assert_eq!(e.new_text, "", "E0022 fix is a pure deletion");
        let s = li.offset(src, e.range.start, OffsetEncoding::Utf8) as usize;
        let en = li.offset(src, e.range.end, OffsetEncoding::Utf8) as usize;
        let mut buf = src.to_string();
        buf.replace_range(s..en, "");
        let re = analyze(&buf, Path::new("/tmp/ca.fsm"));
        assert!(
            re.diagnostics.is_empty(),
            "duplicate removed → E0022 gone, no new diagnostic, got {:?}",
            re.diagnostics
        );
    }

    #[test]
    fn e0022_withheld_when_duplicate_has_stable_id_guard() {
        // The duplicate `GO` carries its own `@id(...)` — deleting only
        // the EVENT_DECL would orphan it. The fix MUST be withheld (a
        // missing fix, never a corrupting one).
        let src = "language fsm 2.0\nmachine M {\n    events { GO @id(\"e-go\") GO }\n    initial Idle\n    state Idle {\n        on GO -> Idle\n    }\n}\n";
        let (diags, cst, li, uri) = setup(src);
        // E0022 IS present...
        assert!(
            diags.iter().any(|d| d.code == DiagnosticCode::E0022),
            "fixture must still produce E0022"
        );
        // ...but NO action is offered for it (the guard declines).
        let r = code_actions(
            &diags,
            &cst,
            &uri,
            full_range(&li, src),
            &li,
            src,
            OffsetEncoding::Utf8,
        );
        assert!(
            r.is_none(),
            "stable-id-on-duplicate → NO quick-fix (bias to safety), got {r:?}"
        );
    }

    #[test]
    fn no_bogus_action_for_a_diagnostic_without_a_safe_fix() {
        // FSM-E0100 (unknown state) is scoped OUT — a file with only that
        // error must offer NO code action (never a speculative one).
        let src = "language fsm 2.0\nmachine M {\n    events { GO }\n    initial Idle\n    state Idle {\n        on GO -> Nowhere\n    }\n}\n";
        let (diags, cst, li, uri) = setup(src);
        assert!(
            diags.iter().any(|d| d.code == DiagnosticCode::E0100),
            "fixture must produce the scoped-out E0100"
        );
        let r = code_actions(
            &diags,
            &cst,
            &uri,
            full_range(&li, src),
            &li,
            src,
            OffsetEncoding::Utf8,
        );
        assert!(
            r.is_none(),
            "a scoped-out diagnostic offers NO action, got {r:?}"
        );
    }
}
