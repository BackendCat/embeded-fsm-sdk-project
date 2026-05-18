# GATE — STAGE-SOLID re-gate (FSM-Studio #118) — 2026-05-18

> **Frozen verdict doc. Never overwritten** (the `AUDIT_*`/`GATE_*` convention).
> The owner's explicit stage-gate: the binary decision that unblocks (or
> blocks) the Factory-Reliability epic's W2. An **independent comprehensive
> re-derivation from source/behaviour** — NOT a trust of prior wave reports.
> Brutal honesty over papering-over; a NOT-SOLID verdict is the correct,
> valued outcome if the foundation is not solid.

- **Worktree:** `/root/dev/embeded-fsm-sdk-wt-stage-solid-regate`
- **Branch:** `phase6.9/stage-solid-regate` (off main `1fb1d36`, verified clean at start)
- **HEAD audited:** `1fb1d36` (`fix(codegen-c): FW110-FU-E …`)
- **Toolchain (asserted from inside the worktree):** `rustup show active-toolchain` → `1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')` — the **repo-pinned 1.75**, not the benign box default (1.95). Toolchain-probe-trap correctly disposed.
- **NO `git stash` used — confirmed.** Base↔HEAD comparison was done exclusively via `git diff <ref>..HEAD`, `git show`, and a throwaway side worktree `git worktree add /tmp/regate-base 2ff8ac3`. The global stash stack was never touched (push/pop/apply/drop). Hard rule honored.
- **No-fork forbidden set — untouched (attested):** `git diff --name-only` on the working tree shows **zero** changes to `crates/fsm-simulator/src/**`, `crates/fsm-cli/src/cmd/test.rs`, `crates/fsm-cli/src/cmd/baseline.rs`; and the diff of `codegen_equivalence_smoke.rs` contains **no** projection/comparator-fn lines (`project_record`/`byte_diff`/`simulator_projection`/`generated_c_projection`/`sorted_csv`/`run_differential`/`kind_str`/`SEP`) — **only the `//!` module-doc header** was reconciled.

---

# OVERALL VERDICT — **STAGE-SOLID**

**The Factory-Reliability epic W2 may proceed.**

All gate conditions hold, independently re-derived:

- **12/12 corpus byte-equal-or-justified**, `KNOWN_DIVERGENT` literally empty, the catalogue-integrity assertion holds (9 + 3 + 0 = 12 = `CORPUS.len()`, no double-classification), every never-game guard green + non-vacuous.
- **Deterministic ≥5×** — in fact **11 consecutive GREEN runs** (6 pre-doc-edit + 5 post-doc-edit), zero flakes. The prior latent-non-determinism flag is conclusively not reproducible.
- **Both keystones re-confirmed from source at `1fb1d36`:** the differential drives the shipped UNFORKED `fsm_simulator::execute_trace` seam (`git diff 71cc1bd..1fb1d36 -- crates/fsm-simulator/src crates/fsm-cli/src/cmd/test.rs crates/fsm-cli/src/cmd/baseline.rs` = **EMPTY**); `crates/fsm-verify` byte-untouched by the FW110 arc; the v1.4 clock-origin-merge soundness lemma (Doc 08 §13.5) holds — re-derived from the lexer keyword set (53 keywords, **no** clock-read token), the parser primary production (literals/names/field-refs/parens only), and the analyzer/IR guard path (zero clock term). The FU-D/FU-E timer waves did **not** introduce a clock-read-in-a-guard.
- **NO unintended production regression** — every production-effective delta vs the v1.5.0 baseline is exactly one of: (i) inert `#ifdef FSM_TRACE` blocks (zero production effect by construction), (ii) cosmetic whitespace (`-Werror`-clean both sides), or (iii) the **intentional, behaviour-correcting** FW-arc fixes (FU-D timer-budget + FW1-FU-2 history/exit-set), proven behaviour-*preserving* where the old code was already correct (motor) and behaviour-*correcting* where v1.5.0 was buggy (traffic-light shallow_history + timer over-fire — HEAD now matches the shipped-simulator oracle = ground-truth UML semantics).
- **Quad green at HEAD** — `build --workspace` / `clippy --workspace --all-targets -D warnings` / `fmt --all --check` clean; **full test suite 878 passed / 0 failed across every test binary** (not a tail; every `test result:` line inspected).
- **#109 and #110 merged/closed** — all FW109/FW110 fix-wave commits are ancestors of `1fb1d36`.
- **No correctness-critical test-debt or doc-drift left unaddressed** — the Doc-32 corpus-size hardcoding reconciled here (engineering decision, disclosed §4); the `codegen_equivalence_smoke.rs` stale module-doc header reconciled here (correctness-prose drift, disclosed §4); the #110 P1 `.contains` conformance oracle confirmed still tracked for the W3/G7 wave (not regressed).

Sub-excellent-but-non-blocking items are listed in §6.

---

## §1. Corpus GREEN-or-justified — deterministic, clean build

**Clean build:** `cargo clean -p fsm-codegen-c -p fsm-simulator` then `cargo test -p fsm-simulator --test codegen_equivalence_smoke --no-run` — recompiled `fsm-codegen-c` + `fsm-simulator` from clean, `Finished` clean.

**≥5 consecutive runs** of `cargo test -p fsm-simulator --test codegen_equivalence_smoke` (pre-doc-edit):

| run | result |
|----|----|
| 1 | ok. 7 passed; 0 failed; 2 ignored — 2.41s |
| 2 | ok. 7 passed; 0 failed; 2 ignored — 2.49s |
| 3 | ok. 7 passed; 0 failed; 2 ignored — 2.44s |
| 4 | ok. 7 passed; 0 failed; 2 ignored — 2.52s |
| 5 | ok. 7 passed; 0 failed; 2 ignored — 2.41s |
| 6 | ok. 7 passed; 0 failed; 2 ignored — 2.41s |

Plus **5 more** after the doc-only edits (proving the comment change does not perturb): all `ok. 7 passed; 0 failed; 2 ignored` (2.40–2.44s). **Total: 11 consecutive GREEN, zero flakes.** Determinism requirement satisfied; the prior latent-non-determinism flag is **not reproducible**.

**Per-fixture verbose output** (single `--nocapture` run; gcc 12.2.0 present ⇒ **HARD** differential, no skip):

- **9 BYTE_EQUAL** all literally `BYTE-EQUAL ✓` (shipped `execute_trace` oracle == FSM_TRACE-compiled-and-RUN generated C): motor, deferred, traffic-light, stress-parallel-cross-exit, stress-deep-history, stress-self-transitions, stress-self-transitions-actions, stress-choice-guard-payload, stress-every-timer.
- **3 BEHAVIOURALLY_EQUIVALENT_JUSTIFIED** all RED with precise first-divergence (vending-machine #5, submachine #0, stress-completion-chain #3) **AND** each independently `PROVEN behaviourally equivalent` via per-prefix quiescent-config (vending 6 cmds, submachine 3 cmds, stress-completion-chain 3 cmds, all + init).
- **`KNOWN_DIVERGENT` is literally `&[]`** (source line 1383–1400: only comments, zero string entries — asserted dynamically by the integrity block at lines 1453–1480).

**Catalogue-integrity assertion** (test body lines 1453–1480, read in full): asserts no double-classification (`all.dedup()` length-preserving), `BYTE_EQUAL.len() + BEHAVIOURALLY_EQUIVALENT_JUSTIFIED.len() + KNOWN_DIVERGENT.len() == CORPUS.len()` (9 + 3 + 0 = 12), and the buckets' union == the corpus. **Holds.**

**Never-game guards — green + non-vacuous (proof bodies read in full):**

- `differential_goes_red_on_a_deliberately_corrupted_oracle` — builds the real motor differential, asserts pre-corruption byte-equal, mutates `cfgA=s-Motor-Running`→`s-BOGUS` at step #1, asserts RED with the report pinning `step #1` and surfacing `s-BOGUS`. Output: `RED-on-divergence proof OK — first divergence correctly pinned to step #1`. **Non-vacuous.**
- `behavioural_equivalence_proof_red_on_corrupted_oracle` — vending-machine; asserts every prefix matches pre-corruption, then corrupts the final quiescent config and asserts it ≠ the generated C's. Output: `RED-on-corruption proof OK`. **Non-vacuous.**
- `oracle_is_the_shipped_execute_trace_seam_not_a_fork` — calls `execute_trace(&ir,&t)` directly (the sole oracle call), asserts records produced + projection starts `STEP`. **Executable no-fork attestation.**
- `field_separator_is_a_pinned_cross_engine_contract` — asserts `SEP==0x1f` AND `fsm_codegen_c::emit::trace_hook::FIELD_SEP==0x1f` AND they are the same byte (the cross-engine projection-shape contract).
- `fw110_fu_e_stress_every_timer_beat_invocation_count_is_four` — drives the shipped `Interpreter` with a counting `beat` extern (oracle) AND the production-codegen C (FSM_TRACE-OFF) with a counting `beat()`; asserts both == 4 (clk=1000,2000,3000,6000) and equal to each other. Output: `beat()==4 in BOTH … the missed-heartbeat defect is closed (was 3 pre-fix)`. **A real observable-side-effect acceptance, not symbol-presence.**

**The 3 JUSTIFIED proofs are genuine + non-vacuous** (proof bodies + catalogue notes read): `behavioural_equivalence_proof_for_justified_fixtures` drives the SHIPPED `Interpreter` per-prefix (`simulator_quiescent_config`, the exact seam `cmd/test.rs` uses) and the generated C per truncated prefix (`generated_c_quiescent_config`, reading the C's OWN emitted `cfgA`), asserting byte-identical quiescent config at every prefix `0..=n`, AND pre-asserts the byte-diff still REDs (a converged fixture must move to BYTE_EQUAL — the catalogue-stays-honest discipline). The cascade/completion fixtures genuinely move states across prefixes (the proof is non-vacuous, per the catalogue notes at lines 1242–1265).

**§1 = PASS.**

---

## §2. Both keystones — independently re-derived from source at `1fb1d36`

### Differential keystone

- **Differential-birth commit identified from git log:** `71cc1bd` (`Phase-6.0-W1: the R7 generic host-trace differential (keystone prerequisite)`) — the first commit touching `crates/fsm-simulator/tests/codegen_equivalence_smoke.rs`.
- **No-fork derivation (the deriving command + result):**

  ```
  $ git diff --stat 71cc1bd..1fb1d36 -- crates/fsm-simulator/src \
        crates/fsm-cli/src/cmd/test.rs crates/fsm-cli/src/cmd/baseline.rs
  (EMPTY)            # git diff --quiet … ⇒ exit 0
  ```

  The shipped oracle seam (`fsm_simulator::execute_trace` + the CLI consumers) is **byte-untouched across the ENTIRE arc** from differential-birth to HEAD.
- **Projection is formatting-only & symmetric** (`project_record`/`simulator_projection`/`generated_c_projection`/`byte_diff`/`sorted_csv` read at HEAD): `project_record` is a pure format of an already-computed `StepRecord` (no transition selection / guard eval / LCA); `simulator_projection` calls the shipped `execute_trace` directly and projects every record; the C side is an emit-hook-only (`#starts_with("STEP")` filter); `byte_diff` is pure `Vec<String>` equality with a precise first-divergence report and **zero** FSM logic. `sorted_csv` canonically sorts state-set members **identically on both sides** (the determinism discipline — behaviour is the *set*, not a list order the two engines never contracted to share). The C-side `STEP` line is emitted by the codegen `#ifdef FSM_TRACE` trace-hook whose `FIELD_SEP` is pinned == the harness `SEP` (= 0x1f) by `field_separator_is_a_pinned_cross_engine_contract`.

### Verification-core keystone

- **`crates/fsm-verify` byte-untouched by the FW110 arc:** `git diff --quiet 2ff8ac3..1fb1d36 -- crates/fsm-verify` ⇒ exit 0 (EMPTY); also `git diff --quiet 71cc1bd..1fb1d36 -- crates/fsm-verify` ⇒ exit 0. No forked transition logic could have entered.
- **v1.4 clock-origin-merge soundness (Doc 08 §13.5 "Absolute-Virtual-Clock Non-Observability") still holds — re-derived from the grammar + analyzer at HEAD, NOT echoed from the doc:**
  - **Lexer keyword set (`crates/fsm-lexer/src/token.rs` `keyword_kind`, read in full): all 53 keywords enumerated — there is NO `now`/`at`/`time`/`clock`/`elapsed`/`uptime`/`timestamp`/`deadline` token.** The only time-related keywords are `after`/`every`/`ms`/`schedule`/`cancel` — timer-*arming/duration* (relative), not absolute-clock-*reading*. (Matches §13.5 point 1 at source.)
  - **Parser primary production (`crates/fsm-parser/src/expr.rs` `parse_prefix`, read in full): the only expression atoms are unary (`!`/`-`/`~`), parenthesised expr, literals (int/float/string/true/false), `else` (guard catch-all), and `Ident` (name-ref / field-ref / call). No clock/time/now/elapsed primary.** (Matches §13.5 point 3 at source.)
  - **Analyzer/IR guard path:** `grep -rniE 'virtual_clock|clock_now|absolute.*clock|now_ms'` over `crates/fsm-analyzer/src` + `crates/fsm-ir/src` (comments/tests excluded) ⇒ **∅**. The FU-A2 analyzer change (`lower/state.rs`) lowers choice/junction branch guards via the **existing** `lower_guard_clause`/`lower_guard_expr` (`GuardExpr::Else` fallback) — it introduces no new guard-expression kind and **no clock term**.
  - **FU-D/FU-E** are confined to `crates/fsm-codegen-c/src/emit/{timer.rs,dispatch_switch.rs}` (codegen runtime timer-budget/dispatch accounting): the diff explicitly works in **relative** terms (`remaining = expiry - now`, ordered armed-set budget consumption; "no absolute per-timer clock"). They are budget/dispatch accounting, **not** a DSL-readable clock value, and do **not** touch `fsm-verify`'s digest. **No clock-read-in-a-guard is reachable ⇒ no P0 soundness block.**

**§2 = PASS (both keystones re-confirmed).**

---

## §3. No UNINTENDED production regression — cumulative across all 6 FW110 codegen waves

**Baseline = v1.5.0 (`2ff8ac3`) — confirmed the correct baseline:** `git tag --points-at 2ff8ac3` ⇒ `v1.5.0` (+ `checkpoint/2026-05-17-v1_5`). `git log --oneline --reverse 2ff8ac3..1fb1d36 -- crates/fsm-codegen-c/src | head -1` ⇒ `71cc1bd Phase-6.0-W1` — i.e. the **first** codegen-c change after v1.5.0 IS the W1 differential-birth; there is **no pre-FW110 codegen change** between v1.5.0 and the factory epic, so the v1.5-was-LSP-only assumption holds and `2ff8ac3` is the true pre-first-emit-change commit. (Codegen-c delta `2ff8ac3..1fb1d36` = +3368/−104 across 11 emit files — exactly the FW arc incl. `trace_hook.rs` born at `71cc1bd`.)

**Method (stale-artifact false-positive guarded):** built the base `fsm` CLI in a **separate** `CARGO_TARGET_DIR=/tmp/regate-base-target` (the worktree uses the git-tracked **shared** `/root/dev/embeded-fsm-sdk-target`; the base build must not contaminate it) and the HEAD `fsm` in `/tmp/regate-head-target`. Generated production C (`fsm generate`, default = no `-DFSM_TRACE`) for **every `.fsm` file present at BOTH base and HEAD** (13 files), with **per-fixture file-count assertions** (never an empty-dir "all identical").

**Corpus parity finding:** the v1.5.0 `examples/` set is a **subset** of HEAD — the 7 `stress-*` fixtures were *born during the epic* (the FW110 corpus broadening 5→12), so they have **nothing to regress**. No common-fixture `.fsm` **source** changed (`git diff --stat 2ff8ac3..1fb1d36 -- examples/{deferred,import-header,integration,motor,per-machine-strategy,submachine,traffic-light,vending-machine,verify}` = EMPTY), so any production-C delta is **purely codegen**.

**`diff -rq` base↔HEAD then FSM_TRACE-strip classification** (a custom nested-`#if`-aware stripper removed every `#ifdef FSM_TRACE … #endif` block — production sees `#ifndef FSM_TRACE`, so the block is inert — then a token-level whitespace-normalized + advance_clock-excision compare classified the surviving delta):

| fixture/file | category | disposition |
|---|---|---|
| deferred/Printer.c, per-machine-strategy/{Counter,Latch}.c, submachine/{Connection,Device}.c, vending-machine/VendingMachine.c, verify/deadlocks/SafetyLatch.c | **A — whitespace-only** | `+\n\n` after the include block (trace-hook include-emission artifact). **Zero compile/semantic effect** — `-Werror`-clean both sides (verified on deferred/Printer.c). |
| motor/Motor.c (×1 canonical + ×4 integration copies), verify/clean/ThermalController.c | **B — `advance_clock` timer-budget rewrite only** | FU-D timer-budget. Behaviour analysed below. |
| traffic-light/TrafficLight.c | **C — advance_clock rewrite + FW1-FU-2 shallow_history/exit-set** | Multiple intentional FW-arc fixes. Behaviour analysed below. |

`import-header/motor_uses_driver.fsm` fails generation **identically** on base and HEAD (it requires `--import-header`; not a corpus member as-invoked) — **not a regression**.

**Production-runtime differential (the decisive evidence)** — compiled the **production** (no `-DFSM_TRACE`) C at base vs HEAD for the category-B/C fixtures, drove the exact `.trace` stimulus, compared observable per-step `_active[0]`:

- **motor:** base and HEAD produce **identical** state sequences (`Running, Idle, Running, Faulted, Idle, Running, Faulted, Idle`). The FU-D timer-budget rewrite is **behaviour-preserving for the single-timer case**. **No regression.**
- **traffic-light:** base and HEAD **diverge** — and this is the **BUG-FIX, not a regression**:
  - base v1.5.0 (buggy): `Red → Green → Green → Red → Green → Manual → ?` — the `?` at RESUME is the **broken shallow_history** (lands in invalid `STATE_HISTORY` — the FW1-FU-2 defect) and the timer chain is wrong (over-fire / missed re-arm — the W1-FU defect).
  - HEAD (fixed): `Red → GreenAcc → Green → Yellow → Red → Manual → Red` — the **correct UML semantics**, *exactly* what `traffic-light.trace`'s `description` + `expected` block specify and what the differential corpus confirms (traffic-light is BYTE_EQUAL at HEAD ⇒ HEAD-C == the shipped-simulator oracle = ground truth).

**Ground-truth corroboration:** the frozen `.trace` files' own `description` records the `expected` blocks were *"refreshed post-P0-4 (timer fix)"* — i.e. **v1.5.0's production C was already KNOWN-BUGGY for timers/history**; the FW arc *corrected* it. verify/clean's ThermalController is the same FU-D timer-budget change (a fsm-verify deadlock example, no `.trace`; the codegen change is the identical sound fix proven above).

**Conclusion:** every production-effective delta is permitted by the gate brief — inert FSM_TRACE blocks (zero effect by construction), cosmetic whitespace (zero compile effect), or the **intentional behaviour-correcting** FW-arc fixes (FU-D timer-budget + FW1-FU-2 history/exit-set) on FSMs that *legitimately exercise those exact constructs* (timer / `every` / shallow_history / composite). **NO unexplained production delta. NO cumulative regression. §3 = PASS.**

---

## §4. Test-debt + doc-drift re-sweep

### Doc-32 corpus-size reconciliation (RECONCILED HERE — disclosed engineering decision)

**Finding:** `docs/32-Factory-Reliability-CI-Wave-Plan.md` (authored at the epic kickoff, commit `83b912a`, when exactly 5 example FSMs had frozen traces — GT-7) hardcodes "the **5** frozen-trace examples / **5** MVP corpus FSMs" in ≥6 places, including the **binding §2 KEYSTONE tag-gate row (line 152)** and the **binding §5 KEYSTONE gate row (line 188)**. The FW110 arc deliberately broadened the differential corpus **5 → 12**. A literal "the 5" gate now *under-tests* (re-runs only 5 of 12 at the W6 gate); a literal "the corpus" gate would *wrongly RED* on the JUSTIFIED record-model fixtures. The #110-era reliability audit (`AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md`) flagged this and **recommended ESCALATE** because gate-semantics looked product-strategic.

**Decision (disclosed):** the stage-solid re-gate brief explicitly rules this a **pure engineering decision the TL holds** (NOT product-strategy), so fix-it-here is in-scope and expected. The prior audit's escalate-recommendation is **superseded** now that the owner/TL has ruled. **Reconciled corpus-size-agnostically** by:
1. Adding **one authoritative normative definition** — *THE CORPUS-SIZE-AGNOSTIC CATALOGUE-INTEGRITY CONTRACT* — into the §2 KEYSTONE block, with an explicit clause that it **supersedes every "the 5 frozen-trace examples" phrasing document-wide** (so the W1/W2/W6 §5.4 rows are corrected by reference, not via 6 risky scattered in-place rewrites).
2. Updating the **two binding tag-gate rows** (§2 line 152, §5 line 188) to reference the contract.

The contract: the host (W1) + emulated-target (W2) differential, and the W6 re-derivation, **PASS iff** — for the corpus *as it stands at the gate commit* (the `codegen_equivalence_smoke.rs` arrays are the single source of truth, not a frozen integer): (1) every `BYTE_EQUAL` member byte-equal vs `execute_trace`; (2) every `BEHAVIOURALLY_EQUIVALENT_JUSTIFIED` member's non-vacuous quiescent-equivalence proof green; (3) `KNOWN_DIVERGENT` empty-or-each-tracked; (4) the catalogue partition holds; (5) the never-game guards green + non-vacuous. The W2 on-target lane gates on (1)+(4)+(5) over the same catalogue (the JUSTIFIED record-model residual is a host-proven property, not re-litigated per substrate). This is the **same keystone invariant** expressed independent of corpus cardinality — the gate stays correct as the corpus keeps broadening. *(The GT-7 ledger entry itself is left as-is: it is a historical verification record correctly documenting what was true at kickoff.)*

### `codegen_equivalence_smoke.rs` module-doc header (RECONCILED HERE — correctness-prose drift)

**Finding:** the file's `//!` module-doc header (written at W1-birth for 5 fixtures, never updated as the corpus broadened across W1-FU→FW110-FU-E) still claimed *"For each of the **5 frozen-trace example FSMs** (GT-7 — motor, submachine, traffic-light, vending-machine, deferred)"* — directly contradicting the file's own `CORPUS` (12) + catalogue arrays + in-body notes. **Second, distinct prose drift:** the documented canonical-line format showed `STEP kind=<k> clk=<u32> evt=…` but `project_record` (read in full) emits **no `clk=` field** (the absolute clock is non-observable per Doc 08 §13.5 — the projection deliberately excludes it).

**Decision (disclosed):** both are trivial, in-scope ("fix-now-if-trivial, disclose") correctness-prose reconciliations of the **module-doc header only** — the projection/comparator FNS, `fsm-simulator/src`, and the cmd seam (the no-fork forbidden set) were **NOT** touched (attested by diff). Reconciled the header to be corpus-size-agnostic (the catalogue arrays are SoT) and corrected the format line to match `project_record` exactly (with a Doc 08 §13.5 cross-reference for *why* no `clk=`). Re-ran the differential 5× post-edit — still deterministic GREEN (a comment change must not perturb; it did not).

### Other doc surfaces — enumerated, triaged

| surface | FW110-introduced drift? | disposition |
|---|---|---|
| **Doc-08 §13.1/§13.3/§13.4/§13.5** | **None** — `git diff --stat 2ff8ac3..1fb1d36 -- docs/08-*.md` = EMPTY. The §13 timer semantics were already normative; FU-D/FU-E made the codegen *match* the pre-existing spec (the entire point of the fix). §13.5 neighbourhood consistent (and independently re-derived in §2). | No action. |
| **Doc-00 §11 ledger** | **None** — ends at §11.79 (v1.5 closeout); no Phase-6.0 rows. **Correct per Doc-32's own §13-refinement** (Phase-6.0 rows assigned at the W6 closeout, NOT pre-allocated). | No action — W6-closeout obligation, not stage-gate drift. |
| **Doc-04** (choice/junction) | **None** — `choice`/`junction` keywords + grammar unchanged; FU-A2 is a lowering-impl detail (reuses `lower_guard_clause`), not a spec change. | No action. |
| **CHANGELOG.md** | **None** — the FW110 arc is unreleased (no tag since v1.5.0). CHANGELOG is release-cut by discipline; absence is **correct**. | Recommend: the next-release CHANGELOG batch is a **W6-closeout obligation** (Doc-32 §5), not a stage-gate item. |
| **README.md** | **None FW110-introduced.** README roadmap is stale ("v1.4 in progress" though v1.5.0 is tagged) but this is a **pre-existing** doc-honesty condition unrelated to the 6 waves (already in the v1.6 D1 doc-honesty scope per Doc-32's reference to `ASSESSMENT_PRODUCT_AND_DOCS`). | Recommend: **batch for v1.6 doc-honesty** (out of this gate's scope — pre-existing, not FW110). |
| **#110 audit-doc errata consistency** (`AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md`) | Its Doc-32 escalate-recommendation is now superseded by this gate's reconciliation. The frozen audit is **not overwritten** (the `AUDIT_*` convention); this verdict doc records the supersession. | No edit to the frozen doc — supersession recorded here (§4). |

### Test-debt re-sweep

- **No new symbol-presence/hollow assertion** introduced by the 6 waves in correctness suites. The FW110 test additions are the `codegen_equivalence_smoke.rs` differential (the *opposite* of hollow — byte-equality + executable non-vacuous quiescent-equivalence proof + non-vacuous never-game guards, all read in full) and the `fw110_fu_e_…_beat_invocation_count` instrumented behavioural acceptance.
- **#110 P1 (conformance codegen-c `.contains` substring oracle) — still tracked, NOT regressed.** `tests/conformance/codegen-c/*/expected/*.c.contains` still present; `COVERAGE_MAP.md` + the prior reliability audit (P1, line 306) + Doc-32 W5/G7 all chart the `.contains`→compile+behavioural conversion as a **W3/G7 wave deliverable**. The FW110 arc did not touch the conformance suite. Correctly tracked future work (G7), not regressed.

**§4 = PASS** (two engineering reconciliations applied + disclosed; everything else triaged — nothing correctness-critical left unaddressed).

---

## §5. Quad + closure

- `cargo build --workspace` — `Finished` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — `Finished` clean (zero warnings).
- `cargo fmt --all --check` — clean (exit 0, no diff; the doc-comment edits are fmt-clean).
- `cargo test --workspace` — **878 passed / 0 failed across every test binary** (every `test result:` line inspected, not a tail; zero `FAILED`/`error[`/`panicked`/`failures:`). The differential binary is visible at `7 passed; 0 failed; 2 ignored … 2.43s`.
- **#109 closed/merged:** `e10b65e` (`fix(codegen-c)+test(fw109): record-model determination`) is an **ancestor of HEAD `1fb1d36`**.
- **#110 closed/merged:** the entire FW110 fix-wave chain `ee623f8` (corpus broadening), `0ccf8c4` (FU-B), `ac6d56b`/`de10291`/`47afd4a`/`c46ba50` (FU-A/FU-A2), `ff9d953` (FU-C), `ea1829c` (FU-D), `1fb1d36` (FU-E) are **all ancestors of HEAD**. (`a98f6ea`/`9e59857` are unrelated old reflog/`On <branch>` artifacts NOT on the HEAD line — irrelevant; the global stash was never touched.)

**§5 = PASS.**

---

## §6. Sub-excellent (non-blocking) — proactively surfaced (stage-gate methodology)

None are gate-blockers; recorded so they are not silently dropped:

1. **The two cosmetic `+\n\n` blank lines** the trace-hook include-emission adds after the `#include` block of every generated `.c` (category-A above). Zero compile/semantic effect and `-Werror`-clean, but it is *unnecessary* generated-output churn vs v1.5.0. Recommend a 1-line tidy in `trace_hook.rs`'s include emission in a future codegen-cosmetic pass (NOT this gate — it touches codegen-c, out of stage-gate scope; explicitly behaviour-irrelevant).
2. **Doc-32's W1/W2/W6 §5.4/deliverable rows still *literally* read "the 5 frozen-trace examples"** — now *superseded* by the new normative §2 contract clause (the engineering-correct minimal reconciliation: one authoritative definition + the two binding rows, not 6 risky rewrites). A future Doc-32-grooming pass *may* inline the contract phrasing into those rows for local readability, but it is **not required** (the supersession clause is unambiguous and the binding rows — the actual tag-blockers — are reconciled).
3. **README.md roadmap staleness** ("v1.4 in progress" vs v1.5.0 tagged) — a **pre-existing** doc-honesty item (not FW110-introduced); recommend folding into the **v1.6 doc-honesty landing** (already its identified scope).
4. **CHANGELOG has no FW110/Phase-6.x entries** — correct now (unreleased), but a reminder that the **W6 closeout** must author the next-release CHANGELOG batch (Doc-32 §5 obligation).
5. **#110 P1 conformance `.contains` oracle remains hollow** — correctly tracked for the W3/G7 wave; not regressed. Surfaced as a standing known-debt the Factory-Reliability epic itself charters to close (not this gate).

---

## Appendix — pinned commands / evidence

```
# Toolchain (from inside the worktree — the pin, not the box default)
$ cd /root/dev/embeded-fsm-sdk-wt-stage-solid-regate && rustup show active-toolchain
1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')

# No-fork attestation (differential-birth = 71cc1bd, identified from git log)
$ git diff --stat 71cc1bd..1fb1d36 -- crates/fsm-simulator/src \
      crates/fsm-cli/src/cmd/test.rs crates/fsm-cli/src/cmd/baseline.rs
(EMPTY ; git diff --quiet ⇒ exit 0)

# Verification-core no-fork
$ git diff --quiet 2ff8ac3..1fb1d36 -- crates/fsm-verify   ; echo $?   → 0
$ git diff --quiet 71cc1bd..1fb1d36 -- crates/fsm-verify   ; echo $?   → 0

# Production baseline confirmation
$ git tag --points-at 2ff8ac3                              → v1.5.0 (+ checkpoint/2026-05-17-v1_5)
$ git log --oneline --reverse 2ff8ac3..1fb1d36 -- crates/fsm-codegen-c/src | head -1
  71cc1bd Phase-6.0-W1 …   (first codegen-c change after v1.5.0 IS the W1 birth)

# Differential determinism: 11 consecutive GREEN (6 pre-edit + 5 post-doc-edit),
# all `ok. 7 passed; 0 failed; 2 ignored` (~2.4s) — zero flakes.

# Quad @ HEAD: build/clippy(-D warnings)/fmt clean; cargo test --workspace
#   = 878 passed / 0 failed across every test binary.

# #109 / #110 closure: all FW109/FW110 fix-wave commits are ancestors of 1fb1d36.

# NO `git stash` used anywhere — base↔HEAD via git diff / git show /
# `git worktree add /tmp/regate-base 2ff8ac3` (throwaway side checkout) only.
```

**END OF GATE — VERDICT: STAGE-SOLID. The Factory-Reliability epic W2 may proceed.**
