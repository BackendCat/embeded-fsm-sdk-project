# AUDIT_PHASE_DEBUG_W4_2026_05_19 — the binding tag-gate keystone phase-audit (interactive debug-interface epic)

> **Frozen evidence doc** (the `AUDIT_PHASE_*` never-overwritten convention). This is the
> **mandatory source-derived keystone re-derivation** for the interactive debug-interface
> epic — the v1.4-W1 / v1.5-W-A2 / factory-W6a precedent, the **binding tag-gate** of
> Doc 33 §2. Per the v1.5-closeout discipline: *a `KEYSTONE-INTACT` verdict that merely
> echoes a wave's own claim is worse than a found problem.* Every load-bearing claim below
> is the auditor's **independent re-derivation** — a command run, a diff computed, lines
> read, a test re-run — explicitly **NOT echoed** from Doc 33, the design docs, or any
> wave's self-report. The auditor did NOT implement debug-W1/W2/W3/W4a.

- **Audit target commit:** `8092d3a02a7bd6a6ee9d3dad306580d9b3019a0b` (`8092d3a`,
  `feat(debug-w4a): capture→.trace.json — recorded-from-the-oracle via the shipped
  write_trace_yaml, byte-identical by construction`)
- **Worktree:** `/root/dev/embeded-fsm-sdk-wt-w4b-keystone`, branch
  `phase8.0/w4b-keystone-audit` (off main `8092d3a`). READ-ONLY on code; no
  `git stash`; no merge/tag/push.
- **Toolchain (asserted FROM the worktree, mandate requirement):**
  `1.75.0-x86_64-unknown-linux-gnu (overridden by
  '/root/dev/embeded-fsm-sdk-wt-w4b-keystone/rust-toolchain.toml')` — the repo pin.
  Bare `rustc --version` → `1.75.0` here as well (the box default would be 1.95;
  1.75 from a bare invocation is benign and consistent with the pin, NOT drift).
- **Release anchor (independently resolved):** the mandate's anchor
  `checkpoint/factory-reliability-ci-2026-05-18` is an **annotated tag**; `git rev-parse
  <tag>` returns the tag-object SHA `a598fb6`, but `<tag>^{commit}` correctly
  dereferences to **commit `fbae38b1ccf7eb00fb344b0a034d16973254add9`** (`fbae38b`,
  `docs(factory-w6c): the §11.30 GATE_VERIFICATION …`). The mandate's anchor commit
  `fbae38b` is therefore correct — there is **no anchor discrepancy** (the apparent
  mismatch is the annotated-tag-object vs the commit it points at).
- **Epic-arc completeness:** `git merge-base --is-ancestor fbae38b 8092d3a` → **TRUE**
  (independently verified) — the EMPTY-core-diff below spans the entire debug-epic arc
  W0→W4a.
- **Auditor environment:** host `gcc 12.2.0` present (so the host differential is a
  **HARD** test here, not a skip); `xvfb-run` present (the ExtHost suite ran headless);
  `arm-none-eabi-gcc` **ABSENT** — **not needed for this audit** (the debug epic is a
  host-layer LSP+webview surface; no on-target lane is in scope). Disk 5.3 G free at
  the end (the shared `/root/dev/embeded-fsm-sdk-target` dir was used; no `cargo
  clean -p` needed; npm-ci was the only large write — `npm`, not `pnpm`, per the
  project rule).

---

## VERDICT: **KEYSTONE-INTACT**

> Re-derived from source + re-run by the auditor — explicitly **NOT echoed**.
> Zero fork-findings. The tag-gate Doc 33 §2 KEYSTONE row **PASSES**.

**Gate rationale.** All four binding obligations of the Doc 33 §2 keystone (the v1.4
§11.63 / v1.5 KEYSTONE-IN-UI / factory KEYSTONE-DRIVE-THE-ORACLE invariant) are
independently satisfied at `8092d3a`:

1. **Structural oracle-untouched (factory-W6a method).** Over the entire epic arc
   `fbae38b..8092d3a`, the only delta under `crates/fsm-simulator/src/` +
   `crates/fsm-cli/src/cmd/test.rs` is the **single sanctioned additive non-stepping
   `Interpreter::set_context_field` helper** (W1, commit `1de83b0`, +25 lines, no
   `StepRecord` by construction). `cmd/test.rs`'s `execute_trace` seam,
   `fsm-simulator/src/trace.rs` (`write_trace_yaml` + `execute_trace`), and every
   transition/guard/step/clock/queue/timer/LCA/completion/history module are
   **blob-SHA byte-identical** — auditor-recomputed per-file.
2. **Negative-grep ∅.** Over the WHOLE debug surface (`simulate.rs` 693 ln +
   `editors/vscode/src/debug/**` 2946 ln incl. `webview/`), there is **no
   definition** of transition-selection / guard-evaluation /
   completion-event-synthesis / RTC-step / active-configuration-computation. Every
   grep hit was read and adjudicated a disclaiming module-doc, a comment, an
   error-label projection (`StepError::GuardEval(_) => "guard-eval"`), a
   StepRecord-field read-projection, the documented breakpoint predicate, or the
   pure IR-JSON edge-id↔stableId bridge. The known adjudicated-clean shapes are all
   confirmed present and clean; no fork found.
3. **Differential / byte-equality, re-RUN by the auditor (not echoed).** The
   host-trace differential is **deterministic green 3×** (`7 passed; 0 failed; 2
   ignored`); the catalogue partitions exactly **11 BYTE_EQUAL + 3
   BEHAVIOURALLY_EQUIVALENT_JUSTIFIED + 0 KNOWN_DIVERGENT = 14 == CORPUS.len()`,
   dynamically asserted in-test (never-game-the-catalogue intact); the W1
   `fsm/simulate` LSP acceptance is **7 passed, 0 failed** (twice); the ExtHost
   suite is **47 passing, 0 failing, exit 0** under `xvfb-run`, including the three
   keystone-critical W2/W3/§W4 byte-match-vs-oracle tests.
4. **§W4 non-vacuity, independently confirmed.** The captured artifact is a
   **`.trace.json`** (not `.trace.yaml`); `fsm test` discovers ONLY
   `*.trace`/`*.trace.json` (re-derived from `cmd/test.rs:159`, byte-identical); an
   independent hand-run `fsm test <dir>` on a trace carrying the oracle's `expected`
   reports a real **`pass` … `1 passed`** (NOT `skip`/nothing-to-verify). The W4a
   `capture` op is **one `write_trace_yaml` call + serde marshalling, zero
   `Interpreter`/semantics**, and rejects empty-`expected` at the source.

A forked oracle / a re-implemented step-transition-guard / a synthesized completion /
a client-side time-travel reconstruction / a catalogue-integrity violation / a
vacuous (`skip`) §W4 replay — the cardinal regression — is **absent**.

---

## §1 — Structural oracle-untouched (independently re-derived; the near-EMPTY core diff)

**Command run by the auditor (NOT echoed):**

```
$ git diff --stat fbae38b..8092d3a -- crates/fsm-simulator/ crates/fsm-cli/src/cmd/test.rs
 crates/fsm-simulator/src/interpreter.rs            | 25 +++++++++
 crates/fsm-simulator/tests/codegen_equivalence_smoke.rs | 64 +++++++++++++--
 2 files changed, 85 insertions(+), 4 deletions(-)
```

**`crates/fsm-cli/src/cmd/test.rs` — BYTE-IDENTICAL.** It is **not in the diff at
all**. Blob-SHA recomputed:

```
$ git rev-parse fbae38b:crates/fsm-cli/src/cmd/test.rs   → 6ffb91aa4a17a9917100328526d1c395dbf9948a
$ git rev-parse 8092d3a:crates/fsm-cli/src/cmd/test.rs   → 6ffb91aa4a17a9917100328526d1c395dbf9948a   (IDENTICAL)
$ git log --oneline fbae38b..8092d3a -- crates/fsm-cli/src/cmd/test.rs   → <empty: never touched>
```

The `execute_trace` seam (`cmd/test.rs:223 execute_trace(&ir, &trace)`; the sole
oracle invocation in `fsm test`, `use fsm_simulator::{execute_trace, …}` at `:37`)
is pristine across the whole epic arc.

**`crates/fsm-simulator/src/` oracle semantic core — blob-SHA per file (auditor-run):**

| file | fbae38b → 8092d3a |
|---|---|
| `src/eval/{arith,expr,extern_registry,mod,stmt}.rs` | **IDENTICAL** (all 5) |
| `src/runtime/{completion,event,lca,machine_index,mod,queue,state,submachine,timer,value}.rs` | **IDENTICAL** (all 10) |
| `src/trace.rs` (`write_trace_yaml:274` + `execute_trace:280`) | **IDENTICAL** |
| `src/lib.rs` | **IDENTICAL** |
| `src/interpreter.rs` | **CHANGED** `383929d… → ca5fc02…` (the ONE sanctioned delta) |

`git log --oneline fbae38b..8092d3a -- crates/fsm-simulator/src/` → **exactly one
commit**: `1de83b0 feat(debug-w1): … + the non-stepping set_context_field helper`.
`-- crates/fsm-simulator/src/trace.rs` → **empty (never touched)**.

**The sole `interpreter.rs` delta (full hunk read):** a single additive method
`Interpreter::set_context_field(&mut self, name: &str, value: Value) -> Result<(),
StepError>` at lines 421–426, body **exactly 3 lines**:
`let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;` →
`rt.context.insert(name.to_owned(), value);` → `Ok(())`. This is **structurally the
same nature** as `Interpreter::init`'s already-shipped `rt.context.insert(f.name…, v)`
at lines 157/161 (raw `HashMap::insert` into `rt.context`, same `NotInitialized`
guard). It runs **no** step/transition/guard/clock/queue/timer logic; the return type
`Result<(), StepError>` carries **no `StepRecord` by construction**. This is precisely
the single permitted additive non-stepping helper the mandate sanctions. **CLEAN.**

**`crates/fsm-simulator/tests/codegen_equivalence_smoke.rs` (the +64/−4) —
adjudicated CLEAN, NOT an oracle change.** Provenance (`git log` for the file):
`3e4ec53 fix(fw-f1): unify const-resolver …` + `9ecb2eb fix(f2): codegen+sim honor
the IR queue{} config …` — **factory-wave producer fixes, NOT debug-epic (W1–W4a)
changes**. The diff is purely: (a) two corpus members added to `CORPUS` and
`BYTE_EQUAL` (`timer-const-ref`, `queue-overflow`); (b) human-readable comments. It
modifies **no** `run_differential` logic, **no** oracle invocation, **no** FSM
semantics — it only **expands** the differential test surface, and the dynamic
partition-arithmetic assertion was correspondingly updated (`11 + 3 + 0 = 14 ==
CORPUS.len()`). This *strengthens* the no-fork guarantee; it does not weaken it. A
test under `crates/fsm-simulator/tests/` is in the mandate's scope (`crates/
fsm-simulator/`), examined, and clean.

**§1 conclusion:** the ONLY permitted delta is present and exactly the sanctioned W1
non-stepping helper; the seam + the oracle semantic core + `write_trace_yaml` are
byte-identical across the entire epic arc. **STRUCTURALLY ORACLE-UNTOUCHED.**

---

## §2 — Negative-grep ∅ for forked semantics (the WHOLE debug surface)

Surface enumerated and grepped (via `command grep`, bypassing the `.ignore`-respecting
wrapper — essential for exhaustiveness; the wrapper's first `∅`s were a path-parse
artifact, re-derived correctly below):

- `crates/fsm-lsp/src/capabilities/simulate.rs` — 693 ln (W1 ops + W4a capture)
- `editors/vscode/src/debug/debugPanel.ts` — 1737 ln (W2 panel, W3 bp/time-travel)
- `editors/vscode/src/debug/index.ts` — 53 ln
- `editors/vscode/src/debug/simulateClient.ts` — 162 ln (W1 transport client)
- `editors/vscode/src/debug/webview/debugWebview.ts` — 994 ln (W2/W3 webview)

Eleven pattern families run (transition-selection/firing; guard-evaluation;
completion-synthesis; RTC/microstep; active-config computation; snapshot/restore/
time-travel; event-name synthesis; edge-id↔stableId bridge; any 2nd
Interpreter/`fsm_simulator` verb; LCA/scope/parallel/history; timer-fire/clock
decision). **Every hit read and adjudicated:**

### (a) The decisive grep — every `Interpreter`/`fsm_simulator` call on the surface
Pattern 9 (`\.dispatch\(|\.advance_clock\(|\.step\(|\.raise\(|\.fire\(|interp\.|
Interpreter::|fsm_simulator::|new Interpreter`) over the whole surface returns calls
in **`simulate.rs` ONLY**, and they are EXACTLY the sanctioned shipped-oracle verbs:
`Interpreter::new(&ir)` (`:293`), `interp.init(opts)` (`:353`),
`interp.dispatch_with_payload(…)` (`:394`), `interp.advance_clock(delta_ms)`
(`:421`), `interp.set_context_field(…)` (`:469`, the sanctioned non-stepping
helper), `interp.snapshot()` (`:496`), `interp.restore(snap)` (`:527`), and the
read accessors `interp.current_states_named()/context()/virtual_clock_ms()`
(`:223–226`). **Zero** `Interpreter`/`fsm_simulator` calls exist in ANY `.ts` file —
the only TS hits (`debugPanel.ts:736,757`) are doc-comments. The webview/panel reach
the oracle ONLY through the W1 `fsm/simulate` JSON-RPC ops. Patterns 10 (LCA/
history/parallel) and 11 (timer-fire decision) are **∅ outside comments**.

### (b) Known adjudicated-clean shapes — confirmed present & clean
- **The breakpoint predicate** `evaluateBreakpoint` (`debugPanel.ts:188–209`): a pure
  filter — `step.enteredStates.includes(targetId)` / `exitedStates.includes(targetId)`
  / `transitionTaken?.stableId === targetId`. Reads ONLY the three oracle-emitted
  `StepRecord` fields; no guard eval, no transition re-select, no config compute. The
  exact keystone-clean shape. **CLEAN.**
- **`StepError::GuardEval(_) => "guard-eval"`** (`simulate.rs:199`, inside
  `step_error_kind`): a pure `match` projecting an *already-produced* `StepError`
  variant to a kebab string; doc says "no decision made here … pure projection". An
  error-label projection, **CLEAN.**
- **The edge-id→stableId bridge** `transitionStableIds` (`debugPanel.ts:336–365`) +
  `eventNamesFromIr` (`:294–313`): pure `JSON.parse` structural reads of the
  `--emit-ir` document (the documented `eventNamesFromIr`/`irGraph.ts` discipline);
  they bridge two IR identity fields / list declared event names; they never decide
  enablement. Malformed IR ⇒ empty (never a fabricated id). **CLEAN.**
- **The module docs declaring "NO semantics"**: `simulate.rs:8–9,38–39,566–567`;
  `debugPanel.ts:9,170,182–183,890`; `simulateClient.ts:10–11`;
  `debugWebview.ts:9–11,17–18` — disclaimers, **CLEAN.**

### (c) The remaining read-projections — adjudicated CLEAN
- `stepDetail` (`debugWebview.ts:503–510`): renders a human string from
  `s.kind==="discarded"` / `s.transitionTaken` (oracle-emitted fields). Pure
  projection — does not decide whether a transition was enabled. **CLEAN.**
- `applyActiveConfig` (`debugWebview.ts:373`): toggles a CSS `active` class by
  set-membership over `resp.configuration.activeStates`/`StepRecord.configAfter`
  read verbatim; "does NOT compute which states are active". **CLEAN.**
- `highlightFromOracle` (`debugWebview.ts:677`): *selects which oracle-emitted field
  to paint at the cursor* (`configAfter` of the last revealed step vs `configBefore`
  of step 0 vs `configuration.activeStates`) — a display-source selection over the
  oracle's own outputs, never a configuration derivation. All three branches pass
  oracle values verbatim to `applyActiveConfig`. **CLEAN.**

### (d) The op handlers — keystone phrase-audit (mandate item 4)
- `init` (`simulate.rs:352–363`): "THE one semantic call" `s.interp.init(opts)` →
  `state_block` + `steps_json` marshalling. **CLEAN.**
- `dispatch` (`:367–404`): "THE one semantic call"
  `s.interp.dispatch_with_payload(&name, payload)` → marshalling (payload is
  `serde_json::from_value`, byte-equal to `TraceCommand::Dispatch`). **CLEAN.**
- `advanceClock` (`:406–430`): "THE one semantic call"
  `s.interp.advance_clock(delta_ms)` → marshalling. **CLEAN.**
- `setContext` (`:455–484`): "THE one (non-stepping) call, per field"
  `s.interp.set_context_field(&k, v)`; explicitly inserts `steps: []` (zero records —
  the DBGUX §3.3 contract). The ONLY caller of the helper. **CLEAN.**
- `snapshot` (`:489–505`): "THE one semantic call" `s.interp.snapshot()` + ring
  plumbing. **CLEAN.** `restore` (`:511–533`): "THE one semantic call"
  `s.interp.restore(snap)` + `state_block` projection — no state reconstruction in
  the transport. **CLEAN.**
- `capture` (W4a, `:578–660`): deserializes `init`/`steps`/`expected` (pure serde),
  assembles a `TraceFile`, then **"THE one shipped call" `write_trace_yaml(&trace)`**
  — zero `Interpreter`, zero semantics; surfaces the serializer's own error verbatim
  (no fake "captured"); **rejects empty `expected` at the source** with an explicit
  "`fsm test` would report nothing-to-verify (not a pass)" error. **CLEAN.**

The LSP transport-registration delta (`server.rs`/`lib.rs`/`capabilities/mod.rs`,
W1 commit) contains, among the semantic-keyword grep matches, **only module
documentation explicitly declaring "NO semantics"** ("re-implements **NO**
transition-selection …", "each `op` is exactly ONE shipped `Interpreter` call +
(de)serialisation"). No semantic definition in the transport.

**§2 conclusion: ∅ across the whole debug surface — no forked semantics defined
anywhere. KEYSTONE INTACT.**

---

## §3 — Re-RUN the differential / byte-equality battery (auditor-run, NOT echoed)

All built + run from the worktree under the **1.75.0** pin (asserted), shared target
`/root/dev/embeded-fsm-sdk-target`.

### Host-trace differential — `cargo test -p fsm-simulator --test codegen_equivalence_smoke`
Run **3 consecutive times** (factory-W6a determinism method):

```
RUN 1: test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
RUN 2: test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
RUN 3: test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
```

Deterministic. (The harness header prints "running 9 tests"; 9 total = 7
run-and-passed + 2 `#[ignore]` diagnostic dumps — the count reconciles; the 2
ignored are NOT gated tests.) From the `--nocapture` output the auditor read
directly:

- **11 BYTE_EQUAL confirmed live** — each `shipped fsm_simulator::execute_trace
  oracle == the FSM_TRACE-compiled-and-RUN generated C`: motor, deferred,
  traffic-light, stress-parallel-cross-exit, stress-deep-history,
  stress-self-transitions, stress-self-transitions-actions,
  stress-choice-guard-payload, stress-every-timer, **timer-const-ref**,
  **queue-overflow**.
- **3 BEHAVIOURALLY_EQUIVALENT_JUSTIFIED** correctly RED (record-model differs,
  observable behaviour proven identical): vending-machine (#5), submachine (#0),
  stress-completion-chain (#3).
- **0 KNOWN_DIVERGENT.**
- Never-game guards green **and** non-vacuous (bodies read):
  `oracle_is_the_shipped_execute_trace_seam_not_a_fork` (asserts the sole oracle
  call is the shipped `execute_trace` + the projection is pure formatting),
  `differential_goes_red_on_a_deliberately_corrupted_oracle` (RED-on-divergence
  proof, step #1), `behavioural_equivalence_proof_red_on_corrupted_oracle`,
  `field_separator_is_a_pinned_cross_engine_contract`,
  `fw110_fu_e_stress_every_timer_beat_invocation_count_is_four` (beat()==4).

### Catalogue-integrity contract — re-derived from source AND runtime
`codegen_equivalence_smoke.rs:1531–1559` asserts **in-test, dynamically** (not just
prose): no FSM in >1 bucket (`assert_eq!(all.len(), n, …)`); `BYTE_EQUAL.len() +
BEHAVIOURALLY_EQUIVALENT_JUSTIFIED.len() + KNOWN_DIVERGENT.len() == CORPUS.len()`;
the buckets' union == the corpus exactly; and every BYTE_EQUAL FSM `panic!`s with
"KEYSTONE-MECHANISM REGRESSION" if it ceases to be byte-equal. The partition is
**11 + 3 + 0 = 14 == CORPUS.len()**, asserted at runtime by the (passing)
`host_trace_differential_byte_equals_simulator_oracle_for_corpus`. The catalogue
cannot be gamed by silently dropping/mis-bucketing a member — the test would fail.

### W1 `fsm/simulate` LSP acceptance — `cargo test -p fsm-lsp --test simulate_lsp_acceptance`
**7 passed, 0 failed** (re-run twice, deterministic). Decisive cases:
`init_dispatch_payload_stream_byte_equals_execute_trace_oracle`,
`advance_clock_timer_stream_byte_equals_execute_trace_oracle`,
`snapshot_restore_rewind_is_byte_identical_to_oracle_replay`,
`set_context_field_emits_zero_step_record_and_is_not_a_step`,
`step_error_is_surfaced_verbatim_not_a_fake_clean_end`,
`malformed_request_is_invalid_params_not_empty_simulate`,
`session_lifecycle_load_list_unload`.

### ExtHost suite — `xvfb-run -a npm test` (npm, not pnpm; real shipped binaries)
`resolveRealBinaries()` resolves the **real** `target/debug/{fsm,fsm-lang-server}`
(rebuilt fresh from the repo root so the 1.75 pin applies; 147 MB / 173 MB).

```
47 passing (22s)
Extension host … exited with code: 0
Exit code:   0
```

The three keystone-critical debug tests (all ✔, under the ExtHost):
- **`panel-rendered config/context/timeline BYTE-MATCHES the W1 fsm/simulate
  response (zero semantics)`** (1674 ms) — W2.
- **`§3.4 E2E: a transition breakpoint pauses PRE-step + rewind, both
  BYTE-IDENTICAL to an independent W1 snapshot/restore (zero semantics)`**
  (1639 ms) — W3.
- **`§W4 E2E: capture→.trace.json replays GREEN via `fsm test` (byte-identical by
  construction) + no premature ✓`** (2078 ms) — §W4.

Plus the honest-degradation guards (stale-banner VERBATIM, transport
disabled-with-reason, copyIr no-fake-✓) all pass.

**§3 conclusion:** the differential is deterministically green, the catalogue
arithmetic is dynamically enforced and unviolated, the W1 + ExtHost byte-match-vs-
oracle suites pass — **byte-equality by construction holds, re-derived.**

---

## §4 — §W4 capture-replay non-vacuity (independently confirmed, NOT echoed)

The mandate requires confirming the §W4 replay is **non-vacuous** (`fsm test`
reports a real `pass`, NOT `skip`/nothing-to-verify) — the
`.trace.json`-not-`.trace.yaml` correctness. The auditor re-derived this from
scratch (separate from the ExtHost test):

1. **`fsm test` discovery rule, re-derived from source.** `cmd/test.rs:140–159`
   (byte-identical to anchor, §1): the trace-mode walker accepts a file ONLY if
   `name.ends_with(".trace") || name.ends_with(".trace.json")` (`:159`).
2. **Independent hand-run.** Using the real corpus FSM `motor` + its trace (which
   carries the oracle's `expected` StepRecords): `fsm test <dir>` →
   **`pass: ./motor.trace` … `fsm test: 1 passed, 0 failed (of 1 total)`** — a
   genuine pass, NOT skip.
3. **`.trace.json` is the discovered form.** The same content renamed to
   `evidence.trace.json` → `fsm test <dir>` → **`pass: ./evidence.trace.json` …
   `1 passed`**. The same content renamed to `probe.trace.yaml` →
   **`warning: no .trace or .trace.json files under .`** — a `.trace.yaml` is
   **silently NOT discovered** (would be VACUOUS).
4. **The W4a self-correction is genuine.** This is exactly why `ae774dd`
   (`capture→.trace.yaml`) was superseded by `8092d3a` (`capture→.trace.json`):
   the panel writes `<basename>.trace.json` (`debugPanel.ts:1391`); the code
   comment at `:1313–1320` documents the *identical* rationale the auditor
   independently re-derived from `cmd/test.rs:159`. The W4a `capture` op is one
   `write_trace_yaml` call (the shipped serializer emits **JSON** content despite
   the historical function name), the panel writes its bytes verbatim to
   `.trace.json`, "captured ✓" is posted ONLY after the file exists (the
   no-fake-✓ / no-"saved"-before-confirm bar), and the op **rejects empty
   `expected`** at the source.

**§4 conclusion:** the §W4 capture replay at HEAD is **non-vacuous** — the
`.trace.json` form IS discovered and IS verified with a real `pass` (the replay's
StepRecord stream byte-equals the captured `expected` because it IS the same shipped
`execute_trace` oracle on the same inputs). The `.trace.json`-not-`.trace.yaml`
correctness of `8092d3a` over `ae774dd` is **real and load-bearing**.

---

## Explicit confirmations

- **Structural-diff verdict:** the exact `crates/` delta is `interpreter.rs` +25
  (the sanctioned W1 `set_context_field` non-stepping helper, commit `1de83b0`) and
  the `codegen_equivalence_smoke.rs` test-corpus expansion (+64/−4, factory-wave
  FW-F1/F-2 — additive, not an oracle change). `cmd/test.rs`, `trace.rs`, and the
  entire `fsm-simulator/src/` semantic core are **blob-SHA byte-identical**.
- **Negative-grep adjudication:** **∅** — every hit is a disclaimer / comment /
  error-label projection / StepRecord-field read / the documented breakpoint
  predicate / the pure IR-JSON bridge. No semantics definition anywhere.
- **Re-run differential/ExtHost:** host differential `7/0` × 3 (deterministic);
  catalogue `11+3+0=14` dynamically enforced; W1 LSP acceptance `7/0` × 2; ExtHost
  `47 passing, exit 0` incl. the W2/W3/§W4 byte-match-vs-oracle trio. All
  re-derived, none echoed.
- **§W4 non-vacuity:** independently confirmed — `fsm test` reports a real `pass`
  (not `skip`) on the `.trace.json` form; `.trace.yaml` would be silently
  undiscovered; the `8092d3a` correction is genuine.
- **Keystone phrase-audit:** every `fsm/simulate` op handler = exactly one shipped
  `Interpreter` call + marshalling; the W4a `capture` op = one `write_trace_yaml`
  call + serde marshalling, zero `Interpreter`/semantics. Confirmed by reading every
  handler.

## Judgment calls (every borderline adjudicated)

- **Anchor-tag resolution.** `git rev-parse checkpoint/factory-reliability-ci-2026-05-18`
  returns `a598fb6` (the **annotated-tag object** SHA), not the mandate's `fbae38b`.
  Adjudicated **NOT a discrepancy**: `<tag>^{commit}` dereferences to `fbae38b`
  (the commit the tag points at). The diff baseline `fbae38b` is correct and used
  throughout. Flagged here because the raw `rev-parse` output is superficially
  alarming.
- **`codegen_equivalence_smoke.rs` is in scope but is a TEST.** The mandate's §1
  scope is `crates/fsm-simulator/`. This file is under `tests/`, not `src/`. Read
  in full and adjudicated a clean *corpus expansion* from factory-waves (NOT
  debug-epic): it adds no semantics and the dynamic catalogue arithmetic was
  correctly kept consistent. It strengthens, not weakens, the no-fork guarantee.
- **`write_trace_yaml` emits JSON, not YAML.** The shipped serializer's name is
  historical; its content is JSON (verified by the hand-run: a `.trace.json` of its
  output is parsed and verified by `fsm test`). Function-name vs content vs file
  extension are consistent at HEAD (`.trace.json`). Not a defect — noted to avoid a
  false "yaml mismatch" reading.
- **`highlightFromOracle` selects among oracle outputs.** Adjudicated CLEAN — it
  picks *which already-computed* oracle field to paint at the cursor; it never
  derives a configuration. A display-source selection, not a second semantics.
- **grep-wrapper artifact.** The environment's `grep` is a wrapper that mis-parsed
  a space-joined multi-file argument and respects `.ignore`. The initial `∅`s were
  spurious. All negative-greps were re-derived with `command grep` + a zsh array of
  explicit file paths (verified the files exist first). The reported ∅ is genuine.

## Unverifiable / out of scope (flagged, never fabricated)

- `arm-none-eabi-gcc` / on-target emulation: **not installed, not needed** — the
  debug epic is a host-layer LSP+webview surface with no on-target lane in scope.
  This is NOT a finding.
- The 2 `#[ignore]` diagnostic-dump tests in the differential were not run (they
  are explicitly opt-in `--ignored` diagnostics, not gated correctness tests) — not
  a gap.
- Nothing else was found unverifiable; every load-bearing claim above is backed by
  an auditor-run command, a recomputed SHA/diff, lines read, or a re-run test.

---

**FROZEN evidence — do NOT merge/tag.** Committed on `phase8.0/w4b-keystone-audit`.
The Doc 33 §2 tag-gate KEYSTONE row **PASSES**: `KEYSTONE-INTACT`, zero
fork-findings, the tag may proceed (subject to the orchestrator's other gate rows).
