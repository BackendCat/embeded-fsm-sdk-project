# Changelog

All notable changes to FSM Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Corrected — v1.0.0 scope statement (audit integrity, 2026-05-15)

The v1.0.0 entry below described scope as "full UML statechart semantics." This was **overstated**: **submachine support is entirely absent** across the pipeline (grammar has no `submachine`/`state … is X` production; `KwIs`/`KwSubmachine` tokens are orphans; `lower.rs` hard-codes `submachines: Vec::new()`; IR `SubmachineRef` is only built by a unit-test fixture; codegen/simulator arms are defensive-only for a never-produced variant). Earlier ROADMAP/backlog claims that "IR + analyzer already lower submachine references" were aspirational prose never reconciled against code — the same class as the P0-1 finding. The three shipped examples (motor, traffic-light, vending-machine) do not use submachines, so `GATE_VERIFICATION_v1_0.md` was accurate for what it tested; the defect was the *scope wording*, not the gate evidence. Correct reading of v1.0.0: **UML statecharts excluding submachines**.

**Update (2026-05-15, epic complete):** the submachine epic landed across four waves — W2a parser/CST/AST (`7a69612`), W2b analyzer/IR lowering + FSM-E0103/E0610/E0500/E0501/E0502 (`2384608`), W2c simulator runtime semantics Doc 08 §12 (`8b66dc2`), W2d C99 codegen both dispatch strategies (`af8c300`). Submachine is now implemented **end-to-end** for top-level `state X is Sub` references — parser → analyzer/IR → simulator → codegen, gcc -std=c99 -Werror-verified (both strategies) and sim≡codegen behaviour-matched, exercised by `examples/submachine/`. **Precise scope (corrected 3rd time after the phase-boundary audit caught me re-overstating — this IS the P0-1/submachine lesson, and it recurred at orchestrator level):** a submachine reference *nested inside a composite or parallel state* is **NOT** an "inert leaf" as previously written — it currently emits dangling `entry_`/`exit_` calls **without prototypes**, producing C that does **not** compile under the project's own mandated `gcc -Werror`. That is a real defect (milder than P0-1: explicitly flagged, generator doesn't crash, simulator side is genuinely inert-safe, the supported top-level path is solid). **Fix in progress (P1-2): the analyzer will REJECT a nested submachine reference with a clear diagnostic** (the honest defer/E0903 precedent — reject cleanly rather than emit broken output, G1 no-undefined-behaviour) until it is properly implemented. **SUB-FU-2** now tracks the *implement-properly* follow-up. Accurate v1.1 statement: **full UML for top-level `state X is Sub`** (gcc + sim≡codegen verified); **nested submachine references are rejected at analysis with a diagnostic** — an explicit, documented v1.1.x limitation. NOT a blanket "full-UML coverage" claim.

### Added (in progress toward v1.1)
- **Submachine support end-to-end** (W2a-d) — top-level `state X is Sub` references; nested-in-composite/parallel deferred as SUB-FU-2.
- `defer EVENT` runtime (removes the v1.0 FSM-E0903 limitation) — bounded buffer + FIFO replay on state exit; sim≡codegen verified.
- IR-schema-validation gate (debug-mode, zero release cost) — malformed IR caught at the analyzer boundary; proven load-bearing.
- Shared `CARGO_TARGET_DIR` + warm-cache build policy (wave-speed + disk-safety).
- TD-BUG-1 + 3 sibling table-strategy degenerate-input codegen bugs fixed (zero-transition machine now gcc -Werror-clean, both strategies).
- Architecture-debt paydown: unified LCA (one generic `ParentResolver`), `LoweringCtx` god-object eliminated, `fsm-simulator` dead-dep removed.
- `likely` / `rare` transition annotations → portable `__builtin_expect` codegen (`<PFX>_LIKELY/_UNLIKELY` macros, GNU/clang builtin + standard-C fallback + opt-out), both dispatch strategies, gcc -Werror -pedantic clean. Contextual keyword — lexer unchanged, fully back-compatible.
- `fsm generate --import-header <path.h>` — auto-`extern` declarations harvested from an existing C header, removing hand-written extern boilerplate for users with established C codebases.
- `examples/integration/{make,cmake,cargo-rust,platformio}/` — four worked end-to-end integrations, each genuinely building the generated C under `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (make/cmake/cargo-rust run+assert the full lifecycle; platformio ships the real `pio` recipe + a verified host-gcc fallback). Plus `docs/25-Integration-Guide.md` (ABI contract, C/C++/Rust recipes, build-system fragments, troubleshooting matrix).
- **Per-machine dispatch strategy override** (W7) — `fsm.toml [machine.<Name>] strategy = "switch"|"table"|"auto"` selects dispatch per machine, enabling MIXED dispatch in one project (machine A switch, machine B table from a single `fsm generate`). Effective precedence per machine: `[machine.M].strategy` > CLI `--strategy` > default `auto`. Invalid value = clean exit-4 config error naming the machine; a `[machine.X]` for a machine not generated this run is a WARN (project-wide fsm.toml generating a subset stays valid). Behaviourally accepted: both strategies gcc -Werror compiled + run with per-machine lifecycle assertions.

### Fixed (toward v1.1)
- **Guard-disambiguated same-event transitions mis-lowered by BOTH dispatch strategies** (W7-FU-1, pre-existing core dispatch defect, P0-1 silent-data-loss class; surfaced by W7's §5.4 behavioural-acceptance discipline, NOT introduced by W7): a state with two or more transitions on the same event disambiguated by guards (`on E [g1] -> A` / `on E [g2] -> B`, incl. an unguarded `on E -> C` fallback — bread-and-butter UML the DSL/analyzer/IR accept and preserve) was independently mis-lowered: the **switch** strategy emitted one C `case <EVENT>:` per transition → `gcc` hard error `duplicate case value` (even without `-Werror`), the 2nd+ guarded transition unreachable; the **table** strategy's `select_for_region` returned the first `(source,trigger)` row ignoring its guard, the executor then silently no-op'd if the guard was false → the event was dropped and the later eligible transition never fired. Both now honour Doc 08 §4.1/§4.2: the first guard-enabled candidate in (priority, document-order) wins, an unguarded/`[else]` transition is the always-enabled catch-all, no enabled candidate ⇒ the event bubbles up (no consume). The simulator was independently verified spec-correct and is the oracle; the fix is codegen-only and makes both strategies match it. Behaviourally accepted on BOTH strategies (gcc -Werror compiled + RUN-asserted: priority-tiebreak when >1 guard is simultaneously true, unguarded fallback, no-match discard, sim==codegen). The honest tripwire was converted to a positive correctness test; conformance CGEN-004 added.
- **Default transition priority lowered as `0`, contradicting the spec's `100`** (W7-FU-2, pre-existing doc-vs-impl divergence; surfaced by W7-FU-1's §5.4 discipline, NOT introduced by it): a transition with **no `priority` clause** was lowered by the analyzer's four `lower_*` arms with `.unwrap_or(0)` → IR priority `0`. **Doc 04 §8.6** ("Lower number = higher priority. Default: `100`.") and **Doc 09 §6** ("default 100", canonical example `"priority": 100`) both specify **100**; the IR `model.rs` field comment already said "Default 100"; no corpus source intends `0`. **Decision: default = `100`** (the impl was the bug, not the docs). Rationale (Doc 08 §4.2 min-wins on `(priority, document_order)` — lower number = higher priority): an unprioritized transition must sit at LOW priority so assigning a small explicit number FLOATS a specific/guarded transition above the default herd; a `0` default made an unprioritized transition out-prioritise any explicitly-deprioritised one and no transition could ever be placed below the default — the priority feature was half-useless. The discrepancy was applied IDENTICALLY by the simulator and both codegen strategies (each only READS the IR `priority`), so sim≡codegen and W7-FU-1's *relative* ordering held regardless — hence P1 (consistent, well-defined) not P0 (silent miscompile). Reconciled to a single source of truth: `fsm_ir::DEFAULT_TRANSITION_PRIORITY = 100`, referenced by the four `lower_*` arms and a serde `default`; the wrong `lower/mod.rs` doc-comment and the IR JSON-schema `priority` annotation were aligned (the W0 IR-schema gate stays green — `priority` is `required`/always-serialized so a JSON-Schema `default` annotation does not affect validation). Behaviourally accepted per §5.4 on BOTH strategies (a hand-built IR — the analyzer's FSM-E0300 forbids a clause-less transition in same-source same-event runtime competition, so the default's *observable* effect is only demonstrable at the codegen/sim contract layer below the compile-time gate: gcc -Werror compiled + RUN-asserted the spec-correct min-wins transition fired, sim≡codegen). The pre-existing `ir_default_priority_is_zero` unit test was pinning the *bug* — corrected to assert `100` (the FAILS-on-old / PASSES-on-new regression anchor + §5.4 unit pin). No shipped example's behaviour changed (no example `.fsm` uses `priority`; the W7-FU-1 fixture/CGEN-004 uses explicit `priority 1/2/3`; FSM-E0300/W0300 keys off `priority.is_some()`, not the materialized default). Doc 00 §11.27.
- **Opaque extern params silently dropped** (OPAQUE-BUG-1, pre-existing core defect, P0-1 silent-data-loss class): `extern` declarations using the documented `opaque "C_type"` form for params/returns were silently lost in lowering (`lower_type_ref` returned `None` for `OPAQUE_TYPE_REF`). Now modelled as `Type::Opaque { c_type }` end-to-end (context field, extern param, extern return, `as`-cast) — gcc-RUN verified. Unblocks passing C handles/pointers/structs to HAL functions from FSM actions.
- **Parser panic on a comment before `language fsm`** (PARSE-BUG-1, pre-existing G1-robustness defect): a `//` license header atop a source file (an extremely common real-world input) tripped the rowan single-root assertion. Leading trivia is now flushed after the FILE node opens — the toolchain parses/diagnoses instead of panicking.

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
- Submachine codegen (IR + analyzer present; codegen silent no-op until
  v1.0.1+).

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

[1.0.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/releases/tag/v1.0.0
