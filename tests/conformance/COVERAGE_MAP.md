# Conformance Coverage Map — Diagnostic Codes

Evidence for MVP gate **G7** (Doc 00 §5.1: "≥ 1 test per diagnostic code").

The table below lists every **live** variant of
`fsm_diagnostics::DiagnosticCode` and the test location(s) that exercise
it. A row marked `UNTESTED` is a Phase 2.3 follow-up: the catalog entry
exists in `crates/fsm-diagnostics/src/lib.rs` but no emitter test asserts
the code. G7 closes when this column is empty.

The live enum is **73** codes (the `all_codes_matches_expected_count`
lock test in `fsm-diagnostics`). It was 75 in v1.0.0; `FSM-E0903` was
retired to `deprecated::DeprecatedCode` in v1.1 (defer runtime shipped)
and `FSM-W0500` in v1.2-FU-DEAD-CODES (vestigial, never emitted), each
−1. Retired codes are not live variants and so are not rows here; their
numbers are permanent and they still parse in suppression / `allow`
contexts (Doc 10 §14). _Note: the `FSM-E0903` row below was a pre-existing
v1.1 drift (retired but the row was never removed); **reconciled in the
v1.2 batched doc-honesty pass** — it is now a struck
`~~FSM-E0903~~` RETIRED marker exactly like `~~FSM-W0500~~`, so the
table no longer carries a live-looking row for a retired code._

## Summary

Row counts over the table below (75 rows total): the table carries **two**
struck RETIRED markers — `~~FSM-W0500~~` and `~~FSM-E0903~~` — which are
**not** live variants, so **live enum = 75 − W0500 − E0903 = 73**
(matches the `all_codes_matches_expected_count` lock test in
`fsm-diagnostics`). G7 "≥1 test per code" is judged over live codes only;
the W0200 IMPLEMENT moved it Tested→ and added formal conformance
coverage. (The E0903 RETIRED marker keeps a note that the now-deprecated
code still parses in suppression / `allow` — Doc 10 §14 — but it is no
longer counted as a tested live code.)

| Status      | Count |
| ----------- | ----- |
| Tested (rows with a test path) | 39 |
| **UNTESTED** | 34 |
| RETIRED markers (`~~FSM-W0500~~`, `~~FSM-E0903~~` — not live codes) | 2 |
| **Total rows** | 75 |

## Per-code map

| Code | Severity | Tested in |
|---|---|---|
| FSM-E0001 | Error | `crates/fsm-lexer/src/lexer.rs::utf8_*` (cyrillic / 4-byte char), `tests/conformance/parser/pos/` (smoke) |
| FSM-E0002 | Error | `crates/fsm-lexer/src/lexer.rs::unterminated_string_yields_e0002`, `::string_with_embedded_newline_is_unterminated` |
| FSM-E0003 | Error | `crates/fsm-lexer/src/lexer.rs::unterminated_block_comment_yields_e0003` |
| FSM-E0004 | Error | `crates/fsm-lexer/src/lexer.rs::hex_leading_underscore_after_prefix_is_invalid`, `::binary_leading_underscore_after_prefix_is_invalid`, `::hex_with_no_digits_is_invalid` |
| FSM-E0005 | Error | **UNTESTED** — float-in-integer-context emission path missing test |
| FSM-E0006 | Error | **UNTESTED** — payload-write rejection currently flows through E0204 path |
| FSM-E0010 | Error | `crates/fsm-parser/tests/grammar.rs::missing_arrow_emits_e0010`, `::import_path_traversal_rejected`, `::opaque_type_injection_rejected` |
| FSM-E0011 | Error | **UNTESTED** — EOF-inside-construct emission path missing test |
| FSM-E0012 | Error | **UNTESTED** — invalid-cast rejection path missing test |
| FSM-E0020 | Error | `crates/fsm-analyzer/tests/negative.rs::e0020_duplicate_machine` |
| FSM-E0021 | Error | `crates/fsm-analyzer/tests/negative.rs::e0021_duplicate_state` |
| FSM-E0022 | Error | `crates/fsm-analyzer/tests/negative.rs::e0022_duplicate_event` |
| FSM-E0023 | Error | `crates/fsm-analyzer/tests/negative.rs::e0023_duplicate_context_field`, `crates/fsm-analyzer/tests/symbol_table.rs` |
| FSM-E0024 | Error | `crates/fsm-analyzer/tests/negative.rs::e0024_duplicate_extern` |
| FSM-E0025 | Error | **UNTESTED** — stable-id duplicate emission missing test |
| FSM-E0100 | Error | `crates/fsm-analyzer/tests/negative.rs::e0100_unknown_state_in_transition_target`, `::e0100_unknown_state_in_initial` |
| FSM-E0101 | Error | `crates/fsm-analyzer/tests/negative.rs::e0101_unknown_event_in_trigger`, `tests/conformance/semantic/neg/001_undeclared_event/` |
| FSM-E0102 | Error | `crates/fsm-analyzer/tests/negative.rs::e0102_unknown_extern_in_call` |
| FSM-E0103 | Error | `crates/fsm-analyzer/tests/negative.rs::e0103_unknown_machine_in_send` |
| FSM-E0104 | Error | `crates/fsm-analyzer/tests/negative.rs::e0104_unknown_context_field`, `tests/conformance/parser/neg/003_unknown_context_field/` |
| FSM-E0105 | Error | **UNTESTED** — cross-machine state-reference emission missing test |
| FSM-E0106 | Error | `crates/fsm-analyzer/tests/negative.rs::e0106_non_pure_extern_in_guard` |
| FSM-E0107 | Error | `crates/fsm-analyzer/tests/negative.rs::e0107_missing_initial_declaration`, `tests/conformance/parser/neg/001_no_initial/` |
| FSM-E0108 | Error | `crates/fsm-analyzer/tests/negative.rs::e0108_multiple_initial_declarations`, `tests/conformance/parser/neg/002_multiple_initials/` |
| FSM-E0109 | Error | `crates/fsm-analyzer/tests/negative.rs::e0109_history_default_unknown_state` |
| FSM-E0110 | Error | **UNTESTED** — local-transition target-not-descendant emission missing test |
| FSM-E0111 | Error | `crates/fsm-analyzer/tests/negative.rs::e0111_history_missing_default`, `::e0111_deep_history_missing_default`, `tests/conformance/validator/neg/003_history_no_default/` |
| FSM-E0200 | Error | `crates/fsm-analyzer/tests/negative.rs::e0200_guard_numeric_literal` |
| FSM-E0201 | Error | `crates/fsm-analyzer/tests/negative.rs::e0201_assign_negative_to_unsigned`, `tests/conformance/validator/neg/001_assignment_type_mismatch/` |
| FSM-E0202 | Error | **UNTESTED** — extern-call argument-type mismatch missing test |
| FSM-E0203 | Error | **UNTESTED** — extern-call arity mismatch missing test |
| FSM-E0204 | Error | `crates/fsm-analyzer/tests/negative.rs::e0204_assign_to_payload_field` |
| FSM-E0205 | Error | `crates/fsm-analyzer/tests/negative.rs::e0205_invalid_assignment_lhs` |
| FSM-E0206 | Error | `crates/fsm-analyzer/src/checks/type_check.rs` (unit test inside checker) |
| FSM-E0207 | Error | **UNTESTED** — divide-by-zero const-expression emission missing test |
| FSM-E0208 | Error | `crates/fsm-analyzer/tests/negative.rs::e0208_negative_default_in_unsigned` |
| FSM-E0210 | Error | **UNTESTED** — opaque-field guard rejection missing test |
| FSM-E0300 | Error | `crates/fsm-analyzer/tests/negative.rs::e0300_multiple_unguarded_transitions_same_event`, `::e0300_overlapping_field_guards`, `tests/conformance/semantic/neg/002_nondeterministic_transitions/`, `crates/fsm-analyzer/tests/determinism.rs::overlapping_interval_guards_emit_e0300` |
| FSM-E0302 | Error | **UNTESTED** — fork-target not initial state of region |
| FSM-E0303 | Error | **UNTESTED** — join-source not in parallel region |
| FSM-E0310 | Error | `crates/fsm-analyzer/tests/negative.rs::e0310_defer_conflicts_with_transition` |
| FSM-E0400 | Error | **UNTESTED** — unreachable-state emission missing test |
| FSM-E0401 | Error | `crates/fsm-analyzer/tests/negative.rs::e0401_external_self_on_composite_state` |
| FSM-E0410 | Error | `crates/fsm-analyzer/tests/negative.rs::e0410_timer_zero_ms`, `::e0410_timer_negative_ms`, `tests/conformance/validator/neg/002_after_zero_ms/` |
| FSM-E0500 | Error | **UNTESTED** — submachine entry-point missing (submachines deferred per Doc 00) |
| FSM-E0501 | Error | **UNTESTED** — submachine exit-point missing (submachines deferred per Doc 00) |
| FSM-E0502 | Error | `crates/fsm-analyzer/src/checks/submachine.rs` (unit test inside checker) |
| FSM-E0600 | Error | `crates/fsm-analyzer/tests/negative.rs::e0600_region_missing_initial`, `tests/conformance/semantic/neg/003_parallel_region_no_initial/` |
| FSM-E0610 | Error | **UNTESTED** — feature-flag missing emission missing test |
| FSM-E0750 | Error | **UNTESTED** — fork-target not parallel region missing test |
| FSM-E0900 | Error | **UNTESTED** — completion-event chain too deep (runtime safety; needs runtime test) |
| ~~FSM-E0903~~ | — | **RETIRED** v1.1 (the `defer EVENT` runtime shipped, removing the v1.0 "defer not supported" hard-fail) → `deprecated::DeprecatedCode::E0903`. Not a live variant — excluded from the G7 live count. `crates/fsm-diagnostics/src/lib.rs::tests::deprecated_codes_display_and_parse` proves it still parses in suppression / `allow` (Doc 10 §14 rule 2). _(Was a stale live-looking row through v1.1; reconciled to a struck marker in the v1.2 batched doc-honesty pass — Doc 00 §11.47.)_ |
| FSM-W0100 | Warning | **UNTESTED** — history-no-stored-no-default emission missing test |
| FSM-W0101 | Warning | `crates/fsm-analyzer/tests/negative.rs::w0101_completion_after_else` |
| FSM-W0200 | Warning | `crates/fsm-analyzer/tests/negative.rs::w0200_while_loop_in_transition_action_block`, `::w0200_for_loop_in_entry_action_block`, `::w0200_not_emitted_for_loop_free_action_block` (control), `tests/conformance/semantic/neg/004_loop_in_action/` |
| FSM-W0201 | Warning | **UNTESTED** — action-block complexity emission missing test |
| FSM-W0300 | Warning | `crates/fsm-analyzer/tests/negative.rs::w0300_priority_resolved_conflict` |
| FSM-W0401 | Warning | **UNTESTED** — large-timer-duration emission missing test |
| ~~FSM-W0500~~ | — | **RETIRED** v1.2-FU-DEAD-CODES → `deprecated::DeprecatedCode::W0500` (vestigial; never emitted; no normative spec). Not a live variant — excluded from the G7 live count. `crates/fsm-diagnostics/src/lib.rs::tests::deprecated_codes_display_and_parse` proves it still parses in suppression / `allow`; `crates/fsm-analyzer/tests/negative.rs::unused_extern_is_clean_w0500_retired_no_substitute_diagnostic` proves the would-trigger input is now clean. |
| FSM-W0501 | Warning | **UNTESTED** — unused-event emission missing test |
| FSM-W0600 | Warning | `crates/fsm-analyzer/tests/negative.rs::w0600_region_with_one_state` |
| FSM-W0601 | Warning | `crates/fsm-analyzer/tests/negative.rs::w0601_timer_exceeds_24_hours` |
| FSM-W0602 | Warning | **UNTESTED** — orphan-state warning emission missing test |
| FSM-W0603 | Warning | `crates/fsm-analyzer/tests/negative.rs::w0603_constant_guard_true` |
| FSM-W0604 | Warning | **UNTESTED** — float-cast precision loss emission missing test |
| FSM-I0001 | Info | **UNTESTED** — informational, emitted by CLI on success |
| FSM-I0002 | Info | **UNTESTED** — informational, emitted by codegen on success |
| FSM-I0003 | Info | **UNTESTED** — simulator-daemon readiness (deferred to v1.1) |
| FSM-I0004 | Info | **UNTESTED** — IR schema forward-compat emission missing test |
| FSM-H0001 | Hint | **UNTESTED** — editor-only hint, no emission test |
| FSM-H0002 | Hint | **UNTESTED** — editor-only hint, no emission test |
| FSM-H0003 | Hint | **UNTESTED** — editor-only hint, no emission test |
| FSM-H0004 | Hint | `crates/fsm-analyzer/tests/negative.rs::h0004_single_region_parallel` |
| FSM-H0005 | Hint | **UNTESTED** — editor-only hint, no emission test |
| FSM-H0006 | Hint | **UNTESTED** — annotation-order swap hint, no emission test |

## How to close the gap

Phase 2.3 follow-up: author one tiny `.fsm` per UNTESTED code that
provably triggers exactly that diagnostic, add it under either
`crates/fsm-analyzer/tests/negative.rs` (for analyzer-emitted codes) or
`tests/conformance/{parser,validator,semantic}/neg/` (for fixtures the
conformance runner will execute). The MANIFEST.json must be extended
with a matching entry. Once every code has at least one test location
the table above is updated and G7 closes.

## Read this map programmatically

The COVERAGE_MAP is markdown-only in v1.0. v1.1 plans to derive the
"Tested in" column from a small `cargo` proc-macro that grep-scrapes
`DiagnosticCode::EXXXX` references across the workspace so the row can
never drift from reality. v1.0 keeps it hand-curated to avoid pulling
in a build-time analyzer.

## Related: behavioural test-debt

This map tracks *diagnostic-code* coverage. The separate concern of
*symbol-presence vs behavioural* assertions (the P0-1-class hazard:
`.contains()` masking behaviourally-empty codegen) is tracked in
`docs/processes/TEST_DEBT.md` — opened by v1.1-W0, which paid down the
highest-risk `fsm-codegen-c` subset and catalogued the deferred remainder.
