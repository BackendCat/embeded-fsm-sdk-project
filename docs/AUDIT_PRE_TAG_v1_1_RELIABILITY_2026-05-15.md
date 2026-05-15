# AUDIT — Pre-Tag v1.1 (Reliability / Security / Test-Integrity)

- **Document ID:** AUDIT_PRE_TAG_v1_1_RELIABILITY_2026-05-15
- **Date:** 2026-05-15
- **Auditor lens:** Reliability, security, and test-integrity (read-only, no code modified)
- **Scope:** v1.1 surface — all commits since v1.0.0 `b480003` through `main` `b6f4db8`
- **Baseline trust:** Clean quad at `b6f4db8` reported green (656 tests, clippy `-D warnings`, fmt, `fsm test examples/` 5/5, `tests/conformance/` 25/25). No tests re-run; this audit is source + test-body inspection.

---

## VERDICT

**TAG-READY: NO** — one P0 must be closed first.
**Reliability/Security P0 count: 1** (SEC-P0-1, below). Reliability P0 count: 0.

The P0 is **not** a behavioural miscompile (sim≡codegen holds; codegen is robust). It is a **new, unprotected file-read attack surface** (`--import-header`) shipped without the v1.0 G-02 hardening that the project's own Doc 18 §10 mandates for tooling that "runs in CI environments". Per the brief's constraint 2 (D.2: a security pass is required before any release with new surface) and the project ethos, shipping a regression in the explicitly-hardened security boundary is a tag-blocker. The fix is small and localized (route `--import-header` paths through the existing `resolve_import`-equivalent + add a read size cap); a v1.1 tag is appropriate immediately after.

Test-integrity is **strong**: every sampled §5.4 behavioural-acceptance test for the riskiest waves genuinely invokes `gcc -Werror` and **runs** the binary asserting observable state — none is a symbol-presence stand-in for behaviour. The carried-over `.contains()` assertions are structural and individually backed by a sibling gcc-RUN test. G7/G9 remain acceptable tracked partials (unchanged from the v1.0 pre-tag audit; not blockers).

---

## Test-Integrity Table (riskiest waves)

| Wave | Test file | Real `gcc` compile + **RUN**? | Behavioural or symbol-presence? |
|---|---|---|---|
| W7-FU-1 guard-disambiguated dispatch | `crates/fsm-cli/tests/guard_disambiguated_dispatch_e2e.rs` | **YES** — `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (L218-241), executes `sel_app` (L243), asserts 5 discriminating cases incl. the silent-drop regression (L131-141) + sim≡spec cross-check (L350-363), both switch & table | **Behavioural** (action counters read from the running binary) |
| W7-FU-2 default transition priority | `crates/fsm-cli/tests/default_transition_priority_e2e.rs` | **YES** — `gcc … -Werror` (L316-338), executes `pri_app` (L340), asserts winner = explicit-50 not default-100, both strategies + sim cross-check (L380-431) | **Behavioural**; hand-built IR justified (analyzer FSM-E0300 makes it inexpressible in `.fsm`; companion unit pin on lowering at `fsm-analyzer/tests/lowering.rs`) |
| W6 worked-integration examples | `crates/fsm-cli/tests/integration_examples.rs` | **YES** — make/cmake/cargo-rust/platformio each build **and run**; assert `VERIFIED_BANNER` ("OK Motor FSM: lifecycle verified") / Rust-FFI banner in stdout (L128/162/189/306) | **Behavioural** (binary must emit the lifecycle-verified banner) |
| W5 import-header | `crates/fsm-cli/tests/import_header_e2e.rs` | **YES** — `gcc … -Werror` links generated C with the example's real `driver.c` (L213-237), runs `ih_app`, asserts imported externs were genuinely invoked via `g_driver` side-effect counters + correct transitions (L48-126), both strategies | **Behavioural**; `import_header_parse.rs` adds CLI-level resilience/skip coverage |
| W1 defer | `crates/fsm-cli/tests/deferred_example_e2e.rs` + `crates/fsm-codegen-c/tests/defer_codegen_runs.rs` | **YES** — generate + `gcc` (stderr-must-be-empty, stricter than `-Werror`) + run `defer_test`/`nested_defer_test`; asserts hold-and-replay + ancestor-defer-not-churned, both strategies; CLI test also replays a recorded trace | **Behavioural** |
| Submachine (W2a-d) | `crates/fsm-codegen-c/tests/submachine_codegen_runs.rs` | **YES** — `gcc` (stderr-empty) + run; asserts exact sub-state transitions, sub-completion advancing parent, fresh re-init on re-entry, transition-wins (L370-476, return codes 10-62), both strategies | **Behavioural** (the deepest behavioural test sampled) |

**Conclusion: all six risky waves are behaviourally-real.** No "assert a symbol is present and call it green" pattern (the P0-1 catastrophe class) found in any sampled §5.4 acceptance test.

### Residual symbol-presence assertions (carried-over TD-DEF-1)

- Total `.contains()` assertions across all `crates/*/tests/*.rs`: **190** (≈ the tracked "~119 non-codegen" item plus growth from v1.1 waves; the item's order of magnitude is confirmed, not understated).
- Highest concentrations: `golden_motor.rs` (24), `grammar.rs` (13), `cli_test_with_simulator.rs` (11), `per_machine_strategy_e2e.rs` (10), `branch_hints_codegen.rs` (9).
- **Do any guard a CORRECTNESS property with no behavioural backing?** No, on the files inspected:
  - `golden_motor.rs` — its module header (L11-28) explicitly states these are structural-only and the *behavioural* guarantee is `gcc_compile.rs::motor_compiles_with_gcc_werror`, which emits the same IR, compiles `-Werror`, **runs** it, and asserts IDLE→RUNNING etc. The string checks (file roles, API prototypes present) are legitimate structural assertions, not behaviour stand-ins.
  - `branch_hints_codegen.rs` (W4) — string-matches the `__builtin_expect` macro definition and the `HINTS_LIKELY(g_go())` wrapper placement, **and** has its own `gcc`-compile-run test in the same file (L230, L380). The `.contains()` here verify the *hint is wired to the right transition* (a structural property a runtime test cannot easily observe — `__builtin_expect` has no semantic effect), backed by a behavioural compile-run for correctness. Acceptable.
  - `per_machine_strategy_e2e.rs`, `import_header_*` — string checks sit alongside `.assert().success()` / gcc-run in the same test.
- **Verdict:** the residual `.contains()` are structural/wiring assertions each paired with a behavioural sibling. None is the P0-1 anti-pattern. **Acceptable for a v1.1 tag**, tracked-as-is under TD-DEF-1 (cosmetic test-debt, not a correctness gap).

---

## Findings

### P0 (tag-blocker)

#### SEC-P0-1 — `--import-header` is an unprotected file-read path: G-02 hardening regression

- **Evidence:**
  - `crates/fsm-cli/src/cmd/generate.rs:328-342` `resolve_header_path`: **absolute paths returned verbatim** (`if p.is_absolute() { return p.to_path_buf() }`); relative paths joined to `search_dir`/CWD with **no shape check** (`..`/NUL/drive-prefix not rejected), **no `Path::canonicalize`**, **no workspace-root containment check**.
  - `crates/fsm-cli/src/import_header.rs:137-141` `parse_header_file`: `std::fs::read_to_string(path)` with **no size cap**; the tokenizer then allocates `String::with_capacity(src.len())` several times (L311, L391) before any limit applies.
  - Contrast the DSL `import` path which IS hardened: `crates/fsm-parser/src/import_resolver.rs:151-187` `resolve_import` does shape-check → `canonicalize` (symlink-resolved) → workspace-root prefix containment; `crates/fsm-parser/src/limits.rs:61-69` caps input at 1 MiB / depth 256. The spliced source *is* parser-capped (`generate.rs:142 parse(&src)` enforces `max_input_bytes`), but the **header file itself is read and tokenized unbounded BEFORE** that point.
  - The project's own threat model scopes this: `docs/18-CLI-Specification.md:640-656` ("The compiler runs in CI environments… v1.0 hardens both at parse time"), `docs/00-Decisions-And-Reconciliation.md:767` G-02. §10.1 covers only `import "path"`; `--import-header` is a NEW surface introduced in v1.1 W5 and is NOT covered.
- **Exploit / failure scenario:** a `.fsm` project (or `fsm.toml [generate] import_headers`) is built on shared CI infrastructure. The G-02 threat model is precisely "a `.fsm` source posted to a shared build host cannot exfiltrate arbitrary files". `import "../../etc/shadow"` is rejected by `resolve_import`; `--import-header /etc/shadow` or `import_headers = ["../../../etc/shadow"]` in `fsm.toml` is **read with no restriction**. Two concrete impacts: (a) **path-traversal / arbitrary-file read** — a crafted `fsm.toml` makes the build read any file the CI user can access (content can leak via the skip-note diagnostics that echo source fragments, or via a synthesized-extern name appearing in generated output); (b) **DoS** — `--import-header /dev/zero` or a multi-GB file exhausts RAM in the unbounded `read_to_string` + the `O(n)` capacity allocations, before the parser's 1 MiB cap ever runs. No malformed-header *panic* path exists (parser is resilient — see VERIFIED-ROBUST), so this is exfiltration + DoS, not RCE.
- **Severity:** P0 for the tag. It is a **regression in the explicitly-hardened G-02 security boundary**, on a surface the project's own spec says must be safe in CI. Real-world likelihood is moderate (requires shared-CI + attacker-influenced `fsm.toml`/CLI args), but D.2 mandates a security pass before any release with new surface, and this surface failed it.
- **Recommended action:** before tagging — (1) route every `--import-header` and `fsm.toml import_headers` path through a containment check equivalent to `resolve_import` (shape-reject `..`/absolute/NUL, `canonicalize`, assert workspace-root prefix); decide deliberately whether an *absolute* header path is ever legitimate (it is plausibly a real use case for a vendored HAL outside the project — if so, gate it behind an explicit opt-in rather than silent allow); (2) cap `parse_header_file` reads (reuse the 1 MiB `max_input_bytes` constant; reject larger with a clean exit-3, never an OOM). Add a negative test mirroring `crates/fsm-parser/tests/import_security.rs` for the header path. **v1.1 tag-blocker.**

### P1

#### REL-P1-1 — Submachine-nested-in-composite/parallel emits non-compilable C; deferral mischaracterized (carried from `AUDIT_PHASE_SUBMACHINE_2026_05_15`)

- **Evidence:** `docs/AUDIT_PHASE_SUBMACHINE_2026_05_15.md:47, 99-102` (P1-2). A submachine ref nested in a composite/parallel state emits `Outer_exit_SUB`/`Outer_entry_SUB` calls with **no prototype** in `Outer_impl.h` → `gcc -Werror: implicit declaration`. CHANGELOG/ROADMAP/backlog describe it as "emitted as an inert leaf" — inaccurate (an inert leaf compiles; this does not).
- **Failure scenario:** a user writes the (spec-legal-looking) nested-submachine construct, `fsm generate` succeeds, then their own `-Werror` build fails with a confusing implicit-declaration error. No silent-data-loss (generation visibly produces unbuildable C; it does not miscompile a working-looking binary), and the limitation IS flagged — but described as more graceful than reality (the prose-vs-code drift class the project is sensitized to).
- **Severity:** P1. Not a silent miscompile and an explicitly-deferred path; but it violates G1 ("toolchain output compiles clean under the mandated `-Werror`") for a construct the DSL/analyzer accept far enough to emit code.
- **Recommended action:** either (a) make the analyzer **reject** nested-submachine-ref at analysis with a precise diagnostic (the P1-2 nested-reject wave `02d4ded` did this for one case — extend to composite/parallel), so it fails fast with a clear message instead of emitting unbuildable C; or (b) at minimum correct CHANGELOG/ROADMAP/backlog wording from "inert leaf" to "rejected / unsupported — does not compile" and add a `docs/00 §11` / `TEST_DEBT.md` entry (tracking is currently informal — that audit's P3). **Acceptable-tracked for the tag IF the wording is corrected** (no silent overstatement); the codegen behaviour itself should be fixed in a fast-follow. Recommend (b) as the minimum tag gate, (a) as the v1.1.1 fix.

#### TEST-P1-2 — `cargo test --workspace` cold-cache failure (carried; status to re-confirm at tag time)

- **Evidence:** `docs/AUDIT_PHASE_SUBMACHINE_2026_05_15.md:19, 62, 91-96` (P1-1): `every_fixture_is_idempotent` (fsm-formatter) panicked on a **stale worktree artifact** path, deterministic across 3 runs, **passes after recompile / isolated** (294 passed once recompiled). The current pre-tag brief states the clean quad at `b6f4db8` is green (656 tests, 0 fail) — i.e. this was resolved by the clean rebuild that produced the warm cache.
- **Severity:** P1 if reproducible from a cold target; per the stated baseline it is **resolved** (the quad was clean). Flagged because a CI runner with a cold cache is the exact condition that surfaced it, and G9 CI has never actually run (see below) — so the cold-cache green is locally-attested only.
- **Recommended action:** none if the `b6f4db8` clean-quad result holds (no action permitted by audit constraints — not re-run here). Treat as **acceptable-tracked**; the post-tag CI push (G9) is the real cold-cache confirmation.

### P2

#### REL-P2-1 — `fsm.toml` per-machine `[machine.X]` parsing relies on the `toml` crate's own limits (no explicit DoS cap)

- **Evidence:** `crates/fsm-cli/src/config.rs:114-132` `load`: `std::fs::read_to_string(&candidate)` (unbounded) then `toml::from_str::<FsmToml>` . No explicit size cap; relies on the `toml` crate being non-recursive-unbounded. Errors are correctly propagated as `ConfigError::Parse`/`Io` (no `unwrap`/`panic` on the parse path — verified). `BTreeMap<String, MachineSection>` for `[machine.*]` is deterministic and bounded by input size.
- **Failure scenario:** a multi-GB `fsm.toml` exhausts RAM in `read_to_string` before `toml` parses. Much lower severity than SEC-P0-1: `fsm.toml` is the *user's own project file* discovered by upward walk (not an attacker-pointed path), and `toml` is a mature non-recursive-descent parser (no stack-overflow class). No panic surface.
- **Severity:** P2 — defense-in-depth gap, not a realistic exploit (you do not get a hostile `fsm.toml` without already controlling the project tree, which is a broader compromise). Consistent with how v1.0 left other `read_to_string` call sites uncapped.
- **Recommended action:** for completeness, apply the same `max_input_bytes` cap to `fsm.toml` reads when SEC-P0-1 is fixed (share the helper). **Acceptable-tracked**, not a tag-blocker.

### P3

#### TEST-P3-1 — Submachine-limitation tracking is informal/scattered

- **Evidence:** `docs/AUDIT_PHASE_SUBMACHINE_2026_05_15.md:85` — SUB-FU-2 lives only in CHANGELOG/ROADMAP/backlog; no entry in `docs/00 §11` or `TEST_DEBT.md`.
- **Severity:** P3 process hygiene.
- **Recommended action:** add a single canonical tracked entry (folds into REL-P1-1's recommended action). **Acceptable-tracked.**

---

## G7 / G9 Ship Verdict

### G7 — 39 of 75 diagnostic codes lack formal conformance fixtures: **SHIP (acceptable, explicitly tracked)**

- 36/75 codes have formal `tests/conformance/` fixtures; the remaining 39 are covered by **crate-level negative tests**, not absent (e.g. `crates/fsm-analyzer/src/checks/defer.rs` exercises E0903; the analyzer/parser negative-test suites hit many uncovered codes). The 75-variant `DiagnosticCode` enum is the canonical inventory.
- This is the **identical** posture the v1.0 pre-tag audit (`docs/AUDIT_PRE_TAG_2026_05_15.md:84, 247`) and CHANGELOG explicitly accepted and tracked; ROADMAP names the v1.1 closure task ("Populate the 39 untested diagnostic codes with conformance fixtures"). v1.1 introduced no *new* uncovered behavioural surface that this gap hides (the new W1/W5/W7 diagnostics — E0903 defer, FSM-E0300/W0300, import-header skip notes — are each behaviourally tested in the waves' §5.4 acceptance suites).
- **Verdict: not a tag-blocker.** It is a *coverage-formality* gap (negative tests exist; they just are not in the formal conformance harness), explicitly documented in CHANGELOG + ROADMAP. Conservative reading: this is NOT the P0-1 class — these codes ARE tested, just not in the canonical fixture suite.

### G9 — CI matrix YAML exists but has never actually run: **SHIP (acceptable, explicitly tracked) with a hard post-tag action**

- `.github/workflows/ci.yml` is well-formed (ubuntu/macos/windows × fmt/clippy/build/test). The repo is **~40 commits ahead of `origin/main`** — CI has never executed against any v1.1 commit. Locally-equivalent checks (the clean quad at `b6f4db8`) are green per the stated baseline.
- This is the **same caveat** the v1.0 pre-tag audit accepted (`docs/AUDIT_PRE_TAG_2026_05_15.md:249, 403`): "configured, not exercised". The user controls the push.
- **Verdict: not a tag-blocker IN ITSELF** — but it is a real residual risk for *platform-specific* regressions (Windows path handling is the obvious one, and SEC-P0-1's fix touches path canonicalization, which is the single most platform-divergent area in the codebase). **Hard post-tag action:** push the tag + unpushed commits immediately to trigger the matrix; keep a v1.1.1 patch lane ready. The cold-cache green (TEST-P1-2) is also only truly confirmed by this run. Acceptable to tag *before* CI runs only because the local quad is green and the patch lane exists — but G9 must be exercised the moment the tag is pushed, not deferred.

---

## Verified-Robust (positive evidence)

- **Panic surface in v1.1-touched modules is essentially nil.** Production (non-`#[cfg(test)]`) code in `dispatch_switch.rs`, `dispatch_table.rs`, `emit/submachine.rs`, `emit/defer.rs`, `cmd/generate.rs`, `analyzer/checks/defer.rs` has **ZERO** `unwrap`/`expect`/`panic!`/`unreachable!`. The only such site in `import_header.rs` production code is L913 `s[after..].find('(').unwrap()` — **provably unreachable**: L910-911 already returns unless `rest` (= `s[after..].trim_start()`) starts with `(`, so the `find` is guaranteed `Some`. `analyzer/checks/submachine.rs:248` `get_key_value(...).unwrap()` is guarded by L246 `if adjacency.contains_key(...)` immediately prior — safe. No panic reachable from valid-but-unusual user input (G1 / PB1-class clean).
- **`#![forbid(unsafe_code)]` invariant holds across all 9 crates** (8 `src/lib.rs` + `fsm-cli/src/main.rs:8`). Verified by exhaustive grep — zero crates missing it.
- **W7-FU-2 fix is complete and single-sourced.** `fsm_ir::DEFAULT_TRANSITION_PRIORITY = 100` (`crates/fsm-ir/src/model.rs:30`); **all four** lowering arms use it (`crates/fsm-analyzer/src/lower/state.rs:444, 480, 517, 552` — `.unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY))`), no stray `.unwrap_or(0)` remains on the transition-priority path. The behavioural e2e + the lowering unit pin + the byte-identity guard are mutually reinforcing.
- **Determinism property holds post-W7-FU-2.** `crates/fsm-analyzer/tests/lower_split_byte_identity.rs` pins a SHA-256 of canonical `to_json(ir)` for all four examples. The W7-FU-2 fingerprint bump (L133-156) is documented as a **deliberate, reviewed semantic change** with a proof-of-delta (pre-fix binary diffed: the ONLY changed lines are transition `"priority": 0 → 100` × known counts + derived `sourceHash`; `RegionObject.priority` correctly stays 0; zero traversal/id/loc/order perturbation — the exact AD-3 regression class the guard exists for, verified clean). The guard is **sound post-W7-FU-2**: same `.fsm` + same compiler → byte-identical IR/C is enforced; the constant update was the correct, non-reflexive response (not a "make it green" hash bump). `config.rs` uses `BTreeMap` (not `HashMap`) for `[machine.*]` so per-machine overrides cannot perturb byte-identity.
- **`import_header` parser is genuinely resilient.** Hand-rolled tokenizer (no shell-out / libclang — correct per Doc 23 dependency rules); 25+ inline unit tests + CLI-level `import_header_parse.rs` incl. two deliberately hostile "gnarly header" tests (nested macros, fn-pointer typedefs, bitfields, string/char literals containing `;`/`)`, attributes) — asserts exactly the one good function survives and **nothing panics**. Resilience-over-completeness is enforced (opaque-signature functions skipped with precise notes, never emitted with silently-dropped params — the OPAQUE-BUG-1 fix `8375fb9`). The defect is *path/size handling around* the parser (SEC-P0-1), not the parser logic itself.
- **Submachine codegen is the deepest-tested wave.** `submachine_codegen_runs.rs` asserts sub-state delegation, sub-completion propagating to parent (synthetic completion), fresh re-init on re-entry, and transition-wins over delegation — through a gcc-compiled, executed binary with 50+ distinct return-code assertions, both dispatch strategies. This is the opposite of the P0-1 anti-pattern.
- **`fsm.toml` config loader is panic-free** on the parse path: malformed TOML → `ConfigError::Parse` propagated to exit-4, not a panic (`config.rs:114-132`; covered by `parse_error_propagates` test).

---

## Summary of recommended pre-tag actions

1. **SEC-P0-1 (blocker):** harden `--import-header` / `fsm.toml import_headers` path resolution (containment + size cap) — small, localized.
2. **REL-P1-1 (min. gate):** correct the "inert leaf" wording for nested-submachine to "unsupported / does not compile" + add one canonical tracked entry; fix codegen-rejection in v1.1.1.
3. Tag, then **immediately push to exercise G9 CI** (P1-2 cold-cache + Windows path handling are only confirmed by this).
4. P2-1 / P3-1 fold into the P0/P1 fixes; otherwise acceptable-tracked.

G7 + G9 are acceptable, explicitly-tracked partials (unchanged from v1.0 posture) — neither blocks the tag.
