# Pre-Tag Audit — FSM Studio v1.1 (Correctness / Traceability / Scope-Honesty)

- **Document ID:** AUDIT_PRE_TAG_v1_1_CORRECTNESS_2026-05-15
- **Date:** 2026-05-15
- **Auditor lens:** Read-only pre-tag auditor. Correctness, Doc 00 §11 traceability, scope-honesty. The cardinal sin on this project is OVERSTATEMENT (P0-1 / submachine class).
- **Scope:** v1.1 surface = all commits since `v1.0.0` (`b480003`), HEAD = `b6f4db8` (main). Doc 00 §11.19–§11.27, CHANGELOG `[Unreleased]`, ROADMAP, dispatch/priority/submachine/defer code + acceptance tests, conformance.
- **Method:** static read of every cited file:line + commit; one warm-cache `cargo test --workspace` spot-confirm (did NOT rebuild from scratch — warm shared `CARGO_TARGET_DIR` preserved per brief).

---

## VERDICT

**TAG-READY: NO** — **Open P0 count: 0**

There are **zero P0 correctness defects** and **zero overstatement findings in the §11.19–27 traceability rows or the CHANGELOG `[Unreleased]` "Added/Fixed" claims** — the historically dangerous surface is clean and, notably, *conservatively* worded (the submachine 3rd-correction discipline held). The v1.1 feature claims are real and code-backed.

The tag is gated **not by a code defect but by a process/release-gate finding (P1-A)**: the warm shared `CARGO_TARGET_DIR` demonstrably serves **stale cross-worktree test binaries with baked dead `-wt-` absolute paths** (reproduced live during this audit — see P1-A). This is exactly the §11.22 / §11.1-hardened hazard, *fully realized*. The brief's "656 tests pass at `b6f4db8`" is therefore only authoritative **if it was the COLD-from-source quad §11.22 mandates**; a warm run is non-authoritative *by the project's own rule*, and I could not (and per constraints must not) independently re-run cold. **Recommendation: do not tag until the §11.22 cold-from-source quad is run and its console output is captured into the release record.** Plus two doc-staleness P2s in the *historical* v1.0 sections (not the v1.1 surface).

---

## Findings

### P0 — none

No silent-miscompile, regression, or overstatement of the P0-1 class found in the v1.1 surface.

---

### P1

#### P1-A · Warm `CARGO_TARGET_DIR` serves stale cross-worktree test binaries — §11.22 hazard reproduced live; release gate not yet satisfied by code evidence I can see

- **Evidence:** During the single permitted warm `cargo test --workspace` (HEAD `b6f4db8`), `fsm-formatter` `every_fixture_is_idempotent` FAILED:
  `read_dir "/root/dev/embeded-fsm-sdk-wt-w7fu2/crates/fsm-formatter/tests/fixtures": No such file or directory (os error 2)` — `crates/fsm-formatter/tests/idempotency.rs:23` uses `env!("CARGO_MANIFEST_DIR")` (compile-time-baked abs path). The fixtures **exist** at the real path `crates/fsm-formatter/tests/fixtures/` (24+ `.fsm`). The `-wt-w7fu2` worktree is **deleted** (`ls /root/dev/embeded-fsm-sdk-wt-*` → no matches). `strings` on the cached binary shows the dead path baked **2×**. Spot-check of the highest-risk v1.1 test binaries: `guard_disambiguated_dispatch_e2e` (11 baked `-wt-` refs), `default_transition_priority_e2e` (6), `submachine_codegen_runs` (5), `nested_submachine_rejected` (10) — and each test has **two cached hashes** (one stale-worktree-built, one main-built) in `…-target/debug/deps/`.
- **Why it matters:** This is the §11.22 / §11.1-hardened risk *exactly* ("warm shared-`CARGO_TARGET_DIR` can serve stale cross-worktree test binaries (baked `-wt-` abs paths)"; "a warm post-merge quad is necessary-not-sufficient"). It is **NOT a code regression** — W7-FU-1/FU-2/submachine logic is correct (verified statically, below) and the test binaries that ran before the abort passed; the formatter failure is a dead-path artifact in a binary compiled inside a now-gone worktree, not a logic defect. But it means a *warm* "656 green" is not release evidence per the project's own decided rule. The release record must contain a **cold from-source** quad's output.
- **Recommended action:** Before `v1.1.0`: run the §11.22 cold-from-source full-workspace quad (build/test/clippy/fmt) — the ROADMAP §47 already prescribes "cold-from-source full-workspace quad … keeping content-addressed registry deps" — and **persist its console tail into the release/CHANGELOG record** so the gate has captured positive evidence, not a verbal "it was run". Treat the warm run (this audit's, or any) as non-authoritative. No code change required.

---

### P2

#### P2-1 · ROADMAP "v1.0 (current — tagging in flight)" section is stale and self-contradicting vs. the corrected submachine reality

- **Evidence:** `docs/ROADMAP.md:11` `## v1.0 (current — tagging in flight)` — but `v1.0.0` is **already tagged** (`git tag` → `v1.0.0` at `b480003`). `docs/ROADMAP.md:16` "FSM-Lang DSL (full UML statechart syntax …)" and `:27` "Submachine codegen silent no-op (IR + analyzer landed; codegen v1.1)" — both contradict the CHANGELOG `[Unreleased]` correction (line 12: submachine was *entirely absent* in v1.0, IR/analyzer claims were "aspirational prose"). The ROADMAP **v1.1** section (lines 37–63) is, by contrast, precise and carries the 3rd-correction discipline correctly.
- **Why it matters:** This is the *exact wording pattern* (false "IR + analyzer landed") that caused the P0-1 / submachine catastrophe — surviving here only in the frozen historical section. It is not a v1.1-surface claim and the authoritative CHANGELOG correction supersedes it, so it is P2 not P0; but a pre-tag doc should not still assert the falsified sentence anywhere, and "tagging in flight" for an already-tagged release is misleading.
- **Recommended action:** In the v1.0 ROADMAP section, mark it shipped/tagged and replace the line-16/line-27 wording with the corrected scope ("UML statecharts excluding submachines" — submachine landed v1.1), or add a one-line pointer to the CHANGELOG `[Unreleased]` correction. (Doc edit; out of this read-only audit's remit.)

#### P2-2 · CHANGELOG `[1.0.0] → Deferred to v1.1+` still lists the falsified "Submachine codegen (IR + analyzer present; codegen silent no-op …)"

- **Evidence:** `CHANGELOG.md:106-107` "Submachine codegen (IR + analyzer present; codegen silent no-op until v1.0.1+)". The `[Unreleased]` "Corrected" block (`CHANGELOG.md:12`) explicitly refutes "IR + analyzer present" as aspirational prose.
- **Why it matters:** Same overstatement-class sentence, in the frozen v1.0.0 entry. The immediately-preceding `[Unreleased]` correction does neutralize it for a careful reader (Keep-a-Changelog entries are immutable history), so this is P2 documentation-hygiene, not a live false claim — but it is the precise phrasing the project keeps re-learning to distrust.
- **Recommended action:** Acceptable to leave as immutable history *iff* the `[Unreleased]` correction stays directly above it (it does). Optionally add an inline "(see [Unreleased] correction — this was overstated)" footnote on line 106. Low priority; flagged for honesty-completeness.

---

### P3

#### P3-1 · Simulator `select_transitions` sort comparator has a redundant `.then_with(|| Ordering::Equal)` no-op

- **Evidence:** `crates/fsm-simulator/src/interpreter.rs:1177-1181` — `candidates.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| std::cmp::Ordering::Equal))`.
- **Why it matters:** **Not a correctness bug.** Rust `sort_by` is stable, so equal-priority candidates already retain `candidates`-Vec insertion order (= `node.transitions` document order) — the spec's `(priority, document_order)` tiebreak (Doc 08 §4.2) holds, and the §11.26(b) "sim is the spec-correct oracle" claim is **verified true**. The trailing `.then_with(Equal)` is dead expression noise that could mislead a future reader into thinking document-order is *explicitly* encoded (it is implicit via stability). Cosmetic / zero-legacy nit.
- **Recommended action:** Drop the `.then_with(...)` or replace with an explicit doc-order key + comment that stability is load-bearing. Non-blocking.

---

## Doc 00 §11 traceability table (v1.1 rows §11.19–§11.27)

| Row | Claim (abridged) | Commit/branch exists? | Code present & matches? | Overstated? |
|---|---|---|---|---|
| §11.19 | Submachine epic 4 waves; nested-in-composite/parallel REJECTED at analysis via FSM-E0502 (reused w/ overriding msg) | ✅ `7a69612`/`2384608`/`8b66dc2`/`af8c300` + `02d4ded` all in `git log` | ✅ Lexer `KwIs`/`KwSubmachine` (`lexer.rs:689,706`); reject wired `checks/submachine.rs:120-132` `Diagnostic::new(E0502,…).with_message(…)`; nesting detector `util.rs:127-144` correct; `examples/submachine/` present; test `nested_submachine_rejected.rs` | **No.** FSM-E0502-reuse is *explicitly documented* in the row + module header as a deliberate reuse-with-override (no reserved code; honest). Conservative wording. |
| §11.20 | Leading comment before `language` no longer panics rowan builder | ✅ `3017c1c` | ✅ (PARSE-BUG-1 fix; `comment_preservation` tests green in warm run) | No |
| §11.21 | `likely`/`rare` contextual kw → portable `__builtin_expect` + fallback + opt-out; sim ignores | ✅ `300d1e4` | ✅ `header.rs:70-88` emits `<PFX>_LIKELY/_UNLIKELY` under `__GNUC__`/`__clang__` w/ scalar fallback; IR `model.rs` hint; wired in `transition.rs`/`dispatch_table.rs` | No (sim≡codegen layout-only claim consistent w/ code) |
| §11.22 | Tags require COLD from-source quad; warm shared target serves stale `-wt-` binaries | ✅ `e2e9294` (§11.1 hardened) | ✅ **And empirically CONFIRMED LIVE this audit — see P1-A.** The decision is correct and load-bearing. | No — *under*-stated if anything; the hazard is real and reproduced |
| §11.23 | OPAQUE-BUG-1 modelled end-to-end; `lower_type_ref_node(parent)` resolves TYPE_REF or OPAQUE_TYPE_REF; all 4 call sites | ✅ `8375fb9` / `phase2.14/ob1-opaque-extern` | ✅ `lower/machine.rs:462` `lower_type_ref_node`; call sites: context field `:246`, extern param `:295`, extern return `:309`, as-cast `expr.rs:365` (4 sites, as claimed) | No |
| §11.24 | W6 integration examples are real builds (workspace-isolation, ffi shim, ASCII-only, platformio, cmake-via-apt); no codegen defect surfaced | ✅ `e74888a` | ✅ `examples/integration/{make,cmake,cargo-rust,platformio}/` exist incl. `cargo-rust/Cargo.toml`; `docs/25-Integration-Guide.md` present | No (claims "no codegen defect surfaced", not "proves codegen perfect" — appropriately bounded) |
| §11.25 | Per-machine dispatch override; single resolution point `strategy_for`; Auto per-unit; absent=WARN; sub inherits parent strategy; invalid=exit-4; empty=byte-identical; surfaced (not introduced) W7-FU-1 | ✅ `e223d83` / `phase2.16/w7-per-machine-strategy` | ✅ `config.rs:43` `machine_strategy_overrides: BTreeMap`, `:70` `strategy_for`; `emit/mod.rs:139` consulted, `:169-172` sub inherits `effective`; absent-machine WARN `cmd/generate.rs:241-252`; invalid→exit-4 `cli/src/config.rs:47`; W7-FU-1 honestly flagged as surfaced-not-introduced | No — and the "surfaced NOT introduced" framing is the honest-surface standard, correctly applied |
| §11.26 | W7-FU-1 fixed both strategies, codegen-only; sim independently spec-verified as oracle; switch=one-case-per-event do/while(0) guard chain; table=`_row_guard_enabled`+`continue`; §5.4 RUN-asserted; tripwire→positive; CGEN-004; priority-default discrepancy surfaced-not-fixed | ✅ `d957d12` / `phase2.17/w7fu1-guard-disambiguated-dispatch` | ✅ switch `dispatch_switch.rs:185-203` stable sort + group-by-event + `emit_event_case` do/while(0) chain (`:283-293`); table `dispatch_table.rs:295-371` `_row_guard_enabled` + `:397-414` `select_for_region` `continue` past disabled rows, pre-sorted `(source,priority,row_idx)` `:177`; sim oracle verified (`interpreter.rs:1126-1182`, stable sort — see P3-1); e2e `guard_disambiguated_dispatch_e2e.rs` real gcc-Werror+RUN+sim==codegen, 3 test fns; conformance CGEN-004 registered (`MANIFEST.json:200-206`, `normative:true`, golden, real `source.fsm`) | **No.** Notably, §11.26(e) *itself surfaced* the priority-default bug honestly rather than silently fixing — exemplary scope-honesty |
| §11.27 | W7-FU-2: default transition priority reconciled 100 (impl was bug); single source `fsm_ir::DEFAULT_TRANSITION_PRIORITY=100`; 4 lower arms + serde default + schema + doc; RegionObject.priority distinct; regression test corrected | ✅ `41b9e46` / `phase2.18/w7fu2-default-transition-priority` | ✅ `model.rs:30` `const DEFAULT_TRANSITION_PRIORITY: u16 = 100`; 4 arms `lower/state.rs:444,480,517,552` `.unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY))`; serde `model.rs:505`; schema `model.json:487` `"default":100`; `RegionObject.priority` distinct (`model.json:774`, no default); regression test `lowering.rs` asserts `DEFAULT_TRANSITION_PRIORITY` (`:195`); e2e `default_transition_priority_e2e.rs` hand-built IR + gcc + RUN | No — the "8 sites" / "impl was the bug, docs win" framing is accurate; `priority` is `required` so W0 schema gate stays green (verified `model.json:468` lists `priority` in `required`) |

**Conclusion of traceability check:** all 9 v1.1 rows — commit exists ✅, code present & matching ✅, **0 overstated**. The class of defect that caused the P0-1 catastrophe (aspirational prose never reconciled to code) is **absent** from the v1.1 §11 surface. Where a residual defect exists (nested submachine; priority default pre-FU-2), it was *flagged honestly, not silently claimed fixed* (§11.19, §11.23, §11.26(e)).

---

## What I verified clean (positive evidence for the gate)

1. **W7-FU-1 dispatch correctness (highest-risk):** switch strategy collapses same-event guarded transitions to ONE `case` with a `do { if(!guard) break; …; return true; } while(0)` chain over a STABLE priority-sorted, document-order-preserving candidate list (`dispatch_switch.rs:185-293`). Table strategy: rows pre-sorted `(source, priority, doc-order)` (`:177`), `select_for_region` `continue`s past guard-disabled rows via `_row_guard_enabled` (`:397-414`), returning the first guard-enabled = spec winner. **Both match Doc 08 §4.1/§4.2 and the simulator oracle.** No duplicate-`case`, no silent event-drop. Acceptance is a real gcc-`-Werror` compile+link+**RUN** + sim==codegen test (`guard_disambiguated_dispatch_e2e.rs`), and the tripwire was genuinely converted to a positive test.
2. **Simulator is the spec-correct oracle (§11.26(b)):** `interpreter.rs:1126-1182` filters guard into `candidates` *before* a stable `sort_by(priority)` that preserves document order — exactly `min(candidates, key=(priority, document_order))`. Verified independently against the spec, not merely against codegen. (Cosmetic no-op noted P3-1; no correctness impact.)
3. **W7-FU-2 priority default (class-of-issues):** single source of truth `DEFAULT_TRANSITION_PRIORITY=100`, all 4 `lower_*` arms, serde default, JSON-schema annotation, IR doc-comment, regression test all consistent and code-present. `RegionObject.priority` correctly kept as a *distinct* field (region dispatch order, no default) — the §11.27 distinction is real. W0 IR-schema gate unaffected (`priority` is `required`). Behavioural e2e is a hand-built-IR gcc-RUN test asserting 50 < 100 fires + sim≡codegen.
4. **Submachine scope statement is EXACTLY true in code:** lexer tokens real, grammar/CST/AST waves merged, analyzer **rejects** nested-in-composite/parallel refs with a wired diagnostic (correct ancestor-walk detector), codegen `emit/submachine.rs` present, `examples/submachine/` exists with `.trace`. The "full UML for top-level `state X is Sub`; nested rejected at analysis" claim matches the code precisely; the wording is conservative (3rd-correction discipline visibly held — no re-overstatement).
5. **`defer EVENT` runtime is behaviourally real:** FIFO replay via `queue_push_front` back-to-front (`emit/defer.rs:134-161`, Doc 08 §10.2/§10.3); simulator `push_back` (`interpreter.rs:252,312`). The old symbol-presence `defer_codegen_unreachable.rs` was **replaced** by `defer_codegen_runs.rs` — a real gcc-`-Werror` compile+link+RUN+assert (explicitly documented as such in its header). Removes the v1.0 FSM-E0903 limitation as claimed.
6. **CHANGELOG `[Unreleased]` "Added/Fixed":** every entry (submachine, defer, IR-schema gate, shared target, TD-BUG-1, arch-debt, likely/rare, --import-header, integration examples, per-machine strategy, W7-FU-1, W7-FU-2, OPAQUE-BUG-1, PARSE-BUG-1) is backed by present code AND, for the behavioural ones, a gcc-compile-and-RUN test (not symbol-presence). The v1.0.0-scope "Corrected" block is honest and accurate.
7. **Conformance:** MANIFEST.json has exactly **25** fixtures (PARSE/VAL/SEM ×{pos,neg}×3 = 18, CGEN-001..004 = 4, FMT-001..003 = 3). Matches CHANGELOG/Doc00 "≥25". **CGEN-004 is real and behavioural** — registered `normative:true`, `kind:golden`, with a genuine `source.fsm` + `expected/`, tagged `W7-FU-1`/`P0-1`.
8. **W7 secondary claims:** absent-machine = WARN (not hard error) emitted once post-generate (`cmd/generate.rs:241-252`); invalid strategy = exit-4 (`cli/src/config.rs:47`); submachine templates inherit parent's effective strategy (`emit/mod.rs:169-172`). All code-present and matching §11.25.
9. **All 20 spot-checked cited commits/branches exist** in `git log` / `git branch` (W2a-d, P1-2, PB1, W4, W5/OB1, W6, W7, W7-FU-1, W7-FU-2, W0, W3, TD1, defer, INFRA).

---

*End of audit. Read-only: no source/doc/test modified; no git mutation; warm `CARGO_TARGET_DIR` preserved (one warm spot-confirm run only, which itself surfaced P1-A). The single blocking item is process (§11.22 cold quad evidence), not code.*
