# §11.3 Post-W2 Phase-Boundary Audit — v1.4 Verification Core (W2: composite/parallel/history/timer/submachine breadth)

**Document ID:** FSM-AUDIT-PHASE-V14-W2
**Version:** 1.0.0 (frozen evidence doc — never overwrite; a new audit is a new versioned file)
**Audit type:** §11.3 phase-boundary audit (Doc 30-mandated gate before any W3 dispatch). READ-ONLY judgment.
**Audited commit:** `3447881` (`Extend fsm-verify to composite/parallel/timer/submachine (W2)`; W2-P0 snapshot-lossless = `efae559`; W1 was `dd2d41e`; the post-W1 §11.3 audit is `a75cd69`).
**Worktree / branch:** `/root/dev/embeded-fsm-sdk-wt-v14-audit` on `phase4.3/v1_4-w2-audit` (cut from `3447881`, clean).
**Auditor:** independent phase-boundary auditor (no `cargo build/test` run — the quad is already §11.1-independently-verified at `/tmp/w2-indep-rerun-report.md` = 114 ok / 0 FAILED on `3447881`; this is a read+reason audit per the mandate, disk margin ~5.7G protected).
**Governing record:** Doc 30 (§1.3, §3.2, §4.1, §4.2-W2/-W3, §5.2 + the "Owner scope-confirmation + TL architecture decision" section), Doc 00 §11.63, Doc 04 §1.5/§3/§8.5/§8.7/§9, Doc 08 §3.1/§4.1/§4.3/§13/§14/§15.1, `[[feedback_embeded_fsm_pipeline_before_ui]]`, the post-W1 §11.3 audit (D-1, D-2).

---

## 0. Executive summary (read this first)

**W3-readiness verdict: `PROCEED-WITH-NOTES`.**

W2 is a sound breadth wave. The two highest-stakes properties are **fully confirmed by my own reading**, not restated from the prior reports. (1) **The keystone holds at the W2 boundary.** I independently re-enumerated *every* `fn` in `crates/fsm-verify/src/**` (15 non-test functions): not one selects a transition, evaluates a guard, fires a timer, computes an LCA, or decides region-join/completion. The explorer is pure orchestration of `fsm_simulator::Interpreter` via `init`/`restore`/`dispatch`/`advance_clock`/`snapshot`/`current_states`; composite/parallel/history/submachine-completion are taken *inside* the interpreter's RTC drain, the timer-fire edge is driven by the real `advance_clock`, and `min_armed_expiry` only reads `snap.timers[].expiry_ms` and returns a `.min()` (it fires nothing). `fsm-analyzer` is `[dev-dependencies]`-only. The honest-bound is structurally enforced: `Verdict::ProvenNoDeadlock` is constructed at exactly one site (engine.rs:483) on the natural frontier-exhausted exit with `StopReason::Exhausted` hard-coded; every bound-hit returns `inconclusive(...)` first, and `inconclusive()` carries `StopReason::Exhausted => unreachable!()` so an "inconclusive-yet-exhausted" outcome is unconstructible. The W2 acceptance suite proves this behaviourally by byte-equating the verifier's reachable set against an independent interpreter-driven hand-BFS over composite/parallel/submachine fixtures and replaying deadlock witnesses through a *fresh* interpreter. (2) **The clock-merge is sound as a formal finding-of-record.** I independently re-derived — quoting Doc 04 §1.5/§8.5/§8.7/§9 and Doc 08 §3.1/§4.1/§4.3/§13/§14/§15.1 — that FSM-Lang has **no construct** (keyword, guard, expression, `pure extern` parameter, or timer duration) that can observe the absolute virtual clock; the *only* occurrence of "virtual clock" in the entire Doc 08 is §13.4, and it injects a *relative* `elapsed` delta. The origin→0 + remaining-duration normalization therefore identifies only strongly-bisimilar configurations, so **a false `ProvenNoDeadlock` is impossible by this mechanism** — and the merge is *required* for cyclic-timer termination (confirmed by the genuine-timer-deadlock acceptance test asserting `Deadlock`, not `Inconclusive`). (3) **W2 actually closed post-W1 D-2.** `InterpreterSnapshot` now losslessly captures `timers` + recursive `submachines`; the change is purely additive (pre-W2 fields keep name/type/order; flat snapshots serialize byte-identically with `"timers":[]`/`"submachines":{}`); a real 435-line `snapshot → restore → re-snapshot` byte-identity round-trip gate exists and is substantive.

No P0. No W3 ship-blocker. The notes are entirely **doc/citation reconciliation deferred to the v1.4 batched closeout** (the project's correct batched-doc discipline) plus W3-input hazards — consolidated and frozen as the §5 carry-list. The W1-audit's headline D-2 ("Doc 30 §4.1 `already the complete state` overstatement") remains uncorrected in Doc 30 — **correctly so** (closeout-batched, not mid-epic) but it must not be lost; it heads the carry-list.

### Findings table

| ID | Sev | One-line | Evidence (file:line) | Disposition |
|---|---|---|---|---|
| **F1** | — | Keystone intact at W2: every `fn` in `fsm-verify/src` is structural-IR-read or pure Interpreter-orchestration; zero second semantics | engine.rs:81,253,489,525,569; deadlock.rs:99,136; digest.rs:111,146,166; reachability.rs:42,82; diagnostics.rs:50-200; Cargo.toml:21-42 | accepted-tracked (PASS) |
| **F2** | — | Honest-bound structurally enforced: `ProvenNoDeadlock` only on the single exhausted exit; `unreachable!()` guards inconclusive-on-exhausted | engine.rs:457,473-487,510 | accepted-tracked (PASS) |
| **F3** | — | Clock-merge sound: no FSM-Lang construct observes absolute time ⇒ merge identifies only strongly-bisimilar configs ⇒ no false `ProvenNoDeadlock` by this mechanism | Doc 04 §1.5 L80-105, §8.5 L680-702, §8.7 L795, §9 L184-193/L931; Doc 08 §3.1 L69-81, §4.1 L105-130, §4.3 L142-146, §13 L575-606, §14 L616, §15.1 L643-645; digest.rs:111-163 | accepted-tracked (PASS) |
| **F4** | — | W2 closed post-W1 D-2: `InterpreterSnapshot` lossless for `timers`+recursive `submachines`, additive, real round-trip gate | runtime/state.rs:280-299,134-198; interpreter.rs:415-464; timer.rs:100-127; snapshot_lossless.rs:59-72,397-435 | accepted-tracked (PASS) |
| **F5** | P3 | Doc 30 §4.1 L192 still says `InterpreterSnapshot … already the complete state` — the W1-audit D-2 overstatement, uncorrected (correctly closeout-deferred) | docs/30-…:192 | closeout-batch |
| **F6** | P3 | Doc 30 §4.2-W1 L204 / §4.3 L251-274 still scope `fsm verify` CLI + E0400/W0602 to W2 (shipped in W1 per §11.63) — W1-audit D-1, still stale | docs/30-…:204,251-274 | closeout-batch |
| **F7** | P3 | Doc 30 §4.2-W2 L206 says "§11.3 audit after W2"; the actual cadence is post-W1 (this doc's predecessor) **and** post-W2 (this doc) | docs/30-…:206,199 | closeout-batch |
| **F8** | P3 | `digest.rs` cites "Doc 08 §13" for absolute-clock non-observability; Doc 08 has no §13.5 / no verbatim non-observability clause (property is *entailed*, not stated) — independent re-run NOTE-2 | digest.rs:23,61,72,245; docs/08-…(no §13.5) | closeout-batch |
| **F9** | P3 | `fsm-verify/vN` versioning policy is binding+well-formed but lives **only** in verify.rs doc-comment; no Doc 18/Doc 13 reflection (factory-contract doc owed) | verify.rs:70-101; docs/18-… (absent) | closeout-batch |
| **F10** | P3 | W1-audit cites `crates/fsm-simulator/src/state.rs` in places; canonical path is `crates/fsm-simulator/src/runtime/state.rs` (no `src/state.rs` exists) | docs/AUDIT_PHASE_V1_4_W1…:175 vs runtime/state.rs | closeout-batch |
| **F11** | — | NOTE-1 (stale parallel-worktree test binary masquerading as a tag-blocking red) is reproducible + one-`clean`-fixes-it; the W4 §11.30 tag-gate must pre-empt it | /tmp/w2-indep-rerun-report.md §Cold-quad | closeout-batch (operational fix into tag procedure) |
| **F12** | — | W3 seam (`execute_trace`/`first_mismatch`) is a stable, drift-free surface the verifier can orchestrate without forking semantics; trace format is JSON-as-v1.0-YAML-shape (versioning hazard to note) | trace.rs:243-334,269-276 | W3-input |

**Verdict line: `PROCEED-WITH-NOTES` — W3 may be planned/dispatched. No P0, no ship-blocker. All notes are closeout-batch doc reconciliation or W3-input hazards.**

---

## 1. Lens 1 — Keystone integrity at the W2 boundary (highest priority)

**Claim under audit:** no `fn` in `crates/fsm-verify/src/**` selects a transition / evaluates a guard / fires a timer / computes an LCA / decides region-join/completion; the verifier only orchestrates `fsm_simulator::Interpreter` as the sole semantic oracle.

### 1.1 Every function independently enumerated + classified (PASS)

I enumerated every `fn` (`grep -rn '^\s*…fn '` then read each file in full — not symbol-presence). The complete non-test set, with my own classification:

| `fn` | file:line | What it actually does (read, not trusted) |
|---|---|---|
| `declared_concrete_states` + inner `walk` | reachability.rs:42,43 | Walks `StateNode` collecting Simple/Final/Composite/Parallel **IDs** — structural shape only |
| `ReachabilityReport::build` | reachability.rs:82 | `declared.difference(&reachable)` — set arithmetic over the oracle's observed set |
| `is_final_state` + inner `walk` | deadlock.rs:99,100 | Walks the IR for a `StateNode::Final` with matching id — a **data discriminator**, no behaviour |
| `is_final_configuration` | deadlock.rs:136 | `!config.is_empty() && config.iter().all(is_final_state)` over the oracle's `current_states()` |
| `normalize_clock_origin_sub` | digest.rs:111 | Sets `virtual_clock_ms=0`, rewrites `expiry_ms -= origin` (saturating), zeroes nested `next_trace_id`, recurses subs — **canonicalization; fires nothing, selects nothing** |
| `canonical_bytes` | digest.rs:146 | Clones snap, applies the normalization, `serde_json::to_vec` — bytes only |
| `ConfigDigest::of` | digest.rs:166 | Two salted `DefaultHasher`s over the canonical bytes |
| `reachability_diagnostics` | diagnostics.rs:50 | Builds E0400/W0602 `Diagnostic`s; **E0400 gated `matches!(stop_reason, Exhausted)`** (diagnostics.rs:56-60) |
| `concrete_state_loc` / `concrete_states` / `transition_target_ids` / `initial_target_ids` + walks | diagnostics.rs:96-200 | Structural IR shape reads (names, spans, transition targets, initial targets) |
| `min_armed_expiry` + inner `sub_min` | engine.rs:81,82 | Reads `snap.timers.iter().map(|t| t.expiry_ms).min()`, recurses `snap.submachines` — returns the **soonest expiry**; does NOT fire, schedule, or select |
| `ExplorationStats::bound_hit` | engine.rs:173 | `!matches!(self.stop_reason, StopReason::Exhausted)` |
| `verify` | engine.rs:253 | The explorer. Every successor = `interp.dispatch(ev)` / `interp.advance_clock(delta)`; every backtrack = `interp.restore(snap)`; every read = `interp.snapshot()` / `interp.current_states()` / `interp.virtual_clock_ms()`. Only direct IR reads: `machine.events…name` (engine.rs:258) + `is_final_configuration` (engine.rs:448) |
| `inconclusive` | engine.rs:489 | Outcome construction; `StopReason::Exhausted => unreachable!()` (engine.rs:510) |
| `select_machine` | engine.rs:525 | `ir.machines.iter().find(|m| m.name == name)` — IR-structural selection |
| `reject_unsupported` | engine.rs:569 | `let _ = machine; Ok(())` — a no-op structural seam (the Interpreter is the authority on driveability) |

**Negative grep, read not trusted.** `grep -rnE 'select_transition|enabled_set|eval_guard|compute_lca|find_lca|completion_drain|fire_timer|join_complete|region_join|priority|doc_order' crates/fsm-verify/src/*.rs` filtered to **non-comment lines** → **zero matches**. I read the filter expression itself (it strips `^…:NN:\s*//` and `//!` lines via a second `grep -vE`, so it is not the `head && echo` false-positive class the brief warns about — there is no `head`/`echo` branch here; the empty result is a true empty). Those tokens appear only in module **prose** (explaining the prohibition) and in `fsm-simulator`. The deadlock predicate is *operational* — "no edge changed the digest ∧ not a final config" (engine.rs:448) — it never decides which transition is enabled; it asks the Interpreter by dispatching/advancing and observing the snapshot.

**Dep-graph structural proof (F1 item iii).** `crates/fsm-verify/Cargo.toml`: `[dependencies]` = `fsm-simulator` (the oracle), `fsm-ir` (structural reads), `fsm-diagnostics`, `serde_json`, `thiserror` (lines 21-34). `fsm-analyzer` is `[dev-dependencies]`-only (lines 36-42, alongside `fsm-parser` — used only to parse+analyze real `.fsm` acceptance fixtures). The §11.63 architecture decision ("`fsm-analyzer` untouched, the interpreter not dragged into the analyzer graph") holds exactly at the W2 boundary.

**Finding 1.1: PASS — keystone independently re-derived from source at `3447881`. No second semantics exists anywhere in `fsm-verify`.**

### 1.2 The W2 edge families are both Interpreter-driven (PASS)

W2's risk is the timer-fire / completion / submachine / parallel-join edges being re-implemented. They are not:

- **Edge family 1 — every declared event** (engine.rs:317-370): `interp.restore(node.snap.clone())` → `interp.dispatch(ev)` → `interp.snapshot()`. Composite/parallel/history transitions **and submachine-completion** (`done ->`) are taken *inside* `dispatch`'s real RTC drain (Doc 08 §3.2 `M_dispatch` loops the internal queue to quiescence; completion is processed at the front of the internal queue — §14). The explorer adds no edge kind for them.
- **Edge family 2 — the timer-fire edge** (engine.rs:380-429): iff `min_armed_expiry(&node.snap)` is `Some`, `interp.restore(...)` → `delta = next_expiry.saturating_sub(interp.virtual_clock_ms())` → `interp.advance_clock(delta)` → `interp.snapshot()`. The *which-timer-fires-and-what-it-triggers* decision is entirely `advance_clock`'s (Doc 08 §13.4). `min_armed_expiry` only chooses the δ at which the oracle's own timer-fire RTC step will produce the next distinct config — it computes a `.min()` over `expiry_ms`, recursively through `submachines`; it does not fire, re-arm, or select a transition.

**Behavioural keystone proof (the strongest form — apply-and-compare vs the real oracle).** `crates/fsm-verify/tests/w2_hierarchy_acceptance.rs` builds `interpreter_reachable_set` (lines 65-205): a **separate** interpreter-driven BFS that lives in the test, drives the real `Interpreter` directly over the *same* edge families (every event + the `advance_clock` timer-fire edge), keyed by a hand-written clock-origin-normalized + trace-id-excluded + recursive key that **independently mirrors** `digest::canonical_bytes`. The acceptance tests then assert the verifier's `reachability.reachable` **byte-equals** this independent BFS for a composite/parallel fixture (`composite_parallel_sound_is_proven_and_matches_interpreter_bfs`, lines 209-235) and a submachine fixture (`submachine_machine_is_proven_and_subinstances_explored`, lines 425+). A forked LCA/region/completion/clock would diverge and fail. Deadlock witnesses are replayed through a **fresh** `Interpreter` and asserted to land in the reported config (`parallel_join_deadlock_…` lines 242-296; `genuine_timer_deadlock_…` lines 351-416), and from the deadlock the interpreter is asked to confirm no event/timer-fire progresses. This is the §4.1 keystone realised and proven against the real oracle, not symbol-presence.

**Finding 1.2: PASS.** Both W2 edge families are Interpreter-driven; the keystone is proven behaviourally against an independent interpreter BFS.

### 1.3 Honest-bound structurally enforced (PASS — F2)

I traced every `Verdict::ProvenNoDeadlock` and `StopReason::Exhausted` site:

- `Verdict::ProvenNoDeadlock` is **constructed at exactly one site** — engine.rs:483 — reached only when `while let Some(node) = frontier.pop_front()` exits **naturally** (frontier empty), with `stop_reason: StopReason::Exhausted` hard-coded at engine.rs:480. Every bound-hit path (`edges_explored >= opts.max_steps` at engine.rs:318,381; `visited.len() > opts.max_states` at engine.rs:353,408) returns `inconclusive(...)` **before** the loop can exhaust. The `Deadlock` terminal (engine.rs:459-468) also hard-codes `Exhausted`, but that is *deadlock-found* (a definite property violation walked-to), not a proof-of-absence — correct.
- `inconclusive()` (engine.rs:489-523) builds `Verdict::Inconclusive` and matches `reason`; the `StopReason::Exhausted` arm is `unreachable!("inconclusive() is never called when exhausted")` (engine.rs:510). So an `Inconclusive`-with-`Exhausted` (or a `ProvenNoDeadlock` on a truncated search) is **structurally unconstructible** — exactly the cardinal-sin guard, structurally enforced, not merely documented.
- **The E0400 dual is the same discipline:** `let exhaustive = matches!(stop_reason, StopReason::Exhausted); if exhaustive { …emit E0400… }` (diagnostics.rs:56-72). On a truncated search no E0400 ("this state is unreachable" from an incomplete search would itself be a false-proven). Tested: `no_e0400_when_search_was_truncated` (diagnostics.rs:265-295) asserts `MaxStatesHit ⇒ no E0400` while the structural W0602 still fires.

**Finding 1.3: PASS.** `ProvenNoDeadlock` is reachable only on the single exhausted exit; the non-exhausted path is guarded by `unreachable!()`. The honest-bound invariant is structurally enforced.

---

## 2. Lens 2 — Clock-merge soundness as a formal finding-of-record

W2's `digest::canonical_bytes` / `normalize_clock_origin_sub` (digest.rs:111-163) normalizes each runtime's `virtual_clock_ms`→0 and rewrites every armed timer's absolute `expiry_ms` to its **remaining duration** (`expiry_ms − virtual_clock_ms`, saturating), recursively per sub-instance (each sub by *its own* origin), also zeroing nested `next_trace_id`. This **merges clock-shift-equivalent configurations into one visited-set key.** A verification core's soundness argument must live in an audit record, not only a code comment — so here it is, **independently re-derived from the spec with the lines quoted**, not restated from `digest.rs` or the §11.1 report.

### 2.1 Is FSM-Lang's absolute virtual clock structurally non-observable? (YES — proven)

**(a) No clock-reading surface syntax exists.** Doc 04 §1.5 (the *single normative keyword list*, lines 80-91) — quoted in full:

> `after as bool cancel choice / composite const context deep_history defer / done else enum every export / extern f32 f64 false feature / final fork i8 i16 i32 / i64 import initial is join / junction language machine ms on / opaque parallel priority pure raise / region schedule send shallow_history state / submachine target to true u8 / u16 u32 u64`

and the contextual keywords (lines 100-105): `entry, exit, entry_point, exit_point, events, queue, if, while, for, likely, rare`. **There is no `now`, `at`, `time`, `clock`, `elapsed`, `uptime`, `timestamp`, or `deadline` token in either list.**

**(b) Guards cannot read time.** Doc 04 §8.5 (lines 680-702): `guard_expr = identifier (extern pure fn) | "!" identifier | field_ref cmp_op literal_or_field | && | || | grouping | "else"`; `field_ref = "ctx" . identifier | "payload" . identifier` (lines 689-691). Line 702 verbatim: **"No arithmetic. No function calls except extern pure references."** A guard is a pure predicate over `ctx`/`payload` fields and `pure extern` calls — no clock term.

**(c) Expressions cannot read time.** Doc 04 §8.7 (line 795): `primary = literal | field_ref | identifier | "(" expr ")"`. Function calls are postfix (§8.7.2); in guard context only `pure extern` is callable (§8.5 line 702 + §8.7 line 851-ish). There is **no clock/now/elapsed built-in primary**.

**(d) Externs cannot receive the clock.** Doc 04 §3.x (lines 1290-1293): `extern_decl = [doc_comment] , ["pure"] , "extern" , identifier , "(" , [param_list] , ")" , [":" , type]`; `param = type , identifier`. A `pure extern` guard's parameters are `ctx` and explicitly-typed scalars/opaque pointers (e.g. `pure extern can_unlock (ctx) : bool`, Doc 04 L236). The language has **no clock/time parameter to pass to an extern**, and the simulator's `virtual_clock_ms` is its private virtual time (Doc 08 §13.4 "the simulator's virtual clock mode") — a host C extern has no handle to it. (Even a host extern returning wall-clock time does not break *this verifier*: it drives the shipped Interpreter, which is presented no differing absolute origin between two clock-shifted configs, so the oracle's behaviour is identical by construction — this strengthens the verdict.)

**(e) Timer durations are compile-time constants.** Doc 04 §9 line 931 verbatim: **"`const_expr` MUST evaluate to a strictly positive integer at compile time."** §2.x lines 184-193: `const_expr = integer | boolean | string | identifier | const_expr (+|-|*|/) const_expr | (const_expr)`; line 193: "Constants are resolved at **compile time**." §9.1/§9.2/§9.3 (lines 938-967): every timer is `"after"/"every" , const_expr , "ms"` — **no `at <abs>` / deadline / runtime-variable-duration form.** Doc 08 §15.1 Note (lines 643-645) verbatim: **"Runtime-variable timer durations (`after ctx.green_ms ms`) are a post-v1.0 feature; v1.0 timers use compile-time constant values only."** — the one construct that could make a duration time-dependent is explicitly foreclosed for v1.0.

**(f) The operational semantics read the clock in exactly one place.** A whole-document scan of Doc 08 for `virtual.?clock|absolute.*clock|wall.?clock|now\(\)|current.*time` returns **only §13.4** (lines 603-606). `RTC_step` (§3.1 lines 69-81), `select_transitions` (§4.1 lines 105-130 — selection key `(t.priority, t.document_order)`, candidates `where trigger matches event and guard(t)=true`), guard evaluation (§4.3 lines 142-146 — "Guards MUST NOT have side effects … contain no assignments"), LCA (§5), completion (§4.4) — **none reference time**. §13.4 (lines 605-606) verbatim: **"In the simulator's virtual clock mode, `M_tick(elapsed)` MUST process all timers that would have fired in the elapsed interval, in chronological order."** — the host injects a **relative `elapsed` delta**; the program never *reads* the absolute clock. §13.1 (timer starts on owning-state entry ⇒ `expiry_ms = entry_clock + X`), §13.3 (`next_fire = last_scheduled_fire + X` — a relative, drift-free recurrence), §14 (timer events, once synthesized, are ordinary events processed by the clock-free §3.1/§4.1 path).

### 2.2 The bisimulation argument (independently stated, with citations)

The only place absolute time enters the operational semantics is the timer-fire condition (§13.4 + §13.1: a timer fires when its owning runtime's `virtual_clock_ms` reaches `expiry_ms`). Every other RTC component is a function of `(active config, event, ctx, history, defer, payload)` and never reads `virtual_clock_ms` (§3.1/§4.1/§4.3/§5/§4.4), and §2.1(a)-(e) prove no surface syntax can read it.

Define R = {(C₁, C₂)} for configurations C₁, C₂ identical in (active states, history, defer_set, context, recursive sub-instance structure) and whose every armed timer has the **same remaining duration** (`expiry_ms − virtual_clock_ms`, recursively per sub-instance with its own origin). R is a **strong bisimulation**:

- **Event/completion edges:** identical effect from C₁ and C₂ (no clock dependence — §3.1/§4.1/§4.3/§4.4).
- **Timer-fire edge:** advancing each by the same δ fires the *same* timer set in the *same* chronological order (§13.4) with the *same* effect; `every`-restart preserves the same relative phase (`next_fire − new_clock = X` in both — §13.3).

The successor sets are pointwise R-related. A uniform shift of `(virtual_clock_ms, all expiry_ms, recursively)` is exactly such an R-pair. Merging an R-class therefore:

1. **Cannot hide a reachable deadlock** — the merged representative has provably identical future behaviour, so a deadlock is reachable from one iff from the other. **⇒ a false `ProvenNoDeadlock` is impossible by this mechanism.** (Soundness of the cardinal-sin direction.)
2. Is **required** for termination: without it a cyclic timer / `every_internal` heartbeat advances the clock forever, the digest never repeats, the visited set never converges, and a genuine timer-deadlock is wrongly reported `Inconclusive`. This is **empirically confirmed** by `genuine_timer_deadlock_is_detected_with_oracle_checked_witness` (w2_hierarchy_acceptance.rs:351-416), which asserts the verdict is `Deadlock` (not `Inconclusive`) and cross-checks via a fresh interpreter that the `every_internal` timer ticks but the configuration is invariant.

The digest is sound in the *splitting* direction too (different remaining phase ⇒ different bytes): `normalize_clock_origin_sub`/`canonical_bytes` keep the *relative* phase and **all** nested sub-instance structure in the key (digest.rs:111-157), tested by `distinct_relative_timer_phase_differs` and `distinct_nested_subinstance_state_differs` (digest.rs:274-353), and the clock-shift collision is tested by `clock_shift_equivalent_configs_collide` (digest.rs:244-271).

**Finding 2: PASS — clock-merge soundness fully confirmed by my own derivation.** No FSM-Lang construct (guard / expression / `pure extern` parameter / timer duration / keyword) can observe absolute virtual time; Doc 08 reads the clock only at §13.4 via a relative `elapsed`. The normalization identifies only strongly-bisimilar configurations ⇒ **no false `ProvenNoDeadlock` is possible by this mechanism**, and the merge is *required* for cyclic-timer termination. **No P0; verdict is NOT BLOCK-W3 on this axis.** (Citation-precision nit F8/NOTE-2 below — the conclusion is independent of it.)

---

## 3. Lens 3 — Did W2 actually close §11.3-post-W1 D-2?

D-2 (the W1-audit headline): `InterpreterSnapshot`/`snapshot()`/`restore()` were lossy for `RuntimeState.submachines` + `timers`, so the digest could conflate behaviourally-distinct submachine/timer configs → premature visited-set pruning → false `ProvenNoDeadlock`. W2's P0 was to fix this *first*.

### 3.1 The snapshot is now lossless (PASS)

`InterpreterSnapshot` (runtime/state.rs:280-299) now has, in declaration order: `active_states, history, defer_set, virtual_clock_ms, timers (NEW, runtime/state.rs:290), context, next_trace_id, initialized, submachines (NEW, runtime/state.rs:298)`. `SubmachineSnapshot` (runtime/state.rs:241-256) mirrors it recursively (`submachines: BTreeMap<String, SubmachineSnapshot>` at runtime/state.rs:255). `snapshot()` (interpreter.rs:415-438) captures `timers: rt.timers.snapshot_sorted()` (interpreter.rs:432) + `submachines: rt.capture_submachines()` (interpreter.rs:436); `restore()` (interpreter.rs:444-464) calls `rt.timers.restore_from(snap.timers)` (interpreter.rs:458) + `rt.restore_submachines(snap.submachines)` (interpreter.rs:462). `capture_submachines`/`capture_as_sub` (runtime/state.rs:134-155) recurse; `restore_submachines`/`overlay_from_sub` (runtime/state.rs:168-198) rebuild each sub's skeleton from `self.machine` + the ref-state key via `build_sub_runtime` (the same path `sync_submachines` uses) and overlay the captured mutable config, recursing into nested subs — so a config restored mid-sub-instance resurrects the *exact* nested state, not a torn-down one (the pre-W2 lossy-restore bug). The timer set is canonicalized to a **sorted** `Vec<Timer>` (timer.rs:100-119 `snapshot_sorted`, total key `(timer_id, source_state, transition_id, expiry_ms, fire_ord)`) so two configs with the same armed *set* reached via different arm orders serialize identically.

### 3.2 The change is purely additive ⇒ W1/simulator zero-regression (PASS)

Pre-W2 `InterpreterSnapshot` (per the frozen W1 audit §1.3 / runtime/state.rs:132-141 at `dd2d41e`) captured exactly `active_states, history, defer_set, virtual_clock_ms, context, next_trace_id, initialized`. At `3447881` **every one of those keeps its name, type, and relative position**; the two new fields are *appended* (`timers` after `virtual_clock_ms`, `submachines` last). serde_json serializes struct fields in declaration order; for a flat machine `timers` is `[]` and `submachines` is `{}`. The regression gate `flat_machine_snapshot_round_trips_and_new_fields_are_empty` (snapshot_lossless.rs:397-435) asserts **literally** `body.contains("\"timers\":[]") && body.contains("\"submachines\":{}")` (snapshot_lossless.rs:420-424) — i.e. the additive-not-breaking guarantee is enforced by a test that inspects the serialized bytes, not merely asserted in a comment. The §11.1 independent re-run further confirmed flat snapshots byte-unregressed on a fresh interpreter (`/tmp/w2-indep-rerun-report.md` §Task2(c): `169==169`, `"timers":[]`,`"submachines":{}`).

### 3.3 The round-trip gate is real and substantive (PASS)

`crates/fsm-simulator/tests/snapshot_lossless.rs` (435 lines, the W2-P0 commit `efae559`). `assert_round_trip_lossless` (snapshot_lossless.rs:59-72) is not a stub: it `snapshot()`s, **actually perturbs** the interpreter (`perturb(interp)` — e.g. dispatching `ACK` to advance a sub-instance, or `START` to move a flat machine), `restore()`s the *original* snapshot, re-`snapshot()`s, and asserts the two JSON encodings are **byte-identical**. Coverage: (P0a) submachine mid-sub-instance + "restore resurrects the EXACT nested leaf" (snapshot_lossless.rs:102-148); (P0b) armed-timer mid-phase + distinct-phase-distinct-snapshot (snapshot_lossless.rs:198-298); (P0c) hierarchical+parallel+timer (snapshot_lossless.rs:340-385); flat-machine regression (snapshot_lossless.rs:397-435). The §11.1 re-run independently re-derived a/b/c byte-exact on a throwaway crate driving the *shipped* interpreter (not the W2 suite) — corroborated.

**Finding 3: PASS.** W2 closed post-W1 D-2 correctly: the snapshot is lossless for `timers` + recursive `submachines`, purely additive (flat snapshots byte-identical, test-enforced on the serialized bytes), with a real, substantive round-trip byte-identity gate. The D-2 → false-`ProvenNoDeadlock` foot-gun is closed in the code; the Doc 30 §4.1 *prose* that mis-stated it as already-done is **still uncorrected** — F5, closeout-batch (the project's batched-doc discipline, not a mid-epic fix).

---

## 4. Lens 4 — Doc/backlog reconciliation STATUS (record, do not fix)

Enumerated precisely. Each: says-now / reality / closeout-edit. **None is fixed here** — the v1.4 batched-doc discipline folds these at the v1.4 closeout, not mid-epic; this section *records what must fold* (the §11.62 discipline).

- **F5 — Doc 30 §4.1 line 192.** *Says-now:* "The visited-set key is the existing `InterpreterSnapshot` (`active_states` + `history` + `defer_set` + `context` + `virtual_clock_ms` — already `Clone`, already the complete state)." *Reality:* false for W2's submachine/timer scope; W2's P0 (`efae559`) extended the snapshot with `timers` + recursive `submachines` precisely because it was **not** complete. *Closeout-edit:* annotate §4.1 that the snapshot was lossy for `submachines`/`timers`, that W2's first unit was the lossless extension (round-trip gated), and that the W1-audit D-2 flagged this.
- **F6 — Doc 30 §4.2-W1 line 204 / §4.3 lines 251-274.** *Says-now:* the `fsm verify` CLI subcommand + FSM-E0400/W0602 emission + composite/parallel/timer/submachine are **W2**; W1 is "library-only … flat machines"; "composite/parallel/timer/submachine are W2 — do NOT attempt them." *Reality:* per Doc 00 §11.63 the CLI + E0400/W0602 shipped in **W1** (`2866821`); the W1-audit recorded this (D-1, authorized by the owner-confirmed brief). *Closeout-edit:* annotate §4.2-W1/§4.3 that the CLI + E0400/W0602 landed in W1 per §11.63 (not W2), and that the §4.3 sub-brief was superseded by the §11.63 authoritative brief (the v1.2/v1.3 precedent).
- **F7 — Doc 30 §4.2-W2 line 206 + §4.2 preamble line 199.** *Says-now:* "**§11.3 phase-boundary audit after W2** (the keystone+breadth boundary, before W3 builds on it)." *Reality:* the §11.3 audit was re-sequenced to run **post-W1** (predecessor doc `a75cd69`, keystone-wave audit) **and** post-W2 (this doc). *Closeout-edit:* annotate that the §11.3 cadence is two audits — post-W1 (keystone) and post-W2 (breadth) — per the W1-audit D-1 re-sequencing rationale.
- **F8 — `digest.rs` "Doc 08 §13" citation (lines 23, 61, 72, 245); independent re-run NOTE-2.** *Says-now:* the code attributes absolute-clock non-observability to "Doc 08 §13." *Reality:* Doc 08 has **no §13.5 and no verbatim "the absolute clock is non-observable" clause** (whole-doc grep confirms); the property is *true and structurally entailed* by §13.1/§13.3/§13.4 + §3.1/§4.1/§4.3 + Doc 04 §1.5/§8.5/§8.7/§9 (the derivation in §2 above), but the citation points at an entailed property, not a stated one. *Closeout-edit:* **either** add an explicit §13.5 non-observability lemma to Doc 08 **or** correct the `digest.rs` comment to cite the structural basis (Doc 04 §1.5/§8.5/§8.7/§9 + Doc 08 §3.1/§4.1/§4.3/§13.1/§13.3/§13.4/§14/§15.1). The soundness conclusion (§2) is independent of this nit.
- **F9 — `fsm-verify/vN` versioning policy.** *Says-now:* the binding additive-vs-breaking policy is fully specified, but **only** in the `verify.rs` module doc-comment (verify.rs:70-101 — well-formed: additive keeps major, breaking bumps, the exit-code contract is part of the versioned surface, co-located "so a factory integrator needs no other doc"). No Doc 18/Doc 13 reflection. *Reality:* the W1 audit recommended this policy be stated explicitly; W2 satisfied that in-source — a real improvement (record it as such, not a defect). The open item is the **factory-contract `fsm-verify/vN` JSON-schema versioning-policy doc** at the spec layer (Doc 18 §3 or a dedicated section), so the contract is discoverable without reading source. *Closeout-edit:* fold the verify.rs policy into Doc 18 (CLI Specification) as the canonical factory-contract reference.
- **F10 — W1-audit path citation.** *Says-now:* the frozen W1 audit cites `crates/fsm-simulator/src/state.rs:62-71` in the §1.3 finding/§6 summary (the body §1 / evidence index correctly use `runtime/state.rs`). *Reality:* the canonical path is `crates/fsm-simulator/src/runtime/state.rs`; there is no `crates/fsm-simulator/src/state.rs`. *Closeout-edit:* record the path-citation correction (the W1 audit is a frozen evidence doc — correct in the closeout ledger, do not overwrite the W1 audit).
- **§11 v1.4 impl rows — correctly DEFERRED, not missing.** Doc 00 §11 rows end at **§11.63** (the v1.4 scope/architecture row, accurate). The closeout note (Doc 00 line 1346) confirms v1.4 *implementation* rows are intentionally deferred to the v1.4 batched closeout (the v1.3 §11.50-62 precedent). **Verified correctly deferred — nothing mid-stream is owed.** §11.63 itself is accurate at the W2 boundary.
- **No new W2-introduced doc↔shipped drift** beyond the above. `#![forbid(unsafe_code)]` is present at `crates/fsm-verify/src/lib.rs:51`; `[lints] workspace = true` at fsm-verify/Cargo.toml:13-14; the §11.1 re-run confirmed 12 forbid roots / clippy+fmt clean at `3447881`.

**Finding 4:** the doc drift is exactly F5-F10 plus the (correct) §11-rows-deferred status. All are closeout-batch; none blocks W3.

---

## 5. The frozen v1.4-CLOSEOUT DOC-BATCH CARRY-LIST

**FROZEN. This is the single numbered checklist the v1.4-closeout agent executes in ONE batch** (the batched-doc discipline — do not fold piecemeal mid-epic). It consolidates the post-W1 audit's D-1, the post-W2 findings F5-F11, and the independent re-run's NOTE-2. Every item is doc/process only — no code change is owed (the code is sound).

1. **[from W1-audit D-1] Doc 30 §4.2-W1 + §4.3:** annotate that the `fsm verify` CLI + FSM-E0400/W0602 emission + reachability report landed in **W1** (`2866821`), per Doc 00 §11.63 (not W2 as §4.2-W1/§4.3 still say); record that the §4.3 "library-only / flat-only" sub-brief was superseded by the §11.63 owner-confirmed authoritative brief (the pipeline-before-UI gate); the `Verdict::Deadlock { witness: Vec<String> }` shape (not `Vec<StepRecord>`) is the accepted better choice. *(W1-audit §4 item 6 / §3.2 D-1.)*
2. **[from W1-audit D-2 → F5] Doc 30 §4.1 line 192:** correct/annotate "`InterpreterSnapshot` … already the complete state" — it was lossy for `RuntimeState.submachines` + `timers`; W2's first unit (`efae559`) extended the snapshot losslessly (recursive `submachines` + sorted `timers`), round-trip-gated; the W1-audit D-2 flagged the overstatement. This is the load-bearing one — it must not be lost.
3. **[F10] W1-audit path-citation correction:** the canonical path is `crates/fsm-simulator/src/runtime/state.rs` (the W1 audit's §1.3 finding/§6 summary say `crates/fsm-simulator/src/state.rs`; no such file exists). Record in the closeout ledger; **do not overwrite** the frozen W1 audit `docs/AUDIT_PHASE_V1_4_W1_2026_05_16.md`.
4. **[F8 / NOTE-2] digest.rs "Doc 08 §13" citation:** **either** add an explicit Doc 08 §13.5 "absolute-virtual-clock non-observability" lemma (citing §13.1/§13.3/§13.4 + §3.1/§4.1/§4.3 + Doc 04 §1.5/§8.5/§8.7/§9) **or** correct the `digest.rs` module/comment citations (lines 23, 61, 72, 245) to the structural basis above. The §2 soundness conclusion is independent of which option is chosen.
5. **[F9] Factory-contract `fsm-verify/vN` JSON-schema versioning-policy doc:** fold the binding policy currently in `verify.rs:70-101` (additive keeps major / breaking bumps major / exit-code contract is part of the versioned surface) into Doc 18 (CLI Specification) as the canonical, discoverable factory-integration reference. Record that W2 already satisfied the W1-audit "state the policy explicitly in-source" recommendation (a positive credit, not a defect) — this item is the *spec-layer* surfacing only.
6. **[F6/F7] v1.4 §11 impl rows + Doc 30 status reconciliation:** at the v1.4 batched closeout, append the v1.4 implementation rows to Doc 00 §11 (currently correctly deferred; rows end at §11.63), and reconcile Doc 30 §4.2-W1/-W2/-W3 + §4.3 + the §4.2 preamble line 199 to merged W1+W2 reality, including the two-audit §11.3 cadence (post-W1 keystone audit `a75cd69` + post-W2 breadth audit, this doc) and the backlog status/wave-plan lines.
7. **[F11 / NOTE-1 — validated operational fix] W4 §11.30-clean tag-gate hardening:** the W4 pre-tag procedure MUST include a mandatory `cargo clean` (or a non-shared `CARGO_TARGET_DIR`) **before** any full-workspace test gate. Rationale (reproducible, validated this epic): a stale parallel-worktree test binary (`env!("CARGO_MANIFEST_DIR")` baked at a deleted `…-wt-v14w2/` worktree's compile time, reused via the shared `target/` because a scoped `cargo clean -p` only refreshed the 3 W2 crates) masqueraded as 6 tag-blocking FAILED tests in `fsm-formatter`/`fsm-lexer`/`fsm-parser` — crates W2 never touched (`git show --stat 3447881` = zero files there); they PASS on recompile. The orchestrator independently cleared it (full clean re-run = 114 ok / 0 FAILED on `3447881`), so NOTE-1 is **resolved-operational, not open** — but the W4 §11.30 tag procedure must *pre-empt* it so it never masquerades as a real red for the pre-tag auditor or the gate.

---

## 6. Lens 5 — W3 readiness

W3 (Doc 30 §4.2-W3, line 208) = trace differential replay: `fsm test --baseline <dir>`, a capture-and-compare harness consuming `execute_trace`/`first_mismatch`, with a baseline-trace corpus captured from `a036c38` (v1.3 semantics) as the drift oracle; sequence after W2 (§3.2 one-build-heavy-wave posture).

**The seam is located, stable, and keystone-compatible (PASS — F12).** `crates/fsm-simulator/src/trace.rs`:

- `pub struct TraceResult { actual: Vec<StepRecord>, matches_expected: bool, first_mismatch: Option<usize> }` (trace.rs:243-253).
- `pub fn execute_trace(ir, trace) -> Result<TraceResult, ExecError>` (trace.rs:280-334): builds one `Interpreter`, `init`s it, replays each `TraceCommand` (`Dispatch`/`AdvanceClock`/`Raise`) **through the real interpreter** (`interp.dispatch_with_payload` / `interp.advance_clock` / `interp.raise_with_payload`), then compares `actual` vs `trace.expected` element-wise with a tail-length-mismatch fallback (trace.rs:317-328). The step comparator is the interpreter's own output vs the expected corpus — **W3 consumes this; it must not re-implement step comparison or re-derive `StepRecord`s** (the §4.1 B-seam constraint is enforceable because the comparator already exists and is the conformance suite's oracle — `fsm-cli/src/cmd/test.rs` is built on it). This is the *same keystone discipline*: W3 drives `execute_trace`, it does not fork semantics. No W2 decision impedes it; W3 is independent of the §2 clock-merge and §3 snapshot work (it replays linear traces, it does not snapshot/restore-explore).

**W3 hazards to carry into the W3 brief (not blockers — W3-input):**

- **Trace format versioning.** `parse_trace_yaml`/`write_trace_yaml` (trace.rs:269-276) are **JSON in v1.0** with the comment "same shape as the eventual YAML format." W3's baseline corpus is a *persisted artifact*; the W3 brief must pin a trace-file schema/version marker (analogous to `fsm-verify/v1`) so a `a036c38`-captured baseline remains parseable across format evolution, and decide the corpus's stability contract before committing it (the F9 versioning-policy discipline applies symmetrically to the trace corpus).
- **Nondeterminism / oracle discipline.** `StepRecord` equality is the drift signal; the W3 brief must confirm `StepRecord` is fully deterministic across builds (the `BTreeMap`/`Vec`/sorted-key wire-format discipline that `ConfigDigest` relies on — Doc 13 §11) so a "drift" is a real semantic change, not a serialization-order artifact. Capturing the baseline at `a036c38` and asserting an unchanged build round-trips clean (the §5.4 acceptance) is the right gate.
- **Digest/oracle discipline parity.** W3 does not need the clock-origin normalization (it replays a fixed trace, not an explored state space) — but if W3 ever de-dups or canonicalizes traces it must reuse the §2-proven normalization, not fork a parallel one (the digest.rs "do not invent a bespoke field-walk" discipline).

**Finding 6: PASS.** W3 is cleanly buildable on the existing `execute_trace`/`first_mismatch` seam under the same keystone discipline; no W2-introduced rework risk. The hazards are trace-format/versioning + determinism, to be pinned in the W3 brief.

---

## 7. Evidence index (the spot-checks that prove the code/spec was read)

| Claim | Evidence (file:line) |
|---|---|
| Every `fn` enumerated; zero second semantics | engine.rs:81,173,253,489,525,569; deadlock.rs:99,136; digest.rs:111,146,166; reachability.rs:42,82; diagnostics.rs:50,96,105,141,172; negative grep (non-comment) = ∅ |
| Explorer = pure Interpreter orchestration | engine.rs:261-267 (new/init), :294,:332 (restore), :335 (dispatch), :397 (advance_clock), :336,:268,:281 (snapshot/current_states); module doc engine.rs:15-45 |
| `fsm-analyzer` dev-only | crates/fsm-verify/Cargo.toml:21-34 (runtime deps) vs :36-42 (dev-deps) |
| Honest-bound structural | engine.rs:483 (sole `ProvenNoDeadlock`), :480/:457 (`Exhausted` hard-coded), :318,:353,:381,:408 (bound-hit → `inconclusive` first), :510 (`unreachable!`) |
| E0400 exhaustive-gated dual | diagnostics.rs:56-72; test :265-295 (`no_e0400_when_search_was_truncated`) |
| Keystone proven vs independent interpreter BFS | w2_hierarchy_acceptance.rs:65-205 (`interpreter_reachable_set`), :209-235 (composite/parallel byte-equal), :425+ (submachine byte-equal), :242-296/:351-416 (witness replay through fresh interpreter) |
| No clock-reading surface syntax | Doc 04 §1.5 L80-105 (keyword lists), §8.5 L680-702 ("No arithmetic. No function calls except extern pure references"), §8.7 L795 (`primary`), §3.x L1290-1293 (`extern_decl` params), §9 L184-193/L931 (`const_expr` compile-time), §15.1 L643-645 (runtime-var durations post-v1.0) |
| Clock read in exactly one Doc 08 place | Doc 08 §3.1 L69-81, §4.1 L105-130, §4.3 L142-146 (no time term); §13.4 L603-606 (only `virtual clock` occurrence; relative `elapsed`); §13.1/§13.3 L577-601 (relative phase); §14 L616 |
| Snapshot lossless + additive | runtime/state.rs:280-299 (struct, new fields :290,:298), :134-198 (capture/restore recursion); interpreter.rs:415-464; timer.rs:100-127; snapshot_lossless.rs:59-72 (real round-trip), :420-424 (literal `"timers":[]`/`"submachines":{}` assertion) |
| Round-trip gate substantive | snapshot_lossless.rs:102-148 (P0a sub), :198-298 (P0b timer), :340-385 (P0c hsm/parallel), :397-435 (flat regression) |
| W3 seam stable + keystone-compatible | trace.rs:243-253 (`TraceResult.first_mismatch`), :280-334 (`execute_trace` drives real interpreter), :269-276 (JSON-as-v1.0-YAML versioning hazard) |
| Doc 30 stale sections (status) | docs/30-…:192 (F5), :204/:251-274 (F6), :206/:199 (F7), :208 (W3 seam confirm) |
| §11 v1.4 rows correctly deferred | Doc 00 §11.63 line 1342 (accurate scope/arch row); line 1346 (v1.4 impl rows closeout-deferred — v1.3 §11.50-62 precedent) |
| NOTE-1 reproducible + cleared | /tmp/w2-indep-rerun-report.md §Cold-quad (6 FAILED = stale `…-wt-v14w2/` binaries; PASS on recompile; full clean = 114 ok/0) |

---

## 8. Summary for the orchestrator

- **W3-readiness: `PROCEED-WITH-NOTES`.** W2 is a sound breadth wave — no P0, no W3 ship-blocker.
- **Keystone (highest priority): PASS, independently re-derived.** All 15 non-test `fn`s in `fsm-verify/src` enumerated and classified — every one is a structural IR-shape read or pure `Interpreter` orchestration. Zero transition-selection / guard-eval / timer-fire / LCA / region-join/completion logic anywhere (negative grep on non-comment lines = ∅; I read the filter, not an echo branch). `fsm-analyzer` is dev-only. Honest-bound is structurally enforced (single `ProvenNoDeadlock` site on the exhausted exit; `unreachable!()` guards the non-exhausted path). Proven behaviourally against an independent interpreter-driven hand-BFS over composite/parallel/submachine fixtures + witness replay through a fresh interpreter.
- **Clock-merge soundness: PASS, formally recorded.** Independently derived from quoted Doc 04 §1.5/§8.5/§8.7/§9 + Doc 08 §3.1/§4.1/§4.3/§13/§14/§15.1: **no FSM-Lang construct** (keyword / guard / expression / `pure extern` parameter / timer duration) can observe the absolute virtual clock; Doc 08 reads the clock only at §13.4 via a *relative* `elapsed`. The origin→0 + remaining-duration normalization therefore identifies only strongly-bisimilar configs ⇒ **a false `ProvenNoDeadlock` is impossible by this mechanism**, and the merge is *required* for cyclic-timer termination (empirically confirmed by the genuine-timer-deadlock test asserting `Deadlock`, not `Inconclusive`). NOT BLOCK-W3 on this axis.
- **Post-W1 D-2 closed: PASS.** `InterpreterSnapshot` is now lossless for `timers` + recursive `submachines`, purely additive (flat snapshots byte-identical — test-enforced on the serialized bytes), with a real 435-line round-trip byte-identity gate. The Doc 30 §4.1 prose that mis-stated it remains uncorrected — **correctly closeout-deferred** (carry-list item 2), not a mid-epic fix.
- **Doc reconciliation: all closeout-batch, none blocks W3.** F5-F11 + W1-audit D-1 consolidated into the FROZEN §5 carry-list (7 numbered items the closeout agent runs in one batch). v1.4 §11 impl rows verified **correctly deferred**, not missing. The most useful deliverable is that frozen list — especially item 2 (the load-bearing D-2 Doc 30 §4.1 correction) and item 7 (the validated NOTE-1 → mandatory `cargo clean` in the W4 tag-gate).
- **W3-readiness: PASS.** The `execute_trace`/`first_mismatch` seam (trace.rs:280-334) is stable and keystone-compatible; W3 consumes it without forking semantics. Hazards (trace-format versioning, `StepRecord` determinism) carried as W3-brief inputs.
- **Branch:** `phase4.3/v1_4-w2-audit` (this doc only; not merged/pushed; no implementation, no other doc touched). For orchestrator review before merge → W3 dispatch.

*End of FSM-AUDIT-PHASE-V14-W2 v1.0.0 — frozen evidence; do not overwrite. A rubber-stamp here would be worse than a found problem; the keystone and clock-merge soundness are stated PASS because each was independently confirmed by this auditor's own reading of source + the quoted spec, not restated from the prior reports.*
