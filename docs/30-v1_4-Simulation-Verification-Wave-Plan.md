# FSM Studio — v1.4 Epic: Simulation & Verification — Wave Plan + ROADMAP↔Shipped Reconciliation

**Document ID:** FSM-PLAN-SIMVER-V14
**Version:** 1.0.0 (epic-kickoff planning extraction; READ-ONLY — produces this doc only, no implementation)
**Status:** Draft for orchestrator + owner review. **Scope is a recommendation, NOT a pre-commitment** — it awaits the owner's scope confirmation at the upcoming review before any v1.4 implementer wave is dispatched.
**Author:** TL/PM (planning extraction under delegated autonomy).
**Date:** 2026-05-16.
**Base commit:** `a036c38` (= the `v1.3.0` tag commit = `checkpoint/2026-05-16-v1_3`).
**Depends on:** Doc 13 (Simulator Protocol), Doc 08 (Formal Execution Semantics), Doc 09 (Canonical IR), Doc 10 (Diagnostic Catalog), Doc 26 (LSP Architecture), Doc 00 §11 (decision ledger), `GATE_VERIFICATION_v1_3.md`, `docs/processes/SUBAGENT_CONVENTIONS.md`.

> **Why a new sibling doc (numbering justification, per the Doc 26/28/29 precedent).** Doc 26 (LSP-Arch) and Doc 28 (v1.3-VSCode-Wave-Plan) / Doc 29 (v1.3-W0-CST-Plan) each established that an *epic-kickoff extraction + wave plan* is a first-class numbered sibling, distinct from the static spec corpus (Docs 01–25) and from the per-release `GATE_VERIFICATION_*` / `AUDIT_PHASE_*` evidence docs. v1.4 is the next epic; its plan is **Doc 30** (the next free integer after Doc 29). It does **not** replace Doc 13 (the protocol *spec*, which stays normative) — it is the *plan that reconciles Doc 13's ambition against shipped reality and sequences the work*, exactly as Doc 28 was the plan that sequenced Doc 27's architecture. This mirrors Doc 28 §"Planning docs of record" being separate from Doc 27.

---

## 0. Executive summary (read this first)

**The ROADMAP's v1.4 "Simulation & verification" theme bundles four sub-epics of wildly different risk and size.** Verified against `crates/fsm-simulator/` + `crates/fsm-analyzer/` at `a036c38`:

1. **Simulator WebSocket/JSON-RPC protocol** (Doc 13) — a *new network attack surface*; the interpreter core already exists and is correct, but **no server, no WS dependency, no `fsm simulate` command exists today**.
2. **Web IDE** (Doc 05) — Monaco + a `wasm32` build of the toolchain + ELK diagram. **The largest, highest-risk, most-deps item; nothing exists** (no wasm-bindgen, no Monaco, no wasm target).
3. **Model checking / exhaustive state-space exploration / deadlock detection** — **does not exist in any form**; the shipped "determinism" + "completion" + "unreachable" checks are *syntactic AST-local* passes, not graph reachability (FSM-E0400 / FSM-W0602 are catalog-reserved but **unimplemented** — a ROADMAP-claim drift, see §1).
4. **Trace replay & differential testing** — the *trace model is fully shipped and battle-tested* (the conformance suite is built on it); differential cross-version testing is a thin, low-risk wrapper over an existing seam.

**Recommended v1.4 cut (for the owner to confirm): a focused "verification core" minor — items (3 narrowed) + (4) — that consumes the already-correct interpreter + analyzer seams and has ZERO new network/wasm surface.** Specifically: a bounded **explicit-state reachability + deadlock-detection analysis** (driven by the *existing interpreter* as the state-transition oracle, not a re-implemented semantics) surfaced as `fsm verify`, plus **trace differential replay** (`fsm test --baseline`). Defer the interactive simulator WebSocket server and the Web IDE to **v1.5/v1.6** as their own minors.

This keeps the validated v1.2/v1.3 small-tight-tagged cadence, avoids opening a network attack surface and a wasm build at the same time as new verification semantics, and delivers the highest user-value-per-risk slice ("prove my FSM can't deadlock" + "prove this release didn't silently change semantics"). Two owner-decisions are surfaced non-blocking in §3 (the network surface; the model-check compute/memory envelope on the shared box).

**A note on this document's discipline (the recurring lesson, applied here).** Per [[feedback_ck_vpd_crm_source_of_truth_extraction]] + [[feedback_verify_status_claims_vs_code]]: every ROADMAP claim below was checked against code at `a036c38`, not trusted from the roadmap sketch. The ROADMAP↔shipped drift in §1 is the load-bearing output — a v1.4 plan that trusted the sketch ("extend the existing dead-transition analyzer") would have mis-scoped the largest wave, because the "dead-transition analyzer" the ROADMAP references is a 67-line `[else]`-textual check, not a reachability engine.

---

## Owner scope-confirmation + TL architecture decision (2026-05-16)

> **Status of this document changes here.** Everything below this section (§1–§6) is the **verified reconciliation + recommendation of record** and is **NOT rewritten** — it stands as authored. This section records the owner's confirmation and the consequent TL architecture decision *on top of* that analysis. The owner message is persisted verbatim at `memory/project_user_messages_embeded_fsm_2026_05_16_v14_strategy.md`; this section is a faithful record of the decisions, not a reinterpretation. Recorded at the `phase4.0/v1_4-owner-reconcile` reconcile commit.

### Scope — CONFIRMED (no longer a recommendation; the §2 cut stands as decided)

The owner confirmed the §2.1 recommended cut **in full**: **v1.4 = the complete Verification core** = the plan's recommended cut **A + B**:

- **A. Bounded explicit-state reachability + deadlock detection** driven by the *shipped interpreter as the semantic oracle* (the §4.1 keystone), surfaced as `fsm verify`, which **honestly closes FSM-E0400 / FSM-W0602** (the §1.3 catalog-reserved-but-unimplemented drift, fixed in the record).
- **B. Trace differential replay** (`fsm test --baseline`).

The deferred menu **stands exactly as recommended**: the **simulator WebSocket/JSON-RPC server → v1.5**; the **Web IDE / wasm32 / Monaco / project-index → v1.6**; **mechanised sim≡codegen equivalence → v1.4 stretch / v1.5**. (Owner: *"complete verification core is the priority"* — confirmed not a partial cut. The §2.1 alternatives were the re-cut menu; the owner did not re-cut, he confirmed.)

### TL architecture decision (the open question the owner explicitly delegated to the TL)

The owner posed the implementation-architecture question — *"the separate server, same server, or the verification server or whatever, or the plugin architecture … you select the architecture"* — and **explicitly delegated it to the TL** (*"you select the implementation way, you select the architecture, you select the technologies"*). Recorded decision (verbatim-in-substance):

1. **The verification core is a NEW single library crate `fsm-verify`** — the **one source of truth for verification semantics**. This realizes the §4 / §4.3 W1-brief recommendation as the confirmed architecture (resolves the §4.2-W0 placement decision: a new `fsm-verify` crate, NOT a `fsm-analyzer` module, NOT a `fsm-simulator` module — the §4.2-W0 rationale holds: a reachability check in `fsm-analyzer` would drag the interpreter into the analyzer's dep graph, architecturally wrong; the most behaviourally-critical crate stays untouched).
2. **`fsm-verify` drives the shipped `fsm_simulator::Interpreter` snapshot/restore seam as the semantic oracle — it NEVER re-implements FSM semantics.** This is the §4.1 keystone, **confirmed as the architecture, not merely the W1 instruction**. The forbidden second-semantics foot-gun (§4.1 / R2) is an architecture-level invariant for the whole epic.
3. **The canonical, pipeline-grade interface is the CLI.** A new **`fsm verify` subcommand in the existing single `fsm` binary** — *not* a new binary, *not* a server, *not* a plugin host. It is:
   - **scriptable with distinct exit codes** — `verified` / `property-violated` / `inconclusive` are three distinct process exit codes (Doc 18 mapping; the §4.2-W4 / §5.2 exit-code contract);
   - **machine-readable `--json`** — structured property results + counterexample/witness traces, parseable by CI / Make / a factory build step;
   - **deterministic, zero-daemon** — no long-lived process, no port, no network surface; a `fsm verify <file>` invocation behaves exactly like `fsm check` / `fsm generate` (the existing proven pipeline shape).
4. **NO separate verification server. NO plugin architecture.** Rationale recorded so a future reader does not re-litigate:
   - **Batch verification for pipelines / a factory is a CLI invocation, not a service.** A factory CI calls `fsm verify` the way it calls `fsm check`; a service would add an attack surface, a lifecycle, and a deployment story for *zero* batch-pipeline benefit (the same reasoning §2.2.5 / OWNER-DECISION-1 used to defer the WS *simulator* server — a server is a *delivery vehicle for interactive debugging*, not the verification trust itself).
   - **"One core, many frontends" is this project's proven spine.** v1.2's "one analysis feeds all consumers" (Doc 26 §8; Doc 00 §11.34) means editor ≡ CLI *by construction*. The **LSP later surfaces verification in-editor by reusing the SAME `fsm-verify` library** — so VS Code ≡ CLI ≡ CI by construction, no divergence, no second implementation. The LSP already **IS** the long-lived process if interactive / incremental verification is ever wanted (a deferred DX concern, not a v1.4 concern) ⇒ **no new server is ever needed** — the question "separate vs same server" dissolves: the answer is "neither — a CLI now, the existing LSP process later, both reusing one lib".
   - **A plugin architecture is YAGNI and fights the fixed/auditable/factory-trustable property.** A verifier a factory trusts for safety must be a *fixed, auditable* artifact, not a plugin-loader with third-party verification logic. Custom properties, *if ever wanted*, are a library API + a `fsm verify` flag — **not** a runtime loader.
5. **Assembler codegen target = researched, deliberately-deferred non-goal.** The owner reasoned ~90–95% of binary targets have C compilers and C is the portable substrate; recorded as a **non-goal**, revisited *only* if a concrete no-C-compiler target emerges. (This is the §1-discipline "record the scope decision honestly" applied to a feature the owner explicitly mused about and then de-prioritized.)

### v1.4 tag-gate ADDITION (folds into §5.2 — see the pointer there)

The owner set an explicit bar **above** the §5.2 gate: *complete + factory-integratable pipeline BEFORE any UI*. The gate MUST therefore additionally require:

> **The full `fsm verify → fsm generate → fsm check` workflow is CLI-only usable for CI / factory integration** — machine-readable output (`--json`), an exit-code contract (verified / property-violated / inconclusive), deterministic, scriptable, with **zero UI dependency and no workflow gaps**. The verification pipeline is *fully closeable headless* before any VS Code / DX convenience work begins.

This is the governing **pipeline-before-UI** sequencing principle (memorialized as `[[feedback_embeded_fsm_pipeline_before_ui]]`): complete the full functional pipeline at the CLI + language-server layer (CI/factory-integratable) *before* the UI/DX convenience layer. **W4 (CLI/UX/docs) is sharpened accordingly** (see the §4.2-W4 + §5.2 pointers): W4's §5.4 acceptance is not just "the example deadlock is caught via the CLI" but "the *whole verify→generate→check* loop is demonstrably driveable by a CI script with machine-readable output and the documented exit codes, zero UI in the path".

> **Pointers into the unchanged analysis below** (the analysis is NOT rewritten; these mark where the confirmed decision lands): §1.3 / §1.5 (FSM-E0400/W0602 — confirmed to be honestly closed by A) · §2.1 + §2.3 (the cut — now CONFIRMED, not awaiting) · §4.2-W0 (placement — RESOLVED: `fsm-verify` is its own crate) · §4.1 keystone (CONFIRMED as an epic-level architecture invariant, not just the W1 instruction) · §4.2-W4 + §5.2 (the CLI-pipeline-complete / exit-code / `--json` gate addition).

---

## 1. ROADMAP-v1.4 ↔ shipped-reality reconciliation

The ROADMAP §"v1.4 — Simulation & verification" (`docs/ROADMAP.md:160-170`) lists four bullets + the deferred-from-v1.3 carry-overs (`docs/ROADMAP.md:155-156`). Each is checked against code at `a036c38`. **`file:line` is cited for every claim that is already-done, infeasible-as-stated, or drifted.**

### 1.1 Bullet: "Simulator WebSocket protocol (Doc 13): JSON-RPC 2.0 server … step-debugging, breakpoints, watchpoints"

| Aspect | ROADMAP claim | Shipped reality at `a036c38` | Drift verdict |
|---|---|---|---|
| Protocol *spec* | Doc 13 | Exists, normative, complete (slash-form methods, StepRecord schema, error codes) — `docs/13-Simulator-Protocol.md` | ✅ spec ready |
| WS server | "JSON-RPC 2.0 server" | **ABSENT.** `crates/fsm-simulator/src/lib.rs:3-7` explicitly: *"The WebSocket JSON-RPC server described in Doc 13 is deferred to v1.1+. No `tokio`, no `tower-lsp`, no `tungstenite`."* No `tungstenite`/`axum`/WS crate in `Cargo.toml` `[workspace.dependencies]` (only `tower-lsp`/`tokio`, for the LSP). No `fsm simulate` subcommand (`crates/fsm-cli/src/cli.rs:30-50` — only `Check/Generate/Test/Doc/Decompile`). | ⚠️ **Net-new subsystem + new network surface.** NOT "extend"; build-from-zero. The interpreter core it would wrap *is* shipped + correct. |
| Step-debugging seam | "step-debugging, breakpoints" | The interpreter has `snapshot()`/`restore()` (`crates/fsm-simulator/src/interpreter.rs:415-442`, `InterpreterSnapshot`) and per-step `StepRecord` emission — the *exact seam* a step-debugger consumes. No breakpoint/pause machinery yet (Doc 13 §5/§9). | Partial seam exists (snapshot/restore) — good news; breakpoints are net-new. |
| Security | (silent in ROADMAP) | Doc 00 §G-02 / §6 D-02 (`docs/00-…:130,146,803`): the v1.0 simulator-server security gap (`0.0.0.0`, no auth) was *removed by deletion*. Re-introducing a server **re-opens exactly that surface**. SEC-P0-1 (§11.28) is the project's precedent for the mandatory converge-not-duplicate security pass on any new file/network surface. | ⚠️ **Owner-decision (see §3.1).** Not a code drift, a scope-risk the ROADMAP omits. |

### 1.2 Bullet: "Web IDE (Doc 05): Monaco + WASM-compiled toolchain + ELK diagram"

| Aspect | ROADMAP claim | Shipped reality at `a036c38` | Drift verdict |
|---|---|---|---|
| `wasm32` `fsm-lsp` build | carried from v1.3 defer (`ROADMAP.md:155`) | **ABSENT.** No `wasm-bindgen`, no `wasm32` target, no `[workspace.dependencies]` wasm entry. The Cargo.toml comment (`Cargo.toml`, "Intentionally NOT included for v1.0: wasm-bindgen") is still in force. Doc 26 §9 says the analysis core *is* tokio-free / WASM-compatible *by design* — but compiling + packaging it is unstarted. | ⚠️ Net-new; large. The *design* is WASM-ready (true), the *build* does not exist. |
| Monaco editor | "Monaco editor" | **ABSENT.** No Monaco anywhere (`grep` only hits Cargo.lock transitive + the VS Code test-host's bundled Copilot, irrelevant). | ⚠️ Net-new front-end subsystem. |
| ELK diagram | "ELK-laid-out diagram" | **PARTIALLY shipped, wrong host.** v1.3-V4 shipped an elkjs ELK-Layered diagram — but as a *VS Code Webview* (`editors/vscode/`), not a browser Web IDE. Reusable conceptually; not a Web IDE. | The diagram *algorithm* is proven (elkjs in V4); the Web IDE shell is net-new. |
| Cross-file / workspace LSP intelligence | carried from v1.3 defer (`ROADMAP.md:156`) | **ABSENT by explicit contract.** Doc 26 §9 + ROADMAP `:140`: single-file was the explicit v1.2 contract; "no in-tree project-index substrate." Backlog confirms `workspaceSymbol` left un-advertised deliberately. | ⚠️ Net-new (the project-index substrate); paired with the Web IDE multi-file story per ROADMAP. |

### 1.3 Bullet: "Model checking integration: dispatch every reachable state×event pair, prove deadlock-free; report unreachable transitions (**extend the existing dead-transition analyzer**)"

**This is the most-drifted ROADMAP claim and the reason a naive plan would mis-scope.**

| Aspect | ROADMAP claim | Shipped reality at `a036c38` | Drift verdict |
|---|---|---|---|
| "the existing dead-transition analyzer" | implies a reachability engine to extend | **There is no reachability engine.** What exists: `crates/fsm-analyzer/src/checks/completion.rs:50-66` — a **67-line, purely textual** check that flags a `done -> X` appearing *after* an `[else]` completion in source order (FSM-W0101). It does **not** build a state graph, does **not** explore reachability, does **not** consider guards/events. | ⚠️ **"extend" is infeasible-as-stated.** There is nothing of the right shape to extend; the deadlock/reachability engine is **net-new**. |
| "report unreachable transitions" / unreachable states | implies an implemented check | **FSM-E0400 (Unreachable state) and FSM-W0602 (no-incoming-transitions) are catalog-reserved but UNIMPLEMENTED.** `grep -rln "E0400|W0602" crates/` → only `tests/negative.rs`, `fsm-diagnostics/src/lib.rs` (the enum), `completion.rs` (a *different* code, W0101), and a CLI fixture. **No emission site in any analyzer check** (`crates/fsm-analyzer/src/checks/mod.rs:run_all` lists 11 checks; none is reachability). The catalog entry `docs/10-…:642` itself hedges: *"Error (if compiler can prove statically), Warning (FSM-W0400 alias if only suspected)"* — i.e. the static prover was always future work. | ⚠️ **ROADMAP implies done; it is not.** This is the §1 headline drift. |
| "determinism" as model-checking | (ROADMAP frames model-checking as new) | `crates/fsm-analyzer/src/checks/determinism.rs` is real but **AST-local**: it groups transitions by `(source_state, trigger)` and proves *pairwise static guard disjointness* on shallow forms (`[A==X]` vs `[A==Y]`); module doc L18-19: *"Exact static disjointness is undecidable in general; we model 'obviously exclusive' forms … Anything else we conservatively flag."* It is **not** state-space exploration; it never executes the machine. | Accurate framing in ROADMAP (model-checking *is* new) — but it must not be confused with "extending determinism". |
| The codegen≡sim equivalence gate | (implied trustable oracle) | **The codegen↔simulator equivalence gate is a documented PLACEHOLDER.** `crates/fsm-simulator/tests/codegen_equivalence_smoke.rs:1-13` + `lib.rs:19-25`: *"placeholder for when codegen-c gains the instrumented hooks needed to observe its run-time behaviour (TODO post-v1.0)"*. The simulator is the *de facto* oracle (used as such by §11.26's W7-FU-1 fix) but a *mechanised* sim≡codegen differential does not exist. | ⚠️ Relevant to scope: "trustable behavioral validation" (the v1.4 theme) has a known gap here. Candidate v1.4 sub-item or explicit v1.5 defer. |

**The good news the ROADMAP undersells:** the interpreter (`crates/fsm-simulator/src/interpreter.rs`, 1852 LOC) is a *complete, deterministic, snapshot/restore-capable RTC engine* covering hierarchy, parallel regions, history, defer, timers, submachines (Doc 08 §12). Reachability/deadlock-detection should **drive this engine** as the transition oracle (consume the seam), NOT re-implement semantics in the analyzer — that is the keystone (see §4.2).

### 1.4 Bullet: "Trace replay & differential testing: capture a trace from one v1.x; replay against a new v1.y; flag any divergence"

| Aspect | ROADMAP claim | Shipped reality at `a036c38` | Drift verdict |
|---|---|---|---|
| Trace capture | "capture a trace" | ✅ **Fully shipped.** `crates/fsm-simulator/src/trace.rs`: `StepRecord` (Doc 13 §11 verbatim shape), `TraceFile`, `write_trace_yaml`, `parse_trace_yaml`, `execute_trace` with a per-step diff verdict (`TraceResult.first_mismatch`). `examples/.../capture_trace.rs` exists. | ✅ Done. |
| Replay + divergence flag | "replay … flag any divergence" | ✅ **The mechanism exists.** `execute_trace` (`trace.rs:280-334`) already compares actual vs `trace.expected` and returns `first_mismatch`. The conformance suite (`crates/fsm-cli/src/cmd/test.rs`) is *built on this*. What's missing is only the **cross-version harness** (run a captured baseline trace through the *current* build and assert no drift) + a CLI affordance. | ⚠️ **Mostly done; only the differential wrapper + CLI surface is net-new.** Lowest-risk item by far. |

### 1.5 Reconciliation summary

| ROADMAP v1.4 item | Real state | Net-new size | Risk | Recommended disposition |
|---|---|---|---|---|
| Simulator WebSocket server (Doc 13) | spec ✅, server ✗ | Large | **High (network surface)** | **Defer to v1.5** (own minor; security-gated) |
| Web IDE (Doc 05) + wasm32 + Monaco + project-index | nothing (design WASM-ready only) | Largest | **Highest (wasm build + new FE)** | **Defer to v1.6** (own minor; depends on v1.5 WS) |
| Model-check / reachability / **deadlock detection** | **nothing** (catalog-reserved, unimplemented; "extend" infeasible) | Medium | Medium (new semantics, but bounded; reuses interpreter) | **v1.4 core — narrowed cut** |
| Unreachable-state/transition report (FSM-E0400/W0602) | catalog-reserved, **unimplemented** | Small-Medium | Low-Medium (graph walk over IR) | **v1.4 core** (falls out of the reachability engine) |
| Trace differential replay | mechanism ✅, harness ✗ | Small | Low | **v1.4 core** |
| sim≡codegen mechanised equivalence | documented placeholder | Medium | Medium (needs codegen trace hooks) | **v1.4 stretch / candidate** (see §2 + §5) |

---

## 2. Scope decision + cut (product-strategy recommendation — owner confirms at review)

### 2.1 Recommended v1.4 cut: "Verification core" (zero new network/wasm surface)

> **This is a recommendation with rationale + explicitly-stated deferred alternatives, NOT a pre-commitment.** The owner confirms or re-cuts at the upcoming review (the ROADMAP's own "propose-with-rationale, orchestrator/owner weighs" mechanism, the §11.40 v1.2 re-scope precedent).
>
> **➤ CONFIRMED 2026-05-16 (see the "Owner scope-confirmation" section at the top).** The owner confirmed this cut **in full** — A + B ship as v1.4; WS → v1.5, Web IDE → v1.6, sim≡codegen → stretch/v1.5 all stand as recommended. The owner did NOT re-cut (the alternatives below were the re-cut *menu*; they were not taken). This subsection is no longer a "proposal awaiting confirmation" — it is the **decided** scope. The architecture (`fsm-verify` lib + CLI-canonical, no server, no plugin) and the CLI-pipeline-complete gate addition are recorded in that top section.

**v1.4 ships (proposed → CONFIRMED):**

- **A. Bounded explicit-state reachability + deadlock detection** — a new `fsm-verify` capability (crate or `fsm-analyzer` module — design decision deferred to W1, see §4) that, *driven by the shipped interpreter as the transition oracle*, performs a bounded BFS/DFS over the reachable configuration×context space and proves/​reports: (a) **deadlock** = a reachable non-final configuration with no enabled transition for any declared event and no pending timer/completion; (b) **unreachable states** (closes FSM-E0400/W0602 — they fall out of the same reachable-set); (c) **unreachable transitions** (declared transitions never taken on any reachable path). Surfaced as `fsm verify <file>` (new subcommand) + integrated diagnostics. **Bounded** = explicit owner-tunable state/step ceiling with an honest "bound reached, result is INCONCLUSIVE not PROVEN" verdict (never a false "proven deadlock-free").
- **B. Trace differential replay** — `fsm test --baseline <captured-trace-dir>`: re-run captured baseline traces through the *current* build's interpreter and fail on any `StepRecord` divergence (consumes the existing `execute_trace`/`first_mismatch` seam). Protects against silent semantic drift across releases (the cross-version commitment in `ROADMAP.md:197-199`).

**v1.4 explicitly does NOT ship (deferred — the alternatives, stated so the owner can re-cut):**

- The **simulator WebSocket/JSON-RPC server** (Doc 13) → **v1.5** (own minor). *Alternative if the owner prefers:* if interactive step-debugging in VS Code is the higher user priority than offline verification, v1.4 could instead be the WS-server minor and verification → v1.5. Rationale for *not* recommending this: it opens a network attack surface (§3.1) and the VS Code extension already ships static diagnostics + diagram; offline "prove no deadlock" is higher value-per-risk and has no new surface.
- The **Web IDE + wasm32 + Monaco + project-index** → **v1.6** (own minor; depends on the v1.5 WS server for the interactive trace). Largest/highest-risk; bundling it with verification would reproduce the mega-v1.2 anti-pattern the §11.40 re-scope explicitly rejected.
- **Mechanised sim≡codegen equivalence** (codegen trace-hook instrumentation) → **v1.4 stretch OR v1.5**. *Alternative:* fold a minimal form into v1.4 if W1 finds the codegen hook is cheap (it likely is not — it needs a host-side trace-emit shim in generated C; see §5). Default: defer, keep v1.4 tight.

### 2.2 Why this cut (rationale)

1. **Highest value-per-risk.** "Prove my embedded FSM cannot deadlock" + "prove this compiler release didn't silently change my machine's behaviour" are the two highest-trust-value claims for an *embedded* code generator, and neither needs a network port or a wasm build. The ROADMAP's own theme is *"Trustable behavioral validation"* — A+B are exactly that; the WS server/Web IDE are *delivery vehicles*, not the trust itself.
2. **Consumes shipped seams; net-new is bounded.** A drives the existing interpreter (the keystone, §4.2); B wraps the existing `execute_trace`. No new pipeline semantics, no new deps beyond possibly nothing (BFS over existing types). Contrast: WS server = new tokio server + WS dep + auth + network surface; Web IDE = wasm toolchain + Monaco + FE build.
3. **Preserves the validated cadence.** v1.0→v1.1→v1.2→v1.3 each shipped one cohesive, independently-valuable, small-tight-tagged increment (§11.40 codified this as the rule, not the exception). A+B is one cohesive theme ("verification"); WS and Web IDE are each their own cohesive theme. Three minors, not one mega-minor.
4. **De-risks the dependency chain.** The Web IDE *needs* the WS server (interactive trace) which *benefits from* a proven-correct verification core (the thing it visualises). Sequencing verification → WS → Web IDE front-loads the lowest-risk, highest-fan-in foundation, mirroring the post-v1.1 "LSP-first" reasoning (`ROADMAP.md:94-96`).
5. **Avoids two new attack/​build surfaces opening at once.** Opening a network surface (WS) *and* a wasm build *and* new verification semantics in one minor maximises audit surface at one tag. The cut isolates the only new-surface items into their own gated minors.

### 2.3 What the owner is being asked to confirm

> **➤ ANSWERED 2026-05-16 (top section).** Item 1 = **CONFIRMED as-recommended** (verification core A+B; WS→v1.5; Web IDE→v1.6). Item 2: OWNER-DECISION-1 (network surface) does not bite — the owner confirmed the no-server architecture, so there is **no new network surface in v1.4 at all** (option A holds *a fortiori*); OWNER-DECISION-2 (model-check compute envelope) keeps its non-blocking default (A: conservative bound, `INCONCLUSIVE`-honest, opt-in raise) — unchanged by the owner, the bounded-by-construction design proceeds.

- Confirm v1.4 = verification core (A+B), WS→v1.5, Web IDE→v1.6; OR re-cut (the §2.1 alternatives are the menu). — **CONFIRMED A+B as recommended.**
- Decide the two §3 owner-decisions (network surface posture; model-check compute envelope) — both have non-blocking defaults so planning/W1 is not stalled. — **#1 moot (no server architecture confirmed); #2 default A holds.**

---

## 3. Infra / owner-decision surfacing (crisp decisions, non-blocking defaults — per [[feedback_infra_constraint_escalation]])

> Both are surfaced as owner-decisions with a **non-blocking default** so the pipeline never stalls waiting for the owner (the escalation discipline; do NOT pre-decide).

### 3.1 OWNER-DECISION-1 — the simulator network attack surface (only bites if WS is pulled into v1.4)

- **The decision.** A simulator WebSocket server (Doc 13) is a *new network-listening attack surface* on the same threat model G-02 covers (`docs/00-…:767`: tooling on shared CI must be safe). Doc 13 §1 already constrains it (`localhost` only, 4 MB frame cap, binary-frame reject, 16-instance cap) and Doc 00 §G-02 records the v1.0 gap was *removed by deletion*. SEC-P0-1 (§11.28) is the binding precedent: **any new file/network surface gets a converge-not-duplicate security pass + a §5.4-grade behavioural security acceptance before its tag.**
- **Why it's an owner-decision, not a TL call.** The *engineering* axis (localhost-bind, no-auth-because-localhost, frame caps, converge on existing input-validation primitives) the TL owns and will simply apply per Doc 13 + SEC-P0-1 discipline. The *residual* — whether a localhost-only dev server is an acceptable posture *at all* on the shared box, vs requiring a token/handshake even on loopback — is a security-posture call the owner owns.
- **Options.** (A, **recommended default**) **WS is NOT in v1.4** (the §2.1 cut) → this decision does not bite until v1.5; v1.4 has zero new network surface; planning proceeds now. (B) If the owner re-cuts WS into v1.4: ship it localhost-only + frame-capped + a mandatory pre-tag security audit lens (the SEC-P0-1 pattern) — non-blocking for *planning*, but adds a security-audit gate to the v1.4 tag. (C) WS with a loopback handshake token even though localhost — higher assurance, more scope.
- **Non-blocking default:** (A). v1.4 planning + W1 proceed with zero network surface regardless of when the owner answers.

### 3.2 OWNER-DECISION-2 — model-checking compute/memory envelope on the shared 78G box

- **The decision.** Explicit-state reachability/exhaustive-exploration is potentially **heavy compute + memory** (configuration×context state-space can blow up). The box is shared across ~10 of the owner's projects; ground truth at this writing: **`/` 78G, 64G used, ~9.8G free, 87%** (`df -h /`). [[feedback_infra_constraint_escalation]] is explicitly marked: the earlier "ceiling lifted ~13–14G" note is **stale/over-optimistic**; current posture = **one build-heavy wave at a time**, §7 hygiene ON, read-only audit/design/Plan waves safe concurrently. A *runaway* model-checker (unbounded BFS holding millions of `InterpreterSnapshot`s in RAM) is a new memory-pressure vector on a box already disk-tight.
- **Why it's an owner-decision.** The *engineering* axis the TL owns and will apply unconditionally: the explorer is **bounded by construction** — an owner-tunable state/step/​wall ceiling, a default conservative bound, an explicit `INCONCLUSIVE` verdict when the bound is hit (never a false "proven"), and **no snapshot retained beyond the frontier** (DFS with on-pop drop, or hash-set of visited *configuration digests* not full snapshots). The *residual* — what the *default* bound should be on this shared box, and whether large-FSM exhaustive runs need a dedicated resource window — is the owner's infra call.
- **Options.** (A, **recommended default**) Ship with a **conservative default bound** (e.g. ≤ a few hundred-thousand visited configuration-digests / a wall-clock cap), `INCONCLUSIVE`-honest, opt-in `--max-states N` to raise it; the test-suite FSMs are tiny so CI is unaffected. (B) Owner provisions a dedicated scratch/memory window for large-FSM exhaustive verification (only needed if a real customer FSM exceeds the default bound — surface *then*, not now). (C) Cap hard + never allow raising it (simplest, least useful for large machines).
- **Non-blocking default:** (A). The bounded-by-construction design means no wave is ever blocked on this; the box is never put under runaway pressure; W1's acceptance proves the bound + the `INCONCLUSIVE` honesty on a deliberately-too-small bound.

---

## 4. Wave breakdown + depth-first sequencing

Each wave is a real §5.4 behavioural-acceptance gate. **For verification, §5.4 means *proven properties / real deadlock detection on real FSMs* — never symbol-presence, never "the function exists".** The acceptance for a deadlock detector is: *a hand-constructed FSM that genuinely deadlocks is flagged with the witnessing trace, and a known-deadlock-free FSM is proven clean within the bound* — cross-checked against the spec semantics (Doc 08), not merely against the simulator (the simulator can share a bug — §5.4's own "cross-checked against the spec, not just the simulator" clause).

### 4.1 Keystone / structural-foot-gun analysis (the recurring "consume the existing seam, don't fabricate")

The project's load-bearing recurring lesson (P0-1 non-functional lowering; the L2–L7 "seam-table is intent, derive from real types"; the V5 MV5-1 "wrong-seam foot-gun"; the W0 leave-and-explain): **build on the real seam; never fabricate a parallel implementation.** Applied to v1.4:

- **THE KEYSTONE: the reachability/deadlock engine MUST drive `fsm_simulator::Interpreter` as the transition oracle.** The interpreter (`interpreter.rs`) is the *single, spec-conformant, snapshot/restore-capable* RTC implementation. `Interpreter::new` → `init` → for each declared event `dispatch` → `snapshot()` to get the resulting configuration digest → `restore()` to backtrack. The visited-set key is the existing `InterpreterSnapshot` (`active_states` + `history` + `defer_set` + `context` + `virtual_clock_ms` — already `Clone`, already the complete state). **Foot-gun (FORBIDDEN):** re-implementing transition selection / LCA / completion in the analyzer to "explore the graph statically." That would be a *second semantics* — the exact P0-1 / "third line/col converter" / MV5-1 anti-pattern. The analyzer's `determinism.rs` deliberately does NOT execute; the verifier deliberately MUST execute (via the interpreter). The W1 brief must pin this explicitly.
- **Sub-foot-gun: the visited-set key.** Retaining full `InterpreterSnapshot`s for millions of states is the §3.2 memory foot-gun. The structural-correct move: hash the snapshot to a stable digest (the snapshot is already all-`Vec`/`BTreeMap`/scalar — deterministically serialisable, the Doc 13 §11 byte-stable contract `trace.rs` already relies on) and keep digests, materialising a snapshot only for the current frontier node + the witness path.
- **Sub-foot-gun: "deadlock" definition.** Must match Doc 08 RTC semantics exactly: a configuration is deadlocked iff no declared event enables any transition AND no completion event is pending AND no timer can ever fire (a state with only `after`/`every` timers is NOT deadlocked — the timer will fire). Naively "no event transition ⇒ deadlock" would false-positive every timer-driven wait state. W1 must derive this from Doc 08, cross-checked, not from intuition.
- **B (differential replay) seam:** `execute_trace(ir, trace)` already returns `TraceResult { first_mismatch }`. The differential harness is a *consumer* of this — it must NOT re-implement step comparison (the `StepRecord` `PartialEq` + `first_mismatch` is the existing oracle). Foot-gun: a parallel "trace comparator" with subtly different equality (e.g. ignoring `actions_executed`) would silently miss real drift.

### 4.2 Wave sequence (depth-first; each wave is independently mergeable + §5.4-gated)

> Depth-first = each wave delivers a *thin but complete vertical slice* with a real behavioural gate before the next builds on it (the v1.2 L1-spine / v1.3 V1-spine precedent), and a §11.3 phase-boundary audit after the keystone wave (W2) before downstream waves build on it.

- **W0 (gate prerequisite — small, may be folded into W1's brief).** Resolve the `fsm verify` *placement* decision (new `fsm-verify` crate vs `fsm-analyzer` module vs `fsm-simulator` module) by extraction against current code: the engine *depends on* `fsm-simulator` (the interpreter) AND emits `fsm_diagnostics::Diagnostic` AND wires into `fsm-cli`. *Recommendation to validate in W0/W1:* a new `fsm-verify` crate depending on `fsm-simulator` + `fsm-ir` + `fsm-diagnostics` (keeps `fsm-analyzer` — the most behaviourally-critical crate — untouched; the W0/§11.49 "don't churn the critical crate" precedent). No reachability check belongs in `fsm-analyzer` (it would drag the interpreter into the analyzer's dep graph — architecturally wrong). **W0 is design-confirmation only**; if trivial, fold into W1.
  > **➤ RESOLVED 2026-05-16 by the TL architecture decision (top section): the placement IS a new single `fsm-verify` crate** = the one source of truth for verification semantics, driving the shipped `fsm_simulator::Interpreter` as the oracle; `fsm-analyzer` stays untouched. W0 collapses to confirming the crate skeleton/dep edges; effectively folded into W1 (no open design question remains).

- **W1 (KEYSTONE — the bounded reachability spine).** Build the minimal complete vertical slice: drive the interpreter over the reachable configuration-digest space for a *flat, single-machine* FSM; detect (a) deadlock and surface ONE witnessing event trace, bounded with the `INCONCLUSIVE`-honest verdict. **§5.4 acceptance:** a hand-written `.fsm` that genuinely deadlocks (a state with one guarded transition whose guard is statically unsatisfiable on the reachable context) → `fsm verify` reports DEADLOCK + the exact event sequence to reach it, verified by hand against Doc 08; a known-clean FSM → PROVEN-CLEAN within bound; a too-small `--max-states` → INCONCLUSIVE (never a false PROVEN). Deadlock-free FSM proof cross-checked against Doc 08 semantics, not just "the simulator said so." **Self-contained implementer brief in §4.3.**

- **W2 (hierarchy/parallel/timer/submachine coverage + the unreachable-state/transition report).** Extend W1's explorer to the full interpreter feature set the interpreter already supports (composite/parallel/history/defer/timer/submachine — the interpreter handles all; the *explorer* must handle timer-fire and completion as exploration edges, per §4.1's deadlock-definition foot-gun). Emit FSM-E0400 (unreachable state) + FSM-W0602 (no-incoming) + unreachable-transition warnings from the computed reachable set — *closing the catalog-reserved-but-unimplemented codes (§1.3) honestly* (which also pays a real G7 conformance-coverage gain). **§5.4 acceptance:** a parallel-region FSM where one region can deadlock the join; a timer-only wait state correctly NOT flagged as deadlock; a genuinely unreachable state flagged FSM-E0400 with a conformance fixture. **§11.3 phase-boundary audit after W2** (the keystone+breadth boundary, before W3 builds on it) — independently re-derive that the explorer drives the interpreter and does NOT re-implement semantics.

- **W3 (trace differential replay — item B).** `fsm test --baseline <dir>`: capture-and-compare harness consuming `execute_trace`/`first_mismatch`. Includes a baseline-trace corpus captured from `a036c38` (the v1.3 semantics) committed as the drift oracle. **§5.4 acceptance:** a deliberately-mutated interpreter behaviour (in a test-only fixture) is caught as drift with the exact diverging `StepRecord`; an unchanged build passes clean; the baseline corpus round-trips. (Low-risk; could run parallel to W2 if disk allows — but §3.2 posture = one build-heavy wave at a time, so sequence after W2 unless the owner lifts the constraint.)

- **W4 (CLI/UX polish + diagnostics integration + docs).** `fsm verify` exit codes (Doc 18 mapping), `--max-states`/`--max-steps`/`--max-wall` flags, JSON output for the VS Code extension to *optionally* consume later, a worked `examples/verify/` showing a caught deadlock, Doc 13/Doc 10/ROADMAP/CHANGELOG/Doc 00 §11 reconciliation. **§5.4 acceptance:** the example deadlock FSM is caught end-to-end via the CLI with the documented exit code.
  > **➤ SHARPENED 2026-05-16 by the owner pipeline-before-UI bar (top section + §5.2 gate addition).** W4 is the wave that makes the **whole pipeline factory-integratable BEFORE any UI**, so its scope and §5.4 acceptance are explicitly raised: (1) the three exit codes are the **distinct verified / property-violated / inconclusive contract** (not a generic 0/1); (2) `--json` is a **first-class machine-readable** mode (property results + counterexample/witness traces), specified as the contract a CI/Make/factory step parses — *not* "for the VS Code extension to optionally consume" framing only; (3) the §5.4 acceptance is upgraded from "the example deadlock is caught via the CLI" to **"the full `fsm verify → fsm generate → fsm check` workflow is driveable by a CI/factory script end-to-end with machine-readable output and the documented exit-code contract, deterministic, with ZERO UI dependency and no workflow gaps"** — a worked CI-shaped script (or `examples/verify/` + a documented invocation recipe) demonstrating the headless closed loop is part of W4's deliverable. This is the load-bearing "pipeline complete + factory-integratable before UI" gate ([[feedback_embeded_fsm_pipeline_before_ui]]).

- **Pre-tag:** independent four-lens pre-tag audit (the v1.1/v1.2/v1.3 §11.30 pattern) → `GATE_VERIFICATION_v1_4.md` → cold-from-source quad at the gate-doc commit → tag (§11.30 commit-ordering, strictly).

### 4.3 W1 implementer brief (self-contained — "you are the implementer, do NOT delegate further")

> Per [[feedback_subagent_r1_misread]]: the implementer must be told explicitly it is the implementer. This brief is self-contained.

```
ROLE: You are a senior verification/compiler engineer implementing the keystone
wave (W1) of the FSM Studio v1.4 "verification core" epic. YOU ARE THE
IMPLEMENTER — read, edit, build, test directly. You do NOT delegate further.
The TL/PM delegation discipline (R1) applies to the main thread that spawned
you, NOT to you. Keep changes surgical; respect the scope boundary; run the
verification quad before commit; no `git push`, no `--no-verify`.

REPO ROOT: /root/dev/embeded-fsm-sdk
GIT SETUP: git worktree add /root/dev/embeded-fsm-sdk-wt-v14-w1 \
  -b phase4.1/v1_4-w1-reachability-spine <main-HEAD-at-dispatch>
  (work in that worktree; commit there; the orchestrator merges.)
DISK: check `df -h /` first; ~9-10G free, shared box, ONE build-heavy wave at
  a time (do not run parallel cargo). Dev builds only, no --release. If disk
  tightens DURING the wave, first reclaim is `rm -rf <shared-target>/debug/
  incremental` — NEVER touch another worktree.

READ FIRST (only these — do not read the whole repo):
- docs/30-v1_4-Simulation-Verification-Wave-Plan.md §1.3, §4.1, §4.2-W1 (this
  plan; your scope + the keystone constraint + the foot-guns).
- docs/08-Formal-Execution-Semantics.md §3.1 (the RTC step), §3.2 (internal
  queue / completion), §9 (completion) — the NORMATIVE definition of "a step"
  and therefore of "deadlock". Your deadlock definition is DERIVED from here,
  cross-checked, NOT from intuition.
- crates/fsm-simulator/src/lib.rs (the public surface) + interpreter.rs:
  `Interpreter::{new,init,dispatch,advance_clock,snapshot,restore,
  current_states,virtual_clock_ms}`, `InitOptions`, `InterpreterSnapshot`,
  `StepError`. THIS IS YOUR ORACLE.
- crates/fsm-simulator/src/trace.rs `StepRecord` (the witness format).
- docs/processes/SUBAGENT_CONVENTIONS.md §4 (verification quad), §5.4
  (behavioural-acceptance — MANDATORY for this wave).

SCOPE (W1 only):
Implement a NEW crate `fsm-verify` (workspace member) exposing a bounded
explicit-state reachability + deadlock detector for a FLAT single-machine
FSM (composite/parallel/timer/submachine are W2 — do NOT attempt them; the
interpreter supports them but YOUR EXPLORER in W1 covers flat machines so the
spine is proven before breadth). API shape (adjust names as the code
demands, keep it minimal):
  pub struct VerifyOptions { pub max_states: usize /* default conservative */,
                             pub max_steps: usize, pub machine_name: String }
  pub enum Verdict { ProvenNoDeadlock, Deadlock { witness: Vec<StepRecord>,
                     config: Vec<String> }, Inconclusive { reason: String } }
  pub fn verify(ir: &fsm_ir::Ir, opts: VerifyOptions) -> Result<Verdict, _>

THE KEYSTONE (non-negotiable — this is why the wave exists):
- The explorer DRIVES `fsm_simulator::Interpreter`. For each frontier
  configuration: `restore()` the interpreter to it, then for EACH declared
  event try `dispatch(event)`; the resulting `snapshot()` is the successor.
  Backtrack via `restore()`. You MUST NOT re-implement transition selection,
  LCA, guard evaluation, or completion — that is a forbidden second
  semantics (the P0-1 / parallel-resolver anti-pattern this project keeps
  re-learning). If you find the interpreter API insufficient, REPORT IT in
  the completion notes (the orchestrator extends scope) — do NOT fork the
  semantics.
- Visited-set key = a STABLE DIGEST of `InterpreterSnapshot`, NOT the full
  snapshot retained. The snapshot is all-Vec/BTreeMap/scalar → serialise it
  deterministically (the same byte-stable contract trace.rs relies on) and
  hash. Retain full snapshots ONLY for the current frontier + the witness
  path. (This is the §3.2 memory foot-gun — bounded by construction.)
- "Deadlock" (derive from Doc 08, cross-check, state your derivation in a
  code comment + completion notes): a reachable configuration that is NOT a
  final configuration AND for which NO declared event enables any transition
  AND no completion is pending. NOTE: W1 is flat machines with NO timers in
  the deadlock fixture (timer-as-non-deadlock is a W2 concern) — but write
  the definition so W2 can extend it; do not hardcode "no event ⇒ deadlock"
  in a way that will be wrong once timers are explored.
- Bounded + HONEST: on hitting `max_states`/`max_steps`, return
  `Inconclusive` with a reason. NEVER return `ProvenNoDeadlock` when the
  bound was hit. A false "proven" is the cardinal verification sin (the
  symbol-presence-equivalent for a prover).

TESTS (§5.4 behavioural-acceptance — REQUIRED; symbol-presence is forbidden
as the sole guard):
Create `crates/fsm-verify/tests/reachability_acceptance.rs`:
1. `genuine_deadlock_is_detected_with_witness`: a hand-written flat `.fsm`
   (real source, parsed+analyzed via the real pipeline) with a state whose
   only outgoing transition has a guard unsatisfiable on the reachable
   context → assert `Verdict::Deadlock`, assert the witness event sequence
   actually reaches that config (replay it through a fresh Interpreter and
   check). Hand-verify the deadlock against Doc 08 and put the reasoning in
   a comment (cross-checked against the SPEC, not just the simulator).
2. `deadlock_free_machine_proven_within_bound`: motor-style start/stop flat
   FSM → `Verdict::ProvenNoDeadlock` with a generous bound.
3. `too_small_bound_is_inconclusive_not_false_proven`: case 2's FSM with
   `max_states: 1` → `Verdict::Inconclusive`, and ASSERT it is NOT
   `ProvenNoDeadlock` (the false-proven guard).
4. `explorer_uses_interpreter_not_a_fork` (structural): a transition with a
   guard + an action; assert the verifier's reachability honours the guard
   EXACTLY as the interpreter does (construct an FSM where a fork-semantics
   bug would diverge — e.g. default-priority 100 — and assert the verifier
   agrees with a direct `Interpreter` run). This is the keystone proof.

VERIFICATION QUAD (before commit, all green):
  cargo build --workspace
  cargo test --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --check --all
New crate MUST carry `#![forbid(unsafe_code)]` (workspace convention, 11
roots/10 crates → becomes 12/11) and `[lints] workspace = true`.

SCOPE BOUNDARY — DO NOT TOUCH:
- crates/fsm-analyzer/** (the reachability check does NOT go here — it would
  drag the interpreter into the analyzer dep graph; W0 decided fsm-verify is
  its own crate). DO NOT add a reachability check to fsm-analyzer.
- crates/fsm-simulator/** EXCEPT: you may add a `pub fn` to expose a
  snapshot digest IF AND ONLY IF one is genuinely needed and not derivable
  externally — and you must REPORT this in completion notes as a judgment
  call (prefer deriving the digest in fsm-verify from the public
  InterpreterSnapshot fields; only touch fsm-simulator if those fields are
  insufficient).
- W2 surface: composite/parallel/history/timer/submachine exploration,
  FSM-E0400/W0602 emission, CLI wiring (`fsm verify` subcommand) — all W2+.
  W1 is library-only + its acceptance tests.
- Cargo.toml workspace members list: you ADD `crates/fsm-verify` (that is in
  scope — a new member needs the workspace entry); do NOT touch other deps.

COMMIT MESSAGE (polish allowed, structure must match):
```
Add fsm-verify: bounded explicit-state reachability + deadlock spine (W1)

W1 of the v1.4 verification-core epic. Introduces `fsm-verify`, a bounded
explicit-state explorer that DRIVES the shipped `fsm_simulator::Interpreter`
as the transition oracle (no re-implemented semantics — the keystone
constraint, Doc 30 §4.1). Detects deadlock on flat single-machine FSMs with
a witnessing trace; bounded by an owner-tunable state/step ceiling with an
INCONCLUSIVE-honest verdict (never a false proven). Visited-set keyed by a
stable digest of InterpreterSnapshot, not retained full snapshots (the §3.2
memory-bound design). Composite/parallel/timer/submachine + diagnostics +
CLI are W2+.

Per docs/30-v1_4-Simulation-Verification-Wave-Plan.md §4.2-W1 / §4.3.

Verified: cargo build/test/clippy/fmt all green workspace-wide.
```

COMPLETION REPORT (report back): branch; LOC delta; new test count + that
each FAILS without the engine / PASSES with it; the exact deadlock
definition you derived from Doc 08 and how you cross-checked it against the
spec (not just the simulator); whether you needed to touch fsm-simulator and
why; any interpreter-API insufficiency found; judgment calls (§8 format).
```

---

## 5. Risks + v1.4-tag gate criteria

### 5.1 Risks

| # | Risk | Likelihood | Mitigation |
|---|---|---|---|
| R1 | **State-space explosion** on real FSMs (context-valued configs) | High for large machines | Bounded-by-construction + INCONCLUSIVE-honest (§3.2 / §4.3). Default conservative bound; opt-in raise. Never a false "proven". |
| R2 | **Semantics fork** — the explorer re-implements transition selection instead of driving the interpreter (the P0-1 / MV5-1 class) | Medium (tempting for "static" speed) | The keystone constraint (§4.1) pinned in the W1 brief + the W2 §11.3 phase-audit independently re-deriving "drives the interpreter, no fork" from source. |
| R3 | **Wrong "deadlock" definition** — false-positive on timer-wait states | Medium | Derive from Doc 08, cross-check vs spec not simulator (§4.1 sub-foot-gun, §4.3); W2 acceptance explicitly tests a timer-only wait state is NOT flagged. |
| R4 | **Memory pressure on the shared box** from retained snapshots | Medium | Digest-keyed visited set; full snapshots only for frontier + witness (§3.2/§4.1). §3.2 owner-decision default is non-blocking. |
| R5 | **Scope creep** — WS server / Web IDE pulled back in mid-epic | Medium (ROADMAP bundles them) | The §2 cut is an explicit owner-confirmed boundary; the §2.1 alternatives are the *menu* for re-cutting at review, not mid-epic drift. |
| R6 | **Differential-replay false confidence** — comparator ignores a `StepRecord` field | Low | W3 consumes `execute_trace`/`first_mismatch` (the existing oracle), forbidden to re-implement step comparison (§4.1 B-seam). |
| R7 | **sim≡codegen gap left implicit** — "trustable validation" theme but the codegen-equivalence placeholder (§1.3) ships unaddressed | Medium | Explicitly scoped: it is a *named* v1.4-stretch / v1.5 item (§2.1), recorded in the gate doc as accepted-tracked, NOT silently omitted (the §11.49 leave-and-explain precedent applied to scope). |
| R8 | **ROADMAP not reconciled** → future reader believes FSM-E0400 etc. shipped | Low (this doc) | §1 is the reconciliation; W4 updates ROADMAP/Doc 10/Doc 00 §11 to mark E0400/W0602 *implemented in v1.4* (closes the §1.3 drift in the record). |

### 5.2 v1.4-tag gate criteria

> **➤ ADDITION 2026-05-16 (owner pipeline-before-UI bar — top section).** The single new gate row below (the **CLI-only pipeline-complete** criterion) is added on top of the existing list; everything else stands. The "No new network surface" row is sharpened to reflect the confirmed no-server architecture.

- **The full verify → generate → check workflow is CLI-only usable for CI / factory integration** — `fsm verify` emits **machine-readable `--json`** (property results + counterexample/witness traces) and a **distinct exit-code contract** (verified / property-violated / inconclusive, Doc 18 mapping), is **deterministic**, **scriptable**, with **zero UI dependency and no workflow gaps**: a CI/factory script can drive `fsm verify → fsm generate → fsm check` end-to-end headless, demonstrated by a worked recipe/example (§4.2-W4 sharpened acceptance). This is the owner's "pipeline complete + factory-integratable BEFORE any UI" bar ([[feedback_embeded_fsm_pipeline_before_ui]]) — a hard tag-gate, not a nice-to-have.
- **0 open P0**, ≤5 well-scoped P1 in the independent pre-tag four-lens audit (the v1.1/v1.2/v1.3 standard).
- **§5.4 behavioural-acceptance on every wave** — for verification: a *genuinely deadlocking FSM is caught with a valid witness*, a *clean FSM is proven within bound*, a *too-small bound is INCONCLUSIVE not false-proven*, all cross-checked against Doc 08 (not just the simulator). Symbol-presence is not acceptance for a prover.
- **The keystone proven structurally** (post-W2 §11.3 phase-audit): the explorer drives `fsm_simulator::Interpreter`; there is NO second transition-selection / guard-eval / completion implementation (independently re-derived from source, the §11.29 verify-the-audit-too rigor on the riskiest wave).
- **FSM-E0400 / FSM-W0602 honestly closed**: implemented + conformance-fixtured (a real G7 gain), and the catalog/ROADMAP/Doc 00 §11 reconciled to say so (the §1.3 drift fixed in the record).
- **Trace differential replay proven**: a mutated behaviour is caught with the exact diverging `StepRecord`; the `a036c38` baseline corpus round-trips clean.
- **Cold-from-source quad green at the gate-doc commit** (§11.22), §11.30 commit-ordering strictly (gate-doc commit → cold quad at THAT commit → tag at THAT commit; the 3rd-consecutive prospective-clean release).
- `#![forbid(unsafe_code)]` extended to `fsm-verify` (workspace 12 roots / 11 crates).
- **No new network surface** (the §2 cut, **CONFIRMED** — and the TL architecture decision makes this structural: `fsm-verify` is a library + a `fsm verify` CLI subcommand, **no server, no daemon, no plugin host** ⇒ OWNER-DECISION-1 cannot bite in v1.4; the WS *simulator* server stays a v1.5 concern with its own SEC-P0-1 gate then). If a future minor re-introduces any listener, the SEC-P0-1-grade security audit lens is a gate for *that* minor (OWNER-DECISION-1 option B), not v1.4.
- **Carry-overs honoured/recorded** (not necessarily resolved — the v1.3 pattern): G9 CI-never-run + the JS lane (owner push); the v1.3.x `makeNonce`/JC-3 tracked items; R-1..R-4 cst residuals — re-stated in `GATE_VERIFICATION_v1_4.md` §"carried owner-escalations" so they are not silently dropped (the §11.62 discipline).
- **CHANGELOG `[1.4.0]`** + Doc 00 §11 rows (v1.4 rows start **§11.63** — next free after §11.62) + ROADMAP retrospective.

---

## 6. See also

- `docs/ROADMAP.md` §"v1.4" (the re-scoped scope this doc reconciles) + §"How the roadmap evolves" (the propose-with-rationale mechanism this doc uses)
- `docs/13-Simulator-Protocol.md` (the WS spec — stays normative; deferred to v1.5 per the §2 cut)
- `docs/08-Formal-Execution-Semantics.md` (the normative "step"/deadlock source for W1)
- `crates/fsm-simulator/src/interpreter.rs` (the keystone oracle), `crates/fsm-simulator/src/trace.rs` (the witness/diff seam)
- `crates/fsm-analyzer/src/checks/{determinism,completion}.rs` (the *AST-local* checks the ROADMAP mis-references as "the dead-transition analyzer to extend")
- `docs/processes/SUBAGENT_CONVENTIONS.md` (§5.4 acceptance, §11.3 phase-audit, §11.22 cold-quad, §11.30 tag-ordering)
- `docs/GATE_VERIFICATION_v1_3.md` (the just-shipped gate + carried owner-escalations v1.4 must re-state)
- `docs/00-Decisions-And-Reconciliation.md` §11 (decision ledger; v1.4 rows start §11.63)

*End of FSM-PLAN-SIMVER-V14 v1.0.0 — awaits owner scope confirmation at review before any v1.4 implementer wave.*
