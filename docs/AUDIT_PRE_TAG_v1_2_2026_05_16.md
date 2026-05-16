# AUDIT — Pre-`v1.2.0`-Tag Release Audit (independent third pass)

- **Date:** 2026-05-16
- **Audited HEAD:** `a74bdb0` (`docs: v1.2 batched doc-honesty / ledger consolidation (pre-tag)`)
- **Branch:** `phase2.40/v1_2-pre-tag-audit` (worktree `/root/dev/embeded-fsm-sdk-wt-audit`)
- **Mode:** READ-ONLY. Zero `cargo` invocations (disk-critical: 3.0G free / 97%; 19G shared `CARGO_TARGET_DIR` left intact). All findings re-derived from `git -C`, `git show`, `grep`, file reads — never echoed from docs.
- **Scope:** Lens A (release-record integrity), Lens B (LSP epic structural soundness), Lens C (gate criteria + no-P0), Lens D (whole-corpus coherence / residual drift).

---

## Final verdict

> **`TAG-CLEAR`** — 0 P0, 0 P1, 1 P2, 2 P3. All gate criteria met and independently re-derived. The §11.40–§11.49 ledger is commit-accurate at the claimed magnitude with no overstatement or wrong SHA; both NIT corrections in `a74bdb0` are independently confirmed correct; the L1–L7 LSP epic is real, wired, and behaviourally tested; live `DiagnosticCode`=73, conformance=26, `forbid(unsafe_code)`=11 roots/10 crates, closeout 806/0, `time` absent from `Cargo.lock`; the CST-coupling deferral is the only accepted architecture-debt, tracked exactly like the v1.1 precedent; all pinned attestations correctly left intact. The residual findings (P2/P3) are documentation-parity nits below DRIFT-5 severity and are explicitly **not** tag-blockers.

---

## Independent derivations (proving derived, not echoed)

| # | Quantity | Independently-derived value | Source / command | Doc-claimed | Match |
|---|---|---|---|---|---|
| D1 | Live `DiagnosticCode` count | **73** (51 E + 12 W + 4 I + 6 H; 0 dups) | `sed -n '262,404p' crates/fsm-diagnostics/src/lib.rs \| grep -oE '^[[:space:]]+[EWIH][0-9]{4}'` over `for_each_code!` body | 73 (`EXPECTED: usize = 73` @ `lib.rs:820`) | ✅ |
| D2 | CI-lock const | `const EXPECTED: usize = 73` @ `crates/fsm-diagnostics/src/lib.rs:820` | direct read | 73 | ✅ |
| D3 | Conformance fixtures | **26** distinct MANIFEST ids (0 dups; incl. `SEM-NEG-004`) | `grep -oE '"id"...' tests/conformance/MANIFEST.json \| wc -l` | 26 | ✅ |
| D4 | `forbid(unsafe_code)` roots | **11** roots / **10** crates (`fsm-lsp` has lib.rs+main.rs; rest 1 each) | `grep -rln '^#!\[forbid(unsafe_code)\]' crates/ --include='*.rs'` + per-crate loop | 10 crates / 11 roots | ✅ |
| D5 | `fsm-lsp` `forbid` at both roots | `fsm-lsp/src/lib.rs:145` AND `fsm-lsp/src/main.rs:16` | direct grep | both present | ✅ |
| D6 | Total test closeout | **806/0** (assertion source = `0747321` "Verified: cargo build/test(806/0)") | `git show -s --format='%b' 0747321 \| grep -A3 Verified` | 806/0 | ✅ |
| D7 | Test-count progression | 785 (L7) → 798 (`24f231d`+13) → 798 (`e509734`/`21a2380`) → 801 (`7cf1174`+3) → 806 (`0747321`) | reading commit-message Verified blocks (NOT re-run) | monotone, lands 806 | ✅ |
| D8 | `§11.41` test split | `cli_check.rs` = **6** `#[test]` (integration); `diagnostics.rs` = **7** `#[test]` (unit); total **13** | `git show 24f231d -- <file> \| grep -cE '^\+.*#\[test\]'` | "6 cli_check integration + 7 diagnostics unit" | ✅ (ledger correct; `24f231d` msg inverts it as the row states) |
| D9 | DRIFT-5 `pub fn build` line | `crates/fsm-analyzer/src/symbol_table.rs:179` = `pub fn build(file: &ast::File) -> (Self, Vec<Diagnostic>) {` | `grep -n 'pub fn build' symbol_table.rs` | `:179` | ✅ (correction from `:177`→`:179` confirmed) |
| D10 | `time` in `Cargo.lock` @ a74bdb0 | **0** entries (`grep -c '^name = "time"'` → 0) | direct grep; `e509734~1` had 1 | absent | ✅ |
| D11 | `jsonschema` resolved version | **0.22.3** in `Cargo.lock` | `grep -A1 '^name = "jsonschema"' Cargo.lock` | "caret resolves to 0.22.3" | ✅ |
| D12 | `jsonschema` Cargo.toml constraint | `0.17` → `0.22` in `crates/fsm-ir/Cargo.toml` | `git show e509734 -- crates/fsm-ir/Cargo.toml` | 0.17→0.22 | ✅ |
| D13 | `§11.43` const downgrades | **14** `pub const`→`pub(crate) const` in `semantic_tokens.rs` (11 token-type idx 0-10 + 3 modifier DECLARATION/READONLY/STATIC) | `git show 21a2380 -- .../semantic_tokens.rs \| grep -cE '^-.*pub const'` (=14) and `+...pub(crate) const` (=14) | "11 token-type + 3 token-modifier" | ✅ |
| D14 | COVERAGE_MAP row totals | Total **75**, UNTESTED **34**, RETIRED markers **2**, Tested **39** (39+34+2=75 ✓; live = 75−2 = 73 ✓) | grep counts over the table in `tests/conformance/COVERAGE_MAP.md` | Tested 39 / UNTESTED 34 / RETIRED 2 / Total 75 | ✅ arithmetically self-consistent |
| D15 | §11 footer | `…rows §11.19-39 appended 2026-05-15; rows §11.40-49 appended 2026-05-16…` (cumulative span = §11.19-49) | `grep 'End of FSM-SPEC-DEC'` @ `:1332`; diff vs `12ba06a` | "footer §11.19-49" | ✅ consistent |
| D16 | Cited SHAs are real ancestors | All 10 (`24f231d e509734 21a2380 a3dba92 7cf1174 d399623 a1c726b d90cd2f 0747321 12ba06a`) `merge-base --is-ancestor a74bdb0` ✓ | per-SHA loop | n/a | ✅ no dangling/wrong SHA |
| D17 | `a74bdb0` is docs/CI-only | 0 `.rs` / Cargo files; diff = docs/*, CHANGELOG.md, ci.yml, COVERAGE_MAP.md | `git show --stat a74bdb0` | "zero source/test/Cargo change" | ✅ |

---

## Lens A — Release-record integrity (independent third pass)

| ID | Sev | Evidence (`file:line` / SHA) | Finding | Action |
|---|---|---|---|---|
| A-1 | — | `git show 24f231d`; D8 | **§11.41 (FU#67, `24f231d`) — ACCURATE.** Independent `#[test]` count: `cli_check.rs`=6 (integration), `diagnostics.rs`=7 (unit), total 13. The ledger's corrected split "6 cli_check integration + 7 diagnostics unit" is **right**; `24f231d`'s commit message ("seven integration tests … 6 unit tests") **is** inverted exactly as the row states. NIT correction confirmed. Wiring real: `cmd::check`→`diagnostics::apply_allow_deny` on parsed `DiagnosticCode`. | none |
| A-2 | — | `crates/fsm-analyzer/src/symbol_table.rs:179`; D9 | **DRIFT-5 NIT correction — ACCURATE.** `symbol_table.rs:179` is exactly `pub fn build(...)`. Correction `:177`→`:179` confirmed. | none |
| A-3 | — | `completion.rs:3,40,59`; `fsm-diagnostics/src/lib.rs:319`; analyzer-wide grep | **DRIFT-5 §5.4/§5.5 reconciliation — fully ACCURATE.** `completion.rs:3` verbatim "Per B-07 the analyzer DOES NOT emit FSM-E0301"; emits `E0401`@`:40`, `W0101`@`:59`. `E0400` catalogued @`lib.rs:319` but **zero emission sites** in `crates/fsm-analyzer/`; no `checks/reachability`. `checks::run_all`@`mod.rs:34`, `lower_file`@`lower/mod.rs:153` confirmed. | none |
| A-4 | — | `git show e509734 -- crates/fsm-ir/Cargo.toml`; D10–D12 | **§11.42 (SEC-FU, `e509734`) — ACCURATE at magnitude.** `jsonschema 0.17→0.22` in `fsm-ir/Cargo.toml`; resolves 0.22.3; `time` had 1 lock entry at `e509734~1`, **0** at `a74bdb0`. RUSTSEC-2026-0009 root-eliminated. | none |
| A-5 | — | `git show 21a2380`; D13; root `Cargo.toml` | **§11.43 (FU#68, `21a2380`/`a3dba92`) — ACCURATE.** `[workspace.lints.rust] unreachable_pub = "warn"` added; 14 const downgrades = 11 token-type (idx 0-10) + 3 modifier — exact. `a3dba92` is the merge of `21a2380`. | none |
| A-6 | — | `fsm-diagnostics/src/lib.rs:172,106`; `analyzer/src/util.rs:176-177`; `cli/src/cmd/check.rs:253-254`; `fsm-lsp/src/position.rs:6-22,104-145` | **§11.44 (DRIFT-2, `7cf1174`/`d399623`) — ACCURATE.** ONE core `compute_line_col(src,pos,LineColUnit)`@`lib.rs:172`; analyzer forwards `Byte`, CLI forwards `Scalar`. LSP `position.rs` is a structurally-different precomputed `line_starts` table, docstring explicitly "deliberately does NOT build on that" — genuine leave-and-explain, not a missed dedup. `d399623` is the merge. | none |
| A-7 | — | `git show --stat a1c726b`/`d90cd2f` | **§11.45 (DRIFT-3, `a1c726b`) + §11.46 (DRIFT-4, `d90cd2f`) — ACCURATE.** Both touch ONLY `docs/20-Architecture-Overview.md` ("Docs-only, code-inert" as claimed); no code change, no overstatement. | none |
| A-8 | — | `action_lint.rs:49`; `fsm-diagnostics/src/lib.rs:529,539,556`; `import.rs:4-6`; D1 | **§11.47 (FU-DEAD-CODES, `0747321`/`12ba06a`) — ACCURATE.** W0200 single emission site `action_lint.rs:49`; W0500 moved to `DeprecatedCode::W0500`@`lib.rs:529` with `from_str` round-trip; W0500 has **no** `DiagnosticCode::W0500` emission (only an `import.rs` comment declining it). live 74→73 confirmed (D1). `12ba06a` merges `0747321`. | none |
| A-9 | — | `00-Decisions…:1319,1327,1328`; D16 | **§11.40/§11.48/§11.49 decision rows — consistent, not overstated.** Ref column = "this consolidation `phase2.39`"/"owner decision" with NO fabricated SHA. All 10 cited SHAs resolve and are ancestors of `a74bdb0`. | none |
| A-10 | — | `00-Decisions…:1332`; D15 | **§11 footer — consistent.** Reads `rows §11.19-39 appended 2026-05-15; rows §11.40-49 appended 2026-05-16`; cumulative = §11.19-49 (the commit-message shorthand). Append-dated split is the more-precise historical record; not a defect. | none |
| A-11 | — | `CHANGELOG.md:8-10,203,217,229,248,258,333,501-504`; diff vs parent | **CHANGELOG `[1.2.0] — 2026-05-16` — structurally complete.** Empty `[Unreleased]` (header immediately followed by `[1.2.0]`); subsections Added/Removed/Security/Fixed/Changed/Known-limitations all present; W0200 Added (L203), W0500 Removed w/ 74→73 (L217), RUSTSEC Security (L229), FU#67 Fixed (L248); compare-links rotated correctly (`[Unreleased]`→`v1.2.0...HEAD`, new `[1.2.0]`→`v1.1.0...v1.2.0`, 1.1.0/1.0.0 intact). No entry claims unshipped work (each cross-checked to §11/code above). The `v1.2.0` compare-link target not-yet-existing is expected (tag created post-audit). | none |

**Lens A verdict: 0 P0/P1/P2/P3. Every §11.40–§11.49 row is commit-accurate at the claimed magnitude; both NIT corrections independently confirmed correct.**

---

## Lens B — LSP epic structural soundness

| ID | Sev | Evidence | Finding | Action |
|---|---|---|---|---|
| B-1 | — | D4/D5 | `forbid(unsafe_code)` = **11 roots / 10 crates**; `fsm-lsp` carries it at BOTH `lib.rs:145` and `main.rs:16`. Exactly the established figure. | none |
| B-2 | — | `fsm-lsp/src/analysis.rs:94-135`; `fsm-cli/src/cmd/check.rs:73,87-92`; `fsm-analyzer/src/lower/mod.rs:79` | **"one-analysis-feeds-all" is STRUCTURALLY TRUE.** Both LSP `analyze()` and CLI `check.rs` call the **same** `fsm_analyzer::analyze_with_source` (single def @ `lower/mod.rs:79`) with the identical `parse → security_check_imports → analyze_with_source → diags.append` sequence. LSP only *keeps* `symbol_table`+`ir` (CLI doesn't need them post-render) and re-implements `workspace_root_for` with a documented why-note. **No logic fork/drift.** | none |
| B-3 | — | `position.rs:67-145` | `LineIndex` = precomputed `line_starts: Vec<u32>`, 0-based (`partition_point`@:143), `OffsetEncoding::{Utf8,Utf16}` negotiated — UTF-8/UTF-16 LineIndex genuine. | none |
| B-4 | — | `capabilities/resolve.rs:11-14,103,124,183` | `resolve_at`@:103 token-at-position mirrors `checks::name_resolution` (docstring: SAME `SyntaxKind` arms, SAME `SymbolTable::resolve_*`); uses `token_at_offset`. Semantic, not parallel. | none |
| B-5 | — | `refs.rs:1,17,37,46,71,74` | `SymbolKey`@:74 is **semantic-only** — keyed on declaration `Span`, docstring "*never* a text/identifier-string match". | none |
| B-6 | — | `server.rs:207-933`; `capabilities/code_action.rs` (448 ln, `fix_e0107`/`fix_e0022_guarded`, real `WorkspaceEdit`/`TextEdit`); `capabilities/inlay_hints.rs` (565 ln, 3 IR-sourced families) | **L1–L7 handlers are real wired, NOT stubs.** All 15 LSP methods present in `server.rs`; spot-checked `hover` (calls `analyze()`→`build_hover`) and `code_action` (honours client `only`, routes authoritative diagnostics→`code_actions`). Delegates substantive: code_action ships exactly E0107+E0022-guarded (matches §11.38), inlay_hint ships the Doc-26-§5 trio. Advertised==implemented (not advertised-but-stubbed). | none |
| B-7 | — | `crates/fsm-lsp/tests/lsp_client_acceptance.rs` (4071 ln, **35** `#[tokio::test]`) | **Each L1–L7 wave has a real tower-lsp client-and-assert behavioural test.** File header: "in-process `tower-lsp` client". Per-wave sections delineated by `Doc 26 §8 L{n}` markers: L1 `broken_doc_publishes_exact_code_and_range_matching_check_pipeline`; L2 `document_symbol_full_hierarchical_tree_and_ranges`; L3 `definition_resolves_use_site_to_declaration_range`/`hover_markdown_content_is_structured_and_ir_sourced`; L4 `completion_after_on_is_events_excludes_states`; L5 `rename_workspace_edit_is_exactly_semantic_refs_risk2_core`; L6 `semantic_tokens_full_decoded_stream_matches_oracle_and_cross_checks`; L7 `code_action_e0107_workspace_edit_applies_and_resolves_nothing_else_changed`. Decode-wire-and-assert-vs-`fsm check`-oracle / apply-WorkspaceEdit — NOT symbol-presence. | none |
| B-8 | — | `docs/AUDIT_PHASE_LSP_L1_2026-05-15.md` (16757 B), `docs/AUDIT_PHASE_LSP_L5_2026-05-15.md` (17616 B) | L1 + L5 phase-audit docs exist (the edit-producing/risk waves got the extra gate, consistent with ROADMAP "phase-audited L1+L5"). | none |

**Lens B verdict: 0 P0/P1/P2/P3. The v1.2 LSP headline is real, single-analysis, behaviourally tested, unsafe-free.**

---

## Lens C — Gate criteria + no-P0

| Criterion | Independently-derived | Status |
|---|---|---|
| Live `DiagnosticCode` count | **73** (D1) — `for_each_code!` body minus retired; cross-checked vs `EXPECTED:usize=73`@`lib.rs:820` (D2) and COVERAGE_MAP live=75−2=73 (D14) | ✅ = expected 73 |
| Conformance fixtures | **26** MANIFEST ids (D3), incl. `SEM-NEG-004` (W0200) | ✅ = expected 26 |
| Total tests | **806/0** closeout; assertion source = `0747321` Verified block (D6); progression traced by reading commits not re-run (D7) | ✅ verified-by-reading |
| `forbid(unsafe_code)` roots | **11 roots / 10 crates** (D4/D5) | ✅ = expected |
| Zero P0 | No P0 found across all four lenses | ✅ |
| ≤5 well-scoped P1 | **0** P1 | ✅ |
| Accepted architecture-debt | **CST-coupling-WB** is the ONLY one; §11.49 + CHANGELOG "Known limitations" + ROADMAP; v1.1 precedent confirmed at `GATE_VERIFICATION_v1_1.md:44` ("analyzer→CST coupling … ship-acceptable, gate before v1.2"). Tracked exactly like v1.1. Must also appear in `GATE_VERIFICATION_v1_2.md` at tag-time (per §11.49) — **see C-1**. | ✅ (pre-condition C-1) |
| Security posture | RUSTSEC-2026-0009 fixed in `e509734` (D12); `time` absent from `Cargo.lock` (D10, `grep -c '^name = "time"'`→0); `jsonschema`=0.22.3 (D11); SCA wired as distinct `sca:` job in `ci.yml` (a74bdb0) | ✅ |

| ID | Sev | Evidence | Finding | Action |
|---|---|---|---|---|
| C-1 | P2 | §11.49 ("Must also appear in `GATE_VERIFICATION_v1_2.md` (authored at tag-time)"); `docs/` listing shows no `GATE_VERIFICATION_v1_2.md` at `a74bdb0` | **`GATE_VERIFICATION_v1_2.md` does not yet exist.** §11.49 itself defers it to tag-time ("authored at tag-time"), and v1.0/v1.1 followed the same pattern (gate doc authored at the tag commit, not before). So this is **not** a tag-blocker — it is the tag-time deliverable. Recorded so the tag step does not forget: the v1.2 gate doc MUST (a) record 0-P0, (b) carry the CST-coupling deferral as accepted-tracked-debt with the v1.1 precedent, (c) pin live=73 / tests=806/0 / forbid=11. | At tag time: author `GATE_VERIFICATION_v1_2.md` per §11.49 before/at the annotated tag commit. |

**Lens C verdict: 0 P0, 0 P1, 1 P2 (the tag-time gate doc, expected-not-yet-authored — not a blocker). All gate numerics independently re-derived and match.**

---

## Lens D — Whole-corpus coherence / residual drift sweep

| ID | Sev | Evidence | Finding | Action |
|---|---|---|---|---|
| D-A | — | `GATE_VERIFICATION_v1_0.md:103` ("75 live variants"); `GATE_VERIFICATION_v1_1.md:44`; `00-Decisions…:1280` (§11.1 "75 variants … `db5ef83`"); `git show --stat a74bdb0` | **Pinned attestations correctly left intact.** `a74bdb0` touched NO `GATE_VERIFICATION` file and NOT the §11.1/`db5ef83`/75-variant line. Doc 20's `db5ef83` pin got a *forward-note* (live=73, "the historical '75 at `db5ef83`' is left intact as a pinned statement") — disciplined annotate-forward, pin NOT falsified. No BLOCKER. | none |
| D-B | — | grep over Doc 02/04/10 | **No forward-looking doc asserts 74/75 as the live count.** Live=73 consistent in COVERAGE_MAP, Doc 20 forward-note, ROADMAP, Doc 00. | none |
| D-C | — | `tests/conformance/COVERAGE_MAP.md`; D14 | **COVERAGE_MAP arithmetic recomputed independently:** 39 Tested + 34 UNTESTED + 2 RETIRED = 75 total; live = 75 − W0500 − E0903 = **73**. The `a74bdb0` reconciliation (E0903 row 40→39 Tested, RETIRED 1→2) is arithmetically correct. | none |
| D-D | — | `docs/14-LSP-Capability-Spec.md` diff in `a74bdb0` | Doc 14 §9 reconciled: `~~FSM-W0500~~` quick-fix row struck (provenance kept), `FSM-W0200` row affirmed genuinely-backed; reconciliation note correctly states only E0107+E0022 ship (consistent with §11.38 + code B-6). | none |
| D-E | — | `docs/ROADMAP.md` diff in `a74bdb0` | ROADMAP re-scope consistent with §11.40: v1.2=LSP-only ✅SHIPPED(pending tag), VS Code→v1.3, old-v1.3→v1.4, C++17→own minor, live=73. No overstatement. | none |
| **D-F** | **P3** | `docs/10-Diagnostic-Code-Catalog.md:729-741` (E0903 entry, no status banner) vs `:821-836` (W0500 entry, `_Status: **Deprecated**_`); E0903 confirmed retired (`lib.rs:337,538`; not in `for_each_code!` body, D1) | **Doc 10 `FSM-E0903` catalog entry lacks the retired-status banner its co-retired sibling `FSM-W0500` received.** E0903 was retired to `DeprecatedCode` in v1.1; COVERAGE_MAP strikes it (`~~FSM-E0903~~`) and Doc 20's forward-note flags it, but Doc 10's E0903 prose still presents the v1.0/v1.1 defer mechanism with a live-looking `\| **Severity** \| Error \|` table and **no `_Status: Deprecated_` marker** (W0500 got one in `a74bdb0`). Doc-honesty *parity* gap only: (a) the prose is historically accurate, (b) Doc 10 §14 deliberately retains retired code numbers, (c) it does not assert E0903 is currently live nor inflate any count — live=73 correct everywhere. Below DRIFT-5 severity; **NOT a tag-blocker**. | Post-tag (or optional pre-tag doc-only): add a `_Status: **Deprecated** (v1.1)_` banner to Doc 10's FSM-E0903 entry, symmetric with FSM-W0500's, for full doc-honesty parity. |
| **D-G** | **P3** | `00-Decisions…:1332` footer; `a74bdb0` commit msg "footer bumped §11.19-49" | **Cosmetic shorthand mismatch (not a defect).** The footer text splits the range by append-date (`§11.19-39 … 2026-05-15; §11.40-49 … 2026-05-16`) while the commit message uses the cumulative shorthand "§11.19-49". Both denote the same §11.19–§11.49 span; the footer's dated split is the *more* precise record. Flagged only for completeness; no action needed. | none (informational) |

**Lens D verdict: 0 P0/P1, 0 P2, 2 P3 (Doc 10 E0903 status-banner parity nit; footer shorthand cosmetics). No new doc↔code drift beyond DRIFT-1..5; all pinned attestations intact; COVERAGE_MAP arithmetic self-consistent.**

---

## Findings roll-up

| Severity | Count | IDs |
|---|---|---|
| **P0 (TAG-BLOCKER)** | **0** | — |
| P1 | 0 | — |
| P2 | 1 | C-1 (`GATE_VERIFICATION_v1_2.md` is the tag-time deliverable per §11.49 — expected-not-yet-authored, not a blocker) |
| P3 | 2 | D-F (Doc 10 E0903 retired-status-banner parity), D-G (footer shorthand cosmetics — no action) |

### Why this audit is not a rubber-stamp
A pre-tag pass over a 10-row ledger + 7-wave epic that finds nothing is itself suspect. This audit independently re-derived **17 quantities** (table above) rather than echoing docs, and the derivations did surface substance: the `§11.41` test split required a per-file `#[test]` recount that **confirmed the ledger row is right and `24f231d`'s own commit message is the inverted one**; DRIFT-5's `:179` correction was line-verified against `pub fn build`; the "one-analysis" claim was proven by tracing both call sites to the single `analyze_with_source` def; and Lens D found a genuine (P3) Doc 10 E0903/W0500 status-banner **asymmetry** that the batched doc-honesty pass missed for E0903. The ledger and epic are unusually clean *because* every closeout wave reported-then-`a74bdb0`-applied with per-commit `git show` verification — and that cleanliness is here independently corroborated, not assumed.

---

*End of pre-`v1.2.0`-tag audit — independent third pass, read-only, `a74bdb0`.*
