# Behavioural Test-Debt Ledger

**Document ID:** FSM-PROC-TESTDEBT
**Status:** Living tracking doc. Opened by v1.1-W0 (plan retrospective PD-2).

## Why this exists

The P0-1 catastrophe shipped behaviourally-empty codegen with 413 "green"
tests because they asserted **symbol presence** (`text.contains("Motor_init")`)
instead of **behaviour** (compile the generated C with `gcc -Werror`, run it,
assert observable state/context/trace). See `SUBAGENT_CONVENTIONS.md` §5.2 /
§5.4.

An inventory at W0 found ~140 `.contains()` assertions across the workspace
test suite. W0 does **not** convert all of them (that would be a multi-wave
ocean-boil). It pays down the **highest-risk subset** — the `fsm-codegen-c`
suite, the exact place P0-1 was masked — and records the rest here so the
remaining debt is tracked, classified, and never silently forgotten.

`.contains()` is legitimate as a **secondary** check *alongside* a real
behavioural test, or when asserting a genuine structural / file-content
invariant or the *absence* of something (no runtime behaviour exists to
observe). It is forbidden as the **sole** guard for behaviour.

---

## Part 1 — Paid down in v1.1-W0 (fsm-codegen-c)

| Area | Before W0 | After W0 |
|---|---|---|
| **B-10 hierarchical dispatch** (`hierarchical_dispatch.rs`) | Symbol-presence ONLY — no test ever compiled the C. P0-1-class. | Added `fault_in_composite_leaf_walks_up_to_error_in_gcc_built_binary_{switch,table}`: gcc-compile-RUN, asserts FAULT-in-`Op.Running` reaches `Error` via the leaf→root parent-table walk. Old `.contains` kept as **secondary structural** (parent-table shape) with justification. |
| **Context-default seeding** (`context_defaults_emitted.rs`) | Symbol-presence ONLY (`m->context.price = 150;`). Could not tell "assignment runs" from "assignment dead". P0-1-class. | Added `non_zero_defaults_are_observable_in_gcc_built_binary`: gcc-compile-RUN, asserts `m.context.price==150`, `balance==0`, `enabled==true` at runtime. Old `.contains` kept as **secondary structural**; `fields_without_default_are_not_assigned` reclassified as a **legitimate absence-invariant**. |
| **Parallel completion** (`parallel_completion.rs`) | Symbol-presence for the B-08 helper shape. | Behaviour already covered by `parallel_dispatch_runs.rs` (gcc-compile-RUN: regions advance independently, both reach Final). Reclassified as **secondary structural / absence-invariant** with module-level justification pointing at the behavioural guard. |
| `golden_motor.rs` | Symbol-presence (API surface, file set, roles). | **Legitimate structural invariant** — Motor behaviour is proven by `gcc_compile.rs::motor_compiles_with_gcc_werror` (compile+run+assert transitions). Justification header added; not converted (would duplicate `gcc_compile.rs`). |
| `license_header.rs` | SPDX text presence/absence. | **Legitimate file-content invariant** (Doc 00 §10.4) — a comment has no behaviour. Justification header added. |
| `too_many_states_returns_error.rs` | `.contains` on the `EmitError` Display string. | **Legitimate error-message API contract**; the behaviour (typed `Err` instead of panic, success under cap) is asserted via `match emit(...)`. Justification header added. |
| `done_autofire_c.rs` | Has gcc-RUN test + secondary `.contains`. | Already behavioural (`done_on_simple_state_drives_runtime_in_gcc_built_binary`). `.contains` reclassified **secondary** with header. |
| `timer_arms_on_entry.rs` | Whole test is gcc-RUN; 2 header `.contains`. | Already behavioural. Header `.contains` reclassified **secondary**. |
| `timer_distinct_from_completion.rs` | Has gcc-RUN test + secondary `.contains`. | Already behavioural (`timer_path_and_event_path_produce_different_final_states`). `.contains` reclassified **secondary**. |

No `.contains()` were *deleted* from fsm-codegen-c — behaviour-guarding ones
were superseded by a co-located gcc-RUN test and demoted to secondary with an
explicit justification; genuinely-structural ones were documented as such.

### Real product defects surfaced by the conversion (NOT fixed in W0)

W0 is a test+invariant wave; per `SUBAGENT_CONVENTIONS.md` §6 + the depth-first
rule, codegen bugs the new behavioural tests revealed are reported here for a
dedicated follow-up wave, not fixed in-band:

1. **`TD-BUG-1` — zero-transition machine emits non-`-Werror` C.**
   A valid machine with no transitions makes codegen emit
   `M_try_transitions_in_state(M_t *m, M_StateId_t s, const M_Event_t *ev)`
   with an empty body that uses none of `m`/`ev`, so the generated C fails
   `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`
   (`-Werror=unused-parameter`). Masked for the life of
   `context_defaults_emitted.rs` because its only coverage was
   symbol-presence that never invoked gcc. Worked around in the W0
   behavioural test by giving the fixture one self-transition (documented
   inline). **Owner: a codegen-c follow-up wave.** Fix sketch: emit `(void)m;
   (void)s; (void)ev;` (or `return false;`) when the per-state switch is
   empty.

2. **`TD-FIX-1` (fixed in W0, in-scope) — duplicate-event test fixture.**
   `crates/fsm-codegen-c/tests/common/mod.rs::hierarchical_motor_ir()`
   double-declared the `FAULT` event, yielding IR with a duplicate event id
   that codegen lowered to a duplicate C enumerator
   (`MOTOR_EVENT_FAULT = 2` AND `= 4`) — uncompilable C. This was a
   **test-helper defect**, in W0 scope; fixed (the redundant `push`
   removed). Not a codegen bug: the analyzer guarantees unique event ids
   (FSM-E0022) and these IR builders bypass the analyzer. Codegen
   robustness against duplicate ids (defensive de-dup or a pre-flight
   assert) is a *possible* hardening — folded into `TD-BUG-1`'s wave.

---

## Part 2 — Deferred, tracked, NOT in W0 scope

These are **lower risk** than the codegen-c set, generally because each area
has independent behavioural coverage elsewhere (the simulator suite, trace
conformance, or CLI e2e gcc-RUN tests). They are catalogued so the debt is
visible; conversion is a future wave, prioritized after higher-value feature
work.

### 2a. Conformance `tests/conformance/codegen-c/` snippet fixtures

`001_motor/expected/*.contains`, `002_composite_dispatch/…`,
`003_parallel_completion/…` (~21 snippet lines total). The conformance
*runner* (`crates/fsm-cli/src/cmd/test.rs::run_codegen_golden`) only does
`emit()` + substring match — it does **not** gcc-compile despite the MANIFEST
descriptions saying "gcc -Werror clean".

- **Risk: LOW.** The three machines these cover are behaviourally proven by
  the fsm-codegen-c crate suite: motor by `gcc_compile.rs`, composite/B-10 by
  W0's new `hierarchical_dispatch.rs` gcc-RUN tests, parallel by
  `parallel_dispatch_runs.rs`.
- **Deferred work (`TD-DEF-1`):** teach `run_codegen_golden` to optionally
  gcc-compile (and ideally run) the emitted C, or migrate these fixtures to
  crate-level gcc-RUN tests. Out of W0 scope because it edits `fsm-cli` src
  (not in W0's touch set) and the runner change is itself a wave.

### 2b. Non-codegen crate `.contains()` (~119 assertions)

| Crate / file | Count | Why LOW risk (behavioural coverage that exists) |
|---|---:|---|
| `fsm-parser/tests/grammar.rs` | 13 | Asserts diagnostic *messages* / parsed-shape. Parser correctness is behaviour-checked by trace conformance + analyzer/sim e2e; a parser that accepts wrong grammar fails those. Message-substring on a `Diagnostic` is the correct tool (it IS the user-facing contract), like `too_many_states`. |
| `fsm-parser/tests/dos_limits.rs` | 6 | DoS-limit diagnostics — message/*code* assertions; the behaviour (parse rejected at the limit) is asserted via the error itself. Correct tool. |
| `fsm-parser/tests/import_security.rs` | 3 | Path-traversal rejection — asserts the security diagnostic fires. Behavioural (input rejected); message-substring is the contract. |
| `fsm-cli/tests/*` (`cli_*`, `motor_emits_guards_and_actions`, `golden_vending_defaults`, `motor_timer_e2e`, `deferred_example_e2e`, `cli_test_with_simulator`) | 38 | Mixed. `motor_timer_e2e` & `deferred_example_e2e` ALREADY gcc-compile-RUN. `motor_emits_guards_and_actions` self-documents that `golden_simulator_runs_motor` (same target) is its behavioural twin (sim proves ctx mutates / state advances). `cli_*` assert CLI **stdout/exit-code** contracts (the correct tool for a CLI surface). `golden_vending_defaults` overlaps W0's new codegen-c runtime default test. |
| `fsm-analyzer/tests/{positive,lowering}.rs` | 5 | Assert lowered-IR *structure*; analyzer behaviour is now ALSO guarded by the W0 IR-schema gate (every `analyze` validates the emitted IR) + trace conformance. Structural by intent. |
| `fsm-simulator/tests/{deterministic_trace,deterministic}.rs` | 3 | Simulator is the behavioural oracle; these assert determinism-report text. The state-transition behaviour is the trace assertions themselves, not the `.contains`. |
| `fsm-lexer` (1), `fsm-formatter` (1) | 2 | Token/format text spot-checks; formatter idempotence + lexer round-trip are the behavioural guards. |

- **Deferred work (`TD-DEF-2`):** revisit per-crate in a future test-quality
  wave, converting any that turn out to be the *sole* guard for a behaviour
  (none identified as such at W0 — each area has an independent behavioural
  oracle). Most are correctly message/stdout/structure assertions and will
  stay with a justification rather than convert.

---

## Closing criteria

- **Part 1** is closed by W0 (this wave).
- `TD-BUG-1` closes when a codegen-c wave makes zero-transition (and,
  defensively, duplicate-id) machines emit `-Werror`-clean C, with a
  gcc-RUN regression test.
- `TD-DEF-1` closes when the conformance codegen-c path gcc-compiles its
  fixtures (or they migrate to crate gcc-RUN tests).
- `TD-DEF-2` closes when each Part-2b area has been reviewed and every
  remaining `.contains()` is either converted or carries a one-line
  justification (structural / absence / message-contract / has-behavioural-
  twin). Track row-by-row here when that wave runs.
