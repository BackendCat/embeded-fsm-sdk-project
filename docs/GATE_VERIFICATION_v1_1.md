# GATE VERIFICATION — v1.1.0

- **Document ID:** GATE_VERIFICATION_v1_1
- **Date:** 2026-05-15
- **Scope:** v1.1.0 release gate — all commits since v1.0.0 `b480003` through the release commit `eb35d4b`.
- **Companion evidence (frozen, never edited):** `docs/AUDIT_PRE_TAG_v1_1_{CORRECTNESS,ARCH,RELIABILITY}_2026-05-15.md`; the v1.0 baseline `docs/GATE_VERIFICATION_v1_0.md`.
- **Verdict:** **PASS — v1.1.0 is tag-ready.** 0 open P0; the one P0 the gate found (SEC-P0-1) is fixed and merged; all other findings are explicitly tracked, non-blocking, and recorded below.

This document is the canonical release record for v1.1.0. It captures the MVP-gate status, the independent 3-lens pre-tag audit verdicts, the resolution of the one tag-blocker, the §11.1/§11.22-mandated **cold-from-source** quad transcript, the known limitations a consumer must be aware of, and the one post-tag action that requires the repository owner.

---

## 1. MVP gate (Doc 23 §9) — v1.1.0 status

| Gate | v1.1 status | Evidence |
|---|---|---|
| **G1** `fsm check` ok / clean reject, no panic, no UB | ✅ | PB1 fixed (leading-comment panic); nested-submachine rejected with `FSM-E0502` not broken C; panic-surface audited ≈ nil (Reliability audit "Verified-Robust") |
| **G2** `fsm check broken.fsm` → exit 1 + Rust-style error | ✅ | unchanged from v1.0; new diagnostics (E0903-retired/defer, FSM-E0300/W0300, E0502, import-header) each behaviourally tested |
| **G3** `fsm generate` emits the C unit set + HAL | ✅ | unchanged; per-machine strategy (W7), `--import-header` (W5, hardened SEC-P0-1), branch hints (W4) extend it |
| **G4** `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` zero-warning | ✅ | every §5.4 acceptance test compiles `-Werror` and **runs**; W6 proves it across make/cmake/cargo-rust/host-gcc |
| **G5** `fsm fmt` idempotent | ✅ | unchanged from v1.0 |
| **G6** all examples full-chain + sim-trace-match | ✅ | `fsm test examples/` **5/5** (motor, traffic-light, vending-machine, deferred, submachine) cold-rebuilt |
| **G7** ≥1 test per diagnostic code | ⚠️ tracked | 36/75 formal `tests/conformance/` fixtures; 39 covered by crate-level negative tests. Identical posture to v1.0; closure to 75/75 in the formal harness tracked for v1.1.x. **Not a behavioural gap** (the new v1.1 diagnostics are each behaviourally tested in their wave's §5.4 suite). |
| **G8** `cargo build/test/clippy/fmt` clean | ✅ | **cold-from-source** quad below: 673 pass / 0 fail, clippy `-D warnings` clean, fmt clean |
| **G9** CI matrix linux/macos/windows | ⚠️ tracked | `.github/workflows/ci.yml` well-formed; local-equivalent green; **never exercised** (repo unpushed by design — owner controls remote). Post-tag action §5. |

G7 + G9 are the **same explicitly-tracked partials accepted at the v1.0 gate** — neither is a v1.1 regression, both are documented in CHANGELOG `[1.1.0]` Known-limitations and the ROADMAP.

---

## 2. v1.1 surface delivered (since v1.0.0 `b480003`)

Infra/process: shared `CARGO_TARGET_DIR` + warm-cache policy; `SUBAGENT_CONVENTIONS` v1.1.0 (§5.4 behavioural-acceptance mandate, §11.1 post-merge quad, §11.22 cold-quad-for-tag).

Features/fixes (all merged, each with a real gcc-compile-and-RUN §5.4 acceptance test, traced in Doc 00 §11.19–§11.29):
W0 IR-schema gate + test-debt · W1 `defer EVENT` runtime · TD1 zero-transition table bugs · W3 architecture-debt paydown (one `ParentResolver` LCA, `LoweringCtx` god-object eliminated, sim dead-dep demoted) · W2a–d submachine epic (top-level) · P1-2 nested-submachine `FSM-E0502` reject · PB1 leading-comment panic · W4 `likely`/`rare` → `__builtin_expect` · W5 `--import-header` · OB1 opaque-extern silent-drop · W6 four worked integrations + Integration Guide · W7 per-machine strategy · W7-FU-1 guard-disambiguated dispatch (both strategies) · W7-FU-2 default-priority reconciliation (= 100, spec) · **SEC-P0-1** import-header G-02 hardening.

---

## 3. Independent 3-lens pre-tag audit (read-only; frozen evidence)

| Lens | Verdict | Key result |
|---|---|---|
| **Architecture** (`AUDIT_PRE_TAG_v1_1_ARCH_2026-05-15.md`) | 0 P0 — TAG-READY | AD-1/AD-2/AD-3 debt-paydown each **verified true in source** (not trusted from claim); dep graph acyclic; 9/9 `#![forbid(unsafe_code)]` held through 15 waves; determinism is a type-level property. 2 carried P1 (pub over-exposure, analyzer→CST coupling) — non-behavioural, ship-acceptable, gate before v1.2. |
| **Correctness** (`AUDIT_PRE_TAG_v1_1_CORRECTNESS_2026-05-15.md`) | 0 P0 | All Doc 00 §11.19–§11.28 rows: commit-exists + code-present + **0 overstated** — the P0-1/submachine prose-vs-code class is absent from the v1.1 surface. Blocker was only P1-A (the mandated cold quad — satisfied §4). |
| **Reliability / Security** (`AUDIT_PRE_TAG_v1_1_RELIABILITY_2026-05-15.md`) | 1 P0 (now fixed) | Test-integrity **strong** — every sampled §5.4 acceptance test is a real gcc-compile-and-RUN, none symbol-presence. Found **SEC-P0-1** (the one tag-blocker, §3.1). Its REL-P1-1 was **stale vs current code** (§3.2). |

### 3.1 SEC-P0-1 (the one P0) — RESOLVED `2b3d221`

`--import-header` / `fsm.toml import_headers` (a new v1.1 file-read surface) bypassed the v1.0 G-02 hardening (no canonicalize/containment, unbounded read → arbitrary-file-read + CI DoS). **Fixed by converging, not duplicating:** the attacker-controlled `fsm.toml` surface routes through `fsm_parser::import_resolver::resolve_import` directly (same primitive as DSL `import`); one shared `safe_io.rs` owns the single bounded-read helper + the single workspace-root resolver (the latter de-duplicated out of `cmd::check`). Trust-split decision: `fsm.toml` (travels with a possibly-hostile repo) contained by default; `--import-header` (invocation-supplied, trusted argv) NUL+cap only; vendored-HAL via explicit default-OFF opt-in. REL-P2-1 (fsm.toml read cap) folded in. 10 new security tests prove each attack rejected with the asserted exit code, target never read, never OOM; the W5 end-to-end gcc-RUN acceptance still passes unchanged. Doc 00 §11.28.

### 3.2 REL-P1-1 was stale vs current code — corrected, not regressed (Doc 00 §11.29)

The reliability audit's REL-P1-1 claimed nested-submachine-in-composite/parallel "emits non-compilable C" and recommended correcting the docs to say so. Verifying against current `crates/fsm-analyzer/src/checks/submachine.rs:106-135`: that case is **rejected at analysis with `FSM-E0502`** + an actionable message, codegen never reached, lowerer refuses too (defence-in-depth) — shipped by P1-2 `02d4ded`, test-pinned by `crates/fsm-analyzer/tests/nested_submachine_rejected.rs`. The auditor had traced the *older* `AUDIT_PHASE_SUBMACHINE` (pre-P1-2) doc. Blindly applying the recommendation would have regressed the release record into a falsehood. The record was corrected to the shipped reality. **Lesson: `verify-status-claims-vs-code` is symmetric — an audit asserting a defect is itself a claim that must be code-verified before the release record changes on its basis.**

---

## 4. Cold-from-source release quad (§11.1 / §11.22) — the tag gate

A warm shared-`CARGO_TARGET_DIR` can serve stale cross-worktree test binaries (the correctness audit reproduced this **live**: a deleted-`-wt-w7fu2`-path `fsm-formatter` test binary). §11.22 therefore mandates a **cold from-source** green quad for a release tag — a warm pass is necessary-but-not-sufficient.

Approach (recorded as the conventional CI-cache practice — fits the shared box without disk provisioning): `cargo clean -p` **all 9 first-party crates** (removes every first-party artifact **and every test/integration binary**, including the stale `-wt-`-tainted ones) while keeping the content-addressed third-party registry dep cache (Cargo-fingerprint-guaranteed path-independent — it cannot be "stale" in the `-wt-` sense). Everything the project authors, and every test binary, is then recompiled **from source in the main checkout** — no worktree path can survive.

Transcript (release commit `eb35d4b`, 2026-05-15):

```
cargo clean -p <9 first-party crates>  → Removed 1436 files, 6.4 GiB
df: 4.2G free → 11G free  (proves the cleaned bulk was first-party/test-bin
                            artifacts; the 6.8 G registry dep cache retained)
cargo build --workspace                → exit 0 (from-source)
cargo test  --workspace                → 673 passed, 0 failed
                                         -wt- stale-path lines: 0   ← cold-clean
cargo clippy --workspace --all-targets -- -D warnings → exit 0
cargo fmt --check --all                → clean
fsm test examples/                     → 5 / 5
fsm test tests/conformance/            → 25 / 25
df after full cold quad: 3.4G free     (stayed safe throughout — no provisioning)
```

`-wt- stale-path lines: 0` on a cold rebuild is the decisive §11.22 evidence: the hazard the correctness audit reproduced is provably absent from the artifacts this tag is cut from.

---

## 5. Post-tag action required of the repository owner (G9)

The tag and the ~40 unpushed commits are **local only** (the owner controls the remote — a hard project rule). The GitHub Actions matrix (linux/macos/windows) has **never executed against any v1.1 commit**; the local-equivalent quad is green but platform-specific behaviour (Windows path canonicalization — exactly the surface SEC-P0-1's fix touches) is unverified. **The owner should push the tag + commits to exercise CI, and keep a v1.1.1 patch lane ready.** This is documented, accepted to tag before CI runs only because the local quad is green and the patch lane exists — but it must be exercised promptly post-push, not deferred.

---

## 6. Sign-off

v1.1.0 meets the release gate: 0 open P0, the one P0 found by the gate (SEC-P0-1) fixed and cold-quad-verified, scope statements code-verified and conservative (no overstatement; the recurring submachine prose-vs-code class confirmed absent and the one stale *audit* claim corrected not propagated), all residual findings explicitly tracked in CHANGELOG `[1.1.0]` and the ROADMAP. Tagging `v1.1.0` (annotated, local) with `checkpoint/2026-05-15` as the rollback anchor.
