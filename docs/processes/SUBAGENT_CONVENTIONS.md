# Subagent Conventions

**Audience:** TL/PM (the orchestrator) writing wave briefs + the subagents executing them.

**Status:** Normative. Every wave brief MUST conform.

**Document ID:** FSM-PROC-SUBAGENT
**Version:** 1.1.0

This document codifies how implementation work gets done by background agents on FSM Studio. It exists because: a project of this size, run autonomously, can only stay coherent if every wave follows the same shape, leaves the same evidence, and respects the same boundaries. Deviations from these rules cause regressions, lost work, and audit findings that should have been prevented.

**v1.1.0 changelog:** Added the behavioural-acceptance mandate (§5.4 — the rule that would have caught the P0-1 non-functional-lowering catastrophe ~8 waves earlier). Hardened orchestrator duties (§11 — mandatory post-merge quad, liveness heartbeat, phase-boundary audits, checkpoint tags). Derived from the 2026-05-15 retrospective (plan PD-2/PD-4/PD-5/PD-6).

---

## 1. Wave brief template

Every brief must have these sections, in this order:

1. **Role framing** — one sentence: "You are a senior X engineer fixing Y." Sets the quality bar.
2. **Repo root** — absolute path. Always `/root/dev/embeded-fsm-sdk`.
3. **Git setup** — exact commands to run first. Worktree or no-worktree decision per §3.
4. **Disk budget** — current `df -h /` free space at dispatch time + the rule "dev builds only, no `--release` unless required".
5. **Read first** — strict list of input docs by path + §section. NEVER "read the whole repo".
6. **Scope** — what the wave fixes/implements, with finding IDs (e.g., `Per docs/AUDIT_…md §P0-X`).
7. **The fix** — concrete steps, file-level, with line-number cites where stable.
8. **Tests** — specific test files to create or update. Behavioral assertions, never just symbol-presence. Tests must FAIL on old behavior + PASS on new.
9. **Verification quad** — `cargo build/test/clippy/fmt --workspace`, all green; manual smoke if user-visible.
10. **Scope boundary** — explicit "DO NOT touch X, Y, Z" list. Names crates + paths another wave owns.
11. **Commit message** — full proposed message inside a triple-backtick block. Agent may polish but structure must match.
12. **Completion report** — bullet list of what the agent must report back: branch, LOC delta, test count delta, judgment calls, verify-quad confirmation.

A brief missing any of these is incomplete and risks scope creep.

---

## 2. Commit message format

```
Imperative subject ≤72 chars summarizing the WHAT

Body explains the WHY: what the bug or gap was, why the chosen approach,
any alternative considered. Wrap at 80 columns. Multiple paragraphs OK.

Per docs/<audit-or-spec>.md §<section> citing the finding ID.

Verified: cargo build/test/clippy/fmt all green workspace-wide.
```

- Subject in imperative mood ("Fix P0-1", "Implement fsm-lexer", "Refactor LoweringCtx")
- Subject ≤72 chars; no trailing period
- Blank line between subject and body
- Body addresses WHY (the WHAT lives in the code; the WHY decays from PR descriptions)
- Cite finding ID(s) when fixing audit items
- Footer "Verified:" line is the agent's attestation that the verification quad passed

No Co-Authored-By trailers unless explicitly briefed.

---

## 3. Worktree isolation rules

**Default: each writer wave gets its own worktree.**

```bash
cd /root/dev/embeded-fsm-sdk
git worktree add /root/dev/embeded-fsm-sdk-wt-<NAME> phase<N>/<DESCRIPTOR>
cd /root/dev/embeded-fsm-sdk-wt-<NAME>
# do work
git commit
# orchestrator merges + removes worktree
```

Exception — work on main checkout directly via a branch is acceptable when:
- Only one writer is active in the workspace
- The wave is small (single-crate, ~200 LOC change)
- The wave is mostly doc edits (markdown only)

The orchestrator pays for clean isolation. Never two writers in the same checkout.

**Naming convention:** `phase<N>.<sub>/<descriptor>`. Examples: `phase1.10/p0-1-lowering`, `phase3/doc-reconciliation`, `phaseR2/ir-visitor-adoption`. Branch name + worktree dir name share the descriptor.

---

## 4. Verification quad (non-negotiable)

Before any commit, the wave must run:

```bash
cargo build --workspace                       # → exit 0
cargo test --workspace                        # → all green
cargo clippy --workspace --all-targets -- -D warnings   # → exit 0
cargo fmt --check --all                       # → exit 0
```

If any fails, the wave does NOT commit; it fixes the issue first. If it cannot fix, it reports the failure in the completion report (the orchestrator decides whether to extend scope or abort).

`--release` is forbidden unless a specific test requires it. Dev builds only by default.

**Pre-tag reliability checklist (in addition to the per-wave quad):** an SCA (software-composition-analysis) scan — `cargo audit` on a *separate recent stable toolchain* (the pinned 1.75 cargo-audit cannot parse the CVSS-4.0 advisory DB) — is part of the pre-tag gate, wired as the distinct `sca` job in `.github/workflows/ci.yml`. It must report 0 vulnerabilities before a `vX.Y.0` tag (Doc 00 §11.48/§11.42).

For waves touching codegen, an additional manual smoke is recommended when feasible:

```bash
cargo run -p fsm-cli -- generate --target c99 examples/motor/motor.fsm --out /tmp/smoke
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror -c /tmp/smoke/*.c
```

---

## 5. Test discipline

### 5.1 Regression coverage rule

Every fix wave must add at least one test that:
- **FAILS** on `main` before the fix
- **PASSES** after the fix

This is the proof of correctness. Without it, the fix is unverified.

Example: P0-1 added `motor_emits_guards_and_actions.rs` asserting `can_start`, `set_speed(100)`, etc. appear in generated Motor.c. The test was authored against the FIXED behavior; without the fix it produced "set_speed not found in 0-byte buffer" failures.

### 5.2 Forbidden test patterns

- **Symbol-presence-only assertions** — `text.contains("Motor_init")` proves the function name appears, NOT that it does anything correctly. Use behavioral assertions: compile the generated code with gcc -Werror and execute it.
- **Empty `expected` blocks in trace files** — `expected: []` lets any simulator output pass. Always populate expected, even if hand-authoring is tedious.
- **Tests with `#[ignore]` but no comment explaining why** — either fix it or document the deferred work.
- **Sleeps for synchronization** — never. This is a compile-time tool; no real-time dependencies.
- **`unwrap()` in test setup that hides errors** — the test is meaningful only if setup succeeds; let it panic with a clear context.

### 5.3 Test naming

Test function names should answer "what scenario does this prove":
- ✅ `motor_emits_guards_and_actions`
- ✅ `parallel_completion_does_not_fire_when_one_region_unfinished`
- ✅ `defer_event_rejected_at_analysis_with_e0903`
- ❌ `test_motor` / `it_works` / `basic_test`

### 5.4 Behavioural-acceptance mandate (v1.1.0 — the P0-1 lesson)

**Any wave that touches the pipeline's behaviour (lowering, codegen, simulator, or a new language feature) MUST ship an end-to-end test that compiles the generated C with `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, RUNS the resulting binary, and ASSERTS observable behaviour** — state transitions taken, context fields mutated, externs called, traces matched.

Why this rule exists: the P0-1 catastrophe. The AST→IR lowering shipped hard-coding `guard:None, actions:[]` for every transition. **413 tests were green** because they asserted symbol *presence* (`text.contains("Motor_init")`), not behaviour. The product was behaviourally empty — every guard ignored, every action dropped — and nothing caught it for ~8 waves until the first audit. That cost ~10 rework waves.

Concretely, a behaviour-touching wave's acceptance test must do all four:
1. `fsm generate` real `.fsm` source → C
2. `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` compile + link with a host HAL stub
3. **Execute** the binary driving an event sequence
4. Assert the final state / context / extern-call-trace is what the DSL semantics *mean* (cross-checked against the spec, not just against the simulator — the simulator can share the same bug)

A wave that only adds unit tests of internal structure for a behaviour change is **incomplete** and must not be merged. The orchestrator rejects it back for an acceptance test.

Symbol-presence assertions are acceptable ONLY as a cheap secondary check alongside a real behavioural test, never as the sole guard for a behaviour. The 128 legacy symbol-presence assertions are tracked for paydown in v1.1-W0.

---

## 6. Scope boundary discipline

Every brief includes an explicit "DO NOT touch X" section. Reasons:
- Parallel waves working on other crates
- Files owned by deferred work (e.g., DO NOT modify `docs/AUDIT_*.md` — frozen history)
- Workspace-level files (Cargo.toml, rust-toolchain.toml) require dedicated waves
- Anything not in the wave's stated scope

Agents respect this. If they discover they NEED to touch an out-of-scope file, they report in the completion notes and the orchestrator extends scope explicitly — they do NOT silently expand.

---

## 7. Disk hygiene (v1.1.0 — warm-cache policy)

**The shared `CARGO_TARGET_DIR` (`.cargo/config.toml` → `/root/dev/embeded-fsm-sdk-target`) is a bounded, reusable cache — NOT runaway growth.** `deps/` is fingerprint-keyed and reused across builds (~5-6G steady); `incremental/` (~1G) regenerates cheaply. Steady-state total ~7-9G, slow creep to ~10-12G over many waves (stale dep-version artifacts + per-test binaries) — never disk-runaway. The earlier per-wave `cargo clean` was a symptom of the now-fixed per-worktree target *multiplication*, NOT a real need. Cleaning a warm cache forces a slow cold rebuild (the 20-30 min/wave anti-pattern).

**Policy — keep the cache warm:**
- **Do NOT `cargo clean` between waves.** Warm builds are minutes; cold are 20-30 min. This is the single biggest wave-speed lever.
- Agents check `df -h /` before starting. If disk is tight *during* a wave, the FIRST reclaim is `rm -rf <shared-target>/debug/incremental` (~1G, regenerates with a tiny penalty — NOT a cold rebuild). Never touch another agent's worktree.
- Full `cargo clean` is a LAST RESORT, only when disk is genuinely starved (<~2G free) — it forfeits the warm cache.
- Never `cargo build --release` to save space.
- After commit, agents leave the worktree in place (orchestrator cleans on merge).

The orchestrator:
- Does NOT pre-clean before waves (keeps the cache warm). Removes merged worktrees immediately (worktree source is tiny; the cache is shared+external so removal doesn't lose it).
- Monitors `df -h /` between waves; if margin trends below the 5% line, does the incremental-only reclaim first, full-clean only if still starved.
- Surfaces a hard disk ceiling to the user as a decision (per [[infra-constraint-escalation]]) rather than grinding clean/rebuild loops.

---

## 8. Judgment call disclosure

Every completion report includes a "Judgment calls" section listing every non-obvious decision the agent made. Format:

> 1. **Subject** — what the choice was, what the alternatives were, why this one.
> 2. **Subject** — ...

These accumulate in `docs/00-Decisions-And-Reconciliation.md §11` (the implementation-time decisions table) so future maintainers can find them.

---

## 9. Brief size + agent context

- Briefs typically run 300-800 words. Longer is fine for major waves (codegen).
- Don't dump full spec docs into the brief — name the section to read and let the agent navigate (saves context for actual work).
- Don't add "general best practices" lists — they live HERE in conventions.
- Each input doc cited gets its expected purpose ("§X — the grammar your output must re-parse against").

The agent's context window is precious. Briefs maximize useful information density.

---

## 10. Anti-patterns (do not do)

| Anti-pattern | Why it hurts | What to do instead |
|---|---|---|
| Skipping the verification quad on a "small" change | Quality regressions accumulate silently | Always run the quad |
| Two writer agents in the same checkout | Index races, lost work | Worktrees |
| Updating audit docs to "fix" findings | Audits are evidence; rewriting hides reality | Fix the code; reference the audit ID in commit |
| `git push` without explicit user approval | User controls remote | Local tags + branches only |
| `unwrap()` in user-input code paths | Process aborts; bad UX | Result-propagated errors with miette rendering |
| Commenting out tests to make CI green | Hides the regression | Fix the test or the code; never disable |
| "Refactor" waves that change behavior | Hidden semantic shifts | Behavioral tests prove pre/post identity |
| Auto-accepting insta snapshots without review | Locks in wrong output | Read each .snap.new; accept only verified-correct |
| Editing fsm.toml or Cargo.toml in a scope-creep way | Workspace surface changes need dedicated waves | Defer to a workspace-config wave |
| Touching another agent's worktree | Cross-contamination + race | NEVER. Each agent owns its own worktree. |
| Refactoring to hit a number (split/merge code to satisfy a metric — CC ≤ N, a dedup count, "no two impls") when the restructure worsens readability/clarity | Optimises a proxy at the cost of the real goal; a contorted "deduplicated" or "low-CC" form is harder to read and maintain than the honest violation | Refactor only when a clean behaviour-safe restructuring genuinely improves clarity. If none exists, **leave the violation in place and explain why** in a code comment + the §11/commit record (e.g. DRIFT-2 left the LSP `LineIndex` as a structurally-different 3rd impl, not folded into the converged byte/scalar core, because forcing it in would worsen clarity for zero behaviour gain — Doc 00 §11.44/§11.48). |

---

## 11. Orchestrator (TL/PM) responsibilities

The orchestrator (the conversation-level Claude that dispatches waves) is responsible for:

- Maintaining the wave queue + dependency ordering
- Sizing each wave so it fits one agent's context comfortably (split if necessary)
- Tracking disk margin between dispatches
- Removing worktrees post-merge
- Updating backlog + memory after each merge
- Updating CHANGELOG.md
- Surfacing learned lessons as new `feedback_*.md` memory entries

### 11.1 Mandatory post-merge verification (v1.1.0 — PD-5)

After EVERY merge to main, the orchestrator independently re-runs the full quad on main: `cargo build/test/clippy/fmt --workspace`. No exceptions, regardless of what the agent's completion report claimed. Trust-but-verify is not optional — a merge can introduce conflicts the agent never saw, and self-reported green is not proof. If the post-merge quad fails, the merge is reverted (or fixed-forward immediately) before any further dispatch.

**Warm-shared-target staleness caveat (2026-05-15, phase-audit P1-1).** The shared `CARGO_TARGET_DIR` (PD-1) has a side-effect: a test binary built inside a worktree bakes that worktree's *absolute* `CARGO_MANIFEST_DIR` into itself; after the worktree is removed, cargo's fingerprint still considers that cached binary "fresh" (source/deps unchanged) and reuses it — so `cargo test --workspace` can **fail spuriously** (a test panics on a now-deleted `-wt-*` path) OR, worse, **pass against a stale binary** that no longer reflects main. A warm post-merge quad is therefore necessary but NOT sufficient. Rules:
- If a post-merge `cargo test` failure's message references a non-existent path containing `-wt-` (or any deleted worktree dir), treat it as a stale artifact, not a regression: `cargo clean -p <crate>` for the affected crate (or `touch` its source) and re-run that crate's tests fresh to get the true status. Confirm green from the fresh build before trusting.
- A `checkpoint/*` or release tag (§11.4) requires a **COLD** green quad — `cargo test --workspace` from an invalidated/clean target — never a warm-cache pass. "Known-good" must mean built-from-source-green, not cache-green.
- Per-wave warm quads remain the routine gate (speed), but the orchestrator stays alert for `-wt-` stale-path failures and never tags on a warm pass alone.

### 11.2 Liveness heartbeat (v1.1.0 — PD-4)

When dispatching a background wave expected to run >10 min, the orchestrator sets a `ScheduleWakeup` (~25 min) as a deadman switch. If the wave's completion notification has not arrived by wakeup, the orchestrator probes liveness (`ps`, output-file mtime, worktree `git status`). A wave can die silently from disk, MCP disconnect, session interruption, or plan-mode activation — 4 instances during v1.0.

**Presumed-dead ≠ confirmed-dead (2026-05-15 incident).** A stale output mtime + no `cargo` process *during a disk/resource crisis* is NOT proof of death — the agent may be kernel-stalled, not terminated. The only authoritative death signals are the harness completion-notification or the *agent's own pid* provably gone across multiple checks. Acting on a false "dead" inference caused a two-writers-one-worktree race (W0). **Mandatory:** if salvaging a presumed-dead agent's WIP, salvage onto a **NEW branch** and dispatch any continuation into a **NEW worktree** — NEVER re-dispatch into the worktree the presumed-dead agent may still hold. If it revives you then have two clean branches to reconcile, not a corrupt shared checkout. Prefer waiting for the notification unless the wave is genuinely blocking and a fresh-worktree continuation is safe. Full detail: `feedback_verify_agent_liveness` memory.

### 11.3 Phase-boundary audits (v1.1.0 — PD-2)

Audits run not only pre-tag but at every phase boundary — specifically after the analyzer/codegen/simulator triad of any new feature lands, before downstream waves build on it. The P0-1 catastrophe (non-functional lowering) sat undetected for ~8 waves because the first audit ran only after all of Phase 1+2. Catch behavioural rot at the boundary, not at the end.

### 11.4 Known-good checkpoints (v1.1.0 — PD-6)

After each clean audit or phase boundary, the orchestrator creates a lightweight annotated `checkpoint/<YYYY-MM-DD>-<descriptor>` git tag. These are rollback anchors — without them, a subtle regression introduced N waves ago has no clean revert target. Release tags (`vX.Y.Z`) are the public milestones; `checkpoint/*` are the internal safety net.

The orchestrator does NOT implement. Implementation is delegated. The orchestrator's value is in coordination, sequencing, independent verification, and protecting the codebase from incoherent change.

---

## 12. References

- `docs/00-Decisions-And-Reconciliation.md` — normative decisions, including §11 implementation-time decisions table
- `docs/AUDIT_*_*.md` — audit findings drive wave priorities
- `~/.claude/projects/-home-backend-cat/memory/project_embeded_fsm_sdk_backlog.md` — wave-by-wave status
- `~/.claude/projects/-home-backend-cat/memory/MEMORY.md` — cross-project rules apply here too
- Plan file: `~/.claude/plans/toasty-prancing-goose.md` — current phase + roadmap

---

*End of FSM-PROC-SUBAGENT v1.0.0*
