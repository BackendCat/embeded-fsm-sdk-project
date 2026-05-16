# GATE VERIFICATION — v1.2.0

- **Document ID:** GATE_VERIFICATION_v1_2
- **Date:** 2026-05-16
- **Scope:** v1.2.0 release gate — all commits since v1.1.0 `abc7004` through the v1.2 code corpus at `bca1411` and the release commit `X` (this document + the Doc 10 E0903 retired-banner parity fix; see §4 + §7).
- **Companion evidence (frozen, never edited):** `docs/AUDIT_PRE_TAG_v1_2_2026_05_16.md` (independent third-pass pre-tag audit); the v1.0/v1.1 baselines `docs/GATE_VERIFICATION_v1_0.md` / `docs/GATE_VERIFICATION_v1_1.md`; the v1.2 release record `docs/00-Decisions-And-Reconciliation.md` §11.40–§11.49.
- **Verdict:** **PASS — v1.2.0 is tag-ready.** 0 open P0; the independent audit returned `TAG-CLEAR` (0 P0 / 0 P1 / 1 P2 / 2 P3); the one P2 is *this very document* (the tag-time deliverable per §11.49, resolving itself by existing); the two P3s are documentation-parity nits (one fixed in this same commit, one no-action footer shorthand). The canonical confirming cold-from-source quad is the gating step the tag is contingent on (§4).

This document is the canonical release record for v1.2.0. It captures the MVP-gate status, the independent four-lens pre-tag audit verdict, the resolution/disposition of every non-blocking finding, the §11.22/§11.30-mandated **cold-from-source** quad posture, the one accepted-tracked-debt item a consumer must be aware of, and the one post-tag action that requires the repository owner.

---

## 1. MVP gate (Doc 23 §9) — v1.2.0 status

| Gate | v1.2 status | Evidence |
|---|---|---|
| **G1** `fsm check` ok / clean reject, no panic, no UB | ✅ | Unchanged from v1.1; the LSP epic *reuses* the exact `fsm check` analysis pipeline (parser + analyzer + import-security) — editor diagnostics can never disagree with `fsm check --json` (audit Lens B B-2: both LSP `analyze()` and CLI `check.rs` call the single `fsm_analyzer::analyze_with_source`, no logic fork) |
| **G2** `fsm check broken.fsm` → exit 1 + Rust-style error | ✅ | Unchanged; `FSM-W0200` ("loop in action block") now genuinely emitted (one site `fsm-analyzer` `checks/action_lint.rs`, FU-DEAD-CODES `0747321`/`12ba06a`); `FSM-W0500` retired to `DeprecatedCode` (still parses in `allow`/`deny`/suppression — Doc 10 §14 rule 2, FU#67 contract test-pinned) |
| **G3** `fsm generate` emits the C unit set + HAL | ✅ | Unchanged from v1.1 (no codegen surface in the v1.2 LSP-only scope) |
| **G4** `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` zero-warning | ✅ | Unchanged; subsumed in the cold quad's `cargo test --workspace` (the §5.4 gcc-compile-and-RUN battery: `gcc_compile.rs`, `vending_machine_gcc.rs`, `degenerate_machines_werror.rs`, `integration_examples.rs`) |
| **G5** `fsm fmt` idempotent | ✅ | Unchanged from v1.0 |
| **G6** all examples full-chain + sim-trace-match | ✅ | `fsm test examples/` **5/5** (assertion source: `0747321` commit-body Verified block — `fsm test examples/ 5/5`) |
| **G7** ≥1 test per diagnostic code | ⚠️ tracked | **26** formal `tests/conformance/` fixtures (W0200's `SEM-NEG-004` added this release, suite 25→26); the remaining live codes exercised by crate-level negative tests. Identical posture to v1.0/v1.1; **not a behavioural gap** (every new v1.2 surface is behaviourally tested — the LSP epic by 35 in-process `tower-lsp` client tests, W0200 by its conformance fixture). Formal-suite closure remains tracked. |
| **G8** `cargo build/test/clippy/fmt` clean | ✅ | **cold-from-source** quad posture below (§4): assertion source `0747321` Verified block — `cargo build/test(806/0)/clippy/fmt all green workspace-wide`; advisory cold run at `bca1411` reproduced it (806 passed / 0 failed); canonical confirming run is the gating step the tag is contingent on |
| **G9** CI matrix linux/macos/windows | ⚠️ tracked | `.github/workflows/ci.yml` well-formed and now carries an SCA `cargo audit` job (§11.42/§11.48; `ci.yml:55` `sca:`); **never exercised against any v1.2 commit** (local-only repo — owner controls the remote, a hard project rule). Post-tag action §5. |

G7 + G9 are the **same explicitly-tracked partials accepted at the v1.0 and v1.1 gates** — neither is a v1.2 regression; both are documented in CHANGELOG `[1.2.0]` *Known limitations* and the ROADMAP.

---

## 2. v1.2 surface delivered (since v1.1.0 `abc7004`)

**Theme: developer tooling — the `fsm-lsp` Language Server.** v1.2 was re-scoped (Doc 00 §11.40, user-endorsed) to **the LSP server only**: the VS Code extension moved to v1.3 (Doc 27, its natural LSP-client home), the old v1.3 "Simulation & verification" theme to v1.4, and C++17 codegen (Doc 12) to its own minor — preserving the validated small-tight-tagged cadence.

The LSP epic (L1–L7, each with a real in-process `tower-lsp` client-and-assert behavioural test — decode-the-wire-and-assert-vs-`fsm check`-oracle, never symbol-presence; Doc 00 §11.32–§11.38, CHANGELOG `[1.2.0]` Added):

- **L1** — LSP spine: `fsm-lang-server` over stdio, `initialize` (UTF-8/UTF-16 `positionEncoding` negotiation), full-document sync, 200ms debounce, `publishDiagnostics` reusing the **exact** `fsm check` pipeline (§11.32).
- **L2** — `documentSymbol` + `foldingRange`; the symbol tree threaded through the **same** single analysis, diagnostics byte-identical to L1 (§11.33).
- **L3** — `hover` + single-file `definition`; shared token-at-cursor seam through the **same** `SymbolTable::resolve_*` (§11.34).
- **L4** — context-aware `completion`; reuses L3's `resolve` substrate, candidates from the single threaded `symbol_table` (§11.35).
- **L5** — `references` + `prepareRename`/`rename`; the `ReferenceIndex` (the one genuinely-new analysis, *derived* from the single analysis), **semantic-only** — the cardinal silent-data-loss (risk-2) guard (§11.36).
- **L6** — `semanticTokens` (`full` + `range`); legend declared once, reuses L3/L5 classifiers, no parallel analyzer (§11.37).
- **L7** — `codeAction` + `inlayHint` (the final capability); edit-producing, inherits L5's risk-2 discipline — only the provably-mechanical `FSM-E0107`/`FSM-E0022` quick-fixes, the other five Doc 14 §9 codes + both `refactor.extract` deliberately scoped-out-and-flagged (§11.38).

Other v1.2 surface (each commit-traced, audit Lens A independently re-derived):

- **FU#67** (`24f231d`, §11.41) — `fsm.toml [compiler] allow/deny` was parsed but applied **nowhere** (the mild P0-1 silent-no-op class); now finalized in `cmd::check` + projected post-analysis by `diagnostics::apply_allow_deny`, matched on the parsed `DiagnosticCode` (never a substring match). +13 tests (6 `cli_check.rs` integration + 7 `diagnostics.rs` unit — the corrected split; `24f231d`'s own commit message inverts it, corrected-here / merged-history-not-rewritten per verify-the-record; audit A-1 independently confirmed the row right and the commit message the inverted one).
- **FU-DEAD-CODES** (`0747321`/`12ba06a`, §11.47) — `FSM-W0200` IMPLEMENTED (one emission site `checks/action_lint.rs`, conformance 25→26 via `SEM-NEG-004`); `FSM-W0500` RETIRED to `DeprecatedCode::W0500` (vestigial; `allow`/`deny`/suppression contract preserved + test-pinned). Live `DiagnosticCode` count moves **74 → 73** (the `all_codes_matches_expected_count` CI-lock updated in lockstep).
- **SEC-FU / RUSTSEC-2026-0009** (`e509734`, §11.42) — `time` ≤0.3.36 DoS (CVSS 6.8), pulled transitive-only via `jsonschema 0.17 ← fsm-ir`. The advisory's literal remedy (`time ≥0.3.47`) was infeasible (needs rustc ≥1.88 / `edition2024`, which the load-bearing **1.75** pin cannot parse). Fixed at the **root**: `jsonschema` `0.17 → 0.22` in `crates/fsm-ir/Cargo.toml` (0.22.0 dropped the `time` edge; caret resolves 0.22.3; MSRV 1.70 still satisfies the 1.75 pin). Re-scan: 0 vulnerabilities; `time` now absent from `Cargo.lock`.
- **FU#68** (`21a2380`/`a3dba92`, §11.43) — `[workspace.lints.rust] unreachable_pub = "warn"` wired workspace-wide with every crate opting in; the 14 residuals (the `fsm-lsp` `semantic_tokens.rs` legend/modifier consts — 11 token-type + 3 modifier) downgraded `pub → pub(crate)`; the over-`pub` invariant now self-enforces via the quad's `clippy -D warnings`.
- **DRIFT-2** (`7cf1174`/`d399623`, §11.44) — the two divergent byte→line/col implementations converged to ONE `fsm_diagnostics::compute_line_col(src, pos, LineColUnit::{Byte|Scalar})` core; the LSP `position.rs::LineIndex` is a structurally-different precomputed table, **deliberately left-and-explained** (the §10 refactor-to-number anti-pattern, practised). No new deps.
- **DRIFT-3/-4/-5** (`a1c726b` / `d90cd2f` / batched in `a74bdb0`, §11.45/§11.46/§11.39) — `docs/20-Architecture-Overview.md` reconciled to shipped reality (annotate-not-delete, every claim cited `file:line`): the aspirational `parse_incremental`/`analyze_incremental` API and §5.2 two-`phase{1,2}_*` analyzer layout do not exist (full re-parse only; flat shipped analyzer) — annotated as v1.3+-deferred / conceptual-model. Docs-only, code-inert (audit A-7).
- **SCA CI step** (`a74bdb0`, §11.42/§11.48) — `.github/workflows/ci.yml` gains a distinct `sca:` `cargo audit` job on a separate recent stable toolchain (the 1.75-pinned `cargo-audit 0.21.1` cannot parse the modern CVSS-4.0 DB); it does **not** gate the 1.75 build matrix. Operationalises the self-dropped v1.0 audit recommendation that caught RUSTSEC-2026-0009 pre-tag.

---

## 3. Independent pre-tag audit (read-only; frozen evidence)

`docs/AUDIT_PRE_TAG_v1_2_2026_05_16.md` (independent third pass, audited HEAD `a74bdb0`, READ-ONLY, zero `cargo`, every quantity re-derived from `git`/`grep`/file-reads — never echoed from docs) returned:

> **`TAG-CLEAR`** — 0 P0, 0 P1, 1 P2, 2 P3.

| Lens | Verdict | Key result |
|---|---|---|
| **A — Release-record integrity** | 0 P0/P1/P2/P3 | Every §11.40–§11.49 row commit-accurate at the claimed magnitude; both NIT corrections (FU#67 test-split, DRIFT-5 `:179`) independently confirmed correct; all 10 cited SHAs resolve and are ancestors of `a74bdb0`; CHANGELOG `[1.2.0]` structurally complete, no entry claims unshipped work. |
| **B — LSP epic structural soundness** | 0 P0/P1/P2/P3 | "One-analysis-feeds-all" structurally true (both LSP and CLI call the single `analyze_with_source`); L1–L7 handlers real-wired (not stubs); each wave has a real `tower-lsp` client behavioural test (35 `#[tokio::test]`); `forbid(unsafe_code)` = 11 roots / 10 crates. |
| **C — Gate criteria + no-P0** | 0 P0, 0 P1, **1 P2 (C-1)** | All gate numerics independently re-derived and matched (see §1 + the pinned numbers below). C-1 = the v1.2 gate doc is the tag-time deliverable, expected-not-yet-authored — **not a blocker**. |
| **D — Whole-corpus coherence / residual drift** | 0 P0/P1/P2, **2 P3 (D-F, D-G)** | No new doc↔code drift beyond DRIFT-1..5; all pinned attestations intact; COVERAGE_MAP arithmetic self-consistent (39 Tested + 34 UNTESTED + 2 RETIRED = 75; live = 75 − 2 = 73). |

### 3.1 Disposition of the non-blocking findings

- **C-1 (P2) — RESOLVED by this document.** §11.49 itself defers `GATE_VERIFICATION_v1_2.md` to tag-time ("authored at tag-time"), and v1.0/v1.1 followed the identical pattern (gate doc authored at the tag commit, not before). The audit recorded it so the tag step would not forget the three musts: (a) record 0-P0, (b) carry the CST-coupling deferral as accepted-tracked-debt with the v1.1 precedent, (c) pin live=73 / tests=806/0 / forbid=11. All three are satisfied by this document (§1, §"Accepted-tracked-debt", §"Pinned gate numbers"). The finding resolves itself by this file existing at the release commit.
- **D-F (P3) — FIXED in this same commit.** The audit found Doc 10's `FSM-E0903` catalog entry lacked the retired-status banner its co-retired sibling `FSM-W0500` received. This commit applies the **identical** banner format to E0903's entry (Deliverable 2; the W0500 banner's authoritative provenance is `0747321`, not `a74bdb0` as the audit's D-F evidence cell loosely attributes — verified by `git log -S` / `git blame`; the parity gap itself is exactly as the audit describes). E0903 independently confirmed genuinely retired: it is in `fsm_diagnostics::deprecated::DeprecatedCode` (`crates/fsm-diagnostics/src/lib.rs:528`), absent from the live `for_each_code!` body, and `DiagnosticCode::from_str("FSM-E0903").is_none()` is test-asserted (`lib.rs:803`). The live count is **unchanged at 73**.
- **D-G (P3) — no action (footer shorthand, same span).** The Doc 00 §11 footer text splits the range by append-date (`§11.19-39 … 2026-05-15; §11.40-49 … 2026-05-16`) while the consolidation commit message uses the cumulative shorthand "§11.19-49". Both denote the same §11.19–§11.49 span; the footer's dated split is the *more* precise historical record. Not a defect — the audit flagged it informational only.

### 3.2 Why the audit is not a rubber-stamp (recorded so the record is honest)

A pre-tag pass over a 10-row ledger + 7-wave epic that finds nothing is itself suspect. The audit independently re-derived **17 quantities** rather than echoing docs, and the derivations surfaced substance: the §11.41 test split required a per-file `#[test]` recount that **confirmed the ledger row is right and `24f231d`'s own commit message is the inverted one**; DRIFT-5's `:179` correction was line-verified against `pub fn build`; the "one-analysis" claim was proven by tracing both call sites to the single `analyze_with_source`; and Lens D found the genuine (P3) Doc 10 E0903/W0500 status-banner **asymmetry** the batched doc-honesty pass missed for E0903 (fixed here). The ledger and epic are unusually clean *because* every closeout wave reported-then-`a74bdb0`-applied with per-commit `git show` verification — corroborated, not assumed.

---

## 4. Cold-from-source release quad (§11.22 / §11.30) — the tag gate

### 4.1 The binding commit-ordering rule (Doc 00 §11.30, quoted)

> **Refinement (binding for v1.2+):** the release sequence is (1) author+commit `GATE_VERIFICATION_v<x>.md`, (2) run the cold-from-source quad at THAT commit, (3) `git tag -a` THAT commit + the `checkpoint/<date>` anchor — quad-commit ≡ tag-commit exactly.

v1.1.0 had a provably-immaterial one-pure-docs-commit gap between its quad commit (`eb35d4b`) and its tag commit (`abc7004`), tolerated **only because** that gate doc contained no code-summarising assertion a docs diff could falsify (§11.30; `GATE_VERIFICATION_v1_1.md` §7). **That grace does NOT apply to this gate doc.** This document *does* make code-summarising assertions (the §1 gate evidence, the §"Pinned gate numbers" — live=73, forbid=11, tests=806/0, conformance=26, RUSTSEC-fixed, `time`-absent) that a code delta could in principle falsify. Therefore the §11.30 sequence is followed strictly: this doc is committed first, the canonical cold quad runs **at that exact commit**, and the tag is placed on **that same commit** — zero off-by-one by construction.

### 4.2 Cold-quad method (§11.22)

§11.22 mandates a **cold from-source** green quad for a release tag: a warm shared-`CARGO_TARGET_DIR` can serve stale cross-worktree (`-wt-` baked-abs-path) test binaries, so a warm pass is necessary-but-not-sufficient. The method: invalidate the shared target (cold from-source — every first-party crate and every test/integration binary recompiled from source so no worktree path can survive), then `cargo build/test/clippy/fmt --workspace` + the conformance runner (`fsm test tests/conformance`) + the §5.4 `gcc` compile-and-**RUN** acceptance (subsumed inside `cargo test --workspace` — `gcc_compile.rs`, `vending_machine_gcc.rs`, `degenerate_machines_werror.rs`, `integration_examples.rs`, the submachine/defer/parallel/opaque-extern runs, the make/cmake/cargo-rust/platformio worked integrations).

### 4.3 Advisory cold run already executed at `bca1411` (de-risking smoke + mandatory disk reclamation)

Recorded here as advisory evidence (the de-risking smoke run + the disk reclamation it required; not the canonical gating run — see §4.5):

```
rustc                                  1.75.0 (82e1608df 2023-12-21)   (pin == rust-toolchain.toml channel "1.75.0" — verified)
cargo build --workspace                exit 0, zero-warning (from-source)
cargo test  --workspace                806 passed / 0 failed / 108 test binaries
fsm test tests/conformance             26 / 26
cargo clippy --workspace --all-targets -- -D warnings   exit 0 (zero)
cargo fmt --all --check                no diff
§5.4 gcc compile-and-RUN battery       all green (gcc_compile.rs, vending_machine_gcc.rs,
                                       degenerate_machines_werror.rs, integration_examples.rs
                                       make/cmake/cargo-rust/platformio, submachine / defer /
                                       parallel / opaque-extern runs)
disk margin                            3.0G → (clean) 22G → (rebuild) 12G = 15.4% free on /
```

The `806/0` / `26/26` / `rustc 1.75.0` / `clippy`-clean / `fmt`-clean / `0`-vulns figures are corroborated by the authoritative closeout assertion: the `0747321` commit-body Verified block — *"Verified: cargo build/test(806/0)/clippy/fmt all green workspace-wide; fsm test examples/ 5/5; tests/conformance/ 26/26; cargo audit 0 vulns / 0 new deps; forbid(unsafe_code) intact; rustc 1.75.0."* (The `108 test binaries` count, the full rustc build-hash string, the §5.4 battery enumeration, and the disk-reclamation figures are the advisory cold run's own recorded results — they are not re-derivable from git alone and are attributed as such, not overstated as a frozen attestation.)

### 4.4 Carry-forward reasoning (the v1.1 §7 logic, stated up front not retro)

`bca1411` **is** the v1.2 code corpus. The gate-doc commit `X = bca1411 + a docs-only delta` (this file `docs/GATE_VERIFICATION_v1_2.md` + the Doc 10 D-F E0903-banner edit; `git diff --stat bca1411 X` is docs-only — zero `crates/` / `Cargo.*` / source / test surface). A Markdown-only delta **cannot** change `cargo build/test/clippy/fmt` / conformance / `gcc`-RUN outcomes. Therefore the §4.3 advisory evidence validly carries to `X`.

### 4.5 The canonical confirming quad (the step the tag is contingent on)

The **canonical confirming cold-from-source quad is executed at the release commit `X` itself** (orchestrator-run after this document commits), and its transcript is recorded in the **annotated `v1.2.0` tag message** — the immutable artifact produced at tag time. So quad-commit ≡ tag-commit = `X`, satisfying §11.22 (cold from-source) **and** §11.30 (zero off-by-one) simultaneously. This run **has not yet happened** at the time this document is written; it is described here as the **gating step the tag is contingent on**, not a completed result — no run-at-`X` transcript is fabricated. The tag is placed only if that canonical quad is green; the §4.3 advisory run is the de-risking predicate that makes a red canonical run highly improbable, not a substitute for it.

---

## 5. Post-tag action required of the repository owner (G9)

The tag and the unpushed commits are **local only** (the owner controls the remote — a hard project rule). The GitHub Actions matrix (linux/macos/windows × fmt/clippy/build/test) has **never executed against any v1.2 commit**; the local-equivalent quad is green but platform-specific behaviour is unverified on the runners. Additionally the new SCA `cargo audit` `ci.yml` job (`ci.yml:55` `sca:`, added `a74bdb0` per §11.42/§11.48) is **likewise unexercised** — it ran for real locally (catching RUSTSEC-2026-0009 pre-tag) but has never run inside CI.

This is the **same documented-accepted-with-patch-lane posture as v1.1 §5**: accepted to tag before CI runs only because the local quad is green and the patch lane exists. **The owner should push the tag + commits to exercise the full CI matrix and the SCA job, and keep a v1.2.1 patch lane ready** — exercised promptly post-push, not deferred.

---

## 6. Accepted-tracked-debt (consumer-relevant)

**`analyzer → parser-CST coupling` (~15 files) is deferred — accepted, tracked debt.** The architecture audit rated it explicitly *ship-acceptable, non-behavioural* P1. Per the binding owner decision in **Doc 00 §11.49**, a ~15-file refactor of the most behaviourally-critical crate (`fsm-analyzer`) **at a release boundary** is worse-EV than doing it cleanly post-tag (a tracked v1.2.1/early-v1.3 wave, DRIFT-2-grade discipline). This is **precedent-consistent with v1.1.0**, which shipped the **identical class** of arch-debt tracked + documented in its `GATE_VERIFICATION_v1_1.md` (the §3 Architecture-lens carried-P1 "analyzer→CST coupling … non-behavioural, ship-acceptable, gate before v1.2", `docs/GATE_VERIFICATION_v1_1.md` §3 / line 44). Deferring here is lower-risk, precedent-consistent, and still gets done properly. Recorded in CHANGELOG `[1.2.0]` *Known limitations* and the ROADMAP. It is the **only** accepted architecture-debt item (audit Lens C confirmed).

---

## 7. Sign-off

v1.2.0 meets the release gate: 0 open P0; the independent third-pass audit returned `TAG-CLEAR` (0 P0 / 0 P1 / 1 P2 / 2 P3); the lone P2 is this tag-time deliverable (self-resolving); D-F is fixed in this same commit; D-G is a no-action footer-shorthand cosmetic. Scope statements are code-verified and conservative — every gate numeric independently re-derived (live `DiagnosticCode`=73, `forbid(unsafe_code)`=11 roots/10 crates, conformance=26, closeout 806/0, RUSTSEC-2026-0009 root-eliminated, `time` absent from `Cargo.lock`); the recurring prose-vs-code overstatement class is confirmed absent from the v1.2 surface; the one accepted architecture-debt item is tracked precedent-consistently with v1.1; all residuals are recorded in CHANGELOG `[1.2.0]` and the ROADMAP. The release is contingent on the canonical cold-from-source quad (§4.5) at the release commit being green.

Tagging `v1.2.0` (annotated, local) with `checkpoint/2026-05-16` as the rollback anchor.

— TL/PM, FSM Studio

---

## 8. Tag topology (pre-emptively correct per §11.30)

Constructed §11.30-clean from the start (no v1.1-style off-by-one to reconcile retroactively):

- **Sequence:** (1) author + commit this document + the Doc 10 D-F edit → commit `X`; (2) run the canonical cold-from-source quad **at `X`**, transcript recorded in the annotated tag message; (3) `git tag -a v1.2.0` **at `X`** + `git tag checkpoint/2026-05-16` **at `X`**. Result: **quad-commit ≡ tag-commit ≡ `checkpoint/2026-05-16` commit ≡ `X`**, exactly — zero off-by-one by construction (§11.30 satisfied at its source, not reconciled in the record after the fact as v1.1 had to).
- **Pin lineage (immutable, not moved):** v1.0.0 → `b4800036…`; v1.1.0 → `abc70044…` (== `checkpoint/2026-05-15`); v1.2.0 → `X` (== `checkpoint/2026-05-16`). The annotated `v1.2.0` tag *object* SHA dereferences to commit `X` (the same annotated-tag-object-vs-commit distinction §11.30 / `GATE_VERIFICATION_v1_1.md` §7 documents — recorded here pre-emptively so no future metrics wave misreads it as a mismatch). Release tags are **immutable and never moved** (re-cutting a release tag is a destructive op never performed).
- **Frozen-attestation discipline:** `GATE_VERIFICATION_v1_0.md` / `_v1_1.md` correctly cite "75" / "36/75" *at their pinned commits*; a frozen attestation is never rewritten. This document cites the v1.2 live values (73 / 26 / 11 / 806-0) *at `X`*. Both are correct at their respective pinned commits — Doc 00 §11.47 / §11.40 codify exactly this.
