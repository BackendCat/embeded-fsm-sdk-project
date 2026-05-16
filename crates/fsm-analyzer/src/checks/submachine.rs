//! Submachine reference checks — Doc 04 §15, Doc 08 §12, Doc 10 §9.
//!
//! W2a landed the real grammar: a top-level `submachine Name { … }` is a
//! `SUBMACHINE_DECL` CST node and `state X is Sub { … }` carries an optional
//! `SUBMACHINE_REF` child on `STATE_DECL`. This pass operates on those real
//! nodes (the previous best-effort `KwSubmachine` token-scan over
//! `file.machines()` is gone — submachine *templates* live in the disjoint
//! `file.submachines()` view, so the old scan could never have seen them).
//!
//! Diagnostics emitted (exact Doc 10 definitions):
//!  - **FSM-E0610** "construct used without required `feature` flag" — any
//!    submachine syntax (`SUBMACHINE_DECL` or `SUBMACHINE_REF`) present while
//!    `feature submachines` is not declared at file scope (Doc 04 §2.2).
//!  - **FSM-E0103** "unknown machine reference" — `state X is Name` where no
//!    `submachine Name` is declared. (Submachines are a disjoint namespace
//!    per Doc 04 §15; a plain `machine` is not a valid `is` target.)
//!  - **FSM-E0500** "submachine entry point not declared" — a referenced
//!    submachine has neither an `initial` nor an `entry_point`, so there is
//!    no entry point to start it at (Doc 08 §12.2: entry is a named entry
//!    point *or* the submachine's initial state).
//!  - **FSM-E0501** "submachine exit point not declared" — the referencing
//!    state declares a `done ->` completion (it expects the sub-instance to
//!    complete) but the submachine has no `final` state / `exit_point`, so
//!    nothing can drive that completion (Doc 08 §12.3).
//!  - **FSM-E0502** "submachine instantiation cycle detected" — submachine A
//!    references B and B (transitively) references A (Doc 10 §9;
//!    recoverable: No). **Also reused (with an overriding message) to reject
//!    a submachine reference nested inside a composite/parallel state** —
//!    phase-audit P1-2. Rationale: Doc 10 §9 reserves no code for the
//!    positional constraint and adding a new `DiagnosticCode` variant is out
//!    of this wave's scope; among the existing submachine-family codes
//!    E0502 is the only one whose semantics ("this submachine
//!    *instantiation structure* is not permitted") and recoverability
//!    (recoverable: **No** — code generation is *blocked*, so no broken C is
//!    produced) match. E0500/E0501 are recoverable: Yes (compilation
//!    continues, emitting the broken C this fix exists to prevent) and would
//!    be the wrong semantics. This mirrors the retired-E0903 / defer
//!    precedent: a not-yet-implemented construct is cleanly *rejected* with
//!    an actionable, version-scoped message rather than silently
//!    miscompiled. Nested-submachine *support* remains the tracked SUB-FU-2
//!    follow-up; this only makes the limitation honest and safe.

use std::collections::{HashMap, HashSet};

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-2 + R-4 leave-and-explain).** Retains
// `fsm_parser::cst` for: (R-2) the document-pre-order
// `file.syntax().descendants()` filtered to SUBMACHINE_DECL/SUBMACHINE_REF
// (the E0610 feature gate + the per-ref E0103/E0500/E0501 scan, which must
// also see refs nested inside submachine *templates*, not just machines) —
// the unsorted diagnostic vector makes traversal order byte-load-bearing
// for the W0 §4.2 gate, and no typed whole-subtree-preorder iterator spans
// `file.machines()` ∪ `file.submachines()`; (R-4) `state_has_completion`
// is a pure `.parent()`-walk structural predicate — the exact Archetype-C /
// R-4 shape of `util::submachine_ref_is_nested` (positional rowan API with
// no typed equivalent; inventing one is a positional-API reimplementation).
// Folding either worsens clarity / risks the byte-identity regression class
// for zero behaviour gain — Doc 00 §11.44/§11.49, the DRIFT-2 `LineIndex`
// precedent. Left-and-explained.
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::{span_of, submachine_ref_is_nested};

/// `feature submachines` declared at file scope (Doc 04 §2.2). Feature flags
/// are file-level only — an in-`machine`-body `feature` is a parse error
/// (W2a), so the file's `features()` view is authoritative.
fn submachines_feature_enabled(file: &ast::File) -> bool {
    file.features()
        .any(|f| f.name().as_deref() == Some("submachines"))
}

pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // -- FSM-E0610: feature gate ------------------------------------------
    //
    // Any submachine syntax (the top-level template OR a `state X is Sub`
    // reference) requires `feature submachines`. Emitted per offending node
    // so the user sees every site; structural checks below still run
    // (cumulative diagnostics, Doc 09 §1) — the feature gate and the
    // structural shape are independent failures.
    if !submachines_feature_enabled(file) {
        for node in file.syntax().descendants() {
            if matches!(
                node.kind(),
                SyntaxKind::SUBMACHINE_DECL | SyntaxKind::SUBMACHINE_REF
            ) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0610, span_of(&node)).with_message(
                        "feature `submachines` is not enabled (add `feature submachines` \
                         at file scope)"
                            .to_string(),
                    ),
                );
            }
        }
    }

    // -- Per-reference checks: FSM-E0103 / FSM-E0500 / FSM-E0501 -----------
    //
    // Walk every `SUBMACHINE_REF` (under a machine OR nested inside another
    // submachine template). For each: resolve the name; if unknown →
    // FSM-E0103; if known but the template has no entry point → FSM-E0500;
    // if known, the referencing state has a `done ->` completion, and the
    // template has no exit point → FSM-E0501.
    for sref_node in file
        .syntax()
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::SUBMACHINE_REF)
    {
        let Some(sref) = ast::SubmachineRef::cast(sref_node.clone()) else {
            continue;
        };
        let Some(ref_name) = sref.name() else {
            // Malformed `state X is { … }` — the parser already diagnosed
            // the missing name (W2a recovery); nothing to resolve here.
            continue;
        };

        // -- P1-2: reject a submachine ref nested in a composite/parallel --
        //
        // A top-level `state X is Sub` is fully supported (W2a–W2d). A ref
        // that is a *descendant* of a composite/parallel state is NOT yet
        // implemented and currently makes codegen emit prototype-less
        // entry_/exit_ calls → C that fails the project's own mandated
        // `gcc -Werror`. We reject it at analysis (error-severity, code
        // generation blocked) so codegen never receives it — clean
        // rejection beats broken output (the retired-E0903 / defer
        // precedent). Emitted independently of name resolution and the
        // entry/exit/cycle checks below (cumulative diagnostics, Doc 09 §1);
        // the lowerer additionally refuses to lower a nested ref to
        // `StateNode::Submachine` (defence-in-depth — see
        // `lower::state::lower_state`).
        if submachine_ref_is_nested(&sref_node) {
            out.push(
                Diagnostic::new(DiagnosticCode::E0502, span_of(&sref_node)).with_message(format!(
                    "submachine reference `is {ref_name}` is only supported on a top-level \
                     state in v1.1; nesting it inside a composite or parallel state is not \
                     yet implemented (tracked SUB-FU-2) — move the `state … is {ref_name}` \
                     to the machine's top level"
                )),
            );
            // Skip the entry/exit-point checks for this ref: they describe
            // the *template*, which is irrelevant while the *position* is
            // rejected, and would be noise on an already-rejected site.
            // E0502's recoverable:No nature means one decisive marker per
            // culprit is the house style (see `check_instantiation_cycles`).
            continue;
        }

        let Some(target) = st.resolve_submachine(&ref_name) else {
            out.push(
                Diagnostic::new(DiagnosticCode::E0103, span_of(&sref_node)).with_message(format!(
                    "unknown submachine '{ref_name}' (no `submachine {ref_name}` is declared)"
                )),
            );
            continue;
        };

        if !target.has_entry_point {
            out.push(
                Diagnostic::new(DiagnosticCode::E0500, span_of(&sref_node)).with_message(format!(
                    "submachine '{ref_name}' declares no entry point — it has no `initial` \
                     state and no `entry_point` (Doc 08 §12.2)"
                )),
            );
        }

        // FSM-E0501 is conditional on the *referencing* state expecting the
        // sub-instance to complete, i.e. it declares a `done ->` completion
        // edge. Without that edge the absence of an exit point is benign
        // (the parent exits the ref via its own external transition,
        // Doc 08 §12.3 case 2).
        if state_has_completion(&sref_node) && !target.has_exit_point {
            out.push(
                Diagnostic::new(DiagnosticCode::E0501, span_of(&sref_node)).with_message(format!(
                    "submachine '{ref_name}' declares no exit point — `done ->` cannot fire \
                     because it has no `final` state and no `exit_point` (Doc 08 §12.3)"
                )),
            );
        }
    }

    // -- FSM-E0502: submachine instantiation cycle ------------------------
    check_instantiation_cycles(file, st, out);
}

/// Does the `STATE_DECL` enclosing this `SUBMACHINE_REF` declare a `done ->`
/// completion edge? The completion is a `COMPLETION_DECL` sibling of the
/// `SUBMACHINE_REF` (both direct children of the `STATE_DECL`, per W2a).
fn state_has_completion(sref_node: &fsm_parser::cst::SyntaxNode) -> bool {
    let Some(state_decl) = sref_node.parent() else {
        return false;
    };
    if state_decl.kind() != SyntaxKind::STATE_DECL {
        return false;
    }
    state_decl
        .children()
        .any(|c| c.kind() == SyntaxKind::COMPLETION_DECL)
}

/// Detect cycles in the submachine instantiation graph (FSM-E0502). Nodes
/// are submachine-template names; an edge `A -> B` exists when template `A`
/// contains a `state Y is B`. A machine that references a submachine is a
/// graph *root* (machines are never `is`-referenced), so only template→
/// template edges can close a cycle.
///
/// We DFS from every template with a colour map (white/grey/black). A grey
/// node reachable again is a back-edge ⇒ cycle; we report FSM-E0502 once per
/// template that participates, at the template's declaration span (the
/// recoverable:No nature means one clear marker per culprit is enough — no
/// per-edge spam).
fn check_instantiation_cycles(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // adjacency: template name -> referenced template names (already
    // collected by the symbol table while it walked each template's CST).
    let adjacency: HashMap<&str, &Vec<String>> = st
        .submachines
        .iter()
        .map(|s| (s.name.as_str(), &s.references))
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum Colour {
        White,
        Grey,
        Black,
    }
    let mut colour: HashMap<&str, Colour> = adjacency.keys().map(|&k| (k, Colour::White)).collect();
    let mut in_cycle: HashSet<String> = HashSet::new();

    // Iterative DFS (templates can self-reference and the graph is tiny,
    // but recursion-free keeps it robust against deep chains).
    for &start in adjacency.keys() {
        if colour.get(start) != Some(&Colour::White) {
            continue;
        }
        // Stack frames: (node, enter?) — enter pushes children & greys,
        // the matching exit blackens.
        let mut stack: Vec<(&str, bool)> = vec![(start, true)];
        let mut path: Vec<&str> = Vec::new();
        while let Some((node, entering)) = stack.pop() {
            if entering {
                colour.insert(node, Colour::Grey);
                path.push(node);
                stack.push((node, false));
                if let Some(refs) = adjacency.get(node) {
                    for r in refs.iter() {
                        match colour.get(r.as_str()) {
                            Some(Colour::Grey) => {
                                // Back-edge → every node on the current grey
                                // path from `r` to `node` is in a cycle.
                                let from = path.iter().position(|p| *p == r.as_str()).unwrap_or(0);
                                for c in &path[from..] {
                                    in_cycle.insert((*c).to_string());
                                }
                                in_cycle.insert(r.clone());
                            }
                            Some(Colour::White) | None => {
                                if adjacency.contains_key(r.as_str()) {
                                    stack.push((
                                        adjacency.get_key_value(r.as_str()).unwrap().0,
                                        true,
                                    ));
                                }
                            }
                            Some(Colour::Black) => {}
                        }
                    }
                }
            } else {
                colour.insert(node, Colour::Black);
                if let Some(p) = path.iter().rposition(|p| *p == node) {
                    path.remove(p);
                }
            }
        }
    }

    if in_cycle.is_empty() {
        return;
    }
    // Report at each culpable template's declaration span.
    for sub in file.submachines() {
        if let Some(name) = sub.name() {
            if in_cycle.contains(&name) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0502, span_of(sub.syntax())).with_message(
                        format!(
                            "submachine '{name}' participates in an instantiation cycle \
                             (submachines may not transitively reference themselves)"
                        ),
                    ),
                );
            }
        }
    }
}
