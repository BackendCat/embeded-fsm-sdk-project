# AUDIT — §11.3 Post-V4 Phase-Boundary Audit (v1.3-V4 diagram WebviewPanel)

- **Scope:** the Doc-28-mandated `V4 → audit → V5` gate (FSM-PROC-SUBAGENT
  §11.3 / Doc 28 §3 "Phase-boundary audits (SUBAGENT §11.3) after **V1**
  (the spine) and after **V4** (the new subsystem)" — Doc 28:337-338,
  379-380). V4 is *the* one substantive new subsystem (the diagram
  WebviewPanel where the §6 codegen-gated-IR DRIFT lives); this is the
  second and last Doc-28-mandated phase-boundary audit before the v1.3 tail
  (V5 chrome, V6 infra-gated bundling).
- **Subject:** the merged `editors/vscode/` at `main` HEAD **`4c4c6a2`**
  ("v1.3-V4: read-only diagram WebviewPanel (IR-sourced ELK graph) (Doc 28
  §3-V4)"). Linear ancestry: `61cc3f0` (V2/V3 merge) → `4aaef5d` (the
  V2/V3 audit doc) → `4c4c6a2` (V4); merge-base spine `6707d1d` (V1),
  W0-clean root `ceb8efd`.
- **Method:** READ-ONLY judgment audit. **ZERO** `cargo`/`npm`/`tsc`/
  `esbuild` build. Derived by reading shipped source
  (`editors/vscode/src/diagram/**`, `…/commands/**`, `…/extension.ts`,
  `…/package.json`, `…/esbuild.mjs`, `…/tsconfig*.json`, the V4 test) +
  `git -C` + `grep`, cross-read against Docs 27/28/22/21/05/09 and the
  prior `AUDIT_PHASE_V1_2026_05_16.md` / `AUDIT_PHASE_V2V3_2026_05_16.md`.
- **Established context (NOT re-litigated — already independently verified
  per the brief):** V4 (`4c4c6a2`) merged; §11.1 V1..V4 returned COMBINED
  GATE GREEN with V1/V3/V4 oracles independently recomputed+matched
  (V1 `{2,0,11,0}`/`{8,8,9,4}`; V3 copyIR sha; V4 3 nodes/2 edges +
  verbatim Doc 05 §1.5.9 banner); 24/24 coexisting zero-regression; the
  `copyIr.ts`→`emitIr.ts` DRIFT-2 extraction proven pre/post-identical
  (V3 tests 18+19 pass); elkjs@0.11.1 lockfile-pristine (1 entry, zero
  transitives, webview-bundle-only); zero Rust delta vs W0-clean
  `ceb8efd`. This audit **re-confirms the byte-identity + the V4 commit
  scope from `git` itself** (verify-the-record, §5), then judges
  combined-extension coherence for V5 and pins the V5 data-source
  contract — it does **not** re-run the §11.1 gate.

---

## 0. Verdict (TL;DR)

**V5-READINESS: PROCEED-WITH-NOTES.**

The phase boundary is **clean enough to dispatch V5** (activity-bar tree
views + context-key chrome) per Doc 28's cadence. V4 is a correctly-scoped,
additive new subsystem that lands on the V1/V2/V3 substrate without rework;
its one DRIFT-class boundary (the codegen-gated IR) is **handled, proven,
and architecturally sealed**; and — the V5 keystone — **the data seam V5
must consume is real, already shipped, already client-registered, and
structurally disjoint from everything V4 added**:

- **The Rust workspace is byte-untouched across the whole W0→V4 batch.**
  `git diff ceb8efd 4c4c6a2 -- crates/ Cargo.toml Cargo.lock
  rust-toolchain.toml` is **empty** (re-confirmed from `git` this audit);
  the full V4 commit delta (`61cc3f0..4c4c6a2`) is 14 files / 2503 ins /
  109 del, **all under `editors/vscode/` + the V2/V3 audit doc**. The
  cargo quad is structurally incapable of regressing from this commit; V5
  (also pure `editors/vscode/`) inherits that safety.
- **The V5 data-source seam is RESOLVED, not assumed (Lens 1, §1 — the
  decisive finding).** Doc 28 §3-V5 / Doc 27 §8-V5 / §3 establish the V5
  tree is fed by the **free LSP `documentSymbol`** response. Verified
  end-to-end against shipped code: `server.rs:251`
  `document_symbol_provider: Some(OneOf::Left(true))` advertises the
  capability; `crates/fsm-lsp/src/capabilities/document_symbol.rs:61-66`
  builds the Doc 14 §13 hierarchical tree from the **same single
  `analyze_with_source` result** `publishDiagnostics` uses (analysis.rs:49,
  53, 83 — "no second analysis"); V1's `LanguageClient`
  (extension.ts:239-244, `documentSelector` `fsm-lang`, **no `capabilities`
  override** — extension.ts:234-236) auto-registers the provider so
  `vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider")`
  round-trips with **zero new extension code** (Doc 27 §3:229 "FREE").
  **V5 MUST consume the LSP-client `documentSymbol` seam — NOT V4's
  `emitIr.ts`/`irGraph.ts` IR path.** This is the decisive pin (§1.3): the
  two seams are different sources for different shapes, and choosing the
  IR path for the tree would (a) re-introduce the codegen-gated-IR DRIFT
  into a chrome wave that has no business touching it, and (b) duplicate
  an analysis the server already does for free. The recurring structural
  foot-gun (V1's omit-transport N-1, V4's "ask the LSP for the IR") here
  is its mirror: **a V5 that builds the tree from `--emit-ir` instead of
  `documentSymbol`** — the brief MUST forbid it (MV5-1, §1.3/§4).
- **V4 is strictly disjoint + additive on V1/V2/V3 (§1.4/§3).** V4
  contributes exactly ONE command (`fsm.openDiagram`) + its manifest
  forward-slots (the keybinding + `editor/title navigation` /
  `editor/context fsm@1` / `commandPalette` slots V3's JC-5 deliberately
  reserved — MV4-3, now verified discharged: package.json delta
  `61cc3f0..4c4c6a2`). The only V1 touch is the additive
  `registerOpenDiagram(...)` call site (extension.ts:145-150) — shape-
  identical to V3's `registerCommands`; V1's omit-transport `Executable`
  (extension.ts:186-193) is **byte-unchanged** (MV4-2 / M-1 honoured by
  construction — V4 touches no client options, constructs no
  `ServerOptions`).
- **The V4 codegen-gated DRIFT (K-3) is sealed, not just handled (§1.2 /
  §3.A).** The recommended `copyIr.ts`→`emitIr.ts` extraction shipped
  (the V2/V3-audit §1.2 "clean move"): the ONE real
  `fsm generate --emit-ir`-to-temp path is now the single shared
  `diagram/emitIr.ts` core (emitIr.ts:86-175) consumed by **both**
  `copyIR` (copyIr.ts:54-91) and the V4 panel (diagramPanel.ts:118-175)
  — verified by `grep` the ONLY `--emit-ir` acquisition site in `src/`
  (no second resolver, no duplicated cardinal-sin logic). The
  codegen-gated boundary is surfaced as a typed `codegenFailed`
  (emitIr.ts:136-148) the panel maps to **last-valid-render-retained +
  the Doc 05 §1.5.9 banner VERBATIM** (diagramPanel.ts:41-42, 129-143;
  the V4 test asserts both at the real Webview ack — diagram.test.ts:368-526).

The **NOTES** (none are V5 ship-blockers; classified by when each must
fold):

- **C-1 (CANONICAL ACCUMULATED DEFERRED-BATCH CATALOGUE — the
  correction-of-record consolidation):** §2 is the single consolidated
  list the orchestrator folds at the v1.3-closeout batched-doc pass into
  `GATE_VERIFICATION_v1_3` / Doc 00 §11.N. It carries forward the V2/V3
  audit §2 in full (F-1+F-2/N-1, N-2..N-4, N-5 CLOSED, N-6
  CORRECTED-IN-SHIPPED, JC-3 codegen-config-schema, JC-1 xvfb, the
  Doc-22-§4-title re-pin) and ADDS the V4 items: the
  `copyIr.ts`→`emitIr.ts` extraction (a **POSITIVE closeout note** —
  audit-§1.2-recommended, copyIR re-proved unregressed §11.1, NOT a
  defect); V4 JC-2 (codegen-ICE boundary tested via the V3-blessed
  `emitIr` non-zero-exit path — sound, audit-blessed); V4 JC-3
  (panel-identity = file-path not machine-name — a real bug its own gate
  caught + fixed; **see §3.A — this also resolves the Doc 05 §1.5.1
  prose tension**); V4 JC-4 (separate strict `tsconfig.webview.json` —
  additive, Zero-Legacy-correct). Per the W0/V1/V2V3 §11.3 pattern **this
  audit doc is the durable correction-of-record** until that batched
  pass. **Mandatory before V5 is not the doc edits but that the V5 brief
  cites §1 and §2 of THIS audit, not the stale Doc 27 §8-V5 / Doc 22 §7
  prose alone.**
- **C-2 (the V5 data-source contract — MANDATORY-BEFORE-V5 as a briefing
  input):** §1.3 is the exact V5 acceptance contract. The V5 brief MUST
  pin: data source = the **client `documentSymbol`** seam
  (`vscode.executeDocumentSymbolProvider`, the free V1-client capability,
  `server.rs:251`), explicitly **NOT** V4's `emitIr.ts`/`irGraph.ts` IR
  path and **NOT** a new server method (R-9/R-10; the Doc 26 §4.5 "do not
  smuggle a subsystem" bar); the tree-acceptance = "the Machines tree's
  nodes equal the client's `executeDocumentSymbolProvider` result
  **exactly**, re-projected not re-analyzed" (Doc 28 §3-V5:345-350),
  proven by an Extension-Host test, **not assumed**; the context keys
  (`fsm.hasOpenFsmFile`/`fsm.serverRunning`) driven by VS Code
  state/client-lifecycle (NOT a new analysis); the R-15 fixture isolation
  inherited.
- **C-3 (the disjoint package.json section V5 owns):** §3.C confirms
  `contributes.{viewsContainers,views,viewsWelcome}` + the context-key
  `when`-machinery are **entirely absent** at `4c4c6a2` (grep-verified,
  zero hits in `package.json` *and* `src/`). V5 owns exactly Doc 22 §7
  (`viewsContainers.activitybar` `fsm-explorer` + `views.fsm-explorer`
  `fsm.machineExplorer`/`fsm.eventExplorer` `when:fsm.hasOpenFsmFile`) +
  Doc 22 §11 context keys + the tree-item `view/title`/`view/item/context`
  menus + the `setContext` plumbing — a region **disjoint from every
  V1-V4 manifest key** (languages/grammars/snippets/commands/keybindings/
  menus[editor*]/configuration). No collision risk; the brief states the
  exact owned region (§4).
- **JC-3 (V2/V3-batch, CARRIED, deferrable-to-closeout):**
  `fsmLang.codegen.outputDir`/`codegen.strategy` extension-consumed by
  `generate.ts` with Doc-22-§8 defaults but NOT in
  `contributes.configuration.properties` (re-verified unchanged this
  audit — V4 added no config keys; package.json delta shows zero
  `configuration` touch). NOT a V5 blocker (V5 contributes views/context
  keys, reads no `codegen.*`). Fold into the v1.3-closeout config-owner
  pass.
- **JC-1 (ENVIRONMENTAL, CARRIED, deferrable-to-closeout):**
  `@vscode/test-electron` needs a DISPLAY; the V4 test added a real
  Webview (diagram.test.ts) so the xvfb requirement is now *strictly*
  load-bearing for the §11.1 / pre-tag JS-quad. Carry on **every** future
  §11.1 brief + wire into the Doc 28 §2.3 Option-A JS lane.

This audit deliberately shows the **V5-data-source seam analysis** (§1, the
keystone) and the **V4-shipped-vs-Doc spot-checks** (§3, the Doc 05 §1.5.1
panel-identity tension + Doc 27 §6.2/§8-V4 CSP/elkjs/click→source) because
"an audit that finds nothing on the one substantive new subsystem that
already surfaced K-3/J-1/JC-class items is suspect" — the V5
`documentSymbol` seam *is* traced to file:line, the V4 doc-vs-shipped
deltas *are* enumerated (the JC-3 file-path-identity vs Doc 05 §1.5.1
wording resolved in §3.A), and the surviving drift is named with the exact
fold-target rather than papered over.

---

## 1. Lens 1 — Combined-extension coherence for V5 (the V5 data-source seam)

V5 per Doc 27 §8-V5 / Doc 28 §3-V5 is the activity-bar chrome:
`fsm.machineExplorer`/`fsm.eventExplorer` `TreeDataProvider`s + Doc 22 §7
view containers + Doc 22 §11 context keys (`fsm.hasOpenFsmFile`/
`fsm.serverRunning`) driving `when`-clauses. It is explicitly the **least
risky remaining wave** (Doc 27 §3:254-256 "chrome … low-risk static/glue")
— but the keystone question is the same one that bit V1 (omit-transport
N-1) and that V4 had to seal (the no-LSP-IR-method K-3): **from which
existing seam does the tree get its data, and is choosing the wrong one
the structural foot-gun here?**

### 1.1 The decisive finding — V5's seam is the FREE `documentSymbol`, end-to-end verified

Doc 27 §3:229/§3:248, Doc 27 §8-V5:596-602, Doc 28 §3-V5:341-344 all
pin the V5 tree data to the **free `documentSymbol`** LSP response. The
brief instructs me to decide and justify which existing seam V5 MUST
consume — the LSP client (live `documentSymbol`/symbols V1's client
already provides) or the IR (the V3 `emitIr`/`cliRunner` codegen-gated
path V4 uses). **It is `documentSymbol`, and it is real + free + already
client-registered, by reading shipped code:**

- **The server advertises it.** `crates/fsm-lsp/src/server.rs:251`:
  `document_symbol_provider: Some(OneOf::Left(true))` — the Doc 14 §2
  capability is shipped (not aspirational; this is the verified-against-
  Rust fact Doc 27 §3:229 / Doc 00 §11.33 record).
- **It is the SAME single-analysis seam, not a new one.**
  `crates/fsm-lsp/src/capabilities/document_symbol.rs:61-66`
  (`document_symbols(...)` → "one root `DocumentSymbol` per declared
  machine", the Doc 14 §13 hierarchical tree) is driven by
  `analyze_with_source` — `analysis.rs:49,53,83` document it as "the SAME
  single `analyze_with_source` result `documentSymbol` uses … no second
  analysis". The tree is therefore as un-drift-able as the squiggle (the
  Doc 27 §6.3 "reuse the one pipeline" discipline — the exact reason V4's
  diagram consumes the canonical IR, mirrored here for the tree consuming
  the canonical symbol projection).
- **V1's client makes it FREE with zero new code.** `extension.ts:239-244`
  builds the `LanguageClient` with `documentSelector:[{language:
  "fsm-lang"}]` and **no `capabilities`/`clientOptions` override**
  (extension.ts:234-236 — the deliberate N-1 guard). `vscode-
  languageclient` therefore auto-registers the document-symbol provider,
  so `vscode.commands.executeCommand(
  "vscode.executeDocumentSymbolProvider", uri)` round-trips to the shipped
  server with **zero extension code** beyond the already-shipped client
  (Doc 27 §3:221-224 "the client library translates … with zero extension
  code"). V5 does not build, request, or parse anything new at the LSP
  layer — it only *re-projects* the response into `TreeItem`s.
- **The V4 IR path is the WRONG seam for the tree — and using it is the
  V5 foot-gun.** V4's `emitIr.ts`/`irGraph.ts` produce the
  **codegen-gated** `fsm-ir` graph (states/transitions for the *diagram*).
  The tree wants the **symbol outline** (machines → states/events/externs,
  Doc 05 §1.3.3-§1.3.6) which `documentSymbol` already returns *without*
  the codegen gate. A V5 that reached for `emitIr`/`irGraph` would (a)
  re-import the codegen-gated-IR DRIFT (K-3) into a chrome wave that has
  no reason to touch codegen — a machine that won't codegen would make
  the *tree* vanish, which is absurd and a regression vs the
  always-available `documentSymbol`; (b) duplicate an analysis the server
  does for free; (c) be the precise Doc-26-§4.5 wrong-seam trap, the
  symmetric analogue of V1's omit-transport N-1 and V4's
  "ask-the-LSP-for-the-IR". **This is the load-bearing pin (§1.3 / MV5-1).**

### 1.2 V4 integration check — the recommended extraction shipped, sealed, drift-free

The V2/V3 audit §1.2 *recommended* extracting the shared
"generate-IR-to-temp → locate `*.ir.json` → read" core out of `copyIr.ts`
into a reusable `emitIr` helper consumed by both `copyIR` and V4. **It
shipped, exactly, and it is sealed:**

- **One seam, verified the ONLY one.** `emitIr.ts:86-175` is the single
  `fsm generate --target c99 --out <tmp> --emit-ir <file>` site. `grep`
  for every `resolveCliBinary`/`runCli` caller in `src/` (excluding
  tests + the seam files themselves) returns only `checkFile.ts`
  (`check`), `generate.ts` (`generate`), `formatDocument.ts` (`fmt`) and
  `emitIr.ts` (`generate --emit-ir`) — **four distinct subcommands, no
  duplicated `--emit-ir` path, no second resolver.** The cardinal-sin
  honest-`runCli` logic is centralised exactly once (cliRunner.ts is
  unchanged from V3).
- **`copyIR` is pre/post-identical by construction.** copyIr.ts:54-91
  delegates to `emitIr` with the exact behaviour mapping its WHY-header
  enumerates (copyIr.ts:18-30): `ok`→write clipboard; `codegenFailed`/
  `noIrFile`→the same "cannot copy IR — codegen failed … codegen-gated"
  decline; `noCli`/`spawnError`→the same errors. The `git diff
  3c49d38..4c4c6a2 -- copyIr.ts` is a pure inline→shared-call extraction
  (the cardinal-sin branch logic moved verbatim into emitIr.ts:136-164).
  Per the established context the V3 copyIR tests 18+19 re-passed (the
  SUBAGENT §10 refactor pre/post-identity bar — satisfied). **This is a
  POSITIVE closeout note, not a defect (§2).**
- **The codegen-gated boundary is the Webview analogue of V3's
  `copyIr.ts:91-104`, proven not assumed.** emitIr.ts:136-148 maps a
  non-zero exit to typed `codegenFailed` (never fabricates an IR — the
  WHY-header emitIr.ts:21-30 cites `generate.rs:165-209`); the panel
  (diagramPanel.ts:118-143) keeps `lastValidModel` and posts the
  `STALE_BANNER` (diagramPanel.ts:41-42, the Doc 05 §1.5.9 string
  VERBATIM); the Webview shows it without touching the rendered SVG
  (diagramWebview.ts:282-298) and on first-ever-open shows an honest
  placeholder, never a blank panel as success (diagramWebview.ts:287-291).
  diagram.test.ts:368-526 drives the **real** boundary (proves the CLI
  exits non-zero + writes no `.ir.json` at :430-467) and asserts
  last-valid RETAINED + the banner VERBATIM at the real Webview ack.

### 1.3 The V5 acceptance contract (C-2 — the exact pin for the V5 brief)

Per Doc 28 §3-V5 + Doc 27 §8-V5 + Doc 22 §7/§11, **proven not assumed**:

1. **Data source (HARD):** the tree is fed by
   `vscode.commands.executeCommand(
   "vscode.executeDocumentSymbolProvider", uri)` — the **free V1-client
   `documentSymbol`** capability (`server.rs:251`, the same
   single-analysis seam, `document_symbol.rs:61`). **NOT** V4's
   `emitIr.ts`/`irGraph.ts` (the codegen-gated IR is the wrong shape +
   re-imports K-3 into chrome), **NOT** a new server/LSP method, **NOT**
   `fsm ir` (R-9/R-10; the Doc 26 §4.5/§9 "do not smuggle a subsystem"
   bar — a V5 that adds a custom request or re-derives symbols from the
   IR is rejected back).
2. **Tree-fidelity (the structural assertion — MANDATORY Extension-Host
   test):** open a multi-machine fixture → assert the Machines tree's
   node set **equals the client's `executeDocumentSymbolProvider` result
   EXACTLY** (re-projected, not re-analyzed — the same
   "parse-the-same-artifact-as-the-oracle" discipline V4's test used for
   the IR, Doc 28 §3-V5:345-350). "A tree view appeared" / "a provider is
   registered" is explicitly **NOT** acceptance (the P0-1 bar; the exact
   wording the V4 gate enforced — diagram.test.ts:254-261 is a
   *precondition*, the structural equality :303-358 is the acceptance —
   V5 inherits this pattern).
3. **Context-key fidelity:** `fsm.hasOpenFsmFile` toggled by VS Code
   editor state, `fsm.serverRunning` by the V1 client lifecycle
   (`client.onDidChangeState` — extension.ts:248-261 already tracks it
   for the status bar; V5 mirrors that signal into `setContext`, NOT a
   new analysis). Test: toggle a `.fsm` open/closed → assert the
   `fsm.hasOpenFsmFile`-gated views appear/disappear (Doc 28 §3-V5:348-350).
4. **Click→declaration:** a tree node click reveals the declaration —
   reuse the **`documentSymbol` range** (the symbol already carries its
   `selectionRange`), the same "navigate via the authoritative source
   location" discipline V4 used for click→source via the IR
   `SourceLocation` (diagramPanel.ts:192-210). No new location math.
5. **R-15 inheritance:** the V5 fixture, like V1's/V4's, MUST have **no
   `fsm.toml [compiler] allow/deny` in scope** — `documentSymbol` runs
   the analyzer; an allow/deny fixture could project a different symbol
   set. Stage the V5 fixture in an OS temp dir with the up-tree
   `fsm.toml` walk hard-asserted empty (the `extension.test.ts:115-125`
   / `diagram.test.ts:177-195` pattern — reuse it verbatim).

**Net (Lens 1):** the V5 data-source seam is **RESOLVED, not open**: the
one real path (`documentSymbol`) is shipped, free, and already
client-registered by V1; V4's IR path is the *wrong* seam for the tree
and using it would re-import the K-3 DRIFT into chrome (the V5 structural
foot-gun the brief MUST forbid). V5 is architecturally unblocked
**provided MV5-1 is in its brief** (so a V5 implementer does not "build
the tree from `--emit-ir`" — the symmetric analogue of V1's omit-transport
N-1 and V4's ask-the-LSP-for-the-IR).

### 1.4 V4 disjointness + M-1 lineage — additive, no rework

- **V4 contributes exactly one command + its reserved manifest slots.**
  package.json delta `61cc3f0..4c4c6a2`: `fsm.openDiagram` command (bare
  title `"Open Diagram"` + `category:"FSM Studio"` + `icon
  $(type-hierarchy)` per Doc 05 §1.5.1 / the JC-4 pattern), the
  `ctrl+shift+d`/`cmd+shift+d` `when:editorLangId==fsm-lang` keybinding,
  and the **exact slots V3's JC-5/JC-6 reserved**: `editor/title` group
  `navigation`, `editor/context` group `fsm@1`, `commandPalette`
  `when:editorLangId==fsm-lang`. **MV4-3 fully discharged + verified.**
- **The only V1-region touch is additive.** extension.ts:145-150 adds
  `registerOpenDiagram(context, {getClient, outputChannel, restartServer,
  extensionPath})` — shape-identical to V3's `registerCommands`
  (extension.ts:129-134), placed before the no-binary early-return so the
  diagram works with no language-SERVER binary (its data is the `fsm`
  CLI, not `fsm-lang-server` — the genuine two-binary need Doc 27 §2.2
  records, resolved by the V3 `cliBinary.ts` seam emitIr.ts:38,92 reuses).
- **M-1 honoured by construction (MV4-2).** V4 constructs no
  `ServerOptions`, sets no `transport`, touches no `clientOptions`. V1's
  omit-transport `Executable` (extension.ts:186-193) + the deliberate
  no-`capabilities`-override (extension.ts:234-236) are **byte-unchanged**
  (`git diff 61cc3f0..4c4c6a2 -- extension.ts` is +17 lines, the
  `registerOpenDiagram` import + call only). The "never re-add
  `transport: TransportKind.stdio`, never switch to a `NodeModule` shape"
  rule rode V4 cleanly and binds V5 too (MV5-2).

---

## 2. Lens 2 — Canonical accumulated deferred-batch catalogue (the correction-of-record)

This is the single consolidated list the orchestrator folds at the
v1.3-closeout batched-doc pass into `GATE_VERIFICATION_v1_3` / Doc 00
§11.N. Per the W0/V1/V2V3 §11.3 pattern **this audit doc is the durable
correction-of-record until that pass.** Status legend: **OPEN** (needs
action), **CLOSED** (resolved in a shipped wave), **CORRECTED-IN-SHIPPED**
(shipped code already right; only doc prose stale), **POSITIVE**
(audit-recommended improvement that shipped — record as a closeout
*credit*, not a defect), **ENVIRONMENTAL** (CI/infra, not code).

| # | Source | What | Status | Exact reconciliation action | Gate |
|---|---|---|---|---|---|
| **F-1+F-2 / N-1** | `AUDIT_PHASE_V1` §3 (Doc 27 §2.2 transport; §2.3 + Doc 28 R-4 UTF-8) | (1) Doc 27 §2.2 `TransportKind.stdio` is a wrong literal — the only round-tripping form is **omit `transport`**. (2) UTF-8 `positionEncoding` is **unreachable** through `vscode-languageclient` (hardcoded `['utf-16']`); the correct negotiated value is **`"utf-16"`**. | **OPEN (doc-of-record fix; shipped code already correct)** | Fold `AUDIT_PHASE_V1` §3.A→Doc 27 §2.2; §3.B→Doc 27 §2.3/§7-r5/§8-V1(c) **and** Doc 28 §2.2/R-4/§3-V1(c)/§5-V1(c). No code change. | **Briefing-input mandatory: the V5 brief inherits the M-1 "never re-add `transport`/`NodeModule`" rule** (V5 touches no client options, but it rides every `editors/vscode/` wave). Re-confirmed unbroken at `4c4c6a2` (§1.4). Doc edits deferrable-to-closeout. |
| **N-2** | `AUDIT_PHASE_V1` §0/§4 (Doc 28 §3-V1/§5-V1 (c)) | Doc 28 §3/§5 (c) textually mandates the FALSE assertion (`positionEncoding == UTF-8`); shipped `extension.test.ts:194-206` correctly asserts `"utf-16"`. | **OPEN (doc-of-record fix; the desired behaviour)** | Fold the §3.B corrected-(c) text into Doc 28 §3-V1/§5-V1 (c). | Deferrable-to-closeout. Not a defect. |
| **N-3** | `AUDIT_PHASE_V1` §0/§3.A | Doc 27 §2.2's `main.rs:39-48` citation conflates three arms (`:39-48` `--port`, `:49-52` unknown-arg exit-2, `:53-65` `None→run_stdio`). | **OPEN (citation re-pin, non-load-bearing)** | Re-pin at closeout (text in `AUDIT_PHASE_V1` §3.A). | Deferrable-to-closeout (doc hygiene). |
| **N-4** | `AUDIT_PHASE_V1` §4 (D-8; Doc 22 §10) | Status-bar "restarting" **tooltip** renders generic `"…starting"` not Doc 22:679's literal. Inside the unexercised recovery path; icon/text already match. **Still present at `4c4c6a2`** (V4 touched no `statusBar.ts`). | **OPEN (cosmetic; the V1-flagged V5 deferral)** | Fold the tooltip-string polish into the **V5 status-bar work** (Doc 27 §8-V5:599 adds the `fsm.currentMachine`/Doc 22 §10 indicator — V5 *does* touch the status bar; this is its natural home). | Deferrable-to-closeout / **a V5 polish item** (NOT a blocker; record in the V5 brief as a fold-in). |
| **N-5** | `AUDIT_PHASE_V1` §4 (D-9) / §5 M-2 | `fsm.showOutputChannel`+`fsm.restartLanguageServer` wired by V1 but not contributed/registered until V3. | **CLOSED by V3** | None. Re-verified still closed at `4c4c6a2` (package.json:95-104 + `clientControl.ts` unchanged; V4 added no client-control). Record as the closed N-5 in `GATE_VERIFICATION_v1_3`. | None — closed. |
| **N-6** | V2 grammar inline `_note` (tmLanguage.json:6) + J-1 | Doc 21 §3's literal JSON is structurally defective (single-line `match`, no body rule → `#action-block` swallowed the machine body). | **CORRECTED-IN-SHIPPED by V2 (J-1 ratified per established context)** | Fold the §11.1-ratified J-1 correction into Doc 21 §3 at closeout (the grammar's inline `_note` is the durable correction-of-record meanwhile). Re-verified unchanged at `4c4c6a2` (V4 touched no `syntaxes/`). | Deferrable-to-closeout. **Not** a V5 concern. |
| **JC-3 (V2/V3-batch, CARRIED)** | V2/V3-audit §0/§3.B | `fsmLang.codegen.outputDir`/`codegen.strategy` extension-consumed by `generate.ts:34-57` with Doc-22-§8 defaults but **NOT** in `contributes.configuration.properties`. | **OPEN (a later config-owner wave)** | Add the two keys per Doc 22 §8:371-384 in the v1.3-closeout config-owner pass (with the R-6..R-8 server-dead-key ledger). **Re-verified at `4c4c6a2`: V4 added ZERO config keys** (package.json delta shows no `configuration` touch — the gap is exactly as the V2/V3 audit recorded). | **Deferrable-to-closeout.** NOT a V5 blocker (V5 contributes views/context-keys, reads no `codegen.*`). Not a correctness bug (defaults == Doc 22 §8). |
| **JC-1 (V2/V3-batch, environmental, CARRIED)** | V2/V3-audit §0 / §11.1 context | `@vscode/test-electron` launches a real Electron Host ⇒ needs DISPLAY. **V4 added a REAL Webview test (`diagram.test.ts`) — the xvfb requirement is now strictly load-bearing** (the V4 Webview ack cannot post without a rendering context). | **ENVIRONMENTAL (CI-lane wiring, not code)** | Wire `xvfb-run` into the Doc 28 §2.3 Option-A separate JS CI lane; record in `GATE_VERIFICATION_v1_3` §"pre-tag JS-quad". No source change. | Deferrable-to-closeout (CI-lane + gate-doc). Carry on **every** future §11.1 brief — now doubly so (the V4 Webview makes it non-optional). |
| **Doc-22-§4-title re-pin (V2/V3-batch, CARRIED)** | V2/V3-audit §3.A | Doc 22 §4 per-command `title`s still literal `"FSM: …"`; shipped uses bare title + `category:"FSM Studio"` (the correct JC-4 realization — palette shows `FSM Studio: <X>`). **V4's `fsm.openDiagram` followed the SAME correct pattern** (bare `"Open Diagram"` + `category:"FSM Studio"` — §1.4). | **CORRECTED-IN-SHIPPED (stale doc prose only)** | Re-pin the Doc 22 §4 titles to bare form (noting `category` produces the prefix) at the same closeout Doc-22 pass as JC-3. Non-load-bearing (N-3 class). | Deferrable-to-closeout. V4 *confirms* the pattern is the project convention. |
| **V4 extraction (NEW — POSITIVE)** | This audit §1.2 / V2/V3-audit §1.2 | The audit-§1.2-recommended `copyIr.ts`→`emitIr.ts` shared-core extraction **shipped**: one honest IR seam consumed by both `copyIR` + the V4 panel; `copyIR` pre/post-identical (V3 tests 18+19 re-passed per §11.1); `grep`-verified the ONLY `--emit-ir` site. | **POSITIVE (audit-recommended improvement that shipped; NOT a defect — a closeout *credit*)** | Record in `GATE_VERIFICATION_v1_3` as a *resolved* DRIFT-2-grade consolidation (the V2/V3-audit §1.2 recommendation, closed by V4). No action. | None — a credit. Documents that the recommended clean move was taken with the refactor-safety bar met. |
| **V4 JC-2 (NEW — codegen-ICE boundary test method)** | This audit §1.2 / §3.A; Doc 28 §3-V4:333-336 | The codegen-gated DRIFT-class assertion is tested via the **V3-blessed `emitIr` non-zero-exit path** (`broken.fsm` → CLI exits non-zero, no `.ir.json` — diagram.test.ts:417-467), NOT a hand-built "analyzer-clean BUT codegen-ICE" fixture. A deterministic such fixture is not constructible in-tree (the analyzer rejects most things codegen would; an ICE is non-deterministic) — the chosen path is the IDENTICAL `emitIr`/`refresh()` code a real codegen-ICE takes, so the contract IS exercised. | **OPEN (record only — the method is SOUND, audit-blessed)** | Record in `GATE_VERIFICATION_v1_3` that the V4 codegen-gated boundary is proven via the `emitIr` non-zero-exit equivalence (the AUDIT_PHASE_V2V3 §1.3.2 "analogue of copyIr.ts:91-104" — explicitly blessed there + here). No action. | None — a sound, audit-blessed test-strategy note (not a gap). |
| **V4 JC-3 (NEW — panel-identity = file-path; ALSO resolves the Doc 05 §1.5.1 tension)** | This audit §3.A; diagramPanel.ts:58-75,219-298 | The panel is keyed by **resolved file path**, NOT machine name (diagramPanel.ts:220-224 `views: Map<string,DiagramView>` by `path.resolve`). Doc 05 §1.5.1:272 says "**one diagram panel per machine**". A machine-name key would be a CORRECTNESS BUG: a parse/codegen error transiently removes the machine from the IR, so a re-open after an error would spawn a FRESH blank panel instead of focusing the existing one + showing last-valid+banner (the §1.5.9 "identity stable across re-renders" contract). **The shipped file-path key is the CORRECT realization of §1.5.1's intent** (in v1.3 the diagram shows the file's *first* machine — diagramPanel.ts:147-149 — so "per machine" ≡ "per file" for the one-machine-per-panel v1.3 scope; the WHY-comment diagramPanel.ts:58-75 documents this explicitly as a load-bearing design point its own gate caught + fixed). | **CORRECTED-IN-SHIPPED (shipped code correct; Doc 05 §1.5.1 prose under-specifies the error-transient case)** | Fold a Doc 05 §1.5.1/§1.5.9 clarification at closeout: "panel identity is the **source file** (the file's first machine); 'per machine' applies within the v1.3 one-machine-per-panel scope; identity is file-path so the panel + last-valid render survive a transient parse/codegen error that removes the machine from the IR" (the diagramPanel.ts:58-75 `_note` is the durable correction-of-record meanwhile — the N-6 DRIFT-2 "fix-where-demonstrably-needed + explain" discipline). | Deferrable-to-closeout. **Not** a V5 concern (V5 is the tree, not the diagram panel). NOT a defect — the *correct* state; recorded so a future "code ≠ Doc 05" mechanical check does not mis-flag it. |
| **V4 JC-4 (NEW — separate strict `tsconfig.webview.json`)** | This audit §3.B; tsconfig.webview.json:1-34 | V4 added a SEPARATE `tsconfig.webview.json` (`lib:["ES2020","DOM"]`, `strict`, `noUnusedLocals/Parameters`, `noImplicitReturns`, `noEmit`) for the browser-context Webview script + wired it into `npm run typecheck`/`pretest` (package.json:241,244 delta). The main `tsconfig.json` (a Node program, `lib ES2022`) correctly excludes it (esbuild owns the browser bundle, `platform:"browser"`). | **POSITIVE (additive, Zero-Legacy-correct; NOT a defect)** | Record in `GATE_VERIFICATION_v1_3` as a *credit*: the Webview is fully type-checked under strict flags + the `DOM` lib it genuinely targets (no untyped code — Zero-Legacy). No action. | None — a credit. The wire shape (irGraph.ts ↔ diagramWebview.ts:31-66) is a deliberate, commented ~5-field duplication of the *model shape* (NOT logic), the correct Webview-can't-import-extension pattern. |

**Mandatory-before-V5 (briefing inputs, not code/doc edits):**
- **C-2 / MV5-1 (the V5 data-source contract):** §1.3 — the V5 brief MUST
  pin data-source = the **free V1-client `documentSymbol`** seam
  (`vscode.executeDocumentSymbolProvider`, `server.rs:251`), explicitly
  **NOT** V4's `emitIr.ts`/`irGraph.ts` IR path (re-imports the K-3
  codegen-gated DRIFT into chrome + wrong shape) and **NOT** a new
  server method (R-9/R-10), with the tree-fidelity asserted EXACTLY
  against the client's `executeDocumentSymbolProvider` result by an
  Extension-Host test ("a tree appeared" is NOT acceptance — the P0-1
  bar), and the R-15 fixture isolation inherited.
- **MV5-2 (M-1 lineage, F-1+F-2/N-1 carry):** the V5 brief MUST carry the
  "never re-add `transport: TransportKind.stdio`, never switch to a
  `NodeModule` server shape" constraint (V5 touches no client options,
  but this binds every `editors/vscode/` wave; cite `AUDIT_PHASE_V1`
  §3.A + this §1.4).
- **MV5-3 (the disjoint manifest region V5 owns — C-3):** the V5 brief
  MUST scope V5 to exactly Doc 22 §7 (`contributes.viewsContainers
  .activitybar` `fsm-explorer` + `contributes.views.fsm-explorer`
  `fsm.machineExplorer`/`fsm.eventExplorer` `when:fsm.hasOpenFsmFile`),
  Doc 22 §11 context keys (`fsm.hasOpenFsmFile`/`fsm.serverRunning` via
  `setContext`; `fsm.simulatorConnected` is OUT — no simulator in v1.3,
  Doc 27 §9), the tree `view/title`/`view/item/context` menus, and
  `contributes.viewsWelcome` for the no-`.fsm`-open state — a region
  **disjoint from every V1-V4 key** (§3.C grep-verified absent). It MUST
  also fold N-4 (the status-bar restarting-tooltip polish) into V5's
  Doc 22 §10 status-bar machine-indicator work.

**Deferrable-to-v1.3-closeout (batched Doc-00 / `GATE_VERIFICATION_v1_3`
pass):** the F-1+F-2/N-1, N-2, N-3 doc edits; **N-4** tooltip (now a V5
fold-in, MV5-3); N-6 → Doc 21 §3 (J-1 ratified); the Doc-22-§4-title
re-pin (N-3 class) + V4 confirms the convention; **JC-3** (→ config-owner
wave); **JC-1** (→ JS CI-lane + gate-doc, now non-optional post-V4
Webview); **V4 JC-3** (→ Doc 05 §1.5.1/§1.5.9 clarification). **CLOSED
(record only):** **N-5** (closed by V3). **POSITIVE (closeout credits, no
action):** the V4 `copyIr→emitIr` extraction; V4 JC-2 (sound test
strategy); V4 JC-4 (strict Webview tsconfig).

---

## 3. Lens 3 — New V4 doc-vs-shipped drift sweep

Beyond the catalogued, I spot-checked the brief's named surfaces — Doc 27
§8-V4 (CSP-locked webview, elkjs, click→source) + Doc 05 §1.5.1/§1.5.9
(panel identity, last-valid+banner) — against the shipped
`src/diagram/*` at `4c4c6a2` (file:line).

### 3.A Doc 05 §1.5.1/§1.5.9 — panel identity (the JC-3 file-path-vs-prose tension)

- **§1.5.1:272 "One diagram panel per machine. Re-using the same command
  focuses existing panel."** The shipped `DiagramController` keys
  `views: Map<string,DiagramView>` by **resolved file path**
  (diagramPanel.ts:220-224, 248-256: `key = path.resolve(fsmPath)`;
  existing → `existing.reveal()` + `refresh()`, Doc 05 §1.5.1 "focus the
  existing panel"). The "focus-existing" half is **byte-faithful** to
  §1.5.1. The "per machine" half is realized as "per file" — and this is
  **the correct realization, not drift** (the JC-3 catalogue item):
  diagramPanel.ts:147-149 renders the file's *first* machine
  (`parseAndBuild(res.json)` with no machine-name arg → `irGraph.ts:251`
  `ir.machines[0]`), so in the v1.3 one-machine-per-panel scope "per
  machine" ≡ "per file". The diagramPanel.ts:58-75 WHY-comment documents
  *why* file-path identity is load-bearing: a machine-name key would make
  a re-open after a transient parse/codegen error spawn a **fresh blank
  panel** instead of focusing the existing one + showing last-valid+banner
  — a §1.5.9 violation. **Verdict: ✅ CORRECT — shipped code is the right
  realization; Doc 05 §1.5.1 prose merely under-specifies the
  error-transient case. Catalogued as V4 JC-3 (CORRECTED-IN-SHIPPED);
  the inline `_note` is the durable correction-of-record; the Doc 05
  clarification folds at closeout.** No NEW drift.
- **§1.5.9 banner string.** Doc 05:407-409 specs the verbatim
  `⚠ Diagram shows last valid state. Fix parse errors to update.`. Shipped
  `STALE_BANNER` (diagramPanel.ts:41-42) and the test's local copy
  (diagram.test.ts:67-68) are **byte-identical** to Doc 05:408; the
  V4 test :531-537 asserts the SUT constant === the verbatim string (a
  paraphrase fails CI, not a user). ✅ **VERBATIM — no drift.**
- **§1.5.9 last-valid-on-failure + identity-stable.** Doc 05:406
  ("re-parse fails → last valid + banner"), :411 ("zoom/pan preserved
  across re-renders if machine identity is the same"). Shipped:
  diagramPanel.ts:129-143 (codegen/spawn/no-IR → keep last-valid + post
  banner, NEVER blank), :164-174 (only a *successful* render updates
  `lastValidModel`), `retainContextWhenHidden:true` (diagramPanel.ts:267
  — the zoom/pan-preservation substrate). The file-path key (JC-3) is
  exactly what makes §1.5.9 "identity stable across re-renders" hold
  across the error-transient. ✅ **Faithful — no drift.**
- **§1.5.1:271 tab title `⬡ MachineName — Diagram`.** Shipped: initial
  title `⬡ <basename> — Diagram` (diagramPanel.ts:263), re-titled to
  `⬡ <Machine> — Diagram` on the first successful render
  (diagramPanel.ts:166-169). ✅ Matches §1.5.1 once a machine resolves;
  the pre-first-render basename title is a sound honest default (not
  drift — there is no machine name to show until the IR parses, and a
  blank/"unknown" title would be worse).
- **§1.5.1:269 editor-title icon `$(type-hierarchy)`.** Shipped
  package.json command `icon: "$(type-hierarchy)"` (§1.4 / package.json
  delta). ✅ **VERBATIM.**

### 3.B Doc 27 §8-V4 / §6.2 — CSP-locked webview, elkjs, click→source

- **CSP / `postMessage`-only / no remote (§6.2:400-410, §8-V4:577).**
  Shipped `html()` (diagramPanel.ts:303-358): `default-src 'none'`;
  `script-src 'nonce-${nonce}'` + `style-src 'nonce-${nonce}'`;
  `img-src ${webview.cspSource} data:`; the ONLY script is the nonce'd
  bundled `dist/webview/diagramWebview.js` via `asWebviewUri`;
  `localResourceRoots` pinned to `dist/webview` (diagramPanel.ts:268-272);
  a cryptographic-ish per-render 32-char nonce (diagramPanel.ts:362-370).
  The Webview↔ext boundary is a tight typed `postMessage` JSON pair
  (diagramPanel.ts:44-56 `WebviewToExt`/`ExtToWebview`). ✅ **Faithful to
  §6.2/§8-V4 — strict CSP, no remote, postMessage-only.** (Nit, NOT a
  drift / NOT actionable: `makeNonce()` uses `Math.random()`, not
  `crypto.randomBytes` — for a CSP nonce on a *local* `vscode-webview://`
  bundle with `default-src 'none'` and no remote/inline-injection vector
  this is adequate; it does not weaken the V4 security contract Doc 27
  §8-V4 states. Recorded here for completeness, NOT catalogued — not a
  V5 concern, not a ship issue.)
- **elkjs / ELK Layered in-Webview (§6.2:388-391, Doc 05 §1.5.9:410).**
  Shipped: `diagramWebview.ts:29` imports `elkjs/lib/elk.bundled.js`,
  `:119-141` runs `elk.layered` (`elk.algorithm:"layered"`,
  `direction:"DOWN"`) — layout is purely client-side, the server/CLI
  never involved (§6.2:391). `esbuild.mjs:41-51` bundles elkjs INTO the
  browser IIFE (`platform:"browser"`, `format:"iife"`) so the CSP
  `default-src 'none'` holds (no node_modules / no remote at runtime).
  Per the established context elkjs@0.11.1 is lockfile-pristine
  (1 entry, zero transitives, webview-bundle-only — re-confirmed
  package.json:261 `"elkjs": "^0.11.1"`). ✅ **Faithful — ELK Layered,
  bundled, CSP-safe.**
- **Click→source via the IR `SourceLocation` (§6.2:402-403, Doc 05
  §1.5.4).** Shipped: `irGraph.ts:30-37` carries the 1-based
  `IrSourceLocation` per node; `diagramWebview.ts:229-235` posts
  `revealSource{line,column}` on node click; `diagramPanel.ts:192-210`
  maps 1-based→0-based and `editor.revealRange`. ✅ **Faithful — click →
  declaration via the authoritative IR location, the same discipline V5
  must mirror with the `documentSymbol` range (§1.3.4).**
- **Read-only scope (§6.2:392-399, Doc 27 §9).** Shipped: the Webview
  only renders + posts `revealSource`/`rendered`/`staleShown`/`ready`
  (diagramWebview.ts:250-302); no edit/drag/diagram→source path exists.
  ✅ **Read-only — interactive editing + simulator overlay correctly
  ABSENT (Doc 27 §9 OUT-scope honoured).**
- **IR→graph purity (§6.3:414-415, Doc 27 §6.3).** `irGraph.ts` is a
  pure `IR-JSON → {nodes,edges,regions}` function with NO `vscode`/
  `elkjs`/DOM imports (irGraph.ts:1-29 WHY-comment + verified imports) —
  the un-drift-able core the V4 test asserts the rendered model against
  (diagram.test.ts:339-358). A malformed/empty IR throws `IrGraphError`
  (irGraph.ts:235-296), never a silent blank (the cardinal-sin bar at
  the model boundary). ✅ **Faithful to §6.3 — the diagram cannot drift
  from the canonical IR; the structural assertion is real, not
  symbol-presence.**

### 3.C Merged `package.json` vs Doc 22 — union integrity + the V5 disjoint surface

- **V4's manifest additions are exactly the §1.4 set + the V3-reserved
  slots; nothing else.** `git diff 61cc3f0..4c4c6a2 -- package.json`:
  the `fsm.openDiagram` command/keybinding/`editor/title`/`editor/context
  fsm@1`/`commandPalette` entries + `"elkjs":"^0.11.1"` (dependencies) +
  the `typecheck`/`pretest` scripts gaining `tsc -p ./tsconfig.webview
  .json`. **Zero touch to `contributes.{languages,grammars,snippets,
  configuration}`** (V4 commit message asserts it; verified — the
  configuration block is byte-identical to the V1 5-key set, JC-3
  unchanged).
- **The V5 surface is VIRGIN — fully disjoint (C-3).** `grep -niE
  'viewsContainers|"views"|viewsWelcome|setContext|fsm\.hasOpenFsmFile|
  fsm\.serverRunning|TreeDataProvider|machineExplorer|eventExplorer|
  createTreeView|registerTreeDataProvider'` over **both**
  `editors/vscode/package.json` AND `editors/vscode/src/` returns **zero
  hits**. V5 owns exactly Doc 22 §7 (`viewsContainers.activitybar`
  `fsm-explorer` + `views.fsm-explorer`
  `fsm.machineExplorer`/`fsm.eventExplorer` `when:fsm.hasOpenFsmFile`,
  Doc 22:278-300) + Doc 22 §11 context keys (Doc 22:577-582;
  `fsm.simulatorConnected` is OUT — Doc 27 §9 no simulator) + the tree
  menus + `viewsWelcome` — a region **disjoint from every V1-V4 manifest
  key**. **No collision risk.** ✅
- **`activationEvents` (package.json:27-32) unchanged by V4** = V1's two
  + V3's two `onCommand:`. V4's `fsm.openDiagram` correctly relies on the
  existing `onLanguage:fsm-lang`/`workspaceContains:**/*.fsm` activation
  (the command is `editorLangId==fsm-lang`-gated; the language is already
  active when a `.fsm` is open) — no new activation event needed, none
  added. ✅ V5 will need `onView:fsm.machineExplorer`/`onView:fsm
  .eventExplorer` (or rely on the view-container activation) — a V5
  briefing detail, NOT a V4 gap.
- **`engines`/`dependencies` integrity.** `engines` (`vscode ^1.85.0`,
  `node >=20`) byte-unchanged; `dependencies` gains exactly
  `"elkjs":"^0.11.1"` (package.json:261, the single sanctioned new
  runtime dep, Doc 27 §6.2 / Doc 28 §3-V4; webview-bundle-only per
  esbuild.mjs:41-51). ✅ Additive + sanctioned.

**Lens-3 verdict: no NEW code-level drift.** The Doc 05 §1.5.1
"per machine" vs file-path-identity is the *intended* JC-3 correction
(shipped code is the right realization; the inline `_note` is the durable
correction-of-record; the Doc 05 clarification folds at closeout) — not a
new defect. CSP/elkjs/click→source/read-only/IR-purity are all faithful
to Doc 27 §6.2/§6.3/§8-V4. The only actionable-at-closeout new items are
the V4 JC-3 Doc-05 clarification + the (already-catalogued, carried)
JC-1/JC-3; the `Math.random` nonce is a recorded non-actionable nit
(adequate for the local-bundle CSP, not a contract weakening).

---

## 4. Lens 4 — V5-readiness verdict

**PROCEED-WITH-NOTES.** The V4 phase boundary is clean enough to dispatch
V5 (activity-bar tree views + context-key chrome) per Doc 28's cadence.
Reasons:

1. **Zero Rust delta across the whole W0→V4 batch; cargo quad
   structurally safe.** `git diff ceb8efd 4c4c6a2 -- crates/ Cargo.toml
   Cargo.lock rust-toolchain.toml` is **empty** (re-confirmed from `git`
   this audit); the entire V4 commit delta (14 files / 2503 ins) is under
   `editors/vscode/` + the V2/V3 audit doc. V5 (also pure
   `editors/vscode/`) cannot regress the workspace the §11.1 / W0-clean
   certification covers.
2. **The V5 data-source seam is RESOLVED, not open (§1).** The one real
   path — the **free V1-client `documentSymbol`** (`server.rs:251`, the
   same single-analysis seam `document_symbol.rs:61` /
   `analysis.rs:49,53,83`) — is shipped and auto-registered by V1's
   un-overridden client (extension.ts:234-244). V4's IR path is the
   *wrong* seam for the tree (re-imports the K-3 codegen-gated DRIFT into
   chrome + wrong shape); the brief MUST forbid it (MV5-1, the symmetric
   foot-gun to V1's N-1 / V4's ask-the-LSP-for-the-IR). V5 builds the
   tree on a proven free seam — **no rework**.
3. **V4 is strictly disjoint + additive (§1.4/§3).** One command +
   exactly the V3-reserved manifest slots; the only V1 touch is the
   additive `registerOpenDiagram` call (extension.ts:145-150); M-1 is
   byte-preserved (no `ServerOptions`, no `transport`, no `clientOptions`
   touch). The V5 manifest region (Doc 22 §7/§11) is grep-verified
   **entirely absent** (§3.C) — zero collision.
4. **The V4 codegen-gated DRIFT (K-3) is sealed, not just handled
   (§1.2/§3.A).** The audit-recommended `copyIr→emitIr` extraction
   shipped (one honest seam, `copyIR` pre/post-identical, the ONLY
   `--emit-ir` site); the boundary is the proven Webview analogue of V3
   `copyIr.ts:91-104` (last-valid RETAINED + the Doc 05 §1.5.9 banner
   VERBATIM, asserted at the real Webview ack — diagram.test.ts:368-526).
   No DRIFT escapes into V5.
5. **No NEW code-level drift (§3)** — Doc 05 §1.5.1 "per machine" vs
   file-path-identity is the intended JC-3 correction (shipped code
   right; Doc-05 clarification folds at closeout); CSP/elkjs/click→
   source/read-only/IR-purity are faithful to Doc 27 §6.2/§6.3/§8-V4.

**Mandatory briefing-inputs for V5 (not code/doc edits):**
- **MV5-1 (C-2 — the V5 data-source contract):** the V5 brief MUST pin
  §1.3: data-source = the **free V1-client `documentSymbol`** seam
  (`vscode.executeDocumentSymbolProvider`, `server.rs:251`), explicitly
  **NOT** V4's `emitIr.ts`/`irGraph.ts` IR path (re-imports the K-3
  codegen-gated DRIFT into chrome + is the wrong shape — the symmetric
  Doc-26-§4.5 wrong-seam trap) and **NOT** a new server/LSP method
  (R-9/R-10, the Doc 26 §4.5/§9 "do not smuggle a subsystem" rule); the
  **tree-fidelity = the Machines tree's nodes equal the client's
  `executeDocumentSymbolProvider` result EXACTLY, re-projected not
  re-analyzed, proven by an Extension-Host test** ("a tree appeared" /
  "a provider is registered" is explicitly NOT acceptance — the P0-1
  bar, the exact discipline the V4 gate enforced); the context keys
  (`fsm.hasOpenFsmFile`/`fsm.serverRunning`) driven by VS Code state /
  the V1 client lifecycle (`onDidChangeState`, extension.ts:248-261 —
  NOT a new analysis); **R-15 fixture isolation inherited** (no
  `fsm.toml [compiler]` in scope, the `diagram.test.ts:177-195` /
  `extension.test.ts:115-125` pattern).
- **MV5-2 (M-1 lineage, F-1+F-2/N-1 carry):** the V5 brief MUST carry the
  "never re-add `transport: TransportKind.stdio`, never switch to a
  `NodeModule` server shape" constraint (V5 touches no client options,
  but this binds every `editors/vscode/` wave; cite `AUDIT_PHASE_V1`
  §3.A + this audit §1.4).
- **MV5-3 (the disjoint manifest region V5 owns — C-3):** the V5 brief
  MUST scope V5 to exactly `contributes.viewsContainers.activitybar`
  (`fsm-explorer`, Doc 22 §7:278-285), `contributes.views.fsm-explorer`
  (`fsm.machineExplorer`/`fsm.eventExplorer`,
  `when:fsm.hasOpenFsmFile`, Doc 22 §7:287-300), Doc 22 §11 context keys
  via `setContext` (`fsm.hasOpenFsmFile`/`fsm.serverRunning`;
  `fsm.simulatorConnected` is OUT — Doc 27 §9 no simulator), the tree
  `view/title` (Doc 05 §1.3.2 refresh/new-file) + `view/item/context`
  (Doc 05 §1.3.6) menus, and `contributes.viewsWelcome` for the
  no-`.fsm`-open state — a region **disjoint from every V1-V4 manifest
  key** (§3.C grep-verified absent). It MUST ALSO **fold N-4** (the
  status-bar restarting-tooltip polish, Doc 22:679) into V5's Doc 22 §10
  status-bar machine-indicator work (Doc 27 §8-V5:599 — V5 *is* the
  status-bar wave; this is N-4's natural home).

**Deferrable-to-v1.3-closeout (the batched-doc / `GATE_VERIFICATION_v1_3`
pass — the §2 catalogue):** F-1+F-2/N-1, N-2, N-3 doc edits; **N-4**
(now a V5 fold-in, MV5-3); N-6 → Doc 21 §3 (J-1 ratified); the
Doc-22-§4-title re-pin (N-3 class, V4 confirms the convention); **JC-3**
(→ config-owner wave); **JC-1** (→ JS CI-lane + gate-doc, now
non-optional post-V4 Webview); **V4 JC-3** (→ Doc 05 §1.5.1/§1.5.9
clarification). **CLOSED — record only:** **N-5** (closed by V3).
**POSITIVE — closeout credits, no action:** the V4 `copyIr→emitIr`
extraction; V4 JC-2 (sound codegen-ICE test strategy); V4 JC-4 (strict
Webview tsconfig).

**V5 is unblocked once MV5-1/MV5-2/MV5-3 are in its brief.** No code or
doc edit is a precondition to dispatching V5. Per Doc 28 §3 the two
Doc-28-mandated phase-boundary audits (SUBAGENT §11.3, after **V1** and
after **V4**) are now BOTH complete; V5 (chrome reusing the proven free
`documentSymbol`) and V6 (the G9-infra-gated multi-platform tail) follow
without a further mandated phase-boundary audit — though the v1.3-closeout
pre-tag gate (`GATE_VERIFICATION_v1_3`) folds this §2 catalogue and
re-runs the §11.1 combined gate.

---

## 5. Audit metadata

- **Audit type:** SUBAGENT §11.3 post-V4 phase-boundary (the Doc 28
  `V4 → audit → V5` gate — the second + last Doc-28-mandated
  phase-boundary audit). READ-ONLY judgment. **ZERO build**
  (`cargo`/`npm`/`tsc`/`esbuild` not invoked).
- **HEAD audited:** `4c4c6a2` (`main`). Worktree:
  `/root/dev/embeded-fsm-sdk-wt-v4audit`, branch
  `phase3.2/v1_3-v4-audit`.
- **Ancestry re-confirmed from `git`:** `git log --oneline 4c4c6a2`:
  `ceb8efd` (W0-clean) → `6707d1d` (V1) → `1542bfb` (V2) → `3c49d38`
  (V3) → `61cc3f0` (V2/V3 merge) → `4aaef5d` (V2/V3 audit doc) →
  `4c4c6a2` (V4). Linear (no merge — V4 was a direct commit on `main`
  after the V2/V3 audit, per Doc 28's post-audit-then-V4 cadence).
- **Byte-identity re-confirmed:** `git diff ceb8efd 4c4c6a2 --
  crates/ Cargo.toml Cargo.lock rust-toolchain.toml` → **empty** (Rust
  workspace == W0-clean `ceb8efd`); full V4 commit delta
  (`61cc3f0..4c4c6a2`) = 14 files / 2503 ins / 109 del, all
  `editors/vscode/` + the V2/V3 audit doc.
- **Independent re-derivations performed (not trusted from context):**
  the V5 seam — `server.rs:251` (`document_symbol_provider:
  Some(OneOf::Left(true))`), `capabilities/document_symbol.rs:61-66`
  (the Doc 14 §13 hierarchical projection), `analysis.rs:49,53,83`
  (the single-analysis "no second analysis" seam), `extension.ts:234-244`
  (the un-overridden `LanguageClient` → `documentSymbol` auto-registered
  free); V4 disjointness — `extension.ts:145-150` (additive
  `registerOpenDiagram`), `git diff 61cc3f0..4c4c6a2 -- extension.ts`
  (+17, import+call only), the package.json delta (the `fsm.openDiagram`
  command/keybinding/menu slots = exactly the V3-reserved set); the
  extraction — `git diff 3c49d38..4c4c6a2 -- copyIr.ts` (pure
  inline→shared-call), `emitIr.ts:86-175` + the `grep` proving it the
  ONLY `--emit-ir` site, `copyIr.ts:54-91` (the exact `ok`/`codegenFailed`/
  `noCli`/`spawnError` mapping); the codegen-gated boundary —
  `emitIr.ts:136-148`, `diagramPanel.ts:41-42,118-175`,
  `diagramWebview.ts:282-298`, `diagram.test.ts:368-537`; the
  panel-identity JC-3 — `diagramPanel.ts:58-75,220-256` (file-path key)
  vs Doc 05 `§1.5.1:272`; CSP — `diagramPanel.ts:303-370`; elkjs —
  `diagramWebview.ts:29,119-141` + `esbuild.mjs:41-51` + package.json:261;
  the V5-surface absence — `grep` over package.json + `src/` (zero hits).
- **Toolchain probe (trap-aware, not needed but asserted):** the brief's
  established context fixes the 1.75.0 pin; **no `rustc` shelled** (zero
  build). Per `feedback_embeded_fsm_toolchain_probe_trap`, had a probe
  been needed: `sh -c 'cd /root/dev/embeded-fsm-sdk && rustup show
  active-toolchain'` (=1.75.0 overridden); a bare `rustc` value
  (1.75 or 1.95 — it fluctuates) is the benign box default, NOT pin
  drift.
- **Verdict:** **PROCEED-WITH-NOTES** — V5 dispatchable; MV5-1 (the V5
  data-source contract = the free `documentSymbol`, NOT V4's IR path),
  MV5-2 (M-1 lineage), MV5-3 (the disjoint Doc 22 §7/§11 manifest region
  V5 owns + the N-4 status-bar fold-in) are mandatory briefing inputs;
  the §2 catalogue folds at v1.3-closeout (N-5 closed; N-6/Doc-22-§4/V4
  JC-3 corrected-in-shipped; JC-3/JC-1 the only genuinely-open carried
  items, both deferrable; the V4 `copyIr→emitIr` extraction + JC-2 + JC-4
  are POSITIVE closeout credits).
- **Pattern:** mirrors `AUDIT_PHASE_W0_2026_05_16.md` /
  `AUDIT_PHASE_V1_2026_05_16.md` / `AUDIT_PHASE_V2V3_2026_05_16.md`
  (verdict TL;DR → Lens-1 next-wave-seam keystone analysis → Lens-2
  canonical accumulated catalogue → Lens-3 doc-vs-shipped sweep →
  Lens-4 readiness verdict + required orchestrator/briefing actions).
  §1+§2 are the durable correction-of-record until the batched-doc
  closeout (the W0/V1/V2V3 §11.3 / N-1 precedent).

*End of AUDIT_PHASE_V4_2026_05_16*
