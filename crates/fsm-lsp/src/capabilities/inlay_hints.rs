//! `textDocument/inlayHint` — Doc 14 §11 / Doc 26 §5/§8 L7.
//!
//! Inlay hints are **read-only display** (no `WorkspaceEdit`, no source
//! mutation) — lower risk than `codeAction`, but they still must be
//! position-correct under both `positionEncoding`s and never invent a
//! value the analysis did not produce. Every hint here is sourced from the
//! **single** `analyze()`'s lowered `Ir` (the SAME one analysis the
//! diagnostics / symbol / hover / completion / references / semantic-token
//! path runs — no second analysis pass), and every position goes through
//! the one [`crate::position::LineIndex`] (no second converter).
//!
//! ## Which Doc 14 §11 hints ship, and why row 5 is scoped OUT
//!
//! Doc 14 §11 lists five hint rows. The brief / Doc 26 §8 L7 say "verify
//! exactly what §8 L7 calls for; don't invent hint kinds", and **Doc 26
//! §5's `inlayHint` row is the authoritative narrowing**: source =
//! "`Ir` (**non-default priority, timer durations, state child counts**)
//! + Doc 22 §8 toggles". Those are exactly three of Doc 14 §11's rows, and
//! Doc 22 §8 provides exactly three matching per-category toggles:
//!
//! | Doc 14 §11 row | Doc 26 §5 IR source | Doc 22 §8 toggle | Verdict |
//! |---|---|---|---|
//! | non-default priority on transition (`// priority: 50`) | `TransitionObject.priority` | `inlayHints.showTransitionPriorities` | **SHIP** |
//! | composite/parallel substate count (`// 3 substates`) | `Composite`/`Parallel` region `states` | `inlayHints.showStateTypes` | **SHIP** |
//! | timer duration > 1000ms (`// 1.5 s`) | `TimerObject.duration_ms` | `inlayHints.showTimerDurations` | **SHIP** |
//! | timer duration > 60000ms (`// 1 min 30 s`) | `TimerObject.duration_ms` | `inlayHints.showTimerDurations` | **SHIP** (same family, the > 60000ms format) |
//! | extern with inferrable param names (`// (ctx, payload)` after a guard call) | — | **none in Doc 22 §8** | **SCOPE OUT.** Doc 26 §5's enumerated IR-sourced inlay set does NOT include it, and Doc 22 §8 has NO toggle for it (only the master + the three above). Inventing an un-toggleable hint kind contradicts Doc 26 §8 L7's "don't invent hint kinds". Flagged Doc 00 §11.38 (a Doc-14-§11-vs-Doc-26-§5/Doc-22-§8 seam precision — derive-correct to the authoritative narrowing, not a defect). |
//!
//! Net: **3 hint families** (priority, substate count, timer duration —
//! the latter covering both the > 1000ms and > 60000ms formats), each
//! gated by its Doc 22 §8 per-category toggle AND the master
//! `enableInlayHints` switch. Row 5 scoped out with a documented reason.
//!
//! ## Position correctness — content-end, not node-end
//!
//! `TransitionObject.loc.span` / `TimerObject.loc.span` are
//! `span_of(node)` and **subsume the node's trailing trivia** (verified:
//! a transition span ends `…-> Fast\n        ` — past the meaningful
//! text, on the next line's indentation). Doc 14 §11 wants the hint
//! "After `on START [...]-> Fast;`" — i.e. immediately after the last
//! *significant* token. So the priority/timer hint anchors at the **end
//! of the last non-trivia token within the `loc.span`**, found by a
//! bounded CST token scan over the already-parsed tree (the SAME
//! locate-a-token-within-an-analysis-provided-span technique L2's
//! `selectionRange` uses, Doc 00 §11.33(4) — NOT a re-parse, NOT a second
//! analysis). The substate-count hint anchors just after the
//! composite/parallel state's opening `{` (its first `LBrace` child
//! token) — Doc 14 §11 row 2 "After `composite Operational {`".

use tower_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range};

use fsm_ir::{Ir, StateNode};

use crate::config::InlayHintConfig;
use crate::position::{LineIndex, OffsetEncoding};

/// Build the inlay hints for the visible `req_range`, gated by `cfg`.
///
/// Pure projection of the single `analyze()`'s `ir` + the parsed `cst`
/// (for the content-end / `{`-anchor token lookups). When the master
/// `cfg.enabled` is `false`, NO hint is emitted regardless of the
/// per-category switches (Doc 14 §3 "Toggle all inlay hints"). Hints
/// outside `req_range` are filtered (LSP: the client passes the visible
/// viewport; returning out-of-range hints is wasteful and non-conformant).
#[allow(clippy::too_many_arguments)]
pub fn inlay_hints(
    ir: Option<&Ir>,
    cst: &fsm_parser::cst::SyntaxNode,
    cfg: InlayHintConfig,
    req_range: Range,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
) -> Vec<InlayHint> {
    // Master gate first — `enableInlayHints=false` suppresses everything.
    if !cfg.enabled {
        return Vec::new();
    }
    let Some(ir) = ir else {
        return Vec::new();
    };
    let mut out: Vec<InlayHint> = Vec::new();
    for m in &ir.machines {
        collect_region_states(&m.root.states, cst, cfg, li, text, enc, &mut out);
    }
    // Keep only hints whose position is within the requested viewport.
    out.retain(|h| pos_in_range(h.position, req_range));
    // Stable order (line, character) — deterministic output the oracle
    // test can compare exactly.
    out.sort_by(|a, b| {
        (a.position.line, a.position.character).cmp(&(b.position.line, b.position.character))
    });
    out
}

/// Recurse the IR state tree, emitting the three hint families. `states`
/// is one region's state list (the machine root is its implicit region).
#[allow(clippy::too_many_arguments)]
fn collect_region_states(
    states: &[StateNode],
    cst: &fsm_parser::cst::SyntaxNode,
    cfg: InlayHintConfig,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
    out: &mut Vec<InlayHint>,
) {
    for s in states {
        match s {
            StateNode::Simple(ss) => {
                emit_priority_and_timer(&ss.transitions, &ss.timers, cst, cfg, li, text, enc, out);
            }
            StateNode::Composite(cs) => {
                emit_priority_and_timer(&cs.transitions, &cs.timers, cst, cfg, li, text, enc, out);
                let child = count_child_states(&cs.regions);
                emit_substate_count(
                    cs.loc.span.start,
                    cs.loc.span.end,
                    child,
                    cst,
                    cfg,
                    li,
                    text,
                    enc,
                    out,
                );
                for r in &cs.regions {
                    collect_region_states(&r.states, cst, cfg, li, text, enc, out);
                }
            }
            StateNode::Parallel(ps) => {
                emit_priority_and_timer(&ps.transitions, &ps.timers, cst, cfg, li, text, enc, out);
                let child = count_child_states(&ps.regions);
                emit_substate_count(
                    ps.loc.span.start,
                    ps.loc.span.end,
                    child,
                    cst,
                    cfg,
                    li,
                    text,
                    enc,
                    out,
                );
                for r in &ps.regions {
                    collect_region_states(&r.states, cst, cfg, li, text, enc, out);
                }
            }
            // Pseudo-states (initial/final/choice/junction/history/fork/
            // join/entry/exit) and submachine refs carry none of the three
            // hinted properties — no hint, by design (not an omission).
            _ => {}
        }
    }
}

/// Doc 14 §11 row 1 (priority) + rows 3/4 (timer durations) for one
/// state's transitions + timers.
#[allow(clippy::too_many_arguments)]
fn emit_priority_and_timer(
    transitions: &[fsm_ir::TransitionObject],
    timers: &[fsm_ir::TimerObject],
    cst: &fsm_parser::cst::SyntaxNode,
    cfg: InlayHintConfig,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
    out: &mut Vec<InlayHint>,
) {
    if cfg.show_transition_priorities {
        for t in transitions {
            // Only an *explicit, user-written* non-default priority on an
            // event/completion transition. `after`/`every` timer-bound
            // transitions are lowered with priority 0 as an internal
            // artifact (NOT a `priority` clause the user typed), so they
            // are excluded — the hint must reflect what the user wrote,
            // and Doc 14 §11 row 1's example is `on START […] -> Fast`.
            let user_priority = t.priority != fsm_ir::DEFAULT_TRANSITION_PRIORITY
                && matches!(
                    t.trigger,
                    Some(fsm_ir::Trigger::Event { .. }) | Some(fsm_ir::Trigger::Completion { .. })
                );
            if user_priority {
                if let Some(anchor) = content_end(cst, t.loc.span.start, t.loc.span.end) {
                    out.push(make_hint(
                        li.position(text, anchor, enc),
                        // Doc 14 §11 row 1 exact format: `// priority: 50`.
                        format!("// priority: {}", t.priority),
                    ));
                }
            }
        }
    }
    if cfg.show_timer_durations {
        for tm in timers {
            // Doc 14 §11 rows 3/4: only durations strictly greater than
            // 1000ms get a human-readable hint (≤ 1000ms is unambiguous
            // as raw ms).
            if tm.duration_ms > 1000 {
                if let Some(anchor) = content_end(cst, tm.loc.span.start, tm.loc.span.end) {
                    out.push(make_hint(
                        li.position(text, anchor, enc),
                        format!("// {}", human_duration(tm.duration_ms)),
                    ));
                }
            }
        }
    }
}

/// Doc 14 §11 row 2: composite/parallel substate count, anchored just
/// after the state's opening `{`.
#[allow(clippy::too_many_arguments)]
fn emit_substate_count(
    state_span_start: usize,
    state_span_end: usize,
    child: usize,
    cst: &fsm_parser::cst::SyntaxNode,
    cfg: InlayHintConfig,
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
    out: &mut Vec<InlayHint>,
) {
    if !cfg.show_state_types {
        return;
    }
    if let Some(anchor) = first_lbrace_end_within(cst, state_span_start, state_span_end) {
        out.push(make_hint(
            li.position(text, anchor, enc),
            // Doc 14 §11 row 2 exact format: `// 3 substates`. Singular
            // `substate` for exactly one (a state with one child is still
            // a composite; the grammatically-correct label).
            if child == 1 {
                "// 1 substate".to_owned()
            } else {
                format!("// {child} substates")
            },
        ));
    }
}

/// Total direct child states across a composite/parallel state's regions
/// (real states only — `Simple`/`Composite`/`Parallel`; pseudo-states are
/// not "substates" in the Doc 14 §11 sense).
fn count_child_states(regions: &[fsm_ir::RegionObject]) -> usize {
    regions
        .iter()
        .map(|r| {
            r.states
                .iter()
                .filter(|s| {
                    matches!(
                        s,
                        StateNode::Simple(_) | StateNode::Composite(_) | StateNode::Parallel(_)
                    )
                })
                .count()
        })
        .sum()
}

/// Byte offset just past the **last non-trivia token** whose range lies
/// within `[start, end)`. The IR `loc.span` subsumes trailing trivia, so
/// this recovers the position immediately after the last meaningful
/// token (Doc 14 §11 "After `… -> Fast;`"). Bounded CST token scan over
/// the already-parsed tree — not a re-parse, not a second analysis (the
/// L2 `selectionRange` technique). `None` only if the span holds no
/// significant token (a broken parse) — a conservative no-hint.
fn content_end(cst: &fsm_parser::cst::SyntaxNode, start: usize, end: usize) -> Option<u32> {
    use fsm_parser::cst::SyntaxKind as K;
    let mut best: Option<u32> = None;
    for tok in cst.descendants_with_tokens().filter_map(|e| e.into_token()) {
        let r = tok.text_range();
        let (ts, te) = (usize::from(r.start()), usize::from(r.end()));
        if ts >= start
            && te <= end
            && !matches!(
                tok.kind(),
                K::Whitespace | K::Newline | K::LineComment | K::BlockComment | K::DocComment
            )
        {
            let te = te as u32;
            best = Some(best.map_or(te, |b| b.max(te)));
        }
    }
    best
}

/// Byte offset just past the **first `{`** whose range lies within
/// `[start, end)` (the composite/parallel state's opening brace). Same
/// bounded-CST-scan technique as [`content_end`]. `None` if the state's
/// span holds no `{` (a broken parse) — a conservative no-hint.
fn first_lbrace_end_within(
    cst: &fsm_parser::cst::SyntaxNode,
    start: usize,
    end: usize,
) -> Option<u32> {
    use fsm_parser::cst::SyntaxKind as K;
    // (start, end) of the earliest qualifying `{` so far.
    let mut best: Option<(u32, u32)> = None;
    for tok in cst.descendants_with_tokens().filter_map(|e| e.into_token()) {
        if tok.kind() != K::LBrace {
            continue;
        }
        let r = tok.text_range();
        let (ts, te) = (usize::from(r.start()), usize::from(r.end()));
        if ts >= start && te <= end {
            let (ts, te) = (ts as u32, te as u32);
            // First by start position → smallest start wins.
            match best {
                Some((bs, _)) if bs <= ts => {}
                _ => best = Some((ts, te)),
            }
        }
    }
    best.map(|(_, te)| te)
}

/// Doc 14 §11 human duration. `> 1000ms` and `<= 60000ms` →
/// seconds with up to one decimal, trailing `.0` dropped (`1500 → "1.5
/// s"`, `2000 → "2 s"`). `> 60000ms` → `"{min} min {sec} s"` (`90000 →
/// "1 min 30 s"`), the seconds part omitted when zero (`120000 → "2
/// min"`). Caller guarantees `ms > 1000`.
fn human_duration(ms: u32) -> String {
    if ms > 60_000 {
        let total_secs = ms / 1000;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        // Sub-second remainder (rare for >60s timers) folded into the
        // seconds with one decimal so the value is never silently lost.
        let frac_ms = ms % 1000;
        if secs == 0 && frac_ms == 0 {
            format!("{mins} min")
        } else if frac_ms == 0 {
            format!("{mins} min {secs} s")
        } else {
            let s = secs as f64 + frac_ms as f64 / 1000.0;
            format!("{mins} min {} s", trim_decimal(s))
        }
    } else {
        // 1000 < ms <= 60000.
        let s = ms as f64 / 1000.0;
        format!("{} s", trim_decimal(s))
    }
}

/// One-decimal format with a trailing `.0` removed (`1.5 → "1.5"`,
/// `2.0 → "2"`, `1.25 → "1.3"` — one-decimal rounding is the Doc 14 §11
/// display convention, sub-ms precision is not meaningful for a timer).
fn trim_decimal(v: f64) -> String {
    let r = format!("{:.1}", v);
    r.strip_suffix(".0").map(str::to_owned).unwrap_or(r)
}

/// One inlay hint at `position` with `label`, kind `Type` (the LSP kind
/// for an inferred/auxiliary annotation — these are derived facts about
/// the program, not parameter names; `Parameter` is reserved for the
/// scoped-out row-5 extern-arg hint). Read-only: no `text_edits`.
fn make_hint(position: Position, label: String) -> InlayHint {
    InlayHint {
        position,
        label: InlayHintLabel::String(label),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(true),
        padding_right: None,
        data: None,
    }
}

fn pos_in_range(p: Position, r: Range) -> bool {
    let ge_start = (p.line, p.character) >= (r.start.line, r.start.character);
    let le_end = (p.line, p.character) <= (r.end.line, r.end.character);
    ge_start && le_end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use std::path::Path;

    fn full(li: &LineIndex, s: &str) -> Range {
        li.range(
            s,
            fsm_diagnostics::Span::new(0, s.len()),
            OffsetEncoding::Utf8,
        )
    }

    #[test]
    fn human_duration_matches_doc14_s11_examples() {
        // The two Doc 14 §11 worked examples, verbatim.
        assert_eq!(human_duration(1500), "1.5 s");
        assert_eq!(human_duration(90_000), "1 min 30 s");
        // Boundary / shape checks.
        assert_eq!(human_duration(2000), "2 s");
        assert_eq!(human_duration(120_000), "2 min");
        assert_eq!(human_duration(61_000), "1 min 1 s");
    }

    #[test]
    fn priority_and_timer_hints_present_with_defaults_off_state_types() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/l7_inlay.fsm"
        ))
        .unwrap();
        let a = analyze(&src, Path::new("/tmp/i.fsm"));
        assert!(a.diagnostics.is_empty(), "fixture must be clean");
        let cst = fsm_parser::parse(&src).syntax();
        let li = LineIndex::new(&src);
        let hints = inlay_hints(
            a.ir.as_ref(),
            &cst,
            InlayHintConfig::default(),
            full(&li, &src),
            &li,
            &src,
            OffsetEncoding::Utf8,
        );
        let labels: Vec<String> = hints
            .iter()
            .map(|h| match &h.label {
                InlayHintLabel::String(s) => s.clone(),
                _ => unreachable!(),
            })
            .collect();
        // Default config: priorities ON, timers ON, state-types OFF.
        assert!(
            labels.contains(&"// priority: 50".to_owned()),
            "explicit `priority 50` → hint, got {labels:?}"
        );
        assert!(
            labels.contains(&"// 1.5 s".to_owned()),
            "1500ms timer → `// 1.5 s`, got {labels:?}"
        );
        assert!(
            labels.contains(&"// 1 min 30 s".to_owned()),
            "90000ms timer → `// 1 min 30 s`, got {labels:?}"
        );
        // showStateTypes default is FALSE → no substate hint.
        assert!(
            !labels.iter().any(|l| l.contains("substate")),
            "substate hint must be OFF by default (Doc 22 §8), got {labels:?}"
        );
        // The implicit timer-transition's priority-0 must NOT surface as a
        // priority hint (only user-written priorities).
        assert!(
            !labels.contains(&"// priority: 0".to_owned()),
            "timer-lowering priority 0 is NOT a user priority, got {labels:?}"
        );
    }

    #[test]
    fn show_state_types_toggle_reveals_substate_count() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/l7_inlay.fsm"
        ))
        .unwrap();
        let a = analyze(&src, Path::new("/tmp/i.fsm"));
        let cst = fsm_parser::parse(&src).syntax();
        let li = LineIndex::new(&src);
        let cfg = InlayHintConfig {
            show_state_types: true,
            ..InlayHintConfig::default()
        };
        let hints = inlay_hints(
            a.ir.as_ref(),
            &cst,
            cfg,
            full(&li, &src),
            &li,
            &src,
            OffsetEncoding::Utf8,
        );
        let labels: Vec<String> = hints
            .iter()
            .map(|h| match &h.label {
                InlayHintLabel::String(s) => s.clone(),
                _ => unreachable!(),
            })
            .collect();
        // `Operational` is a parallel with Work(Sub,Sub2,Sub3) +
        // Aux(AuxIdle,AuxBusy) = 5 child states.
        assert!(
            labels.contains(&"// 5 substates".to_owned()),
            "parallel `Operational` → `// 5 substates`, got {labels:?}"
        );
    }

    #[test]
    fn master_toggle_off_suppresses_everything() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/l7_inlay.fsm"
        ))
        .unwrap();
        let a = analyze(&src, Path::new("/tmp/i.fsm"));
        let cst = fsm_parser::parse(&src).syntax();
        let li = LineIndex::new(&src);
        let cfg = InlayHintConfig {
            enabled: false,
            show_state_types: true,
            ..InlayHintConfig::default()
        };
        let hints = inlay_hints(
            a.ir.as_ref(),
            &cst,
            cfg,
            full(&li, &src),
            &li,
            &src,
            OffsetEncoding::Utf8,
        );
        assert!(
            hints.is_empty(),
            "enableInlayHints=false suppresses ALL hints, got {hints:?}"
        );
    }

    #[test]
    fn priorities_toggle_off_hides_only_priorities() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/l7_inlay.fsm"
        ))
        .unwrap();
        let a = analyze(&src, Path::new("/tmp/i.fsm"));
        let cst = fsm_parser::parse(&src).syntax();
        let li = LineIndex::new(&src);
        let cfg = InlayHintConfig {
            show_transition_priorities: false,
            ..InlayHintConfig::default()
        };
        let hints = inlay_hints(
            a.ir.as_ref(),
            &cst,
            cfg,
            full(&li, &src),
            &li,
            &src,
            OffsetEncoding::Utf8,
        );
        let labels: Vec<String> = hints
            .iter()
            .map(|h| match &h.label {
                InlayHintLabel::String(s) => s.clone(),
                _ => unreachable!(),
            })
            .collect();
        assert!(
            !labels.iter().any(|l| l.starts_with("// priority")),
            "showTransitionPriorities=false hides priority hints, got {labels:?}"
        );
        // Timer hints survive (independent toggle).
        assert!(
            labels.contains(&"// 1.5 s".to_owned()),
            "timer hints unaffected by the priority toggle, got {labels:?}"
        );
    }
}
