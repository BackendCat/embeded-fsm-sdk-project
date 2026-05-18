# Changelog

All notable changes to FSM Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [factory-reliability-ci] — 2026-05-18

> **A quality milestone, NOT a `vX.Y.0` release.** The Factory-Reliability/CI
> epic (Phase-6.0) is tagged `checkpoint/factory-reliability-ci-2026-05-18`
> (the proven `checkpoint/<name>` immutable-anchor pattern — LOCAL, no push).
> It is deliberately **not** a `vX.Y.0` tag: the owner's roadmap reserves
> **v1.6.0 for the landing-page + doc-honesty pass (post-factory)**, and this
> epic is the quality milestone that *precedes* it (naming it `v1.6.0` would
> unilaterally consume the owner-reserved minor). The §11.30/GT-12 cold-quad
> rigor applies to this checkpoint tag identically to a release tag.

**Theme: the Factory-Reliability/CI proof layer — close ALL testing gaps to
factory-grade + a push-and-it-runs CI.**

> **Scope (Doc 32, owner-endorsed).** With the pipeline FACTORY-COMPLETE
> (v1.4) and the UI/DX convenience layer shipped (v1.5), this epic un-parks
> the **testing half** of Certifiability/OWNER-C: a host **and** on-target
> generated-C ↔ simulator-oracle byte-differential that drives the shipped,
> never-forked `fsm_simulator::execute_trace` oracle (the KEYSTONE
> DRIVE-THE-ORACLE binding tag-gate row — independently re-derived from
> source by the W6a phase-audit, verdict `KEYSTONE-INTACT`; **no** forked
> semantics in the W1 trace-hook / the W2 QEMU+target harness / the W5 CI
> glue; the differential oracle byte-untouched across the whole epic arc).
> The **MISRA-codegen half stays separately PARKED** (OWNER-3 — explicit
> owner GO required). The Phase-6.0 **Rust** delta vs v1.5 is **substantial
> and additive by design** (`git diff --shortstat 2ff8ac3 X -- crates/
> Cargo.toml Cargo.lock` = 22 files / +8192 / −190 — independently
> re-derived at the gate-doc commit, stated as exactly that delta, neither
> under- nor over-stated) with **0 new external dependencies** (the 2
> Cargo.lock edges are internal `fsm-codegen-c`/`tempfile` dev-deps already
> in the lock); the differential oracle (`crates/fsm-simulator/src` +
> `cmd/{test,baseline}.rs`) is byte-untouched.

### Added

- **The R7 generic host-trace differential (W1):** a `#ifdef
  FSM_TRACE`-gated codegen trace-tap (`crates/fsm-codegen-c/src/emit/
  trace_hook.rs`) + a host differential (`crates/fsm-simulator/tests/
  codegen_equivalence_smoke.rs`) that compiles **and RUNS** the generated C
  and byte-diffs its trace against the shipped `fsm_simulator::execute_trace`
  `StepRecord` oracle. The trace-hook is a **trace tap, not a second
  interpreter** — it records what the generated C *itself* did; the keystone
  is no forked oracle. A default `fsm generate` (no `-DFSM_TRACE`) carries
  the hook only as preprocessor-stripped text.
- **The on-target QEMU/`mps2-an385` trace differential (W2):** extends W1's
  **identical** `FSM_TRACE` C to a cross-compiled `arm-none-eabi-gcc
  -mcpu=cortex-m3` binary run under `qemu-system-arm -M mps2-an385`
  semihosting, against the **same** oracle (it does not invent a target
  oracle). CI-runner-only by design (the toolchains are never on the
  disk-tight box); the on-target *logic* is host-proven by W1's
  identical-C/same-oracle differential; the toolchain-free no-fork keystone
  guards pass locally; the on-target lane honest-skips when the toolchain is
  absent (an explicit by-design notice, not a gap).
- **The G7 build-failing live-enum-derived conformance lock (W3):** a `cargo
  test` (`crates/fsm-cli/tests/conformance_code_coverage_lock.rs`) that
  **fails the build** unless every live `DiagnosticCode` variant (derived
  from the compiled `DiagnosticCode::all_codes()`) has an exact-set
  conformance fixture OR a justified allowlist entry — `73 live codes = 37
  exact-set conformance fixtures + 36 justified non-fixture allowlist
  entries` (the lock's own runtime output, the partition dynamically
  asserted). `tests/conformance/COVERAGE_MAP.md` became a **generated
  artifact** (byte-derivability asserted read-only; regeneration
  feature-gated + `#[ignore]`d — the hand-maintained-map drift surface
  eliminated). The conformance corpus grew **26 → 53** fixtures.
- **The coverage-ratchet gates (W4):** a `cargo-llvm-cov` Rust-branch gate +
  a `c8` extension-coverage gate; the floor = `floor(measured−2%)`
  (measured-then-ratcheted, not a guessed target), `--check-monotonic`
  rejects a lowered floor (the script enforces, it does not decide), with an
  explicit no-assertion-free anti-gaming statement in `coverage-floors.toml`.
- **The ci.yml 4 additive-isolated lanes + the one-command `make ci-local`
  gate (W5):** the original 2-job `build`(1.75 matrix)+`sca` byte-untouched +
  4 own-job `needs:`-free lanes appended (`conformance` / `extension-host` /
  `coverage-rust` / `on-target`). `make ci-local` runs the quad + conformance
  + the `xvfb-run` Extension-Host JS lane + the `act` Linux-`build`/`sca`
  dry-run + the new-lanes' workflow-lint, and **exits non-zero on a broken
  tree** (the honest scope encoded inline: the on-target/coverage matrix is
  CI-runner-only; the on-target *logic* is host-proven by W1; `act` covers
  the *Linux* `build`/`sca` lanes only).

### Fixed

- **The FW109/FW110-FU codegen-correctness arc — real shipped codegen bugs
  the differential surfaced + fixed** (the audit working as designed, NOT a
  fork): a **timer over-fire** (single-shot `elapsed_ms` decrement → a
  correct multi-period budget-loop), a composite **exit-set** defect, a
  **deep_history** restore defect, **choice/junction** lowering (moved to the
  analyzer), a sibling-targeted **local (~>) transition LCA** defect, and an
  undeclared `_exit_<Final>`. These **deliberately changed production-C
  semantics for the better** — a *correctness fix*, not a regression and not
  a fork (the keystone is *no forked oracle*, not frozen output;
  independently re-derived from source by the W6a/W6b audits).
- **`run_differential` doc-comment honesty** (`crates/fsm-simulator/tests/
  codegen_equivalence_smoke.rs`): a stale doc-comment referencing a
  `Some(corrupt)` parameter absent from the signature was tidied to describe
  the actual contract (the corruption never-game proof lives in the dedicated
  `differential_goes_red_on_a_deliberately_corrupted_oracle` guard, not a
  parameter). Cosmetic; the signature/body are byte-unchanged and the
  differential re-ran deterministic-green post-edit.

### Known limitations

- **Carried owner-escalations (re-stated, not resolved):** the **G9** push
  of the LOCAL `v1.0.0`–`v1.5.0` tags + checkpoints + commits + this
  milestone tag + the CI matrix / SCA / the **4 NEW lanes**
  (`conformance`/`extension-host`/`coverage-rust`/`on-target`) **never run on
  a remote runner** (G9 is now **load-bearing** — the automatic remote
  pipeline + the on-target/coverage matrix depend on it; the JS lane is a
  binding *local* gate + `make ci-local` proves local-green, but remote-
  unrun); the **5-platform binary tail** (blocked-on-G9; no cross toolchains
  on the box; **not** a ci.yml lane); **Marketplace publish** (publish-*ready*
  via v1.5 B1, still owner-published — Phase-6.0 added no extension runtime);
  the top-level **`LICENSE`/`CONTRIBUTING`/`CoC`** gap (owner/legal, carried
  to v1.6 with D1); the parked **Certifiability / MISRA–CERT-C codegen-mode**
  arc (OWNER-3 — owner GO required; this epic un-parked only the *testing*
  half); the **v1.6 D1 landing + the doc-honesty pass that must
  precede/accompany it** (recorded, *not* scoped into this epic — "publish-
  ready" = this factory-reliability proof + the v1.6 D1 landing both land);
  the mechanised **sim≡codegen-equivalence formal proof** (R7 — Phase-6.0
  delivers the *empirical* host+target differential; the fully-mechanised
  proof remains the named v1.4-stretch/later deferral, unchanged); **task
  #17** CI-matrix owner-blocked. The broader README/Doc-07/Doc-14 doc-honesty
  staleness pass is a **separate already-identified backlog item** (flagged
  in `GATE_VERIFICATION_FACTORY_RELIABILITY_2026_05_18.md` §6.2, not fixed in
  this epic; the `docs/ROADMAP.md` "See also" pointer was already reconciled
  at the v1.5 closeout).
- **Accepted-tracked-debt (from the W6b pre-tag four-lens, `TAG-CLEAR`):**
  the 3 pre-existing **devDependency-only** transitive npm vulns
  (`{esbuild, mocha, serialize-javascript}` — 1 moderate / 2 high; NOT in the
  shipped extension runtime, not exploitable in our build/CI usage, no
  non-breaking fix, the epic introduced **zero** of them) — each
  `accept-with-written-rationale`, tracked for the v1.6 FE-hygiene batch; the
  `.contains` codegen-c conformance oracle (the #123-class deferred tail,
  owned by the G7 wave for behavioural conversion — the W1/W2 differential +
  the W3 lock are the binding behavioural gate in the interim).
- **G7 / G9** are the same explicitly-tracked posture accepted at v1.0–v1.5
  (G7 *improved* this epic: a build-failing live-enum lock + corpus 26→53;
  G9's 4 new lanes are *ready* + locally `make ci-local`-provable, still
  remote-unrun). Neither is a regression.

See `docs/GATE_VERIFICATION_FACTORY_RELIABILITY_2026_05_18.md` for the full milestone gate, the KEYSTONE DRIVE-THE-ORACLE evidence (the frozen W6a `KEYSTONE-INTACT` + W6b `TAG-CLEAR` audits), the cold-quad + binding-JS-lane posture, the re-derived pinned gate numbers, and the carried owner-escalations.

## [1.5.0] — 2026-05-17

**Theme: the "Convenience Layer" (UI/DX) — surface the v1.4 verification core in VS Code.**

> **Scope (Doc 31, owner-pre-framed + owner-delegated):** with the headless
> pipeline FACTORY-COMPLETE at v1.4 (the `[[feedback_embeded_fsm_pipeline_before_ui]]`
> "pipeline before UI" bar satisfied), `v1.5.0` is the unblocked UI/DX
> convenience layer: verification surfaced in VS Code via **two
> keystone-honouring seams** (a CLI-spawn of the same `fsm` binary the CI
> calls, *and* an `fsm-lsp`-embedded `fsm/verify` that calls the *same*
> `fsm_verify::verify`/`reachability_diagnostics` the CLI calls — there is
> **NO** verifier re-implementation in `crates/fsm-lsp/**` or
> `editors/vscode/**`; the KEYSTONE-IN-UI invariant, independently re-derived
> from source by the post-A2 audit, verdict `KEYSTONE-INTACT`), a
> Marketplace-ready extension manifest, and a scoped TS frontend-practices
> remediation. The v1.5 **Rust** delta vs v1.4 is **small and additive by
> design** (`git diff --shortstat 3bbcd0f d5fba3c -- crates/ Cargo.toml
> Cargo.lock` = 7 files / +933 / −1, confined to `crates/fsm-lsp/**`: the A2
> single `fsm-verify` path edge + the `fsm/verify` LSP capability + its
> acceptance test) with **0 new external dependencies**; the differential
> oracle (`crates/fsm-verify/` + `crates/fsm-cli/`) is byte-untouched. The
> headline surface is the TypeScript editor UX (TS delta 41 files /
> +2693 / −1026). **D1** (the static docs/landing site — the only
> droppable-from-tag item) **slipped to v1.6**.

### Added

- **Verification in VS Code (`fsm.verify` / `fsm.baseline` — CLI-spawn; A1):**
  two contributed commands consuming the **existing** `fsm verify --json` /
  `fsm baseline --json` (the `fsm-verify/v1` schema) via the proven
  `cliRunner`/`cliBinary` seam — zero Rust delta, zero new spawn machinery.
  INCONCLUSIVE / `bound.hit` rendered **honestly** (exit-2 shown as
  inconclusive, **never** a false "verified" — the cardinal verification-UI
  sin guard); the deadlock counterexample witness as a navigable list reusing
  the diagram's click→source mechanism; reachability `FSM-E0400`/`FSM-W0602`
  surfaced as a dedicated VS Code `DiagnosticCollection`. Host-tested to
  byte-equality against a real `fsm`-binary differential oracle.
- **Live verification in VS Code (`fsm.verifyLive` — LSP-embed; A2):** a new
  `fsm/verify` LSP custom request — the **single Rust-layer change of the
  epic** (one `fsm-verify = { path = "../fsm-verify" }` edge in
  `crates/fsm-lsp/Cargo.toml`; **no new crate**). The capability is a **pure
  frontend** of `fsm-verify` (the same `analyze` seam + the **identical**
  `fsm_verify::verify`/`reachability_diagnostics` the CLI calls + verbatim
  `VerifyOutcome` marshalling — **zero verification facts computed locally**).
  The VS Code client is **debounced** + **large-FSM-ceiling-guarded** +
  `State.Running`-honest-degrade (an auto-verify-on-every-keystroke of a large
  FSM would hang the editor — the trigger is explicit/debounced; `fsm-verify`
  is bounded-by-construction so an explosive model returns *inconclusive*
  within bound, never a hang).
- **Marketplace-ready VS Code extension (B1):** `editors/vscode/package.json`
  made Marketplace-valid — `version` `0.1.0`→`1.0.0`, `icon` + `keywords`
  added, `repository.url` corrected to the owner repo, a Marketplace-facing
  README + a user-facing CHANGELOG. New `fsmLang.codegen.*` settings exposed
  in `contributes.configuration`. A **lint-clean, installable VSIX** is the
  shipped artifact (publishing is an owner credential action — **not**
  published).
- **`prettier` on the VS Code extension (C2 / C1-F2):** `prettier@3.8.3` +
  `.prettierrc.json` + `.prettierignore` + `format`/`format:check` scripts;
  the one-time mechanical reformat isolated in its own byte-behaviour-identity
  commit.
- **Extension-Host E2E for the live verify UX (C2 / C1-F1+C1-F9):** a real
  headless-host test driving the registered `fsm.verifyLive` command
  end-to-end against the real `fsm`-binary differential oracle — asserting the
  witness list **and the INCONCLUSIVE-not-"verified" honesty guard
  end-to-end** (not a renderer/unit mock); the *hardening* of the A1/A2 verify
  UX (the CellWar "R6 — the only truth" standard in the correct primitive for
  an extension). Extension-Host suite: 41 passing, green via
  `xvfb-run @vscode/test-electron`.

### Fixed

- **The GT-2 wrong-repo-slug (B1):** `editors/vscode/package.json`
  `repository.url` was the **concretely-wrong third-party slug**
  `fsmstudio/sm-sdk` (a published extension would have linked users to a
  stranger's/non-existent repo) — corrected to the owner remote
  `https://github.com/BackendCat/embeded-fsm-sdk-project`.
- **`makeNonce` `Math.random`→CSPRNG nonce (B1; the carried v1.3.x nit):**
  `src/diagram/diagramPanel.ts` `makeNonce()` now uses
  `randomBytes(24).toString("base64url")` (was `Math.random()` under a doc
  comment claiming "cryptographically-unpredictable" — comment/code honesty
  restored on the strict-CSP webview).

### Known limitations

- **D1 (docs/landing site) slipped to v1.6.** The static
  EdgeForge-modeled docs/landing site was the **only droppable-from-tag
  item** (owner pre-decision); its v1.5 agent run was output-content-filtered
  and left uncommitted unreviewed work, which was **deliberately discarded
  (not shipped, not built upon)**. Re-scoped to v1.6, carrying with it the
  LICENSE/CONTRIBUTING/CoC gap and the client-trigger-policy doc-surfacing.
- **The v1.6 FE-hygiene batch (deferred-tracked, not dropped):** the C1 audit
  dispositioned the flat ESLint-9 migration (C1-F3), `madge` import-cycle
  guard (C1-F7), and a `noUncheckedIndexedAccess` strictness delta as a
  coherent **v1.6 "VS Code extension FE-hygiene" batch** — real-but-modest
  value that must not ride the v1.5 Marketplace pass (the leave-and-explain
  discipline; the surface is fundamentally sound).
- **Carried owner-escalations (re-stated, not resolved):** the **G9** push of
  the LOCAL `v1.0.0`–`v1.5.0` tags + checkpoints + commits + the CI matrix /
  SCA / JS lane never run on a remote runner (the v1.5 JS lane is a binding
  *local* gate but remote-unrun); **Marketplace publish** (now publish-*ready*
  via B1, still owner-published); the **5-platform binary tail**
  (blocked-on-G9; no cross toolchains on the box); the parked
  **Certifiability / MISRA–CERT-C** track (owner-GO required); the top-level
  **`LICENSE`/`CONTRIBUTING`/`CoC`** gap (owner/legal, now carried to v1.6
  with D1); the mechanised **sim≡codegen-equivalence** proof (R7 — named
  v1.4-stretch/later deferral, unchanged). The broader README/Doc-07/Doc-14
  doc-honesty staleness pass is a **separate already-identified backlog item**
  (flagged in `GATE_VERIFICATION_v1_5.md` §6.4, not fixed in this minor; only
  the `docs/ROADMAP.md` "See also" stale pointer was reconciled here).
- **G7 / G9 partials** are the same explicitly-tracked posture accepted at
  v1.0–v1.4 (formal conformance = 26 fixtures, not all 73 live codes — the
  v1.5 verify *surface* is behaviourally tested via the Extension-Host suite +
  the LSP differential test; the remote CI matrix has never run). Neither is a
  v1.5 regression.

See `docs/GATE_VERIFICATION_v1_5.md` for the full release gate, the KEYSTONE-IN-UI evidence, the cold-quad + re-activated-JS-lane posture, and the carried owner-escalations.

## [1.4.0] — 2026-05-16

**Theme: Trustable behavioral validation — the verification core (`fsm verify` + `fsm baseline`).**

> **Scope (Doc 00 §11.63, owner-confirmed):** `v1.4.0` ships the **complete
> Verification core** = cut **A** (bounded explicit-state reachability +
> deadlock detection driving the shipped `fsm_simulator::Interpreter` as the
> sole semantic oracle, surfaced as `fsm verify`, honestly closing
> FSM-E0400/FSM-W0602) **+ B** (trace differential replay, `fsm baseline`).
> The verification core is a **NEW single `fsm-verify` crate** that drives the
> shipped Interpreter and **never re-implements FSM semantics** (the keystone,
> an epic-level architecture invariant). The canonical interface is the
> **CLI** — no server, no daemon, no plugin host (the pipeline-before-UI bar,
> `[[feedback_embeded_fsm_pipeline_before_ui]]`). The WS simulator server →
> v1.5; Web IDE / wasm32 / cross-file LSP → v1.6 (explicitly deferred, not
> re-cut). The v1.4 Rust delta vs v1.3 is **substantial and additive** (the
> new `fsm-verify` crate + the W2-P0 lossless-snapshot extension + the
> verify/baseline CLI wiring; `git diff --stat a036c38 4eb04dc -- crates/
> Cargo.toml Cargo.lock rust-toolchain.toml` = 51 files / +7398 / −6) with
> **0 new external dependencies** (Cargo.lock adds only the `fsm-verify`
> workspace-member edge).

### Added

- **`fsm verify` — bounded explicit-state verification** (v1.4-W1/W2): a new
  `fsm-verify` crate + `fsm verify <file>` subcommand. Bounded explicit-state
  reachability + deadlock detection by **driving the shipped
  `fsm_simulator::Interpreter`** as the sole transition oracle
  (`new`/`init`/`dispatch`/`advance_clock`/`snapshot`/`restore`) — zero
  re-implemented transition-selection / guard-eval / timer-fire / LCA /
  completion (the §4.1 keystone). Covers flat **and**
  composite/parallel/history/defer/timer/submachine machines (W2). Visited-set
  keyed by a stable digest of `InterpreterSnapshot` (not retained full
  snapshots — the §3.2 memory-bound design); a sound clock-origin
  normalisation merges only strongly-bisimilar configs (the soundness lemma
  now has its normative home at **Doc 08 §13.5**). **Distinct
  verification-verdict exit codes** (0 verified / 1 property-violated /
  2 INCONCLUSIVE / 3 IO / 4 won't-compile — Doc 18 §3.1; never a false
  "verified" on a bound-hit, the honest-bound discipline) + a deterministic
  machine-readable `--json` schema `fsm-verify/v1` (property results +
  counterexample/witness traces). CI/Make/factory-integratable headless, zero
  daemon. Behaviourally proven against an independent interpreter BFS + witness
  replay through a fresh interpreter (the §5.4 bar; never symbol-presence).
- **`fsm baseline` — trace differential replay** (v1.4-W3): a new
  `fsm baseline --record` / `--check` subcommand — the semantic-drift
  regression oracle. `--record` captures every suite `.fsm`'s execution trace
  (driven through the shipped interpreter) into a frozen `fsm-trace/v1`
  corpus; `--check` re-runs on a later build and reports any divergence with a
  precise first-mismatch report. **Consumes** `fsm_simulator::execute_trace` /
  `first_mismatch` (the same seam the conformance runner + `fsm verify` use —
  no forked step comparison). Shipped as a **dedicated sibling subcommand**
  (not `fsm test --baseline` — `fsm test` already has its own exit-code
  semantics; a single-purpose subcommand is a clean sibling of `fsm verify`).
  Distinct exit codes mirroring `fsm verify`'s family (0 no-drift / 1 drift /
  2 INCONCLUSIVE / 3 IO / 4 out-of-scope) + `fsm-trace-diff/v1` `--json`.
- **FSM-E0400 (unreachable state) + FSM-W0602 (no-incoming, not initial) —
  honestly closed** (v1.4-W1/W2): both codes were **catalogued from v1.0 but
  had no emission site** (the Doc 30 §1.3 / R8 *catalog-reserved-but-
  unimplemented* drift). `fsm verify` now **emits them** from the computed
  reachable set — FSM-E0400 **proof-gated** (emitted only on an exhaustive
  search; the dual of the never-false-`ProvenNoDeadlock` invariant),
  FSM-W0602 the structural always-safe subset (unconditional). The drift is
  **closed in the record** (Doc 10 annotated); the live `DiagnosticCode`
  count is **unchanged at 73** — v1.4 gave existing codes their first
  emission site, it did not add a code.
- **Worked CI-shaped factory recipe** (v1.4-W4a): `examples/verify/`
  (`clean.fsm`, `deadlocks.fsm`, a runnable `README.md` CI script) +
  `docs/25-Integration-Guide.md` §9 — the headless `fsm verify → generate →
  check → baseline` loop demonstrated end-to-end + the determinism harness.
- **`#![forbid(unsafe_code)]` extended to `fsm-verify`** — workspace is now
  **12 forbid roots / 11 crates** (re-derived from source at the gate-doc
  commit; the new `crates/fsm-verify/src/lib.rs` is the 12th root).

### Changed

- **`InterpreterSnapshot` made lossless for `timers` + recursive
  `submachines`** (v1.4-W2-P0, `efae559`): **purely additive** — pre-W2
  fields keep name/type/order; flat snapshots serialise byte-identically
  (`"timers":[]` / `"submachines":{}`), proven by a ~435-line
  `snapshot → perturb → restore → re-snapshot` byte-identity round-trip gate.
  This closed the latent false-`ProvenNoDeadlock` foot-gun the post-W1 §11.3
  audit flagged (the pre-W2 snapshot was lossy for those fields, so a
  hierarchical/timer explorer would conflate behaviourally-distinct configs).
  No regression to the flat-machine behaviour (the W2 §11.3 audit confirmed
  byte-unregressed; subsumed in the cold-quad's `cargo test --workspace`).
- **Doc 08 §13.5 added** — a new normative *absolute-virtual-clock
  non-observability* lemma: no FSM-Lang construct can read the simulator
  virtual clock; only relative timer phase is behaviourally significant ⇒
  clock-origin-shift-equivalent configs are strongly bisimilar ⇒ the
  `fsm verify` digest's clock-origin normalisation is sound (never a false
  `ProvenNoDeadlock`). The normative-spec home for the W2 clock-merge
  soundness. `crates/fsm-verify/src/digest.rs` citations repointed to §13.5
  (comment-only, zero behaviour change).
- **Doc 18 §3.1 added** — the sanctioned per-subcommand
  verification-verdict exit-code family (`fsm verify` / `fsm baseline`
  0/1/2/3/4) + the `fsm-verify/vN` / `fsm-trace/vN` / `fsm-trace-diff/vN`
  schema-versioning policy, surfaced at the spec layer (Doc 18 §3 remains the
  single authoritative exit-code source, now correctly including it). Exit 2
  is documented as the first-class **INCONCLUSIVE** verdict (never a tool
  error to blindly retry — the W4a §3.G surfacing). The policy was already
  self-documented in-source (a positive credit); this is the discoverability
  surfacing, not a code change.
- **Doc 30 reconciled by annotation** (not rewritten — it remains the
  wave-plan of record): the §4.1 "`InterpreterSnapshot` … already the
  complete state" overstatement, the CLI+E0400/W0602-landed-in-W1 scope, the
  two-audit §11.3 cadence, the `fsm baseline` surface choice, and the
  baseline-corpus capture-from-current-known-good provenance are all marked
  with reconciliation-note blockquotes beneath the original prose.

### Fixed

- *(No user-facing behaviour fixes — v1.4 is a new-capability epic.)* The
  one internal correctness gap closed is the **pre-W2 `InterpreterSnapshot`
  lossiness** for `timers`/`submachines` (W2-P0, see *Changed*); it was
  latent (never reachable in v1.3 — there was no explorer that
  snapshot/restore'd hierarchical/timer configs) and is closed *before* the
  v1.4 explorer consumes the seam, so no shipped v1.x behaviour regressed.

### Known limitations

- **The mechanised sim≡codegen-equivalence proof is an explicit deferral
  (R7).** v1.4's "trustable validation" theme is the *runtime* verification
  core + the differential-replay drift oracle; a *mechanised* proof that the
  simulator and the generated C are observationally equivalent is a **named**
  v1.4-stretch / v1.5 item (the §11.49 leave-and-explain precedent applied to
  scope) — recorded as accepted-tracked, **not silently omitted**.
  `docs/GATE_VERIFICATION_v1_4.md` §6, Doc 00 §11.71.
- **FSM-E0400/W0602 are tested by the `fsm-verify` acceptance suites, not a
  formal conformance fixture.** The §5.4 *behavioural*-acceptance is a real
  `.fsm` fixture driven through the parse+analyze+verify pipeline and
  byte-cross-checked against an independent interpreter BFS (never
  symbol-presence). The formal `tests/conformance/MANIFEST.json` corpus is
  byte-unchanged at **26 fixtures**; G7's formal-conformance posture is
  unchanged (the same explicitly-tracked partial accepted at v1.0–v1.3 — not
  a v1.4 regression). `docs/GATE_VERIFICATION_v1_4.md` §1 G7.
- **CI matrix + the JS lane never run** (G9). The GitHub Actions matrix
  (linux/macos/windows × fmt/clippy/build/test), the SCA `cargo audit` job,
  and the v1.3 Node/Extension-Host JS lane have **never executed against any
  v1.2/v1.3/v1.4 commit** (local-only repo — owner controls the remote). The
  local-equivalent cold quad is the gate; platform-specific behaviour is
  unverified on the runners. Post-tag owner push action — carried unchanged
  from v1.3 (`docs/GATE_VERIFICATION_v1_4.md` §5).
- **Multi-platform binary bundling host-only (G9-gated); VSIX
  installable-not-published; no top-level `LICENSE`; the v1.3.x
  `makeNonce` CSPRNG one-liner; JC-3 `fsmLang.codegen.*` config-
  discoverability** — all **carried unchanged from v1.3** (v1.4 neither
  touched nor regressed them; the v1.4 surface is the verification core, no
  `editors/` change). `docs/GATE_VERIFICATION_v1_4.md` §6, Doc 00 §11.71.

## [1.3.0] — 2026-05-16

**Theme: First-class editor experience on top of the shipped LSP — the VS Code extension (Doc 27), preceded by the deferred §11.49 analyzer-coupling paydown (W0).**

> **Scope (Doc 00 §11.40, the v1.2-era user-endorsed re-version):** `v1.3.0` ships the **VS Code extension** — the natural `vscode-languageclient` consumer of the v1.2 LSP server. All language intelligence comes **free** over the client now the LSP shipped; only the ELK diagram Webview is substantive-new. The deferred §11.49 CST-coupling debt is paid as **W0 before V1** (the v1.1→v1.2 "L0-before-LSP" precedent, Doc 28 §4). The extension adds **zero Rust surface** (`git diff ceb8efd fa3befc -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` empty — independently re-derived, the four §11.3 phase-audits each confirmed from `git`).

### Added

- **VS Code extension — V1 MVP spine** (v1.3-V1, `editors/vscode/`): a
  greenfield TypeScript extension wiring `vscode-languageclient@9.0.1` to
  the `fsm-lang-server` binary over **stdio** (constructed as an
  `Executable` `ServerOptions` with the `transport` field **deliberately
  omitted** — the only form that round-trips against the shipped server's
  strict no-args/exit-2-on-unknown-arg parser; robust *by construction*
  against the locked client's `Executable`-vs-`NodeModule` dispatch). Live
  diagnostics reuse the **exact** `fsm check` pipeline (editor squiggles
  can never disagree with `fsm check --json`). Doc 22 §2.2 binary
  resolution Rule 1/2/3 with **no silent PATH/guess fallback** (the
  cardinal-sin bar at the client boundary), and the Doc 22 §13.2
  exponential-backoff (3/9/27 s, 60 s success-reset) crash-recovery
  handler. Proven by 4 real `@vscode/test-electron` Extension-Host tests
  with the diagnostics oracle independently recomputed from `fsm check
  --json` and byte-compared (incl. a non-ASCII multibyte fixture under the
  negotiated UTF-16 encoding), R-15 fixture isolation hard-asserted. See
  `docs/27-VSCode-Extension-Architecture.md` §8 V1, `docs/28-v1_3-VSCode-Wave-Plan.md` §3-V1,
  and `docs/00-Decisions-And-Reconciliation.md` §11.52–§11.54.
- **VS Code extension — V2 TextMate grammar + language-configuration +
  snippets** (v1.3-V2): the Doc 21 grammar (every Doc 21 §2 scope name
  preserved **verbatim**), `language-configuration.json` (brackets,
  comments, auto-closing, folding markers), and FSM-Lang snippets. Static
  assets; 7 grammar-test cases. (The Doc 21 §3 *literal JSON* was
  structurally defective — single-line `match` rules with no body rule, so
  the bare-`{` `#action-block` greedily swallowed the machine body; the
  shipped grammar corrects the **structure** to begin/end block rules
  while keeping the **scope names** Doc-21-verbatim. See §11.55 + the
  Doc 21 §3 reconciliation note.) See `docs/00-Decisions-And-Reconciliation.md` §11.55.
- **VS Code extension — V3 CLI-wrapper + client-control commands**
  (v1.3-V3): the 7 Doc 22 §4 commands (`fsm.checkFile`,
  `fsm.generateC99`, `fsm.generateCpp17`, `fsm.formatDocument`,
  `fsm.copyIR`, `fsm.restartLanguageServer`, `fsm.showOutputChannel`) with
  bare titles + `category:"FSM Studio"` (VS Code renders `FSM Studio:
  <X>`), their menus/keybindings/`commandPalette` gating, and a
  centralised honest `cliRunner.ts`/`cliBinary.ts` process seam
  (resolve-never-reject; a non-zero exit is data, never a masked success;
  no silent PATH fallback). `fsm.showOutputChannel` +
  `fsm.restartLanguageServer` (wired by V1's status-bar + crash-exhaustion
  buttons) are now contributed + registered + `onCommand:`
  activation-evented. 9 command-test cases. See
  `docs/00-Decisions-And-Reconciliation.md` §11.55.
- **VS Code extension — V4 read-only diagram WebviewPanel** (v1.3-V4):
  `fsm.openDiagram` → a CSP-locked (`default-src 'none'`, nonce'd,
  `localResourceRoots`-pinned, `postMessage`-only, no remote) split-right
  read-only Webview. Data = the **one real** codegen-gated `fsm generate
  --emit-ir` → `<machine>.ir.json` path via the V3 seam (**no** new
  LSP/server method, **no** `fsm ir` subcommand — the architecture stays
  pinned to the one real IR producer); elkjs ELK-Layered layout
  in-webview; click a rendered state → editor reveals its declaration via
  the IR `SourceLocation`. The codegen-gated boundary is sealed:
  parse-OK-but-codegen-fails keeps the **last valid render** + shows the
  Doc 05 §1.5.9 banner `⚠ Diagram shows last valid state. Fix parse errors
  to update.` **verbatim**, asserted at the real Webview ack. The shared
  IR-acquisition core was extracted to `emitIr.ts` (one honest `--emit-ir`
  seam consumed by both `copyIR` and the diagram; `copyIR`
  pre/post-identical). 4 diagram-test cases. See
  `docs/00-Decisions-And-Reconciliation.md` §11.56.
- **VS Code extension — V5 activity-bar tree views + context-key chrome**
  (v1.3-V5): `fsm.machineExplorer`/`fsm.eventExplorer`
  `TreeDataProvider`s fed by the **free `documentSymbol`** LSP response
  (the same single-`analyze_with_source` seam — **not** the codegen-gated
  IR), the Doc 22 §7 activity-bar view container, and the Doc 22 §11
  context keys (`fsm.hasOpenFsmFile`/`fsm.serverRunning`) driving
  `when`-clauses. Tree fidelity asserted **exactly** against the client's
  `executeDocumentSymbolProvider` result (re-projected, not re-analyzed —
  never symbol-presence). 6 tree-test cases. See
  `docs/00-Decisions-And-Reconciliation.md` §11.57.
- **VS Code extension — V6 host-only binary bundling + installable VSIX**
  (v1.3-V6): the host-triple `fsm-lang-server` + `fsm` built on the
  pinned 1.75 toolchain into `editors/vscode/bin/<host-triple>/` (the
  producer and V1's never-before-exercised Rule-2 resolver are
  re-derived from the **same** `os.platform()`-`os.arch()` expression so
  they cannot drift), packaged via `@vscode/vsce package` into a lean
  **installable `.vsix`** (excludes `node_modules`/`src`/`out`/`test`).
  V1's bundled-binary Rule-2 path fires against a real bundle for the
  first time; a real diagnostic round-trips through the bundled server
  byte-equal to the bundled `fsm check --json` oracle; the SEC-P0-1
  multibyte-line seam is re-verified through the bundle. 4 V6-test cases
  (full V1–V5 regression green alongside). See
  `docs/00-Decisions-And-Reconciliation.md` §11.57.

### Changed

- **`fsm-analyzer` → `fsm-parser`-CST coupling substantially paid down
  (v1.3-W0, DRIFT-2-grade, non-behavioural, additive-only, 0 new deps).**
  The accepted-tracked-debt deferred at the v1.2 tag (Doc 00 §11.49) is
  resolved: `checks/parallel.rs` fully shed `use fsm_parser::cst::*` (its
  lone `INITIAL_DECL` predicate → the typed `RegionDecl::initials()`); ~7
  new typed accessors on `fsm-parser` `ast_node!` types
  (`PriorityClause::value`, `{Transition,Internal,Local}Decl::payload_binding`,
  `{After,Every,EveryInternal}Decl::{action_block,duration}`,
  `{Machine,Region}Decl::initials`) absorb the B-clean single-construct
  relocations. **13 `fsm-analyzer` files deliberately retain the `cst::`
  import as the documented R-1..R-4 leave-and-explain residual** (R-1
  OPAQUE-BUG-1 parent-node type resolution — P0-1 silent-data-loss risk if
  removed; R-2 heterogeneous source-/document-pre-order dispatch — the
  emitted diagnostic vector is unsorted so traversal order is
  byte-load-bearing; R-3 the deliberately-shallow expr/stmt sublanguage —
  folding relocates not eliminates the walk; R-4 `util::span_of`-family
  positional helpers — pure rowan-positional, zero cross-crate callers).
  Each carries a DRIFT-2-form comment citing Doc 00 §11.44/§11.49. Proven
  non-behavioural by the pre/post-identity gate (full corpus + 35 LSP
  client tests + examples 5/5 + conformance 26/26 byte-unchanged). The
  post-W0 §11.3 phase-audit rated it a success of the leave-and-explain
  discipline. Detail: Doc 00 §11.50, `docs/GATE_VERIFICATION_v1_3.md` §2.1/§6.1.
- **`copyIr.ts` refactored to delegate to the shared `emitIr.ts` core**
  (v1.3-V4, audit-recommended) — one honest `fsm generate --emit-ir`
  acquisition seam consumed by both `copyIR` and the diagram panel;
  `copyIR` behaviour is pre/post-identical (the SUBAGENT §10 refactor
  pre/post-identity bar met). Recorded as a positive consolidation credit,
  not a behaviour change. Detail: Doc 00 §11.56/§11.60.

### Fixed

- **Doc-of-record corrections (no code change — the shipped V1 code was
  already correct; the V1 phase-audit independently re-derived both from
  the locked `vscode-languageclient@9.0.1` source and proved the *prose*
  wrong):** Doc 27 §2.2's `TransportKind.stdio` is a wrong literal — the
  only round-tripping form is to **omit** `transport` on the `Executable`
  (Doc 00 §11.52); UTF-8 `positionEncoding` is **unreachable** through the
  official client (it hardcodes `['utf-16']` and throws on non-UTF-16), so
  the correct negotiated value is `"utf-16"` and the shipped acceptance
  test already asserts exactly that (Doc 00 §11.53). These are
  documentation-of-record fixes folded at the v1.3 closeout; the shipped
  extension's behaviour was correct throughout.

### Known limitations

- **CI matrix + the new JS lane never run** (G9). The GitHub Actions
  matrix (linux/macos/windows × fmt/clippy/build/test) and the SCA `cargo
  audit` job have **never executed against any v1.2 or v1.3 commit**
  (local-only repo — owner controls the remote). v1.3 adds a **second**
  never-run lane: the Node/Extension-Host JS lane (Doc 28 §2.3 Option-A;
  it needs `xvfb-run` for a virtual DISPLAY — strictly load-bearing post
  the V4 Webview test). Local-equivalent cargo quad + JS lane are green;
  platform-specific behaviour is unverified on the runners. Post-tag owner
  push action — `docs/GATE_VERIFICATION_v1_3.md` §5.1.
- **Multi-platform binary bundling is host-only (G9-gated).** V6 ships the
  **host-triple** `fsm-lang-server`+`fsm` bundled in the VSIX (Rule-2
  proven against the real bundle). The full 5-platform `bin/{triple}/`
  tail is **blocked-on-G9 existing** (no cross toolchains on the box;
  infra-escalation, not grind). Accepted-tracked-debt — owner/infra
  action, `docs/GATE_VERIFICATION_v1_3.md` §5.2.
- **The VSIX is installable, not published.** `vsce package` produces a
  lean local `.vsix`; marketplace publishing/signing is explicitly OUT —
  an owner credential/product decision (no PAT sought).
  `docs/GATE_VERIFICATION_v1_3.md` §5.3.
- **No top-level `LICENSE` file.** Generated firmware code carries an SPDX
  header (MIT-by-default, `--license` overridable) and the v1.3 VSIX is
  host-only-unpublished, so this does not block the local tag — but a
  *published* extension / distributed source should carry an explicit
  repository license (owner/legal decision, coupled to the
  publishing-OUT item above). Doc 00 §11.62.
- **`fsmLang.codegen.outputDir`/`codegen.strategy` are not Settings-UI
  discoverable** (JC-3). Both are extension-consumed by the V3 generate
  command with the correct Doc 22 §8 defaults but are not in
  `contributes.configuration.properties` (only via raw `settings.json`).
  Not a correctness bug (defaults match the spec); carried to a later
  config-owner wave. Doc 00 §11.58.
- **`makeNonce()` uses `Math.random()`, not a CSPRNG** (the V4 diagram
  Webview CSP nonce). Independently rated **adequate and
  non-contract-weakening** for a local `vscode-webview://` bundle under
  `default-src 'none'` with no remote/inline-injection vector; carried as
  a tracked v1.3.x senior-bar one-liner hardening, not a ship issue.
  Doc 00 §11.61.
- The status-bar "restarting" tooltip renders a generic `"…starting"`
  string (not the literal `"FSM Language Server restarting (attempt
  N/3)..."`); inside the unexercised recovery path, icon/text already
  match. Carried as a V5-area v1.3.x cosmetic polish (N-4).

## [1.2.0] — 2026-05-16

**Theme: Developer tooling — the `fsm-lsp` Language Server.**

> **v1.2 re-scoping (user-endorsed; `docs/00-Decisions-And-Reconciliation.md` §11.40):** `v1.2.0` ships **the LSP server only**. The VS Code extension moved to **v1.3** (its natural LSP-client home; architecture in `docs/27-VSCode-Extension-Architecture.md`), the old v1.3 "Simulation & verification" theme to **v1.4**, and C++17 codegen (Doc 12) to its **own minor**. This keeps the validated small-tight-tagged cadence (v1.0/v1.1) and the post-v1.1 retrospective's LSP-first feed-forward; a mega-v1.2 would contradict the shippable-increment pattern. Reversible via the ROADMAP's own re-versioning mechanism.

### Added

- **`fsm-lsp` — Language Server (LSP) spine** (v1.2-LSP-L1): a new
  `fsm-lang-server` binary speaking LSP over stdio with `initialize`
  (UTF-8/UTF-16 `positionEncoding` negotiation, UTF-8-preferred),
  full-document sync, a 200ms debounce, and `textDocument/publishDiagnostics`
  that reuses the **exact** `fsm check` analysis pipeline (parser +
  analyzer + import-security) — editor squiggles can never disagree with
  `fsm check --json` (byte-exact Range + code parity proven by an
  in-process `tower-lsp` client test under both encodings, incl. a
  multibyte-line fixture). `#![forbid(unsafe_code)]` (workspace now
  10/10). See `docs/26-LSP-Architecture.md` §8 L1 and
  `docs/00-Decisions-And-Reconciliation.md` §11.32.
- **`fsm-lsp` — `documentSymbol` + `foldingRange`** (v1.2-LSP-L2): the
  Doc 14 §13 hierarchical symbol tree (machine → context/events/externs/
  states with composite/region nesting + `SymbolKind`s, name
  `selectionRange`s) and Doc 14 §12 folding regions, both advertised in
  `initialize`. `documentSymbol` reuses the **same** single `fsm check`
  analysis the diagnostics path runs (the `symbol_table` is now threaded
  through — one analysis feeds both, no second pass / no second position
  converter, diagnostics byte-identical to L1); `foldingRange` is a pure
  parse-tree walk. Proven by an in-process `tower-lsp` client test
  asserting the full tree + every range + folds, byte-equal under both
  UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L2 and
  `docs/00-Decisions-And-Reconciliation.md` §11.33.
- **`fsm-lsp` — `hover` + `definition`** (v1.2-LSP-L3, single-file): Doc
  14 §5 GitHub-Markdown hover (symbol kind + IR-sourced detail — event
  payload field types, `pure` extern signatures, context-field
  type/default, state kind + transitions-out) and Doc 14 §6
  goto-definition, both advertised in `initialize` and built on a shared
  token-at-cursor seam whose resolution mirrors the analyzer's own
  name-resolution dispatch and goes through the **same**
  `SymbolTable::resolve_*` `fsm check` uses (a goto/hover can never
  disagree with a squiggle). Cross-file/unresolved symbols return
  `null`/`None` — the spec-correct single-file graceful degradation
  (cross-file is v1.3), never a fabricated location. Hover's structured
  detail comes from the lowered `Ir`, threaded **additively** through the
  single analysis (the exact behaviour-neutral pattern L2 used for
  `symbol_table` — one analysis feeds all, no second lowering, no second
  position converter; L1 diagnostics + L2 symbols/folds byte-identical).
  Proven by in-process `tower-lsp` client tests asserting definition
  ranges byte-equal to the analysis oracle, hover Markdown structured
  content, the cross-file-`null` case, and correct position mapping under
  **both** UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L3 and
  `docs/00-Decisions-And-Reconciliation.md` §11.34.
- **`fsm-lsp` — context-aware `completion`** (v1.2-LSP-L4, single-file):
  Doc 14 §4 `textDocument/completion` advertised with the exact Doc 14 §2
  trigger characters `[".", ":", "@", "[", " "]`; a CST trigger-context
  classifier that **reuses L3's `resolve` substrate** (the same ancestry
  walk + `in_guard` predicate + a shared `prev_significant_token`
  primitive — not a parallel detector) offers ONLY context-correct
  candidates (transition target → state names; after `on `/`raise`/`defer`
  → events; guard → context fields + `pure` externs; `ctx.` → context
  fields; action → fields + all externs + statement keywords; structural
  starts → Doc 04 §1.5 keywords + Doc 14 §4 snippets), every name sourced
  from the single threaded `symbol_table` (one analysis, no second
  converter) and keywords pinned verbatim to Doc 04 §1.5 by a test.
  Wrong-context candidates are structurally impossible; an unclassifiable
  cursor returns an empty list, never a symbol dump. Proven by in-process
  `tower-lsp` client tests asserting, per context, the served set (labels
  + kinds) equals the analysis oracle AND a wrong-context candidate is
  absent, under **both** UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L4 and
  `docs/00-Decisions-And-Reconciliation.md` §11.35.
- **`fsm-lsp` — `references` + `prepareRename`/`rename`** (v1.2-LSP-L5,
  single-file): Doc 14 §7 `textDocument/references` and Doc 14 §8
  `textDocument/prepareRename` + `textDocument/rename` advertised
  (`rename_provider.prepareProvider = true`). Powered by the new
  `ReferenceIndex` — the **one genuinely-new analysis** of the LSP epic,
  *derived* from the single `fsm check` analysis (a single CST walk over
  its `symbol_table` + parse — no second analysis pass, no parallel
  resolver) and **semantic-only**: a token is a reference iff the SAME L3
  `resolve` classifier (mirroring `checks::name_resolution`, resolving
  through the SAME `SymbolTable::resolve_*` `fsm check` uses) resolves it
  to *exactly that declaration* — never an identifier-string/text match.
  A same-spelled token inside a string literal, a comment, or trivia is
  not a CST `Ident` in a classified position, and one in a different
  scope resolves to a different declaration; both are excluded **by
  construction**, so a `rename`'s `WorkspaceEdit` can never silently
  corrupt the user's source (Doc 26 risk-2, the cardinal silent-data-loss
  risk in its most acute form). `references` honours `includeDeclaration`;
  `prepareRename` hard-rejects (up-front, the LSP contract — never a
  silent allow) machine names (codegen/ABI blast radius, out of v1.2
  scope), `@id`/state-id annotation strings, keywords/contextual keywords,
  non-identifier cursors, and any string/comment/trivia position; `rename`
  additionally rejects an invalid new identifier and an in-scope name
  collision with a clear message and **no edit**. Cross-file
  references/rename remain explicitly v1.3 (Doc 26 §4.6/§9). Still one
  analysis, one position converter, one resolver — reused, not
  duplicated; L1–L4 diagnostics/symbols/hover/goto/completion proven
  byte-unchanged. Proven by the most exhaustive in-process `tower-lsp`
  client safety matrix of any wave — the headline risk-2 test asserts a
  safe rename's edit set is **exactly** the semantic references and that a
  same-spelled string-literal substring, comment word, AND different-scope
  symbol are **NONE of them** in the `WorkspaceEdit` (applying the edits
  leaves the other machine + the comment + the string byte-preserved and
  the buffer still parses clean), plus a six-case `prepareRename`
  negative-rejection matrix and all reference/rename ranges correct under
  **both** UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L5 and
  `docs/00-Decisions-And-Reconciliation.md` §11.36.
- **`fsm-lsp` — `semanticTokens` (`full` + `range`)** (v1.2-LSP-L6,
  single-file): Doc 14 §10 `textDocument/semanticTokens/full` and
  `textDocument/semanticTokens/range` advertised in `initialize` with the
  Doc 14 §2/§10 `legend` (11 token types + 4 modifiers) declared **once**
  and reused for both the advertisement and the encoder, so the advertised
  indices and the encoded `tokenType`/`tokenModifiers` can never diverge.
  Semantic tokens are *more precise* than the Doc 21 TextMate grammar: a
  bare `Ident` is one TextMate scope everywhere, but this layer knows —
  from the **same** single `fsm check` analysis the LSP already runs —
  whether it is a state / event / extern / context field / machine and
  whether it is a *declaration* or a *reference*. The decl-vs-ref +
  entity-type split **reuses** L3's `resolve` classifier (use sites) and
  L5's `ReferenceIndex` declaration-name discovery / `SymbolKey` taxonomy
  (declaration sites) through one shared entity model — **no new analysis,
  no parallel classifier** (Doc 26 §8 L6); a declaration and a use of the
  same symbol get the identical legend type and differ only by the
  `declaration` modifier. Non-`Ident` tokens get their lexical `fsm-lexer`
  `SyntaxKind` type (keyword / operator / number / string / comment / `@id`
  decorator); structural punctuation Doc 14 §10 has no legend slot for is
  not emitted (the client uses the Doc 21 TextMate scope — Doc 21 §6
  coexistence). Emitted as the LSP relative delta array
  (`[deltaLine, deltaStartChar, length, tokenType, tokenModifiers]`,
  sorted, deltas relative to the previous token, `deltaStartChar` reset on
  a new line) with `deltaStartChar`/`length` in the negotiated
  `positionEncoding` via L1's one authoritative `LineIndex` (no second
  converter); a multi-line comment is split into one token per line (LSP
  `multilineTokenSupport` defaults off), and `range` is a self-contained
  substream (its first token's deltas relative to the response start).
  Cross-file remains explicitly v1.3 (Doc 26 §4.6/§9). Still one analysis,
  one position converter, one classifier — reused, not duplicated; L1–L5
  diagnostics/symbols/hover/goto/completion/references/rename proven
  byte-unchanged. Proven by in-process `tower-lsp` client tests that
  **decode the relative delta array back to absolute positions** and assert
  the full decoded stream equals the reused-pipeline oracle (symbol
  presence is NOT acceptance — the decoded bytes are), with hard-coded
  per-token cross-checks (a state declaration carries the `declaration`
  modifier, a state use does not, a `ctx.field` ref is the field type, a
  comment is `comment`, a keyword is `keyword`, the `@id` decorator + its
  string), the advertised legend asserted equal to Doc 14 §2/§10 order and
  to the encoder's, and a non-ASCII fixture under **both** UTF-8 and UTF-16
  with the decoded positions/lengths asserted correct in each. See
  `docs/26-LSP-Architecture.md` §8 L6 and
  `docs/00-Decisions-And-Reconciliation.md` §11.37.
- **`fsm-lsp` — `codeAction` + `inlayHint`** (v1.2-LSP-L7, single-file —
  the **final** LSP capability of the v1.2 epic): Doc 14 §9
  `textDocument/codeAction` and Doc 14 §11 `textDocument/inlayHint`
  advertised in `initialize` (`codeActionProvider.codeActionKinds =
  ["quickfix","refactor"]`, `inlayHintProvider`). `codeAction` is
  **edit-producing**, so it inherits L5's risk-2 silent-corruption
  discipline: a `quickfix` is offered **only** when the fix is provably
  mechanical — **FSM-E0107** (insert `initial <FirstState>` after the
  machine's `{`, the first state taken from the same parse the diagnostic
  came from) and **FSM-E0022** (delete the duplicate event declaration,
  whose span the analyzer already pins exactly; **withheld** if the
  duplicate carries its own `@id` annotation, where the deletion would
  stop being mechanical). The other five Doc 14 §9 codes
  (`FSM-E0100`/`E0106`/`W0200`/`W0500`/`E0300`) **and both
  `refactor.extract` actions** are deliberately **scoped out and flagged**
  (Doc 00 §11.38): `W0200`/`W0500` are never emitted by the toolchain at
  all, and `E0100`/`E0106`/`E0300`/extract cannot be produced without a
  heuristic or a forbidden parallel re-analysis — a missing quick-fix is a
  minor UX gap, a wrong edit is the cardinal sin. The edit is matched on
  the server-authoritative diagnostic code (not the client-supplied
  context), tied back to the diagnostic it resolves, and every range goes
  through the one `LineIndex` (no string munging). `inlayHint` is
  **read-only display** sourced from the lowered `Ir` of the **same**
  single `fsm check` analysis — the three Doc 26 §5 families
  (non-default transition priority `// priority: 50`, human timer
  durations `// 1.5 s` / `// 1 min 30 s`, composite/parallel substate
  count `// 5 substates`), each gated by its Doc 22 §8 toggle plus the
  master `enableInlayHints`, read from `initialize`
  `initializationOptions` and live `workspace/didChangeConfiguration`
  (only the behaviour-gating inlay keys are consumed; every other
  `fsmLang.*` key is stub-accepted, never a panic). Doc 14 §11's
  extern-parameter-name row is scoped out (no Doc 22 §8 toggle, not in Doc
  26 §5's IR-sourced set) and flagged. Still one analysis, one position
  converter — reused, not duplicated; L1–L6
  diagnostics/symbols/folding/hover/goto/completion/references/rename/
  semantic-tokens proven byte-unchanged (785 workspace tests, the `fsm`
  examples 5/5 and conformance 25/25 suites green). Proven by in-process
  `tower-lsp` client tests applying L5's apply-and-verify rigor to the
  edit-producer — the headline test requests the FSM-E0107 quick-fix,
  asserts the `WorkspaceEdit` byte-equals the reused-pipeline oracle,
  **applies it**, and asserts the diagnostic is resolved, no new
  diagnostic appears, every other byte is unchanged, and the buffer still
  parses; plus a no-bogus-action negative (a scoped-out diagnostic yields
  no action), an inlayHint oracle with hard-coded Doc 14 §11 family
  cross-checks and a `didChangeConfiguration` toggle round-trip, and a
  non-ASCII fixture under **both** UTF-8 and UTF-16 with the codeAction
  edit range and the inlayHint positions asserted correct in each. After
  this wave the v1.2 LSP capability set is **feature-complete**. See
  `docs/26-LSP-Architecture.md` §8 L7 and
  `docs/00-Decisions-And-Reconciliation.md` §11.38.
- **`FSM-W0200` ("loop in action block") is now emitted** (FU-DEAD-CODES):
  the code was catalogued but had **zero emission sites** anywhere in the
  toolchain (a documented diagnostic that could never fire — surfaced
  writing the L7 `codeAction`). Four normative docs mandate it (Doc 02
  §9.2, Doc 04 §8.7.2, Doc 11 §18, Doc 10's W0200 catalog entry), so a
  single emission site was added (`fsm-analyzer` `checks/action_lint.rs`:
  one `FSM-W0200` per `while`/`for` loop whose ancestor chain contains an
  `ACTION_BLOCK`) — a style nudge, not an error; loops remain permitted.
  Formal conformance fixture `tests/conformance/semantic/neg/004_loop_in_action/`
  added (suite 25→26). See
  `docs/00-Decisions-And-Reconciliation.md` §11.47.

### Removed

- **`FSM-W0500` ("extern declared but never used") retired** (FU-DEAD-CODES):
  it was vestigial — no normative spec mandates it and it was **emitted
  nowhere**. Moved to `fsm_diagnostics::deprecated::DeprecatedCode::W0500`;
  it is **not a behavioural break** — it still parses in suppression
  annotations and `fsm check --allow FSM-W0500` / `fsm.toml [compiler]
  allow` (Doc 10 §14 rule 2 — the FU#67 allow/deny contract is preserved,
  test-pinned). The live `DiagnosticCode` count moves **74 → 73** (the
  `all_codes_matches_expected_count` lock test). See
  `docs/00-Decisions-And-Reconciliation.md` §11.47.

### Security

- **RUSTSEC-2026-0009 eliminated** (SEC-FU; `time` ≤0.3.36 DoS via stack
  exhaustion, CVSS 6.8) — found by a pre-tag SCA scan (itself a
  *self-dropped* recommendation from the v1.0 `AUDIT_2026_05_14` pass, now
  operationalised; see *Known limitations* and Doc 00 §11.48). `time` was
  transitive-only via `jsonschema 0.17 ← fsm-ir` (the W0 IR-schema gate).
  The advisory's literal remedy (`time ≥0.3.47`) was infeasible — every
  such `time` needs rustc ≥1.88 / Cargo `edition2024`, which the
  deliberate load-bearing **1.75** toolchain pin cannot parse. Fixed at
  the **root** by bumping `jsonschema` 0.17 → 0.22 in
  `crates/fsm-ir/Cargo.toml` (0.22.0 dropped the `time` edge outright;
  MSRV 1.70 still satisfies the 1.75 pin; the deprecated `JSONSchema::compile`
  shim migrated to `jsonschema::options()/.build()/Validator`). W0
  schema-gate behaviour byte-identical; re-scan reports **0
  vulnerabilities**; the 1.75 pin is unchanged (the stable toolchain is
  additive — it builds `cargo-audit` only). See
  `docs/00-Decisions-And-Reconciliation.md` §11.42.

### Fixed

- **`fsm.toml [compiler] allow/deny` was parsed but never applied** (FU#67,
  mild P0-1 silent-no-op class — a documented option that did nothing):
  `CompilerSection.{allow,deny}` are now finalized in `cmd::check` and
  projected post-analysis by `diagnostics::apply_allow_deny` (allow →
  suppressed/exit 0; deny → error/exit 1; allow+deny → allow wins; an
  unknown code → loud exit-4; a retired code → accepted; a control code →
  unchanged), matched on the parsed `DiagnosticCode` (never a substring
  match on source). +13 tests. Doc 18 §6/§6.1. See
  `docs/00-Decisions-And-Reconciliation.md` §11.41.

### Changed

- Internal API hygiene: 170 accidentally-`pub` items across `fsm-parser`,
  `fsm-formatter`, and `fsm-cli` `src/` (in private modules / the binary
  crate) narrowed to `pub(crate)`; no public API or behaviour change
  (673/0 tests, 5/5 examples, 25/25 conformance, IR fingerprints all
  unchanged). The `unreachable_pub` lint wiring is deferred — residual
  warnings are confined to shared integration-test helpers, not `src/`
  (see `docs/00-Decisions-And-Reconciliation.md` §11.31).
- **`unreachable_pub` is now wired workspace-wide and self-enforcing**
  (FU#68): §11.31's manual sweep narrowed the 170 over-`pub` `src/` items
  but deferred the lint. `[workspace.lints.rust] unreachable_pub = "warn"`
  is now set with every crate opting in (`[lints] workspace = true`) and
  the shared per-test-binary helpers explicitly `#![allow(unreachable_pub)]`'d
  (the legitimate test-only idiom); the 14 residuals it surfaced (the
  `fsm-lsp` `semantic_tokens.rs` legend/modifier consts) were downgraded
  `pub → pub(crate)` (no external consumer, behaviour-inert). Combined with
  the quad's `clippy -D warnings`, accidental `src/` over-`pub` now fails
  the gate instead of silently re-accreting. See
  `docs/00-Decisions-And-Reconciliation.md` §11.43.
- **The two divergent byte→line/col implementations converged** (DRIFT-2):
  `fsm_analyzer::util::compute_line_col` (bytes, 1-based) and
  `fsm_cli::cmd::check::line_col` (Unicode scalars, 1-based) now both
  delegate to ONE
  `fsm_diagnostics::compute_line_col(src, pos, LineColUnit::{Byte|Scalar})`
  core (each behaviour-identical to its old self — proven byte-for-byte on
  all three contracts incl. multibyte, plus a cold-verified quad). The
  LSP's `position.rs::LineIndex` is a structurally different algorithm
  (precomputed line-start table, 0-based, encoding-negotiated) and is
  **deliberately left-and-explained, not folded in** (forcing it into the
  linear-scan core would worsen clarity for zero behaviour gain — the new
  `SUBAGENT_CONVENTIONS` §10 refactor-to-number anti-pattern, which
  DRIFT-2 codifies by practising). No new deps. See
  `docs/00-Decisions-And-Reconciliation.md` §11.44.
- Docs: `docs/20-Architecture-Overview.md` reconciled to shipped reality
  (DRIFT-1/-3/-4/-5; annotate-not-delete, design intent labelled
  historical, every claim cited to `file:line`). §4.5 / §12.2 advertised
  an incremental-CST-reparse pipeline and a `parse_incremental(old_tree,
  edit)` API that **do not exist** — `fsm-parser` does a **full** re-parse
  only (`parse`/`parse_with_limits`/`parse_with_tokens`); these (plus §4.1
  + ADR-004 and the §12.3 `analyze_incremental` analogue) are annotated as
  a **v1.3+ deferred optimisation, not implemented**, design intent
  preserved as the future target (DRIFT-1 §11.39, DRIFT-3 §11.45). §4.2
  module layout / §4.3 `ParseResult` and §5.2 analyzer layout / §5.3
  `AnalysisResult`/`SymbolTable`/`Scope` types reconciled — the §5.2
  listing's two-`phase{1,2}_*` directory tree, `ir_builder.rs`, `tests.rs`
  and `DiagnosticAccumulator` do not exist; the shipped analyzer is flat
  (`symbol_table.rs` + a `checks/` family + the AD-3 `lower/` tree;
  diagnostics are a plain `Vec<Diagnostic>`), the two-phase split kept as
  the *conceptual* model (DRIFT-3 §11.45, DRIFT-4 §11.46). §5.4/§5.5's
  strict-2-phase narrative + stale per-step code claims reconciled the
  same way (DRIFT-5): the real pipeline is `SymbolTable::build` →
  `checks::run_all` → `lower_file`; **`FSM-E0301` is not emitted** (Doc 00
  B-07 explicitly allows guarded completions — `checks/completion.rs`) and
  the §5.5 forward-reachability/`FSM-E0400` step is **not implemented**
  (catalogued, zero emission sites, `COVERAGE_MAP` `UNTESTED`). The v1.2
  LSP epic deliberately built on full re-parse-on-debounce (Doc 26
  §4.3–§4.5). No code change. See
  `docs/00-Decisions-And-Reconciliation.md` §11.39 / §11.45 / §11.46.
- **`docs/processes/SUBAGENT_CONVENTIONS.md` §10** gains the
  *refactor-to-number anti-pattern* row (the one design take-away of the
  external-kit evaluation, Doc 00 §11.48): do not split/merge code purely
  to hit a metric if it worsens readability/clarity; if no clean
  behaviour-safe restructuring exists, leave the violation and **explain
  why** — exactly what DRIFT-2's left-and-explained LSP `LineIndex`
  practised.
- **`.github/workflows/ci.yml`** gains an SCA (software-composition-analysis)
  job running `cargo audit` on a **separate recent stable toolchain**
  (`rustup toolchain install stable` → `cargo +stable install cargo-audit
  --locked` → `cargo audit`) — the 1.75-pinned `cargo-audit 0.21.1`
  cannot parse the modern CVSS-4.0 advisory DB, so it is a distinct job
  that does **not** gate the 1.75 build matrix. Operationalises the
  self-dropped v1.0 audit recommendation that caught RUSTSEC-2026-0009
  pre-tag (Doc 00 §11.48 / §11.42).

### Known limitations (v1.2) — documented, diagnosed, tracked / accepted debt

- **`analyzer → parser-CST coupling` (~15 files) is deferred — accepted,
  tracked debt.** The architecture audit rated it explicitly
  *ship-acceptable, non-behavioural* P1. A ~15-file refactor of the most
  behaviourally-critical crate (`fsm-analyzer`) **at a release boundary**
  is worse-EV than doing it cleanly post-tag (a tracked v1.2.1/early-v1.3
  wave). **v1.1.0 shipped the same arch-debt class tracked + documented in
  its `GATE_VERIFICATION_v1_1.md`** — deferring here is precedent-consistent
  and lower-risk; it will be recorded in `GATE_VERIFICATION_v1_2.md` as
  accepted-tracked-debt. Doc 00 §11.49.
- **Live diagnostic-code count is 73** (not 75 — `FSM-E0903` retired in
  v1.1 when the `defer` runtime shipped, `FSM-W0500` retired in v1.2 as
  vestigial; each −1). The frozen `GATE_VERIFICATION_v1_0.md` /
  `_v1_1.md` correctly cite "75" / "36/75" *at their pinned commits* (a
  frozen attestation is not rewritten); `GATE_VERIFICATION_v1_2.md`
  (authored at tag-time) will cite 73. Doc 00 §11.47.
- **Conformance coverage:** 26/73 codes have formal `tests/conformance/`
  fixtures (W0200 added one this release); the rest are exercised by
  crate-level negative tests. No behavioural gap; formal-suite closure
  remains tracked.
- **CI matrix not yet exercised:** `.github/workflows/ci.yml` (now incl.
  the SCA job) is configured but has not run against v1.2 commits (the
  repo is unpushed by design — the user controls the remote). Pushing the
  tag to exercise the matrix is the documented post-tag action.
- **v1.2 re-scope:** the VS Code extension is **v1.3** (Doc 27), not part
  of `v1.2.0`; C++17 codegen is its own minor; the old v1.3 "Simulation &
  verification" theme is **v1.4**. Doc 00 §11.40.

## [1.1.0] — 2026-05-15

### Corrected — v1.0.0 scope statement (audit integrity)

The v1.0.0 entry below described scope as "full UML statechart semantics." This was **overstated**: **submachine support is entirely absent** across the pipeline (grammar has no `submachine`/`state … is X` production; `KwIs`/`KwSubmachine` tokens are orphans; `lower.rs` hard-codes `submachines: Vec::new()`; IR `SubmachineRef` is only built by a unit-test fixture; codegen/simulator arms are defensive-only for a never-produced variant). Earlier ROADMAP/backlog claims that "IR + analyzer already lower submachine references" were aspirational prose never reconciled against code — the same class as the P0-1 finding. The three shipped examples (motor, traffic-light, vending-machine) do not use submachines, so `GATE_VERIFICATION_v1_0.md` was accurate for what it tested; the defect was the *scope wording*, not the gate evidence. Correct reading of v1.0.0: **UML statecharts excluding submachines**.

**Resolution (v1.1):** the submachine epic landed across four waves — W2a parser/CST/AST (`7a69612`), W2b analyzer/IR lowering + FSM-E0103/E0500/E0501/E0502/E0610 (`2384608`), W2c simulator runtime semantics Doc 08 §12 (`8b66dc2`), W2d C99 codegen both dispatch strategies (`af8c300`). Submachine is implemented **end-to-end for top-level `state X is Sub`** — parser → analyzer/IR → simulator → codegen, `gcc -std=c99 -Werror`-verified (both strategies) and sim≡codegen behaviour-matched, exercised by `examples/submachine/`. A submachine reference **nested inside a composite or parallel state** is **rejected at analysis with `FSM-E0502`** and a clear actionable message ("only supported on a top-level state in v1.1; … move the `state … is X` to the machine's top level"); the lowerer additionally refuses it (defence-in-depth) — so codegen never receives it and **no broken C is emitted** (the clean-reject-over-broken-output / retired-E0903 precedent, G1 no-undefined-behaviour). Implemented by P1-2 (`02d4ded`); test-pinned by `crates/fsm-analyzer/tests/nested_submachine_rejected.rs`. Proper nested *support* is the tracked **SUB-FU-2** follow-up (→ v1.1.x). Accurate v1.1 scope: **full UML for top-level `state X is Sub`; a nested submachine reference is a documented, diagnosed limitation (FSM-E0502)** — NOT a blanket "full-UML coverage" claim. (History/lesson: this statement was over- then mis-corrected several times — "inert leaf" then "emits broken C / fix in progress"; both were stale. The shipped reality is the clean E0502 reject above. Recorded so the lesson — verify the *current* code, conservative wording, applies to audit claims too — is not lost: Doc 00 §11.29.)

### Added
- **Submachine support end-to-end** (W2a-d) — top-level `state X is Sub` references (parser→analyzer/IR→simulator→codegen, both strategies, gcc -Werror + sim≡codegen); a submachine ref nested in a composite/parallel state is rejected with `FSM-E0502` (proper nested support tracked SUB-FU-2 → v1.1.x).
- `defer EVENT` runtime (removes the v1.0 FSM-E0903 limitation) — bounded buffer + FIFO replay on state exit; sim≡codegen verified.
- IR-schema-validation gate (debug-mode, zero release cost) — malformed IR caught at the analyzer boundary; proven load-bearing.
- Shared `CARGO_TARGET_DIR` + warm-cache build policy (wave-speed + disk-safety).
- TD-BUG-1 + 3 sibling table-strategy degenerate-input codegen bugs fixed (zero-transition machine now gcc -Werror-clean, both strategies).
- Architecture-debt paydown: unified LCA (one generic `ParentResolver`), `LoweringCtx` god-object eliminated, `fsm-simulator` dead-dep removed.
- `likely` / `rare` transition annotations → portable `__builtin_expect` codegen (`<PFX>_LIKELY/_UNLIKELY` macros, GNU/clang builtin + standard-C fallback + opt-out), both dispatch strategies, gcc -Werror -pedantic clean. Contextual keyword — lexer unchanged, fully back-compatible.
- `fsm generate --import-header <path.h>` — auto-`extern` declarations harvested from an existing C header, removing hand-written extern boilerplate for users with established C codebases.
- `examples/integration/{make,cmake,cargo-rust,platformio}/` — four worked end-to-end integrations, each genuinely building the generated C under `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (make/cmake/cargo-rust run+assert the full lifecycle; platformio ships the real `pio` recipe + a verified host-gcc fallback). Plus `docs/25-Integration-Guide.md` (ABI contract, C/C++/Rust recipes, build-system fragments, troubleshooting matrix).
- **Per-machine dispatch strategy override** (W7) — `fsm.toml [machine.<Name>] strategy = "switch"|"table"|"auto"` selects dispatch per machine, enabling MIXED dispatch in one project (machine A switch, machine B table from a single `fsm generate`). Effective precedence per machine: `[machine.M].strategy` > CLI `--strategy` > default `auto`. Invalid value = clean exit-4 config error naming the machine; a `[machine.X]` for a machine not generated this run is a WARN (project-wide fsm.toml generating a subset stays valid). Behaviourally accepted: both strategies gcc -Werror compiled + run with per-machine lifecycle assertions.

### Fixed
- **`--import-header` / `fsm.toml import_headers` bypassed the v1.0 G-02 file-read hardening** (**security, SEC-P0-1**; the one remaining v1.1 tag-blocker, found by the pre-tag security audit): the W5 header-import surface returned absolute paths verbatim, joined relative paths with **no shape-check / no `canonicalize` / no workspace-root containment**, and read the header with **no size cap** — so a crafted `fsm.toml` on shared CI could path-traverse to an arbitrary file (`import_headers = ["../../../etc/shadow"]`) and `--import-header /dev/zero` (or a multi-GB file) could OOM the host before the parser's 1 MiB cap ran. The DSL `import "..."` path was already correctly hardened; the root cause was a **divergent parallel code path**. **Fixed by CONVERGING, not duplicating:** the attacker-controlled `fsm.toml import_headers` surface is routed through `fsm_parser::import_resolver::resolve_import` *directly* — the exact same primitive (shape-reject `..`/NUL/absolute → `canonicalize` (symlink-resolved) → workspace-root prefix) the DSL path uses; **no third copy of the containment logic exists**. A small shared `fsm-cli/src/safe_io.rs` owns the ONE bounded-read helper (capped at the same `ParseLimits::DEFAULT.max_input_bytes` the parser enforces on `.fsm` source, so the caps cannot drift) and the ONE `workspace_root_for` resolver (moved out of `cmd::check` so `fsm check` and `fsm generate` share a single definition). **Deliberate absolute-path / trust decision:** `fsm.toml` travels with the (possibly hostile) repo → contained by default; `--import-header` is invocation-supplied at the same trust level as the `.fsm` path argument and the G-02 threat model scopes the danger to a *source/config posted to a shared host*, not the CI job's own argv — and an absolute vendored-HAL path is the documented normal use of the flag (hard-rejecting it would be both wrong for the threat model and a functional regression) → the CLI flag stays absolute-capable with NUL-shape-reject + the **universal DoS size cap still applied**; the genuine vendored-HAL-via-config case is served by an **explicit, named, default-OFF** opt-in `[generate] allow_unscoped_import_headers = true` (never a silent allow; the DoS cap is non-negotiable regardless). The same size cap is also applied to the `fsm.toml` read itself (REL-P2-1, folded in per the audit — same defect class, same shared helper): an unsized stream (`/dev/zero`, FIFO) is `take`-bounded so it rejects instead of OOMing. Oversized header → clean exit-3, oversized `fsm.toml` → clean exit-4, containment escape → exit-1 (each the established mapping; never an OOM/panic). §5.4 behaviourally accepted: a new `import_header_security.rs` (mirroring `fsm-parser/tests/import_security.rs`) proves traversal / absolute-from-config / **symlink-escape** / NUL / >1 MiB / `/dev/zero` / oversized-`fsm.toml` are each rejected with the asserted exit code and the target is provably never read / never OOMs; the pre-existing W5 end-to-end gcc -Werror + RUN acceptance (both dispatch strategies) **still passes unchanged** (the hardening did not break legitimate use). Doc 00 §11.28.
- **Guard-disambiguated same-event transitions mis-lowered by BOTH dispatch strategies** (W7-FU-1, pre-existing core dispatch defect, P0-1 silent-data-loss class; surfaced by W7's §5.4 behavioural-acceptance discipline, NOT introduced by W7): a state with two or more transitions on the same event disambiguated by guards (`on E [g1] -> A` / `on E [g2] -> B`, incl. an unguarded `on E -> C` fallback — bread-and-butter UML the DSL/analyzer/IR accept and preserve) was independently mis-lowered: the **switch** strategy emitted one C `case <EVENT>:` per transition → `gcc` hard error `duplicate case value` (even without `-Werror`), the 2nd+ guarded transition unreachable; the **table** strategy's `select_for_region` returned the first `(source,trigger)` row ignoring its guard, the executor then silently no-op'd if the guard was false → the event was dropped and the later eligible transition never fired. Both now honour Doc 08 §4.1/§4.2: the first guard-enabled candidate in (priority, document-order) wins, an unguarded/`[else]` transition is the always-enabled catch-all, no enabled candidate ⇒ the event bubbles up (no consume). The simulator was independently verified spec-correct and is the oracle; the fix is codegen-only and makes both strategies match it. Behaviourally accepted on BOTH strategies (gcc -Werror compiled + RUN-asserted: priority-tiebreak when >1 guard is simultaneously true, unguarded fallback, no-match discard, sim==codegen). The honest tripwire was converted to a positive correctness test; conformance CGEN-004 added.
- **Default transition priority lowered as `0`, contradicting the spec's `100`** (W7-FU-2, pre-existing doc-vs-impl divergence; surfaced by W7-FU-1's §5.4 discipline, NOT introduced by it): a transition with **no `priority` clause** was lowered by the analyzer's four `lower_*` arms with `.unwrap_or(0)` → IR priority `0`. **Doc 04 §8.6** ("Lower number = higher priority. Default: `100`.") and **Doc 09 §6** ("default 100", canonical example `"priority": 100`) both specify **100**; the IR `model.rs` field comment already said "Default 100"; no corpus source intends `0`. **Decision: default = `100`** (the impl was the bug, not the docs). Rationale (Doc 08 §4.2 min-wins on `(priority, document_order)` — lower number = higher priority): an unprioritized transition must sit at LOW priority so assigning a small explicit number FLOATS a specific/guarded transition above the default herd; a `0` default made an unprioritized transition out-prioritise any explicitly-deprioritised one and no transition could ever be placed below the default — the priority feature was half-useless. The discrepancy was applied IDENTICALLY by the simulator and both codegen strategies (each only READS the IR `priority`), so sim≡codegen and W7-FU-1's *relative* ordering held regardless — hence P1 (consistent, well-defined) not P0 (silent miscompile). Reconciled to a single source of truth: `fsm_ir::DEFAULT_TRANSITION_PRIORITY = 100`, referenced by the four `lower_*` arms and a serde `default`; the wrong `lower/mod.rs` doc-comment and the IR JSON-schema `priority` annotation were aligned (the W0 IR-schema gate stays green — `priority` is `required`/always-serialized so a JSON-Schema `default` annotation does not affect validation). Behaviourally accepted per §5.4 on BOTH strategies (a hand-built IR — the analyzer's FSM-E0300 forbids a clause-less transition in same-source same-event runtime competition, so the default's *observable* effect is only demonstrable at the codegen/sim contract layer below the compile-time gate: gcc -Werror compiled + RUN-asserted the spec-correct min-wins transition fired, sim≡codegen). The pre-existing `ir_default_priority_is_zero` unit test was pinning the *bug* — corrected to assert `100` (the FAILS-on-old / PASSES-on-new regression anchor + §5.4 unit pin). No shipped example's behaviour changed (no example `.fsm` uses `priority`; the W7-FU-1 fixture/CGEN-004 uses explicit `priority 1/2/3`; FSM-E0300/W0300 keys off `priority.is_some()`, not the materialized default). Doc 00 §11.27.
- **Opaque extern params silently dropped** (OPAQUE-BUG-1, pre-existing core defect, P0-1 silent-data-loss class): `extern` declarations using the documented `opaque "C_type"` form for params/returns were silently lost in lowering (`lower_type_ref` returned `None` for `OPAQUE_TYPE_REF`). Now modelled as `Type::Opaque { c_type }` end-to-end (context field, extern param, extern return, `as`-cast) — gcc-RUN verified. Unblocks passing C handles/pointers/structs to HAL functions from FSM actions.
- **Parser panic on a comment before `language fsm`** (PARSE-BUG-1, pre-existing G1-robustness defect): a `//` license header atop a source file (an extremely common real-world input) tripped the rowan single-root assertion. Leading trivia is now flushed after the FILE node opens — the toolchain parses/diagnoses instead of panicking.

### Known limitations (v1.1) — documented, diagnosed, tracked

- **Submachine nested in a composite/parallel state** is rejected at analysis with `FSM-E0502` + an actionable message (move it to the machine's top level); proper nested *support* is tracked as **SUB-FU-2** (→ v1.1.x). Top-level `state X is Sub` is fully supported.
- **Diagnostic conformance coverage:** 36/75 codes have formal `tests/conformance/` fixtures; the remaining 39 are exercised by crate-level negative tests (not the formal harness). No behavioural gap — closure to 75/75 in the formal suite is tracked for v1.1.x.
- **CI matrix not yet exercised:** `.github/workflows/ci.yml` (linux/macos/windows × fmt/clippy/build/test) is configured; the local-equivalent quad is green but the matrix has not run against v1.1 commits (the repo is unpushed by design — the user controls the remote). Pushing the tag to exercise CI is the documented post-tag action; SEC-P0-1's path-canonicalization is the most platform-divergent surface.
- **Tracked architecture debt → gate before v1.2** (non-behavioural, ship-acceptable per the pre-tag audit): `pub` over-exposure (no `unreachable_pub`/`missing_docs` lint; ~765 items) and analyzer→parser-CST coupling (~15 files reaching into CST rather than typed AST).

### Audit trail (v1.1 pre-tag) — frozen evidence

3-lens read-only audit at `docs/AUDIT_PRE_TAG_v1_1_{CORRECTNESS,ARCH,RELIABILITY}_2026-05-15.md`:
- **Architecture** — 0 P0; AD-1/AD-2/AD-3 debt-paydown each verified *true in source* (one `ParentResolver` LCA; `fsm-simulator` analyzer-dep demoted to dev-dep; no `LoweringCtx` god-object); 9/9 crates `#![forbid(unsafe_code)]`.
- **Correctness** — 0 P0; all Doc 00 §11.19–§11.28 rows: commit-exists + code-present + **0 overstated** (the P0-1/submachine prose-vs-code class is absent from the v1.1 surface).
- **Reliability/Security** — test-integrity strong (every sampled §5.4 acceptance test is a real gcc-compile-and-RUN, not symbol-presence); found **SEC-P0-1** (the one tag-blocker — fixed, see *Fixed* above). That report's **REL-P1-1 was stale vs current code** (it traced the older `AUDIT_PHASE_SUBMACHINE` pre-P1-2 state and claimed nested-submachine "emits broken C"); the nested-reject (`FSM-E0502`) had in fact already shipped in P1-2 `02d4ded` and is test-pinned — the release record was corrected to the shipped reality, **not** regressed to the audit's stale claim (verify-vs-code applies to audit findings too). Doc 00 §11.29.

## [1.0.0] — 2026-05-14

### Added
- DSL: FSM-Lang grammar (Doc 04) — hierarchical UML statechart support.
- Compiler pipeline: lexer + parser (Rowan CST + AST) + analyzer (semantic
  checks + AST→IR lowering) + IR (with JSON Schema).
- C99 code generator with two dispatch strategies
  (`--strategy {switch,table,auto}`).
- In-process simulator (RTC interpreter, virtual clock, deterministic JSON
  traces in `StepRecord` format).
- Canonical formatter (`fsm fmt` — idempotent).
- CLI: `fsm check / generate / fmt / parse / test / doc / decompile / init`.
- HAL contract (`fsm_hal_clock_now_ms`, `fsm_hal_assert`) — mandatory for
  codegen.
- 3 worked examples (motor, traffic-light, vending-machine) — all pass
  `check → generate → gcc -Werror → fmt --check → simulator-trace-match`.
- Conformance test suite scaffold (`tests/conformance/` + `MANIFEST.json`).
- License header in generated C (`--license <SPDX>`, default MIT).
- Memory budget reporting (`fsm generate --report-memory`).
- Parser security: import path canonicalize + DoS limits (depth 256,
  1 MiB input cap).
- Determinism: `BTreeMap` in all serialized public types (Doc 13 §11
  byte-exact).
- Foundation crate `fsm-diagnostics` (Wave 1.0) — zero workspace deps;
  owns `Span`, `SourceLocation`, `Severity`, `DiagnosticCode` (75 variants),
  `Diagnostic`.

### v1.0 scope decisions
- Full UML statechart semantics (composite, parallel, history, fork/join,
  submachines, deferred events except `defer EVENT` itself which lands v1.1).
- C99 codegen only (C++17 codegen → v1.1).
- CLI only (LSP → v1.1; VS Code extension → v1.1; Web IDE → v1.1;
  simulator WebSocket protocol → v1.1).

### Implementation notes (selected — full list in `docs/00-Decisions-And-Reconciliation.md` §11)
- B-10 hierarchical dispatch implemented via leaf-to-root walk on a
  `parent_table[]`.
- B-11 collect-then-execute table dispatch implemented (per-region
  selection then sequential execute).
- B-08 parallel-state completion fires only when all regions reach Final.
- B-09 self-transition LCA: external = parent; local = self.
- B-14 history default mandatory (`FSM-E0111`).
- `after 0 ms` rejected at analyzer (`FSM-E0410`).
- `defer EVENT` rejected at analyzer in v1.0 (`FSM-E0903`,
  "v1.0 limitation; lands v1.1").
- Per-timer event IDs (`MOTOR_EVENT_TIMER_<ID>_FIRED`) — timers never
  collide with each other or with the shared completion event ID.
- Multi-active-leaf representation `m->_active[N]` — non-parallel and
  parallel machines share one dispatch path.
- Context field defaults applied in both `M_init` and the simulator's
  `Interpreter::init`.

### Verified — MVP gate (Doc 23 §9)
- G1–G5 pass (`fsm check` / `fsm generate` / `gcc -std=c99 -Werror` /
  `fmt` idempotent).
- G6 all 3 examples through full chain with simulator-trace-match.
- G7 75 diagnostic codes catalogued; 36 covered by formal conformance
  fixtures + remaining covered at crate-test level.
- G8 `cargo build / test / clippy / fmt` all clean on workspace.
- G9 GitHub Actions CI matrix configured (linux/macos/windows); local-run
  verified.
- ~491 tests passing across 9 crates.

### Deferred to v1.1+
- WebSocket simulator protocol (Doc 13).
- VS Code extension (Doc 22).
- LSP server (Doc 14).
- Web IDE (Doc 05 portions, Doc 03 simulator section).
- C++17 code generator (Doc 12).
- TextMate grammar publishing (Doc 21).
- `defer EVENT` runtime support (currently rejected at analysis with
  `FSM-E0903`).
- Submachine support. _(Correction: the original "IR + analyzer present;
  codegen silent no-op" wording was inaccurate — submachine was **absent
  end-to-end** in v1.0; see the "Corrected — v1.0.0 scope statement" note
  under [1.1.0]. The full epic landed in v1.1.)_

### Audit trail
Pre-tag audits at `docs/AUDIT_*_2026_05_14.md`:
- `AUDIT_2026_05_14.md` — correctness (5 P0 + 9 P1 + 12 P2 + 7 P3; all P0
  + critical P1 resolved).
- `AUDIT_B_ARCHITECTURE_2026_05_14.md` — architecture and coupling (0 P0;
  "architecturally sound").
- `AUDIT_C_QUALITY_2026_05_14.md` — complexity and maintainability (2 P0
  quality-class — not release blockers; cleanup tracked for v1.0.1).
- `AUDIT_D_RELIABILITY_AI_2026_05_14.md` — reliability and
  AI-friendliness (`#![forbid(unsafe_code)]` everywhere; 14/14 Doc 00
  blockers traceable in code).

[Unreleased]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/releases/tag/v1.0.0
