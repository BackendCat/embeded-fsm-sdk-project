# §11.3 Post-W1 Phase-Boundary Audit — v1.4 Verification Core (W1: `fsm-verify` spine)

**Document ID:** FSM-AUDIT-PHASE-V14-W1
**Version:** 1.0.0 (frozen evidence doc — never overwrite; a new audit is a new versioned file)
**Audit type:** §11.3 phase-boundary audit (Doc 30-mandated gate before any W2 dispatch). READ-ONLY judgment.
**Audited commit:** `dd2d41e` (`Merge phase4.1/v1_4-w1-impl`; W1 impl = `2866821`).
**Worktree / branch:** `/root/dev/embeded-fsm-sdk-wt-w1audit` on `phase4.1/v1_4-w1-audit` (cut from `dd2d41e`).
**Auditor:** independent phase-boundary auditor (no `cargo`/build run — verdict derived by reading source + docs + `git`/`grep`, per the read-only mandate).
**Date:** 2026-05-16.
**Governing record:** Doc 30 (§1.3, §3.2, §4.1, §4.2, §4.3, §5.2 + the "Owner scope-confirmation + TL architecture decision" section), Doc 00 §11.63, `[[feedback_embeded_fsm_pipeline_before_ui]]`.

> **Sequencing note (recorded up front — see Drift D-1).** Doc 30 §4.2 / §5.2 / R2 textually place the §11.3 phase-audit **after W2** ("the keystone+breadth boundary"). The orchestrator re-sequenced it to **post-W1** because (a) W1 is the verification-core keystone — it is the wave that realises the "drive the Interpreter, never fork the semantics" architecture invariant the whole epic stands on, and (b) W1 shipped materially more than the §4.3 "library-only" sub-brief (the CLI + the FSM-E0400/W0602 emission), per the **owner-confirmed authoritative brief** (Doc 00 §11.63 + the pipeline-before-UI bar). Auditing the keystone *before* W2 builds on it is the stronger application of the §11.3 discipline (verify the spine before the breadth wave consumes it), not a deviation from intent. This audit treats W1 as the keystone wave accordingly.

---

## 0. Verdict (read this first)

**W2-readiness verdict: `PROCEED-WITH-NOTES`.**

W1 is a genuinely sound keystone. The explorer is **pure orchestration over `fsm_simulator::Interpreter`** — there is **no second transition-selection / guard-eval / LCA / completion semantics anywhere in `crates/fsm-verify/`** (independently re-derived from source + the acceptance suite drives the real `Interpreter` as a cross-check oracle). The factory/CLI contract (`fsm verify`, exit-code map, `--json` schema `fsm-verify/v1`) is a stable, documented, versioned, additive contract a factory CI can integrate against today, and the full headless `verify → generate → check` loop is behaviourally proven. The keystone is structurally durable against the W2 re-implementation temptation **for the part W2 controls**.

**The one load-bearing W2-blocker (a NOTE, not a W1 defect):** the `fsm_simulator::InterpreterSnapshot` / `snapshot()` / `restore()` seam is **lossy for exactly the state W2 adds** — it does **not** capture the live submachine sub-instance configs (`RuntimeState.submachines`) nor the armed timer set (`RuntimeState.timers`). W1 is *correctly and provably* fenced off from this (it loud-rejects composite/parallel/history/timer/submachine via `reject_non_flat`), so W1 ships sound. **But W2 cannot "just extend the explorer through snapshot/restore" as Doc 30 §4.1 implies** — driving the Interpreter via `restore(snapshot)` over a submachine/timer config would silently drop the nested sub-instance + timers, the digest would **conflate behaviourally-distinct configurations**, the visited-set would prune prematurely, and the explorer could return a **false `ProvenNoDeadlock`** (the cardinal verification sin) on precisely the hierarchical/timer FSMs W2 targets. **W2's first task is therefore a `fsm-simulator` change (extend `InterpreterSnapshot`/`snapshot()`/`restore()` to round-trip `submachines` + `timers`), gated by a snapshot-completeness round-trip test, *before* the explorer is extended.** Doc 30 §4.1's "`InterpreterSnapshot` … already the complete state" is an **overstatement** that, left unflagged, would walk W2 straight into the foot-gun the keystone exists to prevent. This is the headline finding and the mandatory W2-briefing input (§4).

No P0 against W1 as shipped. The snapshot-completeness gap is a **P0 for W2** (must be the first W2 unit) and is recorded here so W2 dispatches against a fixed, known prerequisite rather than discovering it mid-wave.

---

## 1. Lens 1 — W1-spine soundness for W2/W3 without rework

### 1.1 The keystone: zero re-implemented semantics (independently re-derived — PASS)

**Claim under audit:** the explorer drives `fsm_simulator::Interpreter` as the sole transition oracle and re-implements no transition selection / guard / LCA / completion.

**Re-derivation from source (not trusted from the report):**

- `crates/fsm-verify/src/engine.rs:200-340` — the entire exploration loop. Every successor is produced by exactly the seam Doc 30 §4.1 mandates:
  - `Interpreter::new(ir)` + `interp.init(init_opts)` (engine.rs:201-207) — build/init the oracle once.
  - Per frontier node: `interp.restore(node.snap.clone())` (engine.rs:234) — put the oracle *at* that config.
  - Per declared event: `interp.restore(node.snap.clone())` then `interp.dispatch(ev)` then `interp.snapshot()` (engine.rs:255-259) — the **real RTC step is the Interpreter's**; the successor *is* `interp.snapshot()`.
  - Backtrack = pure `restore` (engine.rs:255). No re-implemented backtracking.
- The **only** IR reads in the whole crate are structural/semantics-free, and each is annotated as such:
  - declared-event name list: `machine.events.iter().map(|e| e.name.clone())` (engine.rs:198).
  - `final`-state discriminator: `is_final_state` walks `StateNode::Final` (deadlock.rs:76-101) — a data discriminator, not behaviour.
  - declared-state shape for reachability/diagnostics: `declared_concrete_states` (reachability.rs:42-75), `concrete_states`/`transition_target_ids`/`initial_target_ids` (diagnostics.rs:105-200) — pure structural shape reads.
- **Negative check:** grepped `crates/fsm-verify/src/` for any guard/transition/LCA/priority evaluation. There is **none**. No `select_transition`, no guard eval, no LCA, no priority/doc-order logic, no completion drain — those tokens appear only in *prose* (module docs explaining the constraint) and in `fsm-simulator`. The deadlock predicate is *operational*: "no declared event changed the digest AND not a final config" (engine.rs:301) — it never decides *which* transition is enabled; it asks the Interpreter by dispatching and observing.
- **The dep graph proves it structurally** (`crates/fsm-verify/Cargo.toml`): runtime deps = `fsm-simulator` (the oracle), `fsm-ir` (structural reads), `fsm-diagnostics`, `serde_json`, `thiserror`. `fsm-analyzer` is a **dev-dependency only** (test fixtures use the real parse+analyze pipeline). The architecture decision (Doc 00 §11.63: `fsm-analyzer` untouched, interpreter not dragged into the analyzer graph) held exactly.

**The acceptance suite is itself a structural keystone proof, not symbol-presence** (`crates/fsm-verify/tests/reachability_acceptance.rs:284-371`, `verifier_agrees_with_interpreter_oracle`): it constructs a machine with two same-`(source,event)` transitions whose guards are statically disjoint (`gate==1 -> Wrong` vs `gate==0 -> B`, `gate` defaults 0 and is never assigned), runs a **direct `Interpreter`** to establish the oracle truth (`A --EV--> B`, `Wrong` unsatisfiable), then asserts the verifier reaches `B`, **never** `Wrong`, and that the verifier's whole reachable set **byte-equals an independently hand-driven interpreter BFS**. A forked guard-ignoring selector would reach `Wrong` and the test would fail. This is the §4.1 keystone realised and proven the strongest way (apply-and-compare against the real oracle, not range asserts). The deadlock test (lines 64-126) likewise replays the witness through a **fresh `Interpreter`** and asserts the deadlocked config + no-progress against the real semantics.

**Finding 1.1: PASS — keystone independently verified from source.** No second semantics exists. The acceptance suite cross-checks against the real Interpreter and against Doc 08 by hand (the deadlock predicate derivation is in `deadlock.rs:1-57`, chained to Doc 08 §1/§3.1/§9, not intuition).

### 1.2 Keystone durability against the W2 re-implementation temptation — the load-bearing finding

The W2 question Doc 30 poses: is the event-enumeration/exploration seam designed so W2 extends it **through the Interpreter** (timer-fire / completion edges driven via the simulator, NOT a re-implemented enabled-set/clock)?

**The explorer's seam is correctly shaped for this.** The frontier loop (engine.rs:231-323) is event-agnostic in structure: "for each exploration edge from this config, `restore` → produce-successor → digest → enqueue-if-new". W2's timer-fire edge is *structurally* just another edge whose successor is produced by the Interpreter — the Interpreter already exposes `advance_clock(delta_ms)` (interpreter.rs:285, in the public seam Doc 30 §4.3 lists) which runs the *real* timer-fire RTC step. W2 adds "for the relevant clock deltas, also try `advance_clock`" alongside "for each declared event, `dispatch`", and `snapshot()` is still the successor. **There is no design corner in `fsm-verify` that forces a fork** — the loop does not assume edges are only `dispatch(event)`; it only assumes "an edge mutates the oracle and `snapshot()` reads the result". `engine.rs:399-402`'s own comment explicitly names this future seam ("it does not *yet* enumerate timer-fire / submachine-completion edges").

**BUT the durability is conditional on a `fsm-simulator` change W1 cannot make and Doc 30 mis-states as already-done.** See §1.3 — this is the headline.

### 1.3 Digest W2-safety — the headline blocker (NOTE → P0 for W2)

Doc 30 §4.1 (line 192) asserts: *"The visited-set key is the existing `InterpreterSnapshot` (`active_states` + `history` + `defer_set` + `context` + `virtual_clock_ms` — already `Clone`, **already the complete state**)."* and §3.2/§4.1 frame the digest as W2-safe.

**This is verified FALSE for W2's scope, from source:**

- `RuntimeState` (the live config — `crates/fsm-simulator/src/runtime/state.rs:25-72`) has, beyond the seven snapshot fields:
  - **`submachines: BTreeMap<String, Box<RuntimeState>>`** (state.rs:62-71) — the live, value-owned nested sub-instance configs. Per Doc 08 §12 a submachine ref-state's configuration *includes the nested sub-instance's own active leaves / context / timers / history*. Added to `RuntimeState` at `8b66dc2` (v1.1-W2c submachine runtime).
  - **`timers: TimerSet`** (state.rs:46-47) — the armed timer set.
- `InterpreterSnapshot` (state.rs:132-141) captures **only** `active_states, history, defer_set, virtual_clock_ms, context, next_trace_id, initialized`. `snapshot()` (interpreter.rs:417-425) and `restore()` (interpreter.rs:432-442) copy exactly those — **`submachines` and `timers` are dropped on snapshot and not restored.**
- `git blame`: `InterpreterSnapshot` was authored at `faf0db06` (2026-05-14, original simulator); `RuntimeState.submachines` was added later at `8b66dc2` (v1.1-W2c) and **`InterpreterSnapshot` was never extended to follow**. This is a **pre-existing `fsm-simulator` limitation, not W1-introduced.**

**Why this is a true W2 soundness blocker (not cosmetic):** W2 extends the explorer to composite/parallel/history/**timer/submachine**. For a submachine-bearing or timer-bearing config, two reachable configurations that differ *only* in the nested sub-instance's active leaf (or in armed timers / remaining timer phase) would serialise to **identical** canonical bytes (because `canonical_bytes`, digest.rs:48-56, serialises the *snapshot*, which omits both). The `ConfigDigest` would collide them, `visited.insert` (engine.rs:276) would treat the second as already-seen, the explorer would **prune a genuinely-unexplored part of the state space**, and the frontier could empty with the verdict `ProvenNoDeadlock` (engine.rs:335-339) — a **false proof on a truncated-without-knowing search**. That is the exact cardinal verification sin the honest-bound invariant exists to prevent, re-entering through the *digest* instead of the *bound*. `restore(snapshot)` is *also* lossy the same way (it would resurrect a submachine config with its sub-instances silently torn down), so even the *exploration* (not just the visited-set) would be wrong.

**W1 itself is honest and sound here — the gap is fenced, not hidden:**

- `reject_non_flat` (engine.rs:403-431) **loud-rejects** `submachines`, `Composite`, `Parallel`, `History`, `Submachine`, and any `Simple` state with non-empty `timers`, returning `VerifyError::OutOfW1Scope` (STOP-not-wrong). The CLI maps this to exit 4 with a clear message (verify.rs:184-189; acceptance `out_of_w1_scope_exits_four_loudly`, cli_verify.rs:176-191).
- `digest.rs:21-26`'s collision contract enumerates *exactly the snapshot fields* ("same active leaves, history, defer set, virtual clock, and context") and claims **no more** — it does **not** overstate that it captures sub-instances or timers. `digest.rs` is locally honest.
- `deadlock.rs:40-57` (the W2 forward-compat note) correctly says W2 adds timer-fire/submachine-completion as exploration edges and that the flat-machine guard keeps W1 sound "without baking a wrong rule into this module."

So the **W1 code does not lie**; the gap is the *upstream snapshot seam* + Doc 30 §4.1's "already the complete state" overstatement, which is **not flagged anywhere** (not in Doc 30 §4.2-W2, not in the backlog, not in `ASSESSMENT_PRODUCT_AND_DOCS_2026_05_16.md`, not in any W1 code comment as a W2 *prerequisite*). An audit that found nothing on the keystone wave would have missed exactly this — it is the one place the epic's spine is weaker than its own planning doc claims.

**Finding 1.3: NOTE → mandatory W2 P0.** The digest is W2-safe **only after** `InterpreterSnapshot`/`snapshot()`/`restore()` are extended to round-trip `submachines` (recursively) + `timers`. W2 MUST do this in `fsm-simulator` **as its first unit**, gated by a snapshot→restore→re-snapshot byte-identity round-trip test over a submachine + timer config, *before* relaxing `reject_non_flat`. (Touching `fsm-simulator` is a deliberate, declared judgment call — Doc 30 §4.3's scope-boundary explicitly anticipates "you may add a `pub fn`/extend the snapshot IF the fields are insufficient … REPORT it"; here the W1 brief correctly chose **not** to touch it because W1's flat scope does not need it, and reported the boundary by loud-rejecting. W2's brief must now own the snapshot extension explicitly.)

### 1.4 W3 (trace differential replay) buildability on the existing seam — PASS

Doc 30 §1.4 / §4.2-W3 claims `execute_trace`/`first_mismatch` already ships and W3 is a thin consumer. Verified: `crates/fsm-simulator/src/trace.rs:245-331` — `pub struct TraceResult { … first_mismatch: Option<usize> }`, `pub fn execute_trace(ir, trace) -> Result<TraceResult, ExecError>` with per-step compare + tail-length-mismatch handling, plus `parse_trace_yaml`/`write_trace_yaml`. The seam is exactly as Doc 30 described; W3 is cleanly buildable on it as a *consumer* (the §4.1 B-seam constraint — do not re-implement step comparison — is enforceable because the comparator already exists and is the conformance suite's own oracle). **No W1 decision impedes W3.** W3 is independent of the §1.3 snapshot gap (it replays traces, it does not snapshot/restore-explore).

**Finding 1.4: PASS.** W3 has no W1-introduced rework risk.

---

## 2. Lens 2 — Factory / CLI contract durability (the owner's binding tag-gate)

The owner's hard v1.4 gate (Doc 00 §11.63, `[[feedback_embeded_fsm_pipeline_before_ui]]`, Doc 30 §5.2-added row): the full `fsm verify → generate → check` workflow CLI-only, machine-readable, deterministic, exit-code-contract, factory-integratable, zero UI, no gaps.

### 2.1 Exit-code contract — stable, documented, distinct (PASS)

`crates/fsm-cli/src/cmd/verify.rs:10-32` documents the contract in-source as a table; the implementation matches it exactly (verify.rs:146-156, 177-195):

| Exit | Meaning | Source-verified site |
|---|---|---|
| 0 | verified | `ProvenNoDeadlock` ∧ no FSM-E0400 (verify.rs:148) |
| 1 | property-violated | `Deadlock` ∨ any FSM-E0400 (verify.rs:147,149) |
| 2 | inconclusive (explicitly **not** verified) | `Inconclusive` (verify.rs:150) |
| 3 | input not found/unreadable | `read_to_string_capped` err (verify.rs:84-90) |
| 4 | does not parse/analyze, OR out-of-W1-scope/unknown-machine/no-machines (usage, kept distinct from a verdict) | verify.rs:98-113, 177-195 |

The 0/1/2 are **distinct first-class verdict codes**, not a generic 0/1 — satisfying the §5.2 explicit rejection of a generic exit. The clap-usage-vs-verdict exit-2 collision is reasoned about and shown non-colliding (verify.rs:25-32: clap emits 2 *before* this module runs). The out-of-scope path returns **4 (usage)**, deliberately *not* a verdict code, so a factory script never confuses "I asked for the wrong thing / W2 feature" with "the model is unsafe" (verify.rs:177-189) — this is the right call and is behaviourally tested.

Behavioural proof (real binary, `assert_cmd`): `cli_verify.rs` covers exit 0 (`sound_machine_exits_zero_verified`), 1 (`deadlock_exits_one_with_witness_in_json`, `unreachable_state_reports_fsm_e0400_and_exits_one`), 2 (`too_small_bound_exits_two_inconclusive_not_verified`), 3 (`missing_file_exits_three`), 4 (`unanalyzable_model_exits_four`, `out_of_w1_scope_exits_four_loudly`). This is genuine acceptance, not symbol-presence.

### 2.2 `--json` schema `fsm-verify/v1` — versioned, deterministic, additive-extensible (PASS, with a W2 contract note)

`verify.rs:34-67` documents the schema in-source; `build_json` (verify.rs:197-281) emits it via `serde_json::Map` (sorted keys) + stable array order. It is **explicitly versioned**: `root.insert("schema", "fsm-verify/v1")` (verify.rs:247). Determinism is behaviourally proven byte-for-byte across two runs on two fixtures (`json_is_byte_equal_across_two_runs`, cli_verify.rs:135-148), backed by the `BTreeSet`/`BTreeMap` reproducibility discipline throughout (`ReachabilityReport.reachable` is a `BTreeSet`, diagnostics sorted by `(code, span.start)` at diagnostics.rs:90).

**W2 extends this WITHOUT breaking it, by construction:** new properties (timer-liveness, parallel-join-deadlock, unreachable-transition) are **new keys under `properties`** and new diagnostics in the existing `diagnostics` array — purely **additive**. The `properties` object is open (verify.rs:252-263 builds a `Map` and inserts named properties); adding `properties.parallelJoin` etc. does not alter `deadlockFree`/`reachability`. **Recommendation (not a blocker):** W2 should keep `schema: "fsm-verify/v1"` for additive changes (a consumer reading `properties.deadlockFree` is unaffected) and only bump to `v2` if an existing field's *meaning or shape* changes. This convention should be **stated explicitly in the schema doc-comment** in W2 (currently the doc says `v1` but does not state the additive-vs-breaking versioning policy a factory integrator needs). That is the one contract gap W2 should close to make the tag-gate's "stable contract a factory CI integrates against" fully self-documenting.

### 2.3 Headless `verify → generate → check` loop — proven (PASS)

`full_verify_generate_check_pipeline_is_headless_driveable` (cli_verify.rs:197-234) drives the **real** binary: `verify --json` → parse `verdict` programmatically → only-if-verified `generate --target c99 --out <tmp>` → `check`, all with zero UI, machine-readable output, documented exit codes. This is exactly the owner's pipeline-before-UI bar demonstrated at the W1 slice. The loop closes headless **for W1's flat scope**.

**Finding 2: PASS.** The factory/CLI contract is stable, documented, versioned, deterministic, behaviourally proven, and additively extensible. **One contract gap W2-W4 must close:** state the `fsm-verify/vN` additive-vs-breaking versioning policy explicitly in the schema doc-comment (W2), and ensure W2's new properties are additive under `v1` (so existing CI integrations do not break). No other gap. (The pipeline is currently *complete for flat machines*; "no workflow gaps" at the v1.4 tag depends on W2 closing the hierarchy/timer scope so the loop is not silently flat-only — tracked as the normal W2 deliverable, not a W1 defect.)

---

## 3. Lens 3 — FSM-E0400 / W0602 honest-close + drift

### 3.1 The honest-close is genuine and not overstated (PASS)

`crates/fsm-verify/src/diagnostics.rs` is the real emission site (Doc 30 §1.3's catalog-reserved-but-unimplemented codes):

- **FSM-E0400 is gated on exhaustive search** (diagnostics.rs:56-72): `let exhaustive = matches!(stop_reason, StopReason::Exhausted); if exhaustive { … emit E0400 for unreachable … }`. On a bound-truncated search **no E0400 is emitted** — the message even states "(proven by exhaustive bounded exploration)". This is the *dual* of the never-false-proven invariant ("this state is unreachable" from a truncated search would itself be a false-proven) and it is implemented correctly and tested (`no_e0400_when_search_was_truncated`, diagnostics.rs:264-295: asserts truncated ⇒ no E0400 but W0602 still fires).
- **FSM-W0602 is the structural always-safe subset** (diagnostics.rs:74-87): a declared concrete state with zero incoming transition edges (`transition_target_ids`) and not an initial target (`initial_target_ids`) — sound regardless of search exhaustiveness. Correctly emitted unconditionally and tested.
- Coherent layering: a state can get both (no-incoming ⇒ W0602; exhaustive ⇒ also E0400); de-dup is the CLI's concern and it surfaces both, most-severe-first (diagnostics.rs:27-30, 90). The CLI maps any emitted E0400 (severity Error) to verdict `property-violated` / exit 1 and `reachability.result: "violated"` only when exhaustive (verify.rs:143,220-226). On a truncated search `reachability.result` is `"inconclusive"` (verify.rs:221-226) — honest.

This is a **coherent, non-overstated** close: the strong claim (E0400) is proof-gated; the weak claim (W0602) is structural. It pays the promised G7 conformance gain honestly (real emission sites, real `.fsm` fixtures: `unreachable_island.fsm`).

### 3.2 Drift findings

- **D-1 (tracked-here, expected): the §11.3-audit sequencing + the W1-sub-brief CLI/diagnostics-scope expansion.** Doc 30 §4.2-W1 / §4.3 sub-brief scoped the CLI subcommand + FSM-E0400/W0602 emission to **W2** ("W1 is library-only + its acceptance tests"), and the §4.3 API sketch typed `Verdict::Deadlock { witness: Vec<StepRecord>, … }`. W1 as shipped: (a) **ships the `fsm verify` CLI** + the FSM-E0400/W0602 emission + reachability report; (b) types `Verdict::Deadlock { report: DeadlockReport, witness: Vec<String> }` (declared-event-name witness, not `Vec<StepRecord>`). **This is authorised, not a violation:** the W1 commit message (`2866821`) explicitly cites "the top-section TL architecture decision (fsm-verify lib + CLI-canonical) + the pipeline-before-UI bar (§5.2 gate addition)" — i.e., the **owner-confirmed authoritative brief** (Doc 00 §11.63) supersedes the older §4.3 "library-only" cut, because the owner's hard gate is a *complete CLI-integratable pipeline*; deferring the CLI to W2 would have left W1 a library with no headless contract to gate. The `Vec<String>` witness is the *better* choice (a declared-event sequence is directly replayable and is what the witness semantically *is*; `StepRecord` would couple the verifier's public API to the trace record shape unnecessarily — and the §4.3 sketch said "adjust names as the code demands"). **However:** this reconciliation is recorded **only in the commit message**. It is **NOT reflected in Doc 30 §4.2 (still says CLI is W2), the backlog (line 12 still "next = v1.4-W1 dispatch"; line 34 still "post-W2 §11.3 phase-audit"), or any §11 row.** The backlog/Doc-30 are stale relative to shipped reality. **Disposition:** this is the expected "the authoritative brief evolved past the sub-brief" situation (the v1.2/v1.3 precedent) and is **acceptable-tracked, not a blocker** — but the **v1.4 batched closeout MUST** (i) annotate Doc 30 §4.2-W1/§4.3 that the CLI + E0400/W0602 landed in W1 per §11.63 (not W2), (ii) re-state the §11.3-audit-after-W1 re-sequencing, (iii) update the backlog status line + wave-plan line. Recorded here so it is not silently dropped (the §11.62 discipline).
- **D-2 (the headline, from §1.3): Doc 30 §4.1 "`InterpreterSnapshot` … already the complete state" is an overstatement.** It is true for flat machines, false for W2's submachine/timer scope. Not a W1 code defect (W1 fences it) but a **planning-doc drift that would mis-lead W2**. Disposition: **must be corrected in Doc 30 (or the W2 brief)** before W2 dispatch — see §4. This is the one drift that is load-bearing rather than bookkeeping.
- **No other doc↔shipped drift introduced by W1.** `#![forbid(unsafe_code)]` roots verified = **12** (matches the §5.2 gate "12 roots / 11 crates"; `fsm-lang-server` bin has 2 roots). `crates/fsm-verify` is in the workspace `members`. `[lints] workspace = true` present (fsm-verify/Cargo.toml:13-14). The §11 implementation rows are correctly **DEFERRED to the v1.4 batched closeout** (Doc 30 §5.2 / §11.63; v1.4 rows start §11.64; the v1.3 batched-closeout precedent) — **nothing mid-stream is needed**; §11.63 (the last applied row) is the scope/architecture row and is accurate.

**Finding 3: PASS on the honest-close (coherent, not overstated). Two drifts: D-1 (expected, acceptable-tracked, fix in closeout) and D-2 (load-bearing, fix before W2).**

---

## 4. Lens 4 — W2-readiness verdict + mandatory W2 briefing inputs

**Verdict: `PROCEED-WITH-NOTES`.** W1 is a sound keystone; W2 may be planned, but the W2 brief MUST carry the following **mandatory inputs** (without them W2 walks into the foot-gun the keystone exists to prevent):

1. **THE KEYSTONE INVARIANT, re-stated for W2 (the recurring "consume the seam, don't fabricate", now for timer/parallel/submachine):** W2 MUST produce every new exploration edge **through the `fsm_simulator::Interpreter`** — timer-fire edges via `interp.advance_clock(delta)`, completion/submachine-completion edges via the Interpreter's existing completion drain — and read the successor via `interp.snapshot()`. W2 MUST NOT re-implement an enabled-set, a clock, a timer-fire rule, a join rule, or a completion rule in `fsm-verify`. The `verifier_agrees_with_interpreter_oracle`-style cross-check (drive a direct `Interpreter`/hand-BFS and assert byte-equality of the reachable set) MUST be extended to a parallel + a timer + a submachine fixture as W2's keystone proof.

2. **THE SNAPSHOT-COMPLETENESS PREREQUISITE (P0, W2's *first* unit — the headline blocker):** before relaxing `reject_non_flat`, W2 MUST extend `fsm_simulator::InterpreterSnapshot` + `Interpreter::snapshot()` + `Interpreter::restore()` to **round-trip `RuntimeState.submachines` (recursively — it is `BTreeMap<String, Box<RuntimeState>>`) and `RuntimeState.timers`**, gated by a `snapshot → restore → re-snapshot` byte-identity round-trip test over (a) a submachine config mid-sub-instance and (b) an armed-timer config mid-phase. Until this lands, the `ConfigDigest` is **unsound for submachine/timer configs** (silent config conflation → premature visited-set pruning → **false `ProvenNoDeadlock`**). This is a declared, owner-anticipated `fsm-simulator` touch (Doc 30 §4.3 scope-boundary explicitly allows it "IF the fields are insufficient" — they are). It is the reason the verdict is PROCEED-**WITH-NOTES**, not a clean PROCEED.

3. **THE DIGEST CONSTRAINT W2 INHERITS:** `digest.rs::canonical_bytes` zeroes `next_trace_id` and serialises the snapshot. Once the snapshot carries `submachines`/`timers`, W2 MUST audit that *all* of the newly-added snapshot fields are either configuration-relevant (and thus correctly keyed) or book-keeping (and thus excluded like `next_trace_id`), and extend `digest.rs`'s test module (the `trace_id_is_not_part_of_the_key` / `distinct_*_differs` pattern) to cover a sub-instance-differing and a timer-differing pair. The digest's soundness is "byte-equal iff same reachable configuration"; W2 must preserve that biconditional for the enlarged config.

4. **THE SCHEMA CONSTRAINT W2 INHERITS (factory-contract):** new W2 properties go in **additively** under `properties` keeping `schema: "fsm-verify/v1"`; W2 MUST add an explicit additive-vs-breaking versioning-policy sentence to the `--json` schema doc-comment (verify.rs:34-67) so a factory integrator can rely on it. Only a meaning/shape change to an *existing* field bumps to `v2`.

5. **THE DEPTH-FIRST SEQUENCING:** W2's internal order MUST be **(2) snapshot-completeness + round-trip test → relax `reject_non_flat` incrementally (composite → parallel → history → timer → submachine, each with its own §5.4 acceptance) → the keystone cross-check on each → the FSM-E0400/W0602 + unreachable-transition extension**. Do NOT relax `reject_non_flat` before the snapshot is sound (that ordering inversion is the foot-gun). W3 is independent of all of this and can follow W2 (or parallel if disk allows, per Doc 30 §3.2 — but the §3.2 one-build-heavy-wave posture sequences it after W2 by default).

6. **DOC-30 / BACKLOG RECONCILIATION owed (D-1 + D-2):** before W2 dispatch, the orchestrator should correct Doc 30 §4.1 ("already the complete state" → flag the submachine/timer snapshot gap as W2's first prerequisite) and §4.2-W1/§4.3 (CLI + E0400/W0602 landed in W1 per §11.63; §11.3-audit re-sequenced to post-W1), and refresh the backlog status/wave-plan lines. D-2's correction is **load-bearing** (it is the W2 brief's most important input); D-1's is bookkeeping but owed in the v1.4 closeout regardless.

**Carry-overs:** none introduced by W1. The pre-existing v1.3 carry-overs (G9 CI-never-run, the JS lane, `makeNonce`/JC-3, R-1..R-4 cst residuals) are unchanged and remain for the `GATE_VERIFICATION_v1_4.md` "carried owner-escalations" section per Doc 30 §5.2 — W1 neither touched nor regressed them.

---

## 5. Evidence index (the spot-checks that prove the code was read)

| Claim | Evidence (file:line) |
|---|---|
| Explorer drives Interpreter; no second semantics | engine.rs:200-340 (build/init/restore/dispatch/snapshot/restore loop); Cargo.toml deps (fsm-analyzer dev-only) |
| Only structural IR reads | engine.rs:198 (events), deadlock.rs:76-101 (`final`), reachability.rs:42-75, diagnostics.rs:105-200 |
| Keystone proven behaviourally vs real Interpreter | reachability_acceptance.rs:284-371 (`verifier_agrees_with_interpreter_oracle` — byte-equal vs hand BFS); 64-126 (witness replayed through fresh Interpreter) |
| Snapshot omits submachines + timers (the W2 blocker) | runtime/state.rs:62-71 (`submachines`), :46-47 (`timers`) vs :132-141 (`InterpreterSnapshot`); interpreter.rs:417-425 (`snapshot`), :432-442 (`restore`); `git blame` faf0db06 vs `8b66dc2` |
| W1 fences the gap (sound) | engine.rs:403-431 (`reject_non_flat`); cli_verify.rs:176-191 (`out_of_w1_scope_exits_four_loudly`) |
| Digest honest about its own coverage | digest.rs:21-26 (collision contract enumerates exactly snapshot fields), :48-56 (`canonical_bytes`) |
| Exit-code contract documented + matches impl + tested | verify.rs:10-32 (doc), :146-195 (impl); cli_verify.rs (exit 0/1/2/3/4 each behaviourally tested) |
| `--json` versioned, deterministic, additive | verify.rs:34-67 (schema doc), :197-281 (`build_json`, `schema: fsm-verify/v1`); cli_verify.rs:135-148 (byte-equal x2) |
| Headless verify→generate→check proven | cli_verify.rs:197-234 |
| FSM-E0400 exhaustive-gated; W0602 structural | diagnostics.rs:56-72 (E0400 gate), :74-87 (W0602); :264-295 (truncated⇒no E0400 test) |
| W3 seam exists | trace.rs:245-331 (`TraceResult.first_mismatch`, `execute_trace`) |
| forbid(unsafe) = 12 roots; fsm-verify in workspace | 12 lib/main roots with `#![forbid(unsafe_code)]`; root Cargo.toml `members` includes `crates/fsm-verify` |
| §11 rows correctly deferred | Doc 00 last applied row §11.63 (scope/arch, accurate); v1.4 impl rows start §11.64 (closeout-batched) |

---

## 6. Summary for the orchestrator

- **W2-readiness: PROCEED-WITH-NOTES.** W1 is a sound keystone — ship-quality, no P0 against it as shipped.
- **W1-spine soundness:** the keystone is real and independently re-derived from source (zero re-implemented semantics; acceptance suite cross-checks the real Interpreter). The exploration seam is correctly shaped so W2 extends it *through* the Interpreter (timer-fire via `advance_clock`, completion via the existing drain) — **no design corner forces a fork in `fsm-verify`**. W3 is cleanly buildable on the existing `execute_trace`/`first_mismatch` seam.
- **THE headline finding (digest W2-safety):** `InterpreterSnapshot`/`snapshot()`/`restore()` is **lossy for `RuntimeState.submachines` + `timers`** (a pre-existing `fsm-simulator` limitation, fenced by W1's loud-reject so W1 is sound). Doc 30 §4.1's "`InterpreterSnapshot` … already the complete state" is an **overstatement** for W2's scope. Left unflagged, W2's digest would conflate submachine/timer configs → premature pruning → **false `ProvenNoDeadlock`** (the cardinal sin). **W2's first unit MUST be extending the snapshot seam (recursive `submachines` + `timers`) gated by a round-trip byte-identity test, *before* relaxing `reject_non_flat`.** This is the load-bearing W2-briefing input (§4 items 2 & 5).
- **Factory-contract durability:** stable, documented, versioned (`fsm-verify/v1`), deterministic (byte-equal proven), additively extensible; full headless verify→generate→check proven for flat scope. **One gap W2-W4 must close:** state the additive-vs-breaking `fsm-verify/vN` versioning policy explicitly in the `--json` schema doc-comment, and keep W2's new properties additive under `v1`.
- **Drift:** D-1 — the §11.3-audit was re-sequenced to post-W1 and W1 shipped the CLI + E0400/W0602 (authorised by the owner-confirmed brief / §11.63; the `Vec<String>` witness is the better choice) but Doc 30 §4.2/§4.3 + the backlog are stale; acceptable-tracked, **must be reconciled in the v1.4 batched closeout**. D-2 — the §4.1 "complete state" overstatement; **load-bearing, must be corrected before W2 dispatch** (it is the W2 brief's most important input).
- **Branch:** `phase4.1/v1_4-w1-audit` (this doc only; not merged/pushed; no implementation). For orchestrator review before merge → W2 dispatch.

*End of FSM-AUDIT-PHASE-V14-W1 v1.0.0 — frozen evidence; do not overwrite.*
