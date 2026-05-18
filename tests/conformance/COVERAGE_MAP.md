# Conformance Coverage Map — Diagnostic Codes (GENERATED)

DO NOT EDIT BY HAND. This file is a **generated artifact** derived byte-for-byte from
the **compiled live `fsm_diagnostics::DiagnosticCode` enum** (`all_codes()`) ×
`tests/conformance/MANIFEST.json`, by the G7 lock test
`crates/fsm-cli/tests/conformance_code_coverage_lock.rs`. The lock test is the
generator's source of truth; `coverage_map_md_is_byte_derivable_from_the_live_enum_and_manifest`
fails `cargo test` if this file drifts. Regenerate with:

    cargo test -p fsm-cli --features regen-coverage-map \
        --test conformance_code_coverage_lock -- --ignored regenerate_coverage_map

Evidence for MVP gate **G7** (Doc 00 §5.1 / Doc 32 §W3): every live diagnostic
code is EITHER covered by a conformance fixture that triggers **exactly** that
code (the exact-set discipline — verified behaviourally by running the fixture
through the same `parse + analyze_with_source` the conformance harness uses)
OR enumerated in the `NON_FIXTURE_CODES` allowlist with a real test-path
pointer (genuinely not isolatable as a conformance fixture: verify-only,
lexer-recovery-coupled, co-emitted-by-design, or catalog-only). There is NO
hand-maintained Tested/UNTESTED split — the old drift surface (GT-9) is gone.

The live enum is **73** codes (the `all_codes_matches_expected_count` lock in
`fsm-diagnostics`). It was 75 in v1.0.0; `FSM-E0903` was retired in v1.1 and
`FSM-W0500` in v1.2-FU-DEAD-CODES (each −1). Retired codes are not live
variants and are not rows here.

## Summary

| Status | Count |
| --- | --- |
| Exact-set conformance fixture | 37 |
| Non-fixture (allowlisted, with test pointer) | 36 |
| **Total live codes** | **73** |

## Per-code map

| Code | Severity | Coverage | Evidence |
| --- | --- | --- | --- |
| FSM-E0001 | Error | non-fixture (allowlisted) | Lexer emits TokenKind::Error(E0001); parser recovery always also emits E0010 — no conformance fixture can isolate it. Exact-set tested at the token layer. — see `crates/fsm-lexer/src/lexer.rs` |
| FSM-E0002 | Error | non-fixture (allowlisted) | Unterminated string: lexer TokenKind::Error(E0002) + unavoidable parser-recovery E0010; isolated only at the token layer. — see `crates/fsm-lexer/src/lexer.rs` |
| FSM-E0003 | Error | non-fixture (allowlisted) | Unterminated block comment: lexer TokenKind::Error(E0003) + unavoidable parser-recovery E0010; isolated only at the token layer. — see `crates/fsm-lexer/src/lexer.rs` |
| FSM-E0004 | Error | non-fixture (allowlisted) | Invalid integer literal: lexer TokenKind::Error(E0004) + unavoidable parser-recovery E0010; isolated only at the token layer. — see `crates/fsm-lexer/src/lexer.rs` |
| FSM-E0005 | Error | non-fixture (allowlisted) | Catalog-only: no emitter in crates/*/src (float-in-int-context check never built; reconciler-reserved). Whole-enum catalog test asserts its severity/message/round-trip. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0006 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (read-only-payload-write currently flows through E0204; the dedicated check was never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0010 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `PARSE-NEG-004` triggers exactly `FSM-E0010` |
| FSM-E0011 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (EOF-inside-construct surfaces as E0010, not a dedicated E0011). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0012 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (invalid-`as`-cast check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0020 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-005` triggers exactly `FSM-E0020` |
| FSM-E0021 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-006` triggers exactly `FSM-E0021` |
| FSM-E0022 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-007` triggers exactly `FSM-E0022` |
| FSM-E0023 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-008` triggers exactly `FSM-E0023` |
| FSM-E0024 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-009` triggers exactly `FSM-E0024` |
| FSM-E0025 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (duplicate-stable-id check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0100 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-010` triggers exactly `FSM-E0100` |
| FSM-E0101 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-001` triggers exactly `FSM-E0101` |
| FSM-E0102 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-011` triggers exactly `FSM-E0102` |
| FSM-E0103 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-012` triggers exactly `FSM-E0103` |
| FSM-E0104 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `PARSE-NEG-003` triggers exactly `FSM-E0104` |
| FSM-E0105 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (cross-machine-state-reference check never built; single-machine-scope resolution makes it unreachable). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0106 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-013` triggers exactly `FSM-E0106` |
| FSM-E0107 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `PARSE-NEG-001` triggers exactly `FSM-E0107` |
| FSM-E0108 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `PARSE-NEG-002` triggers exactly `FSM-E0108` |
| FSM-E0109 | Error | non-fixture (allowlisted) | History-default-references-unknown-state is always co-emitted with E0100 (name resolution also flags the phantom) — cannot be isolated through the analyze path. Canonical emission test in the analyzer negative suite. — see `crates/fsm-analyzer/tests/negative.rs` |
| FSM-E0110 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (local-transition-target-not-descendant reconciler addition; check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0111 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-003` triggers exactly `FSM-E0111` |
| FSM-E0200 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-014` triggers exactly `FSM-E0200` |
| FSM-E0201 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-001` triggers exactly `FSM-E0201` |
| FSM-E0202 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (extern-arg-type-mismatch check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0203 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (extern-arity-mismatch check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0204 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-004` triggers exactly `FSM-E0204` |
| FSM-E0205 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-015` triggers exactly `FSM-E0205` |
| FSM-E0206 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-005` triggers exactly `FSM-E0206` |
| FSM-E0207 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (const-divide-by-zero check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0208 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-006` triggers exactly `FSM-E0208` |
| FSM-E0210 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (opaque-field-in-guard reconciler addition; check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0300 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-002` triggers exactly `FSM-E0300` |
| FSM-E0302 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (fork-target-not-region-initial check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0303 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (join-source-not-in-region check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0310 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-016` triggers exactly `FSM-E0310` |
| FSM-E0400 | Error | non-fixture (allowlisted) | Unreachable-state is emitted by `fsm verify` (fsm-verify), NOT by analyze_with_source — the conformance run_neg path never runs verify. Verify-only by construction. `island_emits_e0400_and_w0602_when_exhaustive` asserts `d.code == DiagnosticCode::E0400` behaviourally. — see `crates/fsm-verify/src/diagnostics.rs` |
| FSM-E0401 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-017` triggers exactly `FSM-E0401` |
| FSM-E0410 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-002` triggers exactly `FSM-E0410` |
| FSM-E0500 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-018` triggers exactly `FSM-E0500` |
| FSM-E0501 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-019` triggers exactly `FSM-E0501` |
| FSM-E0502 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-020` triggers exactly `FSM-E0502` |
| FSM-E0600 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-003` triggers exactly `FSM-E0600` |
| FSM-E0610 | Error | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-021` triggers exactly `FSM-E0610` |
| FSM-E0750 | Error | non-fixture (allowlisted) | Catalog-only: no emitter (fork-target-not-parallel-region check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-E0900 | Error | non-fixture (allowlisted) | Catalog-only: a runtime-safety code (completion-chain-too-deep) with no static analyzer check — a runtime concern, not a conformance-fixture target. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-W0100 | Warning | non-fixture (allowlisted) | Catalog-only: no emitter (history-no-stored-no-default warning never built; every W0100 reference is test-fixture data). Its COVERAGE_MAP row already reads UNTESTED. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-W0101 | Warning | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-022` triggers exactly `FSM-W0101` |
| FSM-W0200 | Warning | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-004` triggers exactly `FSM-W0200` |
| FSM-W0201 | Warning | non-fixture (allowlisted) | Catalog-only: no emitter (action-block-complexity warning never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-W0300 | Warning | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-023` triggers exactly `FSM-W0300` |
| FSM-W0401 | Warning | non-fixture (allowlisted) | Catalog-only: no emitter (large-timer-duration warning never built; W0601 `>24h` is the implemented sibling). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-W0501 | Warning | non-fixture (allowlisted) | Catalog-only: no emitter (unused-event warning never built; the whole unused-declaration family, incl. retired W0500, was never implemented). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-W0600 | Warning | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-024` triggers exactly `FSM-W0600` |
| FSM-W0601 | Warning | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-007` triggers exactly `FSM-W0601` |
| FSM-W0602 | Warning | non-fixture (allowlisted) | No-incoming-transitions warning is the structural subset of E0400, emitted by `fsm verify` (fsm-verify), not the analyze path. Verify-only by construction. `island_emits_e0400_and_w0602_when_exhaustive` asserts `d.code == DiagnosticCode::W0602` behaviourally. — see `crates/fsm-verify/src/diagnostics.rs` |
| FSM-W0603 | Warning | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `VAL-NEG-008` triggers exactly `FSM-W0603` |
| FSM-W0604 | Warning | non-fixture (allowlisted) | Catalog-only: no emitter (float-cast-precision-loss reconciler addition; check never built). — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-I0001 | Info | non-fixture (allowlisted) | Catalog-only / non-negative: an Info success message ("compilation successful"), not an error/warning a negative fixture asserts. No emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-I0002 | Info | non-fixture (allowlisted) | Catalog-only / non-negative: an Info success message ("code generation complete"). No emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-I0003 | Info | non-fixture (allowlisted) | Catalog-only / non-negative: "simulator ready" Info, deferred (simulator daemon, v1.1). No emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-I0004 | Info | non-fixture (allowlisted) | Catalog-only / non-negative: forward-compat IR-schema Info. No emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-H0001 | Hint | non-fixture (allowlisted) | Catalog-only: editor-only hint (no entry action); no emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-H0002 | Hint | non-fixture (allowlisted) | Catalog-only: editor-only hint (no exit action); no emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-H0003 | Hint | non-fixture (allowlisted) | Catalog-only: editor-only hint (unconditional transition); no emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-H0004 | Hint | exact-set fixture | `tests/conformance/MANIFEST.json` fixture `SEM-NEG-025` triggers exactly `FSM-H0004` |
| FSM-H0005 | Hint | non-fixture (allowlisted) | Catalog-only: editor-only hint (prefer `every`); no emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |
| FSM-H0006 | Hint | non-fixture (allowlisted) | Catalog-only: editor-only hint (`@id` before doc comment); no emitter in crates/*/src. — see `crates/fsm-diagnostics/src/lib.rs` |

## How the gap is closed

Each `non-fixture (allowlisted)` row is the explicit, code-by-code deferred
tail (Doc 32 §6 R-fallback / §W3 (3)): closing it means authoring an emitter
(for catalog-only codes) and/or an isolating conformance fixture, then moving
the code out of `NON_FIXTURE_CODES`. The G7 lock guarantees the residual is
explicit and tracked — it can never become a silent gap, and a new live code
with neither a fixture nor an allowlist entry fails the build immediately.
