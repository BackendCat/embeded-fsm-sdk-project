# FSM Studio — VS Code Extension Architecture

**Document ID:** FSM-ARCH-VSCE
**Version:** 1.0.0
**Status:** Living Document — v1.3 epic architecture. Authoritative for the
VS Code extension package shape and its wave plan. Refines (does not
contradict) Doc 22 (the `package.json` manifest spec) and Doc 05 §1.4/§1.5
(the editor-feature UX); where Doc 22's manifest sketch and the **shipped
`crates/fsm-lsp` reality** disagree on what a contribution can wire to,
**the shipped LSP wins** and the divergence is recorded in §10. The
canonical command-prefix / setting-key / language-ID authority remains Doc
22 per Doc 00 §B-04 / §I-27 / §I-28.
**Depends on:** FSM-SPEC-VSCE (22), FSM-SPEC-TM (21), FSM-SPEC-UI (05),
FSM-ARCH-LSP (26), FSM-SPEC-LSP (14), FSM-SPEC-CLI (18), FSM-SPEC-DEC (00
§B-04/§I-27/§I-28/§11.32–§11.38), FSM-PROC-SUBAGENT.

This is the engineering architecture for the v1.3 VS Code extension. It is
the output of the **extraction-first-for-epics** discipline
(FSM-PROC-SUBAGENT; the same discipline that produced Doc 26 *before* the
LSP waves and caught the Doc-20 §4.5/L787 false-incremental-parse drift):
a multi-wave epic gets a deep-extraction architecture pass scoped against
the **current code** — here the *shipped, feature-complete* `fsm-lsp`
(`crates/fsm-lsp`, Doc 00 §11.32–§11.38) — not a verbal sketch, before any
implementer wave. Every reuse/capability claim cites a concrete
type/function/file verified at HEAD `4f7e9ea` (`fsm-lsp` L1–L7 all
shipped). Doc 22 is **intent-shorthand, not ground truth**: §10 verifies
each of its load-bearing claims against the real server and affirms the
accurate ones, flags the imprecise/aspirational ones precisely.

---

# 1. Why the VS Code extension is the right v1.3 (re-scoping)

The v1.2 LSP epic is feature-complete (`fsm-lang-server`: a full tower-lsp
server — `initialize`/sync/debounce + `publishDiagnostics`,
`documentSymbol`, `foldingRange`, `hover`, `definition`, `completion`,
`references`, `prepareRename`/`rename`, `semanticTokens` full+range,
`codeAction`, `inlayHint`; 785 tests; `#![forbid(unsafe_code)]` at both
crate roots — Doc 00 §11.32–§11.38). The LSP is **inert without a client**:
Doc 26 §1 already states the VS Code extension (Doc 22) is its natural,
highest-fan-out consumer, and the ROADMAP §96(b)/§104–§105 confirms the
extension is the deliverable the LSP-first sequencing was meant to unlock.
v1.3 = the VS Code extension is the TL re-scoping decision (user-endorsed);
this doc scopes its implementer waves against the shipped server's REAL
capability/config surface, exactly as Doc 26 scoped the LSP waves against
the analyzer.

The previous ROADMAP §132 label ("v1.3 — Simulation & verification", the
simulator WebSocket Doc 13) is superseded by this re-scoping; the
simulator-driven diagram-overlay UX (Doc 05 §1.5.8) and Doc 13 protocol
become a **later** epic (§9 scope-out). The orchestrator folds the ROADMAP
v1.3 pointer + the Doc-00 §11.39 row in a separate reconciliation pass —
this read-only wave touches **only this new doc**.

---

# 2. Extension shape

A **new top-level workspace area** `editors/vscode/` (Doc 22 §12 assumes
this path; Doc 21 §3 puts the grammar at
`editors/vscode/syntaxes/fsm-lang.tmLanguage.json`). **Verified: there is
no `editors/` directory at HEAD** (`ls` top-level = `crates/ docs/
examples/ schema/ tests/`) — the entire TS/Node surface is greenfield.
This is the dominant non-LSP risk (§7): a Node/TS toolchain entering a
pure-Rust Cargo workspace.

## 2.1 Language / runtime

- **TypeScript + Node**, compiled to JS, packaged as a `.vsix`. `engines.vscode
  ^1.85.0` (Doc 22 §1) — that baseline ships LSP 3.17 and
  `vscode-languageclient` ≥ 8, which is the version that supports the
  `general.positionEncodings` client capability the shipped server
  negotiates on (`server.rs:216-225`; Doc 00 §11.32(1)). This is a hard
  floor, not a preference: an older client never offers `utf-8`, so the
  server falls back to UTF-16 (still correct, but the UTF-8 fast-path that
  *deletes the transcoding defect class* — Doc 26 §4.1 — is lost).
- The extension is a **thin LSP client + a few editor-only surfaces** (§5):
  the heavy lifting (parse/analyze/all language intelligence) is the
  shipped server's, consumed for free via `vscode-languageclient`. The
  extension must NOT re-implement any analysis (the same Doc 26 §3
  one-pipeline invariant, now at the client boundary: the editor squiggle
  is the server's `fsm check`, never a parallel JS re-analysis).

## 2.2 Locating / launching / shipping `fsm-lang-server`

The shipped binary is **`fsm-lang-server`**, **stdio transport ONLY**.
Verified `crates/fsm-lsp/src/main.rs`: no args → `fsm_lsp::run_stdio()`
(`lib.rs:165-169`: `LspService::new(Backend::new)` over `tokio::io::stdin/
stdout`); `--version`/`-V`/`--help`/`-h` supported; **`--port` is
explicitly NOT implemented — it prints `error: --port (TCP transport) is
not implemented in this server build` and exits `2`** (`main.rs:39-48`).

**Decision — stdio `ServerOptions`, no TCP.** The client launches the
binary as a child process with the `vscode-languageclient` stdio transport
(`TransportKind.stdio`). The Doc 22 §13.1 lifecycle table's "Spawn
`fsm-lang-server` with stdio transport" is therefore **accurate and the
only supported path**; any reading of Doc 14 §1 / Doc 22 that implies a
`--port`/TCP option is a v1.3 non-feature (§10 row; §9 scope-out).

Binary resolution order (Doc 22 §12 + §8 `fsmLang.compilerPath`):

1. If `fsmLang.compilerPath` is a non-empty string → use it verbatim
   (Doc 22 §8 `scope: machine-overridable`; Doc 22 §12 "user-specified
   path takes precedence"). **Naming note (§10):** Doc 22 §8 names this
   key `fsmLang.compilerPath` and Doc 00 §I-28 makes Doc 22 authoritative
   over the Doc 03 `fsmLang.lspPath` spelling — the extension contributes
   exactly `fsmLang.compilerPath`. The key points at the **`fsm-lang-server`**
   binary, NOT the `fsm` CLI, despite the word "compiler" (the Doc 22 §12
   prose says "`fsm` binary"; in v1.3 the language client needs the
   *server* binary — see §6/§10 for why the diagram path *also* needs the
   `fsm` CLI, a genuine two-binary need Doc 22 §12 under-describes).
2. Else the **bundled** binary for the host triple, per the Doc 22 §12
   `${platform}-${arch}` scheme
   (`bin/{linux,darwin,win32}-{x64,arm64}/fsm-lang-server[.exe]`).
3. Else show the Doc 22 §12 error notification verbatim and stay inert
   (no silent fallback — the project's cardinal-sin bar at the client
   boundary).

Crash recovery: the Doc 22 §13.2 exponential-backoff `errorHandler`
(3 restarts, 3 s base, 3× multiplier, 60 s success-reset) is implemented
in the client `LanguageClientOptions.errorHandler` exactly as Doc 22 §13.2
specifies. The shipped server's `shutdown` handler exists
(`server.rs:375`) and `tower-lsp` owns the `exit` half, so the Doc 22
§13.1 "send `shutdown`; wait 2 s; send `exit`; kill" deactivation sequence
maps onto `vscode-languageclient`'s `LanguageClient.stop()` (it performs
exactly that handshake) — the extension does not hand-roll it.

## 2.3 `positionEncoding` wiring (the load-bearing correctness seam)

The shipped server advertises `position_encoding` from a negotiation:
UTF-8 **iff** the client lists it in `general.positionEncodings`, else
UTF-16 (`server.rs:216-239`; Doc 00 §11.32(1)). `vscode-languageclient`
≥ 8 sends `general.positionEncodings` automatically when the extension
does **not** suppress it; the extension MUST therefore NOT override
`initializationOptions`/`clientOptions` in a way that strips that
capability — the entire Doc 26 §4.1 / risk-1 transcoding-defect deletion
rides on the server seeing `utf-8` in the offer. This is a one-line "do
not break the default" client constraint, but it is load-bearing and is a
named V1 acceptance assertion (§8).

## 2.4 Activation events

Doc 22 §2 verbatim: `onLanguage:fsm-lang` + `workspaceContains:**/*.fsm`.
Accurate as written and cheap; no change. The extension MUST NOT add `*`
(activate-on-startup) — Doc 22 §2's explicit "MUST NOT activate without a
`.fsm` file" is honoured.

## 2.5 `contributes` surface — and how config maps to the REAL server

`contributes.languages` / `contributes.grammars` / `contributes.snippets`
are taken **verbatim from Doc 21 §1 + Doc 22 §3** (language id `fsm-lang`,
`scopeName source.fsm`, `.fsm`, the grammar + `language-configuration.json`
+ snippets paths). These are accurate and require no derivation.

`contributes.configuration` (Doc 22 §8) is where intent-shorthand and
shipped reality **diverge sharply** — the most important §10 finding.
**Verified `crates/fsm-lsp/src/config.rs`: the server reads EXACTLY FOUR
`fsmLang.*` keys**, all inlay-hint toggles
(`InlayHintConfig::from_settings`, both the nested-VS-Code and
flat-dotted shapes, defensive defaults — Doc 00 §11.38(3)):

| Doc 22 §8 key | Server actually reads it? | Evidence |
|---|---|---|
| `fsmLang.enableInlayHints` | **YES** (`InlayHintConfig.enabled`) | `config.rs:33,99` |
| `fsmLang.inlayHints.showTransitionPriorities` | **YES** | `config.rs:36,100` |
| `fsmLang.inlayHints.showStateTypes` | **YES** (default *false*) | `config.rs:40,104` |
| `fsmLang.inlayHints.showTimerDurations` | **YES** | `config.rs:43,108` |
| `fsmLang.compilerPath` | NO — *client*-side only (binary resolution §2.2) | not in `config.rs`; `server.rs:228-235` reads only inlay opts |
| `fsmLang.debounceMs` | **NO** — debounce is a hard-coded `const DEBOUNCE = Duration::from_millis(200)` | `server.rs:100` |
| `fsmLang.maxProblems` | **NO** — server emits all analyzer diagnostics, no cap | no cap path in `analysis.rs`/`server.rs` |
| `fsmLang.diagnostics.enableStyleWarnings` / `enableHints` / `actionComplexityThreshold` | **NO** — not wired (Doc 26 §7 open-q 8 "stub-accept the rest"; only the four inlay keys gate behaviour) | `config.rs` doc-comment ll.4-11; only inlay keys parsed |
| `fsmLang.codegen.*` / `fsmLang.format.*` | **NO** — codegen/format are CLI concerns, not LSP | not in `config.rs`; no codegen/fmt in `fsm-lsp` |
| `fsmLang.simulator.port` | **NO** — no simulator in v1.3; server has no TCP at all | `main.rs:39-48` (`--port` unimplemented) |

**Decision — contribute the full Doc 22 §8 schema, wire only the real
four, route the rest correctly, never silently no-op a user setting.**
This is the Doc 00 §11.38 *advertise-the-spec / ship-only-the-safe*
discipline applied at the config layer:

- The **four inlay keys** flow to the server via `initializationOptions`
  (read at `initialize`, `server.rs:233-235`) AND via
  `workspace/didChangeConfiguration` (`server.rs:843`+, live override) —
  the standard `vscode-languageclient` `synchronize.configurationSection:
  "fsmLang"` wiring; the server already round-trips both channels
  (Doc 00 §11.38(3); the L7 acceptance proves the `didChangeConfiguration`
  toggle round-trip).
- `fsmLang.compilerPath` is consumed **client-side** for binary resolution
  (§2.2) — it never goes to the server.
- `fsmLang.codegen.*` (`defaultTarget`, `outputDir`, `strategy`) and
  `fsmLang.debounceMs` etc. that the *server* ignores are consumed by the
  **extension's own commands** where they are actually meaningful: the
  `fsm.generateC99/Cpp17` commands (§5) shell the `fsm` CLI and pass
  `--target`/`--out`/strategy from these keys. A key the server ignores
  AND no command uses (`maxProblems`, `diagnostics.*`,
  `format.*` when no formatter command, `simulator.port`) is **either
  not contributed in v1.3, or contributed with a settings-description
  caveat that it is reserved for a later epic** — the V… wave that owns
  each command makes the per-key cut and records it (Doc 00 §11 table),
  exactly as Doc 26 §7 open-q 8 left the analogous LSP cut to an L-wave.
  The forbidden outcome is contributing a setting that *looks* live but is
  a silent no-op (the cardinal "looks-like-it-works" sin).

`contributes.commands` / `keybindings` / `menus` / `viewsContainers` /
`views` / `statusBarItems` / `contextKeys` are taken from Doc 22 §4–§7 /
§10–§11. **Command/category-prefix internal inconsistency flagged (§10):**
Doc 22 §4 sets every command `"title": "FSM: …"` but `"category": "FSM
Studio"`, while Doc 22 §0's note + Doc 00 §B-04/§I-27 make `FSM Studio:`
the canonical prefix. Resolution per the authority chain (Doc 22 is
authoritative for command IDs/prefix per Doc 00 §I-27): command **IDs**
stay `fsm.*` (Doc 22 §4 verbatim — IDs are API), the user-facing **title
prefix is normalized to `FSM Studio:`** to match the canonical
convention + the `category`. This is a Doc-22-internal imprecision
resolved by its own §0 note, not a new decision.

---

# 3. Capability-reuse map — free-via-the-LSP vs ext-must-build

The governing principle (the client-boundary analogue of Doc 26 §3):
**everything the shipped server already does is free to the extension via
`vscode-languageclient`** — the client library translates VS Code's
provider requests into LSP JSON-RPC and renders the responses with **zero
extension code** beyond registering the client. The extension only writes
code for surfaces the server does *not* provide.

| Ext feature (Doc 22 / Doc 05) | Backed by SHIPPED LSP capability (cite) | Free via client, or ext must build |
|---|---|---|
| Diagnostics squiggles + Problems panel (Doc 05 §1.4.3) | `publishDiagnostics` — `server.rs:199` via `analysis.rs::analyze` = exact `fsm check` pipeline (Doc 00 §11.32(3)) | **FREE** (client renders `publishDiagnostics`) |
| Outline / breadcrumbs (Doc 05 §1.1) | `documentSymbol` — `server.rs:251` `document_symbol_provider:true`, Doc 00 §11.33 | **FREE** |
| Folding (Doc 22 §3 region markers + structural) | `foldingRange` — `server.rs:252`, pure CST walk, Doc 00 §11.33(2) | **FREE** |
| Hover cards (Doc 05 §1.4.4) | `hover` — `server.rs:257`, Markdown from IR, Doc 00 §11.34 | **FREE** |
| Go-to-definition (Doc 05 §1.4.6) | `definition` — `server.rs:258`, single-file, Doc 00 §11.34(3) | **FREE** (single-file; cross-file is a server v1.3+ gap, §9) |
| Completion (Doc 05 §1.4.5) | `completion` — `server.rs:270-280`, triggers `[". " ": " "@" "[" " "]`, Doc 00 §11.35 | **FREE** |
| Find-all-references (Doc 05 §1.4.7) | `references` — `server.rs:286`, semantic `ReferenceIndex`, Doc 00 §11.36 | **FREE** |
| Rename + prepare (Doc 05 §1.4.8) | `rename`+`prepareRename` — `server.rs:294-297` `prepare_provider:true`, Doc 00 §11.36(2) | **FREE** (the conservative rename-safety lives server-side) |
| Semantic colouring (Doc 05 §1.4.2 refinement) | `semanticTokens` full+range — `server.rs:310-319`, Doc 00 §11.37 | **FREE** (client requests + merges over TextMate) |
| Quick-fixes / lightbulb (Doc 05 §1.4.3) | `codeAction` — `server.rs:336-342`, kinds `["quickfix","refactor"]`, only E0107 + guarded-E0022 produced (Doc 00 §11.38(1)) | **FREE** (only 2 fixes exist; that is the server's bias-to-safety, not an ext gap to fill) |
| Inlay hints (Doc 22 §8 toggles) | `inlayHint` — `server.rs:352-358`, IR-sourced trio, Doc 00 §11.38(2) | **FREE** + ext contributes the 4 toggle settings (§2.5) |
| TextMate syntax colouring (offline/startup) | — *(not an LSP capability)* | **EXT BUILDS** — package Doc 21's grammar (§4) |
| `language-configuration.json` (brackets/comments/indent) | — | **EXT BUILDS** — verbatim Doc 21 §4 |
| Snippets (Doc 22 §9) | — *(server `completion` emits the Doc 14 §4 keyword snippets, but Doc 22 §9's `machine/state/composite/parallel/on/after/every/choice/extern` body snippets are a static contribution)* | **EXT BUILDS** — `snippets/fsm-lang.json` verbatim Doc 22 §9 |
| Diagram WebviewPanel (Doc 22 §4 `fsm.openDiagram`, Doc 05 §1.5) | — *(NO LSP method emits a diagram model — see §6)* | **EXT BUILDS** — the single largest new surface (§6) |
| `fsm.generateC99/Cpp17` commands (Doc 22 §4) | — *(codegen is the `fsm` CLI, not the LSP)* | **EXT BUILDS** — shell the `fsm` CLI; consume `fsmLang.codegen.*` |
| `fsm.copyIR` command (Doc 22 §4) | — *(no LSP IR method; `fsm generate --emit-ir` only — §6/§10)* | **EXT BUILDS** — shell `fsm generate --emit-ir`, read the file |
| `fsm.checkFile` / `fsm.formatDocument` (Doc 22 §4) | check = server diagnostics already live; explicit command = `fsm check`/`fsm fmt` CLI | **EXT BUILDS** thin CLI wrappers (the live squiggle already covers check) |
| `fsm.restartLanguageServer` / `fsm.showOutputChannel` (Doc 22 §4) | client API (`LanguageClient.restart()` / output channel) | **EXT BUILDS** (trivial client calls) |
| Status bar server state (Doc 22 §10) | client lifecycle events (`onDidChangeState`) | **EXT BUILDS** (trivial; maps client state → Doc 22 §10 icons) |
| Activity-bar Machines/Events tree views (Doc 22 §7) | `documentSymbol` data can feed a `TreeDataProvider` | **EXT BUILDS** the `TreeDataProvider` *(reuses the free `documentSymbol` response as its data — no new analysis)* |
| Simulator panel / overlay (Doc 22 §4 `fsm.openSimulator`, Doc 05 §1.5.8) | — *(Doc 13 simulator WebSocket is a later epic, NOT shipped)* | **OUT of v1.3** (§9) |

**The single load-bearing conclusion:** every *language-intelligence*
feature is free; the extension's real engineering is (a) packaging Doc 21
(grammar/lang-config) + Doc 22 §9 (snippets), (b) the diagram Webview
(§6), (c) thin CLI-wrapper commands, (d) status-bar/tree chrome. Items
(a),(c),(d) are low-risk static/glue; (b) is the only substantive new
subsystem and the only place a DRIFT-class assumption can bite (§6).

---

# 4. TextMate grammar packaging (Doc 21) + the semantic-tokens coexistence

Doc 21 is a **DRAFT** (`Status: Deferred to v1.1`); v1.3 *consumes* it,
does not redesign it (the same stance Doc 26 §5 took: "Doc 21 is already
drafted; the L-plan consumes it"). The extension packages, verbatim:
`syntaxes/fsm-lang.tmLanguage.json` (Doc 21 §3) and
`language-configuration.json` (Doc 21 §4).

**Coexistence is already settled by code, not by this doc.** The shipped
server implements `semanticTokens` full+range (L6, Doc 00 §11.37) which
**supersedes TextMate for *semantic* colouring**. Doc 21 §6 itself states
the model and **defers the legend to FSM-SPEC-LSP §10** — and Doc 00
§11.37(4) verified there is therefore **no Doc-21-vs-Doc-14 legend
conflict**, only the expected scope-superset → 11-type-legend-subset
mapping (semantic tokens are coarser than Doc 21's rich scopes by LSP
design; they *refine*, they are not 1:1). The extension's job is purely:
ship the grammar so colouring works **offline / pre-server-ready / in the
un-analyzed tail of a large file**, and let VS Code's built-in TextMate ⊕
semantic-tokens merge do the rest (semantic tokens win where the server
has analyzed; TextMate covers the rest — Doc 21 §6, the VS Code default
merge, **no extension code**). The L6 wave already proved the semantic
side end-to-end (Doc 00 §11.37 §5.4 tests decode the delta stream); v1.3
adds only the offline TextMate layer beneath it.

Net for the extension: TextMate grammar = a packaged static asset, **zero
logic**. Its only acceptance is "the contributed grammar parses and the
bundled `fsm-lang` scope resolves" (a `vscode-tmgrammar-test` snapshot,
§8 V2) — it does not need behavioural parity with the analyzer because
the analyzer-precise layer is the *server's* semantic tokens, already
shipped and tested.

---

# 5. Editor-only surfaces the extension must build (non-diagram)

These are the non-language-intelligence surfaces — all thin glue over the
shipped server's client API or the `fsm` CLI; none re-implements analysis.

- **`fsm.checkFile`** — the live `publishDiagnostics` already covers
  "check" continuously; the explicit command is a convenience that runs
  `fsm check <file>` (the CLI) and surfaces its exit/summary. It MUST NOT
  spawn a second analyzer in JS — the squiggle is already the server's
  `fsm check` (Doc 00 §11.32(3)); the command is a user-invoked echo of
  the same pipeline, not a parallel one.
- **`fsm.generateC99` / `fsm.generateCpp17`** — shell the `fsm` CLI
  `generate --target {c99|cpp17} --out <fsmLang.codegen.outputDir>` (the
  CLI is the only codegen path; the LSP has none). `fsmLang.codegen.*`
  (Doc 22 §8) feed these (the keys the *server* ignores but are
  meaningful here — §2.5). C++17 codegen is itself a v1.2 deliverable
  (ROADMAP §96(b)); the command degrades gracefully (clear error) if the
  bundled `fsm` lacks the target.
- **`fsm.copyIR`** — see §6/§10: the only IR-JSON path is `fsm generate
  --emit-ir` writing `<machine>.ir.json`; the command runs it to a temp
  dir and copies the file's content. **There is no `fsm ir` subcommand
  and no LSP IR method** (verified §6) — Doc 22 §4's `fsm.copyIR` is
  satisfiable only this way in v1.3, and the V-wave records the
  judgment.
- **`fsm.restartLanguageServer`** → `LanguageClient.restart()` + reset the
  Doc 22 §13.2 crash counter (Doc 22 §13.2's explicit reset-on-manual
  contract).
- **`fsm.showOutputChannel`** → reveal the client's output channel
  (`FSM Language Server`).
- **Status bar** (Doc 22 §10) — subscribe to the client's
  `onDidChangeState` and map running/warn/error/stopped to the Doc 22 §10
  icon table. Warn/error states derive from whether the latest
  `publishDiagnostics` carried any Warning/Error (a passive read of the
  diagnostics the client already received — no new analysis).
- **Activity-bar tree views** `fsm.machineExplorer` / `fsm.eventExplorer`
  (Doc 22 §7) — a `TreeDataProvider` whose data is the **free
  `documentSymbol` response** for the active file (re-projected, not
  re-analyzed). Context keys `fsm.hasOpenFsmFile` / `fsm.serverRunning`
  (Doc 22 §11) gate visibility.

---

# 6. The diagram Webview (Doc 22 §4 `fsm.openDiagram` / Doc 05 §1.5) — the DRIFT-class risk

This is the single substantive new subsystem and the place an
unverified data-source assumption is exactly the Doc-26-§4.5 prose-vs-code
trap. The verification was done against code, not prose.

## 6.1 Data source — what actually exists (verified)

Doc 05 §1.5/§2.6 describes a live `@elklayout`/ELK-Layered statechart
view; Doc 05 line 577 lists a `FSM Studio: Compile to IR` command and
Doc 18 §`fsm ir` (line 455-473) specs a standalone `fsm ir` IR-JSON dump.
**The DRIFT finding:**

1. **There is NO LSP method that returns a diagram/IR model.** The
   shipped `ServerCapabilities` (`server.rs:238-360`) is exactly the Doc
   14 §2 set + `positionEncoding`; there is **no custom
   `fsm/diagram`/`fsm/ir` request, no `experimental` capability**. The
   server's `Ir` is computed internally for hover/inlay but is **never
   exposed over the wire** (Doc 00 §11.34(4): IR is read in-process for
   hover detail only). Assuming an LSP IR method would be the precise
   Doc-26-§4.5 `parse_incremental`-class fabrication — it does not exist.
2. **`fsm ir` (Doc 18) is specced but NOT implemented.** Verified
   `crates/fsm-cli/src/cli.rs:8-11`: "*the … (`ir`, `completions`) … are
   intentionally NOT exposed here — Doc 20 §8 and Doc 00 §7.2 defer those
   to post-v1.0*". The `Command` enum has `Parse/Check/Generate/Fmt/Test/
   Doc/Decompile/Init` — **no `Ir`**. Doc 18's `fsm ir` is aspirational
   spec prose; relying on it is the same drift class.
3. **The ONE working IR-JSON path: `fsm generate --emit-ir`.** Verified
   `crates/fsm-cli/src/cmd/generate.rs:189-209`: with `--emit-ir` the CLI
   writes `<machine>.ir.json` via `fsm_ir::to_json` (a real public fn,
   `crates/fsm-ir/src/json.rs:43`; `fsm-ir` is "*pure data types + serde
   JSON serialization*", `Cargo.toml:9`, full serde). **Caveat (recorded,
   not papered):** `--emit-ir` is gated *behind a successful codegen
   pass* — the IR is written only after `fs::write` of the generated C
   succeeds (`generate.rs:175-209`); a file that *analyzes* clean but a
   codegen edge rejects would yield no IR. For the diagram this is
   acceptable in v1.3 (a machine that won't codegen is degenerate) but is
   a precise boundary the diagram wave must handle (fall back to the
   "last valid" render + Doc 05 §1.5.9 banner), not assume away.

## 6.2 Decision — v1.3 diagram = read-only, CLI-IR-sourced, ELK-laid-out in the Webview

- **Data source:** the extension runs `fsm generate --emit-ir` (the only
  real path; §6.1.3) for the active file to a temp dir, reads
  `<machine>.ir.json` (`fsm_ir::to_json` schema — `crates/fsm-ir`), and
  feeds that canonical IR to the Webview. **No new server method is
  introduced in v1.3** (introducing an `fsm/diagram` LSP method or an
  `fsm ir` CLI subcommand is a *separate, self-contained* epic with its
  own behavioural acceptance — the exact Doc 26 §4.5/§9 "do not smuggle a
  new subsystem into this epic" stance). Re-render trigger = on save +
  on the live `publishDiagnostics` going clean (debounced, Doc 05 §1.5.9
  "within 500ms"); on a parse/codegen failure the Webview keeps the last
  valid render + the Doc 05 §1.5.9 banner (the IR simply isn't refreshed).
- **Layout:** ELK Layered runs **inside the Webview** (`elkjs`, the
  JS/WASM ELK the Doc 05 §1.5.9 "ELK Layered" + ROADMAP §105
  "@elklayout/core" intend) over the IR's state/region/transition graph.
  Layout is a pure client-side concern; the server/CLI is not involved.
- **Scope = read-only view, NOT an editor.** Doc 05 §1.5.4 (click → editor
  line via go-to-definition; double-click → declaration), §1.5.7 (context
  menu: copy name/stable-id, go-to-source), pan/zoom, SVG/PNG export,
  legend, live re-render. Interactive *graphical editing* (drag a state
  to rewrite the `.fsm`) is **explicitly OUT of v1.3** (§9) — it needs a
  diagram→source transform that has no substrate in-tree (the `decompile`
  command is a "v1.0 stub", `cli.rs:49`). The simulator-overlay parts of
  Doc 05 §1.5.8 are OUT (no Doc 13 simulator in v1.3, §9).
- **Webview ↔ extension protocol:** `postMessage` JSON only (the standard
  VS Code Webview boundary), CSP-locked, no remote resources — the IR
  model in, click/navigate events out (mapped back to
  `vscode.window.activeTextEditor` reveals using the IR's
  `SourceLocation`). The Webview must not preclude the future Web IDE
  (Doc 05 §2.6 "identical feature set … with adaptations"): keeping the
  diagram a pure `IR-JSON → ELK → SVG` function with a `postMessage`
  boundary means the same Webview bundle is reusable as the Web IDE's
  diagram tab (Doc 05 §2.5/§2.6) when that epic lands — the architecture
  stays Web-IDE-compatible without building the Web IDE (the Doc 26 §9
  "stays WASM-compatible, compiling it is later" discipline, mirrored).

## 6.3 Why this is the conservative call

A diagram that consumes the **canonical `fsm-ir` JSON** (the same IR the
whole toolchain — codegen, simulator, `--emit-ir` — is built on) can never
drift from the language semantics, exactly as the LSP's reuse of the `fsm
check` pipeline (Doc 26 §3) made the squiggle un-drift-able. Inventing an
LSP `fsm/diagram` method or depending on the unimplemented `fsm ir`
command would be the Doc-26-§4.5 aspirational-prose pattern the project
has been bitten by; both are explicitly deferred (§9), not half-built.

---

# 7. Risks & open questions

1. **Node/TS toolchain in a Rust Cargo workspace (HIGH — structural).**
   There is no `editors/` dir (§2); v1.3 introduces npm/`tsc`/`esbuild`/
   `@vscode/vsce` into a workspace whose every existing CI gate is the
   `cargo` quad (FSM-PROC-SUBAGENT §4). Open question the orchestrator
   owns: **does the ext build/test live in the same CI as cargo, or a
   separate JS job?** Recommendation: a *separate, additive* CI lane for
   `editors/vscode/` (npm ci → tsc → eslint → `@vscode/test-electron`),
   gated independently of the Rust quad; the Rust workspace stays
   untouched (no `editors/` member in `Cargo.toml` — it is not a crate).
   Mitigation: V1's brief pins the exact toolchain + lockfile and treats
   the JS CI lane as a first-class deliverable, not an afterthought.
2. **Cross-platform binary bundling / VSIX (HIGH — ties to never-run G9
   CI matrix + SEC-P0-1).** Doc 22 §12 bundles 5 platform binaries
   (`linux/darwin/win32 × x64/arm64`). The project's G9 cross-OS CI
   matrix has **never run** and SEC-P0-1 (import path-canonicalisation)
   is the most platform-divergent code area — exactly the seam that
   differs on win32 (`\` vs `/`, drive letters) and is exercised by the
   server's import-security pass the extension launches. Bundling a
   `fsm-lang-server` whose path-canon behaves differently per-OS is the
   sharpest risk. Mitigation: v1.3 ships **the host platform's binary
   only as the MVP** and treats the full 5-platform matrix as its own
   gated sub-wave that *requires* the G9 matrix to exist first; the
   `fsmLang.compilerPath` escape hatch (§2.2) + the Doc 22 §12 "no
   bundled binary for {platform}" error means an un-bundled platform
   degrades honestly, not silently. Recommend the orchestrator surface
   "full multi-platform bundling depends on G9" as an explicit dependency
   (per the project's infra-constraint-escalation discipline).
3. **The diagram data-source DRIFT (MEDIUM — handled, flagged here so no
   wave assumes a freebie).** §6: no LSP IR method, `fsm ir` unimplemented;
   only `fsm generate --emit-ir` (codegen-gated). The architecture is
   pinned to the one real path; the V-diagram wave must NOT introduce a
   server method (separate epic). Residual: the codegen-gated-IR boundary
   (§6.1.3) — handled by the last-valid-render fallback, asserted in §8.
4. **VSIX packaging + bundled-binary size / signing (MEDIUM —
   logistical).** 5 native binaries inflate the `.vsix`; marketplace
   publishing/signing is out of autonomous scope (a product/credential
   decision — escalate, do not grind). v1.3 builds an *installable* VSIX
   (`vsce package`), not a *published* one; publishing is a later owner
   decision (parallels the Resend/DNS escalation pattern).
5. **`vscode-languageclient` default-capability fragility (MEDIUM).**
   §2.3: the UTF-8 fast-path requires the client to keep sending
   `general.positionEncodings`. A future client-options refactor that
   strips it silently regresses to UTF-16 (still correct, but loses the
   Doc 26 §4.1 defect-class deletion). Mitigation: a V1 acceptance
   assertion that the negotiated encoding is UTF-8 under a ≥8 client
   (§8) — a regression fails CI, not a user.
6. **Doc 22-vs-shipped-LSP config drift (MEDIUM — fully enumerated §2.5 /
   §10).** Most Doc 22 §8 keys are server-no-ops; the risk is shipping a
   settings UI that lies. Mitigated by the §2.5 "contribute-the-spec /
   wire-only-the-real / route-the-rest / never-silent-no-op" rule and the
   per-key §10 verdict table the V-waves consume.

---

# 8. Scoped implementer-wave breakdown (V1…V6)

Sequenced so **V1 proves the architecture end-to-end** (extension
scaffold + language client to the *shipped* `fsm-lang-server` + real
diagnostics visible in the editor — the highest-fan-out, highest-risk
"does the client wiring + the position-encoding seam actually work"
bet) before any breadth (grammar, snippets, commands, diagram). Each
wave: deliverable, the reuse seams it touches, and a **§5.4-equivalent
behavioural-acceptance** definition.

**The §5.4 analogue for a VS Code extension.** FSM-PROC-SUBAGENT §5.4
mandates "compile-and-RUN, assert observable behaviour, never
symbol-presence". For the LSP that became the in-process `tower-lsp`
client (Doc 26 §5.4-LSP; the shipped `crates/fsm-lsp/tests/
lsp_client_acceptance.rs` harness — `LspService::new` + `tower::Service::
call` + `ClientSocket` drain). **For the extension the mandated analogue
is `@vscode/test-electron`: launch a real VS Code Extension Host with the
extension + a real (or stubbed-transport) language client, drive editor
actions via the `vscode` API, and assert observable editor state** —
e.g. `vscode.languages.getDiagnostics(uri)` returns the exact
`FSM-Exxxx` + Range, `executeDocumentSymbolProvider` returns the tree,
the Webview posts the IR-derived model. **Symbol/manifest-presence
(`package.json` has the command, the activate fn is exported) is NOT
acceptance** — that is precisely the P0-1 symbol-presence trap at the
extension layer. A wave that only unit-tests JS internals or asserts
manifest shape is **incomplete** and is rejected back for an
extension-host behavioural test (the orchestrator enforces this exactly
as it does for the LSP waves).

### V1 — MVP: extension scaffold + language client + diagnostics (proves the spine)
- **Deliverable:** `editors/vscode/` greenfield (TS, `package.json`
  identity Doc 22 §1, activation Doc 22 §2, build = esbuild bundle);
  `vscode-languageclient` ≥ 8 wired to `fsm-lang-server` over **stdio**
  with the Doc 22 §2.2 binary-resolution order (`fsmLang.compilerPath` →
  bundled host-triple → Doc 22 §12 error); Doc 22 §13.2 crash-recovery
  `errorHandler`; the **four real** `fsmLang.*` inlay keys wired via
  `initializationOptions` + `didChangeConfiguration` (§2.5); status bar
  (Doc 22 §10) from client state; the **host-platform binary only** (risk
  2). Grammar/snippets/diagram/commands explicitly OUT of V1.
- **Reuse seams:** the entire shipped server — `publishDiagnostics`
  (`server.rs:199`), `position` negotiation (`server.rs:216-239`), the
  four inlay config keys (`config.rs`). New-in-ext: the client wiring,
  binary resolver, status bar.
- **§5.4-analogue acceptance (`@vscode/test-electron`):** (a) open a
  **known-broken** `.fsm` in a real Extension Host → assert
  `vscode.languages.getDiagnostics(uri)` contains the **exact**
  `FSM-Exxxx` code(s) **and the exact Range** that `fsm check --json`
  reports for the same source (the established CLI oracle — proves the
  client surfaces the *shipped pipeline*, identical to the LSP L1 §5.4
  cross-check); (b) edit to fix → assert diagnostics clear; (c) assert
  the negotiated `positionEncoding` is **UTF-8** under the ≥8 client
  (the §2.3/risk-5 guard — a stripped capability fails this); (d) a
  fixture whose first error is **after a non-ASCII line** (emoji in a
  `///`) → assert the Range is correct (the Doc 26 §4.1 defect guard,
  inherited end-to-end through the client). **Manifest-presence is NOT
  acceptance — the asserted diagnostic Range bytes are.**

### V2 — TextMate grammar + language-configuration + snippets (static assets)
- **Deliverable:** package Doc 21 §3 grammar + Doc 21 §4
  `language-configuration.json` + Doc 22 §9 `snippets/fsm-lang.json`,
  verbatim; the Doc 22 §3 `contributes.{languages,grammars,snippets}`.
- **Reuse seams:** none server-side (offline layer beneath the shipped
  semantic-tokens, §4). New-in-ext: the three packaged assets.
- **§5.4-analogue acceptance:** `vscode-tmgrammar-test` snapshot over a
  representative `.fsm` corpus → assert the **tokenized scope stream**
  matches expected `source.fsm` scopes (a real tokenization assertion,
  not "the file is contributed"); an Extension-Host test that with the
  server **off** the buffer still colours (TextMate-only) and with the
  server **on** semantic tokens refine it (the Doc 21 §6 coexistence,
  observable). Snippet bodies asserted to expand to the Doc 22 §9 text.

### V3 — thin CLI-wrapper + client-control commands
- **Deliverable:** `fsm.checkFile`, `fsm.generateC99`, `fsm.generateCpp17`,
  `fsm.copyIR`, `fsm.formatDocument`, `fsm.restartLanguageServer`,
  `fsm.showOutputChannel`; Doc 22 §4/§5/§6 commands/keybindings/menus;
  `fsmLang.codegen.*` consumed by the generate commands (§2.5/§5);
  command-title prefix normalized to `FSM Studio:` (§2.5/§10).
- **Reuse seams:** `LanguageClient.restart()`/output channel; the `fsm`
  CLI (`generate --target/--out`, `generate --emit-ir`, `check`, `fmt`).
  New-in-ext: command handlers + CLI process glue.
- **§5.4-analogue acceptance:** Extension-Host test invokes
  `fsm.generateC99` on a real fixture → assert the expected C file
  appears in `fsmLang.codegen.outputDir`; `fsm.copyIR` → assert the
  clipboard holds JSON that `fsm_ir::from_json` round-trips (proves it is
  the real `--emit-ir` artifact, not a stub); `fsm.restartLanguageServer`
  → assert the client transitions stopped→running and diagnostics
  re-publish. (Behavioural — the file/clipboard/client-state, not "the
  command is registered".)

### V4 — diagram WebviewPanel: read-only IR-sourced ELK view (the substantive subsystem)
- **Deliverable:** `fsm.openDiagram` opens a `WebviewPanel` (Doc 05
  §1.5.1 split-right, one-per-machine, focus-existing); data = `fsm
  generate --emit-ir` → `<machine>.ir.json` → `postMessage` to the
  Webview; ELK Layered (`elkjs`) layout in-Webview; Doc 05 §1.5.3-§1.5.10
  read-only render (states/composite/regions/pseudo-states/transitions,
  pan/zoom, click→editor line, double-click→definition, context menu,
  SVG/PNG export, legend, live re-render on save/clean + Doc 05 §1.5.9
  last-valid-banner fallback). CSP-locked `postMessage`-only boundary
  (§6.2). Interactive editing + simulator overlay explicitly OUT (§9).
- **Reuse seams:** the `fsm` CLI `generate --emit-ir` (the ONE real IR
  path, §6.1.3); the `fsm-ir` JSON schema; client `definition` for the
  double-click navigation. New-in-ext: the Webview bundle, ELK
  integration, the IR→diagram renderer, the message protocol.
- **§5.4-analogue acceptance:** Extension-Host test opens the diagram for
  a known fixture → drive the Webview (evaluate in the Webview context)
  and assert the rendered model has **the exact state/transition set of
  the fixture's `fsm-ir` JSON** (parse the same `--emit-ir` output as the
  oracle — a real structural assertion, not "a webview opened"); click a
  rendered state → assert the editor selection moves to that state's
  declaration line (via the IR `SourceLocation`); feed a **codegen-failing
  but parse-OK** fixture → assert the last-valid render persists + the
  Doc 05 §1.5.9 banner shows (the §6.1.3 boundary, proven not assumed);
  SVG export → assert a well-formed SVG with the state labels.

### V5 — activity-bar tree views + context-key chrome
- **Deliverable:** `fsm.machineExplorer` / `fsm.eventExplorer`
  `TreeDataProvider`s fed by the **free `documentSymbol`** response (§5);
  Doc 22 §7 view containers; Doc 22 §11 context keys
  (`fsm.hasOpenFsmFile`/`fsm.serverRunning`) driving `when`-clauses;
  status-bar machine indicator (Doc 22 §10).
- **Reuse seams:** `documentSymbol` (`server.rs:251`, free) as the tree
  data; client state for context keys. New-in-ext: the tree providers +
  context-key plumbing.
- **§5.4-analogue acceptance:** Extension-Host test opens a multi-machine
  fixture → assert the Machines tree's nodes equal the `documentSymbol`
  machine set (re-projected, not re-analyzed — assert it matches the
  client's `executeDocumentSymbolProvider` result exactly); toggle a
  `.fsm` open/closed → assert `fsm.hasOpenFsmFile`-gated views
  appear/disappear; clicking a tree node reveals the declaration.

### V6 — multi-platform binary bundling + VSIX packaging (gated on G9)
- **Deliverable:** the Doc 22 §12 5-platform `bin/{triple}/fsm-lang-server`
  layout + the platform-detection resolver; `@vscode/vsce package` →
  installable `.vsix`; the Doc 22 §12 "no bundled binary for {platform}"
  honest degradation verified per-triple. **Dependency: requires the G9
  cross-OS CI matrix to exist** (risk 2) — if G9 is still absent the
  orchestrator runs V6 host-only and records the multi-platform tail as
  blocked-on-G9 (infra-escalation, not grind). Publishing/signing is OUT
  (risk 4 — owner decision).
- **Reuse seams:** the resolver from V1 (host-only) generalized to all
  triples. New-in-ext: the bundling build step + VSIX packaging lane.
- **§5.4-analogue acceptance:** per available platform in the (G9) matrix,
  an Extension-Host smoke that the bundled `fsm-lang-server` for that
  triple launches and produces a correct diagnostic on a broken fixture
  (the V1 acceptance, re-run per-triple — proves the bundled binary on
  that OS actually works, especially the SEC-P0-1 path-canon seam, risk
  2); an un-bundled triple shows the Doc 22 §12 error, not a silent dead
  client.

**Sequencing rationale.** V1 (spine — the client↔shipped-server wiring +
the position-encoding seam end-to-end, the riskiest bet, validated first,
exactly as LSP-L1 did for the analyzer reuse seam) → V2/V3 (static assets
+ thin CLI/client glue, low-risk, independently parallelisable after V1)
→ V4 (the one substantive new subsystem, the diagram, where the §6 DRIFT
class lives — gated after V1 so the client + IR-CLI seams are proven) →
V5 (chrome reusing the now-proven free `documentSymbol`) → V6 (the
infra-gated multi-platform tail, last because it depends on G9 existing).
A phase-boundary audit (FSM-PROC-SUBAGENT §11.3) after **V1** (the spine)
and after **V4** (the new subsystem) — the same discipline that would
have caught P0-1 at the boundary and that Doc 26 §8 applied at L1/L5.

---

# 9. What this doc deliberately scopes OUT of v1.3 (no implied freebies)

- **A diagram→source transform / graphical editing.** Drag-to-edit needs
  a Webview-edit → `.fsm`-rewrite path; the only in-tree decompile is the
  "v1.0 stub" (`cli.rs:49`). v1.3 diagram is **read-only** (§6.2). Editing
  is a separate epic with its own acceptance.
- **A new LSP `fsm/diagram`/`fsm/ir` request OR the `fsm ir` CLI
  subcommand.** Neither exists (§6.1.1/§6.1.2 — verified against
  `server.rs`/`cli.rs`, the Doc-26-§4.5 drift class). The diagram is
  pinned to `fsm generate --emit-ir` (the one real path). Adding a server
  method or the `fsm ir` command is each a self-contained later epic.
- **Simulator panel / simulator diagram-overlay** (Doc 22 §4
  `fsm.openSimulator`, Doc 05 §1.5.8). The Doc 13 simulator WebSocket is
  unbuilt (no TCP in the server at all, `main.rs:39-48`); the
  `fsm.openSimulator` command + `simulator.port` setting + Doc 05 §1.5.8
  active-state overlay are a **later epic** (the ex-"v1.3 simulation"
  ROADMAP §132 scope, now sequenced after the extension — §1).
- **TCP transport for the language client.** The server is stdio-only
  (`main.rs:39-48`); the extension uses stdio (§2.2). A TCP option is a
  server-side non-feature in v1.3.
- **Cross-file go-to-definition / references / rename in the editor.**
  These degrade per the *server's* own single-file boundary (Doc 26
  §4.6/§9; Doc 00 §11.34(3)/§11.36(3)) — the extension surfaces exactly
  what the server returns; it does not add a client-side project index.
  Cross-file is the server's deferred v1.3+ work (Doc 26 §9), not the
  extension's.
- **Marketplace publishing / signing** (risk 4) — an owner
  credential/product decision; v1.3 produces an installable VSIX only.

If Doc 22 / Doc 05 imply any of the above is "free", it is not — each is
a real subsystem and is deferred explicitly rather than half-built (the
Doc 26 §9 discipline, applied to the extension).

---

# 10. Doc 22 vs shipped LSP — verified

Each load-bearing Doc 22 claim about what the extension wires to,
verified against the real `crates/fsm-lsp` / `crates/fsm-cli` at HEAD
`4f7e9ea`. ✅ = affirmed accurate; ⚠️ = imprecise / intent-shorthand
(works, but the doc over/under-states); ❌ = drift (the doc asserts
something the shipped code does not provide). Per the §11.33(3)/§11.34/
§11.35/§11.36/§11.37/§11.38 discipline: affirm the accurate, flag the
imprecise precisely, do **not** overclaim a defect.

| Doc 22 claim | Verdict | Note + fsm-lsp/cli evidence |
|---|---|---|
| §13.1 "Spawn `fsm-lang-server` with **stdio** transport" | ✅ | Exactly the only supported path. `main.rs:53-65` → `run_stdio()`; `lib.rs:165-169`. |
| §13.1/§13.2 lifecycle: `shutdown` then `exit`; crash `errorHandler` with backoff | ✅ | Server `shutdown` exists (`server.rs:375`); `tower-lsp` owns `exit`; the backoff handler is pure client code per Doc 22 §13.2 — accurate as a client contract. |
| §12 platform-detection bundles `fsm` (the *compiler*) binary | ⚠️ | The bundled/launched binary the **language client** needs is **`fsm-lang-server`**, not the `fsm` CLI. Doc 22 §12's code says `fsm${ext}`; the server bin is `fsm-lang-server` (`Cargo.toml [[bin]]`). v1.3 bundles BOTH (server for the client, `fsm` for the generate/IR commands §5) — Doc 22 §12 under-describes a genuine two-binary need. Not a defect, an imprecision; resolved §2.2/§5. |
| §8 `fsmLang.compilerPath` overrides the binary path | ✅ (client-side) | Honoured by the §2.2 resolver. The server itself never reads it (`config.rs` has only the 4 inlay keys) — correct: it is a client-launch concern. |
| §8 `fsmLang.enableInlayHints` + `inlayHints.{showTransitionPriorities,showStateTypes,showTimerDurations}` | ✅ | The server reads **exactly these four** (`config.rs:33,36,40,43,99-108`; Doc 00 §11.38(3)), nested + flat shapes, Doc 22 §8 defaults. Fully wired. |
| §8 `fsmLang.debounceMs` (50-2000, default 200) tunes re-analysis debounce | ❌ | The server debounce is a **hard-coded `const DEBOUNCE = Duration::from_millis(200)`** (`server.rs:100`); the key is **not read** anywhere. The 200 default coincidentally matches, but the setting is a no-op server-side. v1.3 must NOT present it as live (§2.5 rule). |
| §8 `fsmLang.maxProblems` caps diagnostics per file | ❌ | No diagnostic cap exists in `analysis.rs`/`server.rs`; the server publishes all analyzer diagnostics. Key is a server no-op. |
| §8 `fsmLang.diagnostics.{enableStyleWarnings,enableHints,actionComplexityThreshold}` | ❌ | Not wired — `config.rs` doc-comment (ll.4-11) + Doc 00 §11.38(3) / Doc 26 §7 open-q 8 explicitly "stub-accept the rest"; only the 4 inlay keys gate behaviour. Server no-ops. |
| §8 `fsmLang.codegen.{defaultTarget,outputDir,strategy}` / `format.{indentSize,bracketStyle}` | ⚠️ | Server no-ops (no codegen/format in `fsm-lsp`) — but **meaningfully consumed by the extension's `fsm.generate*`/`fsm.formatDocument` commands** shelling the `fsm` CLI (§5). Not drift against the *extension* (the keys are real *there*); drift only if mis-attributed to the LSP. |
| §8 `fsmLang.simulator.port` (default 7842) | ❌ | No simulator and **no TCP at all** in the server (`main.rs:39-48` `--port` unimplemented, exits 2). Pure no-op in v1.3; belongs to the deferred simulator epic (§9). |
| §13.1 "server **debounces internally**" on didChange | ✅ | Exactly right — `server.rs:100,152-199` 200ms generation-counter debounce server-side (Doc 26 §4.3; Doc 00 §11.32). The client just forwards full-buffer changes. |
| §4 `fsm.copyIR` ("Copy IR JSON") is satisfiable | ⚠️ | Satisfiable, but **only** via `fsm generate --emit-ir` → `<machine>.ir.json` (`generate.rs:189-209`, `fsm_ir::to_json`). There is **no LSP IR method and no `fsm ir` subcommand** (`cli.rs:8-11` defers `ir` post-v1.0). Doc 22 §4 doesn't say *how*; the only real path is the codegen-gated `--emit-ir` (§6.1.3). Not a defect; a precision the V3/V4 waves must honour. |
| §4/§5/§6 command title prefix "FSM: …" vs `category:"FSM Studio"` vs Doc 22 §0/§I-27 canonical "FSM Studio:" | ⚠️ | Doc 22-**internal** inconsistency. Resolved by Doc 22 §0's own note + Doc 00 §I-27 (Doc 22 authoritative): IDs stay `fsm.*`, titles normalized to `FSM Studio:` (§2.5). Not an LSP drift. |
| §3 language id `fsm-lang` / `scopeName source.fsm` / `.fsm` (matches Doc 21) | ✅ | Consistent across Doc 21 §1/§3 and Doc 22 §3; the `fsm-lang-server` operates on this language id; no conflict. |
| §3 grammar supersedes nothing the LSP needs; coexists with semantic tokens | ✅ | Server ships `semanticTokens` full+range (`server.rs:310-319`, Doc 00 §11.37); Doc 21 §6 defers the legend to FSM-SPEC-LSP §10 → no legend conflict (Doc 00 §11.37(4)). TextMate = offline layer beneath; accurate. |
| Implied: `workspace/symbol` available to power a workspace-wide picker | ❌ (not a Doc-22 claim, but a likely ext assumption — flagged) | `workspace_symbol_provider` is **absent** from `ServerCapabilities` (`server.rs:238-360`, only `..Default::default()`); left unadvertised by the L2 judgment call (Doc 00 §11.33(5)). Any ext "go to symbol in workspace" over `.fsm` is unsupported in v1.3 (single-file only, §9). |
| Implied: cross-file go-to-definition / references / rename work in-editor | ❌ (ext assumption — flagged) | Server is **single-file** for all semantic features (Doc 26 §4.6/§9; Doc 00 §11.34(3)/§11.36(3)); `definition`/`references` resolve within the open buffer, `rename` rejects cross-file with Doc 14 §8's verbatim string. The extension surfaces exactly that; it must not imply cross-file works. |

**Summary:** Doc 22's *structural* claims (stdio transport, internal
debounce, lifecycle, language/grammar identity, the four inlay keys, the
diagram being ext-built) are **accurate** and affirmed. Doc 22's
**config schema is largely aspirational against the shipped server**:
of the §8 keys, only the four inlay toggles are server-wired;
`debounceMs`/`maxProblems`/`diagnostics.*`/`simulator.port` are server
no-ops (❌), `codegen.*`/`format.*` are server no-ops but legitimately
ext-command-consumed (⚠️). The two sharpest drifts to brief every wave
with: **no LSP/`fsm ir` IR method (diagram → `fsm generate --emit-ir`
only)** and **no TCP / no simulator in v1.3**. None of these is an LSP
defect — the LSP is feature-complete and correct for its v1.2 scope; they
are Doc-22 intent-shorthand the v1.3 waves must scope against reality.

---

*End of FSM-ARCH-VSCE v1.0.0*
