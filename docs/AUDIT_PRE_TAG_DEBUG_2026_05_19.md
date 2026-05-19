# Independent Pre-Tag Four-Lens Audit — Interactive Debug-Interface Epic (Phase-8.0) — 2026-05-19

**Document ID:** FSM-AUDIT-PRE-TAG-DEBUG (frozen evidence doc — the `AUDIT_PRE_TAG_*`
never-overwritten convention; a new audit is a new versioned file; this mirrors
`docs/AUDIT_PRE_TAG_FACTORY_2026_05_18.md`).
**Auditor role:** independent senior staff release auditor (adversarial,
re-derive-don't-trust; the v1.4-W4c / v1.5-post-A2 / factory-W6b precedent of catching
real issues by independent re-derivation, never echoing a wave's self-report). I
implemented none of the debug epic and I am **NOT** the W4b keystone auditor (a
separate, independent lens — referenced, not duplicated). A `TAG-CLEAR` that merely
restates a wave's claim is worse than a found problem; a real P0 → `BLOCK` (or a real
P1 → gated) is the correct, valued outcome.
**Subject / HEAD:** `8092d3a` (`feat(debug-w4a): capture→.trace.json — recorded-from-
the-oracle via the shipped write_trace_yaml, byte-identical by construction`) — main,
on branch `phase8.0/w4c-four-lens-audit`.
**Scope — DELTA-scoped (the §3 precedent, NOT a whole-codebase re-audit, NOT a
keystone re-litigation):** the last release anchor
`checkpoint/factory-reliability-ci-2026-05-18` (commit `fbae38b`) `..8092d3a` =
**57 files, +10045/−243** — the debug-interface epic (W1 `fsm/simulate` LSP layer +
`set_context_field` · W2 WebviewPanel · W3 breakpoints+time-travel · W4a capture)
**plus** the gating fixes since the factory checkpoint (#128 F-1 const-resolver, F-2
queue-config silent-misconfig, the R1/R2 deterministic-primitive audits, the 3 design
docs, Doc 33). The **keystone / no-fork is W4b's binding job** — referenced, NOT
duplicated or re-litigated here.
**Mode:** READ-ONLY except this doc. `git -C`/`git show`/`git diff` + bounded
`cargo test` (disk 7.7 G free — above the 5% margin) + `cargo +stable audit`
(present, v0.22.1) + `npm audit --package-lock-only` (no install). **No `git stash`
(zero push/pop/apply/drop — `git stash list` empty throughout).** Nothing heavy
installed on the box.
**Toolchain (asserted from inside the worktree):** `rustup show active-toolchain`
→ `1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')`
— the repo pin (re-asserted from the worktree before every build).

---

## Verdict

> **`TAG-CLEAR`** — **0 open P0; 0 P1; 3 P2; 2 P3** (the v1.1–v1.5–factory §3 standard
> of 0 P0 AND ≤5 well-scoped P1 is met with **zero P1**). The Rust SCA lens is **0
> vulnerabilities / 0 warnings over 207 crate dependencies** (binding Doc 00
> §11.48/§11.42 — independently RUN via `cargo +stable audit`, **NOT** honest-skipped;
> the tool is present per the factory precedent). **No new network attack surface**
> (the epic rides `fsm-lsp`'s existing `LspService::build().custom_method` stdio
> trust boundary — verified: NO standalone daemon, NO new port, NO new socket, owner
> D-2 "attack surface removed by construction" posture confirmed). The debug epic
> introduced **ZERO new npm dependencies** (`package.json` is manifest-contributions-
> only; `editors/vscode/package-lock.json` is **not in the delta at all** —
> independently derived); the 3 pre-existing devDep-only transitive npm vulns are the
> SAME ones the factory tag accepted-with-rationale (epic added 0 of them) — tracked
> for v1.6 FE-hygiene, **not gating**. **No architectural break, no
> correctness/security defect, no gamed gate.**

The four per-lens verdicts, each independently re-derived (not echoed):

1. **Lens 1 — Architecture / Boundaries / Coupling: PASS.** The single net-new Rust
   dep edge is `fsm-lsp` → `fsm-simulator` (`Cargo.lock`: `+ "fsm-simulator"` in the
   `fsm-lsp` dep list ONLY) — **0 new external `[[package]]`**, an *internal* path
   dep, exactly the v1.5 W-A2 single-internal-edge shape (the established `fsm-lsp` →
   `fsm-verify` pattern). **No layering inversion** (re-derived: `fsm-simulator`'s
   `Cargo.toml` does NOT depend on `fsm-lsp`/`fsm-cli`/`fsm-verify` — the edge is
   strictly one-directional). `forbid(unsafe_code)` unchanged 12/12 (`fsm-simulator`
   already forbids-unsafe, so the edge adds no new crate root). `fsm/simulate` is a
   clean additive `capabilities/simulate.rs` module wired through the *existing*
   `LspService::build().custom_method` seam alongside `fsm/verify` — no re-layering.
   The v1.3 diagram-webview renderer is REUSED (imported `renderModel`, not
   reimplemented — no second renderer); the one v1.3-source modification
   (`diagramWebview.ts`) is a disclosed behaviour-preserving refactor (additive
   exports + a `getVsCodeApi()` singleton + an `#svg && #banner` standalone guard),
   v1.3-path-byte-identical by the stated `diagram.test.ts` regression bar.
2. **Lens 2 — Correctness: PASS.** The #128 F-1 (const-resolver) + F-2 (queue-config)
   fixes are genuine corrections of a #110-class silent-misconfig, not band-aids:
   each unifies a *divergent second resolver* into ONE shared source of truth
   (`util::eval_const_expr_value` for F-1; `ConfigEntry::value()` +
   `CodegenConfig::resolve_queue` for F-2 — the divergent second resolver WAS the
   bug, the keystone-pattern applied to const-folding). Each adds a hard rejecting
   diagnostic for the genuinely-deferred form (F-1 `FSM-E0411`, F-2 `FSM-E0412`) —
   never a silent drop; the G7 diagnostic-count discipline is followed (EXPECTED
   73→75, +2 conformance fixtures VAL-NEG-009/010, the lock GREEN + non-vacuous). The
   `defer` doc-drift fold is a genuine doc-honesty correction (the stale "v1.0
   limitation / rejected with FSM-E0903" text replaced with honest v1.1-shipped
   status). The byte-identity-by-construction property is **empirically reproduced**
   (the auditor's own `simulate_lsp_acceptance` run: 7/7, incl. 3 byte-equal-to-
   `execute_trace` differential tests; the differential corpus 7/7 incl. the new
   F-1/F-2 fixtures). No NEW P0 introduced by the epic delta (full workspace **911
   passed; 0 failed**, auditor-run).
3. **Lens 3 — Reliability / AI-friendliness: PASS.** The honest-failure UX is
   consistent end-to-end: `StepError` surfaced **verbatim** (`step_error_response`
   carries the `Display` reason + a stable `errorKind` discriminator; the
   `step_error_is_surfaced_verbatim_not_a_fake_clean_end` acceptance test PASS),
   malformed requests → honest JSON-RPC invalid-params (never a simulate-of-empty),
   the v1.3 stale-banner REUSED VERBATIM, transport failures surfaced not collapsed,
   `ok:false` never a fake ✓. The **pending-timer honest-degradation** is correctly
   implemented (`debugWebview.ts:598-609` explicitly shows "advance the clock to fire
   due timers" because the W1 `StepRecord` schema carries no pending-timer list — an
   honest placeholder, the #141 deferred surface NOT faked). The disclosed
   `debugWebview.js` ~3.4mb bundle is a pre-existing-pattern characteristic (it
   bundles its own copy of the v1.3 ELK/`elkjs` renderer it reuses — the same large
   dep the v1.3 diagram webview already bundles per esbuild's per-entrypoint model);
   **NOT an epic regression** (confirmed: `package-lock.json` unchanged; the v1.3
   `diagramWebview.js` bundle byte-untouched).
4. **Lens 4 — Security / SCA: PASS (binding).** `cargo +stable audit` over **207
   crate dependencies = 0 vulnerabilities / 0 warnings** (independently RUN, exit 0,
   1093 advisories loaded, zero `vulnerability`/`warning`/`RUSTSEC`/`error` lines —
   **NOT** honest-skipped; the pinned-1.75 CVSS-4.0 caveat sidestepped via
   `+stable`). The new `fsm-lsp` → `fsm-simulator` Cargo edge adds **0 new external
   crates** (it is an internal path dep already in the lock graph). The debug epic
   added **ZERO new npm deps** — `editors/vscode/package.json` shows ONLY manifest
   contributions (`fsm.openDebug` command/keybinding/menus), and
   `editors/vscode/package-lock.json` is **NOT in the 57-file delta** (independently
   derived). The 3 npm-audit findings (`esbuild` mod, `serialize-javascript` high,
   `mocha` high) are byte-identical to the factory-tag baseline's accepted-with-
   rationale set — devDep/build-time/test-only, not in shipped runtime, no
   non-breaking fix, the epic introduced **0** of them → re-stated + v1.6-FE-hygiene-
   tracked, **not gating** (the §11.48/§11.42 standard applied honestly to pre-
   existing non-shipped devDep transitives). **No new network attack surface**: the
   debug surface rides `fsm-lsp`'s existing stdio `custom_method` boundary — no
   standalone daemon, no new port, no new socket (re-derived from `lib.rs`
   `LspService::build().custom_method("fsm/simulate", …)` + the TS side's
   `client.sendRequest("fsm/simulate", …)` over the existing `LanguageClient`; the
   only `Command::new`/`execFileSync` in the delta are **test-only** — gcc/`fsm test`
   in `tests/` + the ExtHost suite, NOT the shipped runtime).

---

## Why this audit is not a rubber-stamp

A clean verdict on a +10045-line epic that adds a new simulator-touching dep edge,
two production-codegen-correctness fixes, and a stateful interactive surface is itself
suspect — so the load-bearing re-derivations are shown explicitly (full table in
§Independent Derivations). Each is **the auditor's own command/read, NOT echoed** from
W4b, Doc 33, or any wave's self-report:

- **The byte-identity property was independently REPRODUCED, not trusted.** The
  module doc + the brief claim the `fsm/simulate*` `StepRecord` stream is "byte-equal
  to `fsm test`'s `execute_trace` by construction". I did not take that on faith — I
  built `fsm-lsp` at HEAD (toolchain re-asserted 1.75 from the worktree) and ran
  `cargo test -p fsm-lsp --test simulate_lsp_acceptance` myself: **7 passed; 0
  failed**, including the three executable byte-equality differentials
  (`init_dispatch_payload_stream_byte_equals_execute_trace_oracle`,
  `advance_clock_timer_stream_byte_equals_execute_trace_oracle`,
  `snapshot_restore_rewind_is_byte_identical_to_oracle_replay`) plus
  `set_context_field_emits_zero_step_record_and_is_not_a_step` (the non-stepping-
  helper contract) and `step_error_is_surfaced_verbatim_not_a_fake_clean_end` (the
  no-fake-success bar). The property is empirically demonstrated at the Rust
  boundary (the load-bearing layer), not asserted.
- **The "only `set_context_field`" simulator-delta claim was source-verified.** I read
  the FULL `git diff fbae38b..8092d3a -- crates/fsm-simulator/src/interpreter.rs`: it
  is exactly **one 4-line guarded map insert** (`rt.context.insert(name, value)` with
  a `NotInitialized` guard) returning `Result<(), StepError>` — no transition, no
  guard-eval, no step, **zero `StepRecord`** by construction (the return type carries
  none). It is the *same nature* as `InitOptions::initial_context` (which already
  pokes the live context at `init`). This is genuinely non-semantic — corroborated by
  the passing `set_context_field_emits_zero_step_record_and_is_not_a_step` acceptance
  test. (The keystone verdict itself is W4b's; this is the Lens-1/2 *coupling /
  correctness* read of the one new surface, independently derived.)
- **The no-second-semantics posture was negative-grepped myself (corroborating, not
  re-litigating W4b).** I grepped the ENTIRE `editors/vscode/src/debug/` tree for
  semantics-implementing identifiers (`selectTransition`/`evalGuard`/`computeLca`/
  `stepOnce`/`runToCompletion`/`fireTimer`/`takeTransition`/`completion`/
  `synthesize`, code-shaped not comments): **∅** (the one hit, `resolveTargetFsm`, I
  read — it is pure VS Code editor/URI *file-path* resolution, not FSM transition-
  target semantics, and a pre-existing reused helper). `simulate.rs` was read
  line-by-line: every `op` arm is exactly one `Interpreter`/`write_trace_yaml` call
  plus serde. This corroborates the no-fork posture independently of W4b's binding
  verdict.
- **The npm-zero-delta claim was independently derived TWO ways.** (a) The
  `git diff fbae38b..8092d3a -- editors/vscode/package.json` shows ONLY
  `contributes.commands/keybindings/menus` hunks — **no `dependencies`/
  `devDependencies` lines**. (b) `git diff fbae38b..8092d3a --name-status --
  editors/vscode/` does **not list `package-lock.json`** (it is tracked —
  `git ls-files` confirms — but UNTOUCHED by the delta). So the npm vuln set is
  byte-identical to the factory-tag baseline by construction; I still ran
  `npm audit --package-lock-only` and got exactly the factory-tag's 3 accepted-with-
  rationale findings (`esbuild`/`serialize-javascript`/`mocha`). The epic added zero.
- **The Rust SCA was actually run, not assumed.** `cargo +stable audit` (v0.22.1,
  present — no install) over 207 deps: advisory DB loaded (1093 advisories),
  Cargo.lock scanned, **exit 0, zero `vulnerability`/`warning`/`RUSTSEC`/`error`
  lines**. Not honest-skipped (the tool is present on this box, the factory
  precedent).
- **The no-new-network-surface (owner D-2) was source-verified, not echoed.** I read
  `crates/fsm-lsp/src/lib.rs` (`LspService::build(Backend::new).custom_method
  ("fsm/verify", …).custom_method("fsm/simulate", …).finish()` over
  `tokio::io::stdin()/stdout()` — the EXACT W-A2 stdio seam, no `TcpListener`/`bind`/
  `serve`-on-a-port) and `editors/vscode/src/debug/simulateClient.ts`
  (`client.sendRequest("fsm/simulate", params)` over the existing `LanguageClient` —
  no `createServer`/`net.`/`WebSocket`/`child_process`-spawn in the shipped path).
  The grep for net/socket/process in the delta surfaced only **test-only**
  `Command::new("gcc")` (in `tests/queue_config_runs.rs` + `codegen_equivalence_
  smoke.rs`) and `execFileSync` (in `debugPanel.test.ts` invoking `fsm test`). The
  "attack surface removed by construction" posture holds.

The one place a tag-blocker could hide (a forked oracle / a second semantics / a
gamed gate / a new exploitable vuln / a new network surface / an architectural break)
was specifically hunted across all 4 lenses and is **absent**. The only findings are
framing/doc-precision and accepted-tracked carried items — none gating.

---

## Lens 1 — Architecture / Boundaries / Coupling — PASS

| Check | Method (run by the auditor) | Result |
|---|---|---|
| The single net-new Rust dep edge | `git diff fbae38b..8092d3a -- Cargo.lock` + `git diff -- crates/fsm-lsp/Cargo.toml` | exactly `+ "fsm-simulator"` in the `fsm-lsp` dep list — an **internal path dep** (`fsm-simulator = { path = "../fsm-simulator" }`); **0 new external `[[package]]`**; the v1.5 W-A2 single-internal-edge shape, the established `fsm-lsp`→`fsm-verify` precedent |
| No layering inversion | `git show 8092d3a:crates/fsm-simulator/Cargo.toml \| grep -iE 'fsm-lsp\|fsm-cli\|fsm-verify'` | **∅** — `fsm-simulator` does NOT depend back on `fsm-lsp`/`fsm-cli`/`fsm-verify`; the edge is strictly one-directional (dev-host tool → simulator core, never the reverse) — sound |
| `forbid(unsafe_code)` invariant | `git grep -l '#![forbid(unsafe_code)]' {8092d3a,fbae38b} -- 'crates/*/src/*.rs' \| wc -l` | **12 at both** — UNCHANGED (`fsm-simulator` already forbids-unsafe; the edge adds no new crate root) |
| `fsm/simulate` module boundary | read `crates/fsm-lsp/src/capabilities/{mod,simulate}.rs` + `lib.rs` + `server.rs` diffs | clean additive `pub mod simulate;`; wired via the **existing** `LspService::build().custom_method` seam alongside `fsm/verify` (no re-layering of `capabilities/`); `Backend::sims` behind a `Mutex` exactly like `docs`/`debounce`/`inlay_cfg` |
| v1.3-diagram + v1.5-command reuse vs duplication | `git diff fbae38b..8092d3a -- editors/vscode/src/diagram/webview/diagramWebview.ts` + read `debug/webview/debugWebview.ts` import | `renderModel` is **imported + reused VERBATIM** (no second renderer); the one v1.3-source change is additive named exports + a `getVsCodeApi()` singleton (wraps the once-only `acquireVsCodeApi()`) + an `#svg && #banner` standalone-guard — a disclosed behaviour-preserving refactor, v1.3-path-byte-identical (the stated `diagram.test.ts` regression bar). esbuild bundles per-entrypoint ⇒ v1.3's own `diagramWebview.js` bundle byte-untouched |
| Net new editors/vscode files vs re-layering | `git diff fbae38b..8092d3a --name-status -- editors/vscode/` | 4 new `src/debug/*` files + 1 new ExtHost test; `tsconfig.webview.json` correctly drops `rootDir` (which would forbid the legit cross-tree reuse import; `noEmit` makes `rootDir` irrelevant) — additive, no re-layering |
| The `op`-dispatched single custom request (not N WS methods) | read `simulate.rs` module doc + `lib.rs` | a disclosed judgment call: Doc 13's wire SHAPE reused *inside* one `fsm/simulate` custom request with an `op` discriminator (the proven W-A2 single-`custom_method` seam) — keeps the keystone-violable surface a single auditable entry point; faithful to Doc 13 semantics, not a second protocol |

**Verdict: PASS.** One internal dep edge (no external dep, no cycle, no inversion),
forbid-unsafe preserved, the new module additively wired through the existing seam,
the v1.3 renderer reused verbatim (the one v1.3-source change a disclosed behaviour-
preserving refactor). No band-aid layering.

---

## Lens 2 — Correctness — PASS

| Check | Method (run by the auditor) | Result |
|---|---|---|
| F-1 const-resolver — a real fix, not a band-aid | read `git show 3e4ec53 -- crates/fsm-analyzer/src/util.rs lower/ids.rs` + the commit body | the divergent SECOND resolver (lowerer's `eval_i64` omitted `EXPR_NAME_REF` while the check's `resolve_expr_value` resolved it ⇒ `after CONST ms` passed `fsm check` but the lowerer silently dropped the timer — a #110-class silent miscompile) is unified into ONE shared `util::eval_const_expr_value` + `util::file_const_table` consumed by BOTH lowerer and check — lower≡check by construction (the keystone-pattern applied to const-folding); the old divergent copies DELETED — a genuine correction |
| F-1 — no silent drop for the deferred form | `git show 8092d3a:crates/fsm-diagnostics/src/lib.rs \| grep E0411` | a hard `FSM-E0411` ("timer duration must be a compile-time constant; runtime-variable is post-v1.0") replaces the silent drop — the `defer`→`FSM-E0903` deferred-construct precedent, never silent |
| F-2 queue-config — a real fix, not a band-aid | read `git log -n1 {9ecb2eb,e4d246b}` + `git show 8092d3a:crates/fsm-codegen-c/src/config.rs` `resolve_queue` | root cause = `lower_queue` matched the KEY `Ident` not the RHS (capacity silently defaulted) + codegen read the CLI default not the IR queue ⇒ codegen↔sim silently diverged. Fixed via a typed `ConfigEntry::value()` (single SoT shared with the analyzer check — the F-1 principle) + `CodegenConfig::resolve_queue` (explicit per-field precedence: integrator-override > in-source `queue {}` > default; a `note:` on shadow, never silent) — a genuine correction of the silent-misconfig class |
| F-2 — class-of-issues coverage + no silent drop | `grep E0412` + read `resolve_queue` | a hard `FSM-E0412` for a non-power-of-2 in-source capacity (class-of-issues: ALL silent queue-config misconfig, not just capacity); the override-note surfaces conflicts (honor config OR diagnose, never silent) |
| F-2 silent-misconfig class is closed END-TO-END | **auditor-run** `cargo test -p fsm-analyzer --test queue_config -p fsm-codegen-c --test queue_config_runs` | **analyzer 7/7 + gcc-compile-and-RUN 4/4** — the in-source `queue {}` capacity/overflow now reach generated C AND behave correctly at runtime (DROP_NEWEST FIFO survivor order + ASSERT abort on overflow, both Switch & Table strategies). Genuine behavioural acceptance, not symbol-presence |
| G7 diagnostic-count discipline | `git show {fbae38b,8092d3a}:crates/fsm-diagnostics/src/lib.rs \| grep EXPECTED` + `cargo test -p fsm-cli --test conformance_code_coverage_lock` (auditor-run) | EXPECTED **73→75** (+2, exactly one per E0411/E0412); the lock **4/4** incl. `every_live_code_has_a_conformance_fixture_or_a_justified_allowlist_entry` (real fixtures VAL-NEG-009/010, no allowlist dodge) + `red_proof_…` (non-vacuous) + `coverage_map_md_is_byte_derivable_…` (no hand-drift) |
| The `defer` doc-drift fold | `git diff fbae38b..8092d3a -- docs/04-DSL-Specification.md` | the stale "**v1.0 limitation** … rejected by `FSM-E0903`" §9.4 paragraph is removed + replaced with honest v1.1-shipped status (analyzer lowers / sim replays / codegen emits); a `FSM-E0610`-enforcement-status honesty note added (gated only for `submachines`; the rest declarative-only — disclosed as uniform, not a per-flag inconsistency) — a genuine doc-honesty correction |
| Byte-identity-by-construction property | **auditor-run** `cargo test -p fsm-lsp --test simulate_lsp_acceptance` + `cargo test -p fsm-simulator --test codegen_equivalence_smoke` | acceptance **7/7** (incl. 3 byte-equal-to-`execute_trace` differentials + the zero-`StepRecord` `setContext` contract); the differential corpus **7/7** (incl. the new F-1 `timer-const-ref` + F-2 `queue-overflow` BYTE_EQUAL fixtures + the deliberately-corrupted-oracle RED guards — the property holds AND the gate is non-vacuous) |
| No NEW P0 from the epic delta | **auditor-run** `cargo test --workspace` | **911 passed; 0 failed**; `cargo test --workspace` exit 0 — no regression introduced by the +10045-line delta |

**Verdict: PASS.** F-1/F-2 are genuine corrections of a #110-class silent-misconfig
(the keystone-pattern de-duplication of divergent resolvers, each with a hard
rejecting diagnostic for the deferred form), the G7 count discipline is honestly
enforced, the `defer` doc-drift fold is a real doc-honesty correction, the byte-
identity property is empirically reproduced, and no NEW P0 is introduced.

---

## Lens 3 — Reliability / AI-friendliness — PASS

| Check | Method (run by the auditor) | Result |
|---|---|---|
| `StepError` surfaced verbatim (no fake clean end) | read `simulate.rs` `step_error_response`/`step_error_kind` + the `step_error_is_surfaced_verbatim_not_a_fake_clean_end` test (auditor-run, PASS) | the `Display` reason is forwarded **verbatim** + a stable kebab `errorKind` (a pure variant projection, no decision); every `op` arm returns `step_error_response(&e)` on error — never collapsed to a fabricated clean verdict |
| Malformed request → honest invalid-params | read `want_str`/`InvalidParams` + the `malformed_request_is_invalid_params_not_empty_simulate` test (auditor-run, PASS) | a missing `op`/`instanceId`/required field is an honest JSON-RPC invalid-params with a human reason — never a simulate-of-empty/wrong input (the cardinal-sin bar at the boundary) |
| `ir_for` honest "cannot simulate broken model" | read `simulate.rs::ir_for` | a model with analysis errors → the honest "cannot simulate a model that does not compile (run `fsm check`)" reason, never a fabricated session over broken input — the cardinal-sin bar at `load` |
| The honest stale-banner | read `debugPanel.ts` `STALE_BANNER` (line 78) + the v1.3 reuse note | the Doc 05 §1.5.9 banner is REUSED VERBATIM from v1.3; a malformed IR → honest stale-banner + the picker shows no events (never a fabricated list); transition breakpoints "cannot be armed — never a fabricated id" |
| Pending-timer honest-degradation (the #141 deferred surface) | read `debugWebview.ts:598-609` | the panel HONESTLY shows "pending timers: advance the clock to fire due timers" because the W1 `StepRecord` schema carries no pending-timer list (a full surface "would require a W1 schema addition — out of W2 scope") — an honest placeholder, the #141 deferral NOT faked. **The correct honest-degradation posture, NOT a finding** |
| The disclosed `debugWebview.js` ~3.4mb bundle | `git diff -- editors/vscode/esbuild.mjs` + the `package-lock.json`-unchanged derivation | a SEPARATE additive esbuild entrypoint that inlines its own copy of the v1.3 ELK/`elkjs` `renderModel` it reuses (esbuild per-entrypoint) — the SAME large dep the v1.3 diagram webview already bundles; **pre-existing-pattern, NOT an epic regression** (no new npm dep; the v1.3 `diagramWebview.js` bundle byte-untouched) — confirmed |
| No fake ✓ on capture/copyIr | read `debugPanel.ts` (`ok:false`+`message` lines 279-280) + the W4 `capture` arm (`simulate.rs:578-659`) | `ok:false`+`message` is the HONEST failure (write/serde error verbatim, never a fake ✓); the `capture` arm hard-rejects an empty/absent `expected` ("`fsm test` would report nothing-to-verify — not a pass") — the recorded-from-the-oracle no-fake bar |
| Clear seams / AI-legibility | read the module docs across `simulate.rs`/`simulateClient.ts`/`debugPanel.ts` | every keystone-violable surface carries an explicit "this module decides NO semantics … the `Interpreter` owns 100%" disclaimer + the wire schema is re-derived-from-source in-comment — clear, auditable, future-audit-legible |

**Verdict: PASS.** The honest-failure UX is consistent end-to-end (`StepError`
verbatim, honest invalid-params, honest stale-banner, honest pending-timer
degradation, no fake ✓), the disclosed 3.4mb bundle is a pre-existing-pattern
characteristic (not an epic regression), and the seams are clear + self-disclaiming.

---

## Lens 4 — Security / SCA — PASS (binding; Doc 00 §11.48/§11.42)

### 4.1 Rust SCA — `cargo +stable audit` (independently RUN, NOT honest-skipped)

`cargo-audit` v0.22.1 is **present** (`~/.cargo/bin/cargo-audit`) and a `stable`
toolchain exists, so the pinned-1.75 CVSS-4.0-DB-parse caveat (the documented `sca`-
job limitation) is sidestepped via `cargo +stable audit`:

```
$ cd /root/dev/embeded-fsm-sdk-wt-w4c-fourlens && cargo +stable audit
    Loaded 1093 security advisories (from /home/backend_cat/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (207 crate dependencies)
$ echo $? → 0    # zero vulnerability/warning/RUSTSEC/error lines emitted
```

**Result: 0 Rust vulnerabilities, 0 warnings** over **207 crate dependencies**.
Binding Doc 00 §11.48/§11.42 ("0 vulnerabilities before a vX.Y.0 tag") satisfied
empirically for the Rust dependency graph. **Not honest-skipped** (the tool is
present and ran). The new `fsm-lsp` → `fsm-simulator` edge is an *internal* path dep
already in the lock graph — it adds **0 new external crates** (independently derived
from the `Cargo.lock` diff: the only change is `+ "fsm-simulator"` in the `fsm-lsp`
dep list).

### 4.2 npm SCA — the debug epic added ZERO npm deps (independently derived)

The debug epic's `editors/vscode/package.json` change is **manifest contributions
only** (`git diff fbae38b..8092d3a -- editors/vscode/package.json` ⇒ ONLY
`contributes.commands` `fsm.openDebug` + its keybinding + menu entries — **no
`dependencies`/`devDependencies` lines**), and `editors/vscode/package-lock.json` is
**NOT in the 57-file delta** (`git diff --name-status -- editors/vscode/` does not
list it; it is tracked — `git ls-files` confirms — but UNTOUCHED). So the npm
vulnerability set is **byte-identical to the factory-tag baseline by construction**.
`npm audit --package-lock-only --audit-level=low` confirms exactly the factory-tag's
3 accepted-with-rationale findings:

| # | Vuln | Sev | devDep vs runtime | Disposition (the factory-tag precedent, applied) |
|---|---|---|---|---|
| 1 | `esbuild <=0.24.2` (GHSA-67mh-4wv8-2f99) | **moderate** | **devDependency** (the build-time bundler; never shipped in the VSIX) | **accept-with-written-rationale** — pre-existing (identical at the factory baseline), devDep/build-time-only, not attacker-reachable in our one-shot-bundler usage, no non-breaking fix (`--force` ⇒ esbuild@0.28.0 major); **epic added 0 of it**. v1.6 FE-hygiene tracked. |
| 2 | `serialize-javascript <=7.0.4` (GHSA-5c6j-r48x-rmvq RCE / GHSA-qj8w-gfj5-8c6v DoS) | **high** | **transitive of `mocha`** (test runner) | **accept-with-written-rationale** — pre-existing, test-only transitive, trusted-input (the project's own ExtHost fixtures; no untrusted `serialize()`), not in shipped runtime, no non-breaking fix (a major mocha *downgrade*); **epic added 0 of it**. v1.6 FE-hygiene tracked. |
| 3 | `mocha 8.0.0-12.0.0-beta-2` (depends on the vulnerable serialize-javascript) | **high** | **devDependency** (the test runner; excluded from the VSIX) | **accept-with-written-rationale** — identical rationale to #2 (it is the parent); pre-existing, test-only, trusted-input, not-in-runtime; **epic added 0 of it**. v1.6 FE-hygiene tracked. |

Per the Doc 00 §11.48/§11.42 standard applied **honestly** to pre-existing
non-shipped devDep transitives the epic introduced **zero** of: all 3 are
build-time/test-only, not in the shipped extension runtime, no non-breaking fix —
**NOT P0 tag-blockers**; each `accept-with-written-rationale`, re-stated in the gate
doc's carried-escalations + the v1.6 FE-hygiene batch (the §11.62 / v1.5 §6 / factory-
F-03 re-state-don't-drop discipline). Logged collectively as **DF-03 (P2 — accepted-
tracked, not gating)**.

### 4.3 No new attack surface from the epic (owner D-2 — verified, not echoed)

| Surface | Method (run by the auditor) | Result |
|---|---|---|
| The debug transport (no daemon/port/socket) | read `crates/fsm-lsp/src/lib.rs` (`LspService::build(Backend::new).custom_method("fsm/verify",…).custom_method("fsm/simulate",…).finish()` over `tokio::io::stdin()/stdout()`) | the debug surface rides the **EXACT existing W-A2 stdio `custom_method` seam** — NO `TcpListener`/`bind`/`serve`-on-a-port, NO standalone daemon, NO new socket. The "attack surface removed by construction" posture (D-2) holds |
| The VS Code side | read `editors/vscode/src/debug/simulateClient.ts` (`client.sendRequest("fsm/simulate", params)` over the existing `LanguageClient`) | one `sendRequest` over the v1.5 W-A2 `LanguageClient` boundary — NO `createServer`/`net.`/`WebSocket`/`child_process`-spawn in the shipped path |
| Net/socket/process in the Rust delta | `git diff fbae38b..8092d3a -- 'crates/**/*.rs' \| grep -iE 'TcpListener\|bind(\|serve\|::net::\|Command::new\|std::process'` | only **test-only** `Command::new("gcc")` in `tests/queue_config_runs.rs` + `tests/codegen_equivalence_smoke.rs` (gcc-compile-and-run harnesses) — NOT the shipped runtime. No new shipped attack surface |
| Net/socket/process in the TS delta | `git diff fbae38b..8092d3a -- 'editors/vscode/**/*.ts' \| grep -iE 'createServer\|net\.\|WebSocket\|child_process\|spawn(\|execFileSync'` | only **test-only** `execFileSync` in `src/test/suite/debugPanel.test.ts` (invoking the shipped `fsm test` for the W4 byte-identity proof) — NOT the shipped extension runtime. No new shipped attack surface |
| New npm tree | §4.2 (package.json manifest-only; package-lock.json NOT in the delta) | the epic added **0 npm deps** ⇒ **0 new npm attack surface** |

**Verdict: PASS (binding).** Rust SCA = 0 vulns / 0 warnings over 207 deps
(empirically run). The 3 npm findings are pre-existing, devDep-only, not-in-runtime,
no-non-breaking-fix, epic-added-0 → each `accept-with-written-rationale`, re-stated +
tracked, not gating. **No new network attack surface** — the debug surface rides the
existing stdio trust boundary by construction (no daemon/port/socket; the only
net/process in the delta is test-only).

---

## Tracked-deferrals — still tracked (the brief's explicit confirm-not-reopen ask)

Each named deferral is confirmed **tracked-not-dropped** (Doc 33 / the factory-tag
carried items) — **NOT re-opened as a new finding**:

| Deferral | Tracked where (auditor-verified) | Status |
|---|---|---|
| #138 Item-3 queue co-design / D-3 sequencing | Doc 33 §0 D-3, §4 OWNER-1, §7 (`§11.DRAFT-C`) — "debug-interface FIRST … own post-v1.6 minor, `checkpoint/<name>`-tagged, LOCAL only" | **TRACKED** — recorded, not dropped |
| #141 pending-timer surface + G9 push | Doc 33 §4 OWNER-2; the surface itself honestly degraded in `debugWebview.ts:598-609` ("would require a W1 schema addition — out of W2 scope") | **TRACKED** — honest-degraded + recorded, not dropped |
| #139 `raise` codegen gap | Doc 33 §8 / OWNER-3 (the parked full `signal`-primitive epic "cohesively retiring the #127 AUDIT-DETPRIM … GAPs"); the deterministic-primitive audit lineage | **TRACKED** — folded into the parked signal epic, not dropped |
| #134 AVR Harvard hazard (CF-1) | Doc 33 §5 CF-1 + OWNER-4 — "a CI-runner-only on-target-matrix `avr-gcc` link+run candidate, NOT a debug-interface item … recorded so it is auditable and not lost" | **TRACKED** — a named factory-reliability-epic candidate, not dropped |
| #123 W3 §6 catalog tail (`.contains` codegen-c oracle) | the factory-tag audit's F-04 (`AUDIT_PRE_TAG_FACTORY_2026_05_18.md` / `AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md:306`), owned by the G7 wave; not regressed by the debug epic | **TRACKED** — carried, owned by G7, not dropped |
| #103 v1.6 doc-honesty (CF-2) | Doc 33 §5 CF-2 + OWNER-4 — "folds into the owner-reserved v1.6 doc-honesty pass, cross-ref task #103" | **TRACKED** — folded into v1.6, not dropped |

All carried with the explicit §11.49 leave-and-explain / §11.62 re-state-don't-drop
discipline ("recorded so it is tracked, not lost"). **Confirmed: every named deferral
is tracked-not-dropped; none re-opened as a new finding.**

---

## Findings (P0 ship-blocker / P1 well-scoped / P2 minor / P3 nit)

| ID | Lens | Sev | Location | Issue | Recommended action / disposition |
|---|---|---|---|---|---|
| DF-01 | 1 | **P2** | `editors/vscode/src/diagram/webview/diagramWebview.ts` (the one v1.3-source modification) + the brief's "v1.3 core untouched" framing | The brief / commit bodies say "v1.3 diagram core untouched". Precisely: the v1.3 *bundled output* (`dist/webview/diagramWebview.js`) IS byte-untouched (esbuild per-entrypoint), but the v1.3 *source* was modified — additive named exports + a `getVsCodeApi()` singleton (wrapping the once-only `acquireVsCodeApi()`) + an `#svg && #banner` standalone-guard around the message-listener/`ready`-post. This is a **disclosed, behaviour-preserving** refactor (v1.3-path-byte-identical, the stated `diagram.test.ts` regression bar) and architecturally sound (it enables verbatim reuse with NO fork), but "v1.3 core untouched" is imprecise for the *source* layer. | **Non-gating** (the change is a sound behaviour-preserving refactor, not a fork or regression — the v1.3 bundle is byte-identical and the reuse is verbatim). Recommend the gate doc / release record state precisely: "v1.3's *bundled output* is byte-untouched; the v1.3 *source* was behaviour-preservingly refactored (additive exports + singleton API accessor + standalone-guard) to enable verbatim `renderModel` reuse — `diagram.test.ts` is the pre/post-identity bar." → closeout-batch wording. The `diagram.test.ts` v1.3-path-identity is **CI-runner-only** (ExtHost lane) — re-confirm in the W4d cold-quad's JS lane (correctly contingent, not asserted here). |
| DF-02 | 3 | **P3** | `docs/design/DEBUG_INTERFACE_UX_DESIGN_2026_05_19.md` (DBGUX prose) vs the shipped trace filename | DBGUX prose says capture writes `.trace.yaml`, but the SHIPPED `collect_traces` (`cmd/test.rs:159`) globs `.trace`/`.trace.json` and `write_trace_yaml` actually emits JSON — so the panel correctly writes `*.trace.json` (a `.trace.yaml` would be SILENTLY SKIPPED → a vacuous pass). The implementation is **correct** (re-derived-from-source, the trap explicitly avoided + disclosed in `debugPanel.ts:1313-1318`); only the design-doc prose is loosely phrased (the factory-F-02 "loose doc phrasing vs verified source" class). | **Non-gating** (the implementation is correct and the judgment call is explicitly disclosed in-code). Defer the DBGUX-prose `.trace.yaml`→`.trace.json` precision to the owner-reserved **v1.6 doc-honesty pass** (the frozen design doc is design-evidence — do NOT edit it here; same class as the GT-7 DBGUX line-drift Doc 33 already records). |
| DF-03 | 4 | **P2** | `editors/vscode` devDeps (esbuild/serialize-javascript/mocha) | 3 pre-existing devDep-only transitive vulns (1 mod, 2 high) — byte-identical to the factory-tag baseline; not in shipped runtime; not exploitable in our build/CI; no non-breaking fix; **the debug epic introduced 0 of them** (package.json manifest-only, package-lock.json NOT in the delta — independently derived). | **accept-with-written-rationale ×3** (per-vuln §4.2). Re-state in `GATE_VERIFICATION_<tag>.md` §carried-owner-escalations + the v1.6 FE-hygiene batch. **Does NOT gate the tag** (the §11.48/§11.42 standard applied honestly to pre-existing non-shipped devDep transitives the epic did not introduce — the factory-F-03 precedent). |
| DF-04 | 2 | **P2** | `tests/conformance/codegen-c/*` oracle (`.contains`) — the #123 deferred tail | The codegen-c conformance oracle is substring-presence (hollow); already flagged P1 in `AUDIT_RELIABILITY_STAGE_GATE_2026_05_18.md:306`, owned by the **G7 wave**; the W1/W2 differential + the W3 lock are the binding behavioural gate in the interim. **NOT regressed by the debug epic.** | **accept-tracked-with-rationale** — known/owned/deferred test-debt (the G7 conversion), NOT regressed, NOT a tag-blocker. Re-state in the gate doc's carried items (the factory-tag F-04, carried forward). |
| DF-05 | 2 | **P3** | `crates/fsm-analyzer/src/type_check.rs::resolve_simple_literal` (F-1 adjacent-site, characterized in the commit body) | The F-1 commit discloses `type_check.rs::resolve_simple_literal` is a DISTINCT non-defect const-fold (returns i128 to defer overflow to the `FSM-E0206` caller; literal-only by design; check-only, no lowerer counterpart ⇒ provably NOT the F-1 defect) — left untouched + characterized in-place. I confirmed it is a distinct, intentional helper (no lowerer counterpart to drift from), so the leave-and-explain is correct. A purely informational note (it is correctly NOT the same defect). | **Non-gating, informational** — the adjacent-site characterization is correct (no second-resolver-divergence here; check-only with no lowerer counterpart). No action required; recorded for audit completeness (the W1-FU adjacent-site-audit discipline, correctly applied). |

**No P0. No P1.** All findings are framing/doc-precision (DF-01/DF-02), an accepted-
tracked devDep-SCA disposition (DF-03), an accepted-tracked test-debt carried from the
factory tag (DF-04), or an informational adjacent-site note (DF-05) — none gating. The
delta is architecturally sound (one internal dep edge, no inversion, the v1.3 renderer
reused verbatim), the F-1/F-2 fixes are genuine corrections (the silent-misconfig
class closed end-to-end, behaviourally proven by the auditor's own gcc-run), the byte-
identity property is empirically reproduced, the honest-failure UX is consistent, the
Rust SCA is empirically clean, and there is no new network attack surface.

---

## Independent Derivations Table (proof I re-derived, did not echo)

| # | Derived fact | Exact command / source | Output | Echo-or-derived |
|---|---|---|---|---|
| DD-01 | The delta range + size | `git diff --shortstat fbae38b..8092d3a` | 57 files / +10045 / −243 | DERIVED |
| DD-02 | Single net-new Rust dep edge, 0 new external | `git diff fbae38b..8092d3a -- Cargo.lock` | only `+ "fsm-simulator"` in the `fsm-lsp` dep list (internal path dep) | DERIVED |
| DD-03 | No layering inversion | `git show 8092d3a:crates/fsm-simulator/Cargo.toml \| grep -iE 'fsm-lsp\|fsm-cli\|fsm-verify'` | **∅** — one-directional edge | DERIVED |
| DD-04 | forbid(unsafe_code) unchanged | `git grep -l '#![forbid(unsafe_code)]' {8092d3a,fbae38b} -- 'crates/*/src/*.rs' \| wc -l` | 12 at both | DERIVED |
| DD-05 | Only `set_context_field` is the simulator semantic delta (4-line guarded insert, 0 StepRecord) | `git diff fbae38b..8092d3a -- crates/fsm-simulator/src/interpreter.rs` (full read) | one `rt.context.insert` w/ `NotInitialized` guard, `Result<(),StepError>` | DERIVED |
| DD-06 | `fsm/simulate` rides the existing stdio custom_method seam | read `crates/fsm-lsp/src/lib.rs` | `LspService::build().custom_method("fsm/simulate",…).finish()` over `tokio::io::stdin/stdout` — no port/daemon/socket | DERIVED |
| DD-07 | TS side uses the existing LanguageClient (no new transport) | read `editors/vscode/src/debug/simulateClient.ts` | `client.sendRequest("fsm/simulate", params)` over the v1.5 W-A2 `LanguageClient` | DERIVED |
| DD-08 | No semantics fn in the debug TS surface | `grep -rE '(selectTransition\|evalGuard\|computeLca\|stepOnce\|runToCompletion\|fireTimer\|takeTransition\|synthesize)' editors/vscode/src/debug/` (code-shaped) | ∅ (the one hit `resolveTargetFsm` = file-path resolution, read+confirmed) | DERIVED |
| DD-09 | byte-identity property reproduced | **auditor-run** `cargo test -p fsm-lsp --test simulate_lsp_acceptance` | **7 passed; 0 failed** (incl. 3 byte-equal-to-execute_trace differentials) | DERIVED |
| DD-10 | differential corpus green + non-vacuous | **auditor-run** `cargo test -p fsm-simulator --test codegen_equivalence_smoke` | **7 passed; 0 failed; 2 ignored** (incl. corrupted-oracle RED guards + new F-1/F-2 fixtures) | DERIVED |
| DD-11 | F-2 silent-misconfig class closed end-to-end | **auditor-run** `cargo test -p fsm-analyzer --test queue_config -p fsm-codegen-c --test queue_config_runs` | analyzer **7/7** + gcc-RUN **4/4** | DERIVED |
| DD-12 | G7 lock green + non-vacuous | **auditor-run** `cargo test -p fsm-cli --test conformance_code_coverage_lock` | **4 passed; 0 failed** (real fixtures, no allowlist dodge, byte-derivable map) | DERIVED |
| DD-13 | No regression from the delta | **auditor-run** `cargo test --workspace` | **911 passed; 0 failed**; exit 0 | DERIVED |
| DD-14 | EXPECTED diag count discipline | `git show {fbae38b,8092d3a}:crates/fsm-diagnostics/src/lib.rs \| grep EXPECTED` | 73 → 75 (+E0411 +E0412) | DERIVED |
| DD-15 | F-1 unifies divergent resolvers into one SoT | `git show 3e4ec53:crates/fsm-analyzer/src/util.rs` (read `eval_const_expr_value`/`file_const_table`) | one shared resolver consumed by lowerer + check; old copies DELETED | DERIVED |
| DD-16 | F-2 explicit per-field precedence + note-on-shadow | `git show 8092d3a:crates/fsm-codegen-c/src/config.rs` (read `resolve_queue`) | override > in-source > default, per-field, `QueueOverrideNote` on shadow | DERIVED |
| DD-17 | `defer` doc-drift fold is genuine | `git diff fbae38b..8092d3a -- docs/04-DSL-Specification.md` | stale "v1.0 limitation / FSM-E0903" removed → honest v1.1-shipped status | DERIVED |
| DD-18 | Rust SCA clean | **auditor-run** `cargo +stable audit` | 207 deps, exit 0, 0 vuln / 0 warn (1093 advisories) | DERIVED |
| DD-19 | Epic added 0 npm deps | `git diff fbae38b..8092d3a -- editors/vscode/package.json` + `--name-status -- editors/vscode/` | package.json manifest-only; package-lock.json NOT in the delta (tracked but untouched) | DERIVED |
| DD-20 | npm vulns = factory-tag baseline set | **auditor-run** `npm audit --package-lock-only --audit-level=low` | exactly `{esbuild, serialize-javascript, mocha}` 1mod/2high | DERIVED |
| DD-21 | Only test-only net/process in the delta | `git diff fbae38b..8092d3a` grep `Command::new\|execFileSync\|TcpListener\|createServer` | gcc in `tests/*` + `fsm test` in `debugPanel.test.ts` only — no shipped surface | DERIVED |
| DD-22 | v1.3 renderer reused verbatim (one disclosed source refactor) | `git diff fbae38b..8092d3a -- editors/vscode/src/diagram/webview/diagramWebview.ts` + read `debug/webview/debugWebview.ts` import | `renderModel` imported (no second renderer); v1.3 change = additive exports + `getVsCodeApi()` singleton + `#svg&&#banner` guard (behaviour-preserving) | DERIVED |
| DD-23 | Pending-timer honestly degraded (the #141 surface) | read `editors/vscode/src/debug/webview/debugWebview.ts:598-609` | honest "advance the clock to fire due timers" placeholder — not a fake list | DERIVED |
| DD-24 | The W4 trace filename is correctly `.trace.json` (the vacuous-pass trap avoided) | read `crates/fsm-cli/src/cmd/test.rs:140-159` + `editors/vscode/src/debug/debugPanel.ts:1313-1318` | `collect_traces` globs `.trace`/`.trace.json`; panel writes `*.trace.json` (a `.trace.yaml` would be silently skipped — disclosed) | DERIVED |
| DD-25 | Conformance corpus delta | `git show {fbae38b,8092d3a}:tests/conformance/MANIFEST.json \| grep -c '"id"'` | 53 → 55 (+VAL-NEG-009/010 for E0411/E0412) | DERIVED |
| DD-26 | Tracked deferrals still tracked | `grep -niE '#138\|#141\|#139\|#134\|#123\|#103\|CF-1\|CF-2' docs/33-Debug-Interface-Wave-Plan.md` | all recorded §0/§4/§5/§7/§8 with "tracked, not lost" — none dropped | DERIVED |
| DD-27 | clean read-only state / no stash | `git status --porcelain`; `git stash list` (throughout) | both empty | DERIVED |
| DD-28 | toolchain pin (re-asserted from the worktree before every build) | `rustup show active-toolchain` from the worktree | `1.75.0… (overridden by '<worktree>/rust-toolchain.toml')` | DERIVED |

---

## Sequence note (necessary-not-sufficient — the v1.4-W4c / v1.5 / factory-W6b analogue)

`TAG-CLEAR` from this audit is **necessary, not sufficient**. The Phase-8.0 debug-
interface minor's LOCAL `checkpoint/<name>` tag lands on the gate-doc commit `X` (a
*later* closeout motion) **iff** (a) this four-lens is TAG-CLEAR (it is) **and** (b)
W4b's binding source-derived keystone phase-audit is `KEYSTONE-INTACT` (W4b's job —
referenced, NOT pre-judged here; this audit deliberately does NOT re-derive the no-
fork verdict, only corroborates the *coupling/correctness/reliability/security* lenses
that surround it) **and** (c) the §11.30 cold-from-source quad **at `X`** WITH the
MANDATORY NOTE-1 `cargo clean -p` STEP 0 + the **binding JS/ExtHost lane at `X`**
(the v1.3-path-identity `diagram.test.ts` + the W4 capture round-trip — CI-runner-only
by the disk-tight infra constraint, the factory on-target-lane precedent) is green.
The cold-quad + the ExtHost-lane-at-`X` are the closeout wave's to empirically close
at the tag-doc commit and are correctly posed as **contingent** (not asserted here).
This audit is RECORD+QUALITY+SECURITY integrity for the *delta*; nothing here was
built to close a claim. **Note for the closeout wave:** `cargo +stable audit` works on
this box (it ran clean here, §4.1) — re-run it at `X` per the canonical cold-quad
`sca` step; no honest-skip-pending-note is needed for the Rust SCA (the tool is
present). The ExtHost lane (debugPanel.test.ts + diagram.test.ts) genuinely needs
`npm ci` + an Electron download and is the CI-runner-only honest-skip class on this
disk-tight box — its structure was source-verified here (the W4 `fsm test` byte-
identity assertion + the `#svg&&#banner` v1.3-path-identity guard); it must run green
in the `extension-host` CI lane at `X`. Per [[feedback_embeded_fsm_pipeline_before_ui]]
the LSP/CLI pipeline layer (the load-bearing byte-identity proof) WAS empirically
re-run here and is green; the UI/ExtHost layer is pure marshalling over it.

---

## Mechanics

- Worktree: `/root/dev/embeded-fsm-sdk-wt-w4c-fourlens`, branch
  `phase8.0/w4c-four-lens-audit` (at `8092d3a` == main HEAD). Verified clean at start
  and at the time of writing (`git status --porcelain` empty; `git stash list` empty
  throughout — **zero `git stash` push/pop/apply/drop**).
- READ-ONLY except this doc. `git -C`/`git show`/`git diff` + bounded `cargo test`
  (disk 7.7 G free — above the 5% margin; the worktree built its own
  `/root/dev/embeded-fsm-sdk-target` from cold, ~29 s for the `fsm-lsp` arc) +
  `cargo +stable audit` (present, v0.22.1) + `npm audit --package-lock-only` (no
  install). Nothing heavy installed on the box; the VS Code Extension-Host suite was
  NOT run locally (CI-runner-only by the disk-tight infra constraint — `node_modules`
  absent + an Electron download required; its structure was source-verified, the
  factory on-target-lane honest-skip-by-design precedent — NOT a finding).
- Toolchain re-asserted from inside the worktree before every build:
  `1.75.0-x86_64-unknown-linux-gnu (overridden by '<worktree>/rust-toolchain.toml')`
  — the repo pin (the box default reads 1.95 elsewhere; benign, not pin drift).
- This doc is NEW (`docs/AUDIT_PRE_TAG_DEBUG_2026_05_19.md`); no existing audit/doc
  overwritten; exactly one file written. Committed on `phase8.0/w4c-four-lens-audit`
  as a sibling FROZEN evidence commit — **NOT** folded into the tag-doc commit `X`;
  not merged, not pushed, not tagged.

*— End of independent pre-tag four-lens audit, interactive debug-interface epic
(Phase-8.0), 2026-05-19. A rubber-stamp here would be worse than a found problem; the
four lenses are stated `TAG-CLEAR` because each was independently confirmed by this
auditor's own derivation from `git`/source/behaviour at `8092d3a` (the byte-identity
property was empirically RE-RUN, not trusted; the only-`set_context_field` simulator
delta was source-verified; the no-semantics debug surface was negative-grepped; the
npm-zero-delta was independently derived two ways; the Rust SCA was actually run; the
no-new-network-surface was source-verified; the F-2 silent-misconfig closure was gcc-
run by the auditor), NOT restated from W4b, Doc 33, or any wave's self-report. The
keystone/no-fork verdict is W4b's binding, separate lens — referenced, not
duplicated.*
