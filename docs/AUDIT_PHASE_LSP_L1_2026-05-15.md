# AUDIT_PHASE_LSP_L1_2026-05-15

- **Document ID:** `AUDIT_PHASE_LSP_L1_2026-05-15`
- **Date:** 2026-05-15
- **Lens:** Read-only phase-boundary auditor — independent prose-vs-code verification of v1.2 LSP epic wave L1 (Doc 26 §8 + Doc 00 §11.3 mandated post-L1 gate).
- **Scope:** L1 only, at main HEAD `31baa63` ("Implement fsm-lsp L1: LSP spine reusing the exact fsm check pipeline"). Source + design + test-body inspection only. No build run (box at disk ceiling); the clean §11.1 quad at `31baa63` (692 pass / 0 fail; clippy `-D warnings`; fmt; `fsm test examples/` 5/5; conformance 25/25) is trusted as the green baseline. Working tree verified clean at `31baa63`.

---

## VERDICT

**PROCEED to L2: YES**

**Open P0 count: 0**

L1's central promises hold against the code. The reuse seam is a faithful line-for-line mirror of `check.rs` (no re-implemented analysis, no shell-out). `position.rs` is a genuinely-new third *correct* converter, deliberately scoped, with honest DRIFT-2 wording (not overstated as converged). The §5.4 acceptance harness is a real in-process `tower-lsp` client asserting byte-exact Ranges (independently verified against the fixture's actual bytes). No L2 scope creep and no silent no-op handlers. One **P3** doc-precision nit on the "10/10 forbid-unsafe" wording (the `fsm-lsp` *bin target root* unit lacks the attribute though the lib does and no `unsafe` exists anywhere) and the standard pre-existing **P2** DRIFT-1/DRIFT-2 doc-reconciliation debts (correctly tracked, not introduced or relied upon by L1). None is an L2 blocker.

---

## L1 claims → verified?

| Claim | Verdict | Note |
|---|---|---|
| Reuse-seam mirrors `check.rs` exactly (no re-impl, no shell-out) | ✅ | `analysis.rs:60-84` step-for-step == `check.rs:46,59,60,61,65`; same `resolve_import` error-vs-silent matrix; zero `std::process::Command`. |
| position.rs correct + honest scope (DRIFT-2 NOT "converged") | ✅ | 0-based, negotiated-encoding, math verified; the two legacy impls confirmed divergent (analyzer bytes/1-based, CLI scalars/1-based); §11.32(2) wording is honest. |
| §5.4 behavioural acceptance (real client, byte-exact Range, non-ASCII both encodings) | ✅ | In-process `LspService` client; `assert_eq!(got[0].range, want[0].range)` + independent hard-coded col cross-check; fixture bytes recomputed and match exactly. |
| No L2 scope creep / no silent no-op handlers | ✅ | Only `initialize/initialized/shutdown/didOpen/didChange/didClose` + `publishDiagnostics`; zero hover/def/completion/etc.; `--port` fails loud, not silent. |
| `#![forbid(unsafe_code)]` → workspace 10/10 | ⚠️ | True at *crate/lib* granularity (10/10 crates carry it); the `fsm-lsp` **bin target root** (`main.rs`) is the one compilation root without the attribute. No `unsafe` exists anywhere. P3 doc-precision only. |
| `workspace_root_for` re-impl behaviour-identical, correct architecture (not a #64 regression) | ✅ | `analysis.rs:95-111` == CLI fsm.toml-walk semantics incl. empty-parent + fallback; CLI helper genuinely `pub(crate)` *in the binary crate*; no public API narrowed. |
| Doc 26 reuse-seam table vs code (no aspirational drift relied upon) | ✅ | §3/§5 table matches `analyze`; §4.5 honestly flags `parse_incremental`/DRIFT-1 as non-existent and L1 does NOT depend on it (full re-parse on debounce). |

---

## Findings (triaged)

### P0 — none

No correctness, security, or architecture defect that blocks L2 was found.

### P1 — none

### P2

**P2-1 · DRIFT-2 (legacy line/col converters not converged) — correctly deferred, must stay tracked**
- *Evidence:* `crates/fsm-analyzer/src/util.rs:163-178` (`compute_line_col`: counts **bytes**, `line=1/col=1` → **1-based**); `crates/fsm-cli/src/cmd/check.rs:197-212` (`line_col`: `src.char_indices()` → **Unicode scalars**, 1-based); `crates/fsm-lsp/src/position.rs:1-37` (the new **third** 0-based negotiated-encoding impl); `docs/00-Decisions-And-Reconciliation.md` §11.32(2).
- *Why it matters:* Three divergent byte→line/col impls coexist. This is the project's signature defect *class*. The risk for the audit is overstatement — claiming convergence that did not happen.
- *Assessment:* **Not a defect of L1.** position.rs explicitly does not share with or "fix" the other two; §11.32(2) and the position.rs module doc both state plainly "NOT converged in L1 … tracked as DRIFT-2 … not a third silent copy." Wording is honest (conservative, completed-tense, scoped) — exactly the §11.29 lesson applied. Converging the legacy two would change two *shipped* subsystems' observable output, which the project bar forbids in a feature wave.
- *Recommended action:* Keep DRIFT-2 open in the v1.2 backlog; address in a dedicated behaviour-neutral refactor wave (not smuggled into an L-wave). No L1 change.
- *L2 blocker?* **No.**

**P2-2 · DRIFT-1 (`parse_incremental` aspirational prose) — flagged, not relied upon**
- *Evidence:* `docs/26-LSP-Architecture.md` §4.5 (lines 290-308) flags Doc 20 §4.5 / L787 `parse_incremental` as describing code that does not exist; `crates/fsm-lsp/src/server.rs:122` + `document_store.rs:58-64` confirm L1 does **full** re-parse on debounce (rebuilds `LineIndex` + calls `analyze` on the whole buffer).
- *Why it matters:* Same aspirational-prose class as P0-1. A wave silently assuming `parse_incremental` exists would be the recurring trap.
- *Assessment:* L1 correctly does NOT depend on it; Doc 26 §4.5 honestly annotates it as v1.3+ aspiration. The "Doc 20 §4.5/L787 should be annotated as not-implemented in a later doc pass" remains an open documentation-reconciliation debt (acknowledged out-of-scope for the read-only wave that wrote Doc 26).
- *Recommended action:* Track the Doc 20 §4.5/L787 annotation as a doc-reconciliation backlog item before any incremental-parse epic. No L1 code change.
- *L2 blocker?* **No.**

### P3

**P3-1 · "workspace now 10/10 forbid-unsafe" is true per-crate but not per-compilation-root for the fsm-lsp bin target**
- *Evidence:* `crates/fsm-lsp/src/lib.rs:32` has `#![forbid(unsafe_code)]`; `crates/fsm-lsp/src/main.rs` (the `[[bin]] fsm-lang-server`, `Cargo.toml:15-17`) has **no** crate-root attribute (verified head of file; `grep -c forbid main.rs` = 0). The workspace invariant is per-crate-root `#![forbid]` (no `[workspace.lints]` in `Cargo.toml`). Every other crate has exactly one compilation root and it carries the attribute (libs on `lib.rs`; `fsm-cli` has only `main.rs`, which has it). `fsm-lsp` is the sole crate with *two* roots (lib + bin); only the lib root is covered.
- *Why it matters:* The CHANGELOG / §11.32(4) claim "workspace now 10/10 forbid-unsafe" is accurate at *crate* granularity but, read strictly against "verify per-crate-root incl. the bin target," the `fsm-lang-server` binary's own ~45-line crate root (arg dispatch in `main.rs`) is not under `#![forbid(unsafe_code)]`. The lesson of this project is that wording must match code exactly.
- *Assessment:* **Functionally inert.** No `unsafe` token exists anywhere in `fsm-lsp` (src or tests; grep clean); `main.rs` is a trivial arg switch that delegates all logic to `fsm_lsp::run_stdio()` (lib, forbid-covered). The green quad's clippy `-D warnings` would catch any real lint. This is a doc-precision / defence-in-depth nit, not a safety hole.
- *Recommended action:* Add `#![forbid(unsafe_code)]` to `crates/fsm-lsp/src/main.rs` (one line) so the bin crate root literally upholds the invariant the docs claim, OR soften the doc wording to "10/10 crates" and note the bin root is covered transitively. Defer to an L2 housekeeping commit; not worth a standalone wave. *(NOTE: this audit is read-only; no edit made.)*
- *L2 blocker?* **No.**

**P3-2 · Acceptance test `_sink` is dropped; outbound client→server sink unused (test-design observation, not a defect)**
- *Evidence:* `crates/fsm-lsp/tests/lsp_client_acceptance.rs:149,188,242,317` (`let (mut requests, _sink) = socket.split();`).
- *Why it matters:* Worth recording that the harness drives requests via `LspService::call` directly (the canonical tower-lsp test pattern) and only *reads* the server→client `RequestStream`; the `_sink` (server→client responses to server-initiated requests) is intentionally unused because L1 issues none.
- *Assessment:* Correct for L1 (the server makes no `client.*` *requests*, only `publish_diagnostics`/`log_message` notifications, which arrive on `requests`). Not a gap. Flagged only so an L2 auditor knows the harness will need the sink once L2 adds server→client requests (e.g. `workspace/configuration`).
- *Recommended action:* None for L1. L2 capability waves that add server-initiated requests must extend the harness to service `_sink`.
- *L2 blocker?* **No.**

---

## Verified clean (positive evidence — the gate needs proof, not just findings)

1. **Reuse seam is byte-faithful to `check.rs`.** `analysis.rs::analyze` (`analysis.rs:60-84`) executes, in order: (1) `fsm_parser::parse(src)` == `check.rs:46`; (2) `workspace_root_for(uri_path)` == `check.rs:59`; (3) `security_check_imports` *before* analyze == `check.rs:60`; (4) `fsm_analyzer::analyze_with_source(&pr, &path, src)` == `check.rs:61`; (5) `diagnostics.append(&mut import_diags)` == `check.rs:65`. `security_check_imports` (`analysis.rs:124-161`) is a line-for-line mirror of `check.rs:157-195` incl. the **identical** `resolve_import` match arms: `Ok`→silent, `BadShape`→silent (parser already diagnosed), `Unresolved`→silent (sibling may not exist), `OutsideWorkspace`→hard `into_diagnostic(span)`. The span-recovery (first `StringLiteral` token of `IMPORT_DECL`, fallback `Span::new(0,0)`) is identical. The only documented difference (in-memory buffer vs disk read) is correct and the security-containment still uses the URI path. No `std::process::Command` / `fsm` invocation anywhere in `crates/fsm-lsp/src/` (grep clean) — confirming the §11.32(3) "no shelling to the `fsm` binary" claim.

2. **`workspace_root_for` re-impl is behaviour-identical and correct architecture.** `analysis.rs:95-111`: starts at `path.parent()` filtered for non-empty (fallback `"."`), walks parents for `fsm.toml` is_file, falls back to `start` — semantically the CLI's `fsm.toml`-walk. The CLI's helper is genuinely `pub(crate)` *inside the `fsm-cli` binary crate* (not cross-crate API; Doc 26 §6 does not list the CLI as a seam); re-implementing an 11-line walk rather than making the LSP depend on the CLI binary is the correct dependency direction. Nothing cross-crate-public was narrowed → not a #64 pub-hygiene regression. §11.32(3) wording matches.

3. **position.rs math is correct under both encodings.** `LineIndex::new` (`position.rs:102-117`) pushes 0 then every byte-after-`\n` → `line_starts[0]==0`, strictly increasing, trailing-`\n` yields a final entry. `position()` (126-145): clamps `byte.min(len)` (past-EOF safe, no panic), `partition_point(|s| s<=byte)-1` → correct 0-based line, slices `[line_start, byte)`, then UTF-8 = `segment.len()` (byte count) / UTF-16 = `Σ c.len_utf16()`. `range()` (151-161) normalises inverted spans. **Independently recomputed against the real `non_ascii.fsm` bytes:** line-5 prefix `"    state A { /* 🚀 ы */ "` = **28 UTF-8 bytes / 25 UTF-16 units**; with `"on GO -> Nope "` appended = **42 / 39**; absolute byte span **(90,104)** — *exactly* the values the acceptance test hard-codes (`lsp_client_acceptance.rs:285-288`) and the in-test comment predicts. The three-way divergence (byte 28 / scalar 23 / UTF-16 25 at the start) is real, so a naive byte- or scalar-as-character shim provably fails the test — the risk-1 proof is genuine.

4. **§5.4 acceptance is a real client, not a presence stand-in.** `lsp_client_acceptance.rs` builds `LspService::new(Backend::new)`, splits the `ClientSocket`, issues real `jsonrpc::Request`s (`initialize`/`didOpen`/`didChange`) via `service.call`, and drains the server→client `RequestStream` for the actual `textDocument/publishDiagnostics` notification (5 s bounded). Four tests: (a) UTF-8 negotiated + `textDocumentSync==1` + clean doc → empty; (b) broken doc → `assert_eq!(got[0].range, want[0].range)` and `assert_eq!(got[0], want[0])` byte-exact vs the pipeline oracle, code `FSM-E0107`; (c) **both** UTF-8 and UTF-16 over the non-ASCII fixture with byte-exact Range vs per-encoding oracle **plus** independent hard-coded `(28,42)`/`(25,39)` column cross-check and a `>25` cross-encoding-divergence assertion (so a bug corrupting oracle+server identically still fails); (d) `didChange` fix → diagnostics clear, with a fixture-invariant assert that the edited buffer is clean via the same pipeline. No `.contains()` / symbol-presence acceptance anywhere.

5. **No L2 scope creep, no silent stubs.** `capabilities/mod.rs` exposes only `pub mod diagnostics`. `server.rs` `impl LanguageServer` implements exactly `initialize`, `initialized`, `shutdown`, `did_open`, `did_change`, `did_close` — grep for `async fn hover|goto|completion|references|rename|semantic|code_action|folding|inlay|document_symbol|signature|formatting` returns **none**. `initialize` advertises only `position_encoding` + `text_document_sync: FULL` (`..Default::default()` for the rest → no capability falsely advertised). `main.rs` `--port` fails loud (`exit 2`), unknown args fail loud — no silent fallthrough (the cardinal-sin guard is honoured).

6. **Position-encoding negotiation is spec-correct and matches §11.32(1).** `server.rs:143-153`: reads `params.capabilities.general.position_encodings`; UTF-8 iff the client lists it, else UTF-16 (one branch covering both the 3.17-utf16-only and pre-3.17-no-capability cases — the clippy-collapsed single branch §11.32(1) describes). `OffsetEncoding::from_lsp` (`position.rs:67-73`) defensively maps anything non-UTF-8 → UTF-16 (universally-correct fallback, never a silent-wrong UTF-8 assumption). Server echoes the negotiated encoding back in `ServerCapabilities.position_encoding`; the acceptance test asserts the echo per encoding.

7. **Debounce + lifecycle are sound.** Generation-counter debounce (`server.rs:84-129`): each edit bumps the URI generation; the spawned task after 200 ms publishes *only if* its captured generation is still current — burst-collapses with no `JoinHandle` bookkeeping. `did_close` removes the debounce entry (late run can't publish; map can't grow) and clears diagnostics (LSP convention). Doc mutex is released before the await-free `analyze` (snapshot pattern) — no lock held across analysis. `document_store` rebuilds `LineIndex` on every `replace` so it never drifts from `text`; `replace`/`close` before `open` are no-ops, not panics.

8. **Severity mapping faithful (no Hint→Info collapse).** `capabilities/diagnostics.rs:27-34` maps Error→ERROR, Warning→WARNING, Info→INFORMATION, Hint→HINT distinctly — Doc 26 §4.2 explicitly forbids the `miette`-style fold; unit test `severity_does_not_collapse_hint_into_info` pins it. `code` is `format!("{}", d.code)` = the `FSM-XNNNN` wire string identical to `check.rs`'s `--json` `code` field; `source: "fsm"`; related-info projected with its own per-span Range.

9. **Invariants & hygiene (with the P3-1 caveat).** `#![forbid(unsafe_code)]` present at `lib.rs:32` (+ `#![deny(missing_debug_implementations)]`); `fsm-lsp` is the 10th entry in `Cargo.toml` `[workspace] members`. Zero `#[allow(...)]` and zero `unsafe` in `crates/fsm-lsp/src/` or `tests/` (grep clean) → no lint masking; the clippy `-D warnings` claim is plausible. Dependency closure per `Cargo.toml`: `fsm-diagnostics`(serde, no miette — no server-side terminal render), `fsm-parser`, `fsm-analyzer`, `tower-lsp`, `tokio`, `serde_json` — matches Doc 26 §2.1. L1 commit `31baa63` touched only `crates/fsm-lsp/**`, `Cargo.{toml,lock}`, `CHANGELOG.md`, and `docs/00` §11.32 (+3 lines) — no shipped subsystem modified, consistent with "L1 must not change analyzer/CLI behaviour."

---

## Auditor's note on conservatism

Every claim above is cited to `file:line` and was read in the source, not taken from the L1 completion report or CHANGELOG prose. The two items I could not *fully* close from code alone are honestly rated, not passed: the clippy/`-D warnings` and full-suite-green claims (no build run — disk ceiling) are rated *plausible* on the trusted §11.1 baseline + a clean `#[allow]`/`unsafe` grep, **not** independently re-proven here. The fixture byte-math (the load-bearing risk-1 proof) WAS independently recomputed and matches exactly. No unverifiable claim is counted as a pass. Per the project's hardest-won lesson (§11.29, symmetric verify-status-claims-vs-code), this audit asserts a green gate only where HEAD code is the evidence.
