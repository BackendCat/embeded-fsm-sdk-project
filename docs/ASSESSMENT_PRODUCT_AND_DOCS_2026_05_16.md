# FSM Studio — Product-Surface + Feature-Gap + Doc-Corpus Assessment

- **Document ID:** FSM-ASSESS-PRODOC-2026-05-16
- **Version:** 1.0.0 (owner-facing strategic input; READ-ONLY — produces this doc only, zero code/build)
- **Status:** Owner + TL review input for the upcoming review + v1.4-roadmap planning. Does **not** block v1.4-core execution. Recommendations only — owner/TL weigh and decide.
- **Author:** independent assessment (senior product/compiler engineer lens; adversarial — verify-vs-code, do-not-echo-the-docs).
- **Base commit:** `a9a5001` (main HEAD, post-`v1.3.0`; the `docs: v1.4 epic …` commit).
- **Method:** every claim independently derived from `crates/` / `editors/vscode/` / the CLI surface / `schema/` / `.github/`. Where the assessment relies on a doc claim it says so and marks it verified-or-not.
- **Owner questions answered:** (A) "what is our package actually providing" + "features that should be planned but aren't in the docs"; (B) "is the initial doc done well and finally containing the current actual state".

---

## 0. Executive summary (read this first)

**PART A — product surface.** FSM Studio at `a9a5001` is a **complete, deterministic, heap-free FSM-Lang → C99 compiler toolchain with a full LSP and a shipped VS Code extension** — 10 Rust crates (~46k LOC src), a 9-subcommand `fsm` CLI, an 8.7k-LOC `tower-lsp` language server, a ~3.4k-LOC VS Code extension, a battle-tested in-process RTC simulator + trace model, and an incoming verification core (Doc 30, well-planned). The shipped surface is strong and the v1.4 "verification core" cut (Doc 30) is the right next minor — independently confirmed.

**The single highest-value gap NOT in the ROADMAP/docs, ranked #1: MISRA-C / safety-cert compliance of the generated C.** The literal string "MISRA" appears in **0 files** across the entire doc corpus and codebase. For a product whose own Doc 01 positions it at "automotive safety systems (ISO 26262)", "aerospace/defense (DO-178C)", "medical device firmware (IEC 62304)" and that the owner explicitly targets *factory/safety* deployment, shipping generated C with **no documented compliance posture** is the largest strategic miss. Ranked #2: a **published `--json` schema + exit-code contract** (the IR has a JSON Schema; the CLI's machine-readable `--json` diagnostics — the actual CI integration surface — has none, and `--json` exists on only ONE of nine subcommands). Ranked #3: **requirements/property → spec → generated-code traceability artifact** (a regulated factory audit deliverable). Full ranked table in §A.4.

**PART B — doc-corpus verdict: `DOCS-CURRENT-WITH-NOTES`.** The *living* docs — ROADMAP, CHANGELOG, Doc 00 §11 ledger, the GATE_VERIFICATION_v1_* / AUDIT_PHASE_* / AUDIT_PRE_TAG_* frozen evidence, the metrics snapshots, and Doc 30 — are **genuinely current, internally rigorous, and post-v1.3-accurate** (the heavy-reconciliation claim is verified TRUE for these). The reconciliation discipline is exemplary (the pre-tag audit even catches and corrects its own cardinal overstatement). **BUT** the *static spec-corpus header banners + the front-door README + the Doc 07 index + a stale root-level `VALIDATION_REPORT.md` were NOT swept**: Doc 14 still says "the LSP server is deferred to v1.1 … No `fsm-lang-server` binary ships in v1.0" (it shipped in **v1.2**, 8.7k LOC); Doc 13 says "Reserved — v1.1 surface" (its plan is now Doc 30/v1.4); Doc 12 says "Deferred to v1.1 … v1.0 ships C99 codegen only" (it's now its own minor between v1.3 and v2.0); the README "Project Status" says "v1.0 in active development / Deferred to v1.1+: LSP, VS Code extension" at a post-v1.3 HEAD; Doc 07 says "All 22 specification documents have been written" while Docs 23–30 exist. These are **first-contact docs a factory evaluator or new contributor reads first**, so the staleness is materially mis-leading despite the deep ledger being correct. Full findings table in §B.3. Remediations are for a FUTURE doc-honesty pass — nothing fixed here.

---

# PART A — Product surface + feature-gap

## A.1 The full product surface today (independently derived)

Verified against `crates/*/Cargo.toml` + `src/` + `crates/fsm-cli/src/cli.rs` + `editors/vscode/` + `schema/` + `.github/workflows/ci.yml`.

### A.1.1 The compiler core (the product's spine)

| Crate | LOC (src) | What it provides | Verified |
|---|---|---|---|
| `fsm-diagnostics` | 1,077 | Span/Severity/DiagnosticCode/Diagnostic foundation; the 73-live-code enum | ✅ |
| `fsm-lexer` | 1,795 | zero-copy, `no_std`-friendly tokenizer | ✅ |
| `fsm-parser` | 5,796 | recursive-descent over rowan CST + typed AST | ✅ |
| `fsm-analyzer` | 6,362 | symbol table, type check, determinism, AST→IR lowering, 11 semantic checks | ✅ |
| `fsm-ir` | 2,091 | canonical IR pure-data types + serde JSON (the wire form) | ✅ |
| `fsm-codegen-c` | 7,472 | IR → deterministic heap-free C99, two dispatch strategies (switch/table/auto) | ✅ `#![forbid(unsafe_code)]` |
| `fsm-simulator` | 5,058 | pure-Rust RTC step engine; snapshot/restore; StepRecord trace model | ✅ |
| `fsm-formatter` | 3,184 | CST-in → canonical `.fsm`-out, idempotent | ✅ |
| `fsm-cli` | 4,396 | the `fsm` binary — 8 dispatched subcommands | ✅ |
| `fsm-lsp` | 8,717 | `tower-lsp` language server, `fsm-lang-server` binary, L1–L7 full | ✅ |

**Properties verified in code (not echoed from docs):** `fsm-codegen-c/src/lib.rs` carries `#![forbid(unsafe_code)]`; codegen uses `BTreeMap` for machine-strategy overrides specifically "to keep it deterministic"; Doc 11 §1 design goals (heap-free / portable C99 `-Werror` / no global state / deterministic / auditable-source-mapped) match the emit code structure. The "Cross-version commitments" (byte-identical reproducible output) are an architectural intent backed by the BTreeMap/sorted-emission discipline.

### A.1.2 The CLI surface (the factory/CI entry point) — `crates/fsm-cli/src/cli.rs`

Nine subcommands wired (`Parse`, `Check`, `Generate`, `Fmt`, `Test`, `Doc`, `Decompile`, `Init` + clap root). **`simulate`, `lsp`, `ir`, `completions` are spec'd in Doc 18 but deliberately NOT exposed** (Doc 20 §8 / Doc 00 §7.2 defer — verified by the `cli.rs` module doc and the absent variants). Exit codes are a documented 0–4 contract (Doc 18 §3, single-authoritative-source); `--json` machine-readable diagnostics exist on **`check` only** (`CheckArgs.json`; `generate`/`test`/`fmt`/`parse` have format-specific flags but no unified `--json`). This asymmetry is the #2 feature gap (§A.4).

### A.1.3 The simulator + trace model — `crates/fsm-simulator/`

A complete, deterministic, snapshot/restore-capable RTC interpreter (hierarchy, parallel, history, defer, timers, submachines per Doc 08 §12). `trace.rs` ships `StepRecord` (Doc 13 §11 byte-stable shape), `TraceFile`, `write/parse_trace_yaml`, `execute_trace` with per-step `first_mismatch`. The conformance suite is built on this. **No WebSocket/JSON-RPC server** (explicit in `lib.rs:3-7`) — Doc 30 correctly scopes that to v1.5.

### A.1.4 The LSP — `crates/fsm-lsp/` (shipped v1.2)

`tower-lsp`-based `fsm-lang-server` binary: `initialize` (UTF-8/UTF-16 encoding negotiation) + `publishDiagnostics`/`documentSymbol`/`foldingRange`/`hover`/`definition`/`completion`/`references`/`prepareRename`/`rename`/`semanticTokens`/`codeAction`/`inlayHint` — the full L1–L7 epic, single-file scope, all fed by ONE `analyze()` byte-consistent with `fsm check`. Cross-file/workspace intelligence + the `wasm32` build are deliberately deferred (v1.4 carry-over, Doc 26 §9).

### A.1.5 The VS Code extension — `editors/vscode/` (shipped v1.3, ~3.4k TS src + ~3.5k test, zero Rust delta)

`vscode-languageclient@9.0.1` → `fsm-lang-server`; Doc 21 TextMate grammar + language-config + snippets; 7 client/CLI commands; 2 activity-bar tree views; a CSP-locked read-only ELK diagram Webview (the one substantive-new piece, `elkjs`); host-only bundled installable VSIX (V6). 34 `@vscode/test-electron` Extension-Host tests. **VSIX publishing/signing is OUT** (owner credential decision); the 5-platform binary tail is G9-gated.

### A.1.6 The incoming verification core (Doc 30, planned, not yet built)

Doc 30 (FSM-PLAN-SIMVER-V14) is a high-quality kickoff plan: bounded explicit-state reachability + deadlock detection (`fsm verify`, driving the existing interpreter as the oracle) + trace differential replay (`fsm test --baseline`). It already does the ROADMAP↔shipped reconciliation work and surfaces two non-blocking owner-decisions. **Independently confirmed as the correct v1.4 cut** (see §A.3).

### A.1.7 CI / release infra — `.github/workflows/ci.yml`

A well-formed fmt+clippy+build+test matrix (ubuntu/macos/windows) + a separate `cargo-audit` SCA job (RustSec). **Never exercised** — local-only repo, owner controls the remote (a hard project rule; G9 carry-over since v1.0). No release-artifact pipeline, no `cargo install` story, no SBOM, no signed binaries (the #2/#5 gaps).

## A.2 What the product is, in one sentence

> A formally-specified, deterministic, heap-free **FSM-Lang → C99 compiler** with a complete developer toolchain (CLI + LSP + VS Code extension + simulator + formatter + conformance suite), targeting embedded/safety/factory deployment, MIT-licensed, with a verification core landing in v1.4.

This is an accurate, defensible product. It is **not yet** a *certifiable* toolchain for a regulated factory — which is exactly the gap §A.4 #1 identifies.

## A.3 Independent confirmation of the Doc 30 v1.4 cut

Doc 30's §1 reconciliation was spot-checked against code at `a9a5001`:

- **FSM-E0400 (unreachable state) / FSM-W0602 (no-incoming) are catalog-reserved but UNIMPLEMENTED** — confirmed: `grep -rn "E0400\|W0602" crates/` hits only the enum, `tests/negative.rs`, a CLI fixture, and `completion.rs` (a *different* code, W0101). No emission site in any analyzer check. Doc 30 §1.3 is correct; the ROADMAP's "extend the existing dead-transition analyzer" is genuinely infeasible-as-stated.
- **No WS server / no `tungstenite`/`axum`** in `Cargo.toml [workspace.dependencies]` (only `tower-lsp`/`tokio`, for the LSP). Confirmed.
- **No `wasm-bindgen` / no Monaco** anywhere. Confirmed.
- **Trace differential mechanism exists** (`execute_trace`/`first_mismatch`); only the cross-version harness is net-new. Confirmed.

**Verdict on the cut: endorse.** v1.4 = verification core (reachability/deadlock + trace differential replay), WS→v1.5, Web IDE→v1.6. Zero new network/wasm surface; highest value-per-risk; preserves the validated small-tight-tagged cadence. Doc 30 is one of the strongest planning docs in the corpus.

## A.4 Valuable features NOT in ROADMAP/docs — ranked by value-to-the-factory-mission

> Ranking criterion: value to the *factory/safety* deployment audience the owner named, weighted by what unblocks adoption in a regulated environment. **The top 3 are the ones this assessment most strongly argues should be planned.**

### 🥇 #1 — MISRA-C / CERT-C compliance posture for the generated C  *(STRONGLY ARGUE: PLAN IT)*

- **What.** A documented statement of which MISRA C:2012 (and/or CERT-C) rules the generated C conforms to, which it deliberately deviates from (with rationale + deviation records), and a `fsm generate --profile misra` (or equivalent) that emits a compliance-tightened variant + a per-machine compliance report. At minimum: a **MISRA conformance matrix doc** (rule-by-rule: Compliant / Deviation+rationale / Not-applicable) even before a code mode.
- **Why.** The literal string "MISRA" / "CERT-C" appears in **0 files** in the entire corpus (`grep -rl "MISRA" docs/ README.md` = 0; the earlier broad grep matched "qualified_name"/"certify" substrings, not the standard). Yet Doc 01 §1 explicitly positions the product for "automotive safety systems (ISO 26262)", "aerospace/defense (DO-178C)", "medical device firmware (IEC 62304)". A safety factory **cannot adopt a code generator into a MISRA-mandated codebase without a compliance/deviation story** — it is frequently a procurement gate, not a nice-to-have. The generator already does the hard part (heap-free, no UB by construction, deterministic, bounded, source-mapped); it is plausibly *close* to MISRA-clean already. Closing this is disproportionately high leverage: it converts "interesting tool" into "adoptable in our regulated line."
- **Value to the factory audience.** Highest in this list. This is the difference between a pilot and a production-line dependency for the named audience.
- **Effort.** Medium for the conformance-matrix doc + a MISRA-checker pass over the existing emitted corpus (run `cppcheck --addon=misra` / a MISRA linter over the examples' generated C, triage findings, write deviation records). Medium-Large if a tightened `--profile misra` codegen mode + per-machine deviation report is included. Recommend phasing: doc+audit first, mode second.
- **Recommended slot.** **v1.5-candidate (matrix+audit) — strongly argue for explicit planning now.** The codegen mode could be v1.6 or its own minor. *Not v1.4* (v1.4 verification core is correctly scoped + already planned; do not bloat it — but this should enter the ROADMAP as a named, sequenced item, not stay invisible).

### 🥈 #2 — Published `--json` schema + exit-code contract + CI templates  *(STRONGLY ARGUE: PLAN IT)*

- **What.** (a) A versioned JSON Schema for the CLI `--json` diagnostic output (mirror what `schema/ir/1.0.0/model.json` already does for the IR — that pattern is proven in-tree). (b) `--json` extended to the factory-relevant subcommands (`generate`, `test`, `verify`) not just `check`. (c) A documented stable exit-code contract surfaced as a machine-contract doc section (Doc 18 §3 already has the table — promote it to a "CI integration contract" with stability guarantee). (d) Ready-to-copy CI templates (GitHub Actions / GitLab CI / a generic `Makefile` target) under `examples/ci/` that run `fsm check --json` + `fsm test` + (later) `fsm verify` and fail the pipeline on non-zero.
- **Why.** The factory/pipeline-completeness-before-UI philosophy the owner stated makes the *machine-consumed CLI contract* the product's primary integration surface — yet `--json` exists on **only `check`** of nine subcommands, there is **no published schema** for it (only for the IR), and there are **zero CI templates** (the `examples/integration/{make,cmake,...}` are *build* integrations, not *CI-gate* integrations). A factory integrating this into a regulated pipeline needs a stable, versioned, schema-validated machine contract or every toolchain upgrade is a silent breakage risk (this is literally the "non-reproducible across compiler versions" risk Doc 01 §1 sells against — ironic to leave it open in the *tool's own* output).
- **Value to the factory audience.** Very high — this is the actual integration seam for "FSM Studio in our CI gate."
- **Effort.** Small-Medium. The schema is a mechanical mirror of the existing Doc 18 §4 JSON shape + the proven `schema/ir/` pattern; extending `--json` is per-command plumbing; CI templates are small.
- **Recommended slot.** **v1.4-scope-adjacent / v1.5.** The schema + exit-code-contract doc is cheap enough that folding the *schema + contract doc* into the v1.4 cycle (alongside `fsm verify`'s own `--json` which Doc 30 §4.2-W4 already anticipates) is the natural moment — v1.4 *adds* a subcommand whose output a factory will want to consume, so define the contract as v1.4 lands it rather than retrofitting. CI templates can trail to v1.5.

### 🥉 #3 — Requirements/property → spec → generated-code traceability artifact  *(ARGUE: PLAN IT)*

- **What.** A `fsm generate --emit-traceability` (or a `fsm trace-matrix`) producing a machine-readable artifact mapping: each FSM-Lang construct (state/transition/guard) → its IR node (the IR already carries `SourceLocation`) → the exact generated-C line(s) (Doc 11 §1 design goal #5 already requires source-line comments in the emit) → optionally a user-supplied requirement ID tag. Output: a CSV/JSON/HTML matrix a regulated factory attaches to its safety case.
- **Why.** Bidirectional traceability (requirement ↔ design ↔ code) is a **named deliverable** in ISO 26262 / DO-178C / IEC 62304 — the exact standards Doc 01 cites. The raw material **already exists**: the IR has `SourceLocation`, codegen already emits source-line-mapped comments (design goal #5, verified in `emit/license.rs` / the source-mapping discipline). What's missing is *materializing it as a first-class audit artifact*. This is unusually low-effort for unusually high regulated-adoption value because the data is already threaded end-to-end.
- **Value to the factory audience.** High — a safety-case enabler; pairs naturally with #1 (a MISRA deviation record + a traceability matrix are the two artifacts a safety auditor asks for first).
- **Effort.** Medium. The plumbing exists; the work is a dedicated emitter + a stable artifact schema + worked example.
- **Recommended slot.** **v1.5** (sequence with / after #1; they are the same "regulated-adoption" theme and should be one planned arc, not scattered).

### #4 — gcov / coverage harness for generated firmware

- **What.** A documented recipe + `examples/coverage/` showing the generated C compiled with `--coverage`, exercised by a captured trace corpus, producing gcov/lcov state+transition coverage; optionally `fsm` emitting a coverage-instrumentation-friendly variant.
- **Why.** Structural coverage of the *generated* state machine (every state entered, every transition taken) is a DO-178C/IEC 62304 evidence item and a natural pairing with the trace differential replay landing in v1.4 (the trace corpus *is* the coverage driver). Doc 01 §1 explicitly sells "untestable logic" as the pain it solves — coverage evidence closes that loop.
- **Value to the factory audience.** Medium-High (regulated test-evidence).
- **Effort.** Small-Medium (mostly a documented recipe + example; the trace corpus from v1.4 W3 is the input).
- **Recommended slot.** **v1.5** (rides on the v1.4 trace-baseline corpus; bundle with the #1/#3 regulated-adoption arc).

### #5 — Reproducible-build attestation + SBOM + signed/prebuilt CLI release artifacts + `cargo install`

- **What.** (a) `cargo install fsm-cli` / crates.io publish OR signed prebuilt binaries as GitHub Release assets (Doc 03 §660 already *mandates* "Binaries MUST be distributed as GitHub Releases assets" — unimplemented). (b) An SBOM (CycloneDX/SPDX) generated in CI (`cargo cyclonedx`). (c) A reproducible-build attestation doc (the "byte-identical output" cross-version commitment is asserted but not *attested* with a reproducibility CI check). (d) Checksums + signing (cosign/minisign) of release artifacts.
- **Why.** Supply-chain provenance is increasingly a procurement gate for safety/defense (SLSA, EO 14028 lineage). The product *promises* deterministic reproducible output but ships no *attestation* of it, no SBOM, no signed artifacts, and no install path — a factory security team will ask for all four. The `cargo-audit` SCA job exists in CI but CI has never run (G9).
- **Value to the factory audience.** Medium-High (security/procurement gate; lower urgency than #1–#3 because it gates *deployment* not *adoption-decision*).
- **Effort.** Medium. Gated on the owner enabling the remote/CI (the standing G9 owner-action) — flag this dependency explicitly.
- **Recommended slot.** **v1.6 / paired with the owner's G9-remote-enablement decision.** Surface the Doc 03 §660 "MUST" as a *known unmet normative requirement* in the doc-honesty pass (it currently reads as done-by-fiat).

### #6 — Multi-machine projects + cross-machine reference resolution

- **What.** The ROADMAP v2.0 item ("discover, address, route across compile units"). Today `send EVT to Foo` works for a known instance only.
- **Why.** Real factory systems are multi-FSM. Currently single-machine-centric.
- **Value.** Medium (real but the named audience's *first* blocker is certifiability, not multi-machine).
- **Effort.** Large.
- **Recommended slot.** **Keep at v2.0** (ROADMAP placement is correct; noted here only to confirm it should NOT be pulled forward ahead of the regulated-adoption arc).

### #7 — Web IDE (Monaco + wasm) / docs-landing site

- **What.** ROADMAP already has Web IDE at v1.6. A separate marketing/docs landing site (the owner cited EdgeForge `/dev/18` as a model) is DX/very-future.
- **Why.** DX/discoverability — *not* a factory-adoption blocker. The factory audience integrates via CLI, not a browser IDE.
- **Value.** Low-Medium for the *named* mission (high for broad OSS adoption, a different audience).
- **Effort.** Largest (Web IDE) / Medium (landing site).
- **Recommended slot.** **Web IDE stays v1.6** (correct). **Docs-landing site = future / explicit DX-track, NOT on the factory critical path** — recommend an explicit "non-goal for the safety-mission roadmap; revisit if OSS-adoption becomes a goal" note so it doesn't silently compete for slots.

### #8 — Profile-guided codegen / assembler-codegen target

- **Profile-guided codegen** is already a v2.0 ROADMAP item — correct placement, no change.
- **Assembler-codegen target** (Doc 17 exists): **recommend explicit DEFER / probable non-goal**, per the owner's own stated reasoning. The C99 path + `arm-none-eabi-gcc` covers the embedded target space; a hand-maintained asm backend is enormous surface for marginal value vs. a certifiable C path. Recommend the doc-honesty pass add an explicit "Doc 17 is a reference strategy, not a planned codegen target for v1.x/v2.0" note (Doc 17 currently reads as a peer spec, implying intent).

### A.4.1 The 2–3 to plan now (the strong argument)

1. **#1 MISRA-C / cert compliance posture** — without this the named factory/safety audience structurally cannot adopt the tool into a regulated line. It is invisible in the docs (0 mentions) yet the codegen is plausibly close. **Highest strategic leverage; argue for an explicit ROADMAP slot (matrix+audit ≈ v1.5).**
2. **#2 Published `--json` schema + exit-code contract** — the factory-first philosophy makes the machine CLI contract the *primary product surface*, yet it's unschema'd and `--json` is on 1 of 9 commands. Cheap, foundational, and v1.4 is the natural moment (it adds `fsm verify` whose output a factory will consume). **Fold the schema+contract doc into v1.4; templates v1.5.**
3. **#3 Traceability artifact** — a regulated safety-case deliverable whose data is *already threaded end-to-end* (IR `SourceLocation` + source-mapped emit). Disproportionate value-per-effort; same regulated-adoption arc as #1. **v1.5.**

Together #1+#3(+#4 coverage) form one coherent **"regulated-adoption / certifiability" arc** that the ROADMAP currently lacks entirely. The strongest single recommendation of this assessment: **add a named "Certifiability & factory-integration" track to the ROADMAP** (slotting #1/#2/#3/#4/#5), sequenced after the v1.4 verification core, because verification (deadlock-freedom proof) is itself a safety-case input — the arc composes.

---

# PART B — Doc-corpus accuracy / completeness

## B.1 Verdict

> ## `DOCS-CURRENT-WITH-NOTES`

The **living/evidence docs are current and rigorous** (heavy-reconciliation claim **verified TRUE** for them). The **static spec-corpus status banners + README + Doc 07 index + a stale root report were NOT swept** and now materially misrepresent shipped state to a first-time reader. No internal *contradiction within* the living set; the staleness is concentrated in the static-spec front matter the reconciliation passes did not re-touch.

## B.2 What is genuinely current (verified, not assumed)

- **ROADMAP.md** — last-updated 2026-05-16; correctly shows v1.0/v1.1/v1.2/v1.3 SHIPPED, v1.4 next, the §11.40 re-scope worked example, C++17 as its own minor. Accurate.
- **CHANGELOG.md** — `[1.3.0]` 2026-05-16 entry is detailed, honest (W0 + V1–V6, "zero Rust delta vs ceb8efd" correctly scoped), Known-limitations sections per release. Accurate.
- **Doc 00 §11 ledger** — runs to **§11.62** (v1.3 closeout), heading-format `| §11.NN | …` table rows; the v1.3 rows (§11.50–§11.62) match shipped reality. Accurate. (v1.4 rows correctly reserved to start §11.63 per Doc 30.)
- **GATE_VERIFICATION_v1_3.md** — PASS verdict, G1–G9 table, the W0-is-the-Rust-delta framing correctly stated (not the erased-W0 overstatement). Accurate + the §11.29 "verify-the-audit-too" rigor is visibly applied.
- **AUDIT_PRE_TAG_v1_3_2026_05_16.md** — TAG-CLEAR, independently re-derives every load-bearing git number, *explicitly catches that a prior closeout had ONE cardinal overstatement and verifies it was corrected*. This is exemplary record discipline.
- **docs/metrics/2026-05-16-v1_3.json** — every metric carries a reproducible `method`; the Rust-delta-is-exactly-W0 fact is stated with the exact `git diff` invocation. Accurate.
- **Doc 30 (v1.4 plan)** — base commit `a036c38`, reconciles ROADMAP↔shipped against code, surfaces owner-decisions with non-blocking defaults. High quality; current.

**The heavy-reconciliation claim is TRUE for the living corpus.** The verify-the-record check passes for everything that gets touched per-release.

## B.3 Findings table (drift / staleness / gaps — for a FUTURE doc-honesty pass; NOT fixed here)

| # | File:line | Drift / gap | Severity | Recommended remediation | Fold-point |
|---|---|---|---|---|---|
| **D-01** | `README.md:132-140` ("## Project Status" → "v1.0 in active development" / "Phase 0 scaffolded: 2026-05-11" / "Scope: full UML → C99 → CLI tool only" / "Deferred to v1.1+: … LSP, VS Code extension, Web IDE, …") | **The single most misleading staleness.** HEAD is post-`v1.3.0`; LSP shipped v1.2, VS Code extension shipped v1.3. The product's front door tells a factory evaluator the tool is a pre-1.0 CLI-only scaffold. | **High** | Rewrite "Project Status" to reflect v1.3.0 shipped + v1.4 next; update the "What v1.0 ships" / "Roadmap (post-v1.0)" sections (Roadmap still lists "LSP server" / "VS Code extension" as future). Point to ROADMAP.md as the live source. | Doc-honesty pass (do FIRST — it's the front door) |
| **D-02** | `docs/14-LSP-Capability-Spec.md:5-12` ("Status: Deferred to v1.1" + "v1.0 NOTE … No `fsm-lang-server` binary ships in v1.0") | The LSP **shipped in v1.2** — `crates/fsm-lsp/` is 8,717 LOC, `fsm-lang-server` binary exists, L1–L7 complete. The spec's own status banner says it doesn't ship. | **High** | Change Status to "Implemented in v1.2 (single-file scope; cross-file v1.4)"; keep the normative capability spec body, replace the "v1.0 NOTE … deferred" admonition with an "as-shipped" pointer to Doc 26 + CHANGELOG [1.2.0]. | Doc-honesty pass |
| **D-03** | `docs/13-Simulator-Protocol.md:5-12` ("Status: Reserved — v1.1 surface" + "v1.0 NOTE … does NOT activate in v1.0") | The WS server is now v1.5-scoped (Doc 30 §2), not "v1.1". The "v1.1" label is two minors stale; the in-process simulator it describes as the v1.0 fallback is itself far more capable now. | **Medium** | Status → "Spec normative; server deferred to v1.5 (Doc 30 §2.1). In-process simulator shipped." Cross-link Doc 30. | Doc-honesty pass |
| **D-04** | `docs/12-Codegen-CPP17.md:3-15` ("Status: Deferred to v1.1" + "v1.0 NOTE … v1.0 ships C99 codegen only") | C++17 is now "its own minor between v1.3 and v2.0" (ROADMAP). "Deferred to v1.1" is stale; "v1.0 ships C99 only" is true-but-misleading at a post-v1.3 HEAD. | **Medium** | Status → "Deferred to its own post-v1.3 minor (ROADMAP §C++17); unscheduled." Replace the v1.0 NOTE accordingly. | Doc-honesty pass |
| **D-05** | `docs/07-Specification-Roadmap.md:~24-40` ("All 22 specification documents have been written" / status table stops at Doc 22 / per-doc Status all "✅ Complete" with no impl-status column) | Docs 23–30 exist (Onboarding, Testing, Integration, LSP-Arch, VSCE-Arch, the V13/W0/V14 wave plans) and are unlisted; the index claims completeness at 22. A reader using Doc 07 as the master index misses 8 docs incl. the entire v1.2/v1.3/v1.4 planning corpus. | **Medium-High** | Extend the table to Docs 23–30; either retitle to "specification + planning corpus" or split a "Planning docs of record" subsection (the Doc 28 precedent). Optionally add an "Impl status" column (Spec'd / Shipped vX / Planned vX). | Doc-honesty pass |
| **D-06** | `VALIDATION_REPORT.md:3-4` (repo root; "Date: 2026-02-18", "Scope: All 24 specification documents", "BLOCKER 14 / GAP 42 / Verdict: Needs revision") | A **pre-implementation (2026-02-18) spec-gate review** sitting at repo root with a "Needs revision / 14 blockers" verdict. Those blockers were resolved long ago (it predates Phase 0 scaffolding 2026-05-11). A new contributor / evaluator reading the root sees a damning stale verdict. | **Medium-High** | Either move to `docs/archive/` with a dated "SUPERSEDED — pre-impl 2026-02-18 gate; all 14 blockers resolved, see CHANGELOG/Doc 00 §11" header, or delete (the GATE_VERIFICATION_v1_* docs are the live equivalent). Do NOT silently delete history — prefer archive-with-header. | Doc-honesty pass |
| **D-07** | `docs/03-Infrastructure-Requirements.md:660` ("Binaries MUST be distributed as GitHub Releases assets") + `:196` ("language server MUST be distributed as a standalone binary: `fsm-lsp`") | Normative MUSTs that are **unmet** (no release pipeline; the LSP binary is `fsm-lang-server` not `fsm-lsp` per `Cargo.toml`/`cli.rs` — a naming drift vs the requirement). Reads as satisfied-by-fiat. | **Medium** | Mark these as known-unmet-normative (link the G9-remote owner-action + the §A.4 #5 gap); reconcile the `fsm-lsp` vs `fsm-lang-server` binary-name requirement to shipped reality. | Doc-honesty pass (note: this is also feature-gap #5) |
| **D-08** | `README.md:46-52` ("Roadmap (post-v1.0)" lists "VS Code extension — … LSP, live diagram panel" and "LSP server" as future bullets) | Same class as D-01: README's own roadmap section lists shipped (v1.2/v1.3) items as future. | **Medium** | Folded into the D-01 README rewrite (single edit). | Doc-honesty pass (with D-01) |
| **D-09** | Doc-corpus-wide | **MISRA-C / CERT-C absent entirely** (0 files). Not a *drift* (nothing claims it) but a **completeness gap** for a doc set positioning itself (Doc 01 §1) at ISO 26262 / DO-178C / IEC 62304 audiences. A factory evaluator reading Doc 01's safety-market framing will look for and not find any compliance posture. | **Medium** (gap, not contradiction) | Out of scope for a *honesty* pass; this is the §A.4 #1 *product* recommendation. Noted here only so the doc-honesty pass and the roadmap pass are aware it spans both. | Roadmap pass (§A.4 #1), not the honesty pass |
| **D-10** | `docs/01-Scientific-Justification.md:Status` (Version 1.0.0, no status qualifier) + general | Doc 01 still reads as a pre-build justification ("a … toolchain is therefore required") in present-need tense though the toolchain now substantially exists. Minor — it's a justification doc, tense is defensible — but a "Status: foundational; the toolchain described is shipped through v1.3, see ROADMAP" header would prevent a reader thinking it's still aspirational. | **Low** | Add a one-line "as-of" status header pointing to ROADMAP/CHANGELOG. | Doc-honesty pass (low priority) |

### B.3.1 Pattern behind the findings (the actionable root cause)

**The reconciliation discipline is applied per-release to the *living* docs (ROADMAP/CHANGELOG/Doc 00 §11/GATE/metrics) but the *static spec-corpus front matter* (the `**Status:** … v1.0 NOTE …` banners on Docs 07/12/13/14, the README Project Status/Roadmap, the root VALIDATION_REPORT) was frozen at the 2026-05-14 v1.0 reconciliation and never re-swept as v1.1/v1.2/v1.3 shipped.** The bodies of those specs remain normatively useful; only their *status/applicability headers* lie. A future doc-honesty pass should: (1) introduce a single convention — every spec doc's Status line reads "Implemented vX / Planned vX / Spec-only" and is part of the per-release reconciliation checklist (the same rule the living docs already follow); (2) prioritize D-01/D-02/D-05/D-06 (the first-contact surfaces); (3) treat D-09 as a *product* item (§A.4 #1), not a doc edit.

## B.4 Claims that could not be verified

- **CI/quad GREEN assertions** (the `673/0`, `806/0/108`, cold-quad, `@vscode/test-electron` 34-test results) are stated in GATE/metrics docs but are **READ-ONLY-unverifiable here** (zero build by mandate). They are *internally consistent and reproducibly-described* (every metric carries a `method` invocation) and the pre-tag audit independently re-derived the git-level numbers — but the assessment cannot itself attest the test counts pass. Flagged per instruction. (This is expected and not a finding against the docs — it is the nature of a no-build review; the docs' own reproducibility-method discipline is the right mitigation.)
- **"Byte-identical reproducible output across versions"** (the cross-version commitment) is *architecturally supported* (BTreeMap/sorted-emission verified in `fsm-codegen-c`) but there is **no reproducibility-attestation CI check** proving it empirically — this is feature-gap §A.4 #5(c), not a doc lie (the docs state it as a *commitment*, not as an *attested fact*).
- The Doc 30 v1.4 plan's *quality* is assessable (high) but its *acceptance* (whether the implemented verifier will be sound) is necessarily unverifiable until built — flagged as the obvious open item, not a current defect.

---

## C. One-screen summary for the owner

**PART A — what the package provides:** a complete deterministic heap-free **FSM-Lang → C99 compiler** + full toolchain (CLI/LSP/VS Code ext/simulator/formatter/conformance) + an incoming (well-planned, Doc 30) verification core. Strong, defensible, MIT. The v1.4 verification-core cut is independently endorsed.

**PART A — the 3 features to plan that aren't in the docs (strong argument):**
1. **MISRA-C / cert compliance posture** (0 mentions corpus-wide; the named factory/safety audience structurally can't adopt without it; codegen is plausibly close) → **add a named "Certifiability" ROADMAP track; matrix+audit ≈ v1.5**.
2. **Published `--json` schema + exit-code contract + CI templates** (`--json` is on 1 of 9 subcommands; no schema for the CLI's machine output though the IR has one) → **fold schema+contract into v1.4** (it ships `fsm verify` a factory will consume); templates v1.5.
3. **Requirements→spec→code traceability artifact** (a regulated safety-case deliverable; the data is *already* threaded end-to-end via IR SourceLocation + source-mapped emit) → **v1.5, same arc as #1**.
   *Plus the meta-recommendation:* these three + gcov coverage + SBOM/signing form one coherent **"Certifiability & factory-integration" track the ROADMAP entirely lacks** — it composes with v1.4 verification (deadlock-freedom is itself safety-case input). This is the strongest single strategic input.

**PART B — doc verdict: `DOCS-CURRENT-WITH-NOTES`.** Living docs (ROADMAP/CHANGELOG/Doc 00 §11/GATE/metrics/Doc 30) are genuinely current and rigorously reconciled — the heavy-reconciliation claim is verified TRUE *for them*. **But** the static spec-corpus *status banners* + the front-door README + the Doc 07 index + a stale 2026-02-18 root `VALIDATION_REPORT.md` were never re-swept and now tell a first-time evaluator the LSP/VS-Code are unshipped and the tool is a pre-1.0 scaffold (10 findings, D-01/D-02/D-05/D-06 are first-contact surfaces — fix those first in a future doc-honesty pass). Root cause: per-release reconciliation covers the living docs but not the static-spec front matter; fix = make every spec's Status line part of the same per-release checklist.

*End of FSM-ASSESS-PRODOC-2026-05-16 v1.0.0 — owner-facing strategic input; does not block v1.4-core execution. No code/docs changed by this assessment beyond authoring this file.*
