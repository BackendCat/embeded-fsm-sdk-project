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

### Integration features
- **`fsm generate --import-header <path.h>`** — auto-extern from existing C header files. Reduces friction for users with substantial existing C codebases who currently must hand-write each `extern` declaration. Implementation: lightweight C declaration parser or `bindgen` integration.
- **`examples/integration/{make,cmake,cargo-rust,platformio}/`** — minimal worked examples showing end-to-end FSM-Lang in each ecosystem. Each compiles + runs + traces.
- **Per-machine strategy override** via `fsm.toml`:
  ```toml
  [machine.MotorControl]
  strategy = "switch"
  ```
  More fine-grained than CLI flag; enables mixed dispatch in one project.
- **`docs/25-Integration-Guide.md`** — first-class doc covering C / C++ / Rust / Ada / Zig integration patterns. Includes ABI notes, build-system fragments, troubleshooting.

### UML completion
- **`defer EVENT` real implementation.** Bounded defer queue per state; replay on state exit. Removes the v1.0 hard-fail with FSM-E0903.
- **Submachine support — EPIC COMPLETE 2026-05-15.** Originally mis-claimed "IR+analyzer already lower submachines; codegen no-op" — that was false (absent end-to-end; the P0-1 aspirational-prose pattern). Reconstructed as a real 4-wave epic, all merged: **W2a** parser/CST/AST `7a69612` → **W2b** analyzer/IR + FSM-E0103/E0610/E0500/E0501/E0502 `2384608` → **W2c** simulator RTC Doc 08 §12 `8b66dc2` → **W2d** C99 codegen both strategies `af8c300`. Implemented **end-to-end for top-level `state X is Sub`** — gcc -Werror (auto+table) + sim≡codegen verified, exercised by `examples/submachine/`. **Precise residual (corrected 3rd time — phase-boundary audit caught me re-overstating; the lesson recurred at orchestrator level):** a submachine ref *nested inside a composite/parallel state* is **NOT** an inert leaf — it emits prototype-less `entry_/exit_` calls → C that fails the project's own `gcc -Werror`. **P1-2 fix:** analyzer will reject it with a clear diagnostic (defer/E0903 precedent — clean reject > broken output, G1) until properly implemented; **SUB-FU-2** tracks the implement-properly follow-up. Accurate statement: **full UML for top-level `state X is Sub`** (gcc+sim≡codegen verified); nested submachine refs **rejected at analysis** — explicit v1.1.x limitation, NOT a blanket full-UML claim. Lesson, hard-earned twice ([[verify-status-claims-vs-code]]): claims are intent until code-verified; corrections must be exact and conservative — when unsure, say LESS, and let the §11.3 phase audit be the check.
- **`likely` / `rare` transition annotations** → `__builtin_expect` codegen. Small DSL addition with measurable instruction-cache benefit on tight loops.

### Test + conformance hardening
- Populate the 39 untested diagnostic codes with conformance fixtures (G7 evidence becomes 75/75 in the formal suite, not just crate-level).
- Add cross-architecture gcc cross-compile test (ARM Cortex-M0 + AVR + RISC-V) for at least the Motor example.
- Add a fuzzing harness for the parser (cargo-fuzz integration).

### Quality cleanup carried over from Audit B/C
- LCA dedup (currently 3 implementations across analyzer/simulator/codegen). Generic `ParentResolver` trait in fsm-ir.
- analyzer → AST coupling (16 sites reach into fsm-parser's CST instead of using typed AST). Extend AST accessors then strip cst::* imports.
- `pub → pub(crate)` sweep across 9 crates (~340 over-exposed items per Audit B P2-A1).

### Release criteria for v1.1
- 0 P0, ≤5 well-scoped P1s in pre-tag audit
- 600+ tests passing
- One real-world customer-style integration example proven end-to-end on a representative embedded target

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
