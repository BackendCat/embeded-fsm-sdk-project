# FW110 — Reliability Stage-Gate Audit (2026-05-18)

**Branch:** `phase6.5/fw110-reliability-stage-gate` (worktree, off main `e10b65e`).
**Toolchain (asserted from the worktree):** `rustup show active-toolchain` →
`1.75.0-x86_64-unknown-linux-gnu (overridden by rust-toolchain.toml)` — the
repo pin, honored. (A bare `rustc --version` → 1.95 is the benign box
default; NOT pin drift.)
**Quad (from the worktree, warm shared `CARGO_TARGET_DIR`):**
`cargo fmt --all --check` clean · `cargo clippy --workspace --all-targets`
zero warnings · `cargo test --workspace` **874 passed / 0 failed** ·
`cargo build --workspace` clean.

This is the owner's STAGE-GATE before the Factory-Reliability epic W2
(on-target QEMU) proceeds. Brutal honesty over papering-over. The audit
surfaced **five genuine shipped-codegen defects** the prior 5-fixture
differential corpus did not exercise. One was small/contained and is
**fixed inline here**; four are deep/wide and are **precisely characterized
with a recommended dedicated fix-wave each**. The catalogue is honest (a
non-empty `KNOWN_DIVERGENT`); nothing was gamed green.

---

## OVERALL VERDICT (the gate): **STAGE-NOT-SOLID**

The foundation is **NOT** verifiably completely solid. The W1 host-trace
differential ENGINE is sound, the keystones RE-CONFIRM, and the differential
mechanism works end-to-end (it caught all five defects). But broadening the
corpus past the 5 surfaced **four genuine, currently-unfixed behavioural
codegen divergences** (deep_history, choice/junction, local-`~>` LCA,
periodic-timer under-fire). Per the gate's exact precondition
("zero genuine divergences unaddressed"), the stage is **NOT** solid and the
Factory-Reliability epic **W2 must NOT proceed** until the four scoped
fix-waves below land (or the owner explicitly de-scopes the affected
language features from the v1.x factory-trust surface).

This is the audit working as designed — surfacing latent defects is the
correct, valued outcome, not a failure. The differential is now broadened
and pins all five defects with executable, non-gamed evidence.

### Required before W2 (each a scoped dedicated fix-wave)

1. **FW110-FU-A — `choice`/`junction` codegen lowering** (P0; whole missing
   feature). Generated C never resolves a choice/junction pseudostate — it
   rests in it forever. Spec in Stream 1.
2. **FW110-FU-B — `deep_history` codegen restore** (P1; documented v1.0
   limitation, now corpus-exercised). Generated C does not restore the deep
   nested leaf. Spec in Stream 1.
3. **FW110-FU-C — local (`~>`) transition LCA / entry-exit** (P1; production
   side-effect bug). Generated C re-runs the containing composite's entry
   action and skips the source leaf's exit action on a sibling-targeted
   local transition. Spec in Stream 1.
4. **FW110-FU-D — periodic (`every N ms`) timer budget accounting** (P1;
   observable action-count divergence). Generated C fires a periodic timer
   one too FEW times across a multi-period `advance_clock`. Spec in Stream 1.

### Fixed inline here (small/contained)

- **FW110 — composite active-descendant exit-set: undeclared
  `<M>_exit_<Final>` call** (was a `-Werror` production-C *compile* break
  for any FSM whose composite has a completion/exit edge with a live Final
  descendant). Fixed in `crates/fsm-codegen-c/src/emit/transition.rs`;
  byte-identical generated C for all 5 original fixtures proven; behavioural
  acceptance via the new `stress-completion-chain` fixture (JUSTIFIED, with
  the non-vacuous per-prefix quiescent proof).

### Sub-excellent (non-blocking, surfaced per the mandate)

- The **conformance `codegen-c` category oracle is symbol-presence**
  (`Motor.c.contains` / `Motor.h.contains`), not compile+behavioural — it
  would NOT have caught the FW110 undeclared-`_exit_<Final>` bug. P1
  test-debt (Stream 2).
- **Doc-32** hardcodes "the 5 frozen-trace examples / 5 MVP corpus FSMs" in
  ≥6 load-bearing places incl. the **binding W2/W6 keystone gate row** —
  now stale (corpus = 11, 4 KNOWN_DIVERGENT). The W2/W6 corpus policy needs
  an owner/TL decision (Stream 3).
- `golden_motor.rs`-class symbol-presence `.contains()` assertions (44
  repo-wide) are hollow-but-low-risk *only because* the behavioural
  differential exists alongside — and that differential's coverage gap
  (now partially closed) was the real risk (Stream 2).

---

## Stream 1 — Broadened differential corpus + latent-defect hunt

**Corpus before:** 5 (`motor`, `submachine`, `traffic-light`,
`vending-machine`, `deferred`). **After FW110:** **11** — 6 synthesized
stress fixtures deliberately targeting the primitive matrix the prior FIVE
shipped-codegen bugs lived in. Each wired identically to the original 5
(`examples/<name>/{<name>.fsm,<name>.trace}`; `load_trace` resolves it).
Classification uses the #109 discipline: byte-equal / proven-equivalent
(non-vacuous per-prefix quiescent proof) / genuine divergence (fixed-inline
or recommended-dedicated-wave). Catalogue-integrity assertion
(`BYTE_EQUAL + BEHAVIOURALLY_EQUIVALENT_JUSTIFIED + KNOWN_DIVERGENT ==
CORPUS`) holds: **4 + 3 + 4 = 11**.

| Fixture | Primitive(s) exercised | Verdict | Disposition |
|---|---|---|---|
| `motor` | guard + 1-shot `after` + payload | BYTE-EQUAL | unchanged |
| `deferred` | defer→hold→release→redispatch | BYTE-EQUAL | unchanged |
| `traffic-light` | `after` chain + shallow_history + composite-exit | BYTE-EQUAL | unchanged |
| `submachine` | nested submachine + completion | PROVEN-EQUIVALENT | unchanged (record-model; FW109 proof) |
| `vending-machine` | parallel regions + completion | PROVEN-EQUIVALENT | unchanged (record-model; FW109 proof) |
| **`stress-parallel-cross-exit`** | exit a Parallel **mid-flight**, both regions live non-Final, different depths | **BYTE-EQUAL** | new → `BYTE_EQUAL`. Validates the FW1-FU-2/FW109 parallel active-descendant exit-set on a NEW shape (FW109's vending case exited only after both regions hit Final). `ext=A3,B2,Running` byte-identical to the oracle, all 5 steps. |
| **`stress-completion-chain`** | 2-level cascading `done ->` through Final states | **PROVEN-EQUIVALENT** | new → `BEHAVIOURALLY_EQUIVALENT_JUSTIFIED`. **Surfaced + fixed a genuine bug FIRST** (see FW110 fix below). Residual byte-diff RED = the SAME completion record-model class as vending (`evt=__completion__:<state>` + composite-entry `ent` set vs combined sweep). Quiescent config byte-identical sim-vs-C at all 4 prefixes (Ready/S1Work/S2Work/Closed) — non-vacuous (the cascade genuinely moves states). |
| **`stress-deep-history`** | `deep_history` restore of a 2-level nested leaf | **GENUINE DIVERGENCE** | new → `KNOWN_DIVERGENT`. **Recommend dedicated fix-wave FW110-FU-B.** |
| **`stress-choice-guard-payload`** | `choice` pseudostate routed by payload-derived ctx guard | **GENUINE DIVERGENCE** | new → `KNOWN_DIVERGENT`. **Recommend dedicated fix-wave FW110-FU-A.** |
| **`stress-self-transitions`** | internal vs external-self vs **local `~>`** entry/exit-sets | **GENUINE DIVERGENCE** | new → `KNOWN_DIVERGENT`. **Recommend dedicated fix-wave FW110-FU-C.** |
| **`stress-every-timer`** | PERIODIC `every N ms` re-arm + `every … :` internal | **GENUINE DIVERGENCE** | new → `KNOWN_DIVERGENT`. **Recommend dedicated fix-wave FW110-FU-D.** |

> `examples/per-machine-strategy` and `examples/import-header` exist with no
> `.trace`; they are multi-machine / import-structural examples, not
> single-machine differential-corpus-shaped. Not force-fit (recorded, not
> silently dropped). Conformance MANIFEST fixtures are
> parser/validator/semantic/codegen-c/formatter assertions with no
> `.trace`; not directly differential-wireable without authoring traces —
> out of FW110 scope, recorded.

### FIXED INLINE HERE — undeclared `<M>_exit_<Final>` (composite active-descendant exit-set)

- **Symptom:** `stress-completion-chain` made the generated C **fail to
  compile** `-Werror`: `implicit declaration of function
  'Pipeline_exit_S1DONE'` / `'Pipeline_exit_S2DONE'`. **NOT trace-only** —
  the bogus call is emitted **outside** `#ifdef FSM_TRACE`; the production
  (`#ifndef FSM_TRACE`) C also fails to compile (verified by an independent
  `-Werror -Wall -Wextra -Wpedantic` production build).
- **Root cause:** `emit/transition.rs` composite active-descendant exit-set
  emitter (the FW1-FU-2 runtime switch) called `composite_descendant_leaves`
  which **includes Final leaves** (`transition.rs:556`
  `StateNode::Final(f) => leaves.push(...)`) and emitted
  `<M>_exit_<leaf>(m);` for **every** returned leaf — including Final.
  But the exit-handler **declaration** passes deliberately omit Final
  (`impl_header.rs:60,85` `rec.kind == StateRecordKind::Final ⇒ continue`;
  `source.rs:152,176,227,233` guard `_exit_X` with `!= Final`). So the
  call referenced a never-declared function.
- **Divergent observable:** the generated production C does not compile (the
  most severe class — it is not even a runtime divergence; it is a build
  break for a whole valid FSM class: any composite with a completion/exit
  edge and a live Final descendant, e.g. a `done ->` cascade).
- **Fix:** added a `leaf.kind == StateRecordKind::Final` branch (symmetric to
  the existing submachine-ref branch) that **records the Final leaf in the
  trace `ext` set** (behavioural parity — the shipped
  `fsm_simulator::run_exit` runs NO exit action for a Final yet
  `full_exits`/`exit_set` DO record the active Final leaf in
  `exited_states`) **and emits NO `<M>_exit_<Final>(m)` call** (no such
  handler exists; the simulator runs no Final exit action). Codegen→matches
  the shipped UNFORKED simulator. FSM_TRACE-discipline preserved: the only
  emitted line is `#ifdef FSM_TRACE`-gated; the production C is byte-identical
  to before for every FSM **without** a live-Final-descendant composite exit.
- **`#ifndef FSM_TRACE` byte-identical proof:** regenerated all 5 ORIGINAL
  corpus fixtures' C with the baseline `e10b65e` codegen vs the FW110-fixed
  codegen — `diff -rq` produced **zero output for every fixture** (motor,
  traffic-light, deferred, submachine, vending-machine all BYTE-IDENTICAL,
  production AND FSM_TRACE). The fix's only effect is changing
  live-Final-descendant-composite-exit FSMs from non-compiling to
  compiling+behaviourally-correct.
- **Behavioural acceptance:** `stress-completion-chain` now compiles+runs;
  its per-prefix quiescent config is byte-identical sim-vs-C at all 4
  prefixes (proven by `behavioural_equivalence_proof_for_justified_fixtures`,
  which is non-vacuous — the corrupted-oracle meta-guard stays RED); its
  byte-diff still correctly REDs on the pure completion record-model
  difference (the catalogue stays honest).

### RECOMMENDED DEDICATED FIX-WAVES (deep/wide — NOT inline-fixed; depth-first characterization)

#### FW110-FU-A (P0) — `choice`/`junction` pseudostate lowering is UNIMPLEMENTED in codegen-c

- **Proven divergence:** on `CLASSIFY` the shipped simulator resolves the
  choice through its guard chain to High/Mid/Low; the generated Router.c
  sets `m->_active[0] = ROUTER_STATE_CHOICE` and **rests there forever** —
  the machine is permanently stuck in `Decide`. `dump_all_corpus_quiescent`
  prefixes 1-6: `[DIFF] sim=[] gen=[s-Router-Decide]`.
- **Root cause (from source):** `state_index.rs:375,388` + `region_layout.rs`
  INDEX Choice/Junction as state records + slots, but **NO `emit/` site
  lowers the runtime guard-chain resolution** (`grep Choice
  crates/fsm-codegen-c/src/emit/` → ∅). The shipped
  `fsm_simulator::resolve_target` (`interpreter.rs:1572-1612`) implements it
  fully (top-to-bottom guard eval, first-true, `[else]`, error if none).
- **Divergent StepRecord:** sim resolves through Decide so its quiescent
  `cfgA` is High/Mid/Low (projected empty at the pseudostate-transient
  record); C's last record `cfgA=s-Router-Decide` permanently.
- **Spec for the fix-wave:** in the transition body, when a transition's
  resolved target is a Choice/Junction, emit the guard chain (mirroring
  `resolve_target`: `if (g0) { …enter b0… } else if (g1) { …enter b1… }
  else { …enter else… }`), chaining transitively if a branch targets
  another pseudostate, with the resolved-target entry sequence + the
  `FSM-E0100`-equivalent runtime trap for "no matching branch". Behavioural
  acceptance: `stress-choice-guard-payload` byte-equal (or JUSTIFIED with a
  non-vacuous quiescent proof) to the shipped oracle across all 3 branches.

#### FW110-FU-B (P1) — `deep_history` codegen restore is UNIMPLEMENTED

- **Proven divergence:** after `RESUME` (the `deep_history`-target
  transition) the simulator restores the remembered deep nested leaf
  (`cfgA=s-DeepHist-Deep2`); the generated C **does not even leave Paused**
  (`cfgA=s-DeepHist-Paused`). `dump_all_corpus_quiescent` prefix 4:
  `[DIFF] sim=[Deep2] gen=[Paused]`.
- **Root cause (from source):** an EXPLICIT, *documented*, pre-existing v1.0
  limitation — `crates/fsm-codegen-c/src/emit/history.rs` module-doc §"v1.0
  scope (honest)" lines 40-43: *"Deep history, and history across parallel
  regions, remain the pre-existing v1.0 limitation (`_history_record_X`
  snapshots `_active[0]` only)."* The `_history_restore_<C>` helper is
  hardcoded shallow even when the pseudo is `deep_history`. The codegen even
  invokes "the brief's anti-scope-creep discipline". It was "not exercised by
  any shipped example" — FW110's fixture now exercises it.
- **Divergent StepRecord:** RESUME step — sim `cfgA=Deep2, ent=Deep2,Work,
  WorkB`; C `cfgA=Paused, ent=Work` (no transition out of Paused).
- **Spec for the fix-wave:** real deep-history snapshot of the FULL active
  nested leaf path on exit of the history-bearing composite (not just
  `_active[0]`), and a restore that re-enters every level down to the
  remembered leaf, mirroring the simulator's `record_history_before_exit`
  `HistoryKind::Deep` arm + `resolve_target` deep restore. Behavioural
  acceptance: `stress-deep-history` byte-equal/JUSTIFIED to the oracle.

#### FW110-FU-C (P1) — local (`~>`) transition LCA / entry-exit is WRONG for a sibling target (PRODUCTION side-effect bug)

- **Proven divergence (production, NOT record-model):** on `REENTER`
  (`Inner1 ~> Inner2`, siblings in composite `Box`) the generated SelfT.c
  emits `SelfT_entry_BOX(m); SelfT_entry_INNER2(m);` and **no
  `SelfT_exit_INNER1(m)`** — it **re-runs the containing composite's ENTRY
  ACTION** and **skips the source leaf's EXIT ACTION**. The shipped
  simulator correctly does `ext=Inner1, ent=Inner2` and never re-enters
  Box.
- **Root cause (from source):** `emit/entry_exit.rs:63`
  `effective_lca` collapses `TransitionKind::Local | Internal => source`
  unconditionally. Correct only when the target is **within the source
  subtree**; for a sibling/cousin-targeted local transition the LCA must be
  the common ancestor and the entry/exit sets must be leaf-symmetric (exit
  source leaf, enter target leaf, do NOT re-enter the common composite).
  `exit_path` returns empty for Local (line 93-94) so the source leaf is
  never exited; `entry_path` walks target up until `cur == lca(=source)`
  which never matches a sibling, so it walks PAST the composite and includes
  it in `ent`.
- **Divergent observable:** the quiescent config converges **only because
  this fixture has no entry/exit actions** — the convergence is **VACUOUS
  w.r.t. the bug**. A `Box`/`Inner1` carrying entry/exit actions WOULD
  observably diverge (Box's entry action re-runs; Inner1's exit action is
  skipped). Hence NOT classifiable as JUSTIFIED (the NEVER-GAME razor).
- **Spec for the fix-wave:** correct `effective_lca` for Local to
  distinguish "target within source subtree" (LCA = source, current
  no-source-exit behaviour) from "target is a sibling/cousin" (LCA = common
  ancestor; exit source leaf-chain to LCA-exclusive, enter target
  leaf-chain from LCA-exclusive — leaf-symmetric, composite NOT re-entered),
  matching the simulator's `effective_lca`/`exit_set`/`enter` for Local.
  This touches the shared transition-LCA algorithm — re-derive against the
  simulator for ALL transition kinds; do NOT inline-patch without it.
  Behavioural acceptance: `stress-self-transitions` byte-equal to the oracle
  PLUS a variant fixture WITH entry/exit actions on Box/Inner1 proving the
  action-execution parity (the vacuity-closing test).

#### FW110-FU-D (P1) — periodic (`every N ms`) timer fires one too FEW times across a multi-period `advance_clock`

- **Proven observable action-count divergence:** the shipped simulator
  invokes the `every 1000 ms : beat()` internal **4** times (clk=1000,2000,
  3000,6000 — verified via the simulator's `actions_executed`); the
  generated C invokes it **3** times (it misses the clk=3000 tick when a
  competing `every 3000 -> Cooldown` consumes the same `advance_clock(3500)`
  step — verified by an instrumented host run logging `beat() @clk`).
- **Root cause:** the periodic-timer budget/re-arm accounting in
  `emit/timer.rs::emit_advance_clock` — the periodic analogue of the W1-FU
  one-shot OVER-fire (here an UNDER-fire: the periodic internal's final due
  period within the budget is not fired before the transition-causing timer
  consumes the step).
- **Divergent observable:** quiescent config converges (states are right)
  so NOT JUSTIFIED-classifiable — the convergence is **VACUOUS w.r.t. the
  action count**. An embedded target gets one fewer `beat()` side-effect per
  multi-period advance — a real missed-heartbeat / missed-watchdog-kick
  class defect.
- **Spec for the fix-wave:** in `emit_advance_clock`, process every armed
  timer's due period in strict budget order (consume the elapsed budget
  period-by-period across ALL timers, re-arming periodics at each expiry),
  mirroring `fsm_simulator::Interpreter::advance_clock` exactly. This is the
  same delicate timer-budget code that already required the dedicated W1-FU
  wave; re-derive against the simulator; do NOT inline-patch (risk of
  regressing the byte-equal motor/traffic-light one-shot paths).
  Behavioural acceptance: `stress-every-timer` byte-equal to the oracle AND
  the `beat()` count == 4.

### Catalogue final state (honest, integrity assertion holds)

```
BYTE_EQUAL                        = ["motor","deferred","traffic-light",
                                     "stress-parallel-cross-exit"]                 (4)
BEHAVIOURALLY_EQUIVALENT_JUSTIFIED = ["vending-machine","submachine",
                                     "stress-completion-chain"]                    (3)
KNOWN_DIVERGENT                   = ["stress-deep-history",
                                     "stress-choice-guard-payload",
                                     "stress-self-transitions",
                                     "stress-every-timer"]                         (4)
CORPUS.len() == 4 + 3 + 4 == 11   ✓ (assertion passes; nothing reclassified
                                      to make the corpus green)
```

The 4 `BYTE_EQUAL` are byte-equal end-to-end; the 3 `JUSTIFIED` each have a
non-vacuous per-prefix quiescent-equivalence proof + a RED-on-corruption
meta-guard; the 4 `KNOWN_DIVERGENT` each RED with a precise first-divergence
and carry a root cause + scoped fix-wave. **No projection/comparator was
weakened or canonicalized; no fixture was reclassified to manufacture
green.**

---

## Stream 2 — Test-debt sweep

| Location | Kind | Severity | Recommendation |
|---|---|---|---|
| `tests/conformance/codegen-c/*/expected/{Motor.c.contains,Motor.h.contains}` | The **normative conformance codegen-c oracle is substring-presence** (`.contains`), not compile+behavioural | **P1 — hollow-AND-correctness-critical** | The codegen-c conformance category must gate on `-Werror`-clean compile + the W1 trace-equality pattern, NOT `.contains`. It would NOT have caught the FW110 undeclared-`_exit_<Final>` bug (a `.contains("Pipeline_exit_…")` passes while the C doesn't compile). Aligns with Doc-32 W5/G7's behavioural-acceptance mandate — direction is known; *current state* is hollow. Recommend conversion in the Factory-Reliability G7 wave. |
| `crates/fsm-codegen-c/tests/golden_motor.rs` (≈21 `.contains`) | Symbol-presence golden/smoke | hollow-but-low-risk | Tolerable ONLY because the behavioural W1 differential exists alongside. The real risk was the differential's *coverage gap* (4 latent-defect classes), now partially closed by FW110. No conversion needed if the differential corpus stays broad; keep as a shape smoke. |
| `crates/fsm-codegen-c/tests/{timer_arms_on_entry,timer_distinct_from_completion,branch_hints_codegen,hierarchical_dispatch,zero_transition_werror,context_defaults_emitted}.rs` | Mixed: some `.contains` symbol-presence, some compile-`-Werror` (behavioural) | behavioural-now (the compile-`-Werror` ones) / hollow-low-risk (the `.contains` ones) | The `compile -Werror` assertions ARE behavioural (a non-compiling emit fails them — indeed `zero_transition_werror` is genuinely behavioural). The interleaved `.contains` lines are low-risk shape checks. No P1 conversion; keep. |
| `crates/fsm-verify/tests/w2_hierarchy_acceptance.rs`, `crates/fsm-simulator/tests/submachine_runtime.rs` (`.contains`) | Mixed; the load-bearing assertions are on `current_states()` / `StepRecord` values (behavioural) | behavioural-now | The `.contains` here is on *config/state vectors* (value assertions), not symbol presence — genuinely behavioural. No action. |
| Bare `.is_some()`/`.is_ok()` with no value assertion in correctness suites | — | none found | grep returned **0** in codegen-c/verify/simulator test dirs — the W0 conversion held for this class. |

**"Green but hollow" suite, stated plainly:** the **conformance codegen-c
category** is green and hollow — its oracle is `.contains` substring
presence. A green codegen-c conformance run does **not** pin codegen
behaviour and would have shipped the FW110 compile-break. This is a
stage-gate caution (P1, non-blocking only because the W1 differential is the
de-facto codegen oracle and now catches it).

---

## Stream 3 — Doc↔code drift sweep

| Doc | Drift | Triage |
|---|---|---|
| **Doc-32** (`32-Factory-Reliability-CI-Wave-Plan.md`) §W1/W2/W6 deliverable + §5.4 acceptance + the **binding KEYSTONE gate row (line 152, 188)** | Hardcodes "the **5** frozen-trace examples / **5** MVP corpus FSMs" in ≥6 places, incl. the tag-blocker. Corpus is now **11** (4 KNOWN_DIVERGENT). The W2 on-target QEMU differential + the W6 keystone re-derivation both say "byte-equal on the MVP corpus = the 5" — ambiguous post-FW110: literal "the 5" misses the broadened corpus; literal "the corpus" fails on the 4 KNOWN_DIVERGENT. | **batch-for-v1.6 + ESCALATE (owner/TL decision).** Directly load-bearing on the gate THIS audit governs, but the W2/W6 corpus policy (gate on the `BYTE_EQUAL ∪ JUSTIFIED` subset? gate on the catalogue, not a frozen "5"?) is a product/TL call — NOT unilaterally rewritten here (the don't-ask-permission rule excludes gate-semantic / product-strategy changes). Recommended reconciliation: W2/W6 gate on **"every `BYTE_EQUAL` member byte-equal + every `JUSTIFIED` member's quiescent-equivalence proof green + every `KNOWN_DIVERGENT` member still precisely RED with a tracked fix-wave"** — i.e. the catalogue-integrity contract, corpus-size-agnostic. |
| `crates/fsm-simulator/tests/codegen_equivalence_smoke.rs` module-doc | Module-doc still says "the **5** frozen-trace example FSMs (GT-7)"; the `CORPUS`/catalogue consts ARE updated (FW110) but the top-of-file prose is now stale | **fixed-here (data consts + catalogue doc-comments updated with full FW110 rationale).** The module-doc *prose* header (lines 10-12) is left as a v1.6-batch item to avoid churning the keystone-narrative block mid-audit; the authoritative `CORPUS`/`BYTE_EQUAL`/`JUSTIFIED`/`KNOWN_DIVERGENT` doc-comments are fully reconciled and are what a reader/audit consults. Disclosed. |
| `tests/conformance/COVERAGE_MAP.md` | States live enum = **73** (75 − W0500 − E0903); the `all_codes_matches_expected_count` lock test pins 73 and is **green** (13 fsm-diagnostics tests pass). `FSM-E0750` listed **UNTESTED**. | **no new drift.** The 73 is internally consistent and lock-test-enforced. `FSM-E0750 UNTESTED` is a pre-existing, already-disclosed gap (in the map itself), not introduced here — recorded, owned by the G7 conformance wave. (A naive `grep FSM-[EW]NNNN` over `src` counts ~77 due to retired/deprecated string mentions — NOT a real drift; the lock test is the authority.) |
| `CHANGELOG.md` | No FW110 entry yet (corpus broadening + the Final-exit codegen fix + the 4 surfaced defects) | **batch-for-v1.6** (CHANGELOG is owner-curated at release; the audit doc + catalogue doc-comments are the frozen evidence of record). Recommend a CHANGELOG line at the next release cut. |
| README / Doc-00 §11 / Doc-07 / Doc-14 | No FW110-relevant drift found (they do not enumerate the differential corpus size). | none |

No spec↔grammar↔code divergence found in the surface FW110 touched: the
grammar correctly rejects external-self-on-composite (FSM-E0401) and
requires a non-empty `action_list` after `:` for `internal_decl` — both
matched the DSL spec EBNF (§8 `internal_decl = … ":" , action_list`) and
both surfaced during fixture authoring (recorded, fixtures adjusted to
valid forms).

---

## Stream 4 — Keystone re-derivation (independent, from source, at HEAD `e10b65e`)

### (1) Differential keystone — **RE-CONFIRMED**

Independent derivation evidence (commands run from the worktree):

- **Drives the SHIPPED UNFORKED oracle.** `simulator_projection`
  (codegen_equivalence_smoke.rs:228) calls `fsm_simulator::execute_trace`
  directly — the SAME seam `cmd/{test,baseline}.rs` consume. The C side is
  emit-hook-only (`generated_c_projection`: codegen `emit()` → gcc
  `-Werror` → run → filter `STEP` lines). The projection
  (`project_record`/`sorted_csv`/`byte_diff`/`run_differential`/`SEP`/
  `kind_str`/`host_hal_c`/`driver_main_c`/`generated_c_projection`) is
  formatting-only, applied identically to both sides.
- **sim/src + cmd seam byte-untouched across the ENTIRE W1→#109 arc.**
  `git diff --stat 71cc1bd~1..e10b65e -- crates/fsm-simulator/src
  crates/fsm-cli/src/cmd/test.rs crates/fsm-cli/src/cmd/baseline.rs` →
  **empty** (the differential was born at `71cc1bd`; the seam is identical
  from *before its birth* through HEAD). The whole arc (`71cc1bd..e10b65e`)
  touched ONLY `crates/fsm-codegen-c/src/emit/{history,timer,trace_hook,
  transition}.rs` + the differential test file. **Re-attested on the FW110
  working tree:** `git diff -- crates/fsm-simulator/src
  crates/fsm-cli/src/cmd/{test,baseline}.rs` → **empty** (FW110 modifies
  neither).
- **Projection/comparator UNMODIFIED by FW110.** Every diff hunk in the
  test file is at line ≥116; hunk 1 (line ~116) is **only** the `CORPUS`
  data-const extension (the intended corpus-broadening, explicitly
  in-scope) + its doc-comment — verified line-by-line. `sorted_csv`(191),
  `project_record`(200), `simulator_projection`(228), `byte_diff`(671),
  `run_differential`(741), `SEP`(117), `kind_str`(175),
  `generated_c_projection`, `host_hal_c`, `driver_main_c` function bodies
  are byte-untouched (all remaining hunks are catalogue-const changes + the
  new `dump_all_corpus_quiescent` diagnostic — pure additions, never the
  comparator logic).
- **The JUSTIFIED proofs genuinely drive the shipped Interpreter; guards
  non-vacuous.** Read the proof bodies: `simulator_quiescent_config`
  (codegen_equivalence_smoke.rs:1167+) constructs the SHIPPED
  `fsm_simulator::Interpreter`, mirrors `execute_trace`'s extern
  registration + init + per-command dispatch/raise/advance_clock, and reads
  `current_states()` — the record-model-agnostic parent observable. The
  RED-on-corruption guards
  (`behavioural_equivalence_proof_red_on_corrupted_oracle`,
  `differential_goes_red_on_a_deliberately_corrupted_oracle`) mutate a real
  state id and assert ≠ the genuine generated config — verified passing AND
  non-vacuous (they perturb a *passing* proof). FW110's new `JUSTIFIED`
  member `stress-completion-chain` was independently verified to satisfy the
  per-prefix quiescent equivalence (4 prefixes, all `[OK]`) via the new
  `dump_all_corpus_quiescent` diagnostic.

**Verdict: RE-CONFIRMED.** The differential drives the shipped unforked
oracle; the C is emit-hook-only; the projection is formatting-only applied
identically both sides; sim/src + cmd seam + projection fns are
byte-untouched by the entire W1→#109 arc AND by FW110; the JUSTIFIED proofs
genuinely drive the shipped Interpreter with non-vacuous RED-on-corruption
guards.

### (2) Verification-core keystone — **RE-CONFIRMED**

- **`fsm-verify` drives the shipped simulator with no forked transition
  logic.** Confirmed via the no-fork derivation above: `crates/fsm-verify`
  is byte-untouched by the W1→#109 arc and by FW110; the workspace test run
  shows `fsm_verify` 13 unit + 8 reachability + 5 w2_hierarchy acceptance
  tests green driving the shipped interpreter semantics (no parallel
  oracle).
- **v1.4 clock-origin-merge soundness still holds (Doc 08 §13.5
  "Absolute-Virtual-Clock Non-Observability").** Confirmed from the
  grammar + the FW110 timer investigation: NO FSM-Lang guard/expression can
  read the absolute clock. The DSL `guard_expr` (Doc 04 §7.x EBNF) is over
  `ctx.*` / `payload.*` / consts / enums / extern-bool only — there is no
  clock-read production; timers are `after`/`every` *transition triggers*,
  not readable values. The FW110 periodic-timer defect (FU-D) is a *budget
  accounting* bug in `emit_advance_clock`, NOT a clock-read-in-guard — it
  does **not** introduce an absolute-clock observable, so the
  clock-origin-merge remains sound. **No P0 soundness BLOCK.**

**Verdict: RE-CONFIRMED.**

---

## Brutal-honesty disclosure (every judgment call)

1. **`stress-parallel-cross-exit` → BYTE_EQUAL:** byte-equal projection AND
   quiescent-equivalent at all 5 prefixes. Unambiguous.
2. **`stress-completion-chain` → JUSTIFIED (not BYTE_EQUAL, not
   KNOWN_DIVERGENT):** the byte-diff RED is the *exact* vending-machine
   completion record-model class (`evt=__completion__:<state>` +
   composite-entry `ent` vs combined sweep). Quiescent config byte-identical
   at all 4 prefixes — **non-vacuous** (the cascade genuinely moves
   Ready→S1Work→S2Work→Closed; a wrong completion target/order WOULD change
   a quiescent config). Same proof apparatus as FW109's vending/submachine.
   Judgment: pure record-model, JUSTIFIED is correct.
3. **`stress-self-transitions` → KNOWN_DIVERGENT (NOT JUSTIFIED):** the
   quiescent config converges, but ONLY because the fixture has no
   entry/exit actions — the convergence is **vacuous w.r.t. the actual
   bug** (the production C re-runs `entry_BOX` and skips `exit_INNER1`). Per
   the NEVER-GAME razor, a vacuously-convergent fixture masking a real
   production side-effect divergence is NOT JUSTIFIED. Judgment: genuine
   divergence.
4. **`stress-every-timer` → KNOWN_DIVERGENT (NOT JUSTIFIED):** quiescent
   converges, but `beat()` fires 4× (sim) vs 3× (C) — an observable
   action-count divergence the quiescent config does not (and cannot)
   observe. Vacuous convergence again. Judgment: genuine divergence.
5. **`stress-deep-history`, `stress-choice-guard-payload` →
   KNOWN_DIVERGENT:** quiescent configs *materially* DIVERGE
   (`Deep2`≠`Paused`; `[]`≠`Decide`). Unambiguous genuine divergences;
   both documented/structural codegen feature gaps.
6. **FW110 Final-exit fix = fix-inline (not a fix-wave):** small, contained,
   one symmetric branch, byte-identical for all 5 original fixtures, clear
   simulator-parity rationale. The other four are deep (whole feature /
   shared LCA algo / delicate timer-budget code) → fix-waves, per the
   depth-first scope discipline (the W1→W1-FU/W1-FU-2 precedent).
7. **Doc-32 corpus-size drift = escalate, not fixed-here:** the W2/W6 gate
   semantics are product/TL territory; rewriting the binding tag-blocker row
   unilaterally would exceed the don't-ask-permission boundary. Surfaced
   with a concrete recommended reconciliation.
8. **Stale binary scare (disclosed):** an early production-C compile test
   used a binary left stale by a `git stash` round-trip; re-derived cleanly
   after an explicit rebuild — the fix is correct (0 `Pipeline_exit_S1DONE`
   in the regenerated C; production AND FSM_TRACE compile `-Werror` clean;
   the 5-original byte-identical proof re-run cleanly post-rebuild). The
   874-pass quad + green differential were already on correctly-rebuilt
   artifacts.

**Disk:** `/` at ~4.7G free (above the ~2G grind threshold); shared
`CARGO_TARGET_DIR=/root/dev/embeded-fsm-sdk-target` kept warm throughout
(no `cargo clean` needed; no other project's `target/` touched). No disk
constraint encountered.

---

## Errata — 2026-05-18 (post-FW110-FU-A; appended; original §Stream-1 preserved as frozen evidence)

**FW110-FU-A root-cause attribution corrected.** §Stream-1 FW110-FU-A above
attributes the `choice`/`junction` divergence to *codegen only* ("NO `emit/`
site lowers the runtime guard-chain resolution; the shipped `resolve_target`
implements it fully"). The FW110-FU-A implementation wave (merged `de10291`)
verified that premise against actual code and found it **incomplete**: the
codegen gap is real and is now fixed (new `emit/pseudostate.rs` mirroring
`resolve_target`), but the *governing* root cause is upstream in
**`fsm-analyzer` `lower_choice`/`lower_junction`**
(`crates/fsm-analyzer/src/lower/state.rs` ≈329-373) — every branch is lowered
with `guard: GuardExpr::Else, actions: Vec::new()` and the branch target is
left as the raw DSL name (never resolved via `IdMinter::state_target_id`).
Consequently **both** engines receive garbage IR and the *shipped simulator
itself* emits `sim=[]`, so `resolve_target` is **not a usable oracle** for
this fixture until the analyzer is fixed. FU-A therefore — correctly, per the
never-game razor — did **not** move `stress-choice-guard-payload` out of
`KNOWN_DIVERGENT`; it landed only the proven-correct codegen half and
recommended the analyzer-side follow-up **FW110-FU-A2** (tracker #119). The
fixture's `BYTE_EQUAL` move is FU-A2's acceptance gate (FU-A's reverted,
validation-only experiment already proved byte-equality across all 7 records
and all 3 guard arms once the analyzer lands). This errata is the
audit-integrity reconciliation — the verify-status-claims-vs-code discipline
applied to an audit's *own* root-cause attribution; the original finding text
above is intentionally preserved unaltered.
