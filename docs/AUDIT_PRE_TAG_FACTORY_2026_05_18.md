# Independent Pre-Tag Four-Lens Audit — Factory-Reliability/CI Epic (Phase-6.0) — 2026-05-18

**Document ID:** FSM-AUDIT-PRE-TAG-FACTORY (frozen evidence doc — the `AUDIT_PRE_TAG_*`
never-overwritten convention; a new audit is a new versioned file).
**Auditor role:** independent senior staff release auditor (adversarial,
re-derive-don't-trust; the v1.4-W4c / v1.5-post-A2 precedent of catching real issues
by independent re-derivation, never echoing a wave's self-report). A `TAG-CLEAR`
that merely restates a wave's claim is worse than a found problem; a real P0 → `BLOCK`
is the correct, valued outcome.
**Subject / HEAD:** `654cf64` (`docs(factory-w6): the binding source-derived keystone
phase-audit — KEYSTONE-INTACT`) — main post-W6a, on branch `phase6.6/factory-w6-closeout`.
**Scope:** the FACTORY-EPIC DELTA over the v1.5.0 baseline `2ff8ac3` → `654cf64`
(`2ff8ac3..654cf64`; the W1→W6 arc + the FW110-FU-{A,B,C,A2,D,E} arc) — the
standing pre-tag quality+security audit of **that delta**, re-derived from
source/behaviour at HEAD. **NOT** a whole-codebase re-audit; **NOT** a keystone
re-litigation (W6a `docs/AUDIT_PHASE_FACTORY_W6_2026_05_18.md` did that and is frozen
`KEYSTONE-INTACT` — this is the *broader* pre-tag quality/security audit).
**Baselines:** v1.5.0 frozen at `2ff8ac3` (`git rev-list -1 v1.5.0` = `2ff8ac3…`,
re-derived — the epic-boundary anchor); W6a's audit target `edd4483`; `654cf64` is
W6a's doc-only commit over `edd4483` (the crates/ delta `edd4483..654cf64` is **EMPTY** —
re-derived — so W6a's keystone verdict at `edd4483` holds byte-for-byte at HEAD).
**Mode:** READ-ONLY except this doc. `git -C`/`git show`/`git diff` + a throwaway
`/tmp/w6b-base` worktree at `2ff8ac3` + bounded `cargo test`/`cargo run -p fsm-cli`
(disk 17 G free — no `cargo clean -p` needed; deps warm) + `cargo +stable audit`
(present, v0.22.1) + `npm audit --package-lock-only` (no install). **No `git stash`
(zero push/pop/apply/drop — `git stash list` empty throughout)**; nothing heavy
installed on the box.
**Toolchain (asserted from inside the worktree):** `rustup show active-toolchain`
→ `1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')`
— the repo pin (bare `rustc --version` → 1.75 here; benign box default reads 1.95
elsewhere, not pin drift).

---

## Verdict

> **`TAG-CLEAR`** — **0 open P0; 0 P1; 3 P2; 3 P3** (the v1.1–v1.5 §3 standard of
> 0 P0 AND ≤5 well-scoped P1 is met with **zero P1**). The Rust SCA lens is **0
> vulnerabilities** (binding Doc 00 §11.48/§11.42 — independently run). The binding
> JS-lane npm-audit finding is **3 PRE-EXISTING devDependency-only transitive
> vulnerabilities (1 moderate, 2 high), each `accept-with-written-rationale`**
> (build-time/test-only, NOT in the shipped extension runtime, no non-breaking fix,
> the factory epic introduced **ZERO** of them — independently derived) — they do
> **not** gate the tag under the Doc 00 §11.48/§11.42 standard applied honestly to
> pre-existing devDep transitives. **No architectural break, no correctness/security
> defect, no gamed gate.**

The four per-lens verdicts, each independently re-derived (not echoed):

1. **Lens 1 — Architecture / Boundaries / Coupling: PASS.** The W5 ci.yml change is
   **purely additive** (`git diff 2ff8ac3..654cf64 -- .github/workflows/ci.yml` =
   one hunk `68a69,321` — 253 lines appended after the original 68; the `build` 1.75
   matrix + the `sca` job are **byte-untouched**; the 4 new lanes `conformance /
   extension-host / coverage-rust / on-target` are each own-job `needs:`-free, the
   proven `sca`-job isolation). The W1 trace-hook is a clean new sibling
   `emit/trace_hook.rs` (`pub mod trace_hook;`), every emission `#ifdef
   FSM_TRACE`-gated (re-derived: 21 `FSM_TRACE` tokens in a default `Motor.c`, all
   inside `#ifdef…#endif`). The keystone seam is intact (W6a, frozen) and the
   *broader* coupling is clean: the only Cargo.lock change is **2 internal dev-dep
   edges** (`fsm-codegen-c`, `tempfile`) added to `fsm-simulator` — **0 new external
   `[[package]]`** (both were already in the lock at `2ff8ac3`); the documented
   no-cycle (codegen-c → fsm-ir + fsm-diagnostics only) holds.
2. **Lens 2 — Correctness / Code-Quality / Complexity: PASS.** The FW110 fixes are
   genuine codegen-correctness fixes (the differential found + fixed a real shipped
   timer over-fire — re-derived: the default `motor` non-`FSM_TRACE` C changed from a
   single-shot `elapsed_ms` decrement to a correct multi-period budget-loop), not
   band-aids; differential-proven (W6a, frozen; my own re-run `7 passed; 0 failed`).
   The new harnesses are large but justified test/harness code (well-structured,
   semantics-disclaiming doc-comments); no shipped-runtime complexity hotspot is
   architectural debt. The two carried stale-doc-comment micros are confirmed
   **cosmetic P3** (one of them — `symbol_table.rs:194` — is **not in the epic
   delta** at all, out of scope); the `.contains` codegen-c conformance-oracle debt
   is a tracked P2 (the #123-class deferred tail).
3. **Lens 3 — Reliability / AI-friendliness: PASS.** The differential engine drives
   the shipped unforked `execute_trace` oracle (W6a frozen; re-confirmed: W2 `.rs`
   semantics-`fn` negative-grep = ∅, the on-target strong `fsm_trace_emit` is a
   byte-passthrough). The gates are non-vacuous (re-derived: the W3 RED-PROOF
   factors the IDENTICAL gap-detection core + deliberate-removal proof; the W4
   ratchet enforces-not-decides + `--check-monotonic` + an explicit no-assertion-free
   anti-gaming statement in `coverage-floors.toml`); the honest-skip discipline is
   real (the on-target lane honest-skips by design — NOT a finding); clear seams +
   durable never-game guards.
4. **Lens 4 — Security / SCA: PASS (binding).** `cargo +stable audit` over 207 crate
   deps = **0 vulnerabilities / 0 warnings** (independently run; the pinned-1.75
   CVSS-4.0 caveat avoided via `+stable`). The 3 npm-audit findings are pre-existing,
   devDep-only, not-in-runtime, no-non-breaking-fix — each `accept-with-written-
   rationale` (per-vuln below); the epic added **0** of them (W4 added `c8` + a
   24-package transitive tree that is **clean**; baseline vuln-set == HEAD vuln-set,
   independently diffed). No new attack surface from the W2 harness (CRT/semihosting
   plumbing, no network) or the W5 CI installs (CI-runner-only, isolated).

---

## Why this audit is not a rubber-stamp

A clean verdict on an +8188-line epic that deliberately changed production codegen
semantics, doubled the conformance corpus, and added a new npm devDep tree is itself
suspect — so the load-bearing re-derivations are shown explicitly (full table in
§Independent Derivations). Each is **the auditor's own command/read, NOT echoed**
from W6a, the stage-gate docs, or any wave's self-report:

- **The "byte-identical production C" claim was independently FALSIFIED-then-correctly-
  scoped.** The brief + the `trace_hook.rs` doc-comment say a default `fsm generate`
  is "byte-identical". I built `examples/motor/motor.fsm` at *both* `2ff8ac3` and
  `654cf64` and diffed: the file *text* differs by 244 lines. I then mechanically
  stripped all `#ifdef FSM_TRACE` blocks and re-diffed the **non-gated** C: it STILL
  differed by 19 lines — and reading them showed the **timer-advance logic was
  rewritten** (single-shot `elapsed_ms` decrement → a correct `for(;;) { step =
  min-remaining; budget -= step; … }` budget-loop). This is **not a regression and
  not a fork** — it is FW110-FU-D/E (`f92c1cb`/`ea1829c`/`1fb1d36`): the differential
  surfaced a genuine shipped **timer over-fire bug** and fixed it. So the precise
  truth (not the doc's loose phrasing) is: the *trace-hook instrumentation itself*
  adds nothing when `FSM_TRACE` is undefined, but the *epic deliberately changed
  production-C semantics for the better* via the FW110 correctness fixes. The keystone
  is about no *forked oracle*, not frozen output — and that holds (W6a; my re-runs).
  This is the audit working as designed (the differential caught a real bug). The
  loose "byte-identical" phrasing is logged **F-02 (P2)**.
- **The npm SCA was re-derived at TWO bases, not trusted.** I ran `npm audit
  --package-lock-only` at HEAD `654cf64` AND at baseline `2ff8ac3` (via a throwaway
  `/tmp/w6b-base` worktree) AND `--json`-diffed the vulnerability sets:
  **byte-identical `{esbuild, mocha, serialize-javascript}` — 1 moderate, 2 high — at
  BOTH commits.** The factory epic's only npm change is W4's `c8: ^10.1.3` + a
  24-package transitive tree (enumerated from the lock diff) which contributes **ZERO**
  vulnerabilities. So the brief's "W5 added zero npm deps" is *imprecise* (W4 added
  `c8`'s tree — consistent with Doc-32 GT-11 "c8 is genuinely net-new"; that tree is
  vuln-free), and the 3 vulns are **strictly pre-existing devDep transitives the epic
  did not introduce.** Logged **F-01 (P2)** for the brief/record framing precision.
- **The Rust SCA was actually run, not assumed.** `cargo +stable audit` (v0.22.1,
  present — no install) over 207 deps: the advisory DB loaded (1090 advisories),
  Cargo.lock scanned, **exit 0, zero `vulnerability`/`warning`/`RUSTSEC` lines** —
  0 Rust vulns. Not honest-skipped (the tool is present); the binding Doc 00 §11.48
  lens is satisfied empirically.
- **The keystone-broader-coupling was re-enumerated, not echoed from W6a.** W6a froze
  `KEYSTONE-INTACT` at `edd4483`; I independently verified `654cf64` is doc-only over
  `edd4483` (`git diff --shortstat edd4483..654cf64 -- crates/ Cargo.toml Cargo.lock`
  = **EMPTY**) so W6a's verdict holds at HEAD by construction, then re-ran the host
  differential (`7 passed; 0 failed`), the W2 toolchain-free guards (`6 passed`), and
  the W3 lock (`4 passed`) myself, and re-grepped the W2 `.rs` for any semantics `fn`
  (∅) and read the on-target C strong-override (byte-passthrough). No "W6a said X"
  reasoning is load-bearing here.
- **The gate non-vacuity was re-derived from the test bodies.** I read the W3
  `red_proof_lock_fails_when_a_live_code_is_neither_fixture_nor_allowlisted` (it
  factors the gap logic into a shared `uncovered_codes()` so the proof exercises the
  *identical* logic + a deliberate-removal RED half) and the `coverage-gate.py`
  structure (`--check-monotonic` BASE rejects a lowered floor; the script *enforces*
  not *decides*; `coverage-floors.toml` carries an explicit "NO assertion-free
  line-execution test was added to inflate any baseline"). The gates are genuinely
  non-vacuous, not symbol-presence.

The one place a tag-blocker could hide (a forked oracle / a gamed gate / an
un-triaged exploitable vuln / an architectural break) was specifically hunted across
all 4 lenses and is **absent**. The only findings are framing-precision (F-01/F-02
P2), an accepted-tracked devDep-SCA disposition (F-03 P2), and cosmetic/scoping nits
(F-04..F-06 P3) — none gating.

---

## Lens 1 — Architecture / Boundaries / Coupling — PASS

**Is the epic delta architecturally sound? Clean boundaries, no improper coupling, no
band-aid layering?** Independently re-derived:

| Check | Method (run by the auditor) | Result |
|---|---|---|
| W5 ci.yml additive-isolation — `build`+`sca` untouched | `diff <(git show 2ff8ac3:.github/workflows/ci.yml) <(git show 654cf64:…)` | **one hunk `68a69,321`** — 253 lines APPENDED after the original 68; **zero modification** to `build` (1.75 matrix) or `sca` (Rust audit). The 4 new lanes are own-job `needs:`-free (comment line 79: "each lane is its OWN job under `jobs:`, `needs:`-free") — the proven `sca`-job isolation; a new lane RED never gates the 1.75 matrix |
| W1 trace-hook module boundary | `git diff 2ff8ac3..654cf64 -- crates/fsm-codegen-c/src/emit/mod.rs` | clean additive sibling: `+pub mod pseudostate;` `+pub mod trace_hook;` alongside the existing emitters — no re-layering of `emit/` |
| Trace emission is compile-gated (zero default-codegen coupling) | built `examples/motor/motor.fsm` @ `654cf64`; `grep -c FSM_TRACE Motor.c` = 21, all inside `#ifdef FSM_TRACE…#endif`; the struct hook is `super::trace_hook::emit_trace_struct_fields()` wrapped `#ifdef FSM_TRACE` (header.rs:341 read) | the instrumentation is **compile-time-opt-in**; default `fsm generate` carries it as preprocessor-stripped text only — the append-only-instrumentation discipline (the boundary is correct) |
| Cargo.lock — no new external dep / no cycle | `git diff 2ff8ac3..654cf64 -- Cargo.lock` ⇒ exactly `+ "fsm-codegen-c",` `+ "tempfile",` (the `fsm-simulator` dep list); `git show 2ff8ac3:Cargo.lock \| grep -c '^name = "tempfile"'`=1, `…"fsm-codegen-c"`=1 | **0 new external `[[package]]`**; both edges are *internal* dev-deps already in the lock; `fsm-simulator/Cargo.toml` documents the no-cycle (`fsm-codegen-c` depends on `fsm-ir`+`fsm-diagnostics` ONLY, dev-dep only) — sound |
| `#![forbid(unsafe_code)]` invariant | `git grep -l '#![forbid(unsafe_code)]' 654cf64 -- 'crates/*/src/*.rs'` ⇒ 12 roots / 11 distinct crates; same at `2ff8ac3` ⇒ 12 | **UNCHANGED** 12/11 (the epic added a dev-dep edge + harnesses + CI config, no crate root) — Doc-32 §5 expectation met, re-derived |
| W6a keystone applicable at HEAD | `git diff --shortstat edd4483..654cf64 -- crates/ Cargo.toml Cargo.lock` | **EMPTY** — `654cf64` is W6a's doc-only commit over its audit target `edd4483`; W6a's frozen `KEYSTONE-INTACT` (no-fork, ∅ negative-grep, catalogue-integrity) holds byte-for-byte at HEAD |
| W2 harness coupling (the keystone seam, broader) | `git show 654cf64:…/on_target_qemu_differential.rs \| grep -E 'fn [a-z_]*(select_transition\|eval_guard\|deadlock\|step_once\|resolve_target\|run_to_completion\|advance_clock\|compute_lca)'` ⇒ ∅; on-target C strong-`fsm_trace_emit` read (startup_mps2_an385.c:92-105) | ∅ semantics-fn; the strong override "passes the codegen's already-NUL-terminated line straight through … No reformatting — byte-for-byte the same line" — the substrate changes, the oracle/format does not. **No improper coupling, no second interpreter** |

**Verdict: PASS.** The epic is additively isolated (CI lanes, the trace-hook module),
introduces no external dependency and no dependency cycle, preserves the
forbid-unsafe invariant, and the keystone seam (W6a frozen) plus the *broader*
coupling are clean. No band-aid layering.

---

## Lens 2 — Correctness / Code-Quality / Complexity — PASS

**The delta's code quality + complexity + test-debt posture** (the 11 FW110 fixes are
differential-proven correct per W6a/#110/#118 — here I assess quality/maintainability,
not re-prove correctness):

| Check | Method | Result |
|---|---|---|
| The FW110 fixes are real corrections, not band-aids | built `motor` @ `2ff8ac3` and @ `654cf64`; stripped `#ifdef FSM_TRACE`; diffed non-gated C | the timer-advance was rewritten **single-shot `elapsed_ms` decrement → a correct multi-period budget-loop** (FW110-FU-D/E `f92c1cb`/`ea1829c`/`1fb1d36`) — a genuine **timer over-fire** fix the differential surfaced; clean, not a patch-over |
| The analyzer change shape (FW110-FU-A2) | `git diff 2ff8ac3..654cf64 -- crates/fsm-analyzer/src/lower/state.rs` | adds `lower_choice_branch` + a `BranchLike` trait abstraction (choice/junction branch guard+actions+target lowering) — a clean trait-factored abstraction over the two branch syntaxes, **not** duplicated if/else; +101/-… additive-shaped |
| Complexity hotspots | `wc -l` on the new/changed files @ `654cf64` | largest: `codegen_equivalence_smoke.rs` 2222, `on_target_qemu_differential.rs` 1207, `conformance_code_coverage_lock.rs` 915, `trace_hook.rs` 717 — **all test/harness** (size inherent to a 12-member differential corpus + catalogue-integrity + non-vacuity guards); largest *shipped runtime* = `timer.rs` 794 / new `pseudostate.rs` 558 (FW110 fixes, differential-proven). Each is well-structured with clear fn inventories + semantics-disclaiming doc-comments. **No shipped-runtime hotspot is architectural debt** |
| Glue quality — `scripts/coverage-gate.py` | read fn inventory + the floors-toml header | enforces-not-decides (single source of truth = `coverage-floors.toml`); `--check-monotonic`; honest measured numbers; an explicit anti-gaming statement — high quality |
| Glue quality — `scripts/workflow-lint.py` / Makefile / ci.yml | W6a §2(c) (frozen) + my re-grep | INVOCATION/lint-only, FSM-semantics keywords appear ONLY in disclaiming comments — sound |
| Test-debt posture — the `.contains` codegen-c oracle (#123-class deferred tail) | `docs/AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md:306` (frozen, in the epic delta) | the normative codegen-c conformance oracle is **substring-presence** (`.contains`), explicitly flagged **P1 hollow-AND-correctness-critical**, recommended for behavioural conversion in the G7 wave — **already tracked + owned**; the 4 codegen-c MANIFEST fixtures are the deferred tail. **F-04 (P2)** — carried, tracked, not regressed; the W3 lock + W1/W2 differential cover behavioural correctness for the corpus |
| Carried micro #1 (stale doc-comment) | `git show 654cf64:…/codegen_equivalence_smoke.rs` lines 807-820 | `run_differential`'s doc-comment says "`Some(corrupt)` mutates the oracle's Nth record" but the signature is `fn run_differential(example: &str)` — no `corrupt` param; the green path body is `byte_diff(example,&oracle,&generated)` (no corruption). **Cosmetic comment-vs-signature drift; corruption lives in the dedicated never-game guard (read+re-run green). F-05 (P3)** |
| Carried micro #2 (stale doc-comment) | `git show 654cf64:crates/fsm-analyzer/src/symbol_table.rs` line 194; `git diff --stat 2ff8ac3..654cf64 -- …/symbol_table.rs` = **EMPTY** | the doc-comment "Emits FSM-E0020..E0024 / E0025 …" slightly over-claims, BUT this file is **NOT in the factory-epic delta** — a pre-existing micro, **out of scope** for a delta-scoped pre-tag audit. **F-06 (P3 — defer to a general doc-honesty pass, NOT this epic's closeout-batch)** |
| Conformance corpus delta (W3 deliverable) | `git show 654cf64:tests/conformance/MANIFEST.json` ⇒ 53 fixtures (parser 7/validator 11/semantic 28/codegen-c 4/formatter 3); was 26 @ v1.5 | the conformance corpus roughly **doubled 26→53** (W3's "bulk fixtures authored") — a positive quality delta, correctly attributed |

**Verdict: PASS.** The FW110 fixes are genuine corrections (the differential caught a
real timer bug); the analyzer abstraction is clean; complexity is concentrated in
justified test-harness code; the glue is high quality; the test-debt (`.contains`
codegen-c oracle) is honestly tracked + owned for G7. The two stale-doc micros are
cosmetic (one out-of-scope). No correctness or maintainability defect.

---

## Lens 3 — Reliability / AI-friendliness — PASS

**The holistic reliability posture of the new factory-reliability machinery + clear
seams + durable never-game guards:**

| Check | Method (run by the auditor) | Result |
|---|---|---|
| The differential engine drives the shipped unforked oracle | W6a §1/§2 (frozen, applies at HEAD per the EMPTY `edd4483..654cf64` crates/ diff) + my re-grep of the W2 `.rs` (∅ semantics fn) + reading the on-target strong-`fsm_trace_emit` (byte-passthrough) | **no fork** — the oracle is the shipped `fsm_simulator::execute_trace` `StepRecord`; the substrate (host/QEMU) changes, the oracle does not |
| The host differential is non-vacuous + deterministic | `cargo test -p fsm-simulator --test codegen_equivalence_smoke` (my own run) | **`7 passed; 0 failed; 2 ignored`** — independently reproduced; the 7 incl. the corrupted-oracle RED guards + the `oracle_is_the_shipped_execute_trace_seam_not_a_fork` executable attestation + the catalogue-integrity assertion |
| The W2 toolchain-free keystone guards green | `cargo test -p fsm-simulator --test on_target_qemu_differential` (my own run) | **`6 passed; 0 failed`** — the toolchain-free no-fork guard + `byte_equal_corpus_matches_w1_catalogue` + the host-gcc-strengthened W2 driver all green; the QEMU lane honest-skips by design |
| The honest-skip discipline is real (not a silent pass) | W6a §3 on-target honest-skip (frozen) + the lane prints an explicit by-design notice; `cargo test` still PASSES | the on-target lane is CI-runner-only (Doc-32 §W2/GT-10/OWNER-2); the *logic* is host-proven by W1's identical-C/same-oracle differential — **NOT a finding**, the correct honest-skip posture |
| The W3 gate is non-vacuous (ratchet/lock) | read `conformance_code_coverage_lock.rs` `red_proof_…` (lines 676-720) + `cargo test -p fsm-cli --test conformance_code_coverage_lock` (my run) | **`4 passed; 0 failed`**; the RED-PROOF factors the gap logic into a shared `uncovered_codes()` core so it exercises the **IDENTICAL** logic the real lock uses + a deliberate-removal RED half + a GREEN-on-real-inputs half — genuinely non-vacuous, the live target derived from `DiagnosticCode::all_codes()` (the compiled enum, line 535-536), not a doc |
| The W4 ratchet monotonicity + anti-gaming | read `coverage-gate.py` (`--check-monotonic` rejects a lowered `floor_*`; the script ENFORCES not DECIDES) + `coverage-floors.toml` header | `target_*` recorded-not-Day-1; floor = `floor(measured−2%)`; explicit "NO assertion-free line-execution test was added to inflate any baseline … only … BEHAVIOURAL §5.4 tests" — a durable never-game guard |
| The COVERAGE_MAP drift-proofing | read the W3 `coverage_map_md_is_byte_derivable_…` test + the `regen-coverage-map` feature gate | the map's byte-derivability is asserted **read-only**; regeneration is feature-gated (`--features regen-coverage-map`) + `#[ignore]`d so neither the normal suite nor a stray `--ignored` run can rewrite it — a hand-maintained-map drift surface eliminated (GT-9 closed) |
| The never-game catalogue ratchet | W6a §3 (the non-byte-equal loop fails LOUDLY `FW109 CATALOGUE STALE` if a JUSTIFIED/DIVERGENT fixture converges) — frozen; consistent with the catalogue partition I re-derived via the green test run | a converged fixture *must* move — an empty `KNOWN_DIVERGENT` cannot be gamed |
| AI-friendliness / clear seams | the disclaiming-comment discipline across trace_hook/W2-harness/CI-glue (W6a §2, frozen) + the single-source-of-truth catalogue arrays + `coverage-floors.toml` | clear, auditable seams; every fork-possible surface carries an explicit "defines NO semantics … oracle remains exclusively `fsm_simulator::execute_trace`" disclaimer — maintainable + future-audit-legible |

**Verdict: PASS.** The new machinery is reliable, non-vacuous, and durable: the
differential drives the unforked oracle, the gates RED on real perturbation, the
ratchet only climbs, the honest-skip is genuine, and the seams are clear +
self-disclaiming (AI-friendly / future-audit-legible).

---

## Lens 4 — Security / SCA — PASS (binding; Doc 00 §11.48/§11.42)

### 4.1 Rust SCA — `cargo +stable audit` (independently run, NOT honest-skipped)

`cargo-audit` v0.22.1 is **present** (`~/.cargo/bin/cargo-audit`) and a `stable`
toolchain exists, so the pinned-1.75 CVSS-4.0-DB-parse caveat (the documented `sca`-job
limitation) is sidestepped via `cargo +stable audit`:

```
$ cd /root/dev/embeded-fsm-sdk-wt-w6-closeout && cargo +stable audit
Loaded 1090 security advisories (from ~/.cargo/advisory-db)
Scanning Cargo.lock for vulnerabilities (207 crate dependencies)
$ echo $? → 0       # zero `vulnerability`/`warning`/`RUSTSEC`/`error` lines emitted
```

**Result: 0 Rust vulnerabilities, 0 warnings** over 207 crate deps. Binding Doc 00
§11.48/§11.42 ("0 vulnerabilities before a vX.Y.0 tag") satisfied empirically for the
Rust dependency graph. **Not honest-skipped** (the tool is present and ran).

### 4.2 JS-lane npm-audit triage (the binding W5-flagged finding) — each vuln disposed

`npm audit --package-lock-only --audit-level=low` on `editors/vscode` at HEAD
`654cf64` AND at baseline `2ff8ac3` (a throwaway `/tmp/w6b-base` worktree) — the
`--json` vulnerability sets were **byte-identical at both commits** (independently
diffed): `{esbuild, mocha, serialize-javascript}` — **1 moderate, 2 high**.
Runtime `dependencies` = exactly `{elkjs, vscode-languageclient}` (independently read
from `package.json@654cf64`); the VSIX = the esbuild-bundled `dist/extension.js`
(`.vscodeignore` excludes `node_modules/**` entirely; `main: ./dist/extension.js`).

| # | Vuln | Sev | devDep vs runtime (derived) | Exploitable in our build/CI? | Non-breaking fix? | **Disposition** |
|---|---|---|---|---|---|---|
| 1 | `esbuild <=0.24.2` (GHSA-67mh-4wv8-2f99 — any website can probe the dev server) | **moderate** | **devDependency** (`esbuild ^0.20.2` in `devDependencies`, NOT `dependencies`); it is the **build-time bundler** that *produces* `dist/` — never shipped *in* the VSIX (esbuild is the tool, not a runtime dep; `.vscodeignore` drops node_modules) | **No.** The advisory is about a *running esbuild dev-server* (`esbuild serve`). The project uses esbuild only as a one-shot `esbuild.mjs` bundler in `npm run bundle` (no dev-server, no `serve`); CI runs it headless. No attacker-reachable dev-server in our build/CI. | **No.** `npm audit fix` (no `--force`) resolves nothing; the only fix is `--force` → esbuild@0.28.0, a **major bundler bump** (breaking, would need a full re-bundle/re-test of the shipped extension). | **`accept-with-written-rationale`** — pre-existing (identical at v1.5.0 baseline), devDep-only/not-in-runtime, no attacker-reachable dev-server in our one-shot-bundler usage, no non-breaking fix; the epic introduced ZERO of it. Re-state in the gate doc's carried-escalations; a deliberate esbuild major-bump is its own scoped chore (v1.6 FE-hygiene), NOT a Phase-6.0 tag-blocker. |
| 2 | `serialize-javascript <=7.0.4` (GHSA-5c6j-r48x-rmvq RCE via RegExp.flags; GHSA-qj8w-gfj5-8c6v CPU-exhaustion DoS) | **high** | **transitive of `mocha`** (test runner) — not a direct dep, not in runtime `dependencies`; mocha+its tree are excluded from the VSIX (`.vscodeignore` + the esbuild bundle inlines only runtime deps) | **No.** `serialize-javascript` is reached only inside the Mocha test harness (`@vscode/test-electron` Extension-Host run) over **the project's own trusted test fixtures** — no untrusted input is serialized; the RCE/DoS require attacker-controlled input to `serialize()`, which never occurs in our test run. Test-only, trusted-input. | **No.** Fix path = `mocha@7.2.0` (a **downgrade** from `^10.4.0` — itself a regression) or wait for an upstream mocha bump; no non-breaking `npm audit fix`. | **`accept-with-written-rationale`** — pre-existing, test-only transitive, trusted-input (no untrusted data serialized), not in shipped runtime, no non-breaking fix (the offered fix is a major mocha *downgrade*); the epic introduced ZERO of it. Re-state in the gate doc's carried-escalations; track for the v1.6 FE-hygiene batch (await an upstream mocha that pulls serialize-javascript ≥7.0.5). |
| 3 | `mocha 8.0.0-12.0.0-beta-2` (depends on the vulnerable serialize-javascript) | **high** | **devDependency** (`mocha ^10.4.0` in `devDependencies`) — the test runner; excluded from the VSIX | **No.** Same as #2 — mocha runs only the project's own trusted Extension-Host tests; the vulnerability is wholly via the bundled serialize-javascript with trusted input. No production/runtime exposure. | **No.** Same as #2 (the only `--force` path is a mocha *downgrade*). | **`accept-with-written-rationale`** — identical rationale to #2 (it IS the parent of #2); pre-existing, test-only, trusted-input, not-in-runtime, no non-breaking fix. Re-state in carried-escalations; v1.6 FE-hygiene tracked. |

**Per the Doc 00 §11.48/§11.42 "0 vulnerabilities before tag" standard applied
*honestly* to pre-existing devDep transitives (W4/W5 added zero of them):** all 3 are
build-time/test-only, **not in the shipped extension runtime**, **not exploitable in
our build/CI usage** (no esbuild dev-server; trusted-input-only Mocha), and have **no
non-breaking fix** (the offered fixes are a major esbuild bump and a major mocha
downgrade — each more disruptive than a non-shipped, non-reachable risk). They are
**NOT P0 tag-blockers**; each is `accept-with-written-rationale`, **re-stated in the
gate doc's carried-escalations + tracked for the v1.6 FE-hygiene batch** (the §11.62 /
v1.5 §6 re-state-don't-drop discipline). This matches the v1.5 precedent (the v1.5
audit accepted analogous pre-existing devDep nits without blocking the tag). Logged
collectively as **F-03 (P2 — accepted-tracked, not gating)**.

### 4.3 New attack surface from the epic

| Surface | Method | Result |
|---|---|---|
| W4's new npm tree (`c8`) | enumerated the 24 new `node_modules/*` entries from the lock diff; `--json` vuln-set diff baseline↔HEAD | `c8` + its tree (@bcoe/v8-coverage, @istanbuljs/schema, @jridgewell/*, istanbul-*, v8-to-istanbul, …) add **ZERO vulnerabilities** (baseline vuln-set == HEAD vuln-set); devDependency-only (coverage tooling, not shipped) — **no new attack surface** |
| W2 harness | W6a §2(b) (frozen) + the on-target C read | CRT bring-up + semihosting + a deterministic counter-clock — **no network, no untrusted input, no file I/O beyond the test temp dir**; pure platform plumbing. No new attack surface |
| W5 CI-runner installs | `git show 654cf64:.github/workflows/ci.yml` (the new lanes' `apt-get`/`cargo install`/`npm`) | every heavy install (`qemu-system-arm`, `gcc-arm-none-eabi`, `cargo-llvm-cov`, `c8`) is **CI-runner-only**, in own `needs:`-free jobs (never on the box, never in the shipped artifact); the additive-isolation contains the surface to ephemeral CI runners. No new shipped attack surface |

**Verdict: PASS (binding).** Rust SCA = 0 (empirically run). The 3 npm findings are
pre-existing, devDep-only, not-in-runtime, not-exploitable-in-our-usage,
no-non-breaking-fix → each `accept-with-written-rationale`, re-stated + tracked, not
gating under the honestly-applied §11.48/§11.42 standard. No new attack surface from
the epic.

---

## Carried-inputs disposition (the brief's explicit asks)

| Carried input | Independent finding | P-level | Disposition |
|---|---|---|---|
| The 3 extension transitive-devDep npm vulns (1 mod, 2 high) | Pre-existing (byte-identical at `2ff8ac3` & `654cf64`); devDep/build-time/test-only; NOT in shipped runtime; not exploitable in our build/CI; no non-breaking fix; epic added 0 of them | **F-03 P2** | **accept-with-written-rationale (×3, each per-vuln above)** — NOT P0; re-state in `GATE_VERIFICATION_<tag>.md` §carried-owner-escalations + the v1.6 FE-hygiene batch. **Does NOT gate the tag.** |
| `cargo audit` result | `cargo +stable audit` present, ran: **0 vulns / 0 warnings** over 207 deps | — | Binding Doc 00 §11.48 satisfied empirically; **not** honest-skipped, **nothing flagged for W6d's cold-quad `sca` lane** beyond the standard re-run-at-`X` (the tool works here; W6d should still run it at the tag-doc commit `X` per the canonical cold-quad). |
| `codegen_equivalence_smoke.rs` ~L807 `Some(corrupt)` doc-vs-signature drift | Confirmed: doc-comment mentions a `corrupt` param absent from `fn run_differential(example:&str)`; green path applies no corruption (body read); corruption is in the dedicated never-game guard (read + re-run green) | **F-05 P3** | Cosmetic comment-vs-signature drift → **W6c closeout-batch** (tidy the doc-comment; non-gating, read-only here). |
| `fsm-analyzer/src/symbol_table.rs:194` over-claiming E0025 | Confirmed slight over-claim BUT `git diff --stat 2ff8ac3..654cf64 -- …/symbol_table.rs` = **EMPTY** → **not in the epic delta** | **F-06 P3** | **Out of scope for this delta-scoped pre-tag audit** → defer to a general doc-honesty pass, **NOT** the W6c Phase-6.0 closeout-batch (it predates and is untouched by the epic). |
| The W3 §6 catalog-only deferred tail (#123 — the `.contains` codegen-c conformance oracle) | Confirmed tracked: `AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md:306` flags it **P1 hollow-AND-correctness-critical**, recommends behavioural conversion in the **G7 wave**; the 4 codegen-c MANIFEST fixtures are the tail; W1/W2 differential + the W3 lock cover behavioural correctness for the corpus in the interim | **F-04 P2** | **Accepted-tracked-with-rationale** — a known, owned, deferred test-debt item (the G7 behavioural-acceptance conversion), NOT regressed by the epic, NOT a tag-blocker (the differential + lock are the binding behavioural gate today). Re-state in the gate doc's carried items. |

---

## Findings (P0 ship-blocker / P1 well-scoped / P2 minor / P3 nit)

| ID | Lens | Sev | Location | Issue | Recommended action / disposition |
|---|---|---|---|---|---|
| F-01 | 4 | **P2** | the brief / `GATE_VERIFICATION_<tag>.md`-to-be (record framing) | "W5 added zero npm deps" is imprecise — **W4** added `c8: ^10.1.3` + a 24-package transitive tree (consistent with Doc-32 GT-11 "c8 is genuinely net-new"). The tree is **vuln-free** (independently diffed) and devDep-only; the substantive claim (the epic added no *vulnerable* / no *runtime* npm dep) is **TRUE**. Provenance precision only. | Non-gating. The gate doc should state precisely: "W4 added the `c8` coverage devDep + its 24-pkg transitive tree (devDependency-only, 0 new vulnerabilities — independently audited); W5 added 0 npm deps." → **W6c closeout-batch** wording. |
| F-02 | 1/2 | **P2** | `crates/fsm-codegen-c/src/emit/trace_hook.rs` module doc (`//!` lines ~8-11) + the brief | "a default `fsm generate` produces **byte-identical** output" is loosely phrased: it is precise about the *trace-hook block* (preprocessor-stripped when `FSM_TRACE` undefined) but the **epic deliberately changed production-C semantics** (FW110-FU-D/E — the `motor` non-`FSM_TRACE` C's timer-advance was rewritten; independently re-derived). This is **correct** (a real timer-over-fire fix the differential caught), but the doc-comment's unqualified "byte-identical" could mislead a future reader into thinking *no* production C changed in the epic. | Non-gating (the change is a *correctness fix*, not a regression; the keystone is no-fork not frozen-output, and that holds). Recommend the gate doc's release-record-symmetry section state the production-C delta precisely as "the FW110 codegen-correctness fixes (intended) + the compile-gated trace-hook (zero default-codegen effect)". → **W6c closeout-batch** + optionally tighten the `trace_hook.rs` `//!` qualifier (a future codegen-touching wave). |
| F-03 | 4 | **P2** | `editors/vscode` devDeps (esbuild/mocha/serialize-javascript) | 3 pre-existing devDep-only transitive vulns (1 mod, 2 high); not in shipped runtime; not exploitable in our build/CI; no non-breaking fix; epic added 0 of them. | **accept-with-written-rationale ×3** (per-vuln §4.2). Re-state in `GATE_VERIFICATION_<tag>.md` §carried-owner-escalations + the v1.6 FE-hygiene batch. **Does NOT gate the tag** (the §11.48/§11.42 standard applied honestly to pre-existing non-shipped devDep transitives). |
| F-04 | 2 | **P2** | `tests/conformance/codegen-c/*` oracle (`.contains`) | The codegen-c conformance oracle is substring-presence (hollow); already flagged P1 in `AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md:306`, owned by the **G7 wave** for behavioural conversion; the W1/W2 differential + the W3 lock are the binding behavioural gate in the interim. | **accept-tracked-with-rationale** — known/owned/deferred test-debt (G7 conversion), NOT regressed, NOT a tag-blocker. Re-state in the gate doc's carried items. |
| F-05 | 2 | **P3** | `crates/fsm-simulator/tests/codegen_equivalence_smoke.rs` ~L807-809 | `run_differential` doc-comment mentions a `Some(corrupt)` param absent from the signature; green path applies no corruption (body read); corruption is in the dedicated never-game guard (read + re-run green). Cosmetic. | **W6c closeout-batch** (tidy the doc-comment). Non-gating; read-only here. |
| F-06 | 2 | **P3** | `crates/fsm-analyzer/src/symbol_table.rs:194` | Doc-comment slightly over-claims E0025 emission. **NOT in the epic delta** (`git diff --stat 2ff8ac3..654cf64` empty for this file). | **Out of scope** for this delta-scoped pre-tag audit → defer to a general doc-honesty pass, **NOT** the Phase-6.0 W6c closeout-batch. |

**No P0. No P1.** All findings are framing-precision (F-01/F-02), an
accepted-tracked devDep-SCA disposition (F-03), an accepted-tracked test-debt
(F-04), or cosmetic/out-of-scope nits (F-05/F-06). The corpus is architecturally
sound, the new machinery non-vacuous + secure, the keystone intact (W6a, frozen,
applicable at HEAD), and the Rust SCA empirically clean.

---

## Independent Derivations Table (proof I re-derived, did not echo)

| # | Derived fact | Exact command / source | Output | Echo-or-derived |
|---|---|---|---|---|
| D-01 | v1.5.0 baseline anchor | `git rev-list -1 v1.5.0` | `2ff8ac3…` | DERIVED |
| D-02 | W6a verdict applicable at HEAD | `git diff --shortstat edd4483..654cf64 -- crates/ Cargo.toml Cargo.lock` | **EMPTY** (654cf64 = W6a doc-only over edd4483) | DERIVED |
| D-03 | Full epic Rust delta | `git diff --shortstat 2ff8ac3..654cf64 -- crates/ Cargo.toml Cargo.lock` | 22 files / +8188 / −190 | DERIVED |
| D-04 | 0 new external dep | `git diff 2ff8ac3..654cf64 -- Cargo.lock`; `git show 2ff8ac3:Cargo.lock \| grep -c '^name = "tempfile"'` / `…"fsm-codegen-c"` | only `+ "fsm-codegen-c"`,`+ "tempfile"` (internal); both =1 at baseline ⇒ 0 new `[[package]]` | DERIVED |
| D-05 | W5 ci.yml purely additive | `diff <(git show 2ff8ac3:.github/workflows/ci.yml) <(git show 654cf64:…)` | one hunk `68a69,321`; build+sca byte-untouched | DERIVED |
| D-06 | 4 new lanes own-job needs-free | `git show 654cf64:.github/workflows/ci.yml` job scan + comment L79 | conformance/extension-host/coverage-rust/on-target each own job, `needs:`-free | DERIVED |
| D-07 | forbid(unsafe_code) unchanged | `git grep -l '#![forbid(unsafe_code)]' 654cf64 -- 'crates/*/src/*.rs'` vs `2ff8ac3` | 12/11 both | DERIVED |
| D-08 | Production C deliberately changed (FW110, not frozen) | built `motor` @ 2ff8ac3 & 654cf64; stripped `#ifdef FSM_TRACE`; diffed non-gated C | 19-line diff = timer-advance rewritten single-shot→budget-loop | DERIVED |
| D-09 | Trace emission compile-gated | `grep -c FSM_TRACE /tmp/w6b-motor-head/Motor.c` + `#ifdef…#endif` scan | 21 tokens, all inside `#ifdef FSM_TRACE` | DERIVED |
| D-10 | Host differential green (non-vacuous) | `cargo test -p fsm-simulator --test codegen_equivalence_smoke` (auditor-run) | `7 passed; 0 failed; 2 ignored` | DERIVED |
| D-11 | W2 toolchain-free guards green | `cargo test -p fsm-simulator --test on_target_qemu_differential` (auditor-run) | `6 passed; 0 failed` | DERIVED |
| D-12 | W3 lock green + non-vacuous | `cargo test -p fsm-cli --test conformance_code_coverage_lock` (auditor-run) + read `red_proof_…` body | `4 passed; 0 failed`; shared `uncovered_codes()` core + deliberate-removal RED | DERIVED |
| D-13 | W2 .rs no semantics fn | `git show 654cf64:…/on_target_qemu_differential.rs \| grep -E 'fn [a-z_]*(select_transition\|eval_guard\|deadlock\|…)'` | ∅ | DERIVED |
| D-14 | on-target strong-emit = byte-passthrough | read `startup_mps2_an385.c:92-105` | "No reformatting — byte-for-byte the same line" | DERIVED |
| D-15 | Rust SCA clean | `cargo +stable audit` (auditor-run) | 207 deps scanned, exit 0, 0 vuln/0 warn | DERIVED |
| D-16 | npm vulns pre-existing + identical | `npm audit --json` @ 654cf64 vs @ 2ff8ac3 (`/tmp/w6b-base`) set-diff | byte-identical `{esbuild,mocha,serialize-javascript}` 1mod/2high both | DERIVED |
| D-17 | vulns are devDep, runtime = elkjs+languageclient | `git show 654cf64:editors/vscode/package.json` deps vs devDeps | runtime deps = `{elkjs, vscode-languageclient}`; esbuild/mocha in devDependencies | DERIVED |
| D-18 | VSIX excludes node_modules | `git show 654cf64:editors/vscode/.vscodeignore` + `main: ./dist/extension.js` | node_modules/** excluded; runtime = the esbuild bundle | DERIVED |
| D-19 | c8 tree adds 0 vulns | enumerate 24 new `node_modules/*` from lock diff + D-16 set-diff | 0 new vulns from the c8 tree | DERIVED |
| D-20 | no non-breaking npm fix | `npm audit fix --package-lock-only --dry-run` | resolves nothing; only `--force` (esbuild@0.28.0 major / mocha@7.2.0 downgrade) | DERIVED |
| D-21 | Live DiagnosticCode = 73 | `git show 654cf64:crates/fsm-diagnostics/src/lib.rs` ⇒ `const EXPECTED: usize = 73` | 73 (unchanged from v1.5) | DERIVED |
| D-22 | Conformance corpus doubled | `git show 654cf64:tests/conformance/MANIFEST.json` fixture count | 53 (was 26 @ v1.5) — W3 bulk fixtures | DERIVED |
| D-23 | symbol_table.rs out of epic delta | `git diff --stat 2ff8ac3..654cf64 -- crates/fsm-analyzer/src/symbol_table.rs` | EMPTY | DERIVED |
| D-24 | run_differential doc-vs-sig drift | `git show 654cf64:…/codegen_equivalence_smoke.rs` L807-820 | doc mentions `Some(corrupt)`; sig = `fn run_differential(example:&str)`; green path no corruption | DERIVED |
| D-25 | tags placed; no Phase-6.0/v1.6 tag | `git tag -l` | v1.0.0–v1.5.0 + checkpoints; **no** Phase-6.0/v1.6 tag (correctly contingent on this audit) | DERIVED |
| D-26 | .contains codegen-c debt tracked | `docs/AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md:306` | flagged P1, owned by G7 wave | DERIVED |
| D-27 | clean read-only state / no stash | `git status --porcelain`; `git stash list` (throughout) | both empty | DERIVED |
| D-28 | toolchain pin | `rustup show active-toolchain` from worktree | `1.75.0… (overridden by '<worktree>/rust-toolchain.toml')` | DERIVED |

---

## Sequence note (necessary-not-sufficient — the v1.4-W4c F-05 / v1.5 analogue)

`TAG-CLEAR` from this audit is **necessary, not sufficient**. The Phase-6.0 tag lands
on the gate-doc commit `X` (a *later* W6c/W6d motion) **iff** (a) this four-lens is
TAG-CLEAR (it is) **and** (b) W6a's keystone phase-audit is `KEYSTONE-INTACT` (it is —
frozen `docs/AUDIT_PHASE_FACTORY_W6_2026_05_18.md`, independently confirmed applicable
at HEAD per D-02) **and** (c) the §11.30 cold-from-source quad **at `X`** WITH the
MANDATORY NOTE-1 `cargo clean -p` STEP 0 + the **binding JS lane at `X`** (Doc-32 §5)
is green. The cold-quad + the JS-lane-at-`X` are **W6d's** to empirically close at the
tag-doc commit and are correctly posed as contingent (not asserted here). This audit
is RECORD+QUALITY+SECURITY integrity for the *delta*; nothing here was built to close
a claim. **Note for W6d:** `cargo +stable audit` works on this box (it ran clean
here, §4.1) — W6d should still re-run it at `X` per the canonical cold-quad `sca`
step; no honest-skip-pending-note is needed (the tool is present, contrary to the
brief's contingency).

---

## Mechanics

- Worktree: `/root/dev/embeded-fsm-sdk-wt-w6-closeout`, branch
  `phase6.6/factory-w6-closeout` (at `654cf64` == main post-W6a). Verified clean at
  start and at the time of writing (`git status --porcelain` empty throughout).
- READ-ONLY except this doc. `git -C`/`git show`/`git diff` + a throwaway
  `git worktree add /tmp/w6b-base 2ff8ac3` (the documented base↔HEAD method, NOT
  `git stash`) + bounded `cargo test`/`cargo run -p fsm-cli` (disk 17 G free, deps
  warm — no `cargo clean -p` needed) + `cargo +stable audit` (present) + `npm audit
  --package-lock-only` (no install). **Zero `git stash` (push/pop/apply/drop) — `git
  stash list` empty throughout.** Nothing heavy installed on the box; the
  ARM/QEMU/`act` toolchains remain absent by design (Doc-32 §2/GT-10/OWNER-2 — the
  on-target lane is CI-runner-only; its honest-skip is NOT a finding).
- Toolchain asserted from inside the worktree: `1.75.0-x86_64-unknown-linux-gnu
  (overridden by '<worktree>/rust-toolchain.toml')` — the repo pin.
- This doc is NEW (`docs/AUDIT_PRE_TAG_FACTORY_2026_05_18.md`); no existing
  audit/doc overwritten; exactly one file written. Committed on
  `phase6.6/factory-w6-closeout` as a sibling evidence commit — **NOT** folded into
  the tag-doc commit `X`; not merged, not pushed, not tagged.

*— End of independent pre-tag four-lens audit, Factory-Reliability/CI epic
(Phase-6.0), 2026-05-18. A rubber-stamp here would be worse than a found problem; the
four lenses are stated `TAG-CLEAR` because each was independently confirmed by this
auditor's own derivation from `git`/source/behaviour at `654cf64` (the production-C
"byte-identical" claim was specifically falsified-then-correctly-scoped; the npm SCA
was re-derived at two bases and the c8 tree independently cleared; the Rust SCA was
actually run; the gates' non-vacuity was re-derived from the test bodies), NOT
restated from W6a, the stage-gate docs, or any wave's self-report.*
