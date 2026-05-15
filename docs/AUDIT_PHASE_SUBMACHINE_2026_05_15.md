# Phase-Boundary Audit — Submachine Epic (W2a–W2d)

**Audit type:** §11.3 phase-boundary audit (post analyzer/codegen/simulator triad, pre W4–W7)
**Date:** 2026-05-15
**Repo:** `/root/dev/embeded-fsm-sdk` · branch `main` · HEAD `75aa0dd`
**Mode:** READ-ONLY (no branch/commit; this file is the only output)
**Auditor mandate:** skeptical claim-vs-code verification — the submachine epic was *born* from a false "already implemented" claim ([[verify-status-claims-vs-code]]); new "done" claims get the same treatment.

---

## Phase-Boundary Verdict

```
⚠️ PROCEED WITH NOTED P1s

The supported scope — top-level `state X is Sub` — is verified solid end-to-end
(parser → analyzer/IR → simulator → codegen), independently re-derived, no P0.
Two P1s must be addressed before/alongside W4–W7:
  P1-1  cargo test --workspace FAILS on main (stale W2d-worktree-compiled
        formatter test binary baked an absolute CARGO_MANIFEST_DIR that no
        longer exists). §11.1 post-merge quad gap. NOT a submachine-code defect.
  P1-2  SUB-FU-2 (submachine ref nested in composite/parallel) is documented as
        "emitted as an inert leaf" but actually emits non-compilable C
        (dangling entry_/exit_ calls, no prototypes) — fails the project's own
        mandated gcc -Werror. The deferral docs (CHANGELOG/ROADMAP/backlog) are
        inaccurate; "full-UML coverage" is a residual overstatement.
```

**Severity counts:** P0 = 0 · P1 = 2 · P2 = 1 · P3 = 2

**Checkpoint tag recommended: NO** — not until P1-1 is fixed (a clean
`cargo test --workspace` is a precondition for a "known-good" tag; tagging
`75aa0dd` would anchor a state whose verify quad does not pass as-merged).
Once P1-1 is resolved and the quad is green, `75aa0dd`-equivalent is a strong
checkpoint candidate (the submachine *code* itself is sound).

---

## Part 1 — Submachine epic: claim vs code (skeptical)

| Wave | Headline claim | Verification performed | Holds? | Evidence |
|---|---|---|---|---|
| **W2a** parser | `submachine X {}` + `state X is Sub` actually parsed; `KwIs`/`KwSubmachine` consumed (not orphan tokens) | Read grammar `state.rs` + `top_level.rs`; ran `fsm parse --emit-ast` + `fsm check` on the example and on 6 crafted inputs | ✅ YES | `crates/fsm-parser/src/grammar/top_level.rs:309 parse_submachine_decl` (real `SUBMACHINE_DECL` node); `crates/fsm-parser/src/grammar/state.rs:71-72,94 parse_submachine_ref` (`SUBMACHINE_REF` child of `STATE_DECL`). `fsm check examples/submachine/submachine.fsm` → exit 0 (old orphan-token FSM-E0010 gone). |
| **W2b** analyzer/IR | `lower.rs` populates `MachineObject.submachines` (NOT `Vec::new()`) + emits `StateNode::Submachine`; FSM-E0103/E0610/E0500/E0501/E0502 emitted; IR schema-valid (W0 gate) | Grepped for hard-coded empty; read `lower/machine.rs:188`; emitted real IR via `fsm generate --emit-ir`; crafted one negative `.fsm` per diagnostic | ✅ YES | `crates/fsm-analyzer/src/lower/machine.rs:188-201` iterates `ast_file.submachines()` and recursively lowers (the `else { Vec::new() }` is the correct recursion base, not the P0 pattern). The only `submachines: vec![]` literal is `lca.rs:199` — a `#[cfg(test)]` fixture builder, not a production path. Emitted `Device.ir.json`: `machines[0].submachines` = full populated `Connection` template; `machines[0].root.states[1]` `kind="submachine_ref"`, `submachineId="m-Connection"`. All 5 diagnostics fire on crafted bad input (E0610 no-feature, E0103 unknown-ref, E0500 no-entry, E0501 done-without-final, E0502 A→B→A cycle). IR-schema gate is `default=["schema-validate"]` in both `fsm-analyzer` and `fsm-ir` Cargo.toml; `cargo test -p fsm-analyzer --test ir_schema_gate` 4/4 incl. negative-rejection test. |
| **W2c** simulator | Sim actually RUNS the sub-instance (not a defensive no-op); `.trace` substantive, not trivially-passing | Read `runtime/submachine.rs` + `interpreter.rs:700-934`; ran `fsm test examples/submachine/`; read the 10-record trace | ✅ YES | `interpreter.rs:706-777` instantiates newly-active ref-states (full entry sequence + context init); `:779+` completion sweep fires parent `done ->` via the **existing R1 path**; `:884-918` delegates parent-unconsumed events into the sub's queue and runs it to quiescence one depth deeper. `examples/submachine/submachine.trace` has a **populated 10-record `expected`** exercising Idle→Handshake→Established→Done + `__completion__:s-Device-Connecting`→Online — not `expected: []`. `fsm test examples/submachine/` 1/1. The "leaf no-op" branch (`build_sub_runtime` → `None`) only triggers on an unresolved ref that FSM-E0103 already rejects at analysis — correct partial-IR defence, not the supported path. |
| **W2d** codegen | Sub-instance is a real nested struct member with `Sub_init`/`Sub_dispatch` delegation (not inert leaf); compiles `-Werror` clean both strategies; hand-driven run advances parent on sub-completion | Generated C via CLI for `auto` + `table`; gcc `-std=c99 -Wall -Wextra -Wpedantic -Werror`; read generated `Device.h`/`Device.c`; wrote an **independent** driver against the real CLI-generated API (not the test's IR fixture) and executed it | ✅ YES | `Device.h:63` `Connection_t _sub_CONNECTING;` (real nested member); `Device.c:69,88,215` `Connection_init(&m->_sub_CONNECTING)`; `:158` `Connection_dispatch(&m->_sub_CONNECTING,&__sev)`; `:174` `Connection_current_state(...) == CONNECTION_STATE_DONE` drives the parent `done ->`. Both strategies compile clean under the strict flags. Independent run: `init parent=CONNECTING sub=IDLE → CONNECT sub=HANDSHAKE (parent stays) → ACK sub=ESTABLISHED → ESTABLISHED parent=ONLINE (sub-completion fired done-> with NO external event) → RECONNECT parent=CONNECTING sub=IDLE (fresh re-init)`. Existing `submachine_codegen_runs.rs` is an exemplary §5.4 test: exit-code behavioural assertions over BOTH strategies, NOT `.contains()`. |
| **SUB-FU-2** | Submachine ref nested in composite/parallel is *inert (leaf) but NOT broken* (doesn't crash/miscompile) — limitation honestly documented | Constructed a composite-nested case AND a parallel-nested case; `fsm check` + `fsm generate` + strict gcc; ran sim | ❌ **NO — claim is false** | See **P1-2** below. Composite case: `Outer.c:79 Outer_exit_SUB(m)` + `:164 Outer_entry_SUB(m)` emitted, but `Outer_impl.h` declares **zero** `SUB` prototypes → `gcc -Werror`: `error: implicit declaration of function 'Outer_exit_SUB'`. Parallel case identical (`P.c:75 P_exit_S1`, `:171 P_entry_S1`, undeclared). It does NOT crash the generator and the **simulator side is genuinely inert-safe** (no panic; deterministic trace) — but it **miscompiles**, contradicting the documented "inert leaf". |
| CHANGELOG / ROADMAP / backlog accuracy | Wording reconciled to code, not re-overstated | Cross-read `CHANGELOG.md:8-25`, `ROADMAP.md:56`, backlog memory L156-157 | ⚠️ **Partially overstated** | The supported-scope wording (top-level `is Sub` end-to-end, gcc+sim≡codegen) is **accurate**. But SUB-FU-2 is uniformly described as "**emitted as an inert leaf**" (CHANGELOG L14/17, ROADMAP L56, backlog L157) and the CHANGELOG concludes "**v1.1 reaches full-UML coverage**". Both are inaccurate: a nested submachine does not degrade to a harmless leaf — it produces C that won't build under the project's own mandated `-Werror`. Same *class* as the P0-1 prose-vs-code drift (mischaracterizing a gap as more graceful than reality), though far milder — it is an *explicitly flagged* known limitation, not a silent one, and the supported path is genuinely complete. |

### Per-wave conclusion
W2a, W2b, W2c, W2d **all hold under skeptical verification for their stated supported scope** (top-level `state X is Sub`). The epic is real, not aspirational prose — independently re-derived end-to-end with my own driver against CLI-generated C. The single substantive defect is in the *explicitly-deferred* SUB-FU-2 path, where the deferral is mischaracterized (P1-2).

---

## Part 2 — Cumulative v1.1 regression / debt sweep

### Verify quad (literal, run on `main` @ `75aa0dd`)

| Command | Result |
|---|---|
| `cargo build --workspace` | ✅ exit 0 — `Finished dev [unoptimized + debuginfo] in 8.80s` (warm cache) |
| `cargo test --workspace` | ❌ **FAIL** — `every_fixture_is_idempotent` (fsm-formatter) panics: `read_dir "/root/dev/embeded-fsm-sdk-wt-w2d/crates/fsm-formatter/tests/fixtures": No such file or directory`. Deterministic across 3 runs. **Passes when run isolated** (`cargo test -p fsm-formatter --test idempotency` → ok) and **passes after the test file is recompiled**. Root cause = stale artifact, NOT code (see P1-1). With a fresh-compiled formatter test, full workspace = **294 tests passed, 0 failed**. |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ exit 0 — `Finished … in 8.31s`, zero warnings |
| `cargo fmt --check --all` | ✅ exit 0 |

### Functional smoke

| Check | Result |
|---|---|
| `fsm test examples/` | ✅ **5/5** (deferred, motor, submachine, traffic-light, vending-machine) |
| `fsm test tests/conformance/` | ✅ **24/24** |
| `fsm test examples/submachine/` | ✅ 1/1 (substantive 10-record trace) |
| defer (W1) end-to-end | ✅ trace passes + generated `Printer.c` compiles `-Werror` clean |
| sim≡codegen submachine (both strategies) | ✅ `submachine_codegen_runs` 2/2 — same observable progression as the hand-verified `.trace` |
| degenerate / zero-transition (TD1) | ✅ `zero_transition_werror` + `degenerate_machines_werror` green |

### Regression findings

- **PD-2 symbol-presence creep:** No regression introduced by W2a–d. The W2d acceptance test (`submachine_codegen_runs.rs`) uses process-exit-code behavioural assertions, NOT `.contains("symbol")`. `submachine_runtime.rs:170,174` uses `Vec<StepKind>.contains(&StepKind::…)` — a typed semantic assertion, not the forbidden text-substring proxy. The only text-substring `.contains()` in sim tests (`deterministic_trace.rs:170-172`) is a pre-existing ordering test, not epic-introduced.
- **IR-schema gate (W0/PD-3):** Still load-bearing. `default = ["schema-validate"]` in `fsm-analyzer/Cargo.toml:29` and `fsm-ir/Cargo.toml:34`. `cargo test -p fsm-analyzer --test ir_schema_gate` 4/4 including `gate_rejects_schema_violating_ir_with_useful_error` (a no-op gate would fail this). Submachine IR validates clean (`every_shipped_example_class_lowers_to_schema_valid_ir` passes).
- **W3 guarantees intact:** Exactly one `MachineIndex` (`crates/fsm-simulator/src/runtime/machine_index.rs`). `grep fsm_analyzer crates/fsm-simulator/src` → zero hits; `fsm-analyzer` correctly demoted to `[dev-dependencies]` in `fsm-simulator/Cargo.toml` (used only by `examples/capture_trace.rs`). `LoweringCtx`/`LoweringContext` → zero hits (not re-introduced).
- **sim≡codegen:** Genuinely matched for defer + submachine (both verified via real gcc-run, not just structural).
- **`unwrap()`/`panic!`/`todo!` in epic user-input paths:** None unsafe. Parser `p.expect(...)` are recoverable diagnostics (emit E0010 + resync), not panics. `checks/submachine.rs:203 adjacency.get_key_value(r).unwrap()` is guarded by an immediately-preceding `if adjacency.contains_key(r)` on the same key, same single-threaded scope — infallible-by-construction (P3 style nit only: a `.expect("contains_key checked")` would document intent).
- **`forbid(unsafe_code)` 9/9:** Intact. 8 crates carry it in `src/lib.rs`; `fsm-cli` (no `lib.rs`) carries `#![forbid(unsafe_code)]` at `src/main.rs:8`.
- **Doc tracking:** SUB-FU-1 (backlog L156) accurately marked "Not a defect" (mirrors plain-machine event-collection: events come from explicit `events {}` blocks). SUB-FU-2 (backlog L157) tracked but **mischaracterized** (see P1-2). `TD-DEF-1`/`TD-DEF-2` (TEST_DEBT.md L130/147) are unrelated to the epic and still accurately open. Note: SUB-FU-1/2 live only in CHANGELOG/ROADMAP/backlog — there is **no** SUB-FU entry in `docs/00-Decisions-And-Reconciliation.md §11` or `TEST_DEBT.md` (P3: tracking is informal/scattered).

---

## Part 3 — Findings (P0..P3, each concrete)

### P1-1 — `cargo test --workspace` fails on `main` (stale W2d-worktree artifact; §11.1 post-merge-quad gap)
- **Where:** `crates/fsm-formatter/tests/idempotency.rs:23` (`fs::read_dir(&dir).unwrap_or_else(...)`), `dir = env!("CARGO_MANIFEST_DIR")/tests/fixtures`.
- **Symptom:** `cargo test --workspace` → `every_fixture_is_idempotent` panics: `read_dir "/root/dev/embeded-fsm-sdk-wt-w2d/crates/fsm-formatter/tests/fixtures": No such file or directory (os error 2)`. Deterministic (3/3 runs).
- **Root cause:** The `idempotency` test binary cached in the shared `CARGO_TARGET_DIR` was compiled inside the **W2d worktree** (`/root/dev/embeded-fsm-sdk-wt-w2d`); `env!("CARGO_MANIFEST_DIR")` is baked at compile time as that path. The worktree was removed post-merge (correct, per §11), but the stale test binary was never invalidated, so `--workspace` reuses it. Recompiling the test (isolated `-p fsm-formatter` run, or `touch`) makes it pass — confirming **not a code defect**.
- **Why it matters:** §11.1 mandates an *independent* post-merge `cargo test --workspace` on `main` after every merge. Either it was not run after the W2d merge, or it was run from a warm target that masked the staleness. This is exactly the "self-reported green is not proof" scenario §11.1 exists to prevent — and the same kind of stale-artifact/illusion the project has been bitten by ([[uncommitted-drift-illusion]] sibling pattern). The submachine *code* is fine; the *process* gate leaked.
- **Severity P1** (not P0): not a behavioural defect, trivially resolved by a clean rebuild of that one test crate; but a green `cargo test --workspace` from a cold/invalidated target is a precondition for any checkpoint tag, and a CI runner with a cold cache would surface this as a hard failure.
- **Fix direction (not applied — read-only):** Cargo cannot detect a moved-out-from-under-it `env!` path; mitigations: (a) orchestrator runs the §11.1 post-merge quad with `cargo test --workspace` after `cargo clean -p fsm-formatter` (targeted, not full clean) when a worktree that built test binaries is removed; or (b) make the fixtures-dir resolution relative to the test binary / `CARGO_WORKSPACE_DIR` rather than the compile-time `CARGO_MANIFEST_DIR`; or (c) the merge worktree-removal step invalidates affected cached test binaries.

### P1-2 — SUB-FU-2 emits non-compilable C; deferral is mischaracterized (audit-integrity / prose-vs-code class)
- **Where:** codegen-c ref-state collection (`collect_sub_refs` walks the parent **root region only**, per backlog L157). Reproduced output: composite case `Outer.c:79 Outer_exit_SUB(m)`, `Outer.c:164 Outer_entry_SUB(m)`; parallel case `P.c:75 P_exit_S1`, `P.c:171 P_entry_S1`. In both, `*_impl.h` declares **zero** prototypes for the nested ref-state's entry/exit (`grep -c SUB Outer_impl.h` → 0; only the root-region states are declared).
- **Symptom:** `fsm generate` succeeds (no crash), but `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` → `error: implicit declaration of function 'Outer_exit_SUB'`. The generated C **does not build under the project's own §5.4-mandated strict gate**, even with a correct user-extern contract.
- **Documented claim (3 places, identical wording):** CHANGELOG.md:14/17, ROADMAP.md:56, backlog L157 all say the nested ref is "**emitted as an inert leaf**" / "**still an inert leaf**", and CHANGELOG concludes "**v1.1 reaches full-UML coverage**". An inert leaf compiles like any leaf; this does not. The limitation is real and *explicitly flagged* (good — not silent), but it is **described as more graceful than it is** — the same class as the P0-1 / original-submachine prose-vs-code drift the project is sensitized to, and the precise risk [[verify-status-claims-vs-code]] warns about.
- **Mitigating facts:** generator does not crash; **simulator side IS genuinely inert-safe** (no panic, deterministic trace — verified composite + a no-panic run); the *supported* top-level path is fully correct; the limitation is tracked, not hidden. So this does **not** block W4–W7 (they build on the verified top-level path).
- **Severity P1:** because (a) it produces broken output under the canonical gate while docs imply graceful degradation, (b) the inaccuracy is in audit-integrity-sensitive scope wording the project has explicitly committed to keeping precise, and (c) "full-UML coverage" is, as stated, false. Correct before W4–W7 land more weight on the submachine subsystem.
- **Fix direction (not applied):** either (a) make nested-ref codegen *actually* inert — emit a real leaf (no `entry_SUB`/`exit_SUB` calls, or emit + declare them) so output compiles, matching the documented behaviour; or (b) keep current emission but correct all three docs to state precisely "nested-in-composite/parallel submachine refs currently generate non-compilable C — do not nest submachine refs; SUB-FU-2", and drop/qualify "full-UML coverage". (a) is the honest fix; (b) is the minimum integrity fix.

### P2-1 — `runtime/submachine.rs` module doc references functions that do not exist in the module (broken intra-doc links)
- **Where:** `crates/fsm-simulator/src/runtime/submachine.rs:15-27` — the module doc enumerates `[instantiate_on_entry]`, `[delegate_event]`, `[teardown_on_exit]` as if they live here. Only `build_sub_runtime`, `entry_target`, `sub_reached_final` are actually defined in this file; the instantiation/delegation/teardown logic lives inline in `interpreter.rs:700-934`.
- **Impact:** Misleading to a maintainer reading the module in isolation (suggests a lifecycle API surface that isn't here); `cargo doc` rustdoc intra-doc links `[instantiate_on_entry]` etc. resolve to nothing. No runtime/behavioural effect.
- **Severity P2** (doc-accuracy on a load-bearing new module; the kind of "comment decayed from code" the comment-doctrine targets).

### P3-1 — SUB-FU-1/SUB-FU-2 tracking is informal and scattered
- SUB-FU-1/2 exist only in CHANGELOG + ROADMAP + backlog memory. There is no entry in `docs/00-Decisions-And-Reconciliation.md §11` (implementation decisions table) or `docs/processes/TEST_DEBT.md`. The conventions point future maintainers at Doc 00 §11 for decisions; a deferred-codegen-feature with a known broken output deserves a tracked TEST_DEBT/Doc-00 line, not only a CHANGELOG sentence.

### P3-2 — `checks/submachine.rs:203` `.unwrap()` on `get_key_value` after `contains_key`
- Infallible by construction (guarded by the immediately-preceding `if adjacency.contains_key(...)` on the same key, single-threaded). Not a panic risk. Style only: `.expect("key present — contains_key checked above")` would document the invariant per the unwrap-discipline spirit.

---

## What I was skeptical about — and how it actually checked out

| Suspicion (given the epic's false-claim origin) | Finding |
|---|---|
| "End-to-end" is prose; lowering still hard-codes empty submachines (the original sin) | **False alarm — genuinely fixed.** `lower/machine.rs:188` really iterates+recurses; emitted IR has a fully-populated `submachines` array + a `submachine_ref` state. The only `vec![]` literal is a `#[cfg(test)]` fixture. |
| Codegen "emits a nested struct" but it's an inert symbol that does nothing (the P0-1 symbol-presence trap) | **False alarm — genuinely works.** I wrote my *own* driver against the real CLI-generated API (not the test fixture) and executed it: the sub advances Idle→Handshake→Established→Done and the parent done-fires to Online with no external event. |
| The W2d acceptance test is a `.contains()` symbol-presence test dressed up | **False alarm — exemplary.** `submachine_codegen_runs.rs` compiles both strategies with strict gcc, executes, and asserts via distinct exit codes, each tied to the pre-W2d inert-leaf bug. Model §5.4 test. |
| The `.trace` has `expected: []` and passes trivially | **False alarm.** 10 fully-populated `expected` records including the `__completion__` parent-fire; `fsm test` genuinely verifies it. |
| The "single caveat" SUB-FU-2 is a polite way of saying something is broken | **CONFIRMED — this is the real find.** It does not degrade to an inert leaf; it emits C that won't compile under the project's own mandated gate. The deferral docs say "inert leaf" and "full-UML coverage" — both inaccurate. Exactly the prose-vs-code drift class this audit exists to catch, found at the boundary as intended (milder than P0-1: flagged, not silent, supported path intact). |
| The verify quad is quietly red and the completion reports glossed it | **CONFIRMED (process, not code).** `cargo test --workspace` fails on `main` from a stale W2d-worktree-compiled test binary — the §11.1 independent post-merge quad either didn't run or ran warm. The submachine code is clean; the gate leaked. |

---

## Recommendation to the orchestrator

1. **Before any W4 dispatch:** fix P1-1 (targeted `cargo clean -p fsm-formatter` + re-run §11.1 quad; adopt the worktree-removal cache-invalidation or relative-fixtures-path mitigation so this class can't recur). A green `cargo test --workspace` from a cold target is the precondition for the checkpoint tag.
2. **Correct P1-2's docs immediately (audit integrity, [[verify-status-claims-vs-code]] mandate):** the "inert leaf" / "full-UML coverage" wording in CHANGELOG, ROADMAP, and backlog must be made precise — nested-in-composite/parallel submachine refs currently produce non-compilable C. Decide whether to *fix the codegen to be genuinely inert* (preferred) or *qualify the claim* (minimum). Add a tracked SUB-FU-2 line to Doc 00 §11 / TEST_DEBT.md (closes P3-1).
3. **Then** the supported top-level submachine subsystem is sound to build W4–W7 on; tag `checkpoint/2026-05-15-submachine` only after step 1 yields a clean quad.
