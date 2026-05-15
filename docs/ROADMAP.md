# FSM Studio — Roadmap

**Status:** Living document. Updated each release; major edits are commit-tracked.
**Owner:** TL/PM (in-conversation Claude under user-granted autonomy).
**Last updated:** 2026-05-15.

This roadmap captures the strategic direction of FSM Studio beyond v1.0. It is informed by user product-strategy conversations, audit findings, and the spec corpus. Items are prioritized by user impact AND by what reduces the cost of subsequent features.

---

## v1.0 — SHIPPED (tagged `v1.0.0`, 2026-05-15, local)

**Theme: Correct, tested, embedded-ready core.**

What ships:
- FSM-Lang DSL (full UML statechart syntax, hierarchical, parallel regions, history, choice, junction, fork/join)
- Compiler pipeline: lexer → parser (Rowan CST + AST) → analyzer (semantic + AST→IR lowering) → IR
- C99 code generator with two dispatch strategies (`--strategy {switch,table,auto}`)
- In-process simulator (RTC interpreter, virtual clock, deterministic JSON traces)
- Canonical formatter (`fsm fmt`, idempotent)
- CLI: `fsm check / generate / fmt / parse / test / doc / decompile / init`
- HAL contract (`fsm_hal_clock_now_ms`, `fsm_hal_assert`), mandatory
- 3 worked examples (motor, traffic-light, vending-machine), all pass full chain check → generate → gcc -Werror → fmt --check → simulator-trace-match

Known v1.0 limitations (each becomes a v1.1+ item):
- `defer EVENT` rejected at analysis (FSM-E0903); will land in v1.1
- Submachine — _absent end-to-end in v1.0_ (the earlier "IR + analyzer landed; codegen no-op" wording was inaccurate; full epic landed in v1.1 — see the v1.1 section + CHANGELOG [1.1.0] "Corrected" note)
- C++17 backend not started (Doc 12 spec exists; impl v1.1+)
- LSP / VS Code / Web IDE / simulator WebSocket all deferred (Docs 13/14/22/05 specs exist; impl v1.2+)
- Conformance coverage: 36/75 codes have formal fixtures (rest have crate-level negative tests); v1.1 closes the gap

See `docs/GATE_VERIFICATION_v1_0.md` for proof of MVP gate compliance (G1–G9).
See `docs/00-Decisions-And-Reconciliation.md` §11 for the full implementation-time decisions log.

---

## v1.1 — Integration ergonomics & UML completion — ✅ SHIPPED (tagged `v1.1.0`→`9634c7e`, 2026-05-15, local; `checkpoint/2026-05-15` anchor)

**Theme: Make the DSL usable in real customer codebases. Finish UML feature parity.**

Adoption-blocking items first; UML completeness second.

> ✅ **W7-FU-1 (P0) RESOLVED** (`d957d12`) — guard-disambiguated same-event dispatch fixed Doc-08-§4.1/§4.2-correct on **both** strategies (simulator independently spec-verified as the oracle; codegen-only bug; tripwire converted to a positive test; conformance CGEN-004). Detail: Doc 00 §11.26, CHANGELOG *Fixed*.
>
> ✅ **W7-FU-2 (P1) RESOLVED** (`41b9e46`) — default transition priority reconciled to **100** (corpus-verified: Doc 04 §8.6, Doc 09 §6, the IR field's own doc-comment; the `unwrap_or(0)` impl was the bug). Class-of-issues: 8 materialization sites → one `DEFAULT_TRANSITION_PRIORITY` constant; byte-identity delta proven exactly `0→100` with no structural perturbation; no shipped-example runtime change. Detail: Doc 00 §11.27, CHANGELOG *Fixed*.
>
> ✅ **SHIPPED 2026-05-15.** 3-lens pre-tag audit (frozen `020f009`): Arch **0 P0** (AD-1/2/3 verified true in source); Correctness **0 P0**, 0 overstated Doc-00 rows; Reliability/Security test-integrity strong, found **SEC-P0-1** (the one P0 — `--import-header`/`fsm.toml` G-02 bypass) → fixed `2b3d221` by converging on `resolve_import` (no third variant; +17 security tests). REL-P1-1 was **stale vs current code** (nested-submachine reject had already shipped in P1-2 `02d4ded`); the record was corrected to the shipped `FSM-E0502` reality, not regressed — verify-vs-code applies to audit claims too (Doc 00 §11.29). **§11.22 cold-from-source quad GREEN:** `cargo clean -p` 9 crates (1436 files/6.4 GiB) → full from-source rebuild → 673 pass / 0 fail, **`-wt-` stale-path = 0**, clippy `-D warnings` clean, fmt clean, examples 5/5, conformance 25/25 (disk safe throughout — the conventional workspace-from-source approach fit without provisioning). Canonical record: `docs/GATE_VERIFICATION_v1_1.md`. **Post-tag owner action (only thing needing the user):** push the tag + commits to exercise the never-run CI matrix (G9; SEC-P0-1's path-canonicalization is the most Windows-divergent surface) — `GATE_VERIFICATION_v1_1.md` §5.

### Integration features
- **`fsm generate --import-header <path.h>`** — auto-extern from existing C header files. ✅ **COMPLETE 2026-05-15** (W5). Reduces friction for users with substantial existing C codebases who currently must hand-write each `extern` declaration. (OPAQUE-BUG-1, surfaced *by* this wave's honest scoping, then fixed in §11.23 — opaque pointer/struct HAL params now model end-to-end; relaxing W5's importer opaque-skip tracked as W5-FU-1.)
- **`examples/integration/{make,cmake,cargo-rust,platformio}/`** — ✅ **COMPLETE 2026-05-15** (W6 `e74888a`). Four worked end-to-end integrations; each *genuinely* builds the generated C under `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (make/cmake/cargo-rust run+assert the full lifecycle; platformio ships the real `pio` recipe + a verified host-gcc fallback since `pio` is absent on the box). No codegen defect surfaced — independent corroboration the pipeline holds under real ecosystems.
- **Per-machine strategy override** via `fsm.toml`:
  ```toml
  [machine.MotorControl]
  strategy = "switch"
  ```
  More fine-grained than CLI flag; enables mixed dispatch in one project.
- **`docs/25-Integration-Guide.md`** — ✅ **COMPLETE 2026-05-15** (W6). First-class doc: generated-C ABI contract, C/C++ `extern "C"` recipe, Rust `cc`+FFI recipe (incl. workspace-isolation nuance), Make/CMake/PlatformIO fragments, mandatory-HAL + `_POSIX_C_SOURCE`-ordering gotcha, integration knobs, a 10-row troubleshooting matrix. Cross-linked from README + `examples/integration/README.md`.

### UML completion
- **`defer EVENT` real implementation.** Bounded defer queue per state; replay on state exit. Removes the v1.0 hard-fail with FSM-E0903.
- **Submachine support — EPIC COMPLETE 2026-05-15.** Originally mis-claimed "IR+analyzer already lower submachines; codegen no-op" — that was false (absent end-to-end; the P0-1 aspirational-prose pattern). Reconstructed as a real 4-wave epic, all merged: **W2a** parser/CST/AST `7a69612` → **W2b** analyzer/IR + FSM-E0103/E0610/E0500/E0501/E0502 `2384608` → **W2c** simulator RTC Doc 08 §12 `8b66dc2` → **W2d** C99 codegen both strategies `af8c300`. Implemented **end-to-end for top-level `state X is Sub`** — gcc -Werror (auto+table) + sim≡codegen verified, exercised by `examples/submachine/`. **Precise scope (shipped reality, verified in code 2026-05-15):** a submachine ref *nested inside a composite/parallel state* is **rejected at analysis with `FSM-E0502`** + an actionable message (move it to top level); the lowerer additionally refuses it (defence-in-depth) so codegen never receives it — **no broken C is emitted** (clean-reject > broken-output, the retired-E0903/defer precedent, G1). Shipped by **P1-2 `02d4ded`**; test-pinned by `crates/fsm-analyzer/tests/nested_submachine_rejected.rs`. **SUB-FU-2** tracks proper nested *support* (→ v1.1.x). Accurate statement: **full UML for top-level `state X is Sub`** (gcc+sim≡codegen verified); a nested submachine ref is a documented, diagnosed limitation (FSM-E0502) — NOT a blanket full-UML claim. Lesson ([[verify-status-claims-vs-code]]): this residual was over- then *mis*-corrected several times ("inert leaf" → "emits broken C / fix in progress" — both stale; the pre-tag REL-P1-1 audit even repeated the stale "emits broken C" from an older audit doc). The discipline — verify the *current* code, conservative completed-tense wording — applies to audit claims too; the record was corrected to the shipped E0502 reject, not regressed. Doc 00 §11.29.
- **`likely` / `rare` transition annotations** → `__builtin_expect` codegen. ✅ **COMPLETE 2026-05-15** (W4 `300d1e4`): full vertical slice — contextual keyword (back-compat, lexer unchanged) → CST `BRANCH_HINT` → IR `TransitionObject.hint` (serde-default, schema-valid) → analyzer lowering → portable `<PFX>_LIKELY/_UNLIKELY` macro (`__builtin_expect` on GNU/clang, `(x)` fallback + `<PFX>_NO_BUILTIN_EXPECT` opt-out), both dispatch strategies, gcc -Werror -pedantic clean. Sim accepts+ignores (proven layout-only: sole `Motor.c` delta is the macro wrap; sim≡codegen + 5/5 traces unchanged). Spec'd in Doc 04 §8.8 + Doc 11 §28. +10 tests (589 total).

### Test + conformance hardening
- Populate the 39 untested diagnostic codes with conformance fixtures (G7 evidence becomes 75/75 in the formal suite, not just crate-level).
- Add cross-architecture gcc cross-compile test (ARM Cortex-M0 + AVR + RISC-V) for at least the Motor example.
- Add a fuzzing harness for the parser (cargo-fuzz integration).

### Quality cleanup carried over from Audit B/C
- LCA dedup (currently 3 implementations across analyzer/simulator/codegen). Generic `ParentResolver` trait in fsm-ir.
- analyzer → AST coupling (16 sites reach into fsm-parser's CST instead of using typed AST). Extend AST accessors then strip cst::* imports.
- `pub → pub(crate)` sweep across 9 crates (~340 over-exposed items per Audit B P2-A1).

### Release criteria for v1.1
- 0 P0, ≤5 well-scoped P1s in pre-tag audit — **0 open P0, 0 open spec-conformance items (W7-FU-1 + W7-FU-2 both resolved); pre-tag audit in progress; final gate = cold-from-source full-workspace quad**
- 600+ tests passing
- One real-world customer-style integration example proven end-to-end on a representative embedded target — ✅ **SATISFIED 2026-05-15** (W6: four ecosystems, each gcc -Werror RUN-verified; platformio `native`+`uno` AVR target)

---

## v1.1 retrospective (post-tag, 2026-05-15)

Codified post-release step (this section's own "How the roadmap evolves" rule). Metrics snapshot: `docs/metrics/2026-05-15-v1.1.0.json` (v1.0→v1.1 deltas, D.8 contract).

- **The §5.4 behavioural-acceptance mandate was the single highest-ROI process change of the sprint.** Requiring every behaviour-touching wave to gcc-compile *and RUN* the generated C and assert observable behaviour surfaced **four pre-existing P0/P0-class defects** that ~600+ symbol-presence-era tests had passed straight over: **OB1** opaque-extern silent-drop (Doc 00 §11.23), **W7-FU-1** guard-disambiguated dispatch broken on *both* strategies (§11.26), **W7-FU-2** default-priority spec divergence `0` vs `100` (§11.27), **SEC-P0-1** import-header G-02 containment bypass (§11.28). These were latent in shipped code; nothing structural would have caught them. `.contains("Motor_init")` proves a name exists, not that the machine *does* anything — the mandate is now the project's load-bearing correctness guard (26 gcc-RUN acceptance files concentrated on exactly the risk surface).

- **`verify-status-claims-vs-code` is symmetric (§11.29).** The pre-tag REL-P1-1 finding ("nested submachine emits non-compilable C") was a *stale pessimistic audit claim* — the auditor traced an older pre-P1-2 doc; current code rejects that case cleanly at analysis with `FSM-E0502`. Blindly applying the recommendation would have regressed the release record into a falsehood. **An audit finding is a claim; code at HEAD is ground truth.** The same rigor that catches optimistic overstatement must catch a pessimistic stale audit, or the record drifts the other way.

- **The 3-lens pre-tag gate is well-calibrated.** Across a 15-wave surface it caught **exactly one** real P0 (SEC-P0-1) — **0 P0 arch, 0 P0 correctness, 0 overstated Doc-00 rows**. It neither over-fires (didn't manufacture blockers) nor under-fires (caught the one real security regression on a new file-read surface). Keep the 3-lens gate as-is for v1.2.

- **Infra/disk pattern validated — no provisioning needed.** The §7 warm-cache sawtooth sustained 15 build-heavy waves on a shared 78G box (steady ~7–9G target, never runaway). The §11.22 release gate's mandatory **cold** quad fit via the conventional CI-cache approach: `cargo clean -p` the 9 first-party crates (drops every first-party + test binary, including stale `-wt-`-tainted ones) while **keeping the content-addressed registry dep cache** (Cargo-fingerprint path-independent). Cold quad ran 673/0 with `-wt-` stale-path = 0; disk stayed ≥3.4G throughout. This is the recorded validated pattern — do not provision disk for a cold release quad; clean first-party-from-source, keep registry deps.

- **Feed-forward to v1.2 (recommendation — the orchestrator decides):**
  - **(a) Run a dedicated cleanup wave BEFORE v1.2 features.** Both pre-tag audits flagged the same two accumulating debts as the standing v1.2 gate: `pub` over-exposure (only a small fraction used cross-crate; `fsm-parser` is the dominant offender at ~230–280 public items — add `unreachable_pub` + `missing_docs` lints and a `pub→pub(crate)` sweep) and analyzer→parser-CST coupling (~15 sites reaching into `cst::*` instead of typed AST — extend AST accessors, strip the imports). Both are non-behavioural and ship-acceptable *now*, but they compound: every v1.2 feature built on the over-wide surface makes the eventual narrowing harder. Sequence the cleanup wave first; it is bounded and has no feature risk.
  - **(b) Sequencing opinion for the v1.2 feature set — LSP-first over C++17 codegen.** Both are in v1.2 scope. **Recommendation: do the LSP server (Doc 14) first.** Rationale: LSP is the *required substrate* for both the VS Code extension and the future Web IDE — it unlocks two downstream deliverables, so an early LSP de-risks the largest chunk of v1.2+v1.3 dependency. C++17 codegen (Doc 12) is self-contained (it wraps the already-correct C emitter with `extern "C"` + a CRTP shell — no new pipeline semantics, low coupling), so it can land *in parallel* or *after* without blocking anything and carries little integration risk. Front-loading the high-fan-out item (LSP) and trailing the isolated one (C++17) maximises critical-path progress. This is a reasoned recommendation, not a decree — the orchestrator owns the call.

---

## v1.2 — Developer tooling

**Theme: Editor & IDE experience.**

- **LSP server** (Doc 14): `tower-lsp`-based. Hover, diagnostics, go-to-definition, completion, rename, code actions. Required substrate for VS Code + Web IDE.
- **VS Code extension** (Doc 22): TextMate grammar (Doc 21 already drafted), LSP client wiring, diagram WebviewPanel using @elklayout/core.
- **C++17 code generator** (Doc 12): wraps generated C with `extern "C"` + adds a CRTP class API for embedded C++ users. No-STL profile per Doc 12 §3.

### v1.2 refinement — LSP-first sequencing (extraction pass, 2026-05-15)

The v1.1 retrospective feed-forward (b) recommended LSP-first over C++17; the extraction-first-for-epics discipline (FSM-PROC-SUBAGENT) then produced a deep-extraction architecture pass scoped against the *current code at HEAD `0a657d4`*. Result: **`docs/26-LSP-Architecture.md`** (FSM-ARCH-LSP v1.0.0) — the authoritative LSP crate shape + wave plan; it refines (does not contradict) Doc 20 §9.

**Why LSP-first holds (confirmed against code, not prose):** the LSP is the required substrate for both the VS Code extension (Doc 22, an LSP client) and the Web IDE (Doc 05 §2.5, LSP client in a Web Worker) — largest downstream fan-out. C++17 codegen is self-contained (wraps the already-correct C emitter) and may land in parallel or after without blocking anything.

**Dependency edge:** the LSP epic is gated on the standing v1.2 pub-hygiene wave (retrospective feed-forward (a) — `pub→pub(crate)` sweep + `unreachable_pub`/`missing_docs`). That wave MUST run **first** and be briefed with the Doc 26 §6 "keep-public, document-as-intended-`fsm-lsp`-API" allowlist, so the surface narrowing is informed by the LSP's real needs rather than blind. This is listed in Doc 26 as L0 (the gate prerequisite, not new scope).

**Wave plan (one line per wave; full briefs + §5.4-LSP behavioural-acceptance definitions in Doc 26 §8):**
- **L0** (gate) — pub-hygiene sweep + Doc 26 §6 intended-API allowlist (the existing standing gate wave; pinned here as the dependency edge).
- **L1** — MVP spine: `fsm-lsp` crate + `fsm-lang-server` bin + `initialize` (UTF-8/UTF-16 `positionEncoding` negotiation) + `publishDiagnostics` reusing the *exact* `fsm check` pipeline (`parse` + `analyze_with_source`). Proves the reuse seam + position encoding end-to-end before any breadth.
- **L2** — `documentSymbol` + `foldingRange` (pure forward-`SymbolTable`/CST; no new analysis).
- **L3** — `hover` + `definition` (single-file; `resolve_*` decl spans + IR enrichment).
- **L4** — `completion` (context-sensitive; keywords from Doc 04 §1.5; snippets from Doc 22 §9).
- **L5** — `references` + `rename` + `prepareRename` (introduces the one new analysis: `ReferenceIndex`, a derived CST walk resolved through `SymbolTable`).
- **L6** — `semanticTokens` (consumes the already-drafted Doc 21; decl-vs-ref disambiguation).
- **L7** — `codeAction` + `inlayHint` (diagnostic-driven fixes + IR-derived hints).

Phase-boundary audits (FSM-PROC-SUBAGENT §11.3) after L1 and after L5.

**Explicitly scoped OUT of v1.2 (Doc 26 §9 — no implied freebies):** incremental CST parsing (`parse_incremental` does **not** exist — Doc 20 §4.5/L787 is aspirational prose; full re-parse-on-debounce is the v1.2 contract); cross-file/workspace-wide definition/references/rename + the startup workspace scan (no in-tree project-index substrate — single-file in v1.2, shipping Doc 14 §8's own cross-file degradation message; deferred to v1.3 with the Web IDE multi-file story); the `wasm32` build of `fsm-lsp` (architecture stays WASM-compatible — analysis core is tokio-free — but compiling it is v1.3 scope); `workspaceSymbol` (same project-index dependency).

---

## v1.3 — Simulation & verification

**Theme: Trustable behavioral validation.**

- **Simulator WebSocket protocol** (Doc 13): JSON-RPC 2.0 server lets VS Code / Web IDE drive the simulator interactively. Step-debugging, breakpoints, watchpoints.
- **Web IDE** (Doc 05): Monaco editor + WASM-compiled toolchain + ELK-laid-out diagram. Demo target: edit FSM in browser, immediate diagram, immediate trace.
- **Model checking integration**: dispatch every reachable state×event pair, prove deadlock-free; report unreachable transitions (extend the existing dead-transition analyzer).
- **Trace replay & differential testing**: capture a trace from one v1.x; replay against a new v1.y; flag any divergence — protects against silent semantic drift across releases.

---

## v2.0 — Production scale

**Theme: Multi-machine systems + advanced optimization.**

- **Multi-machine projects with rich inter-machine messaging.** Today: `send EVT to Foo` works for a known instance. Tomorrow: discover, address, route across compile units.
- **Per-region `@strategy(switch|table)` annotations.** Mixed-strategy dispatch within a single machine for hot-vs-cold path optimization.
- **Profile-guided codegen** (opt-in). Optional dev-build instrumentation; profile data feeds back into codegen as branch hints.
- **`fsm bench`** — micro-benchmark generated C against reference embedded targets (Cortex-M0, M3, M4, AVR). Reports ROM, RAM, dispatch latency.
- **Code coverage for generated firmware** via `gcov` hooks.
- **Plugin API** for custom code generators (per Doc 03 §plugin notes). Third parties can ship `fsm-codegen-rust`, `fsm-codegen-vhdl`, etc.

---

## Cross-version commitments (the "always" promises)

Regardless of version:
- **Deterministic output.** Same `.fsm` source + same compiler version + same `fsm.toml` → byte-identical generated C. Reproducible builds.
- **Heap-free generated runtime.** No `malloc`, no `new`, no growable containers. The embedded G2 promise.
- **Backwards compatibility within a major version.** v1.x consumes v1.y `.fsm` source without surprise. Breaking changes wait for v2.0.
- **Audit + CHANGELOG + tag rigor.** No tag ships without 0 P0s + ≤5 well-scoped P1s + comprehensive CHANGELOG entry + signed-off MVP gate verification.

---

## Out-of-scope (never)

These have been considered and rejected for embedded reasons:
- **Hash-map / bloom-filter dispatch.** ROM cost > switch/table for typical embedded FSMs.
- **Runtime profile collection** built into firmware. Overhead exceeds benefit; users with that need can implement themselves via `pure extern` calls.
- **Native distributed FSM mesh.** This is a code generator, not an actor framework. Users wanting distributed coordination wrap the generated machines with their own transport.

See `docs/00-Decisions-And-Reconciliation.md` §10 + §11 for the full set of rejected-for-good-reasons items.

---

## How the roadmap evolves

- After each tag (v1.0, v1.1, ...): pre-tag audit + post-tag retrospective update this doc with empirical lessons. v1.1 may get reshuffled based on what v1.0 customers want.
- Strategic conversations with the user (recorded in `memory/project_user_messages_*.md`) feed into roadmap revisions.
- Audit findings drive priority shifts: a P1 from a fresh audit that affects user value typically gets fast-tracked into the next minor.

If you (a future maintainer / agent / contributor) think a feature should jump versions, propose the move with: (a) what user value it unlocks, (b) what dependency chain it sits on, (c) what risk it adds. The orchestrator weighs and updates.

---

## See also

- `docs/00-Decisions-And-Reconciliation.md` — normative TL decisions + §11 implementation log
- `docs/AUDIT_*_*.md` — historical audit evidence (informs priorities)
- `docs/processes/SUBAGENT_CONVENTIONS.md` — how wave-level work gets done
- `CHANGELOG.md` — what shipped when
- `~/.claude/plans/toasty-prancing-goose.md` — the live multi-phase execution plan
