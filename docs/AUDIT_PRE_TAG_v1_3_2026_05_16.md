# Independent Pre-`v1.3.0`-Tag Four-Lens Audit — 2026-05-16

**Auditor role:** independent senior release auditor (adversarial, re-derive-don't-trust).
**Subject:** FSM Studio, `main` HEAD `03e2a14` (= the v1.3 release commit "X" = the
`docs: v1.3.0 release record + batched closeout consolidation` commit).
**Baselines:** v1.2.0 frozen at `71043935`; W0-clean at `ceb8efd` (== `checkpoint/2026-05-16-w0-clean`).
**Mode:** READ-ONLY, zero `cargo`/`npm`/build, `git -C` only. Worktree
`/root/dev/embeded-fsm-sdk-wt-v13pretag`, branch `phase3.3/v1_3-pretag-audit`.
**Mandate context:** the v1.3 closeout consolidation already produced ONE cardinal
overstatement (the falsely-claimed "zero Rust delta vs v1.2 / `71043935`", erasing W0)
that was caught + corrected pre-merge. This audit assumes a second may exist and
re-derives every load-bearing number from source/`git show`, never echoing prior prose.

---

## Verdict

> **`TAG-CLEAR`** — 0 P0; 0 P1; 3 P2; 4 P3 (all wording/precision nits, none gating).
> The Rust-delta-framing correction **verified held** with **no new cardinal
> overstatement**: every surviving "zero/empty/unchanged" Rust claim is correctly
> scoped to `ceb8efd`/post-W0 or to Cargo.lock-0-new-deps (independently proven TRUE);
> the forbidden "zero Rust delta vs v1.2/`71043935`" / "rust_delta…EMPTY (vs v1.2)" /
> "UNCHANGED vs v1.2 (Rust)" overstatement is **absent** (adversarial grep = 0 hits).
> W0 is everywhere presented AS the v1.3 Rust delta (18 files +421/−93, NOT empty).

---

## Lens 1 — Release-record integrity (independent pass)

### 1.1 The Rust-delta-framing correction — VERIFIED HELD (the cardinal check)

**Independent ground truth (re-derived, not echoed):**

| Claim under test | Command | Result |
|---|---|---|
| W0 IS the v1.3 Rust delta | `git diff --stat 71043935 03e2a14 -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | **18 files, +421/−93** (fsm-analyzer 14 files + fsm-parser 4 files) — **NOT empty** |
| V1–V6 add zero Rust | `git diff --stat ceb8efd 03e2a14 -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | **empty** |
| 0 new deps | `git diff --stat 71043935 03e2a14 -- Cargo.lock` | **empty** |
| W0 == checkpoint | `git rev-parse ceb8efd checkpoint/2026-05-16-w0-clean` | both `ceb8efd…` (identical) |
| Cargo.lock W0 | `git diff --stat 7d6792a ceb8efd -- Cargo.lock` | **empty** |

**Adversarial overstatement grep** (`GATE_VERIFICATION_v1_3.md` + `metrics/2026-05-16-v1_3.json`),
forbidden pattern `(zero|no|empty|unchanged|0).{0,40}(rust).{0,40}(v1\.2|71043935)` and its
reverse, **after** excluding the legitimate Cargo.lock / 0-new-deps / dependency-surface
claims: **0 matches.** Every Rust-delta sentence in both files is the corrected framing —
e.g. GATE §1 G1 / §4 line 100 / metrics `v1_3_rust_delta_is_exactly_w0` all state
*"`git diff --stat 71043935 fa3befc …` = 18 files / +421 / −93 — this IS W0 … **NOT
empty**"* and scope every "empty/0/unchanged" word to `ceb8efd..fa3befc` (post-W0) or
to Cargo.lock. The legitimate TRUE claims ("zero Rust delta vs W0-clean `ceb8efd`",
"Cargo.lock byte-identical = 0-new-deps") are present and correct — and distinct from
the forbidden vs-v1.2 form. **The correction is accurate, not a new overstatement.**

### 1.2 Per-row §11.50–§11.62 verification (every cited commit `git show`-checked)

| Row | Claim (abridged) | Cited commit(s) | Independent verdict |
|---|---|---|---|
| §11.50 | W0 §11.49 paydown; parallel.rs sole full cst removal; 13-file R-1..R-4 residual; 0 new deps | W0 `ceb8efd`; audit `255a3e7` | **ACCURATE** — `ceb8efd` = the paydown (18f +421/−93); `parallel.rs` zero `cst::`; **exactly 13** fsm-analyzer files retain `use fsm_parser::cst`; `255a3e7` adds `AUDIT_PHASE_W0` (545 L); `7d6792a..ceb8efd` Cargo.lock empty |
| §11.51 | Toolchain-probe-trap: 1.75 pin IS honored; bare `rustc` 1.95 is benign | post-W0 §11.3 `255a3e7` | **ACCURATE** — `rust-toolchain.toml` byte-UNTOUCHED vs `71043935`; `channel = "1.75.0"` |
| §11.52 | Doc 27 §2.2 `TransportKind.stdio` wrong; omit-`transport` is shipped+correct | `AUDIT_PHASE_V1` `4280406`; V1 `6707d1d` | **ACCURATE** — `4280406` adds `AUDIT_PHASE_V1` (636 L); shipped `extension.ts:220-224` builds `Executable` with `transport` deliberately omitted |
| §11.53 | UTF-8 unreachable; correct negotiated = `"utf-16"`; test asserts it | `AUDIT_PHASE_V1` `4280406`; V1 `6707d1d` | **ACCURATE** — `extension.test.ts:194,206-207` asserts `negotiatedPositionEncoding === "utf-16"` |
| §11.54 | V1 spine decision (omit-transport, no-PATH-fallback, backoff) | V1 `6707d1d`; audit `4280406` | **ACCURATE** — `6707d1d` is the V1 commit; AUDIT_PHASE_V1 PROCEED-WITH-NOTES |
| §11.55 | V2 grammar (scope names verbatim, N-6 begin/end fix); V3 7 commands; N-5 closed | V2 `1542bfb` / V3 `3c49d38` (merge `61cc3f0`); audit `4aaef5d` | **ACCURATE** — `tmLanguage.json:6` `_note` documents the J-1 fix verbatim, scope names unchanged; package.json contributes the 7 core commands bare-title + `category:"FSM Studio"` |
| §11.56 | V4 diagram WebviewPanel; copyIr→emitIr single-seam extraction | V4 `4c4c6a2`; audit `203b7d5` | **ACCURATE** — only non-test `--emit-ir` invocation is `emitIr.ts:114`; no second resolver |
| §11.57 | V5/V6 + the 3 V6 owner-escalations | V5 `1db80cd` / V6 `fa3befc`; Doc28 `87ffebd` | **ACCURATE** — tree/ has zero emitIr/irGraph import; no bin/.vsix git-tracked at `03e2a14`; .gitignore L34/L42 cover them |
| §11.58 | JC-3 config-discoverability carried to owner wave | V2/V3-audit `4aaef5d`; V4-audit `203b7d5` | **ACCURATE** — recorded GATE §6.4, not overstated as resolved |
| §11.59 | JC-1 xvfb load-bearing for JS lane | V2/V3-audit `4aaef5d`; V4-audit `203b7d5` | **ACCURATE** — folded GATE §4.2 / §5.1 |
| §11.60 | copyIr→emitIr POSITIVE credit | post-V4 §11.3 `203b7d5` | **ACCURATE** — single-seam confirmed (see §11.56 row) |
| §11.61 | makeNonce Math.random→CSPRNG tracked v1.3.x | post-V4 §11.3 `203b7d5` | **ACCURATE** — recorded GATE §6.4 as tracked non-blocker |
| §11.62 | No-LICENSE owner/legal escalation | this closeout | **ACCURATE** — `git ls-tree fa3befc` has no LICENSE; recorded GATE §6.3 |

All 14 wave-commit SHAs in the brief's map resolve to the claimed subjects (verified via
`git log --oneline -1 <sha>` and `git log --follow --diff-filter=A` for Doc 28/29).

### 1.3 Frozen-artifact + structural integrity

| Check | Method | Result |
|---|---|---|
| GATE_VERIFICATION_v1_0/_v1_1/_v1_2 byte-untouched | `git diff --stat 71043935 03e2a14 -- <f>` | **UNTOUCHED** (all 3) |
| Doc 00 §11.1–§11.49 untouched (append-only) | `git diff --numstat … docs/00…md` = 14 ins / 1 del; the 1 del = old §11 footer line | **APPEND-ONLY** — 13 new rows + footer swap; §11.1-49 byte-intact |
| §11 footer = `§11.50-62` | grep footer line 1345 | reads `…rows §11.50-62 appended 2026-05-16, the v1.3 batched doc/ledger consolidation` ✅ |
| v1.1 metrics frozen | `git diff --stat … docs/metrics/2026-05-15-v1.1.0.json` + `…2026-05-15.json` | **UNTOUCHED** (both) |
| v1.2 metrics frozen | `2026-05-16.json` shows +281 vs `71043935` — **investigated**: file did NOT exist at `71043935`; introduced by `55ecc50` ("v1.2 **post-tag** metrics snapshot"); `55ecc50..03e2a14` over it = **empty** | **NOT A FINDING** — correct post-tag-snapshot pattern (same as v1.1); byte-untouched across the entire v1.3 epic |
| CHANGELOG `[1.3.0]` structural completeness | read lines 10-187 | Added (V1-V6) / Changed (W0 + emitIr) / Fixed (doc-of-record, "no code change") / Known limitations (G9, multi-platform, VSIX, LICENSE, JC-3, makeNonce, N-4) — **complete; no unshipped claim** |
| Closeout commit scope | `git show --stat 03e2a14` | **docs/metrics ONLY** (10 files: CHANGELOG, Doc 00/05/21/27/28/29, GATE_v1_3, ROADMAP, metrics v1_3) — zero crates/editors/source; `03e2a14^ == fa3befc` exactly |

---

## Lens 2 — v1.3 corpus structural soundness

| Keystone | Method | Result |
|---|---|---|
| W0 13-file R-1..R-4 residual present | `grep -rl 'use fsm_parser::cst' crates/fsm-analyzer/src` | **exactly 13 files** |
| W0 parallel.rs fully clean (sole Archetype-A removal) | `grep -n 'cst::' …/parallel.rs` | **empty** (zero cst refs) |
| W0 R-1..R-4 leave-and-explain comments (DRIFT-2 form) | grep `§11.44/§11.49` + `LineIndex` in fsm-analyzer | present across all residual files (e.g. `lower/state.rs:805,819,860`, `util.rs:19`, `symbol_table.rs:33`) |
| V1 omit-`transport` client | `extension.ts:220-224` | `const executable: Executable = {…}` with `transport` deliberately omitted + M-1 comment |
| V1 UTF-16 client assertion | `extension.test.ts:194,206` | `negotiatedPositionEncoding === "utf-16"` |
| V2 grammar scope-names verbatim + `_note` | `fsm-lang.tmLanguage.json:6` | `_note` documents J-1 begin/end fix verbatim, Doc 21 §2 scope names unchanged |
| V3 7 Doc 22 §4 commands | `package.json` contributes | 7 core commands, bare titles + `category:"FSM Studio"` (JC-4) |
| V4 emitIr/codegen-gated single seam | grep actual `--emit-ir` invocation | **only** `emitIr.ts:114` (no 2nd resolver); copyIr + diagram both delegate |
| V5 tree consumes documentSymbol (zero IR path) | `tree/index.ts:191` + import grep | `vscode.executeDocumentSymbolProvider`; **zero** emitIr/irGraph import (MV5-1 honoured) |
| V6 no binary/.vsix git-tracked at `03e2a14` | `git ls-tree -r 03e2a14 -- editors/vscode/bin` + `*.vsix` | **empty**; `.gitignore:34` `*.vsix` + `:42` `bin/` |
| Behavioural-acceptance §5.4-grade real | 4 frozen `AUDIT_PHASE_*` + GATE §4 | all 4 = **PROCEED-WITH-NOTES** "0 ship-blockers"; final §11.1 combined = **34/34** (GATE line 76) |
| `forbid(unsafe_code)` workspace-wide | grep crates/ | **11 attribute lines / 10 crates** (fsm-lsp = main.rs:16 + lib.rs:145) |

All keystones verified in **shipped source**, not from the docs' own claim. No
contradiction between shipped corpus and the §11.50-62 prose.

---

## Lens 3 — Gate criteria + no-P0 (every numeric independently re-derived)

| Gate numeric | Independent derivation | Value | Doc claim | Match |
|---|---|---|---|---|
| Live `DiagnosticCode` | counted `for_each_code!` macro entries `XNNNN => (Sev,` (lib.rs 262-386): 51 E + 12 W + 4 I + 6 H, all 73 distinct | **73** | 73 (const `EXPECTED:usize=73` lib.rs:820; GATE pinned-numbers) | ✅ |
| `#![forbid(unsafe_code)]` | `grep -rl crates/` = 11 files; distinct crate dirs = 10 | **11 roots / 10 crates** | 11/10 | ✅ |
| Extension-Host tests | `test(`/`it(` per suite: ext 4, grammar 7, commands 9, diagram 4, tree 6, bundled(V6) 4 | **34** | 34 (V1 4+V2 7+V3 9+V4 4+V5 6+V6 4) | ✅ |
| W0 = sole v1.3 Rust delta | `git diff --stat 71043935 03e2a14 -- crates/ Cargo.*` = 18f +421/−93; `ceb8efd..03e2a14` empty; Cargo.lock `71043935..03e2a14` empty | **18f +421/−93, 0 new deps** | 18f +421/−93, 0 new deps | ✅ |
| RUSTSEC-2026-0009 eliminated | `grep '^name = "time"' Cargo.lock` @ `03e2a14` → exit 1 (no `time` entry) | **`time` ABSENT** | eliminated, dependency surface unchanged | ✅ |
| Zero P0 | GATE §1/§7 "0 open P0"; 4 phase-audits PROCEED-WITH-NOTES; cross-checked vs shipped keystones | **0 P0** | 0 P0 | ✅ |

**Accepted-tracked-debt + owner-escalations — each honestly recorded (not dropped,
not overstated as resolved):**

| Item | Recorded at | Framing verdict |
|---|---|---|
| R-1..R-4 analyzer→CST residual | GATE §6.1, §11.50, CHANGELOG Changed | accepted-tracked-debt, "not a regression — the resolution" — **honest** |
| G9 multi-platform 5-platform tail | GATE §5.2, §6.2, CHANGELOG | "blocked-on-G9", "host-only" — **honest, not resolved** |
| VSIX packaged-not-published | GATE §5.3, §6.2, CHANGELOG | "installable not published", "no PAT sought" — **honest** |
| No-LICENSE file | GATE §6.3, §11.62, CHANGELOG | "no top-level LICENSE", owner/legal, non-blocking-for-local-tag — **honest** |
| JC-3 config-schema gap | GATE §6.4, §11.58, CHANGELOG | "not in `contributes.configuration`", defaults correct — **honest** |
| makeNonce CSPRNG hardening | GATE §6.4, §11.61, CHANGELOG | "Math.random", "tracked v1.3.x, not a ship issue" — **honest** |
| CI-matrix + JS-lane never-run | GATE §5.1, §6.2, CHANGELOG | "never executed against any v1.2 OR v1.3 commit" — **honest** |

---

## Lens 4 — Whole-corpus coherence / residual drift + tag-topology

| Check | Method | Result |
|---|---|---|
| Reconciliation pointers accurate + annotate-not-rewrite | `git diff --numstat fa3befc 03e2a14` per doc | Doc 05 +2/−0, Doc 21 +2/−0, Doc 27 +4/−0, Doc 28 +2/−0, Doc 29 +2/−0 — **pure additions, zero deletions** |
| Pointers are marked annotations citing frozen evidence | read Doc 27:86/132, Doc 05:274 | `⚠ v1.3-V1/V4 reconciliation note` blockquotes → Doc 00 §11.52/53/56 + frozen `AUDIT_PHASE_V1/V4`; original prose preserved beneath |
| Frozen `AUDIT_PHASE_*` evidence untouched | `git diff --stat fa3befc 03e2a14 -- AUDIT_PHASE_{W0,V1,V2V3,V4}` | **UNTOUCHED** (all 4); each frozen 0-commits-since-introduction at its §11.3 commit |
| No NEW doc↔code drift | Lens-2 keystone cross-check (shipped vs §11.50-62) | none found — every prose keystone matches shipped source |
| §11.30 tag-topology pre-emptively correct | GATE §8 line 161-162 | states cargo-quad ≡ JS-lane ≡ tag ≡ checkpoint ≡ `X` by construction; **records the annotated-tag-object-vs-commit distinction** ("`git rev-parse v1.3.0`=tag-OBJECT sha; `git rev-list -1 v1.3.0`=commit `X`") so a future wave does not misread it |
| `X` construction correct | `git rev-parse 03e2a14^` == `fa3befc` | exact — `X = fa3befc + docs/ledger-only closeout` (matches GATE claim) |

---

## Independent Derivations Table (proof I re-derived, did not echo)

| # | Derived fact | Exact command / source | Output | Echo-or-derived |
|---|---|---|---|---|
| D1 | v1.3 Rust delta = W0 | `git -C … diff --stat 71043935 03e2a14 -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | 18 files, +421/−93 (NOT empty) | derived |
| D2 | V1-V6 zero Rust | `git diff --stat ceb8efd 03e2a14 -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | empty | derived |
| D3 | 0 new deps | `git diff --stat 71043935 03e2a14 -- Cargo.lock` | empty | derived |
| D4 | W0==checkpoint | `git rev-parse ceb8efd checkpoint/2026-05-16-w0-clean` | both `ceb8efdd3acf…` | derived |
| D5 | No surviving overstatement | `grep -nE '(zero\|no\|empty\|unchanged\|0).{0,40}(rust).{0,40}(v1\.2\|71043935)\|…reverse' GATE_v1_3 + metrics_v1_3` minus Cargo.lock/0-dep | **0 matches** | derived |
| D6 | Live DiagnosticCode | count `XNNNN => (Sev,` in `for_each_code!` body, lib.rs 262-386, by severity | 51E+12W+4I+6H = **73** (73 distinct) | derived (not the const) |
| D7 | forbid(unsafe_code) | `grep -rl 'forbid(unsafe_code)' crates/` ; distinct crate dirs | **11 lines / 10 crates** | derived |
| D8 | Extension-Host tests | `grep -cE '(test\|it)\s*\(' ` per suite ×6 | 4+7+9+4+6+4 = **34** | derived |
| D9 | `time` absent (RUSTSEC-2026-0009) | `grep '^name = "time"' Cargo.lock` @ 03e2a14 | exit 1, no match | derived |
| D10 | W0 = exactly 13-file cst residual | `grep -rl 'use fsm_parser::cst' crates/fsm-analyzer/src` | 13 files | derived |
| D11 | parallel.rs clean | `grep -n 'cst::' crates/fsm-analyzer/src/checks/parallel.rs` | empty | derived |
| D12 | V4 single emit-ir seam | `grep -rn "'--emit-ir'" editors/vscode/src` minus comments/tests | only `emitIr.ts:114` | derived |
| D13 | V5 tree zero IR import | `grep -rn 'import.*emitIr\|import.*irGraph' editors/vscode/src/tree` | empty | derived |
| D14 | No bin/.vsix tracked @ X | `git ls-tree -r 03e2a14 -- editors/vscode/bin` + `… \| grep '\.vsix$'` | empty | derived |
| D15 | Doc 00 append-only | `git diff --numstat 71043935 03e2a14 -- docs/00…md` + grep deleted lines | 14 ins / 1 del (= old footer) | derived |
| D16 | Frozen GATE docs untouched | `git diff --stat 71043935 03e2a14 -- GATE_v1_0/_v1_1/_v1_2` | all empty | derived |
| D17 | v1.2 metrics file = post-tag, untouched in v1.3 | `git ls-tree 71043935 -- …2026-05-16.json` (empty) + `git log 55ecc50..03e2a14 -- …` (empty) | created `55ecc50`, 0 v1.3-epic edits | derived |
| D18 | Closeout = docs/metrics only | `git show --stat 03e2a14` | 10 docs/metrics files, 0 crates/editors | derived |
| D19 | X = fa3befc + closeout | `git rev-parse 03e2a14^` vs `fa3befc` | identical SHA | derived |
| D20 | rust-toolchain pin held | `git diff 71043935 03e2a14 -- rust-toolchain.toml` (empty); `channel` | UNTOUCHED, `1.75.0` | derived |
| D21 | AUDIT_PHASE_* frozen | `git diff --stat fa3befc 03e2a14 -- AUDIT_PHASE_{W0,V1,V2V3,V4}` | all empty | derived |

---

## Findings (severity P0 ship-blocker / P1 well-scoped / P2 minor / P3 nit)

| ID | Lens | Sev | Location (file:line / SHA) | Issue | Recommended action |
|---|---|---|---|---|---|
| F-01 | 1 | **P2** | `docs/metrics/2026-05-16-v1_3.json:76` | `"method": "Rust: unchanged vs frozen v1.2 (byte-identical workspace)"` — the bare phrase "byte-identical workspace" is locally imprecise (W0 changed 18 Rust files). It is **governed/contradicted in the same object** by `rust_note` (L74, explicitly states W0 +421/−93) + `interpretation` (L77) + the top-level `$comment` KEY FACT + the `v1_3_rust_delta_is_exactly_w0` block. Intended meaning = "the *LOC metric value* is carried from the frozen v1.2 attestation". NOT the cardinal overstatement (which was a headline erasure); this is a method-string shorthand. | Reword to `"Rust LOC value carried from the frozen v1.2 attestation (not rewritten); W0's +421/−93 acknowledged in rust_note"`. Non-gating — the object is internally self-correcting. |
| F-02 | 1/4 | **P3** | `docs/GATE_VERIFICATION_v1_3.md:48` (§2.2 header) | Sub-header `"The VS Code extension — V1–V6 (editors/vscode/, pure additive — zero Rust delta)"`. Scoped to the V1-V6 epic (genuinely zero-Rust: `ceb8efd..fa3befc` empty); §2.1 directly above is the W0 section documenting +421/−93, so in context unambiguous and TRUE. A stricter reader could want "(zero Rust delta vs W0-clean)". | Optional: append "vs W0-clean `ceb8efd`" to the header for symmetry with §1 G1. Non-gating. |
| F-03 | 1 | **P3** | `docs/GATE_VERIFICATION_v1_3.md:166` (Pinned numbers) | "macro body … (lines 262–403)" — the `=> (Sev,` entries actually span ~262–386 (macro closes before the 388+ table-build). Upper bound 403 is generous; the *count* (73) is exactly correct (independently re-derived D6). | Tighten the line range to 262–386. Cosmetic. |
| F-04 | 1 | **P3** | `docs/00-…md:1328` (§11.49) / `:1329` (§11.50) | §11.49 (carried from v1.2 era) says the coupling is "~15 files"; §11.50 reports the residual as 13 files + the clean removals. Not a contradiction (~15 = pre-paydown estimate; 13 = post-paydown residual after parallel.rs + ~7 accessor relocations), but a reader skimming sees "15" then "13". | None required — already explained in §11.50 prose. Note only. |
| F-05 | 4 | **P3** | `docs/GATE_VERIFICATION_v1_3.md:84-106` (§4.4) | The canonical cold-quad + JS lane are documented as **not-yet-run** ("These runs have not yet happened … the gating step the tag is contingent on"). This is the correct §11.30 prospective posture (honest, not a fabricated transcript) — but it means TAG-CLEAR from this audit is necessary-not-sufficient: the contingent quad/JS-lane at `X` must still be green before the actual tag. | Orchestrator must run the canonical cold cargo quad + JS Extension-Host lane (under `xvfb`, + `npm audit`) **at `03e2a14`** and only then place the annotated tag, per GATE §4.4 / §5. Not an audit defect — a release-sequence reminder. |
| F-06 | 2 | **P2** | `editors/vscode/package.json` (contributes.commands) | 10 `fsm.*` commands are contributed (7 Doc 22 §4 core + V4 `openDiagram` + V5 `refreshMachineExplorer`/`refreshEventExplorer`); §11.55 says "the 7 Doc 22 §4 commands". The extra 3 are correctly documented in §11.56/§11.57 (V4/V5), so no command is unaccounted-for, but §11.55's "7" read alone undercounts the shipped total. | None required — cross-row reading (§11.55+§11.56+§11.57) is complete. Documentation precision note. |
| F-07 | 1 | **P2** | `docs/ROADMAP.md` (closeout delta +11/−7) | ROADMAP got a small v1.3 status edit in the closeout with 7 deletions (not pure-append, unlike Doc 05/21/27/28/29). ROADMAP is an explicitly-living document ("How the roadmap evolves" mechanism) so in-place edits are by-design, NOT a frozen-doc violation; the frozen attestations (GATE_v1_0/1/2, AUDIT_PHASE_*, §11.1-49, v1.1/v1.2 metrics) are all byte-untouched (D15-D21). | None — ROADMAP is intentionally mutable. Flagged only to record the 7 deletions are roadmap-status churn, not a frozen-record rewrite. |

**No P0. No P1.** All findings are wording/precision/sequence notes. The corpus is
internally consistent and the shipped source matches the §11.50-62 record.

---

## Why this audit is not a rubber-stamp

A clean verdict on a corpus whose closeout already produced one cardinal overstatement
is itself suspect, so the load-bearing re-derivations are shown explicitly: the
**Rust-delta diff** (D1 = 18f +421/−93 NOT empty; D2/D3 = post-W0 + Cargo.lock empty),
the **adversarial overstatement grep** (D5 = 0 surviving forbidden hits, after
distinguishing the legitimate `ceb8efd`/0-dep claims), and the gate trio
**73 / 11-10 / 34** each counted from source (D6/D7/D8), not echoed from the const or
the doc. The one place a second overstatement could hide (metrics `loc.method`
"byte-identical workspace", F-01) was caught, traced, and judged a self-correcting
method-string shorthand — not a headline erasure — because every governing field in
the same object correctly states W0 IS the +421/−93 v1.3 Rust delta. The
Rust-delta-framing correction **held**.

---

## Mechanics

- Worktree: `/root/dev/embeded-fsm-sdk-wt-v13pretag` (manual, `git -C` only).
- Branch: `phase3.3/v1_3-pretag-audit` (off `03e2a14`).
- This doc is NEW (`docs/AUDIT_PRE_TAG_v1_3_2026_05_16.md`); no existing audit overwritten.
- Committed as `docs: v1.3 independent pre-tag four-lens audit`. **Not merged, not pushed, not tagged.**

*— End of independent pre-`v1.3.0`-tag four-lens audit, 2026-05-16.*
