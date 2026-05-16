# FSM Studio — v1.3 VS Code Extension Epic: Wave Plan + Doc 27 ↔ Shipped-fsm-lsp Reconciliation

**Document ID:** FSM-PLAN-VSCE-V13
**Version:** 1.0.0
**Status:** Living Document — v1.3 epic-kickoff *planning* artifact. Authoritative
for the v1.3 implementer-wave queue, the Doc 27 ↔ shipped-`fsm-lsp` drift
ledger, the Node-toolchain owner-decision, and the §11.49 CST-coupling
sequencing. Does **not** contradict Doc 27 (FSM-ARCH-VSCE) — it *re-validates*
Doc 27 against the code as actually shipped at HEAD `55ecc50` and pins the
wave gates. Where Doc 27's prose or `file:line` citations have gone stale
against shipped reality, **shipped reality wins** and the drift is recorded
in §1.
**Depends on:** FSM-ARCH-VSCE (27), FSM-ARCH-LSP (26), FSM-SPEC-VSCE (22),
FSM-SPEC-TM (21), FSM-SPEC-UI (05), FSM-SPEC-LSP (14), FSM-SPEC-CLI (18),
FSM-SPEC-DEC (00 §11.30/§11.40–§11.49), FSM-PROC-SUBAGENT, FSM-GATE-v1.2
(`GATE_VERIFICATION_v1_2.md`).

---

## 0. Why a new Doc 28 (numbering justification)

Doc 26 (FSM-ARCH-LSP) was authored as a **new sibling** of Doc 14
(FSM-SPEC-LSP) for the v1.2 LSP-epic kickoff — it did **not** edit Doc 14;
it added an engineering architecture + an L1–L7 wave plan scoped against the
*then-current code*. The v1.3 analogue is symmetric: Doc 27 (FSM-ARCH-VSCE)
is the v1.3 *architecture* (the Doc-14 analogue — the package shape, the
capability-reuse map, the deliberate scope-outs). This Doc 28 is the
*epic-kickoff planning pass* (the Doc-26 analogue): it re-validates Doc 27
against the **shipped** `fsm-lsp` at HEAD `55ecc50`, flags every prose-vs-code
drift, pins the V1–Vn gates with real behavioural-acceptance definitions,
sequences the §11.49 debt, and produces the V1 implementer brief. Editing
Doc 27 in place would lose the kickoff-pass-as-distinct-artifact pattern that
caught the Doc-20 §4.5 false-incremental-parse drift; a new numbered sibling
is the convention-consistent choice. Doc 27 remains the architecture of
record; Doc 28 is the **plan of record** built on it.

The orchestrator folds the ROADMAP §144 pointer + a Doc-00 §11 row in a
separate reconciliation pass — this read-only planning wave touches **only
this new doc**.

---

## 1. Doc 27 ↔ shipped-`fsm-lsp` reconciliation (the highest-leverage unknown)

**Method.** Doc 27 was committed at `63e6d26` (merged `10304b5`) and every
reuse/drift claim it makes cites HEAD `4f7e9ea`. The current HEAD is
`55ecc50`. Between `4f7e9ea..55ecc50` the `fsm-lsp`/`fsm-cli`/`fsm-ir`
*code* changed (verified `git diff --stat 4f7e9ea..55ecc50`):
`fsm-cli/src/cmd/check.rs` (+75), `fsm-cli/src/diagnostics.rs` (+195),
`fsm-cli/src/config.rs` (+15), `fsm-ir/Cargo.toml` (+16),
`fsm-ir/src/json.rs` (+9), `fsm-lsp/Cargo.toml` (+5),
`fsm-lsp/src/capabilities/semantic_tokens.rs` (±28),
`fsm-lsp/src/position.rs` (±47), plus the DRIFT-2 line/col convergence
(`7cf1174`/`d399623`), the FU#67 allow/deny wiring (`24f231d`), FU#68
`unreachable_pub` (`21a2380`), FU-DEAD-CODES (`0747321`/`12ba06a`), and
RUSTSEC-2026-0009 (`e509734`). Doc 27's `file:line` citations and the Doc-26
"two divergent converters" framing it inherits are therefore audited
line-by-line below. Verdict legend: ✅ = Doc 27 affirmed accurate vs shipped;
⚠️ = Doc 27 still correct in substance but a citation/framing is now stale
(re-pin, non-load-bearing); ❌ = Doc 27 asserts something shipped reality
contradicts (none found — see summary).

| # | Doc 27 claim (load-bearing for the client binding) | Verdict | Shipped reality at `55ecc50` (file:line) |
|---|---|---|---|
| R-1 | Doc 27 §2 / §10: there is **no `editors/` dir**; the entire TS/Node surface is greenfield | ✅ | `ls` top-level = `crates docs examples schema tests` (no `editors/`); confirmed **also absent at `4f7e9ea`** (`git ls-tree 4f7e9ea`). Zero `.ts`/`package.json`/`node_modules` in-tree. Holds. |
| R-2 | Doc 27 §2.2 / §10: server bin is `fsm-lang-server`, **stdio only**; `--port` prints the error and exits `2`; `--version`/`-V`/`--help`/`-h` supported | ✅ | `main.rs:39-48` `--port` → `eprintln!("error: --port (TCP transport) is not implemented …")` + `ExitCode::from(2)`; `main.rs:23-37` version/help; `main.rs:53-65` `None → run_stdio()`. Doc 27 cited "`main.rs:39-48`" — **still exact**. |
| R-3 | Doc 27 §2.2: `lib.rs:165-169` `LspService::new(Backend::new)` over `tokio::io::stdin/stdout` | ✅ | `lib.rs:165-170` `pub async fn run_stdio()` = exactly that. Doc 27's `lib.rs:165-169` is off-by-one at the closing brace (`:170`) — immaterial. |
| R-4 | Doc 27 §2.3 / §2.5 / §8: positionEncoding negotiation — UTF-8 **iff** client lists it in `general.positionEncodings`, else UTF-16; advertised back in `initialize` | ✅ **(load-bearing — affirmed)** | `server.rs:216-226` reads `params.capabilities.general.and_then(g.position_encodings)`, `negotiated = if contains(UTF8) {Utf8} else {Utf16}`; `server.rs:239` `position_encoding: Some(negotiated.to_lsp())`. Doc 27 cited `server.rs:216-239`/`216-225` — **still exact**. The one-line "do not strip the default `general.positionEncodings`" client constraint (Doc 27 §2.3, V1 acceptance assertion (c)) binds correctly. |
| R-5 | Doc 27 §2.5 / §10: server reads **exactly four** `fsmLang.*` inlay keys via `InlayHintConfig::from_settings`, nested + flat, defensive defaults (master/priorities/timers ON, state-types OFF) | ✅ | `config.rs:74-114` `from_settings` reads exactly `fsmLang.enableInlayHints` / `inlayHints.{showTransitionPriorities,showStateTypes,showTimerDurations}`, both shapes, default-on-missing/mistyped. Doc 27 cited `config.rs:33,36,40,43,99-108` — the **field decls are now `config.rs:33,36,40,43`** (exact) and the **reads are `config.rs:99-111`** (Doc 27's `:99-108` is short by 3 lines — re-pin to `:99-111`; non-load-bearing). |
| R-6 | Doc 27 §2.5 / §10: the four inlay keys flow via `initializationOptions` (read at `initialize`) AND `workspace/didChangeConfiguration` (live override) | ✅ | `server.rs:233-235` `if let Some(opts) = &params.initialization_options { *inlay_cfg = InlayHintConfig::from_settings(opts) }`; `server.rs:855-857` `did_change_configuration` re-reads the four keys live. **Doc 27 cited `server.rs:233-235` (still exact) and `server.rs:843+` for `didChangeConfiguration` — STALE: it is now `server.rs:855-857`** (shifted +12 by intervening edits). Behaviour identical; re-pin the citation. |
| R-7 | Doc 27 §2.5 / §10: `fsmLang.debounceMs` is a **server no-op** — debounce is a hard-coded `const DEBOUNCE = Duration::from_millis(200)` | ✅ ❌(for the *setting*, correctly flagged) | `server.rs:100` `const DEBOUNCE: Duration = Duration::from_millis(200)`; `config.rs` parses no `debounceMs`. Doc 27 cited `server.rs:100` — **still exact**. The Doc-22 `debounceMs` setting is genuinely dead server-side exactly as Doc 27 §10's ❌ row states. |
| R-8 | Doc 27 §2.5 / §10: `maxProblems` / `diagnostics.*` / `simulator.port` / `codegen.*` / `format.*` are server no-ops; `codegen.*`/`format.*` are legitimately ext-command-consumed | ✅ | `config.rs` doc-comment (ll.1-20) + `from_settings` confirm only the four inlay keys are parsed; everything else stub-ignored ("Doc 26 §7 open-question 8"). Doc 27 §2.5/§10 ❌/⚠️ verdicts hold verbatim. |
| R-9 | Doc 27 §3 / §6.1.1 / §9 / §10: there is **NO custom LSP method** (`fsm/diagram`/`fsm/ir`), **no `experimental` capability**, **no `workspace_symbol_provider`** | ✅ **(the diagram DRIFT anchor — affirmed)** | `grep` of `server.rs` for `workspace_symbol|experimental|fsm/diagram|fsm/ir|on_request|custom_method` = **zero hits**. `ServerCapabilities` literal `server.rs:238-360` ends `..Default::default()` (all unadvertised providers off). The advertised set is exactly: `position_encoding`, `text_document_sync:FULL`, `document_symbol`, `folding_range`, `hover`, `definition`, `completion`(triggers `[". " ": " "@" "[" " "]`, `resolve:false`), `references`, `rename`(`prepare:true`), `semantic_tokens`(full+range), `code_action`(kinds `quickfix`+`refactor`, `resolve:false`), `inlay_hint`(`resolve:false`). Doc 27 §6.1.1 is **verified correct against code, not prose**. |
| R-10 | Doc 27 §6.1.2 / §9 / §10: `fsm ir` (Doc 18) is **NOT implemented**; the CLI `Command` enum has no `Ir` | ✅ | `cli.rs:8-11` defer comment (`ir`,`completions` "intentionally NOT exposed … defer those to post-v1.0"); `cli.rs:30-54` `enum Command` = `Parse/Check/Generate/Fmt/Test/Doc/Decompile/Init` — **no `Ir`**. Doc 27 §6.1.2 holds. |
| R-11 | Doc 27 §6.1.3 / §6.2 / §10: the **one** working IR-JSON path is `fsm generate --emit-ir` → `<machine>.ir.json` via `fsm_ir::to_json`, and it is **codegen-gated** (IR written only after the generated C `fs::write` succeeds) | ✅ **(the diagram data-source — affirmed)** | `generate.rs:171-209`: codegen `emit()` first, `fs::write` each emitted file, **then** `if args.emit_ir { … fsm_ir::to_json(&ir) → fs::write(<first-machine>.ir.json) }`. `fsm_ir::to_json` is `crates/fsm-ir/src/json.rs:43` (`from_json` at `:52`). Doc 27 cited `generate.rs:189-209` / `fsm-ir/src/json.rs:43` — the `if args.emit_ir` block is now at `generate.rs:189-209` (still exact) and `to_json` at `json.rs:43` (still exact). The codegen-gated boundary Doc 27 §6.1.3 flags is real and unchanged. |
| R-12 | Doc 27 §3: `publishDiagnostics` at `server.rs:199` via `analysis.rs::analyze` = exact `fsm check` pipeline | ✅ | `server.rs:199` `.publish_diagnostics(uri, lsp_diags, Some(version))`. `lib.rs:136-140` confirms `analysis::analyze` = `fsm_parser::parse` + `fsm_analyzer::analyze_with_source` + `resolve_import` in `fsm check`'s exact order. Doc 27's `server.rs:199` — **still exact**. |
| R-13 | Doc 27 §3 / §10: the per-capability advertisement lines (`server.rs:251` documentSymbol … `:352-358` inlayHint) and `shutdown` (`server.rs:375`) | ✅ | Verified each against the `ServerCapabilities` literal: `:251` `document_symbol_provider`, `:252` `folding_range`, `:257` `hover`, `:258` `definition`, `:270-280` `completion`, `:286` `references`, `:294-297` `rename`, `:310-319` `semantic_tokens`, `:336-342` `code_action`, `:352-358` `inlay_hint`, `:375-377` `shutdown`. **All Doc 27 capability-block citations still exact** (the post-Doc-27 edits were to `position.rs`/`semantic_tokens.rs`/CLI, not the `server.rs` capabilities block). |
| R-14 | **DRIFT-2 framing** — Doc 27 §6.3 / §2.3 inherits Doc 26 §4.1's "**two** divergent byte→line/col converters" model | ⚠️ **STALE FRAMING (substance unaffected)** | The DRIFT-2 convergence (`7cf1174`/`d399623`, Doc 00 §11.44) shipped **after** Doc 27. Shipped reality is now **three intentionally-different contracts** with a **converged linear-scan core** `fsm_diagnostics::compute_line_col(src,pos,LineColUnit::{Byte\|Scalar})` (analyzer→`Byte`, CLI→`Scalar`), and the LSP's `position.rs::LineIndex` **deliberately left structurally separate** (0-based, negotiated unit, O(log n) `Vec<u32>` line-start table — `position.rs:104-205`). **Client-binding impact: NONE.** The extension never touches the converged core; it binds only to the LSP's *wire* `Range`/`positionEncoding`, whose semantics are byte-unchanged by DRIFT-2 (Doc 00 §11.44 explicitly: "the LSP `LineIndex` is left-and-explained"). Doc 27's *conclusion* (the squiggle is un-drift-able) holds; only the "two converters" *prose* is stale. **Action: a wave reading Doc 27 §2.3/§6.3 must read it through this row** — do not re-derive the binding from the obsolete two-converter story. |
| R-15 | **Post-Doc-27 oracle change** — Doc 27 §3/§8 use `fsm check --json` as the V1 diagnostics oracle; `check.rs` cited `:46,61` for the reuse seam and `:197` for `line_col` | ⚠️ **STALE CITATION (oracle still valid, stronger)** | `24f231d` reorganized `check.rs` (+75) and `diagnostics.rs` (+195) to wire `fsm.toml [compiler] allow/deny` through `fsm check` (FU#67, Doc 00 §11.41). Consequence: (a) `check.rs` no longer has `fn line_col` at `:197` — DRIFT-2 moved line/col into `fsm_diagnostics::compute_line_col`; Doc 27 §10's `check.rs:197` citation is **dead, re-pin to `fsm_diagnostics::compute_line_col`**. (b) `security_check_imports`/`resolve_import` are now `check.rs:87,29`. (c) **The oracle is now allow/deny-aware**: `fsm check --json` output for a file under an `fsm.toml` with `[compiler] allow/deny` is *projected* post-analysis (`diagnostics::apply_allow_deny`). **V1 acceptance impact (must be in the V1 brief):** the V1 cross-check fixture MUST either have no `fsm.toml [compiler]` allow/deny in scope, OR the brief must state that the LSP does **not** apply allow/deny (the LSP runs `analyze` + `resolve_import` but **not** the CLI's `apply_allow_deny` projection — verified: `apply_allow_deny` is `fsm-cli`-only, the `fsm` binary does not depend on `fsm-lsp` and vice-versa). Otherwise the oracle and the squiggle legitimately differ and V1 fails on a non-bug. This is a **new V1 boundary Doc 27 could not have known** and is the single most actionable reconciliation finding for the V1 implementer. |
| R-16 | **Post-Doc-27 legend visibility** — Doc 27 §3/§4: `semanticTokens` legend free via the client | ✅ (reinforced) | `21a2380` (FU#68) downgraded the 11 token-type + 3 modifier legend consts from `pub` → `pub(crate)` (`semantic_tokens.rs:84-94`). **Client-binding impact: NONE — strictly reinforces Doc 27 §4.** The extension's only contract with the legend is the **wire-advertised** `SemanticTokensLegend` (`server.rs:313` `legend: semantic_tokens_legend()`), which `vscode-languageclient` consumes automatically; no Rust API is needed. Legend *content* unchanged (NAMESPACE..DECORATOR / DECLARATION/READONLY/STATIC). |
| R-17 | **Post-Doc-27 dep bump** — Doc 27 §6.2 pins the diagram to the `fsm_ir::to_json` schema | ✅ (verified schema-neutral) | `e509734` bumped `jsonschema 0.17→0.22` (RUSTSEC-2026-0009; `fsm-ir/Cargo.toml`, `fsm-ir/src/json.rs:104-115`). Cargo.toml comment + `json.rs` diff confirm "the structural-Draft7 API this crate uses is unchanged across 0.17→0.22"; `to_json`/`from_json` **signatures unchanged**. Doc 27 §6.2's "feed canonical `fsm-ir` JSON to the Webview" data-source decision is **unaffected**; the `<machine>.ir.json` wire-format is stable. |
| R-18 | Doc 27 §10: implied cross-file goto/refs/rename + `workspaceSymbol` are **unsupported** (single-file only) | ✅ | `server.rs` advertises no `workspace_symbol_provider`; CHANGELOG `[1.2.0]` + Doc 00 §11.34(3)/§11.36(3) confirm single-file; cross-file/`workspaceSymbol`/`wasm32`-`fsm-lsp` are explicitly v1.3 *server-side* deferrals (ROADMAP §112, Doc 26 §9). Doc 27 §9's scope-out is correct; the extension surfaces exactly the server's single-file boundary and must not imply cross-file works. |

### 1.1 Reconciliation summary — the two findings every wave inherits

1. **No structural drift; Doc 27's architecture is sound against shipped
   code.** Every load-bearing claim (stdio-only, the four inlay keys, the
   positionEncoding negotiation, **no custom LSP/diagram method**, **no
   `fsm ir`**, the codegen-gated `--emit-ir` as the one IR path, single-file
   boundary) is **affirmed accurate** against `55ecc50` (R-1..R-13, R-18).
   The Doc 27 §8 V1–V6 plan can be built on as-is, *modulo* the two
   re-pin/awareness items below.
2. **Two stale-but-non-load-bearing items the V-waves MUST be briefed with:**
   - **R-14 (framing):** Doc 27 §2.3/§6.3 inherits Doc 26's obsolete
     "two converters" prose; shipped reality post-DRIFT-2 is "three
     contracts + converged core, LSP `LineIndex` left-separate". The
     client binding is **unchanged** (the extension binds to the *wire*
     `Range`/`positionEncoding`, which DRIFT-2 left byte-identical).
     Read Doc 27 §2.3/§6.3 through R-14.
   - **R-15 (V1 oracle boundary — the actionable one):** the post-Doc-27
     FU#67 change made `fsm check --json` **allow/deny-aware**. The V1
     behavioural acceptance cross-checks the LSP squiggle against
     `fsm check --json`; the LSP does **not** run the CLI's
     `apply_allow_deny` projection. **V1's fixture must have no
     `fsm.toml [compiler] allow/deny` in scope** (or the brief must state
     the LSP-vs-CLI projection difference explicitly) — else V1 fails on a
     non-bug. This is the single highest-value reconciliation output for
     the V1 implementer and is hard-wired into the §5 brief.

   Citation re-pins (non-load-bearing, for doc hygiene when the
   orchestrator folds the Doc-00 row): `didChangeConfiguration`
   `server.rs:843+` → **`:855-857`**; inlay reads `config.rs:99-108` →
   **`:99-111`**; `line_col` `check.rs:197` → **gone, now
   `fsm_diagnostics::compute_line_col`**; `security_check_imports`
   → **`check.rs:87`**. Doc 27 is otherwise citation-exact.

**Net: the wave plan below builds on shipped reality, not on Doc 27's
pre-shipped assumptions, and the only behaviourally-relevant correction
(R-15) is folded into the V1 gate.**

---

## 2. Infra owner-decision: the Node/TypeScript toolchain on the disk-constrained box

**This is the second-highest-leverage unknown and MUST be owner-resolved
BEFORE any implementer wave runs `npm install`.** Surfaced per the
infra-constraint-escalation discipline (SUBAGENT §7 final bullet: "surface a
hard disk ceiling as a decision rather than grinding") — presented as a
crisp owner-decision with options + a non-blocking recommended default. The
planner does **not** pre-decide for the owner.

### 2.1 The measured constraint (verified, not assumed)

| Fact | Measured value (this box, `2026-05-16`) |
|---|---|
| Disk total / used / free | **78 G / 63 G used / 12 G free / 85 % used** (`df -h /`) |
| Shared cargo target (`/root/dev/embeded-fsm-sdk-target`, out-of-tree per `.cargo/config.toml:22`) | **9.8 G now** (warm-with-`fsm-lsp` it creeps toward the ~12-19 G the brief/SUBAGENT §7 cite; bounded, not runaway) |
| Other `/root/dev` projects | ~20 projects, next-largest single dir 2.7 G; the Rust shared target is by far the biggest consumer |
| **Cold release-quad disk envelope** (the binding constraint — `GATE_VERIFICATION_v1_2.md` §4.3) | the v1.2 cold-from-source release quad recorded **`3.0 G → (clean) 22 G → (rebuild) 12 G = 15.4 % free`** — i.e. the *mandatory* release gate already transiently drops the box to **~3 G free** and rebuilds to ~12 G. v1.3's release quad inherits this. |
| Node toolchain present | **already installed** via nvm: `node v24.13.1`, `npm 11.8.0` (`~/.nvm/versions/node/v24.13.1`) — *newer* than a typical VS Code extension target |
| Rust pin | `rust-toolchain.toml` channel **`1.75.0`** (load-bearing — see Doc 00 §11.42: the 1.75 pin already forced the `jsonschema 0.22` floor and blocks `cargo-audit 0.21.1`) |

**The core tension.** A VS Code extension introduces `node_modules`
(`vscode-languageclient` + `esbuild` + `@vscode/test-electron` +
`@vscode/vsce` + `elkjs` + typescript + eslint: empirically **300–800 MB**
for an extension of this shape) **plus** a headless VS Code download
(`@vscode/test-electron` fetches a full VS Code build, **~150–250 MB**, into
a cache) **plus** the per-run Extension-Host disk churn. Stacked on a box
whose *mandatory release gate already touches ~3 G free*, an uncoordinated
`npm install` + first `@vscode/test-electron` run is the precise
disk-cliff the escalation discipline exists to pre-empt. This is the
**dominant non-LSP risk** Doc 27 §7 risk-1 already named ("a Node/TS
toolchain entering a pure-Rust Cargo workspace") — Doc 28 quantifies it and
turns it into an owner-decision.

### 2.2 Fixed (non-decision) parameters — extracted from Doc 27, stated precisely

These are *not* owner-options; they are determined by Doc 27 / Doc 22 / the
shipped server and are stated so the owner decides only the genuinely-open
axis (footprint placement + CI shape):

- **Where it lives:** a **new top-level `editors/vscode/`** (Doc 27 §2; Doc
  22 §12; Doc 21 §3 `editors/vscode/syntaxes/`). **NOT a Cargo workspace
  member** — it is not a crate; `Cargo.toml` `members` is untouched (Doc 27
  §7 risk-1 mitigation, confirmed: `Cargo.toml:3-13` lists only `crates/*`).
- **Node version:** pin an **LTS** in `editors/vscode/.nvmrc` +
  `package.json` `engines.node`. The box has `v24.13.1`; do **not** silently
  rely on it — the brief pins the exact line (recommend **Node 20 LTS**, the
  `@vscode/test-electron`/`vscode-languageclient ≥8` well-trodden baseline;
  Node 24 works but is not what the VS Code extension ecosystem CI-tests
  against — pinning removes a "works-on-this-box-only" trap, the §7
  disk-hygiene/repro discipline applied to Node).
- **VS Code engine floor:** `engines.vscode ^1.85.0` (Doc 27 §2.1, **hard
  floor** — that baseline ships `vscode-languageclient ≥ 8`, the version
  that sends `general.positionEncodings` so the shipped server's UTF-8
  fast-path (R-4) is reachable; an older client silently regresses to
  UTF-16, losing the Doc 26 §4.1 defect-class deletion).
- **Bundler:** `esbuild` (Doc 27 §8 V1 "build = esbuild bundle") — one fast
  dev-dep, no webpack tree.
- **Test runner:** `@vscode/test-electron` (Doc 27 §8 — the mandated
  §5.4-analogue: a real headless Extension Host, the client round-tripping
  against the **real** `fsm-lang-server` binary; **not** a `.ts`-compiles
  check, **not** symbol-presence).
- **Lockfile:** `package-lock.json` committed; CI uses `npm ci` (reproducible,
  the npm analogue of the cold-quad's from-source discipline).

### 2.3 The owner-decision (3 options + a non-blocking recommended default)

**Open axis:** *where the Node/VS-Code-test footprint lives and how its CI
relates to the Rust quad, given the box already touches ~3 G free at the
mandatory release gate.* The planner recommends a default but does **not**
decide — this is an owner call (the infra-escalation discipline: surface,
don't grind).

| Option | What it is | Disk / CPU posture | Trade-off |
|---|---|---|---|
| **A — Separate additive JS CI lane, host-only `@vscode/test-electron`, cache pinned out-of-tree (RECOMMENDED DEFAULT)** | `editors/vscode/` builds/tests in a **separate, additive** CI job (`npm ci → tsc → eslint → esbuild → @vscode/test-electron`), gated **independently** of the Rust cargo quad (Doc 27 §7 risk-1 recommendation). The `@vscode/test-electron` VS Code download is pinned to an **out-of-tree cache dir** (e.g. `~/.vscode-test`, the tool's default — *not* under `/root/dev`), mirroring how the cargo target is already out-of-tree (`.cargo/config.toml:22`). `node_modules` lives under `editors/vscode/` and is `.gitignore`d. | `node_modules` ~300–800 MB + VS Code cache ~150–250 MB, **both reclaimable and out of the cargo-target blast radius**. The Rust cold release quad's ~3 G-free trough is **unaffected** (different trees; the JS lane never runs concurrently with a cold cargo rebuild because they are separate gated jobs). CPU: `@vscode/test-electron` is a real Electron run (~30–90 s/suite) — additive, not on the Rust critical path. | Two CI lanes to maintain; a developer must `npm ci` once (~1 GB transient). **Lowest risk**: the Rust workspace stays byte-untouched, the disk-cliff is structurally avoided by keeping both big caches out-of-tree and the lanes non-concurrent. This is the v1.1/v1.2 "keep the cache warm + out-of-tree, surface the ceiling" posture applied to Node. |
| **B — Shared CI job, JS after Rust, aggressive prune** | One CI job runs the cargo quad then the JS lane; `node_modules` pruned (`npm ci --omit=dev` for packaging, full only for the test step) and the VS Code test cache deleted after each run. | Lower steady footprint (prune between phases) but **higher peak**: if the JS step's `npm ci` + VS Code download lands while the cargo target is warm-large (~12–19 G), peak free can dip toward the release-quad danger zone (~3 G). Mitigable with explicit `rm -rf` between phases (the SUBAGENT §7 incremental-reclaim pattern) but couples the two toolchains' disk lifecycles. | Single lane (simpler mental model) but **couples Rust and JS disk lifecycles** — exactly the coupling the v1.1 per-worktree-target multiplication incident taught us to avoid (SUBAGENT §7). A JS failure can now mask/precede a Rust signal. Not recommended on a box already at 85 % used. |
| **C — Defer the headless-extension-host test infra to a later v1.3 sub-wave; V1 ships with a lighter (non-Electron) client smoke** | V1's behavioural acceptance uses a lighter harness (e.g. driving `vscode-languageclient` against the real `fsm-lang-server` binary in a Node process **without** a full Extension Host), and the full `@vscode/test-electron` lane lands as a gated later wave once the owner provisions disk. | Smallest immediate footprint (no VS Code download for V1); ~300–500 MB `node_modules` only. | **Weakens the V1 gate.** Doc 27 §8's whole point (and the §5.4 mandate, and PD-7 depth-first) is that V1 proves the spine **in a real editor**. A non-Electron smoke is closer to the symbol-presence trap the project was bitten by (P0-1). Acceptable **only** if the owner judges disk genuinely cannot absorb ~250 MB more before a provisioning step — and even then the §5.4-grade Electron test must be a hard gate before the v1.3 tag, not dropped. **Not recommended; listed for completeness.** |

**Recommended default (non-blocking): Option A.** It is the direct
application of the project's proven posture — keep the big cache out-of-tree
(as `.cargo/config.toml` already does for the cargo target), keep the lanes
non-concurrent and independently gated, and surface the absolute ceiling
rather than grinding clean/rebuild loops. It structurally removes the
disk-cliff (the two large caches never co-peak with the cold cargo quad's
~3 G trough) at the cost of one extra CI lane. **Owner action required
before V1 dispatch:** confirm A (or pick B/C), and confirm there is
headroom for a one-time ~1 GB transient `npm ci` + ~250 MB VS Code download
**outside** the `/root/dev` cargo-target tree. If the owner cannot confirm
~1.5 G of reclaimable headroom outside the cargo trough, that is itself the
escalation signal — provision disk **before** V1, do not start V1 and
grind. **No `npm install` runs until this row is owner-resolved.**

---

## 3. Wave breakdown V1..V6 (depth-first; real behavioural-acceptance gates)

Doc 27 §8 already defines a sound V1–V6 decomposition. Doc 28's job is to
**pin each wave's gate to a real behavioural-acceptance definition** (the
§5.4 analogue for an extension), enforce the **PD-7 depth-first rule** (a
thin end-to-end vertical slice as the hard V1 gate before any breadth), and
fold the R-15 oracle correction into V1. The wave *content* is Doc 27 §8
verbatim; the *gates* below are the binding acceptance contracts.

**The §5.4 analogue for a VS Code extension (binding for every wave).**
SUBAGENT §5.4 mandates "compile-and-RUN, assert observable behaviour, never
symbol-presence" — the rule that would have caught P0-1 ~8 waves earlier.
For the LSP that became the in-process `tower-lsp` client (`LspService::new`
+ `tower::Service::call` + `ClientSocket` drain — verified
`tests/lsp_client_acceptance.rs:1-90`, **35 behavioural tests**, oracle
recomputed from the *exact* pipeline). **For the extension the mandated
analogue is `@vscode/test-electron`:** launch a real VS Code Extension Host
with the extension + the **real `fsm-lang-server` binary**, drive editor
actions via the `vscode` API, and assert **observable editor state** —
`vscode.languages.getDiagnostics(uri)` returns the exact `FSM-Exxxx` + Range,
`executeDocumentSymbolProvider` returns the tree, the Webview posts the
IR-derived model. **`package.json`-has-the-command / activate-fn-is-exported
/ the-`.ts`-compiles is NOT acceptance — that is the symbol-presence trap at
the extension layer.** A wave that only unit-tests JS internals or asserts
manifest shape is **incomplete** and is rejected back for an Extension-Host
behavioural test (the orchestrator enforces this exactly as it did for the
35 LSP wave tests; SUBAGENT §5.4 + §11.1).

**PD-7 depth-first (the hard V1 gate).** V1 is a **thin end-to-end vertical
slice**: extension activates in a real headless VS Code + connects to the
**shipped `fsm-lang-server`** + **one real feature (live diagnostics)
round-trips in-editor**. Breadth (grammar, snippets, commands, diagram,
trees, multi-platform) is **forbidden in V1** and only begins after V1's
Electron gate is green and a phase-boundary audit (SUBAGENT §11.3) passes.
This mirrors LSP-L1 proving the analyzer-reuse + positionEncoding seam
before any L2–L7 breadth.

### V1 — MVP spine: scaffold + language client + live diagnostics (the hard depth-first gate)
- **Content (Doc 27 §8 V1, verbatim):** `editors/vscode/` greenfield (TS,
  `package.json` identity Doc 22 §1, activation Doc 22 §2 =
  `onLanguage:fsm-lang` + `workspaceContains:**/*.fsm` — **never `*`**,
  build = esbuild); `vscode-languageclient ≥ 8` wired to `fsm-lang-server`
  over **stdio** with the Doc 22 §2.2 binary-resolution order
  (`fsmLang.compilerPath` → bundled host-triple → Doc 22 §12 error, **no
  silent fallback**); Doc 22 §13.2 crash-recovery `errorHandler`; the **four
  real** `fsmLang.*` inlay keys via `initializationOptions` +
  `didChangeConfiguration`; status bar from client state; **host-platform
  binary only**. Grammar/snippets/diagram/commands/trees explicitly OUT.
- **Behavioural-acceptance gate (`@vscode/test-electron`, all required):**
  (a) open a **known-broken** `.fsm` in a real Extension Host → assert
  `vscode.languages.getDiagnostics(uri)` contains the **exact** `FSM-Exxxx`
  code(s) **and the exact Range** that `fsm check --json` reports for the
  **same source** — the established CLI oracle, the LSP-L1 §5.4 cross-check
  pattern, proving the client surfaces the *shipped pipeline*;
  **(R-15 boundary — MANDATORY): the fixture MUST NOT have an
  `fsm.toml [compiler] allow/deny` in scope** (the LSP runs `analyze` +
  `resolve_import` but **not** the CLI's post-Doc-27 `apply_allow_deny`
  projection; an allow/deny fixture makes the oracle and the squiggle
  legitimately differ and fails V1 on a non-bug); (b) edit to fix → assert
  diagnostics clear; (c) assert the negotiated `positionEncoding` is
  **UTF-8** under the ≥ 8 client (the R-4 / Doc 27 §2.3 guard — a stripped
  `general.positionEncodings` capability fails this, a regression caught by
  CI not a user); (d) a fixture whose first error is **after a non-ASCII
  line** (emoji in a `///`) → assert the Range is correct (the Doc 26 §4.1
  defect-class guard, inherited end-to-end through the client).
  **Manifest-presence is NOT acceptance — the asserted diagnostic Range
  bytes are.** A phase-boundary audit (SUBAGENT §11.3) runs after V1 before
  any V2+ dispatch.

### V2 — TextMate grammar + language-configuration + snippets (static assets)
- **Content (Doc 27 §8 V2):** package Doc 21 §3 grammar + Doc 21 §4
  `language-configuration.json` + Doc 22 §9 `snippets/fsm-lang.json`
  verbatim; the Doc 22 §3 `contributes.{languages,grammars,snippets}`.
- **Behavioural-acceptance gate:** `vscode-tmgrammar-test` snapshot over a
  representative `.fsm` corpus → assert the **tokenized scope stream**
  matches expected `source.fsm` scopes (a real tokenization assertion, not
  "the file is contributed"); an Extension-Host test that with the server
  **off** the buffer still colours (TextMate-only) and with the server
  **on** semantic tokens refine it (the Doc 21 §6 coexistence, *observable*
  — and consistent with R-16: the legend is wire-advertised, the client
  merges it). Snippet bodies asserted to expand to the Doc 22 §9 text.

### V3 — thin CLI-wrapper + client-control commands
- **Content (Doc 27 §8 V3):** `fsm.checkFile`, `fsm.generateC99`,
  `fsm.generateCpp17`, `fsm.copyIR`, `fsm.formatDocument`,
  `fsm.restartLanguageServer`, `fsm.showOutputChannel`; Doc 22 §4/§5/§6;
  `fsmLang.codegen.*` consumed by the generate commands; command-title
  prefix normalized to `FSM Studio:` (Doc 27 §2.5/§10 — IDs stay `fsm.*`).
- **Behavioural-acceptance gate:** Extension-Host test invokes
  `fsm.generateC99` on a real fixture → assert the expected C file appears
  in `fsmLang.codegen.outputDir`; `fsm.copyIR` → assert the clipboard holds
  JSON that `fsm_ir::from_json` round-trips (proves it is the real
  `--emit-ir` artifact per R-11/R-17, not a stub) **and** the test fixture
  is one that **codegen-succeeds** (the R-11 codegen-gated-IR boundary —
  `--emit-ir` writes nothing if codegen fails; the command must surface that
  honestly, asserted here); `fsm.restartLanguageServer` → assert the client
  transitions stopped→running and diagnostics re-publish. Behavioural — the
  file/clipboard/client-state, not "the command is registered".

### V4 — diagram WebviewPanel: read-only IR-sourced ELK view (the substantive subsystem; the §6 DRIFT lives here)
- **Content (Doc 27 §8 V4 / §6.2):** `fsm.openDiagram` → `WebviewPanel`
  (split-right, one-per-machine, focus-existing); data = `fsm generate
  --emit-ir` → `<machine>.ir.json` → `postMessage` to the Webview; ELK
  Layered (`elkjs`) layout in-Webview; Doc 05 §1.5.3–§1.5.10 **read-only**
  render (states/composite/regions/pseudo-states/transitions, pan/zoom,
  click→editor line, double-click→definition, context menu, SVG/PNG export,
  legend, live re-render). CSP-locked `postMessage`-only boundary.
  Interactive editing + simulator overlay explicitly OUT (Doc 27 §9). **No
  new server method / no `fsm ir` subcommand** (R-9/R-10 — that is a
  separate epic, not smuggled here).
- **Behavioural-acceptance gate:** Extension-Host test opens the diagram for
  a known fixture → drive the Webview and assert the rendered model has
  **the exact state/transition set of the fixture's `fsm-ir` JSON** (parse
  the same `--emit-ir` output as the oracle — a real structural assertion,
  not "a webview opened"); click a rendered state → assert the editor
  selection moves to that state's declaration line (via the IR
  `SourceLocation`); feed a **codegen-failing but parse-OK** fixture →
  assert the **last-valid render persists + the Doc 05 §1.5.9 banner shows**
  (the R-11 codegen-gated boundary, **proven not assumed** — this is the
  DRIFT-class assertion); SVG export → assert a well-formed SVG with the
  state labels. A phase-boundary audit (SUBAGENT §11.3) runs after V4 (the
  one substantive new subsystem) before V5/V6.

### V5 — activity-bar tree views + context-key chrome
- **Content (Doc 27 §8 V5):** `fsm.machineExplorer`/`fsm.eventExplorer`
  `TreeDataProvider`s fed by the **free `documentSymbol`** response; Doc 22
  §7 view containers; Doc 22 §11 context keys
  (`fsm.hasOpenFsmFile`/`fsm.serverRunning`) driving `when`-clauses.
- **Behavioural-acceptance gate:** Extension-Host test opens a multi-machine
  fixture → assert the Machines tree's nodes equal the client's
  `executeDocumentSymbolProvider` result **exactly** (re-projected, not
  re-analyzed); toggle a `.fsm` open/closed → assert
  `fsm.hasOpenFsmFile`-gated views appear/disappear; clicking a tree node
  reveals the declaration.

### V6 — multi-platform binary bundling + VSIX packaging (infra-gated on G9)
- **Content (Doc 27 §8 V6):** the Doc 22 §12 5-platform
  `bin/{triple}/fsm-lang-server` layout + the platform-detection resolver;
  `@vscode/vsce package` → installable `.vsix`; the Doc 22 §12 "no bundled
  binary for {platform}" honest degradation per-triple. Publishing/signing
  is OUT (Doc 27 §9 risk-4 — owner credential/product decision).
- **Dependency (binding):** requires the **G9 cross-OS CI matrix to exist**
  (Doc 27 §7 risk-2). G9 has **never executed against any v1.2 commit**
  (`GATE_VERIFICATION_v1_2.md` §1 G9 / §5 — local-only repo, owner controls
  the remote). If G9 is still absent the orchestrator runs V6 **host-only**
  and records the multi-platform tail as **blocked-on-G9** (infra-escalation,
  not grind — the same posture as the v1.2 §5 post-tag owner action).
- **Behavioural-acceptance gate:** per available platform in the (G9)
  matrix, an Extension-Host smoke that the bundled `fsm-lang-server` for
  that triple launches and produces a correct diagnostic on a broken fixture
  (V1's gate, re-run per-triple — proves the bundled binary on that OS works,
  especially the SEC-P0-1 path-canon seam, Doc 27 §7 risk-2); an un-bundled
  triple shows the Doc 22 §12 error, **not a silent dead client**.

**Sequencing rationale (depth-first, PD-7).** V1 (spine — client↔shipped-
server + positionEncoding seam end-to-end, the riskiest bet, validated
first + phase-audited, exactly as LSP-L1 did) → V2/V3 (static assets + thin
glue, low-risk, independently parallelisable **after V1's gate is green**)
→ V4 (the one substantive new subsystem, the diagram, where the §6 DRIFT
class lives — gated after V1 so the client + IR-CLI seams are proven, and
phase-audited after) → V5 (chrome reusing the now-proven free
`documentSymbol`) → V6 (the infra-gated multi-platform tail, last because it
depends on G9). Phase-boundary audits (SUBAGENT §11.3) after **V1** (the
spine) and after **V4** (the new subsystem).

---

## 4. Sequencing the deferred §11.49 CST-coupling cleanup

**The decision (Doc 00 §11.49, quoted):** the `analyzer→parser-CST coupling`
cleanup (arch-audit P1, **~15 files**, the *most behaviourally-critical
crate* `fsm-analyzer`) is "DEFERRED out of the v1.2 critical path —
accepted-tracked-debt … a tracked **v1.2.1/early-v1.3** wave, DRIFT-2-grade
discipline". It is explicitly **ship-acceptable, non-behavioural** P1, and
v1.1.0 set the precedent of shipping this exact debt class tracked +
documented (`GATE_VERIFICATION_v1_2.md` §6).

**Sequencing decision: a dedicated v1.3-W0 debt-paydown wave, run BEFORE
V1, NOT in a separate v1.2.1 patch lane, NOT parallel with the feature
waves.** Rationale:

1. **It is a refactor of `fsm-analyzer` — the crate the entire LSP (and
   thus the entire VS Code extension) transitively depends on for *every*
   feature** (R-12: the squiggle, symbols, hover, completion, refs all flow
   through `fsm_analyzer::analyze_with_source`). The §11.49 cleanup is
   exactly the kind of "narrow/reshape the substrate" work Doc 26 §6 ran as
   **L0 before** the LSP L1–L7 (the pub-hygiene wave preceded the LSP waves
   so the LSP built on a deliberately-shaped surface). The symmetric move
   for v1.3 is a **W0 before V1**: pay the analyzer-coupling debt on a
   stable, fully-tested `fsm-analyzer` *before* a new high-fan-out consumer
   (the extension's Electron test corpus) starts depending on its behaviour.
   Doing it after V1+ means a ~15-file refactor of the load-bearing crate
   while the extension's behavioural tests are simultaneously pinning its
   wire behaviour — maximal regression-blast-radius, the exact
   "big-risky-refactor-at-a-boundary" EV the §11.49 owner decision rejected.
2. **Not a v1.2.1 patch lane.** §11.49 says "v1.2.1 **or** early v1.3". A
   v1.2.1 patch lane is for *fixes to shipped v1.2 behaviour* (the
   `GATE_VERIFICATION_v1_2.md` §5 post-tag CI/SCA lane). The CST-coupling
   cleanup is **non-behavioural** (no user-visible v1.2 change) — putting it
   in a patch lane misframes it as a v1.2 fix and risks contending with a
   real v1.2.1 hotfix. As **v1.3-W0** it is correctly scoped as v1.3 epic
   prep, sequenced before the feature waves it de-risks, and recorded in the
   v1.3 gate doc — precedent-consistent with the v1.1→v1.2 L0 pattern.
3. **Not parallel with V1–V6.** SUBAGENT §10 forbids two writer agents in
   overlapping surface and the §11.1 warm-stale-target caveat makes
   concurrent refactor + feature waves on a shared `CARGO_TARGET_DIR`
   especially hazardous. W0 must land + post-merge-quad-green +
   phase-audit-clean **before** V1's analyzer-dependent Electron tests are
   written, so V1's oracle is computed against the *post-cleanup*
   `fsm-analyzer`, never a moving target.

**Discipline W0 inherits (binding):** it is **DRIFT-2-grade** (Doc 00
§11.44 / SUBAGENT §10 last row — the *refactor-to-number / leave-and-explain*
anti-pattern). W0 restructures **only** where a clean behaviour-safe
restructuring genuinely improves clarity; where the coupling cannot be
cleanly removed without worsening clarity or risking behaviour, W0
**leaves it and explains why** in a code comment + the §11 record (exactly
as DRIFT-2 left the LSP `LineIndex` structurally separate rather than
folding it to hit a dedup count). W0 is **non-behavioural**: its acceptance
is the SUBAGENT-§5.4 *pre/post-identity* proof — the full `fsm-analyzer`
test corpus + the 35 LSP `tower-lsp` client tests + `fsm test examples/`
5/5 + conformance 26/26 pass **byte-unchanged** before and after (the
"behavioral tests prove pre/post identity" rule for refactor waves, SUBAGENT
§10 row "Refactor waves that change behavior"). It ships **0 new deps**
(Cargo.lock unchanged, the DRIFT-2 precedent) and is recorded in the v1.3
gate doc as the closed §11.49 item.

**Net sequence:** **W0 (§11.49 CST-coupling paydown, before any feature
wave) → [owner resolves §2 Node decision] → V1 → audit → V2/V3 → V4 →
audit → V5 → V6 (G9-gated).**

---

## 5. V1 implementer brief (precise, self-contained)

> **You are the implementer for V1. You implement it yourself — do NOT
> delegate this further to another agent (the [[feedback_subagent_r1_misread]]
> rule: a wave brief is an instruction to *do the work*, not to re-dispatch
> it; sub-delegation of an implementer wave is the misread that rule
> exists to stop). You own `editors/vscode/` end-to-end for V1.**

**Precondition (HARD GATE — do not start until both are true):**
1. The §2 Node-toolchain owner-decision is **resolved** (the owner has
   picked Option A/B/C and confirmed reclaimable headroom outside the
   `/root/dev` cargo-target tree). **No `npm install`/`npm ci` runs before
   this.** If unresolved, stop and surface it — do not grind.
2. **v1.3-W0 (§11.49 CST-coupling paydown) has landed, post-merge-quad-green,
   phase-audit-clean.** V1's diagnostics oracle must be computed against the
   post-W0 `fsm-analyzer`.

**Scope (exactly Doc 27 §8 V1 — the depth-first spine, nothing more):**
`editors/vscode/` greenfield TS; `package.json` identity (Doc 22 §1) +
activation `onLanguage:fsm-lang` + `workspaceContains:**/*.fsm` (Doc 22 §2 —
**never add `*`**); esbuild bundle; `vscode-languageclient ≥ 8` →
`fsm-lang-server` over **stdio** (`TransportKind.stdio`) with the Doc 22
§2.2 binary-resolution order (`fsmLang.compilerPath` verbatim → bundled
host-triple → Doc 22 §12 error notification, **no silent fallback** — the
cardinal-sin bar at the client boundary); Doc 22 §13.2 exponential-backoff
`errorHandler` (3 restarts, 3 s base, 3× mult, 60 s reset); the **four
real** `fsmLang.*` inlay keys (R-5) via `synchronize.configurationSection:
"fsmLang"` + `initializationOptions`; status bar (Doc 22 §10) from client
`onDidChangeState`; **host-platform binary only**.

**Scope boundary — DO NOT (SUBAGENT §6):** no TextMate grammar (V2), no
snippets (V2), no commands (V3), no diagram Webview (V4), no tree views
(V5), no multi-platform bundling (V6). Do **not** touch `crates/*`,
`Cargo.toml`, `rust-toolchain.toml`, or any `docs/*` (V1 is purely additive
under `editors/vscode/`). Do **not** add `editors/vscode` to `Cargo.toml`
`members` (it is not a crate). If you discover you need an out-of-scope
file, report it — do not silently expand.

**Read these (named sections only, do not dump):** Doc 28 §1 (the R-4 / R-14
/ **R-15** reconciliation — R-15 is the boundary that will make your oracle
test fail on a non-bug if you ignore it); Doc 27 §2.2/§2.3/§2.5/§8-V1; Doc
22 §1/§2/§8/§12/§13; the shipped `crates/fsm-lsp/src/main.rs` (stdio/`--port`
contract, R-2) and `crates/fsm-lsp/tests/lsp_client_acceptance.rs:1-90` (the
§5.4-LSP harness shape — your `@vscode/test-electron` test is its
extension-layer analogue: drive the real server, recompute the oracle from
the real pipeline, byte-compare).

**The behavioural-acceptance test you MUST ship (the V1 gate — not
optional, not symbol-presence):** a `@vscode/test-electron` suite that
launches a **real headless VS Code Extension Host** with the V1 extension +
the **real `fsm-lang-server` binary**, and asserts all of:
- (a) open a **known-broken** `.fsm` → `vscode.languages.getDiagnostics(uri)`
  contains the **exact** `FSM-Exxxx` code(s) **and exact Range** that
  `fsm check --json` reports for the same source. **R-15 (MANDATORY): the
  fixture has NO `fsm.toml [compiler] allow/deny` in scope** — the LSP runs
  `analyze`+`resolve_import` but **not** the CLI's `apply_allow_deny`
  projection; an allow/deny fixture makes oracle≠squiggle *correctly* and
  fails V1 on a non-bug. Use a fixture directory with no `fsm.toml
  [compiler]` table.
- (b) edit to fix → diagnostics clear.
- (c) the negotiated `positionEncoding` is **UTF-8** under the ≥ 8 client
  (R-4 / Doc 27 §2.3 — assert the client did not strip
  `general.positionEncodings`; a regression fails CI, not a user).
- (d) a fixture whose first error is **after a non-ASCII line** (emoji in a
  `///` doc comment) → the Range is **correct** (the Doc 26 §4.1
  defect-class guard, end-to-end through the client).
**Manifest-presence (`package.json` has X / `activate` is exported / the
`.ts` compiles) is explicitly NOT acceptance — the asserted diagnostic
Range bytes are the proof.** A test suite that only checks manifest shape or
unit-tests JS internals is **incomplete** and will be rejected back
(SUBAGENT §5.4 / §11.1).

**Verification quad (SUBAGENT §4 — non-negotiable, the JS-lane analogue):**
`npm ci` → `tsc --noEmit` (typecheck clean) → `eslint` (clean) → `esbuild`
bundle (succeeds) → `@vscode/test-electron` suite (the (a)–(d) gate, green).
The **Rust workspace cargo quad must remain green and untouched** (V1 adds
no Rust surface — confirm `git status` shows only `editors/vscode/` +
`.gitignore`). Per §2/SUBAGENT §7: check `df -h /` before starting; the JS
lane's caches (`node_modules`, the `@vscode/test-electron` VS Code download)
must live **out of the `/root/dev` cargo-target tree** (Option A); if disk
is tight *during* the wave, surface it — do not `cargo clean` (that forfeits
the warm Rust cache for a JS-side problem).

**§11.x discipline inherited:** SUBAGENT §5.4 (behavioural-acceptance, the
P0-1 lesson — your Electron test is the analogue of gcc-compile-and-RUN),
§6 (scope boundary — the DO-NOT list above), §7 (disk hygiene / surface the
ceiling), §8 (judgment-call disclosure — every non-obvious choice in your
completion report), §11.1 (the orchestrator independently re-runs your gate
post-merge — self-reported green is not proof), §11.3 (a phase-boundary
audit runs after V1 before any V2+ dispatch).

**Completion report (SUBAGENT §1.12 / §8):** branch, LOC delta, the
Extension-Host test count + the exact (a)–(d) assertions, every judgment
call (esp. the host-triple binary-resolution choice and the R-15
fixture-isolation handling), the JS-quad confirmation transcript, and
explicit confirmation that the Rust cargo quad is byte-untouched.

---

## 6. Risks + v1.3-tag gate criteria

### 6.1 Risks (Doc 27 §7 carried forward, re-validated against `55ecc50` + the two Doc-28 additions)

| # | Risk | Severity | Status / mitigation |
|---|---|---|---|
| K-1 | Node/TS toolchain on the 85 %-used box; the mandatory cold release quad already touches ~3 G free | **HIGH (structural)** | **§2 owner-decision (default Option A: out-of-tree caches + separate non-concurrent CI lane).** Must be owner-resolved before any `npm install`. The single biggest non-LSP risk; quantified here from real `df`/gate figures, not assumed. |
| K-2 | Cross-platform binary bundling ties to the **never-run G9 matrix** + the platform-divergent SEC-P0-1 path-canon | **HIGH** | V6 ships **host-only as MVP**; full 5-platform tail is **gated on G9 existing** (Doc 27 §7 risk-2). G9 status: never run vs any v1.2 commit (`GATE_VERIFICATION_v1_2.md` §1/§5). Surface "multi-platform depends on G9" as an explicit owner dependency (infra-escalation). |
| K-3 | The diagram data-source DRIFT (no LSP/`fsm ir` IR method; only codegen-gated `--emit-ir`) | **MEDIUM (handled)** | **Verified still true at `55ecc50` (R-9/R-10/R-11/R-17).** Architecture pinned to the one real path; V4 must NOT introduce a server method (separate epic). Residual codegen-gated-IR boundary is a **named V4 acceptance assertion** (the last-valid-render + Doc 05 §1.5.9 banner, proven not assumed). |
| K-4 | VSIX size / marketplace publishing-signing | **MEDIUM (logistical)** | v1.3 builds an *installable* VSIX (`vsce package`), **not** a *published* one; publishing/signing is an owner credential/product decision (Doc 27 §9 risk-4 — escalate, do not grind, the Resend/DNS-escalation pattern). |
| K-5 | `vscode-languageclient` default-capability fragility (a refactor strips `general.positionEncodings` → silent UTF-16 regression) | **MEDIUM** | R-4 verified the server-side negotiation is exact at `55ecc50`. **V1 acceptance (c) is the CI guard** — a stripped capability fails the Electron test, not a user. |
| K-6 | Doc-22-vs-shipped config drift (most §8 keys are server no-ops) | **MEDIUM (fully enumerated)** | **R-5..R-8 re-verified the four-key reality at `55ecc50`.** Mitigated by Doc 27 §2.5 "contribute-the-spec / wire-only-the-real / route-the-rest / never-silent-no-op"; the per-key §10 verdict table the V-waves consume is affirmed accurate. |
| **K-7 (Doc-28-new)** | **Doc 27 framing-vs-shipped staleness** (R-14: the obsolete "two converters" prose; R-15: the post-Doc-27 allow/deny oracle change; ±citation drift) | **MEDIUM (handled by §1)** | The §1 reconciliation ledger is the mitigation. R-15 is folded into the V1 gate as a MANDATORY fixture constraint. A wave reading Doc 27 §2.3/§6.3/§10 reads it **through §1**, never the stale prose. |
| **K-8 (Doc-28-new)** | **§11.49 CST-coupling refactor regressing the load-bearing `fsm-analyzer` mid-epic** | **MEDIUM (sequenced out)** | §4: run it as **v1.3-W0 before V1** (the L0-before-LSP precedent), DRIFT-2-grade leave-and-explain discipline, non-behavioural pre/post-identity acceptance, 0 new deps. Never parallel with feature waves; V1's oracle computed against post-W0 analyzer. |

### 6.2 What "v1.3.0 ready" means (the tag gate)

v1.3.0 is tag-ready when **all** hold (the v1.0/v1.1/v1.2 gate pattern,
`GATE_VERIFICATION_v1_2.md` discipline, applied to the extension):

1. **W0 (§11.49) closed** — the CST-coupling debt paid (or, if any residual
   is left-and-explained per the DRIFT-2 anti-pattern, that residual is
   recorded as the new accepted-tracked-debt with reason); pre/post-identity
   proven (full corpus + 35 LSP client tests + examples 5/5 + conformance
   26/26 byte-unchanged). Recorded as the **closed §11.49 item** in
   `GATE_VERIFICATION_v1_3.md`.
2. **V1–V5 shipped, each with its real Extension-Host behavioural-acceptance
   gate green** (§3 — never symbol-presence; the §5.4 analogue enforced).
   Phase-boundary audits after V1 and V4 returned PROCEED.
3. **V6 host-only shipped**; the **5-platform tail explicitly carried as
   accepted-tracked-debt blocked-on-G9** if G9 has not run (the v1.2 §5
   posture — documented, not silently dropped). An installable host-platform
   `.vsix` is produced; publishing/signing explicitly OUT (K-4).
4. **The §11.30/§11.22 release sequence followed exactly:** author + commit
   `GATE_VERIFICATION_v1_3.md` → run the **cold-from-source** quad **at that
   commit** (the Rust quad — the extension adds no Rust surface, so the
   cold-cargo gate is unchanged in shape; the **JS lane's `npm ci` +
   Extension-Host suite is run green at that same commit** and its transcript
   recorded in the annotated tag message alongside the cargo transcript) →
   `git tag -a` **that same commit** + the `checkpoint/<date>` anchor
   (quad-commit ≡ tag-commit, zero off-by-one). The cold quad must respect
   the §2 disk envelope — the JS caches out-of-tree so the cold-cargo
   ~3 G-free trough is not worsened by `node_modules`.
5. **0 open P0**; an independent pre-tag audit (the v1.2 four-lens pattern)
   returns TAG-CLEAR; the SCA `cargo audit` job reports 0 vulns (the
   `GATE_VERIFICATION_v1_2.md` §"Pre-tag reliability checklist" carried
   forward — and re-checked because V1+ adds an `npm` dependency tree: an
   **`npm audit`** of `editors/vscode/` at 0 high/critical is added to the
   pre-tag reliability checklist, the SCA discipline extended to the new
   Node surface).
6. **The post-tag owner action recorded** (push to exercise the never-run
   G9 matrix — now also covering the new JS CI lane; the v1.2 §5 pattern,
   with a v1.3.1 patch lane kept ready).

CHANGELOG `[1.3.0]` records: the extension scope, the §11.49 closure, the
host-only-binary + G9-blocked multi-platform tail as *Known limitations*,
and the publishing-OUT decision.

---

*End of FSM-PLAN-VSCE-V13 v1.0.0*
