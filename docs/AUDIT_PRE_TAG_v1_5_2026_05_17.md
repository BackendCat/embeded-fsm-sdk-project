# Independent Pre-`v1.5.0`-Tag Four-Lens Audit — 2026-05-17

**Document ID:** FSM-AUDIT-PRE-TAG-V15 (frozen evidence doc — never overwrite; a new audit is a new versioned file).
**Auditor role:** independent senior release auditor (adversarial, re-derive-don't-trust; the v1.4-W4c precedent of catching real issues by independent re-derivation, never trusting the gate-doc's self-report).
**Subject:** FSM Studio, `main` HEAD `2ff8ac3` (= the v1.5 §11.30 gate-doc commit "X" = `2ff8ac31f91eff922e48fb29ab1026b91a9166df` = `v1.5 closeout: GATE_VERIFICATION_v1_5 + the §11.30 batched gate-doc commit`).
**Baselines:** v1.4.0 frozen at `3bbcd0f` (== `git rev-list -1 v1.4.0` == `checkpoint/2026-05-16-v1_4`; tag-object `f0f931e`); the v1.5 implementer-corpus HEAD pre-gate at `d5fba3c` (`X^` = the C2-prettier commit); the gate-doc pinned Rust-delta basis = `3bbcd0f..d5fba3c`.
**Mode:** READ-ONLY, zero `cargo`/`npm`/build, `git -C`-only (the binding disk-protection constraint — `/` is at ~91%; the cold-quad/JS-lane are the LATER tag wave's contingent steps, the v1.4-W4c posture; the empirical layer is already covered by the post-A2 keystone audit `d22ef3b`'s own 867/0 re-run + C2's 41-test Extension-Host suite + the contingent quad-at-`X`). Worktree `/root/dev/embeded-fsm-sdk-wt-v15pretag`, branch `phase5.8/v15-pretag-audit`, verified clean at `X`.
**Mandate context:** the v1.3 pre-tag audit caught a cardinal "zero Rust delta" *understatement*; v1.4's symmetric exposure was the opposite (a +7398 verification-core epic that must be neither under- nor over-stated); v1.5's exposure is a **third shape** — a small-but-nonzero +933 delta (only A2 is Rust; A1/B1/C2 are TS; C1 is docs) that is *easy to mis-summarise as "no Rust change."* This audit assumes such an error may exist and re-derives every load-bearing number from `git`/source at `X`, never echoing the gate-doc's prose. A rubber-stamp here is worse than a found problem.

---

## Verdict

> **`TAG-CLEAR`** — **0 P0; 0 P1; 1 P2; 2 P3** (all precision/disclosure nits, none gating; the v1.1–v1.4 standard of ≤5 well-scoped P1 is met with **zero P1**).
>
> The four keystone invariants are **fully confirmed by my own derivation, not echoed**:
> 1. **The v1.5 Rust-delta framing is correct** — the third-shape analogue of the v1.3/v1.4 cardinal exposures is stated correctly: a SMALL ADDITIVE delta. `git diff --shortstat 3bbcd0f d5fba3c -- crates/ Cargo.toml Cargo.lock` = **7 files / +933 / −1** (independently re-derived); **the same diff at `X` itself = byte-identical 7 files / +933 / −1** (X is docs-only over `d5fba3c` — confirmed: the full `d5fba3c..X` delta is exactly 5 docs/meta files, zero `crates/`/`editors/`; **so there is NO v1.4-W4c F-01-class measurement-basis discrepancy — the v1.5 record measures at the correct commit**); the `fsm-verify`+`fsm-cli` differential-oracle diff = **EMPTY** at both `d5fba3c` and `X` (the keystone byte-untouched); `git diff 3bbcd0f d5fba3c -- Cargo.lock | grep -c '^+source = '` = **0**, no new `[[package]]`, the only Cargo.lock change = the single `+ "fsm-verify",` dependency-list edge. Stated everywhere as **"7 files / +933 / −1, a small additive delta, NOT empty and NOT zero"** — neither under-stated (the v1.3 cardinal "zero Rust delta" error) nor over-stated (the v1.4 +7398 epic).
> 2. **The cardinal-error-FORM judgment-call is independently re-judged CLEAR** — see §"Why this audit is not a rubber-stamp". I derived from `git` that **A1 (`5558d68`) has a literally-empty Rust delta** (`5558d68^..5558d68 -- crates/ Cargo.toml Cargo.lock` = empty; A1 touches only 6 `editors/vscode/**` TS files, +1380). Therefore the record's per-wave statements "A1 = zero Rust delta" are **literally-true facts about A1**, NOT a false assertion about the whole v1.5 delta. Every broad-regex `zero|no|empty …delta` hit (read individually) is exactly one of: (a) the literally-true per-wave fact "A1 = zero Rust delta"; (b) the brief-MANDATED v1.3-error-repudiation clause ("neither under-stated [the v1.3 'zero Rust delta' cardinal error] nor over-stated"); (c) a correctly-scoped sub-domain statement ("no `fsm-diagnostics` change", "no codegen delta"). **There are ZERO false assertions that the whole v1.5 Rust delta is zero/empty** (the actual cardinal sin). The gate-doc agent's narrow-assertion-form reading is independently correct — this, the single highest-stakes interpretation in the record, holds.
> 3. **Doc 00 is append-only** — the sorted set-difference `comm -23 (rows@d5fba3c-sorted) (rows@X-sorted)` = **EMPTY** (zero pre-existing §11.x row removed/altered); the *only* deleted line across `d5fba3c..X` is the **single §11-footer-line swap** (the v1.3 D15 / v1.4 invariant); §11.72–§11.79 are absent at `d5fba3c` (grep -c = 0) and are the v1.5 batch appended at the next free integer; the 7 carry-list-equivalent items (keystone K6/K7/K8; C1 defer C1-F3/F7/`noUncheckedIndexedAccess`; the D1-slip; G9/OWNER-A..D/#17/R7) each verifiably folded into GATE §6 + CHANGELOG `[1.5.0]` + Doc 00 §11.78/§11.79.
> 4. **The keystone holds at `X`** — re-derived from source at `X`: the negative-grep over `crates/fsm-lsp/src/**` + `editors/vscode/src/**` for any `fn` *defining* reachability/deadlock/transition-selection/guard-eval/completion/exploration logic = **∅ non-comment** (the single raw hit `server.rs:225 verify_request` read in full = a pure `spawn_blocking` orchestration shim that delegates to `capabilities::verify::run_verify`, defining no verification logic); `capabilities/verify.rs` read = a pure frontend (parses via the same `crate::analysis::analyze` seam, calls the identical `fsm_verify::verify`/`reachability_diagnostics`, marshals `VerifyOutcome` verbatim, uses the canonical `fsm_diagnostics::compute_line_col`); `git grep -l '#![forbid(unsafe_code)]' X -- 'crates/*/src/*.rs'` = **12 roots / 11 crates**; `fsm-verify`+`fsm-cli` byte-untouched vs `3bbcd0f`. The post-A2 audit `d22ef3b` (`KEYSTONE-INTACT`, audited `7eb7e59`, frozen, 0 commits-since-intro) is correctly the recorded source, and `X` (docs-only over `d5fba3c`) cannot have regressed it.
>
> The single non-trivial nuance (F-01, **P2**) is a benign informational artifact: the metrics/GATE toolchain line cites the *authoring* worktree path `…-wt-v15gate` (not the audit/tag worktree). The substantive claim is the pin `channel = "1.75.0"` — independently confirmed (`rust-toolchain.toml` byte-unchanged `3bbcd0f..X`, reads `1.75.0` at `X`). This is the Doc 00 §11.51 frozen-attestation pattern (a recorded assertion cites its authoring context; the pin is the load-bearing fact), not a misstatement. Non-gating.

---

## Why this audit is not a rubber-stamp

A clean verdict on a corpus whose v1.3 closeout produced a cardinal error, and whose v1.5 Rust delta is the easiest-yet to mis-summarise (only 1 of 7 waves is Rust), is itself suspect — so the four load-bearing re-derivations are shown explicitly here (full table in §"Independent Derivations"):

- **The cardinal-numeric re-derivation (my own commands, two bases).** `git diff --shortstat 3bbcd0f d5fba3c -- crates/ Cargo.toml Cargo.lock` = **7 files / +933 / −1**; the same diff at `X` itself (`3bbcd0f 2ff8ac3 -- crates/ Cargo.toml Cargo.lock`) = **byte-identical 7 files / +933 / −1**. I then proved *why* they are identical (not assumed): `git diff --name-status d5fba3c 2ff8ac3` = exactly `CHANGELOG.md`, `docs/00-…md`, `docs/GATE_VERIFICATION_v1_5.md` (A), `docs/ROADMAP.md`, `docs/metrics/2026-05-17-v1_5.json` (A) — **zero `crates/`/`editors/`**. So unlike v1.4 (where `X`=`4eb04dc`+a comment-only `digest.rs` +1 created the F-01 +7398-vs-+7399 measurement-basis nuance), **the v1.5 gate-doc commit `X` has an exactly-identical Rust delta to the pinned `d5fba3c` basis — the symmetric-to-v1.3-and-v1.4 catch is hunted and the basis is found clean, with NO F-01-class discrepancy to even disclose.** `fsm-verify`+`fsm-cli` diff vs `3bbcd0f` = empty at both commits; `^+source =` count = 0; only Cargo.lock change = `+ "fsm-verify",`.
- **The cardinal-error-FORM independent re-judgment (the single highest-stakes interpretation).** The naive broad regex `\b(zero|no|empty|nothing|none)\b[^.]{0,40}(rust )?delta` returns >0 hits on BOTH `GATE_VERIFICATION_v1_5.md` AND the metrics — exactly as the gate-doc agent disclosed. I did **not** trust the agent's narrow-form argument; I re-derived its load-bearing premise from `git`: `git diff --shortstat 5558d68^ 5558d68 -- crates/ Cargo.toml Cargo.lock` = **EMPTY** (A1 = `5558d68` = `v1.5 W-A1: surface shipped v1.4 verification in VS Code via CLI-spawn`; its full stat = 6 `editors/vscode/**` files, +1380, zero Rust). So **"A1 = zero Rust delta" is a literally-true derived fact**, not a false whole-delta summary. I then read **every** broad-regex hit in full sentence context (GATE lines 5/22/39/161/176/182/185; metrics lines 2/32/47/48/126/185/193/206/213) and classified each into the three benign classes (a)/(b)/(c) above. The whole-delta assertion is stated **"NOT empty and NOT zero"** explicitly (metrics lines 32, 185 — verbatim "NOT empty and NOT zero"). **Independent verdict on the disclosed judgment-call: there is ZERO false assertion that the whole v1.5 Rust delta is zero/empty. The gate-doc agent's "0 false whole-delta-is-zero ASSERTIONS" reading is CORRECT.** This is the actual cardinal sin (the v1.3 error) and it is absent; the required v1.3-error-repudiation quote and the literally-true "A1 = zero" per-wave fact are distinct from it and benign.
- **The Doc 00 append-only set-difference (not trusted from the metrics' "append-only verified" claim).** `comm -23 <(git show d5fba3c:docs/00-…md | grep -E '^\| §11\.[0-9]+ \| ' | sort) <(git show 2ff8ac3:docs/00-…md | grep -E '^\| §11\.[0-9]+ \| ' | sort)` = **EMPTY** (zero pre-existing §11.x row removed or altered). The full line-level `git diff d5fba3c 2ff8ac3 -- docs/00-…md` has **exactly one deleted line** = the §11-footer-line swap (verbatim shown, D-15); the new footer correctly records "rows §11.72-79 appended 2026-05-17, the v1.5 batched doc/ledger closeout". §11.72-79 grep -c at `d5fba3c` = **0** (the append is genuinely new).
- **The keystone re-enumeration at `X`.** Not echoed from the post-A2 audit: the negative-grep raw output read verbatim (1 hit = `server.rs:225 pub async fn verify_request`, read in full at `X` = a `spawn_blocking(|| crate::capabilities::verify::run_verify(&parsed))` orchestration shim — defines NO verification logic, delegates entirely); `capabilities/verify.rs` read at `X` (calls `fsm_verify::verify` at :234, `fsm_verify::reachability_diagnostics` at :262, reads `outcome.verdict`/`StopReason` at :275-278/:330-347/:401-403, parses via `crate::analysis::analyze` at :206, uses `fsm_diagnostics::compute_line_col` at :357 — a pure frontend, zero verification facts computed locally); `git grep -l '#![forbid(unsafe_code)]' 2ff8ac3 -- 'crates/*/src/*.rs'` = **12 roots / 11 distinct crates** (re-derived, not echoed from the v1.4 figure). `fsm-verify`+`fsm-cli` byte-untouched vs `3bbcd0f` (D-04).

The one place a cardinal error could hide (the v1.5 Rust-delta framing and its error-FORM) was specifically hunted: the framing **held, correctly, as the third-shape instance** of the release-record-symmetry discipline (small additive, neither under- nor over-stated), and the error-FORM judgment-call was **independently re-judged clear** by deriving A1's empty Rust delta from `git` and reading every broad-regex hit individually. The v1.4-W4c F-01-class measurement-basis nuance was checked and found **absent** (X's Rust delta is byte-identical to the pinned basis).

---

## Lens 1 — Release-record integrity (the cardinal lens; independent pass)

### 1.1 The v1.5 Rust-delta framing — VERIFIED CORRECT (the third-shape analogue; the symmetric-to-v1.3 catch)

**Independent ground truth (re-derived, not echoed):**

| Claim under test | Command | Result |
|---|---|---|
| v1.5 Rust delta (the GATE/metrics-pinned basis) | `git diff --shortstat 3bbcd0f d5fba3c -- crates/ Cargo.toml Cargo.lock` | **7 files, +933 / −1** — NOT empty, NOT zero (small additive) |
| Same at the gate-doc commit `X` itself | `git diff --shortstat 3bbcd0f 2ff8ac3 -- crates/ Cargo.toml Cargo.lock` | **7 files, +933 / −1** — **byte-identical** (NO F-01-class +1; X is docs-only over `d5fba3c`) |
| Why identical (proven, not assumed) | `git diff --name-status d5fba3c 2ff8ac3` | exactly 5 docs/meta files (`CHANGELOG`, Doc 00, GATE_v1_5 [A], ROADMAP, metrics v1_5 [A]) — **zero crates/editors** |
| Confined to `crates/fsm-lsp/**` | `git diff --shortstat 3bbcd0f d5fba3c -- crates/fsm-lsp/` | **6 files, +932 / −1** (the A2 dep edge + the `fsm/verify` capability + its acceptance test) |
| Differential oracle byte-untouched | `git diff --stat 3bbcd0f d5fba3c -- crates/fsm-verify/ crates/fsm-cli/` | **EMPTY** (also empty at `3bbcd0f 2ff8ac3`) — the keystone holds |
| 0 new external deps | `git diff 3bbcd0f d5fba3c -- Cargo.lock \| grep -c '^+source = '` | **0**; `grep -E '^\+(name\|source) ='` ⇒ **none** (no new `[[package]]`); the only change = `+ "fsm-verify",` |
| `3bbcd0f` IS the v1.4.0 commit | `git rev-list -1 v1.4.0` | `3bbcd0f5…` (== `3bbcd0f`); == `git rev-list -1 checkpoint/2026-05-16-v1_4` — the baseline anchor is real |
| `rust-toolchain.toml` byte-unchanged | `git diff --shortstat 3bbcd0f 2ff8ac3 -- rust-toolchain.toml` | **empty**; reads `channel = "1.75.0"` at `X` — the pin held |

The delta is everywhere stated as a **small additive** delta (the metrics `interpretation`/`deltas_vs_prev.rust_workspace.note` say "NOT empty and NOT zero"; GATE §"Pinned gate numbers" says "A SMALL ADDITIVE delta … NEITHER under-stated [the v1.3 cardinal 'zero Rust delta' error] NOR over-stated [the v1.4 +7398]"). **The framing is accurate — the third-shape instance done correctly. There is no v1.4-W4c F-01 measurement-basis discrepancy to even disclose (X ≡ d5fba3c for crates/).**

### 1.2 The cardinal-error-FORM judgment-call — INDEPENDENTLY RE-JUDGED CLEAR (the single highest-stakes interpretation)

The gate-doc agent disclosed that the naive broad regex `zero|no|empty .* delta` returns >0 on the record BY CONSTRUCTION, and argued the binding check is "0 false *assertions* that the WHOLE v1.5 Rust delta is zero/empty" (a narrow assertion-form reading). **I re-judged this from the actual GATE/metrics text, not the agent's argument:**

- **Premise re-derived from `git`:** A1 = `5558d68`. `git diff --shortstat 5558d68^ 5558d68 -- crates/ Cargo.toml Cargo.lock` = **EMPTY**; `git show --stat 5558d68` = 6 `editors/vscode/**` files, +1380, **zero Rust**. ⇒ "A1 = zero Rust delta" is a **literally-true derived fact**.
- **Every broad-regex hit read individually** (GATE 5/22/39/161/176/182/185; metrics 2/32/47/48/126/185/193/206/213) and classified:
  - **Class (a) — literally-true per-wave fact:** GATE 5/39, metrics 47/48/126/206 ("A1 = zero Rust delta", "A1 via CLI-spawn — zero Rust delta"). Verified true above.
  - **Class (b) — brief-MANDATED v1.3-error-repudiation:** GATE 185, metrics 2/32/185 ("neither under-stated [the v1.3 'zero Rust delta' cardinal error] nor over-stated"). Quoting the v1.3 error to mark it resolved is *required* by the brief — it is the repudiation, not the error.
  - **Class (c) — correctly-scoped sub-domain:** GATE 22 ("no simulator/codegen/baseline delta in the v1.5 scope" — true: the delta is confined to `fsm-lsp`), GATE 161/176/182, metrics 193/213 ("added no diagnostic code", "the Rust delta is confined to `crates/fsm-lsp/**`" — true and correctly scoped). metrics line 2 also contains "ZERO false-zero whole-delta assertions" — a *meta-statement asserting the absence of the sin*, not the sin.
- **The whole-delta assertion** is stated **"7 files / +933 / −1, a small additive delta, NOT empty and NOT zero"** — verbatim "NOT empty and NOT zero" appears (metrics 32, 185).

**Independent verdict: there is ZERO *false* assertion that the whole v1.5 Rust delta is zero/empty** (the actual cardinal sin — the v1.3 error). The (a) literally-true "A1 = zero" facts and the (b) required repudiation-quote are categorically distinct from the cardinal sin and are benign. **The gate-doc agent's narrow-assertion-form reading is independently CORRECT.** This judgment-call is fully confirmed.

### 1.3 Per-row §11.72–§11.79 verification (every cited commit `git show`-checked)

| Row | Claim (abridged) | Cited commit(s) | Independent verdict |
|---|---|---|---|
| §11.72 | A0 Doc 31 the v1.5 wave-plan-of-record (Doc 28/29/30 precedent; GT-2 wrong-slug framing correction) | `df16055` | **ACCURATE** — `df16055` = `Add Doc 31 — v1.5 UI/DX wave-plan-of-record`; Doc 31 present + frozen (0 commits-since-intro to `X`) |
| §11.73 | A1 verification in VS Code via CLI-spawn; ZERO Rust delta; the proven `cliRunner`/`cliBinary` seam | `5558d68` | **ACCURATE** — `5558d68` = `v1.5 W-A1: surface … via CLI-spawn`; Rust delta `5558d68^..5558d68` = **empty** (re-derived); +1380 TS only |
| §11.74 | A2 the `fsm/verify` LSP-embed = the SINGLE Rust-adding wave (one Cargo.toml `fsm-verify` path edge, no new crate, a pure frontend) + the binding KEYSTONE-IN-UI gate row | `4737259` Rust + `7eb7e59` client | **ACCURATE** — `4737259` Rust delta = 6 files/+932/−1; `7eb7e59` = the client; `forbid` 12/11 re-derived; no new `[[package]]`; `capabilities/verify.rs` read = pure frontend (§Lens 2) |
| §11.75 | post-A2 source-derived KEYSTONE phase-audit `d22ef3b` `KEYSTONE-INTACT`; the no-fork re-derived FROM SOURCE + the differential battery RE-RUN by the auditor; K6/K7/K8 P3 carry-list | `d22ef3b` (audited `7eb7e59`) | **ACCURATE** — `d22ef3b` frozen doc verdict line = **`KEYSTONE-INTACT`**, audited `7eb7e59`, auditor re-ran `git diff --stat 5558d68..7eb7e59 -- crates/fsm-verify crates/fsm-cli` = empty + quad 867/0; 0 commits-since-intro |
| §11.76 | B1 Marketplace-readiness manifest pass; GT-2/GT-10 fixed; the v1.3.x `makeNonce`-CSPRNG + JC-3 nits resolved; publish-READY not published | `7b04f91` | **ACCURATE** — `7b04f91` = `v1.5 W-B1 (VS Code): Marketplace-readiness manifest pass` |
| §11.77 | C1 the read-only TS frontend-practices audit (0 P0, 2 fix-in-v1.5, leave-and-explain) + the v1.6 FE-hygiene defer-batch | `38f80c1` (audited `d22ef3b`) | **ACCURATE** — `38f80c1` frozen doc = audited `d22ef3b`, "No P0. No ship-blocker." + 2 fix-in-v1.5 (C1-F1 incl. folded C1-F9; C1-F2); 0 commits-since-intro |
| §11.78 | C2 scoped remediation (the `fsm.verifyLive` Extension-Host E2E folding C1-F9 + prettier); §11.30-NOTE-1 + the RE-ACTIVATED JS lane; the D1-slip disposition | `dc8d926` + `d5fba3c` | **ACCURATE** — `dc8d926` = `v1.5 W-C2 (C1-F1 + folded C1-F9): Extension-Host E2E for fsm.verifyLive`; `d5fba3c` = `v1.5 W-C2 (C1-F2): adopt prettier, scoped to editors/vscode/**` |
| §11.79 | the v1.5 batched closeout = the §11.30 gate-doc commit `X`; the FROZEN keystone-audit P3 carry-list + the C1 v1.6 FE-hygiene defer + the carried owner-escalations folded in ONE batch | `X` (this commit) | **ACCURATE** — the row explicitly identifies itself as the §11.30 gate-doc commit; `git show --stat 2ff8ac3` = the 5-file docs/meta batch only; all carry items folded (§1.5) |

All cited wave-commit SHAs resolve to the claimed subjects (`git log --format='%s' -1 <sha>`).

### 1.4 Frozen-artifact + structural integrity

| Check | Method | Result |
|---|---|---|
| `docs/31-…md`, `AUDIT_KEYSTONE_V1_5_A2…md`, `AUDIT_VSCODE_FRONTEND_PRACTICES…md` byte-untouched `d5fba3c..X` | `git diff --shortstat d5fba3c 2ff8ac3 -- <f>` | **UNTOUCHED** (all empty) |
| `GATE_VERIFICATION_v1_{0,1,2,3,4}.md` byte-untouched `d5fba3c..X` | same | **UNTOUCHED** (all 5 empty) |
| Frozen v1.5 phase-audits / Doc 31 untouched since intro | `git log --oneline <intro>..2ff8ac3 -- <f>` | `AUDIT_KEYSTONE_V1_5_A2` (intro `d22ef3b`), `AUDIT_VSCODE_FRONTEND_PRACTICES` (intro `38f80c1`), Doc 31 (intro `df16055`) — **0 commits-since-intro** (all) |
| Prior metrics frozen | `git diff --shortstat d5fba3c 2ff8ac3 -- <f>` | `2026-05-16-v1_4.json`, `2026-05-16-v1_3.json` — **UNTOUCHED** |
| `AUDIT_PRE_TAG_v1_4` itself untouched | same | **UNTOUCHED** |
| The broader doc-honesty surface NOT touched by the gate-doc | `git diff --shortstat d5fba3c 2ff8ac3 -- README.md docs/07-Specification-Roadmap.md docs/14-LSP-Capability-Spec.md` | **UNTOUCHED** (all 3 empty) — only the ROADMAP "See also" per Doc 31 §6 H6/§7.4 was reconciled (§Lens 4) |
| Doc 00 append-only | `git diff --numstat d5fba3c 2ff8ac3 -- docs/00-…md` = **+9/−1**; full `diff` deleted-lines = **1** (the footer) | **APPEND-ONLY** — 8 new rows (§11.72-79) + footer swap; the **sorted set-difference (old\new) = EMPTY** |
| §11.72-79 are the v1.5 batch | `git show d5fba3c:docs/00-…md \| grep -cE '^\| §11\.(7[2-9]) \| '` | **0** (absent at `d5fba3c`) |
| New footer correct | read added footer line @ `X` | records "rows §11.72-79 appended 2026-05-17, the v1.5 batched doc/ledger closeout — the UI/DX 'Convenience Layer' A0–C2 … + the §11.30 gate-doc-commit folding the frozen keystone-audit P3 carry-list + the C1 v1.6 FE-hygiene defer + the D1 slip to v1.6" ✅ |
| Closeout commit scope | `git show --stat 2ff8ac3` | **5 docs/meta files ONLY** (CHANGELOG, Doc 00 +7/−3, GATE_v1_5 [new], ROADMAP, metrics v1_5 [new]); **zero crates/editors** |
| CHANGELOG `[1.5.0]` structurally complete | read block | dated 2026-05-17; Added / Fixed / Known limitations; D1-slip, FE-hygiene defer, carried owner-escalations (G9/Marketplace/5-platform/Certifiability/LICENSE/R7), G7/G9 partials — **complete; no unshipped claim** (the tag is stated contingent) |

### 1.5 The 7 carry-list-equivalent items — each verifiably folded

| # | Carry-list item | Independent verdict |
|---|---|---|
| 1 | Keystone-audit K6 (re-stated-marshalling drift hazard) | **FOLDED** — GATE §6.1 [K6] + metrics `p3_keystone_carry_list_this_release` + Doc 00 §11.79 (recorded P3, the differential-oracle test is the durability mechanism; sound today) |
| 2 | Keystone-audit K7 (`fsm-verify/vN` policy source-only — v1.4-W2 F9 carry, now +LSP consumer) | **FOLDED** — GATE §6.1 [K7] + metrics + §11.79 (re-stated as the carried v1.4-W2 F9 with the additional LSP consumer) |
| 3 | Keystone-audit K8 (client-side trigger-policy spec-undocumented) | **FOLDED** — GATE §6.1 [K8] + metrics + §11.79 (keystone unaffected; carried for the deferred-D1 doc pass) |
| 4 | C1 defer-tracked → v1.6 FE-hygiene (C1-F3 ESLint-9 + C1-F7 madge + `noUncheckedIndexedAccess`) | **FOLDED** — GATE §6.2 + CHANGELOG `[1.5.0]` Known limitations + ROADMAP v1.6 candidates + metrics `c1_defer_tracked_v1_6_fe_hygiene_batch` |
| 5 | The D1 slip (deferred to v1.6 per OWNER-D, droppable, the discarded output-filtered run) | **FOLDED** — GATE §6.3 + CHANGELOG Known limitations + ROADMAP "Static docs/landing site (D1): DEFERRED to v1.6" + Doc 00 §11.78 + metrics `d1_slip_this_release` |
| 6 | Carried owner-escalations (G9/OWNER-A/OWNER-B/OWNER-C/OWNER-D/task #17) | **FOLDED** — GATE §6.3 (each named: `OWNER-A` :49/:146, `OWNER-B` :147, `OWNER-C` :148, `OWNER-D` :149, `task #17` :150, `G9` :25/:113) + CHANGELOG + metrics `carried_owner_escalations` |
| 7 | R7 (mechanised sim≡codegen-equivalence proof — named v1.4-stretch deferral) | **FOLDED** — GATE §6.3 line 151 "R7 … remains a named v1.4-stretch/later DEFERRAL (carried unchanged … named-not-omitted)" + CHANGELOG + metrics |

---

## Lens 2 — v1.5 corpus structural soundness (the keystone, re-derived at `X`)

| Keystone | Method (at `2ff8ac3`) | Result |
|---|---|---|
| KEYSTONE-IN-UI negative-grep — no `fn` *defining* verification logic in `fsm-lsp/src` + `editors/vscode/src` | `git grep -nE '(fn\|function)[^=]*\b(reachab\|deadlock\|select_transition\|enabled_set\|eval_guard\|compute_lca\|find_lca\|completion_drain\|fire_timer\|join_complete\|region_join\|explore_state)' 2ff8ac3 -- 'crates/fsm-lsp/src/**/*.rs' 'editors/vscode/src/**/*.ts'` | **∅ (NO_MATCH)** |
| Broader verification-keyword `fn` definitions (raw, every hit read — the `cmd\|head&&echo` false-positive caveat applied: raw lines shown, no echo branch) | `git grep -nE 'fn (compute_reachab\|check_deadlock\|select_transition\|eval_guard\|find_lca\|explore\|verify_)' 2ff8ac3 -- 'crates/fsm-lsp/src' 'editors/vscode/src'` | **1 raw hit**, read in full: `server.rs:225 pub async fn verify_request` — a `spawn_blocking(\|\| crate::capabilities::verify::run_verify(&parsed))` orchestration shim (parses params → offloads → maps a worker-join error). **Defines NO verification logic; delegates entirely.** Not a forked-semantics site |
| `capabilities/verify.rs` is a pure frontend (read at `X`) | `git show 2ff8ac3:crates/fsm-lsp/src/capabilities/verify.rs` (read 1-60 + all `fsm_verify::`/`analyze`/`compute_line_col` sites) | **PURE FRONTEND** — parses via `crate::analysis::analyze` (the SAME `fsm_parser::parse`+`analyze_with_source` the CLI uses, :206); calls `fsm_verify::verify` (:234) + `fsm_verify::reachability_diagnostics` (:262); reads `outcome.verdict`/`StopReason` off the returned `VerifyOutcome` (:275-278, :330-347, :401-403); verdict-string map = a fixed total projection of `outcome.verdict`+`has_e0400` (schema marshalling, not a verification decision); uses the canonical `fsm_diagnostics::compute_line_col` (:357, defined nowhere in `fsm-lsp`/`editors`). **ZERO verification facts computed locally** |
| `fsm-verify` + `fsm-cli` byte-untouched vs `3bbcd0f` | `git diff --stat 3bbcd0f 2ff8ac3 -- crates/fsm-verify/ crates/fsm-cli/` | **EMPTY** (the differential oracle byte-untouched at `X`) |
| `#![forbid(unsafe_code)]` = 12 roots / 11 crates from source | `git grep -l '#![forbid(unsafe_code)]' 2ff8ac3 -- 'crates/*/src/*.rs'` ⇒ 12; `\| sed crate \| sort -u \| wc -l` ⇒ 11 | **12 root files / 11 distinct crates** (`fsm-lsp` = lib.rs+main.rs ⇒ 2 roots/1 crate; `fsm-verify/src/lib.rs` the 12th root/11th crate) — matches GATE/metrics exactly |
| GATE records KEYSTONE-IN-UI PASS sourced from the independent post-A2 audit `d22ef3b` | read GATE §3 + §7 + the frozen `AUDIT_KEYSTONE_V1_5_A2…md` | **CONFIRMED** — GATE §3 row + §7 cite `d22ef3b` verdict `KEYSTONE-INTACT` (audited `7eb7e59`, the auditor re-ran the no-fork from source + the differential battery + the 867/0 quad); the frozen doc's verdict line = `KEYSTONE-INTACT`; **`X` is docs-only over `d5fba3c` (the 5-file delta excludes all `crates/`) ⇒ it cannot have regressed the keystone** |

All keystones verified in **shipped source at `X`**, not from the docs' own claim. No contradiction between the shipped corpus and the §11.72–§11.79 prose. The post-A2 audit's own differential-battery re-run + quad are frozen evidence (not re-litigated here; this audit confirms `X` cannot have regressed them and the record cites them correctly).

---

## Lens 3 — Gate criteria + no-P0 (every numeric independently re-derived from source at `X`)

| Gate numeric | Independent derivation at `2ff8ac3` | Value | Doc claim | Match |
|---|---|---|---|---|
| Live `DiagnosticCode` | the GATE's method `git show …:fsm-diagnostics/src/lib.rs \| sed -n '261,387p' \| grep -cE '^\s+[EWIH][0-9]{4}\s*=>\s*\('` = **73**; whole-file cross-count = **73**; by-severity `=> ((Error\|Warning\|Info\|Hint` = **51E/12W/4I/6H**; CI-lock `const EXPECTED: usize = 73` (lib.rs:820) | **73** | 73 (metrics `by_severity` 51/12/4/6) | ✅ |
| v1.5 added no code (the reasoning) | the Rust delta is confined to `crates/fsm-lsp/**` (D); `crates/fsm-diagnostics/` not in the delta; `git diff --shortstat 3bbcd0f 2ff8ac3 -- tests/conformance/MANIFEST.json` empty corroborates | reasoning sound | "unchanged — v1.5 added/retired no code" | ✅ |
| Conformance fixtures | `git show 2ff8ac3:tests/conformance/MANIFEST.json \| jq '[.categories[].fixtures\|length]\|add'` | **26** (parser 6 + validator 6 + semantic 7 + codegen-c 4 + formatter 3) | 26 | ✅ |
| Conformance MANIFEST byte-untouched | `git diff --shortstat 3bbcd0f 2ff8ac3 -- tests/conformance/MANIFEST.json` | **empty** | byte-untouched | ✅ |
| `#![forbid(unsafe_code)]` | `git grep -l … 2ff8ac3 -- 'crates/*/src/*.rs'` ⇒ 12; distinct crates ⇒ 11 | **12 / 11** | 12 / 11 (A2 added a dep edge not a crate — `^+name=/^+source=` ⇒ none) | ✅ |
| v1.5 Rust delta | `git diff --shortstat 3bbcd0f d5fba3c …` ⇒ 7f/+933/−1; `… 3bbcd0f 2ff8ac3 …` ⇒ **identical** 7f/+933/−1; `^+source =` ⇒ 0 | **7f / +933 / −1; 0 new ext deps** | 7f / +933 / −1; 0 new ext deps | ✅ (NO F-01 — X ≡ d5fba3c for crates/) |
| TS delta | `git diff --shortstat 3bbcd0f d5fba3c -- editors/vscode` | **41 files / +2693 / −1026** | 41 / +2693 / −1026 | ✅ |
| §11 rows total | `git show 2ff8ac3:docs/00-…md \| grep -cE '^\| §11\.[0-9]+ \| '` | **79** | 79 | ✅ |
| Zero P0 | GATE §1/§7 "0 open P0"; post-A2 `KEYSTONE-INTACT` + C1 "0 P0, 0 ship-blocker" (both frozen, re-read); cross-checked vs shipped keystones (Lens 2) | **0 P0** | 0 P0 | ✅ |

**The §11.30 cold-quad + the re-activated JS lane are correctly posed as CONTINGENT (the v1.4-W4c F-05 honest-prospective discipline):** GATE §4.2 writes the MANDATORY NOTE-1 `cargo clean -p` of all 11 first-party crates STEP 0 (binding v1.4+, the box-infeasible virgin-target ideal explicitly rejected, the disk-protective registry-cache-preserving pattern recorded); GATE §4.3 makes the JS lane (`npm audit` triaged + `vsce package` lint-clean VSIX + the 41-test `xvfb-run @vscode/test-electron` Extension-Host suite) a **binding re-activated gate at `X`** (binding for v1.5 because the surface *is* `editors/vscode/**`, TS delta 41/+2693/−1026 — unlike v1.4 which carried it unrun); GATE §4.3/§4.4 explicitly state the run "has not yet happened … the gating step the tag is contingent on" — **no fabricated transcript**. De-risking predicate (the post-A2 auditor's own 867/0 quad at `7eb7e59` + B1's 4.96 MB VSIX + C2's 41 passing) is correctly stated as "NOT a substitute for the canonical run at `X`."

**Accepted-tracked-debt — each honestly recorded:** R7 (named-not-omitted, GATE §6.3 :151 / CHANGELOG / metrics); G7 (26 fixtures, the verify *surface* behaviourally tested — same v1.0–v1.4 posture); G9 (remote CI / JS lane never run on a remote runner — re-stated, not resolved); the D1-slip (OWNER-D, droppable, recorded explicitly so not misread as silently dropped); the FE-hygiene defer-batch (none silently dropped). 0 open P0.

---

## Lens 4 — Whole-corpus coherence + tag-topology

| Check | Method | Result |
|---|---|---|
| No new doc↔code drift from the gate-doc batch | Lens-2 keystone cross-check (shipped @ `X` vs §11.72-79) + the X-is-docs-only proof | **none** — every prose keystone matches shipped source; the only `X` delta is the 5 docs/meta files (zero crates/editors) |
| §7.4 ROADMAP "See also" reconcile — the LIVE pointer actually fixed (not just narrated) | `git diff d5fba3c 2ff8ac3 -- docs/ROADMAP.md` | **FIXED** — the old line `- ~/.claude/plans/toasty-prancing-goose.md — the live multi-phase execution plan` is **DELETED**; the new live pointer (the "See also" bullet) points at `memory/project_embeded_fsm_sdk_backlog.md` (the backlog SoT) + `docs/31-v1_5-UIDX-Wave-Plan.md`. `toasty-prancing-goose` survives at `X` in **exactly 2 places, both as the audit-traceable record-of-the-fix** (the retrospective + the parenthetical "this previously pointed at … that was a v1.1-era plan … the stale pointer is removed"); it is **nowhere** still described as "the live multi-phase execution plan." The live pointer is genuinely repointed |
| Doc 31 actually mandates the §7.4 reconcile at the closeout | `git show 2ff8ac3:docs/31-…md` §6 H6 + §7 item 4 | **MANDATED** — Doc 31 §6 H6 + §7 item 4 cite the stale `docs/ROADMAP.md` "See also" pointer as verified ground truth (`ROADMAP.md:248`), explicitly carried to be reconciled at the v1.5 closeout (not a code wave) — the closeout's ROADMAP edit is exactly the mandated scope; the broader README/Doc-07/Doc-14 doc-honesty pass is correctly FLAGGED-not-fixed (GATE §6.4) |
| §11.30 tag-topology pre-emptively correct in GATE §8 | read GATE §8 + §4.2/§4.4 | **CORRECT** — GATE §8 states cold-quad-commit ≡ JS-lane-commit ≡ tag-commit ≡ `checkpoint/2026-05-17-v1_5` ≡ `X` by construction; §4.2 MANDATORY STEP 0 + §4.3 JS lane are the binding pre-tag preconditions; §4.4 states the run "has not yet happened" (honest prospective, no fabricated transcript); the 5th-consecutive prospective-clean framing accurate (v1.2/v1.3/v1.4 + this) |
| `git rev-parse` vs `git rev-list -1` annotated-tag-object distinction recorded | `git rev-parse v1.4.0`/`v1.3.0` vs `git rev-list -1 …` | **RECORDED + re-confirmed** — v1.4.0 tag-object `f0f931e…` ≠ commit `3bbcd0f…`; v1.3.0 tag-object `a24c195b…` ≠ commit `a036c381…`; GATE §8 + metrics `tag_object_vs_commit_note` record this so no future wave misreads the `v1.5.0` tag-object sha as a mismatched commit |
| `v1.5.0` / `checkpoint/2026-05-17-v1_5` not yet placed | `git tag -l 'v1.5.0' 'checkpoint/2026-05-17-v1_5'` | **empty** — correct: the tag is the contingent step gated on this audit (TAG-CLEAR) + the cold-quad+JS-lane at `X`. (All existing tags: v1.0.0–v1.4.0 + 6 checkpoints; immutable) |
| `v1.4.0` baseline immutable | `git rev-list -1 v1.4.0` == `git rev-list -1 checkpoint/2026-05-16-v1_4` | both `3bbcd0f5…` — the v1.5 baseline anchor is real and not moved |

---

## Independent Derivations Table (proof I re-derived, did not echo)

| # | Derived fact | Exact command / source | Output | Echo-or-derived |
|---|---|---|---|---|
| D-01 | v1.5 Rust delta (pinned basis) | `git diff --shortstat 3bbcd0f d5fba3c -- crates/ Cargo.toml Cargo.lock` | 7 files, +933 / −1 | DERIVED |
| D-02 | v1.5 Rust delta @ `X` itself | `git diff --shortstat 3bbcd0f 2ff8ac3 -- crates/ Cargo.toml Cargo.lock` | **byte-identical** 7 files, +933 / −1 (NO F-01) | DERIVED |
| D-03 | Why D-01≡D-02 (X is docs-only) | `git diff --name-status d5fba3c 2ff8ac3` | exactly 5 docs/meta files; zero crates/editors | DERIVED |
| D-04 | Differential oracle byte-untouched | `git diff --stat 3bbcd0f d5fba3c -- crates/fsm-verify/ crates/fsm-cli/`; also `3bbcd0f 2ff8ac3` | EMPTY (both) | DERIVED |
| D-05 | 0 new external deps | `git diff 3bbcd0f d5fba3c -- Cargo.lock \| grep -c '^+source = '`; `\| grep -E '^\+(name\|source) ='` | 0; none (no new `[[package]]`); only `+ "fsm-verify",` | DERIVED |
| D-06 | A1 = literally-zero Rust delta (the error-FORM premise) | `git diff --shortstat 5558d68^ 5558d68 -- crates/ Cargo.toml Cargo.lock`; `git show --stat 5558d68` | EMPTY; 6 `editors/vscode/**` files +1380 | DERIVED |
| D-07 | No false whole-delta-is-zero assertion | broad regex `\b(zero\|no\|empty\|nothing\|none)\b[^.]{0,40}(rust )?delta` over GATE+metrics; **every hit read individually** | hits ⊂ {(a) true "A1=zero", (b) mandated v1.3-repudiation, (c) scoped sub-domain}; whole-delta = "NOT empty and NOT zero" | DERIVED |
| D-08 | `3bbcd0f` = v1.4.0 commit | `git rev-list -1 v1.4.0`; `git rev-list -1 checkpoint/2026-05-16-v1_4` | both `3bbcd0f5…` | DERIVED |
| D-09 | Live `DiagnosticCode` | `git show 2ff8ac3:…lib.rs \| sed -n '261,387p' \| grep -cE …`; whole-file cross-count; by-severity; const | 73; 73; 51E/12W/4I/6H; `const EXPECTED: usize = 73` | DERIVED (not the const alone) |
| D-10 | Conformance fixtures | `git show 2ff8ac3:tests/conformance/MANIFEST.json \| jq '[.categories[].fixtures\|length]\|add'` | 26 (6/6/7/4/3) | DERIVED |
| D-11 | MANIFEST byte-untouched | `git diff --shortstat 3bbcd0f 2ff8ac3 -- tests/conformance/MANIFEST.json` | empty | DERIVED |
| D-12 | forbid(unsafe_code) | `git grep -l '#![forbid(unsafe_code)]' 2ff8ac3 -- 'crates/*/src/*.rs'`; distinct crates | 12 roots / 11 crates | DERIVED |
| D-13 | Keystone negative-grep @ `X` | `git grep -nE '(fn\|function)[^=]*\b(reachab\|deadlock\|…)' 2ff8ac3 -- fsm-lsp/src + editors/vscode/src`; broader `fn (…\|verify_)` | ∅; 1 raw hit `server.rs:225 verify_request` read in full = orchestration shim | DERIVED (raw read, no echo branch) |
| D-14 | `capabilities/verify.rs` pure frontend | `git show 2ff8ac3:…/capabilities/verify.rs` (1-60 + all `fsm_verify::`/`analyze`/`compute_line_col` sites) | calls `fsm_verify::verify`:234 / `reachability_diagnostics`:262; `analyze`:206; `compute_line_col`:357; zero local verification | DERIVED |
| D-15 | Doc 00 append-only / footer-only deletion | `git diff --numstat d5fba3c 2ff8ac3 -- docs/00-…md`; full `diff` deleted-lines | +9/−1; deleted = 1 (the footer line, verbatim) | DERIVED |
| D-16 | Doc 00 set-difference EMPTY | `comm -23 <(rows@d5fba3c sorted) <(rows@2ff8ac3 sorted)` | EMPTY (zero pre-existing §11.x row removed/altered) | DERIVED |
| D-17 | §11.72-79 absent at `d5fba3c` | `git show d5fba3c:docs/00-…md \| grep -cE '^\| §11\.(7[2-9]) \| '` | 0 | DERIVED |
| D-18 | Frozen audits/Doc 31 untouched since intro | `git log --oneline <intro>..2ff8ac3 -- <f>` ×3 | 0 / 0 / 0 commits-since-intro | DERIVED |
| D-19 | Frozen GATE_v1_{0..4}/prior-metrics/AUDIT_PRE_TAG_v1_4 byte-untouched | `git diff --shortstat d5fba3c 2ff8ac3 -- <f>` × | all empty | DERIVED |
| D-20 | Broader doc-honesty surface NOT touched | `git diff --shortstat d5fba3c 2ff8ac3 -- README.md docs/07-…md docs/14-…md` | all empty (only ROADMAP "See also" reconciled) | DERIVED |
| D-21 | ROADMAP §7.4 live pointer actually fixed | `git diff d5fba3c 2ff8ac3 -- docs/ROADMAP.md` (the `-`/`+` See-also lines) | old `toasty…live multi-phase` line DELETED; new pointer → backlog SoT + Doc 31; `toasty` survives only as record-of-fix | DERIVED |
| D-22 | Doc 31 mandates the §7.4 reconcile at closeout | `git show 2ff8ac3:docs/31-…md` §6 H6 + §7 item 4 | cites `ROADMAP.md:248` stale pointer, carried to the v1.5 closeout (not a code wave) | DERIVED |
| D-23 | annotated-tag distinction | `git rev-parse v1.4.0`/`v1.3.0` vs `git rev-list -1 …` | tag-object ≠ commit (both pairs: f0f931e≠3bbcd0f, a24c195b≠a036c381) | DERIVED |
| D-24 | tag not yet placed | `git tag -l 'v1.5.0' 'checkpoint/2026-05-17-v1_5'` | empty | DERIVED |
| D-25 | cited commits resolve | `git log --format='%s' -1 <c>` for df16055/5558d68/4737259/7eb7e59/d22ef3b/7b04f91/38f80c1/dc8d926/d5fba3c | each = the claimed subject | DERIVED |
| D-26 | post-A2 / C1 verdicts | `git show 2ff8ac3:docs/AUDIT_KEYSTONE_V1_5_A2…md` / `…FRONTEND_PRACTICES…md` | `KEYSTONE-INTACT` (audited `7eb7e59`); "No P0. No ship-blocker." (audited `d22ef3b`) | DERIVED |
| D-27 | toolchain pin held | `git diff --shortstat 3bbcd0f 2ff8ac3 -- rust-toolchain.toml`; `git show 2ff8ac3:rust-toolchain.toml` | empty; `channel = "1.75.0"` | DERIVED |

---

## Findings (severity P0 ship-blocker / P1 well-scoped / P2 minor / P3 nit)

| ID | Lens | Sev | Location (file:line / SHA) | Issue | Recommended action / disposition |
|---|---|---|---|---|---|
| F-01 | 3/4 | **P2** | `docs/metrics/2026-05-17-v1_5.json:187` + `docs/GATE_VERIFICATION_v1_5.md:187` (the "Toolchain (asserted from inside the worktree)" row) | The recorded `rustup show active-toolchain` cites `overridden by '/root/dev/embeded-fsm-sdk-wt-v15gate/rust-toolchain.toml'` — the *authoring* (v15gate) worktree path, **not** the audit worktree and not the eventual tag worktree. The **substantive** claim (the pin is `1.75.0`) is independently confirmed (D-27: `rust-toolchain.toml` byte-unchanged `3bbcd0f..X`, reads `channel = "1.75.0"` at `X`). This is the Doc 00 §11.51 frozen-attestation pattern (a recorded toolchain assertion legitimately cites the context where it was captured; the *pin* is the load-bearing fact, and the pin is honored from inside any in-repo worktree). **NOT a misstatement** — the path is informational provenance, the pin is correct. | Optional, non-gating: a future metrics/status wave *could* note "the path is the authoring worktree; the pin 1.75.0 is the substantive invariant, byte-unchanged vs the v1.4 tag." No tag impact — the toolchain-probe-trap caveat (Doc 31 §6 H3 / Doc 00 §11.51) is correctly applied and the pin is independently re-derived. |
| F-02 | 1/3 | **P3** | `docs/GATE_VERIFICATION_v1_5.md:182` + `:188` (Pinned-numbers live-code method) | The method string says `sed -n '261,387p' … \| grep -cE …`. The `=> (Sev,` arms in fact span lines **264–384** (first/last EWIH arm, independently derived), so the `261,387` window is *generous* (correctly encloses the full macro body: 264 ≥ 261, 384 ≤ 387). The **count 73 is exactly correct** by the literal command AND a whole-file cross-count AND the by-severity split AND the CI-lock const (D-09). Identical in character to the v1.4 audit's F-03 P3 (a generous-but-correct upper bound). | Cosmetic — tighten the upper bound (e.g. `…,385p`) if a future wave touches the catalog. The pinned **73** is independently confirmed four ways; non-gating. |
| F-03 | 4 | **P3** | `docs/AUDIT_VSCODE_FRONTEND_PRACTICES_2026-05-17.md` / Doc 31 §6 H6 (frozen) | The frozen Doc 31 §6 H6 / §7 item 4 cite the stale ROADMAP pointer "at `docs/ROADMAP.md:248`". After this closeout's ROADMAP edit (which deleted the old line and added the multi-line reconcile + a retrospective + v1.6 candidates) the *line number* `248` is no longer where the (now-record-of-fix) text sits. This is **correct frozen-evidence behaviour** (Doc 31 reflects its pinned commit `df16055`; the closeout resolves the finding by *fixing the live pointer* — D-21 — not by overwriting the frozen plan, verified byte-untouched D-18) and is exactly the intended carry mechanism. | None — flagged only to record that the Doc 31 `:248` citation is a pinned-at-`df16055` line reference the closeout correctly resolved by fixing the live pointer (D-21), with Doc 31 properly NOT overwritten. |

**No P0. No P1.** All findings are precision/provenance/frozen-evidence-scoping notes. The corpus is internally consistent and the shipped source at `X` matches the §11.72–§11.79 record.

---

## Sequence note (necessary-not-sufficient — the v1.4-W4c F-05 analogue)

`TAG-CLEAR` from this audit is **necessary, not sufficient**. GATE §4.3/§4.4 correctly state the canonical cold-from-source quad at `X` — **with the MANDATORY NOTE-1 `cargo clean -p` STEP 0** (§4.2, binding v1.4+) — **and the re-activated JS lane** (§4.3, binding for v1.5 because the surface *is* `editors/vscode/**`: `npm audit` triaged + `vsce package` lint-clean + the 41-test Extension-Host suite green) "has not yet happened … the gating step the tag is contingent on" (honest prospective §11.30 posture — no fabricated transcript). The tag lands on `X` **iff** (a) this audit is TAG-CLEAR (it is) **and** (b) that canonical cold quad + JS lane at `X` is green. The empirical re-derivation (the cold quad; the Extension-Host suite) is **out of scope here by mandate** and is the binding independent empirical gate at the tag commit (the post-A2 keystone audit `d22ef3b`'s own 867/0 quad + C2's 41-test suite + the contingent quad-at-`X` are the empirical layer). This audit is RECORD INTEGRITY only; nothing here was built to close a claim — the one claim that cannot be confirmed without a run (the cold-quad+JS-lane green at `X`) is explicitly the later wave's to empirically close and is correctly posed as contingent by the GATE, not asserted.

---

## Mechanics

- Worktree: `/root/dev/embeded-fsm-sdk-wt-v15pretag` (pre-created, `git -C`-only, READ-ONLY, zero `cargo`/`npm`/build — the binding disk-protection constraint honored).
- Branch: `phase5.8/v15-pretag-audit` (off `X`=`2ff8ac3`); verified clean at start and at the time of writing.
- This doc is NEW (`docs/AUDIT_PRE_TAG_v1_5_2026_05_17.md`); no existing audit/doc overwritten; exactly one file written.
- Committed as `Add v1.5 pre-tag four-lens audit`. **A sibling evidence commit — NOT folded into `X`; the tag lands on `X` (the v1.4-W4c precedent). Not merged, not pushed, not tagged.** The orchestrator gates (this audit → the cold-quad+JS-lane at `X` → tag at `X`).

*— End of independent pre-`v1.5.0`-tag four-lens audit, 2026-05-17. A rubber-stamp here would be worse than a found problem; the v1.5 Rust-delta framing, the cardinal-error-FORM judgment-call, the Doc 00 append-only invariant, and the keystone are stated `TAG-CLEAR` because each was independently confirmed by this auditor's own derivation from `git`/source at `X`, not restated from the prior reports — the third-shape analogue of the v1.3/v1.4 cardinal exposures (a small-but-nonzero delta easy to mis-summarise as "no Rust change") was specifically hunted, the highest-stakes "is there a false whole-delta-is-zero assertion" judgment-call was independently re-judged by deriving A1's empty Rust delta from `git` and reading every regex hit, and the v1.4-W4c F-01 measurement-basis nuance was checked and found absent (X's Rust delta is byte-identical to the pinned basis).*
