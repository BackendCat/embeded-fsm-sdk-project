# AUDIT_PHASE_FACTORY_W6_2026_05_18 — the binding tag-gate keystone phase-audit

> **Frozen evidence doc** (the `AUDIT_PHASE_*` never-overwritten convention). This is
> the **mandatory source-derived keystone re-derivation** for Factory-epic **Phase-6.0
> W6a** — the binding tag-gate row of Doc-32 §2 / §5. Per Doc-32 §6 H7 and the
> v1.5-closeout discipline: *a `KEYSTONE-INTACT` verdict that merely echoes a wave's
> own claim is worse than a found problem.* Every load-bearing claim below is the
> auditor's **independent re-derivation** — a command run, a diff computed, lines read
> — explicitly **NOT echoed** from Doc-32 or any wave's self-report.

- **Audit target commit:** `edd4483f348b8f9407874eb88f00584c0cfac209` (`edd4483`,
  `feat(factory-w5): ci.yml 4 additive-isolated lanes + the one-command make ci-local gate`)
- **Worktree:** `/root/dev/embeded-fsm-sdk-wt-w6-closeout`, branch
  `phase6.6/factory-w6-closeout` (off main `edd4483`)
- **Toolchain (asserted from the worktree):**
  `1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')`
  — the repo pin; bare `rustc --version` → 1.75 here (benign).
- **Differential-birth ancestor:** `71cc1bd` =
  `Phase-6.0-W1: the R7 generic host-trace differential (keystone prerequisite)`;
  `git merge-base --is-ancestor 71cc1bd edd4483` → **TRUE** (independently verified —
  `71cc1bd` is a genuine ancestor of HEAD, so the EMPTY-diff below spans the entire
  epic arc).
- **Auditor environment:** host `gcc 12.2.0` present (so the host differential is a
  **HARD** test here, not a skip); `arm-none-eabi-gcc` + `qemu-system-arm`
  **ABSENT — BY DESIGN** (Doc-32 §2 / GT-10 / OWNER-2; the W2 on-target lane is
  CI-runner-only; the auditor did **NOT** install them, and the honest-skip is **NOT**
  a finding); disk 19 G free (no `cargo clean -p` needed).

---

## VERDICT: **KEYSTONE-INTACT**

> Re-derived from source + re-run by the auditor — explicitly **NOT echoed**.

**Gate rationale.** All three binding conditions of the Doc-32 §2/§5 KEYSTONE row
are independently satisfied at `edd4483`:

1. **No-fork (differential keystone):** the shipped oracle
   (`crates/fsm-simulator/src`) and the CLI seam (`cmd/test.rs`, `cmd/baseline.rs`)
   are **byte-untouched** across the entire epic arc `71cc1bd..edd4483` — the
   auditor's `git diff` is **EMPTY** (and blob-SHA-identical per-file).
2. **Negative-grep ∅** for step / transition-selection / guard-eval / deadlock
   semantics *defined* in all three fork-possible surfaces — the W1 trace-hook, the
   W2 QEMU harness, the W5 CI glue. Every grep match was read and adjudicated a
   C-code emitter / a formatter / an invocation / a disclaiming comment — **no
   semantics definition, no forked oracle, no second `StepRecord` re-derivation.**
3. **The host + target differential battery, re-run by the auditor**, satisfies the
   corpus-size-agnostic catalogue-integrity contract: host **3×** deterministic
   `7 passed; 0 failed`; the catalogue partitions exactly **9 + 3 + 0 = 12**; the
   never-game guards are green **and** non-vacuous (proof bodies read); the W2
   toolchain-free keystone guards pass and the on-target lane **honest-skips**
   (by design).

A forked oracle / a re-implemented step-transition-guard-deadlock / a
catalogue-integrity violation / a non-deterministic differential — the cardinal
regression — is **absent**. The tag-gate KEYSTONE row **PASSES**.

---

## §1 — No-fork differential keystone (independently re-derived; the EMPTY diff)

**Command run by the auditor (NOT echoed):**

```
$ cd /root/dev/embeded-fsm-sdk-wt-w6-closeout
$ git diff --stat 71cc1bd..edd4483 -- \
      crates/fsm-simulator/src \
      crates/fsm-cli/src/cmd/test.rs \
      crates/fsm-cli/src/cmd/baseline.rs
   <no output>
$ git diff --quiet 71cc1bd..edd4483 -- crates/fsm-simulator/src \
      crates/fsm-cli/src/cmd/test.rs crates/fsm-cli/src/cmd/baseline.rs ; echo $?
   0        # EMPTY
```

**Result: the diff is EMPTY** (exit 0). The shipped `fsm_simulator::execute_trace`
oracle source tree, plus the CLI seam `cmd/{test,baseline}.rs`, are **byte-untouched
across the ENTIRE epic arc** `71cc1bd` (differential-birth, an independently-verified
ancestor of HEAD) → `edd4483` (HEAD).

**Belt-and-suspenders — per-file blob-SHA at both endpoints (auditor-computed):**

| File | blob @71cc1bd | blob @edd4483 | verdict |
|---|---|---|---|
| `crates/fsm-simulator/src/trace.rs` | `19ed2de6…` | `19ed2de6…` | **IDENTICAL** |
| `crates/fsm-simulator/src/lib.rs` | `2c5e9daa…` | `2c5e9daa…` | **IDENTICAL** |
| `crates/fsm-cli/src/cmd/test.rs` | `6ffb91aa…` | `6ffb91aa…` | **IDENTICAL** |
| `crates/fsm-cli/src/cmd/baseline.rs` | `4f722ed5…` | `4f722ed5…` | **IDENTICAL** |

`git diff --quiet 71cc1bd..edd4483 -- crates/fsm-simulator/src/` → exit 0
(**the ENTIRE `fsm-simulator/src` tree is blob-identical**, not just the four files).

**Projection / comparator functions at HEAD — read by the auditor, confirmed
formatting-only, applied identically to both sides, driving the shipped GT-4 seam**
(`crates/fsm-simulator/tests/codegen_equivalence_smoke.rs`):

- **`project_record` (lines 269–293)** — formats an *already-computed* `StepRecord`'s
  fields into the canonical US-separated line (`STEP␟kind=…␟evt=…␟tr=…␟src=…␟dst=…
  ␟cfgB=…␟cfgA=…␟ent=…␟ext=…`). Body is pure `format!`/`join`. Doc-comment (read):
  *"it computes nothing about the FSM; it only formats what the interpreter (the
  oracle) recorded."* — **formatter, NOT semantics.**
- **`kind_str` (244–258)** — a pure `match StepKind → &'static str` label map. **Formatter.**
- **`sorted_csv` (260–264)** — sorts strings and joins. **Formatter** (both engines
  sort; emission order is not canonical, the SET is — confirmed by reading the
  preamble comment lines 147–155).
- **`simulator_projection` (297–306)** — the ORACLE side: `let res =
  execute_trace(ir, &t)` — **drives the shipped, unforked `fsm_simulator::
  execute_trace` seam** (the exact GT-4 seam `cmd/{test,baseline}.rs` use), then maps
  `res.actual` through `project_record`. Clears `t.expected` (documented
  capture-not-assert; `execute_trace` short-circuits diffing when `expected` is set —
  rooted in `trace.rs`). **No forked re-derivation.**
- **`generated_c_projection` (658–730)** — the GENERATED side: `emit(ir,
  &CodegenConfig::default())` (the **shipped, UNMODIFIED** codegen), writes the
  files, compiles `-std=c99 -Wall -Wextra -Wpedantic -Werror -DFSM_TRACE` with the
  on-box gcc, **runs** the binary, filters stdout for lines starting with `STEP`.
  Contains **NO** FSM step/transition/guard/deadlock logic — the trace-emit logic
  lives in the generated C (from the W1 trace-hook), not here. **Runner, NOT a fork.**
- **`byte_diff` (740–778)** — the SOLE comparison: `oracle[i] != generated[i]`
  byte-equality with first-divergence + length-divergence reporting. Doc-comment
  (read): *"NO semantics here — just the diff … pure equality, no FSM logic."*
- **`run_differential` (810–820)** — orchestrator: `oracle = simulator_projection`
  (shipped `execute_trace`); `generated = generated_c_projection` (shipped codegen +
  gcc + run); asserts the oracle is non-empty (anti-vacuity); `byte_diff`. **No
  corruption parameter in the green path** (the `Some(corrupt)` mentioned in a stale
  doc-comment fragment is not in the signature; corruption lives in the dedicated
  never-game guards — verified §3).

The projection (`project_record`) is **applied identically to both sides** — the
simulator side via `simulator_projection`, the generated side emits the *same*
canonical line shape (pinned by `field_separator_is_a_pinned_cross_engine_contract`,
which asserts `SEP == 0x1f == fsm_codegen_c::emit::trace_hook::FIELD_SEP`, §2/§3).
**This is formatting-only-both-sides, NOT a fork.**

**Conclusion §1: PASS.** The differential drives the shipped, byte-untouched
`fsm_simulator::execute_trace` / `StepRecord` (GT-4); the comparator/projection are
formatting-only and identical on both sides; the EMPTY diff proves the oracle + seam
were not touched anywhere in the epic.

---

## §2 — Negative-grep ∅ for step/transition/guard/deadlock semantics (all three fork-possible surfaces)

> Discipline: the auditor grepped, then **read every matched line** to adjudicate
> *definition* vs *mention/formatter/comment/invocation*. A comment, an import, an
> *invocation* (`cargo test --test …`), or a *formatter* (`project_record`,
> `kind_str`) is **NOT** a semantics definition; only a `fn`/`def` that computes
> step-selection / transition-selection / guard-eval / deadlock is.

### (a) The W1 trace-hook — `crates/fsm-codegen-c/src/emit/trace_hook.rs` (717 lines) + the harness portions of `codegen_equivalence_smoke.rs`

**Grep for semantics-fn names** matched two fns *by name*:
`emit_trace_step_begin` (391), `emit_trace_step_emit` (511). **Adjudicated by
reading:**

- `emit_trace_step_begin` (391–436): body is exclusively
  `s.push_str(&format!("…"))` building `#ifdef FSM_TRACE`-gated **C source text** —
  it resets `m->_trace_tr/src/dst` to `"-"`, snapshots config-before from the C's
  **OWN** `_active[]`. Computes no step/transition logic in Rust. → **C-code
  emitter / trace-tap, MENTION not definition.**
- `emit_trace_step_emit` (511–578): emits C string literals that derive
  kind/event-name from *the event id the C is processing* (`if (ev->id ==
  …_EVENT__COMPLETION)` etc., emitted as text). Doc-comment (read): *"a projection
  of WHICH event the C received … not a re-derivation of semantics."* →
  **EMITTER.**
- `emit_trace_record_transition` (443–502): emits `m->_trace_tr =
  c_string_literal(&t.stable_id)` — records the *compile-time literals* of the
  transition **the codegen's own dispatch already selected**. Doc-comment (read):
  *"the transition the C's own dispatch selected — its own decision, captured, not
  re-derived."* → **EMITTER** (records an already-selected transition's IDs; no
  selection logic).
- `emit_trace_init_emit` / `emit_trace_deferred_emit` / `emit_trace_event_name_fn`
  (583–717): all pure C-string emitters; doc-comments explicitly disclaim
  re-derivation (*"the C already decided to defer; this only records that observed
  fact"*; *"Pure generated data, no semantics"*).
- **Emitted C helpers (read in `emit_trace_preamble`, 86–~250):** `fsm_trace_emit`
  (`fputs(line, stdout)` — pure I/O), `fsm_trace_csv_append` (byte-copy comma-list),
  `fsm_trace_emit_sorted_csv` (tokenise + insertion-sort + re-emit — *"Pure string
  ops"*), `_trace_append_config` (maps the C's OWN `_active[]` through the generated
  `_trace_ir_id` table — *"This reads state the C already tracked — it derives
  nothing"*), `_trace_ir_id[]` (generated **data**, *"not logic"*). The preamble
  header comment is dispositive: *"It is a TRACE TAP, not a second interpreter: it
  emits what THIS generated C did; the shipped fsm_simulator remains the sole
  semantic oracle the host harness byte-diffs against."*

**Broad core-semantics keyword scan** (`select_transitions|resolve_target|
eval_guard|…|step_once|run_to_completion|compute_lca|exit_set|entry_path`) over the
*whole file incl. comments/strings*: **ONE** match — line 479, a **comment**:
*"`entry_path`/`exit_path` decisions — the cross-check target, not a re-derivation
of the simulator."* That is an explicit disclaiming mention, **not** a definition.
No `select_transitions`/`resolve_target`/`eval_guard`/`deadlock`/`step_once`/
`run_to_completion` is defined or even named outside that one disclaiming comment.

**git history (auditor-checked):** trace_hook.rs touched by exactly two commits —
`71cc1bd` (W1 birth) and `a2aaba7` (`fix(codegen-c): FW1-FU-2 — composite exit-set +
shallow_history restore`). Both are *codegen-correctness fixes* on the trace-hook /
codegen emit side, **NOT** oracle forks: §1 independently proves `fsm-simulator/src`
stayed byte-identical, and the fork test (semantics *defined* in the trace-hook) is
∅ at HEAD per the line-by-line read above. The catalogue (read, §3) documents these
as real shipped-codegen bug fixes the differential surfaced — the audit working.

→ **(a) ∅ — no step/transition/guard/deadlock semantics *defined* in the W1
trace-hook; only `#ifdef FSM_TRACE`-gated C emitters + pure string/data helpers.**

### (b) The W2 QEMU harness — `crates/fsm-simulator/tests/on_target_qemu_differential.rs` (1207 lines) + `tests/on-target/{startup_mps2_an385.c,mps2_an385.ld,semihost.h}`

**Negative-grep for semantics fn-definitions in the .rs:** ∅ (`grep -E "fn
[a-z_]*(select_transition|guard|deadlock|step_once|transition_taken|eval_guard|
resolve_target|run_to_completion|exit_set|entry_path|advance_clock)…"` → no match).
**Broad core-semantics keyword scan over the whole .rs incl. comments/strings:** ∅
(no `select_transitions`/`resolve_target`/`eval_guard`/`deadlock`/`step_once`/… name
anywhere).

The W2 .rs **imports and calls the shipped oracle**: line 77
`use fsm_simulator::{execute_trace, parse_trace_yaml, StepKind, StepRecord,
TraceFile};`; `simulator_projection` (251–262) → `let res = execute_trace(ir, &t)`.
Its `kind_str`/`sorted_csv`/`project_record` (read) are **byte-identical
formatting-only copies** of W1's (the duplication forced by "do not modify
codegen_equivalence_smoke.rs", made safe by `projection_shape_matches_w1_canonical_
form` — §3). `target_qemu_projection` (577–723, read): `emit(ir,
&CodegenConfig::default())` (**UNMODIFIED** codegen), cross-compile
`arm-none-eabi-gcc -mcpu=cortex-m3 -DFSM_TRACE -Werror`, run `qemu-system-arm -M
mps2-an385` semihosting, filter `STEP` lines — **pure platform plumbing, no FSM
logic**. `byte_diff` (731–769): pure `Vec<String>` equality (identical shape to
W1's). `run_target_differential` (771–781): same orchestration shape as W1, oracle =
shipped `execute_trace`.

**The on-target C harness** (`startup_mps2_an385.c` 181 ln, `semihost.h` 75 ln,
`mps2_an385.ld` 97 ln): every function is platform plumbing — `semihost_call` /
`semihost_write0` / `semihost_exit_success` (semihosting ABI), `fsm_test_set_clock` /
`fsm_hal_clock_now_ms` (deterministic counter-clock — the established host-HAL
pattern), `fsm_hal_assert`, `Reset_Handler` (CRT bring-up: `.data` copy, `.bss`
zero, call `main()`, clean exit), `Default_Handler` (fault trap). The strong
`fsm_trace_emit` override (read, 104–110): `if (line != 0) semihost_write0(line);`
— passes the codegen's already-NUL-terminated line **straight through, zero
reformatting, byte-for-byte the same line** (the documented weak/strong seam, NOT a
fork). Semantics-keyword scan over the C harness: matches are **only disclaiming
comments** — `startup_mps2_an385.c:29-30`: *"NONE of this defines or re-implements
step / transition-selection / guard-eval / deadlock semantics. It is pure platform
plumbing (CRT…)."* No FSM-semantics keyword in `semihost.h` / `mps2_an385.ld`.

→ **(b) ∅ — no step/transition/guard/deadlock semantics *defined* in the W2 harness
or the target C; the .rs is formatting-only + a QEMU runner driving the shipped
oracle; the C is CRT + semihosting + counter-clock.**

### (c) The W5 CI glue — `.github/workflows/ci.yml` (321 ln) + `Makefile` (171 ln) + `scripts/workflow-lint.py` (260 ln) + `scripts/coverage-gate.py` (508 ln)

**Negative-grep for semantics `def` in the Python scripts:** ∅ (`grep -E "def
[a-z_]*(select_transition|guard|deadlock|step_once|…|execute_trace|StepRecord)…"`
→ no match). **Every FSM-semantics keyword anywhere in the CI glue (incl.
comments)** is a **disclaiming comment** stating no semantics is defined here:

- `Makefile:24-29`: *"Makefile defines NO step/transition/guard/deadlock/oracle
  semantics … only INVOKES the W2/W3/W4 mechanisms … The differential oracle remains
  exclusively `fsm_simulator::execute_trace`. A self-negative-grep for
  step/transition/guard/oracle DEFINITIONS on non-comment lines of this …"*
- `scripts/workflow-lint.py:13, 250-256`: *"INVOCATION-only: it defines NO
  step/transition/guard/oracle semantics … This script defines NO step /
  transition-selection / guard-eval / deadlock / trace-recording / oracle
  semantics. It only … The differential oracle remains exclusively
  `fsm_simulator::execute_trace` (in fsm-simulator, NOT here)."*
- `.github/workflows/ci.yml:91-94, 268-274, 312-315`: *"this CI glue defines NO step
  / transition-selection / guard-eval / deadlock / oracle semantics. The lanes only
  INVOKE the W2/W3/W4 mechanisms; the differential oracle stays the shipped
  `fsm_simulator::execute_trace` / `StepRecord` … a second oracle, NOT)."*

**Function/def inventory (read), all orchestration/lint/gate — no FSM logic:**
`workflow-lint.py` → `fail`, `_render_run_bodies`, `lint_structure`,
`prove_untouched` (itself a keystone-protective guard), `main`. `coverage-gate.py`
→ `load_floors`, `measure`, `read_measured`, `evaluate_gate`, `cmd_gate`,
`cmd_check_monotonic`, `cmd_red_proof`, `main` (coverage-ratchet). The ci.yml lanes
**invoke** the shipped test binaries: `cargo test --workspace` (the W1 host
differential rides here), `cargo test -p fsm-cli --test
conformance_code_coverage_lock` (W3), `xvfb-run -a npm run test:coverage` (W4 c8),
`cargo test -p fsm-simulator` (W2). The Makefile `ci-local` target (71) is a
dependency list shelling out to `cargo` / `xvfb-run npm` / `act -n` / the python
scripts. **Invocation/orchestration only.**

→ **(c) ∅ — no step/transition/guard/deadlock/oracle semantics *defined* in the W5
CI glue; only invocation/lint/gate orchestration + disclaiming comments.**

**Conclusion §2: ∅ across all three fork-possible surfaces — KEYSTONE INTACT.**
Every grep match was adjudicated by reading the matched line(s); none is a
semantics definition. No forked oracle, no second `StepRecord` re-derivation, no
re-implemented step / transition-selection / guard-eval / deadlock anywhere in the
W1 trace-hook, the W2 QEMU/target harness, or the W5 CI glue (Rust *or* C *or*
YAML *or* Python *or* Make).

---

## §3 — Re-RUN the host + target differential battery (auditor-run, NOT echoed)

### Host — `cargo test -p fsm-simulator --test codegen_equivalence_smoke`, 3 consecutive runs

| Run | Result | Time |
|---|---|---|
| **#1** | `test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out` | 2.42s |
| **#2** | `test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out` | 2.42s |
| **#3** | `test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out` | 2.48s |

**Deterministic** `7 passed; 0 failed; 2 ignored` across all 3 (the 2 ignored are
the `#[ignore]` opt-in diagnostic dumps). The 7 passing tests:
`host_trace_differential_byte_equals_simulator_oracle_for_corpus`,
`behavioural_equivalence_proof_for_justified_fixtures`,
`differential_goes_red_on_a_deliberately_corrupted_oracle`,
`behavioural_equivalence_proof_red_on_corrupted_oracle`,
`oracle_is_the_shipped_execute_trace_seam_not_a_fork`,
`field_separator_is_a_pinned_cross_engine_contract`,
`fw110_fu_e_stress_every_timer_beat_invocation_count_is_four`.

### Corpus-size-agnostic catalogue-integrity contract (Doc-32 §2) — re-derived from source AND from the differential's own runtime output

**Catalogue arrays read directly from
`codegen_equivalence_smoke.rs` at `edd4483`:**

- **`CORPUS` (lines 182–195) = 12 members:** motor, submachine, traffic-light,
  vending-machine, deferred, stress-deep-history, stress-every-timer,
  stress-self-transitions, stress-self-transitions-actions, stress-choice-guard-payload,
  stress-completion-chain, stress-parallel-cross-exit.
- **`BYTE_EQUAL` (1173–1183) = 9 members:** motor, deferred, traffic-light,
  stress-parallel-cross-exit, stress-deep-history, stress-self-transitions,
  stress-self-transitions-actions, stress-choice-guard-payload, stress-every-timer.
- **`BEHAVIOURALLY_EQUIVALENT_JUSTIFIED` (1284–1285) = 3 members:** vending-machine,
  submachine, stress-completion-chain.
- **`KNOWN_DIVERGENT` (1401–1418) = literally `&[]`** — the auditor stripped all
  `//`-comment + blank lines from the array body; what remains is exactly
  `const KNOWN_DIVERGENT: &[&str] = &[` + `];` (every interior line is a comment ⇒
  **0 members**).

**Partition: 9 + 3 + 0 = 12 = `CORPUS.len()`** — matches the Doc-32 §2 contract
exactly. The integrity assertion (read, lines 1471–1498) enforces this **dynamically**
at runtime: (i) `all.dedup(); assert_eq!(all.len(), n)` — no fixture in two
buckets; (ii) `assert_eq!(BYTE_EQUAL.len() + BEHAVIOURALLY_EQUIVALENT_JUSTIFIED.len()
+ KNOWN_DIVERGENT.len(), CORPUS.len())` — counts sum; (iii) `assert_eq!(all,
sorted_corpus)` — union == corpus, none missing/extra. The test passed (run #1–3) ⇒
the partition holds.

**The differential's own per-fixture verdicts (auditor ran with `--nocapture`):**

- **Gate (1) — every `BYTE_EQUAL` member byte-equal vs the oracle:** all **9**
  printed `BYTE-EQUAL ✓ (shipped fsm_simulator::execute_trace oracle == the
  FSM_TRACE-compiled-and-RUN generated C)` (motor, deferred, traffic-light,
  stress-parallel-cross-exit, stress-deep-history, stress-self-transitions,
  stress-self-transitions-actions, stress-choice-guard-payload, stress-every-timer).
- **Gate (2) — every `BEHAVIOURALLY_EQUIVALENT_JUSTIFIED` member REDs correctly:**
  vending-machine `RED … FIRST DIVERGENCE at step #5`; submachine `RED … step #0`;
  stress-completion-chain `RED … step #3` — each *"RED (correctly caught — a GENUINE
  record-model difference; BEHAVIOURALLY_EQUIVALENT_JUSTIFIED)"*.
- **Gate (3) — `KNOWN_DIVERGENT` empty:** confirmed `&[]` from source; no fixture
  emitted a `KNOWN_DIVERGENT (unexplained)` line.

**The non-vacuous quiescent-equivalence proof (proof bodies READ + run):**

`behavioural_equivalence_proof_for_justified_fixtures` (read, 1956–2004): for each
JUSTIFIED fixture it **first asserts the byte-diff still REDs** (keeps the catalogue
honest), then for **every prefix `k in 0..=n`** asserts `simulator_quiescent_config
== generated_c_quiescent_config`. `simulator_quiescent_config` (read, 1875–1920)
drives the **shipped `Interpreter`** (`Interpreter::new`/`init`/
`dispatch_with_payload`/`raise_with_payload`/`advance_clock` — the exact
`cmd/test.rs` seam) and returns `interp.current_states()` — **no forked semantics**.
`generated_c_quiescent_config` (read, 1928–1947) truncates the trace, re-runs the
generated C, extracts the C's OWN last `cfgA=` field — **pure field extraction**.

Auditor-run output (`--nocapture`): vending-machine *"PROVEN behaviourally
equivalent — the quiescent configuration after each of the **6** external command(s)
(+ init) is byte-identical"*; submachine *"each of the **3**"*;
stress-completion-chain *"each of the **3**"*. The proof iterates real prefixes
(6/3/3 commands) — **genuinely non-vacuous** (it compares config at every prefix; a
wrong transition/guard/target would change a quiescent config and RED).

**Gate (5) — never-game guards green AND non-vacuous (proof bodies READ):**

- `differential_goes_red_on_a_deliberately_corrupted_oracle` (read, 2063–2107):
  builds the real `motor` oracle+generated, asserts un-corrupted byte-equal
  (precondition), corrupts step #1 `cfgA=s-Motor-Running → cfgA=s-BOGUS`, asserts
  `byte_diff` returns `Err` **and** pins `step #1` **and** surfaces `s-BOGUS`. ✓
  (run #1–3). A genuine perturbation genuinely REDs — **non-vacuous**.
- `behavioural_equivalence_proof_red_on_corrupted_oracle` (read, 2011–2055):
  pre-asserts un-corrupted match at every prefix, corrupts the simulator's final
  config (`s-VendingMachine-Done → s-VendingMachine-BOGUS`), asserts `corrupted !=
  gen_final`. ✓ — **non-vacuous**.
- `oracle_is_the_shipped_execute_trace_seam_not_a_fork` (read, 2137–2154): calls
  `execute_trace(&ir, &t)` (the shipped seam imported from `fsm_simulator`), asserts
  non-empty records + `project_record` is pure formatting. ✓ — the no-fork
  attestation is **executable**.
- (Plus `field_separator_is_a_pinned_cross_engine_contract`, read 2117–2130:
  `SEP == 0x1f == fsm_codegen_c::emit::trace_hook::FIELD_SEP` — the binding
  cross-engine separator contract, so the projection cannot silently drift. ✓.)

The non-byte-equal loop (read, 1528–1555) additionally fails **LOUDLY** (`FW109
CATALOGUE STALE`) if any JUSTIFIED/DIVERGENT fixture *converges* to byte-equal —
the ratchet that forces the catalogue to track reality (so an empty
`KNOWN_DIVERGENT` cannot be gamed; a converged fixture *must* move).

### Target — `cargo test -p fsm-simulator --test on_target_qemu_differential`

**Result: `test result: ok. 6 passed; 0 failed; 0 ignored`.**

- `oracle_is_the_shipped_execute_trace_seam_not_a_fork` ✓ — the **W2 toolchain-free
  no-fork keystone guard** (calls the shipped `execute_trace`); passes WITHOUT the
  ARM toolchain (pre-stages this audit).
- `byte_equal_corpus_matches_w1_catalogue` ✓ — re-derives W1's `BYTE_EQUAL` 9-set
  and asserts `W2_BYTE_EQUAL_CORPUS` equals it exactly **and** that the 3 JUSTIFIED
  fixtures are NOT in the W2 corpus (host-proven, not re-litigated per substrate —
  Doc-32 §2 line ~164/200). Auditor independently confirmed `W2_BYTE_EQUAL_CORPUS`
  (read, lines 99–109) is byte-identical to W1's `BYTE_EQUAL`.
- `projection_shape_matches_w1_canonical_form` ✓ — pins `SEP == 0x1f ==
  fsm_codegen_c::emit::trace_hook::FIELD_SEP`, the exact 10-field canonical W1 shape,
  the same `clk=`/`trace_id=`/`actions=` exclusion set.
- `host_gcc_strengthening_w2_driver_byte_equals_oracle` ✓ — the W2 driver path
  proven locally with host gcc (isolating "cross-compile + QEMU" as the sole
  CI-only delta; QEMU oracle-irrelevant per Doc-32 §2 determinism rationale).
- `on_target_harness_artifacts_present` ✓.
- `on_target_qemu_trace_byte_equals_simulator_oracle_for_byte_equal_corpus` →
  **HONEST-SKIP** with the explicit by-design notice: *"on-target toolchain absent
  (arm-none-eabi-gcc: false, qemu-system-arm: false) — skipping the QEMU
  differential; cargo test still PASSES … This is BY DESIGN, not a gap: the
  on-target lane is CI-RUNNER-ONLY (Doc 32 §W2 / GT-10 / OWNER-2 …). The on-target
  *logic* is locally smoke-proven by W1's host differential (the IDENTICAL FSM_TRACE
  C + the SAME fsm_simulator::execute_trace oracle …). In CI (lane installs both)
  this is a HARD byte-differential."* — **This is NOT a finding.** The auditor did
  **not** install the ARM/QEMU toolchain (Doc-32 §2 / GT-10 / OWNER-2); the
  on-target *logic* is host-proven by W1's identical-C / same-oracle differential
  (§3 host battery, 3× green).

**Conclusion §3: PASS.** Host differential 3× deterministic `7 passed; 0 failed`;
the catalogue-integrity contract holds (9 BYTE_EQUAL byte-equal; 3 JUSTIFIED REDs
correctly with a non-vacuous per-prefix quiescent proof PROVEN; KNOWN_DIVERGENT
literally empty; partition 9+3+0=12 dynamically enforced; never-game guards green
and non-vacuous, proof bodies read). W2 toolchain-free keystone guards pass; the
on-target lane honest-skips by design (NOT a finding).

---

## Explicit confirmations

- **Re-derived from source, NOT echoed.** Every load-bearing claim is the auditor's
  own derivation: the EMPTY `git diff 71cc1bd..edd4483` + per-file blob SHAs (run by
  the auditor); the per-surface negative-grep with **each matched line read** and
  adjudicated definition-vs-mention/formatter/comment/invocation; the **3×**
  host-differential runs + the catalogue-integrity re-derivation from both source
  and the differential's own `--nocapture` output + the never-game-guard **proof
  bodies read**; the W2 toolchain-free guards + the honest-skip — all executed by
  the auditor. No "W2's report said X" reasoning anywhere.
- **Modified nothing but the audit doc.** `git status --porcelain` was empty
  throughout the read-only audit; the sole write+commit is
  `docs/AUDIT_PHASE_FACTORY_W6_2026_05_18.md` on `phase6.6/factory-w6-closeout`.
- **No `git stash`.** The auditor issued **zero** `git stash` (push/pop/apply/drop)
  commands at any point; `git stash list` is empty.
- **Did not install the ARM/QEMU toolchain.** `arm-none-eabi-gcc` /
  `qemu-system-arm` remain absent (by design — Doc-32 §2 / GT-10 / OWNER-2); the W2
  on-target honest-skip is BY DESIGN and is explicitly **NOT** a finding.
- **Toolchain assertion from the worktree:**
  `1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')`
  — the repo pin.

### Judgment calls (every grep-match adjudicated; every borderline)

1. `emit_trace_step_begin` / `emit_trace_step_emit` (trace_hook.rs 391/511) —
   matched the semantics grep by the substring "step". **Read → adjudicated
   EMITTER:** bodies are exclusively `s.push_str(&format!("…"))` building
   `#ifdef FSM_TRACE` C text; no Rust step/transition computation. *Mention, not
   definition.*
2. `emit_trace_record_transition` (trace_hook.rs 443) — matched on "transition".
   **Read → adjudicated EMITTER:** records the *compile-time literals* of a
   transition the codegen's own dispatch already selected; doc-comment *"its own
   decision, captured, not re-derived."* No selection logic.
3. trace_hook.rs line 479 — broad-scan match on `entry_path`/`exit_path`. **Read →
   it is a COMMENT** explicitly disclaiming re-derivation (*"the cross-check
   target, not a re-derivation of the simulator"*). Mention, not definition.
4. startup_mps2_an385.c lines 29-30, 61, 95 — semantics-keyword matches. **Read →
   all DISCLAIMING/PLUMBING COMMENTS** (*"NONE of this defines or re-implements
   step / transition-selection / guard-eval / deadlock semantics. It is pure
   platform plumbing (CRT…)"*). Not definitions.
5. Makefile / workflow-lint.py / ci.yml semantics-keyword matches — **all
   DISCLAIMING COMMENTS** stating "defines NO step/transition/guard/oracle
   semantics … only INVOKES … oracle remains exclusively
   `fsm_simulator::execute_trace`." Not definitions.
6. `run_differential` doc-comment fragment mentions a `Some(corrupt)` parameter not
   present in the actual signature (`fn run_differential(example: &str)`).
   **Adjudicated benign stale doc fragment:** the green path applies no corruption
   (verified by reading the body, 810–820); corruption lives only in the dedicated
   never-game guards (`differential_goes_red_on_a_deliberately_corrupted_oracle`),
   which the auditor read and ran (genuine, non-vacuous). Not a keystone issue;
   noted for completeness as a borderline (a cosmetic comment-vs-signature drift,
   **not** in scope to fix here — read-only audit).
7. trace_hook.rs git history shows `a2aaba7` (FW1-FU-2 codegen fix) touched the
   trace-hook. **Adjudicated NOT a fork:** §1 independently proves
   `fsm-simulator/src` byte-identical across the whole arc; the fork test
   (semantics *defined* in the trace-hook) is ∅ at HEAD; the catalogue (read)
   documents these as real shipped-codegen bug fixes the differential surfaced
   (the audit working as designed — a codegen *correctness* fix, not an oracle
   re-derivation).

---

*Auditor: independent verification/reliability engineer (FSM-Studio Factory-epic
W6a). This doc is frozen and never overwritten (the `AUDIT_PHASE_*` convention).*
