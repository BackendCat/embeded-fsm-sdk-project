# FSM Studio — Roadmap

**Status:** Living document. Updated each release; major edits are commit-tracked.
**Owner:** TL/PM (in-conversation Claude under user-granted autonomy).
**Last updated:** 2026-05-15.

This roadmap captures the strategic direction of FSM Studio beyond v1.0. It is informed by user product-strategy conversations, audit findings, and the spec corpus. Items are prioritized by user impact AND by what reduces the cost of subsequent features.

---

## v1.0 (current — tagging in flight)

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
- Submachine codegen silent no-op (IR + analyzer landed; codegen v1.1)
- C++17 backend not started (Doc 12 spec exists; impl v1.1+)
- LSP / VS Code / Web IDE / simulator WebSocket all deferred (Docs 13/14/22/05 specs exist; impl v1.2+)
- Conformance coverage: 36/75 codes have formal fixtures (rest have crate-level negative tests); v1.1 closes the gap

See `docs/GATE_VERIFICATION_v1_0.md` for proof of MVP gate compliance (G1–G9).
See `docs/00-Decisions-And-Reconciliation.md` §11 for the full implementation-time decisions log.

---

## v1.1 — Integration ergonomics & UML completion

**Theme: Make the DSL usable in real customer codebases. Finish UML feature parity.**

Adoption-blocking items first; UML completeness second.

> ⛔ **v1.1 TAG IS BLOCKED by W7-FU-1 (P0).** W7's behavioural-acceptance discipline surfaced a pre-existing P0-1-class core defect: a state with ≥2 transitions on the **same event** disambiguated by guards (`on E [g1]->A` / `on E [g2]->B`, incl. an `on E ->C` fallback — bread-and-butter UML) is mis-lowered by **both** dispatch strategies (switch → hard `duplicate case` gcc error even without `-Werror`; table → silent guard-ignoring event-drop, P0-1 silent-data-loss class). No shipped example trips it (so nothing currently-tested is broken), but tagging "full UML" while a core legal construct silently miscompiles would repeat the exact P0-1/submachine overstatement the project keeps re-learning. Fix lands ahead of W8. Detail: backlog `W7-FU-1`, Doc 00 §11.25, CHANGELOG.

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
- **Submachine support — EPIC COMPLETE 2026-05-15.** Originally mis-claimed "IR+analyzer already lower submachines; codegen no-op" — that was false (absent end-to-end; the P0-1 aspirational-prose pattern). Reconstructed as a real 4-wave epic, all merged: **W2a** parser/CST/AST `7a69612` → **W2b** analyzer/IR + FSM-E0103/E0610/E0500/E0501/E0502 `2384608` → **W2c** simulator RTC Doc 08 §12 `8b66dc2` → **W2d** C99 codegen both strategies `af8c300`. Implemented **end-to-end for top-level `state X is Sub`** — gcc -Werror (auto+table) + sim≡codegen verified, exercised by `examples/submachine/`. **Precise residual (corrected 3rd time — phase-boundary audit caught me re-overstating; the lesson recurred at orchestrator level):** a submachine ref *nested inside a composite/parallel state* is **NOT** an inert leaf — it emits prototype-less `entry_/exit_` calls → C that fails the project's own `gcc -Werror`. **P1-2 fix:** analyzer will reject it with a clear diagnostic (defer/E0903 precedent — clean reject > broken output, G1) until properly implemented; **SUB-FU-2** tracks the implement-properly follow-up. Accurate statement: **full UML for top-level `state X is Sub`** (gcc+sim≡codegen verified); nested submachine refs **rejected at analysis** — explicit v1.1.x limitation, NOT a blanket full-UML claim. Lesson, hard-earned twice ([[verify-status-claims-vs-code]]): claims are intent until code-verified; corrections must be exact and conservative — when unsure, say LESS, and let the §11.3 phase audit be the check.
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
- 0 P0, ≤5 well-scoped P1s in pre-tag audit — **currently 1 open P0: W7-FU-1 (must be green before W8 runs)**
- 600+ tests passing
- One real-world customer-style integration example proven end-to-end on a representative embedded target — ✅ **SATISFIED 2026-05-15** (W6: four ecosystems, each gcc -Werror RUN-verified; platformio `native`+`uno` AVR target)

---

## v1.2 — Developer tooling

**Theme: Editor & IDE experience.**

- **LSP server** (Doc 14): `tower-lsp`-based. Hover, diagnostics, go-to-definition, completion, rename, code actions. Required substrate for VS Code + Web IDE.
- **VS Code extension** (Doc 22): TextMate grammar (Doc 21 already drafted), LSP client wiring, diagram WebviewPanel using @elklayout/core.
- **C++17 code generator** (Doc 12): wraps generated C with `extern "C"` + adds a CRTP class API for embedded C++ users. No-STL profile per Doc 12 §3.

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
