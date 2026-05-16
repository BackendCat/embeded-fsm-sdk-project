# Independent Pre-`v1.4.0`-Tag Four-Lens Audit — 2026-05-16

**Document ID:** FSM-AUDIT-PRE-TAG-V14 (frozen evidence doc — never overwrite; a new audit is a new versioned file).
**Auditor role:** independent senior release auditor (adversarial, re-derive-don't-trust).
**Subject:** FSM Studio, `main` HEAD `3bbcd0f` (= the v1.4 release commit "X" = `3bbcd0f5d83cf43a88a700150648e2c5badc7f30` = `v1.4-W4b: batched closeout + GATE_VERIFICATION_v1_4 + metrics (the §11.30 gate-doc commit)`).
**Baselines:** v1.3.0 frozen at `a036c38` (== `git rev-list -1 v1.3.0` == `checkpoint/2026-05-16-v1_3`); W4a HEAD pre-gate at `4eb04dc` (`X^`).
**Mode:** READ-ONLY, zero `cargo`/`npm`/build, `git -C`-only (the binding disk-protection constraint — the box is at ~6.6G; W4d needs the disk for the canonical cold-quad; the v1.3-pre-tag posture is read+reason). Worktree `/root/dev/embeded-fsm-sdk-wt-v14pretag`, branch `phase4.4/v14-pretag-audit`, clean at `X` (verified no partial state — a prior attempt was lost to a transient infra error before producing anything; this audit was performed from scratch).
**Mandate context:** the v1.3 pre-tag audit caught a cardinal "zero Rust delta" understatement that would have entered the frozen record. v1.4's symmetric exposure is the *opposite* — a SUBSTANTIAL additive verification-core delta that must be neither under- nor over-stated, nor mis-scoped as "fsm-verify changed nothing." This audit assumes such an error may exist and re-derives every load-bearing number from source/`git show`, never echoing prior prose. A rubber-stamp here is worse than a found problem.

---

## Verdict

> **`TAG-CLEAR`** — **0 P0; 0 P1; 2 P2; 3 P3** (all precision/disclosure nits, none gating; the v1.3 TAG-CLEAR standard of ≤5 well-scoped P1 is met with zero P1).
>
> The three keystone invariants are **fully confirmed by my own derivation, not echoed**:
> 1. **The Rust-delta framing is correct** — the symmetric analogue of v1.3's cardinal error is stated correctly: a SUBSTANTIAL ADDITIVE delta (`git diff --shortstat a036c38 4eb04dc -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` = **51 files / +7398 / −6**, independently re-derived; at `X` itself = +7399 due to the disclosed comment-only `digest.rs` +1 line), **0 new external deps** (`+source =` count = **0**), neither under-stated (the v1.3 sin) nor over-stated; the adversarial "fsm-verify changed nothing / zero delta / mis-scope" grep = **0 hits**, and the v1.3 cardinal-error form ("zero/no/empty rust delta vs v1.3/a036c38") = **0 hits**.
> 2. **Doc 00 is append-only** — the sorted set-difference `comm -23 (old-§11-rows-sorted) (new-§11-rows-sorted)` = **EMPTY** (zero pre-existing §11.x rows removed/altered); the only deleted line across `a036c38..X` is the **single footer-line swap**; §11.63 is byte-identical (md5 `9e42645…`) across its net-zero relocation; §11.64–§11.71 are the W4b batch (absent at `4eb04dc`).
> 3. **The keystone holds at `X`** — every non-test `fn` in `crates/fsm-verify/src/**` re-enumerated at `X` is structural-IR-read or pure `fsm_simulator::Interpreter` orchestration; the negative-grep on non-comment lines = **EMPTY** (the single raw match is a `//!` doc-comment, read verbatim — not an echo branch); the honest-bound is structurally enforced (single `ProvenNoDeadlock` site on the exhausted exit; `unreachable!()` guards inconclusive-on-exhausted); the W4b `digest.rs` §13→§13.5 repoint changed **5 ins / 4 del, every line a `//`/`///`/`//!` comment** (`git diff -U0`), forking zero semantics.
>
> The only non-trivial nuance (F-01, **P2**) is that the GATE/metrics pin the Rust-delta at `4eb04dc` (+7398) while the gate-doc commit is `X` (+7399); this is **comprehensively pre-disclosed** in GATE §4.3 + the pinned-numbers header (both explicitly define the basis as "`4eb04dc` + the docs/ledger-only delta" and quantify the `digest.rs` comment-only +1) — it is the symmetric-opposite of v1.3's error done *correctly*, a disclosed measurement-basis choice, not a misstatement. Non-gating.

---

## Why this audit is not a rubber-stamp

A clean verdict on a corpus whose v1.3 closeout produced a cardinal overstatement is itself suspect, so the four load-bearing re-derivations are shown explicitly here (full table in §"Independent Derivations"):

- **The adversarial Rust-delta-framing greps.** Two adversarial patterns run over `GATE_VERIFICATION_v1_4.md` + `metrics/2026-05-16-v1_4.json`: (a) "fsm-verify … (zero|no|nothing|empty|unchanged) (delta|change)" and its reverse ⇒ **0 hits**; (b) the v1.3 cardinal-error form "(zero|no|nil|empty)…rust…delta" / "rust…(unchanged|empty|zero)…(v1.3|a036c38)" ⇒ **NO_MATCH** (`grep … || echo NO_MATCH_GOOD` ⇒ `NO_MATCH_GOOD`). Every "unchanged near rust" hit was read individually (D5 context dump): each is correctly scoped ("v1.4 adds no diagnostic code", "rust-toolchain.toml byte-unchanged", "0 known vulns", "does NOT carry unchanged for v1.4 … substantial ADDITIVE delta"). The delta is stated as **+7398/−6 across 51 files, additive, 0 new external deps** — the symmetric opposite of v1.3's error, correctly.
- **The Doc 00 append-only set-difference.** Not trusted from the metrics' "append-only verified" claim: `git show a036c38:…00….md | grep '^| §11\.[0-9]+ | ' | sort` vs the same at `3bbcd0f`, then `comm -23` ⇒ **EMPTY**. The full line-level `diff` old→new has **exactly 1 deleted line** = the footer swap (verbatim shown, D15). §11.63 md5-identical across relocation (D16).
- **The keystone re-enumeration at `X`.** Every `fn` in `fsm-verify/src/**` listed at `3bbcd0f` (not at `3447881` and not echoed from the W2 audit) — the non-test set matches the W2-audit enumeration exactly (engine.rs:81/173/253/489/525/569; deadlock.rs:99/136; digest.rs:112/147/167; reachability.rs:42/82; diagnostics.rs:50/96/105/141/172); the negative grep raw output read verbatim (1 match = a `//!` line); the honest-bound `engine.rs` region read in full at `X` (the single `ProvenNoDeadlock` site + the `unreachable!("inconclusive() is never called when exhausted")` guard); the `digest.rs` W4b touch proven comment-only by `git diff -U0 4eb04dc 3bbcd0f`.
- **The gate numerics from source at `X`.** Live `DiagnosticCode` = **73** by the GATE's own method (`sed -n '261,387p' … | grep -cE '^\s+[EWIH][0-9]{4}\s*=>\s*\('`) AND a whole-file cross-count (also 73) AND the by-severity bare-token split **51E/12W/4I/6H** (matching the metrics) AND the CI-lock `const EXPECTED: usize = 73`; conformance = **26** via `jq` with the MANIFEST byte-untouched `a036c38..X`; `forbid(unsafe_code)` = **12 roots / 11 crates** via `git grep -l` (fsm-verify the 12th root). None echoed.

The one place a second cardinal error could hide (the Rust-delta figure's commit basis, F-01) was caught, traced to GATE §4.3's explicit pre-disclosure, and judged a disclosed-measurement-basis nuance — not a headline misstatement — because the GATE itself defines the basis as "`4eb04dc` + the docs/ledger-only delta" and quantifies the comment-only `digest.rs` +1 that accounts for the +7398→+7399 difference. The Rust-delta framing **held, correctly, as the symmetric opposite of v1.3's error.**

---

## Lens 1 — Release-record integrity (the cardinal lens; independent pass)

### 1.1 The v1.4 Rust-delta framing — VERIFIED CORRECT (the symmetric-analogue check)

**Independent ground truth (re-derived, not echoed):**

| Claim under test | Command | Result |
|---|---|---|
| v1.4 Rust delta vs v1.3 tag (the GATE/metrics-cited basis) | `git diff --shortstat a036c38 4eb04dc -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | **51 files, +7398 / −6** — NOT empty, NOT zero (substantial additive) |
| Same at the gate-doc commit `X` | `git diff --shortstat a036c38 3bbcd0f -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | **51 files, +7399 / −6** (+1 vs `4eb04dc` = the disclosed comment-only `digest.rs` line) |
| Per-crate breakdown (`a036c38..4eb04dc`) | `git diff --shortstat … -- crates/fsm-verify/` … | fsm-verify **17f/+3042** (src 6f/+1695); fsm-simulator **6f/+684/−6**; fsm-cli **26f/+3657** — matches GATE/metrics exactly |
| 0 new external deps | `git diff a036c38 4eb04dc -- Cargo.lock \| grep -c '^+source = '` | **0** (also 0 at `a036c38..3bbcd0f`) |
| `a036c38` IS the v1.3.0 commit | `git rev-list -1 v1.3.0` | `a036c381…` (== `a036c38`) — the corpus-provenance equivalence argument's anchor is real |
| `X = 4eb04dc + docs/ledger-only` | `git rev-parse 3bbcd0f^` vs `4eb04dc` | identical SHA; `git show --stat 3bbcd0f` = 10 docs/metrics files + the comment-only `digest.rs` (no other crates/) |

**Adversarial overstatement greps** (both over `GATE_VERIFICATION_v1_4.md` + `metrics/2026-05-16-v1_4.json`): (a) "fsm-verify … zero/no/nothing/empty/unchanged delta/change" + reverse ⇒ **0 hits**; (b) "(zero|no|nil|empty)…rust…delta" / "rust…(unchanged|empty|zero|nil)…(v1\.3|a036c38)" ⇒ **NO_MATCH**. Every "unchanged near rust" sentence read individually = correctly scoped (D5). The forbidden v1.3-cardinal form is **absent**; the delta is everywhere stated as a **substantial additive** delta (the symmetric opposite of v1.3's understatement), with 0 new external deps. **The framing is accurate, not a new cardinal error.**

The `4eb04dc`-vs-`X` +1 (F-01, P2) is comprehensively **pre-disclosed**: GATE §4.3 line 93 — *"`X = 4eb04dc + a docs/ledger-only delta` … the **comment-only** `digest.rs` §13→§13.5 repoint … 5 ins / 4 del, zero code … it cannot change cargo build/test/clippy/fmt"* — and the pinned-numbers header line 152 explicitly bases the figure on "`4eb04dc` + the docs/ledger-only delta." This is a disclosed measurement-basis choice (the gate-doc commit's only crates/ delta vs `4eb04dc` is non-substantive), not the v1.3-class headline erasure.

### 1.2 Per-row §11.64–§11.71 verification (every cited commit `git show`-checked)

| Row | Claim (abridged) | Cited commit(s) | Independent verdict |
|---|---|---|---|
| §11.63 | v1.4 scope-confirm + `fsm-verify`-lib/CLI-canonical/no-server-no-plugin + pipeline-before-UI (appended PRE-W1) | `3e0aac3` / merge `118fee2` | **ACCURATE** — both touch only `docs/00…md` (3 lines) + `docs/30…md` (58 lines); §11.63 present at `4eb04dc` (count 1), md5-identical at `X` |
| §11.64 | W1 keystone: NEW `fsm-verify` crate driving Interpreter; §4.3 sub-brief superseded; CLI+E0400/W0602 in W1; `Vec<String>` witness; forbid 12/11 | `2866821` / merge `dd2d41e`; W1-audit `a75cd69` | **ACCURATE** — `2866821` adds `crates/fsm-verify/{Cargo.toml,src/{lib,engine,deadlock,digest,diagnostics,reachability}.rs}` + `fsm-cli/src/cmd/verify.rs` (326) + `cli_verify.rs` (234) + acceptance fixtures; matches "keystone + CLI + E0400/W0602" exactly; forbid 12/11 re-derived (§3) |
| §11.65 | W2 load-bearing snapshot-completeness P0 + hierarchy breadth; W1-audit D-2 "complete state" closed in code; Doc 08 §13.5 soundness home | `efae559` (P0) + `3447881` (breadth) | **ACCURATE** — `efae559` = `runtime/state.rs`(+160)/`timer.rs`(+67)/`interpreter.rs`(+22)/`snapshot_lossless.rs`(435); `3447881` = composite/parallel/timer/submachine fixtures + `w2_hierarchy_acceptance.rs`(485); Doc 30 §4.1 D-2 correction note present (line 194); Doc 08 §13.5 added (§4) |
| §11.66 | TWO-audit §11.3 cadence (post-W1 keystone `a75cd69` + post-W2 breadth), re-sequenced | `a75cd69` (+ W2 audit doc) | **ACCURATE** — `a75cd69` adds only `AUDIT_PHASE_V1_4_W1_2026_05_16.md` (197); both audit docs frozen (§1.3); Doc 30 §4.2 two-audit note present (line 203) |
| §11.67 | W3 `fsm baseline` sibling subcommand; corpus = capture-from-current-known-good NOT `a036c38`; `a036c38`=v1.3.0 | `fc88ec1` | **ACCURATE** — `fc88ec1` adds `cmd/baseline.rs`(653)+`cli_baseline.rs`(651)+`w3_baseline_corpus/` (6 `.fsm.json`); `baseline.rs` consumes `execute_trace`/`matches_expected`/`first_mismatch` (§2); design-intent-equivalence argument is honest, no fabricated `a036c38` provenance |
| §11.68 | W4a FACTORY-COMPLETE; §3.G Doc-18 doc-discoverability deferred→folded | `4eb04dc` | **ACCURATE** — `4eb04dc` adds the W4a audit doc (180) + `examples/verify/{clean,deadlocks}.fsm`/README + Doc 25 §9 (108, append-only); W4a verdict re-read = FACTORY-COMPLETE; §3.G folded into Doc 18 §3.1 (§4) |
| §11.69 | W4b single batched closeout = the §11.30 gate-doc commit; 7-item carry-list folded; W1-audit path-correction in ledger NOT by overwrite | `3bbcd0f` (this commit) | **ACCURATE** — `git show --stat 3bbcd0f` = the 10-file docs/metrics batch + comment-only `digest.rs`; all 7 carry-list items folded (§1.4); frozen W1 audit byte-untouched |
| §11.70 | NOTE-1 hardening — mandatory `cargo clean`/non-shared `CARGO_TARGET_DIR` Step 0; binding v1.4+ | operational (W2-audit F11) | **ACCURATE** — written verbatim into `GATE_VERIFICATION_v1_4.md` §4.2 MANDATORY STEP 0 (read in full) |
| §11.71 | v1.4 accepted-tracked-debt (R7 named DEFERRAL) + carried v1.3 owner-escalations | doc-carry | **ACCURATE** — R7 + G7/G9/LICENSE/VSIX/makeNonce/JC-3 all recorded in GATE §6 + CHANGELOG `[1.4.0]` Known limitations (§1.3) |

All cited wave-commit SHAs resolve to the claimed subjects (`git log --format='%H %s' -1 <sha>` + `git show --stat`).

### 1.3 Frozen-artifact + structural integrity

| Check | Method | Result |
|---|---|---|
| `GATE_VERIFICATION_v1_{0,1,2,3}` byte-untouched | `git diff --shortstat a036c38 3bbcd0f -- <f>` | **UNTOUCHED** (all 4 empty) |
| Frozen v1.4 phase-audits untouched since intro | `git log --oneline <intro>..3bbcd0f -- <f>` | `AUDIT_PHASE_V1_4_W1` (intro `a75cd69`), `_W2` (intro `3df275c`), `AUDIT_FACTORY_INTEGRATION_V1_4` (intro `4eb04dc`) — **0 commits-since-intro** (all) |
| Prior metrics frozen | `git diff --shortstat a036c38 3bbcd0f -- <f>` | `2026-05-15.json`, `2026-05-15-v1.1.0.json`, `2026-05-16.json`, `2026-05-16-v1_3.json` — **UNTOUCHED** (all) |
| `AUDIT_PRE_TAG_v1_3` itself untouched | same | **UNTOUCHED** |
| Doc 00 append-only | `git diff --numstat a036c38 3bbcd0f -- docs/00…md` = **+10/−1**; full `diff` deleted-lines = **1** (the footer) | **APPEND-ONLY** — 9 new rows (§11.63 + §11.64-71) + footer swap; the **sorted set-difference (old\new) = EMPTY** |
| §11.63 byte-preserved across relocation | md5 of `^\| §11\.63 \|` row @ `4eb04dc` vs @ `3bbcd0f` | both `9e4264523dd2e93fb5dfefd572e81a8f` — **byte-identical** |
| New footer correct | read footer @ `X` | records "row §11.63 appended … the v1.4 owner scope-confirm …; rows §11.64-71 appended … the v1.4 batched doc/ledger closeout" ✅ |
| Doc 30 annotate-not-rewrite | `git diff --numstat 4eb04dc 3bbcd0f -- docs/30…md` | **+17 / −0** — pure annotation, 6 reconciliation-note blockquotes, zero original prose removed |
| Doc 08 §13.5 pure-additive | `git diff --numstat 4eb04dc 3bbcd0f -- docs/08…md` | **+89 / −0** — §13.5 is the LAST §13.x (after the previously-terminal §13.4, before §14); absent at `4eb04dc` |
| Doc 18 §3.1 pure-additive | `git diff --numstat 4eb04dc 3bbcd0f -- docs/18…md` | **+90 / −0** — §3.1 verdict-family table; closes the W4a §3.G finding |
| Doc 10 append-only | `git diff --numstat 4eb04dc 3bbcd0f -- docs/10…md` | **+4 / −0** — E0400/W0602 "IMPLEMENTED in v1.4" annotation, proof-gated/structural distinction recorded |
| CHANGELOG `[1.4.0]` structurally complete | read block | Added / Changed / Fixed / Known limitations; R7 named-deferral, G7 (26 fixtures, behavioural-acceptance), G9 (CI/JS never-run), LICENSE, multi-platform — **complete; no unshipped claim** |
| Closeout commit scope | `git show --stat 3bbcd0f` | **10 docs/metrics + the comment-only `digest.rs` ONLY** (CHANGELOG, Doc 00/08/10/18/30, GATE_v1_4, ROADMAP, metrics v1_4, digest.rs 9-line comment repoint); zero substantive crates/editors |

### 1.4 The 7 carry-list items (W2-audit §5) — each verifiably folded

| # | Carry-list item | Independent verdict (file:line) |
|---|---|---|
| 1 | Doc 30 §4.2-W1/§4.3 CLI+E0400/W0602-in-W1 + `Vec<String>` witness; §4.2-W3 corpus provenance | **FOLDED** — Doc 30 §4.2-W1 note (`30…md:210`), §4.3 note (`:234`), §4.2-W3 notes (`:218,:221`), §4.2-W1·ii (`:214`); all "original prose preserved" |
| 2 (load-bearing) | Doc 30 §4.1 "complete state" D-2 correction | **FOLDED** — `30…md:194` reconciliation note ("THE LOAD-BEARING correction; carry-list item 2 / W2-audit F5 / W1-audit D-2; Doc 00 §11.65"); original prose at `:192` preserved |
| 3 | W1-audit path-citation (`runtime/state.rs`) recorded in ledger, NOT by overwrite | **FOLDED** — Doc 00 §11.69 records the `runtime/state.rs` correction; frozen W1 audit byte-untouched (§1.3) |
| 4 | digest.rs "Doc 08 §13" → new Doc 08 §13.5 lemma + comment-only repoint | **FOLDED** — Doc 08 §13.5 lemma added (normative, +89/−0); `digest.rs` cites "§13.5" at lines 23/62/73/246; the repoint = 5 ins/4 del all comment (`git diff -U0`) |
| 5 | `fsm-verify/vN` factory-contract versioning + §3.G → Doc 18 §3.1 | **FOLDED** — Doc 18 §3.1 "Verdict subcommands … 0/1/2/3/4 verification-verdict exit-code family" (+90/−0), exit-2-is-INCONCLUSIVE-never-a-pass instruction |
| 6 | v1.4 §11 impl rows + Doc 30 §4.2/§4.3 + two-audit cadence | **FOLDED** — §11.64-71 appended; Doc 30 §4.2 preamble/§4.2-W2 two-audit note (`30…md:203`) |
| 7 | NOTE-1 → mandatory `cargo clean`/fresh-target in the §11.30 tag-gate | **FOLDED** — GATE §4.2 MANDATORY STEP 0 (binding v1.4+) + Doc 00 §11.70 |

---

## Lens 2 — v1.4 corpus structural soundness (the keystone, re-derived at `X`)

| Keystone | Method (at `3bbcd0f`) | Result |
|---|---|---|
| Every `fn` in `fsm-verify/src` structural-read or pure Interpreter-orchestration | `git grep -nE '^\s*(pub…)?fn …'` per file at `X`; non-test set classified | **PASS** — non-test set = engine.rs:81 `min_armed_expiry`(.min over `expiry_ms`, fires nothing), :173 `bound_hit`, :253 `verify`(every successor = `interp.dispatch`/`advance_clock`; backtrack = `interp.restore`), :489 `inconclusive`, :525 `select_machine`(IR `.find`), :569 `reject_unsupported`(no-op seam); deadlock.rs:99/136 (IR final-state discriminators); digest.rs:112/147/167 (canonicalisation/hash, no selection); reachability.rs:42/82 (set arithmetic over the oracle's observed set); diagnostics.rs:50/96/105/141/172 (E0400/W0602 build + IR shape reads). **Exactly the W2-audit enumeration at `3447881`** — unchanged at `X` (the only `crates/` delta `4eb04dc..X` is the comment-only `digest.rs`) |
| Negative grep (no forked semantics), non-comment lines | `git grep -nE 'select_transition\|enabled_set\|eval_guard\|compute_lca\|find_lca\|completion_drain\|fire_timer\|join_complete\|region_join' 3bbcd0f -- 'crates/fsm-verify/src/*.rs'` → **raw output read verbatim** | **1 raw match**, read in full: `deadlock.rs:12://! - Doc 08 §3.1: ` — a `//!` doc-comment (prose, not code). After stripping `//`/`///`/`//!`: **EMPTY**. (No `head`/`echo` branch — the raw match is shown, not an echoed "OK"; the brief's false-positive class is avoided) |
| W4b `digest.rs` §13→§13.5 repoint = comment-only, forks zero semantics | `git diff -U0 4eb04dc 3bbcd0f -- crates/fsm-verify/src/digest.rs` | **5 ins / 4 del, every changed line `//!`/`///`/`//`** (the four §13→§13.5 citation edits + the lemma-name expansion). `digest.rs` now cites "Doc 08 §13.5" coherently with the new lemma. Zero code change ⇒ semantics unforked |
| `fsm baseline` consumes `execute_trace`/`matches_expected`/`first_mismatch`, forks no comparison | `git grep -nE 'execute_trace\|matches_expected\|first_mismatch' 3bbcd0f -- crates/fsm-cli/src/cmd/baseline.rs` | `baseline.rs:366,467` call `execute_trace(&ir, …)`; `:475` reads `result.matches_expected`; `:484` reads `result.first_mismatch`; `:466` comment "is the interpreter crate's, not re-implemented" — **B-seam keystone holds** |
| Honest-bound structurally enforced (held through W3+W4) | read `engine.rs` 450–525 at `X` in full | **PASS** — `Verdict::ProvenNoDeadlock` constructed at **exactly one site**, only after the frontier exhausts naturally with `stop_reason: StopReason::Exhausted` hard-coded; `Verdict::Deadlock` also hard-codes `Exhausted` but is a definite property-violation walked-to (correct); `inconclusive()` has `StopReason::Exhausted => unreachable!("inconclusive() is never called when exhausted")` — `Inconclusive`-with-`Exhausted` / `ProvenNoDeadlock`-on-truncation is **structurally unconstructible** |
| E0400 exhaustive-gated (the dual) | read `diagnostics.rs` 50–75 at `X` | `let exhaustive = matches!(stop_reason, StopReason::Exhausted); if exhaustive { …emit E0400… }` — E0400 proof-gated; W0602 unconditional structural always-safe subset |
| `#![forbid(unsafe_code)]` = 12 roots / 11 crates from source | `git grep -l '#!\[forbid(unsafe_code)\]' 3bbcd0f -- 'crates/*/src/*.rs'` | **12 root files / 11 distinct crates** (fsm-lsp = lib.rs+main.rs ⇒ 2 roots/1 crate; `fsm-verify/src/lib.rs` the 12th root / 11th crate) — matches GATE/metrics exactly |

All keystones verified in **shipped source at `X`**, not from the docs' own claim. No contradiction between the shipped corpus and the §11.64–§11.71 prose. The W2-audit's independent clock-merge bisimulation derivation is **not re-litigated here** (it is frozen evidence; this audit confirms its conclusion is now given a normative spec home at Doc 08 §13.5 — §4).

---

## Lens 3 — Gate criteria + no-P0 (every numeric independently re-derived from source at `X`)

| Gate numeric | Independent derivation at `3bbcd0f` | Value | Doc claim | Match |
|---|---|---|---|---|
| Live `DiagnosticCode` | the GATE's method `sed -n '261,387p' fsm-diagnostics/src/lib.rs \| grep -cE '^\s+[EWIH][0-9]{4}\s*=>\s*\('` = **73**; whole-file cross-count of all such arms = **73**; by-severity bare-token `(Error\|Warning\|Info\|Hint` = **51E/12W/4I/6H**; CI-lock `const EXPECTED: usize = 73` (lib.rs:820) | **73** | 73 (GATE pinned; metrics `by_severity` 51/12/4/6) | ✅ |
| GATE's *reasoning* for equality with v1.3 | the macro body at `X` contains E0400/W0602 *catalog* arms pre-v1.4 (count was 73 at v1.3); v1.4 added an *emission site* (`diagnostics.rs`) not a code — `git diff --shortstat a036c38 3bbcd0f -- tests/conformance/MANIFEST.json` empty corroborates "no code added" | reasoning sound | "unchanged because catalogued pre-v1.4, gained emission site" | ✅ |
| Conformance fixtures | `jq '[.categories[].fixtures\|length]\|add' tests/conformance/MANIFEST.json` | **26** (parser 6 + validator 6 + semantic 7 + codegen-c 4 + formatter 3) | 26 | ✅ |
| Conformance MANIFEST byte-untouched | `git diff --shortstat a036c38 3bbcd0f -- tests/conformance/MANIFEST.json` | **empty** | byte-untouched (no formal E0400/W0602/reach fixture) | ✅ |
| `#![forbid(unsafe_code)]` | `git grep -l … 3bbcd0f -- 'crates/*/src/*.rs'` ⇒ 12; `… \| sed crate \| sort -u \| wc -l` ⇒ 11 | **12 / 11** | 12 / 11 (correctly increments from v1.3 11/10) | ✅ |
| v1.4 Rust delta | `git diff --shortstat a036c38 4eb04dc …` ⇒ 51f/+7398/−6; `… a036c38 3bbcd0f …` ⇒ 51f/+7399/−6; `+source =` ⇒ 0 | **51f / +7398 / −6 @ `4eb04dc`; 0 new ext deps** | 51f / +7398 / −6; 0 new ext deps | ✅ (F-01 P2: +7399 @ `X`, pre-disclosed) |
| §11 rows total | `git show 3bbcd0f:…00….md \| grep -cE '^\| §11\.[0-9]+ \| '` | **71** | 71 | ✅ |
| Zero P0 | GATE §1/§7 "0 open P0"; 2 §11.3 audits PROCEED-WITH-NOTES + W4a FACTORY-COMPLETE (all frozen, re-read); cross-checked vs shipped keystones (Lens 2) | **0 P0** | 0 P0 | ✅ |

**W3 baseline-corpus provenance reconciliation** — the GATE/Doc-30 framing is the **honest design-intent-equivalence argument, not a fabricated `a036c38` provenance**: `a036c38` IS the v1.3.0 commit (`git rev-list -1 v1.3.0` independently confirmed); the shipped corpus is `crates/fsm-cli/tests/fixtures/w3_baseline_corpus/` (6 `.fsm.json`), gated by `committed_corpus_matches_a_fresh_capture_on_this_build` (Doc 30 §4.2-W3 note `:221`); the equivalence argument ("the v1.4 Rust delta vs v1.3 is *additive*, the simulator's flat/legacy behaviour byte-unregressed ⇒ current-known-good capture IS v1.3 semantics by construction") is explicitly argued, with "no `a036c38` provenance is fabricated" stated. Honest.

**W4a transcript consistency** (read `verify.rs`/`baseline.rs` exit-code maps at `X`, NOT re-run):

| Code | `verify.rs` (source @ `X`) | `baseline.rs` (source @ `X`) | W4a claimed | Consistent? |
|---|---|---|---|---|
| 0 | `ProvenNoDeadlock` → `"verified"` SUCCESS (verify.rs:182,187) | `"no-drift"` → SUCCESS (baseline.rs:522) | A=0 verified; G=0 no-drift | ✅ |
| 1 | `Deadlock`/`ProvenNoDeadlock if has_e0400` → `"property-violated"` `from(1)` (:181,183,188) | `"drift"` → `from(1)` (:518) | B=1 property-violated (non-empty witness) | ✅ |
| 2 | `Inconclusive` → `"inconclusive"` `from(2)` (:184,189) | `"inconclusive"` → `from(2)` (:520) | C=2 inconclusive (`bound.hit==true`, reachability.result="inconclusive") | ✅ |
| 3 | IO/read error → `from(3)` (:122) | IO → `from(3)` (:194,205,…) | D1=3 IO (missing path) | ✅ |
| 4 | won't-compile/out-of-scope → `from(4)` (:139,146,226,230) | out-of-scope → `from(4)` (:301,308,341,471) | D2=4 won't-compile | ✅ |

W4a's claimed A–I exit codes are **fully consistent with the documented in-source exit-code contract**. The §3.G finding (Doc 18 §3 generic table maps exit 2 → "tool error"; the per-subcommand INCONCLUSIVE superset diverges) is correctly characterised as a **doc-discoverability** finding (not a code defect — the owner-mandated 0/1/2/3/4 family is correct per Doc 30 §4.2-W4/§5.2) and is **folded into Doc 18 §3.1 by W4b** (Lens 4). **No inconsistency to flag for W4d.**

**Accepted-tracked-debt — each honestly recorded (not dropped, not overstated as resolved):** R7 (mechanised sim≡codegen — explicit NAMED DEFERRAL, GATE §6.1 / §11.71 / CHANGELOG); G7 (26 formal fixtures, FSM-E0400/W0602 via fsm-verify acceptance — §5.4-analogue, GATE G7); G9 (CI/JS-lane never-run, GATE §5.1/G9); 5-platform tail, VSIX, LICENSE, makeNonce, JC-3 (GATE §6.3 carried). All cross-checked present + honestly framed.

---

## Lens 4 — Whole-corpus coherence + tag-topology

| Check | Method | Result |
|---|---|---|
| No new doc↔code drift from the W4b batch | Lens-2 keystone cross-check (shipped @ `X` vs §11.64-71) + the comment-only `digest.rs` proof | **none** — every prose keystone matches shipped source; the only crates/ touch is the comment-only `digest.rs` repoint |
| digest.rs↔Doc 08 §13.5 citation now coheres | read Doc 08 §13.5 body (`:608-699`) + `digest.rs` citations at `X` | **COHERES** — Doc 08 §13.5 "Absolute-Virtual-Clock Non-Observability (lemma)" is a NEW, **normative**, purely-additive subsection after the previously-terminal §13.4; it states formally the non-observability property (no FSM-Lang construct reads `virtual_clock_ms`; only relative timer phase is significant; clock-shift-related configs are strongly bisimilar) with the structural proof basis (Doc 04 §1.5/§8.5/§8.7/§9 + §13.1/§13.3/§13.4); `digest.rs:23,62,73,246` now cite "§13.5". The citation points at a **stated** property (was an *entailed-but-unstated* one — W2-audit F8/NOTE-2 closed) |
| §3.G fold coherent | Doc 18 §3.1 read | Doc 18 §3.1 authoritatively documents the sanctioned per-subcommand 0/1/2/3/4 verdict superset with the exact "treat exit 2 as failure-to-prove (INCONCLUSIVE) — never a pass, never a blind retry" instruction; Doc 18 remains the single authoritative source, now correctly including it |
| §11.30 tag-topology pre-emptively correct in GATE §8 | read GATE §8 + §4.2/§4.4 | **CORRECT** — GATE §8 states cold-quad-commit ≡ tag-commit ≡ `checkpoint/2026-05-16-v1_4` ≡ `X` by construction; §4.2 MANDATORY STEP 0 (`cargo clean`/non-shared `CARGO_TARGET_DIR`) is the binding pre-quad precondition (NOTE-1, binding v1.4+); §4.4 explicitly states the run "has not yet happened … the gating step the tag is contingent on" (honest prospective posture, no fabricated transcript) |
| `git rev-parse` vs `git rev-list -1` annotated-tag-object distinction recorded | `git rev-parse v1.2.0`/`v1.3.0` vs `git rev-list -1 …` | **RECORDED + re-confirmed** — v1.2.0 tag-object `3b37db7c…` ≠ commit `71043935…`; v1.3.0 tag-object `a24c195b…` ≠ commit `a036c381…`; GATE §8 line 149 + metrics `tag_object_vs_commit_note` record this so no future wave misreads the `v1.4.0` tag-object sha as a mismatched commit |
| `v1.4.0` / `checkpoint/2026-05-16-v1_4` not yet placed | `git tag -l 'v1.4.0' 'checkpoint/2026-05-16-v1_4'` | **empty** — correct: the tag is the contingent step gated on this audit (TAG-CLEAR) + W4d's cold-quad at `X` |

---

## Independent Derivations Table (proof I re-derived, did not echo)

| # | Derived fact | Exact command / source | Output | Echo-or-derived |
|---|---|---|---|---|
| D1 | v1.4 Rust delta @ `4eb04dc` (cited basis) | `git -C … diff --shortstat a036c38 4eb04dc -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | 51 files, +7398 / −6 | DERIVED |
| D2 | v1.4 Rust delta @ `X` | `git diff --shortstat a036c38 3bbcd0f -- crates/ Cargo.toml Cargo.lock rust-toolchain.toml` | 51 files, +7399 / −6 (+1 = disclosed comment `digest.rs`) | DERIVED |
| D3 | 0 new external deps | `git diff a036c38 4eb04dc -- Cargo.lock \| grep -c '^+source = '` | 0 (also 0 @ `..3bbcd0f`) | DERIVED |
| D4 | per-crate breakdown | `git diff --shortstat a036c38 4eb04dc -- crates/fsm-verify/{,src/}` etc. | fsm-verify 17f/+3042 (src 6f/+1695); fsm-simulator 6f/+684/−6; fsm-cli 26f/+3657 | DERIVED |
| D5 | No surviving overstatement | grep (a) "fsm-verify…zero/no/empty delta"+rev ⇒ 0; (b) "(zero\|no\|nil\|empty)…rust…delta" / "rust…(unchanged\|empty\|zero)…(v1\.3\|a036c38)" ⇒ `NO_MATCH_GOOD`; every "unchanged near rust" hit read individually | 0 hits / NO_MATCH | DERIVED |
| D6 | `a036c38` = v1.3.0 commit | `git rev-list -1 v1.3.0` | `a036c381…` | DERIVED |
| D7 | `X = 4eb04dc + docs/ledger` | `git rev-parse 3bbcd0f^` vs `4eb04dc`; `git show --stat 3bbcd0f` | identical SHA; 10 docs/metrics + comment-only digest.rs | DERIVED |
| D8 | digest.rs W4b touch comment-only | `git diff -U0 4eb04dc 3bbcd0f -- crates/fsm-verify/src/digest.rs` | 5 ins / 4 del, every line `//!`/`///`/`//` | DERIVED |
| D9 | Live `DiagnosticCode` | `sed -n '261,387p' …lib.rs \| grep -cE '^\s+[EWIH][0-9]{4}\s*=>\s*\('`; whole-file cross-count; by-severity bare-token | 73; 73; 51E/12W/4I/6H; const EXPECTED=73 | DERIVED (not the const alone) |
| D10 | Conformance fixtures | `jq '[.categories[].fixtures\|length]\|add' tests/conformance/MANIFEST.json` @ `3bbcd0f` | 26 (6/6/7/4/3) | DERIVED |
| D11 | MANIFEST byte-untouched | `git diff --shortstat a036c38 3bbcd0f -- tests/conformance/MANIFEST.json` | empty | DERIVED |
| D12 | forbid(unsafe_code) | `git grep -l '#![forbid(unsafe_code)]' 3bbcd0f -- 'crates/*/src/*.rs'` ; distinct crates | 12 roots / 11 crates | DERIVED |
| D13 | Keystone fn-set @ `X` | `git grep -nE '^\s*(pub…)?fn …' 3bbcd0f -- crates/fsm-verify/src/*.rs` | non-test set = W2-audit enumeration exactly | DERIVED |
| D14 | Negative grep (non-comment) | `git grep -nE 'select_transition\|enabled_set\|…' 3bbcd0f -- 'crates/fsm-verify/src/*.rs'` → raw read | 1 raw match = `deadlock.rs:12 //!` doc-comment; non-comment = EMPTY | DERIVED (raw lines read, no echo branch) |
| D15 | Doc 00 append-only / footer-only deletion | `git diff --numstat a036c38 3bbcd0f -- docs/00…md`; full `diff` deleted-lines | +10/−1; deleted = 1 (the footer line, verbatim) | DERIVED |
| D16 | Doc 00 set-difference EMPTY | `comm -23 <rows@a036c38-sorted> <rows@3bbcd0f-sorted>` | EMPTY (zero pre-existing §11.x row removed/altered) | DERIVED |
| D17 | §11.63 byte-preserved | md5 of `^\| §11\.63 \|` @ `4eb04dc` vs @ `3bbcd0f` | both `9e4264523dd2e93fb5dfefd572e81a8f` | DERIVED |
| D18 | §11.64-71 are the W4b batch | `git show 4eb04dc:…00….md \| grep -cE '^\| §11\.(6[4-9]\|7[01]) \| '` | 0 (absent at `4eb04dc`) | DERIVED |
| D19 | Frozen GATE/audits/metrics untouched | `git diff --shortstat a036c38 3bbcd0f -- <f>` ×; `git log <intro>..3bbcd0f -- <f>` | all empty / 0 commits-since-intro | DERIVED |
| D20 | Doc 08/18/30/10 fold shapes | `git diff --numstat 4eb04dc 3bbcd0f -- docs/{08,18,30,10}…md` | 89/0, 90/0, 17/0, 4/0 (additive / annotate-not-rewrite) | DERIVED |
| D21 | Honest-bound structural @ `X` | read `engine.rs` 450–525 | single `ProvenNoDeadlock` site on exhausted exit; `unreachable!()` guards inconclusive-on-exhausted | DERIVED |
| D22 | E0400 exhaustive-gated @ `X` | read `diagnostics.rs` 50–75 | `if exhaustive { …E0400… }`; W0602 unconditional | DERIVED |
| D23 | baseline consumes the seam | `git grep -nE 'execute_trace\|matches_expected\|first_mismatch' 3bbcd0f -- …baseline.rs` | :366/:467 call execute_trace; :475/:484 read matches_expected/first_mismatch | DERIVED |
| D24 | verify/baseline exit-code maps | read `verify.rs` :122,139,146,181-189,226,230 / `baseline.rs` :518,520,522,3xx,4xx | 0/1/2/3/4 family exactly as W4a claimed | DERIVED |
| D25 | annotated-tag distinction | `git rev-parse v1.2.0`/`v1.3.0` vs `git rev-list -1 …` | tag-object ≠ commit (both pairs) | DERIVED |
| D26 | tag not yet placed | `git tag -l 'v1.4.0' 'checkpoint/2026-05-16-v1_4'` | empty | DERIVED |
| D27 | Doc 08 §13.5 states the cited property | read `08…md:608-699` | normative non-observability lemma + structural proof basis; digest.rs cites §13.5 | DERIVED |

---

## Findings (severity P0 ship-blocker / P1 well-scoped / P2 minor / P3 nit)

| ID | Lens | Sev | Location (file:line / SHA) | Issue | Recommended action / disposition |
|---|---|---|---|---|---|
| F-01 | 1/3 | **P2** | `docs/GATE_VERIFICATION_v1_4.md:152,159` + `docs/metrics/2026-05-16-v1_4.json:21-22,159,183` | The pinned Rust-delta figure is `git diff --shortstat a036c38 **4eb04dc**` = +7398/−6, but the **gate-doc commit is `X`=`3bbcd0f`** where the same diff = **+7399/−6** (+1 line). This is **comprehensively pre-disclosed**: the pinned-numbers header (`:152`) explicitly defines the basis as "`4eb04dc` + the docs/ledger-only delta", and GATE §4.3 (`:93`) quantifies the comment-only `digest.rs` §13→§13.5 repoint (5 ins/4 del, zero code) — the +1 is fully accounted for and the figure's *substance* (substantial additive, 0 new ext deps) is unaffected. **NOT the v1.3 cardinal class** (that was a headline erasure of a real +421/−93; this is a disclosed measurement-basis choice for a non-substantive +1). | Optional, non-gating: a future metrics/status wave could additionally state "+7399 at the gate-doc commit `X` (the +1 is the disclosed comment-only `digest.rs` line)" for symmetry, or pin the figure at `X` directly. The record is internally self-correcting via GATE §4.3 + the header. No tag impact. |
| F-02 | 1 | **P3** | `docs/metrics/2026-05-16-v1_4.json:198` (`deltas_vs_prev.decisions_section_11_rows`) | States `"v1_3": 62, "v1_4": 71, "delta": 9` and "all 63 pre-existing §11 rows byte-identical". The v1.3-tag count is **62** (independently re-derived: `git show a036c38:…00….md \| grep -cE '^\| §11\.[0-9]+ \| '` = 62), and the working-tree count is **71** — so the prose "all **63** pre-existing rows" is the post-§11.63-relocation set (62 v1.3-era + §11.63 = 63 pre-existing before §11.64-71). Not a contradiction (the set-difference is EMPTY, D16; §11.63 was pre-W1 at `3e0aac3`), but "62" vs "63" in adjacent clauses can momentarily read as inconsistent. | None required — the numbers are each correct in context (62 = v1.3-tag rows; 63 = pre-§11.64 rows incl. the pre-W1 §11.63; 71 = total). Note only; the append-only invariant is independently EMPTY (D16). |
| F-03 | 3 | **P3** | `docs/GATE_VERIFICATION_v1_4.md:156` (Pinned numbers, live-code method) | The method string says "`sed -n '261,387p' … \| grep -cE …`". The macro body's `=> (Sev,` arms in fact close before line 387 (the by-severity re-derivation needed the bare-token form `(Error\|Warning\|Info\|Hint`, not `Severity::…`). The **count (73) is exactly correct** by the literal command AND a whole-file cross-count AND the CI-lock const (D9); the 387 upper bound is merely generous. | Cosmetic — tighten the upper bound if a future wave touches the catalog. The pinned **73** is independently confirmed three ways. |
| F-04 | 4 | **P3** | `docs/AUDIT_PHASE_V1_4_W2_2026_05_16.md:32` (frozen) | The frozen W2 audit's F8 said Doc 08 "has **no §13.5**". That was true at `3447881`; W4b created §13.5 (carry-list item 4) — so the frozen-doc statement is now historically-scoped, not currently-true. This is **correct frozen-evidence behaviour** (a frozen audit reflects its pinned commit; the closeout resolves it by annotation, not by overwriting the frozen doc — verified byte-untouched, §1.3) and is exactly the intended carry-list mechanism. | None — flagged only to record that the W2-audit F8 "no §13.5" is a pinned-at-`3447881` statement the W4b batch correctly resolved (Doc 00 §11.69 / §13.5 added), with the frozen doc properly NOT overwritten. |

**No P0. No P1.** All findings are precision/disclosure/frozen-evidence-scoping notes. The corpus is internally consistent and the shipped source at `X` matches the §11.64–§11.71 record.

---

## Sequence note (necessary-not-sufficient — the v1.3 F-05 analogue)

`TAG-CLEAR` from this audit is **necessary, not sufficient**. GATE §4.4 correctly states the canonical cold-from-source quad at `X` "has not yet happened … the gating step the tag is contingent on" (honest prospective §11.30 posture — no fabricated transcript). The tag lands on `X` **iff** (a) this audit is TAG-CLEAR (it is) **and** (b) W4d's canonical cold quad at `X` — **with the MANDATORY NOTE-1 STEP 0** (`cargo clean` or non-shared `CARGO_TARGET_DIR`, GATE §4.2, binding v1.4+) + conformance 26/26 + the §5.4 gcc-RUN battery — is green. The empirical re-derivation of the headless loop (W4a's real-binary drive; the §11.1-W3 independent re-run; W4d's `cargo test --workspace` at `X`) is **out of scope here by mandate** and is the binding independent empirical gate at the tag commit. This audit is RECORD INTEGRITY only; nothing here was built to close a claim — the two claims that could not be confirmed without a run (the live test totals; the cold-quad green) are explicitly W4d's to empirically close and are correctly posed as contingent by the GATE, not asserted.

---

## Mechanics

- Worktree: `/root/dev/embeded-fsm-sdk-wt-v14pretag` (pre-created, `git -C`-only, READ-ONLY, zero `cargo`/`npm`/build — the binding disk-protection constraint honored).
- Branch: `phase4.4/v14-pretag-audit` (off `X`=`3bbcd0f`); clean at start (no partial state from the lost prior attempt — performed from scratch).
- This doc is NEW (`docs/AUDIT_PRE_TAG_v1_4_2026_05_16.md`); no existing audit/doc overwritten; exactly one file written.
- Committed as `Add v1.4 pre-tag four-lens audit`. **A sibling evidence commit — NOT folded into `X`; the tag lands on `X` (the v1.3 precedent). Not merged, not pushed, not tagged.** The orchestrator gates (this audit → W4d cold-quad at `X` → tag at `X`).

*— End of independent pre-`v1.4.0`-tag four-lens audit, 2026-05-16. A rubber-stamp here would be worse than a found problem; the Rust-delta framing, the Doc 00 append-only invariant, and the keystone are stated `TAG-CLEAR` because each was independently confirmed by this auditor's own derivation from `git`/source at `X`, not restated from the prior reports — the symmetric-analogue of the v1.3 cardinal catch was specifically hunted and found correctly stated.*
