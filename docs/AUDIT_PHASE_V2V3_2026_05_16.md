# AUDIT — §11.3 Post-V2/V3-Batch Phase-Boundary Audit (v1.3-V2 grammar/snippets + v1.3-V3 CLI-wrapper/commands)

- **Scope:** the Doc-28-mandated `V2/V3 → audit → V4` gate (FSM-PROC-SUBAGENT
  §11.3 / Doc 28 §3 "Phase-boundary audits (SUBAGENT §11.3) after **V1**
  (the spine) and after **V4** (the new subsystem)" — and the orchestrator's
  cadence of auditing the V2/V3 integration batch before the most
  substantive remaining subsystem, V4 the diagram WebviewPanel, where the §6
  codegen-gated-IR DRIFT lives).
- **Subject:** the merged `editors/vscode/` at `main` HEAD **`61cc3f0`**
  ("Merge phase3.2/v1_3-v3-impl (V3: CLI-wrapper + client-control commands +
  N-5 closure)"). Merge parents: `1542bfb` (V2) + `3c49d38` (V3); merge-base
  ancestor `6707d1d` (V1).
- **Method:** READ-ONLY judgment audit. **ZERO** `cargo`/`npm`/`tsc`/
  `esbuild` build. Derived by reading shipped source
  (`editors/vscode/src/**`, `editors/vscode/syntaxes/`,
  `editors/vscode/package.json`) + `git -C` + `grep`, cross-read against
  Docs 27/28/22/21/05 and the prior `AUDIT_PHASE_V1_2026_05_16.md`.
- **Established context (NOT re-litigated — already independently verified
  per the brief):** V1/V2/V3 all merged → `61cc3f0` (clean 3-parent-chain
  merge; manifest union validated); the §11.1 combined gate returned
  COMBINED EXTENSION-HOST GATE GREEN @ `61cc3f0` with both load-bearing
  oracles independently recomputed + byte-matched, 20/20 coexisting, zero
  cross-wave regression, zero Rust delta vs W0-clean `ceb8efd`. This audit
  **re-confirms the byte-identity and the merge structure from `git`
  itself** (verify-the-record, §5), then judges V2/V3-batch coherence for V4
  and pins the V4 IR-data-source contract — it does **not** re-run the
  §11.1 gate.

---

## 0. Verdict (TL;DR)

**V4-READINESS: PROCEED-WITH-NOTES.**

The phase boundary is **clean enough to dispatch V4** (the diagram
WebviewPanel) per Doc 28's cadence. The V2/V3 batch is a correctly-scoped,
disjoint, additive pair that lands on V1's proven spine without rework, and
— critically — **the one real IR data path V4 must consume already exists,
already exercised, and already handles its DRIFT-class boundary in V3's
`copyIr.ts`/`cliRunner.ts`**:

- **The Rust workspace is byte-untouched across the whole W0→V1→V2→V3 batch.**
  `git diff ceb8efd 61cc3f0 -- crates/ Cargo.toml Cargo.lock
  rust-toolchain.toml` is **empty**; the full `61cc3f0` delta is 38 files /
  9136 insertions, **all under `editors/vscode/`**. The cargo quad is
  structurally incapable of regressing from this commit; V4 (also pure
  `editors/vscode/`) inherits that safety.
- **The V4 IR-data-source seam is RESOLVED, not assumed (Lens 1, §1 — the
  decisive finding).** Doc 28 K-3 / Doc 27 §6 flag the DRIFT: there is **no
  LSP `fsm/diagram`/`fsm/ir` method and no `fsm ir` subcommand**; the **only**
  IR-JSON path is the **codegen-gated `fsm generate --emit-ir`**. V3's
  `copyIr.ts` **already implements exactly that path** (generate to a temp
  dir, locate `*.ir.json`, surface the codegen-gated boundary honestly —
  copyIr.ts:54-134) over a centralised honest `runCli` seam
  (cliRunner.ts:37-76). **V4 MUST consume the V3 `cliBinary.ts` +
  `cliRunner.ts` seam (the reusable substrate) and MUST NOT introduce a
  server method** — and its R-11 boundary acceptance is *already
  demonstrated* by V3's `copyIr.ts` codegen-fail handling (the named V4
  contract is `last-valid-render + Doc 05 §1.5.9 banner`, the §1.3 pin).
- **V2/V3 are strictly disjoint and additive on V1 (§3).** V3 contributes
  exactly its 7 commands + their menus/keybindings/`commandPalette` gating;
  V2 contributes only `languages[].configuration`/`grammars`/`snippets`;
  the V1 client-spawn (`extension.ts` `Executable`, transport-omitted) is
  **byte-identical** and the only V1 touch is the additive
  `registerCommands(...)` call site (extension.ts:128-133). M-1 (the
  transport foot-gun) is honoured by construction — V3 restarts via V1's
  unchanged `restartServer()`/`client.restart()` and **never constructs a
  `ServerOptions`** (clientControl.ts:8-16, index.ts:17-27).
- **The N-5 forward-dependency the V1 audit flagged is CLOSED by V3**
  (§2): `fsm.showOutputChannel` + `fsm.restartLanguageServer` are now
  contributed **and** registered (package.json:95-104, clientControl.ts) —
  V1's status-bar click + crash-exhaustion buttons are now functional.

The **NOTES** (none are V4 ship-blockers; classified by when each must
fold):

- **C-1 (CANONICAL ACCUMULATED DEFERRED-BATCH CATALOGUE — the
  correction-of-record consolidation):** §2 is the single consolidated
  list the orchestrator folds at the v1.3-closeout batched-doc pass into
  `GATE_VERIFICATION_v1_3` / Doc 00 §11.N. It carries forward Findings 1+2 /
  N-1..N-4 (from `AUDIT_PHASE_V1_2026_05_16.md`), records **N-5 CLOSED by
  V3** and **N-6 corrected-in-shipped by V2**, and adds the V2/V3-batch
  items **JC-3** (config-discoverability) and **JC-1** (xvfb/DISPLAY CI
  environmental). Per the W0/V1 §11.3 pattern **this audit doc is the
  durable correction-of-record** until that batched pass. **Mandatory
  before V4 is not the doc edits but that the V4 brief cites §1 and §2 of
  THIS audit, not the stale Doc 27 §6 / Doc 05 §1.5.9-only prose.**
- **C-2 (the V4 IR-data-source contract — MANDATORY-BEFORE-V4 as a briefing
  input):** §1.3 is the exact V4 acceptance contract. The V4 brief MUST
  pin: data source = the **V3 `cliBinary.ts`+`cliRunner.ts` seam running
  `fsm generate --emit-ir` to a temp dir** (NOT a new LSP/server method —
  the R-9/R-10 prohibition, the Doc 26 §4.5 "do not smuggle a subsystem"
  bar); the **codegen-gated boundary handled exactly like V3's `copyIr.ts`
  R-11 path** (parse-OK-but-codegen-fails ⇒ NO `.ir.json` ⇒ **keep the
  last-valid render + show the Doc 05 §1.5.9 `⚠ Diagram shows last valid
  state. Fix parse errors to update.` banner**, proven by an
  Extension-Host assertion, **not assumed**).
- **JC-3 (DRIFT — config-discoverability, deferrable-to-closeout, a later
  config-owner wave):** `fsmLang.codegen.outputDir` / `codegen.strategy`
  are read by V3's `generate.ts` with the Doc-22-§8 defaults
  (`"generated"` / `"switch"` — generate.ts:34-57) but are **NOT** in
  `contributes.configuration.properties` (package.json:180-207 ships only
  the 5 V1 keys). Same class as the Doc 27 §10 / Doc 28 R-6..R-8 / K-6
  config-drift ledger, now with two *extension-consumed* (not server-dead)
  keys silently un-discoverable in the Settings UI. **Not a V4 blocker**
  (V4 consumes `--emit-ir` to a temp dir with a fixed `--target c99`, not
  `codegen.outputDir`/`strategy` — copyIr.ts:58-66) and not a correctness
  bug (the defaults match Doc 22 §8). Fold into the v1.3-closeout
  config-owner pass alongside the R-6..R-8 ledger.
- **JC-1 (ENVIRONMENTAL — CI lane, not code, deferrable-to-closeout):**
  `@vscode/test-electron` (package.json:225) launches a real Electron
  Extension Host → requires a DISPLAY; the CI JS-lane + every future
  §11.1 re-run MUST invoke the suite under `xvfb-run`. This is the Doc 28
  §2.3 Option-A JS-lane operational detail, **not a code defect** and not
  in this audit's `editors/vscode/` review surface — recorded in the
  catalogue so the v1.3-closeout CI-lane wiring + `GATE_VERIFICATION_v1_3`
  pre-tag JS-quad transcript carry it.

This audit deliberately shows the **V4-data-source seam analysis** (§1, the
keystone) and the **doc-vs-shipped spot-checks** (§3, Doc 22 §4/§5/§6 + the
V2 grammar vs Doc 21 §2) because "an audit that finds nothing on the
integration-keystone batch that already surfaced J-1/JC-1/JC-3/JC-7 is
suspect" — the V4 seam *is* traced to file:line, the V2/V3 doc-vs-shipped
deltas *are* enumerated, and the surviving drift (JC-3) is named with the
exact fold-target rather than papered over.

---

## 1. Lens 1 — Combined-extension coherence for V4 (the V4 IR-data-source seam)

V4 per Doc 27 §6 / Doc 28 §3-V4 is the diagram `WebviewPanel`: `fsm.openDiagram`
→ split-right webview, **data = `fsm generate --emit-ir` →
`<machine>.ir.json` → `postMessage`**, ELK-laid-out in-webview, read-only.
It is "the single substantive new subsystem and the place an unverified
data-source assumption is exactly the Doc-26-§4.5 prose-vs-code trap"
(Doc 27 §6). The keystone question is therefore not "does V2/V3 compile"
(the §11.1 gate covered green) but **"is the IR data path V4 needs real,
already-built, and does V3 give V4 a reusable seam — or must V4 re-derive
it (and risk the no-LSP-method DRIFT)?"**

### 1.1 The decisive finding — the ONLY IR path exists, and V3 already exercises it

Doc 28 K-3 / Doc 27 §6.1 / R-9 / R-10 / R-11 establish (verified there
against shipped Rust) that **(a)** there is **no** LSP `fsm/diagram` or
`fsm/ir` request and no `experimental` capability; **(b)** `fsm ir`
(Doc 18) is **not** implemented (`cli.rs` `Command` enum has no `Ir`); and
**(c)** the **one** working IR-JSON path is the **codegen-gated**
`fsm generate --emit-ir` writing `<first-machine>.ir.json` via
`fsm_ir::to_json`, **only after** the generated-C `fs::write` succeeds
(`crates/fsm-cli/src/cmd/generate.rs:189-209`). The brief instructs me to
**confirm V4's data-source MUST reuse exactly that one real path** and to
**state whether V3's `copyIr.ts`/`cliRunner.ts` is the reusable seam**.

**It is — and it is already correct, by reading:**

- **`copyIr.ts` IS the `fsm generate --emit-ir` path, end to end.**
  copyIr.ts:54-72: `fs.mkdtempSync(... "fsm-copyir-")` → an **isolated
  temp dir** (Doc 27 §6.2's "the extension runs `fsm generate --emit-ir` …
  for the active file to a temp dir, reads `<machine>.ir.json`" — the
  V4 data-source decision, *verbatim*); args =
  `["generate","--target","c99","--out",tmpDir,"--emit-ir",fsmPath]`
  (copyIr.ts:58-66); runs it through the shared `runCli`
  (copyIr.ts:70-72); locates `*.ir.json` by suffix and reads it
  (copyIr.ts:108-123). **This is exactly the byte-for-byte producer V4's
  diagram model must consume.** V4's "open the diagram" is structurally
  *the same call* as `copyIR` minus "write to clipboard" plus "parse the
  JSON → ELK → postMessage".
- **The R-11 codegen-gated boundary is ALREADY handled honestly
  (copyIr.ts:88-120) — this is the V4 DRIFT-class assertion, pre-proven.**
  copyIr.ts:91-104: `if (res.code !== 0)` → `errorWithLog("… cannot copy
  IR — codegen failed, so no IR was produced (the IR is codegen-gated).
  …")` and **`return` — clipboard NOT mutated**. copyIr.ts:111-120: the
  defensive "exit 0 but no `.ir.json`" arm *also* refuses to fake output.
  The load-bearing comment (copyIr.ts:6-16) cites
  `crates/fsm-cli/src/cmd/generate.rs:165-209` and states the exact R-11
  contract. **V4's required boundary (Doc 27 §6.2 / Doc 05 §1.5.9: on a
  codegen failure keep the last valid render + show the banner) is the
  Webview analogue of this exact branch** — V3 has already demonstrated
  the extension-side handling is sound; V4 changes only the *consequence*
  (last-valid-render-retained + banner, instead of clipboard-untouched).
- **`cliRunner.ts` is the reusable honest-process seam.** cliRunner.ts:37-76:
  `runCli` resolves (never rejects) with `{code,stdout,stderr,spawnError}`;
  ENOENT → `spawnError:true` (`:48-55`); a non-zero exit is **data, not a
  masked success** (`:57-68`, the cardinal-sin bar enforced in one place —
  the WHY-comment cliRunner.ts:1-10). `maxBuffer: 32 MiB` (cliRunner.ts:46)
  is adequate headroom for an IR-JSON dump. **V4 should consume `runCli`
  unchanged.**
- **`cliBinary.ts` is the reusable `fsm`-CLI resolver V4 needs (NOT V1's
  `serverBinary.ts`).** cliBinary.ts:73-108 resolves the **`fsm` CLI**
  (Rule 1: the sibling of `fsmLang.compilerPath`'s server binary; Rule 2:
  bundled `bin/<triple>/fsm`; Rule 3: `undefined` → caller emits the
  verbatim `noCliBinaryMessage` and stays inert — **no silent
  PATH/guess**, cliBinary.ts:46-52,106-107). The WHY-header
  (cliBinary.ts:1-19) correctly cites Doc 27 §2.2's *genuine two-binary
  need*: the diagram path needs the **`fsm` CLI** (codegen), not
  `fsm-lang-server`. V4 needs precisely this resolver — **it must reuse
  `cliBinary.ts`, not re-implement a third resolver and not mis-reach for
  the server binary.**

### 1.2 Seam verdict — V4 consumes the V3 seam; it does NOT need its own

**The reusable V4 substrate is, exactly:** `resolveCliBinary` (cliBinary.ts)
→ `runCli` (cliRunner.ts) → `fsm generate --target c99 --out <tmp>
--emit-ir <file>` → read `<tmp>/*.ir.json` → (V4-new) `JSON.parse` → ELK →
`postMessage`. **V4 must NOT build its own IR acquisition.** Building one
would (a) duplicate the cardinal-sin honest-`runCli` logic (drift risk),
(b) risk the R-9/R-10 DRIFT (a wave that "just asks the LSP for the IR"
fabricates a non-existent method — the precise Doc-26-§4.5 trap Doc 27 §6.3
exists to prevent), and (c) re-introduce the binary-resolution-fallback
foot-gun V3 already closed. The single legitimate V4-new code is the
**Webview** (CSP-locked `postMessage`, elkjs layout, SVG/PNG, click→source
via the IR `SourceLocation`) and a **thin orchestration wrapper** that, vs
`copyIr.ts`, swaps "write clipboard / refuse on codegen-fail" for
"postMessage new model / **retain last-valid render + show banner** on
codegen-fail". A small refactor extracting the shared "generate-IR-to-temp
→ locate `*.ir.json` → read" core out of `copyIr.ts` into a reusable
`emitIr(fsmPath): {json}|{codegenFailed,detail}` helper (consumed by both
`copyIR` and V4) is the clean move — **recommended for the V4 brief**, but
note it makes V4 touch `copyIr.ts` (a V3-owned file): that is acceptable
*shared-substrate extraction* (not scope-creep) **iff** V4's behavioural
gate re-asserts `copyIR` is unregressed (the SUBAGENT §10 refactor
pre/post-identity bar). If the orchestrator prefers strict file-disjointness
for V4, V4 may instead *call the existing `cliBinary`/`cliRunner` seam
directly* and accept a small, deliberate, commented duplication of the
~6-line "locate-and-read `*.ir.json`" block — **stating which choice in the
V4 brief is itself a C-2 briefing input.**

### 1.3 The V4 acceptance contract (C-2 — the exact pin for the V4 brief)

Per Doc 28 §3-V4 + Doc 27 §6.2 + Doc 05 §1.5.9, **proven not assumed**:

1. **Data source (HARD):** `fsm generate --emit-ir` via the **V3
   `cliBinary.ts`+`cliRunner.ts` seam** to a temp dir; parse
   `<tmp>/*.ir.json` (the `fsm_ir::to_json` schema, `crates/fsm-ir`).
   **NO new server/LSP method, NO `fsm ir` subcommand** (R-9/R-10; the
   Doc 26 §4.5/§9 "separate self-contained epic, do not smuggle" bar — a
   V4 that adds an `fsm/diagram` request is rejected back).
2. **Codegen-gated boundary (the DRIFT-class assertion — MANDATORY
   Extension-Host test):** feed a fixture that **parses/analyzes clean but
   a codegen edge rejects** → assert the Webview **keeps the last valid
   render AND shows the Doc 05 §1.5.9 banner verbatim**
   (`⚠ Diagram shows last valid state. Fix parse errors to update.`) —
   the exact analogue of V3 `copyIr.ts:91-104` refusing to fake output,
   surfaced as last-valid-retention rather than clipboard-untouched. **A
   V4 that asserts only "a webview opened" is incomplete and rejected**
   (the §5.4 / P0-1 bar; Doc 28 §3-V4 "the DRIFT-class assertion, proven
   not assumed").
3. **Structural fidelity:** drive the Webview, assert the rendered model
   has **the exact state/transition set of the fixture's `--emit-ir`
   JSON** (parse the *same* artifact as the oracle — a real structural
   assertion, not symbol-presence); click a rendered state → editor
   selection moves to that state's declaration line via the IR
   `SourceLocation`; SVG export → well-formed SVG with the state labels
   (Doc 28 §3-V4 gate, verbatim).
4. **R-15 inheritance:** the V4 fixture, like V1's, MUST have **no
   `fsm.toml [compiler] allow/deny` in scope** — `--emit-ir` runs the
   CLI; an allow/deny fixture would project the diagnostics the CLI emits
   pre-IR and could mask a codegen failure differently than the oracle
   expects. Stage the V4 fixture in an OS temp dir with the up-tree
   `fsm.toml` walk hard-asserted empty (the `extension.test.ts:115-125`
   pattern V1 established — reuse it).

**Net (Lens 1):** the V4 data-source DRIFT (K-3) is **handled, not open**:
the one real path exists, V3 already drives it and already handles its R-11
boundary honestly, and the reusable seam (`cliBinary`+`cliRunner`, plus an
optional extracted `emitIr` core) is identified. V4 is architecturally
unblocked **provided C-2 is in its brief** (so a V4 implementer does not
"just ask the LSP for the IR" — the only structural foot-gun, the symmetric
analogue of V1's omit-transport N-1).

---

## 2. Lens 2 — Canonical accumulated deferred-batch catalogue (the correction-of-record)

This is the single consolidated list the orchestrator folds at the
v1.3-closeout batched-doc pass into `GATE_VERIFICATION_v1_3` / Doc 00
§11.N. Per the W0/V1 §11.3 pattern **this audit doc is the durable
correction-of-record until that pass.** Status legend: **OPEN** (needs
action), **CLOSED** (resolved in a shipped wave), **CORRECTED-IN-SHIPPED**
(shipped code already does the right thing; only doc prose is stale),
**ENVIRONMENTAL** (CI/infra, not code).

| # | Source | What | Status | Exact reconciliation action | Gate |
|---|---|---|---|---|---|
| **F-1+F-2 / N-1** | `AUDIT_PHASE_V1_2026_05_16.md` §3 (Doc 27 §2.2 transport; Doc 27 §2.3 + Doc 28 R-4 UTF-8) | (1) Doc 27 §2.2 `TransportKind.stdio` is a wrong literal — the only round-tripping form is **omit `transport`** on the `Executable`. (2) UTF-8 `positionEncoding` is **unreachable** through `vscode-languageclient` (v8/v9 hardcode `['utf-16']`, throw on non-UTF-16); the correct negotiated value is **`"utf-16"`**. | **OPEN (doc-of-record fix; shipped code already correct)** | Fold `AUDIT_PHASE_V1` §3.A into Doc 27 §2.2; §3.B into Doc 27 §2.3/§7-risk-5/§8-V1(c) **and** Doc 28 §2.2/R-4/§3-V1(c)/§5-V1(c). No code change implied. | **Briefing-input mandatory: the V4 brief inherits the §3.A constraint** (a webview wave touches no client options, but the M-1 "never re-add `transport`/`NodeModule`" rule rides on every future `editors/vscode/` wave). Doc edits deferrable-to-closeout. |
| **N-2** | `AUDIT_PHASE_V1` §0/§4 (Doc 28 §3-V1/§5-V1 clause (c)) | Doc 28 §3/§5 clause (c) textually mandates the FALSE assertion (`positionEncoding == UTF-8`). The shipped `extension.test.ts:194-206` **correctly asserts `"utf-16"`** — i.e. the gate already, correctly, diverges from its own brief. **Re-confirmed from source this audit** (`extension.test.ts:194` `test("(c) … (UTF-16)")`, `:206` `assert … negotiatedPositionEncoding`). | **OPEN (doc-of-record fix; the desired behaviour, recorded so a future "test ≠ brief" mechanical check does not mis-flag it)** | Fold the §3.B corrected-(c) text into Doc 28 §3-V1/§5-V1 clause (c). | Deferrable-to-closeout. Not a defect — explicitly the correct state. |
| **N-3** | `AUDIT_PHASE_V1` §0/§3.A | Doc 27 §2.2's `main.rs:39-48` citation conflates three arms. Correct: `:39-48` (`--port`), `:49-52` (unknown-arg exit-2), `:53-65` (`None → run_stdio`). | **OPEN (citation re-pin, non-load-bearing)** | Re-pin the Doc 27 §2.2 citation at closeout (text in `AUDIT_PHASE_V1` §3.A parenthetical). | Deferrable-to-closeout (doc hygiene). |
| **N-4** | `AUDIT_PHASE_V1` §4 (D-8; Doc 22 §10) | The status-bar "restarting" **tooltip** renders generic `"…starting"` not Doc 22:679's literal `"FSM Language Server restarting (attempt N/3)..."`. Inside the unexercised recovery path; icon/text already match. **Still present at `61cc3f0`** (V2/V3 did not touch `statusBar.ts`; V3 only *registered* the commands its click targets). | **OPEN (cosmetic; the V1-flagged V3/V5 deferral)** | Fold the tooltip-string polish into the V5 status-bar work (V5 adds the `fsm.currentMachine` second item + a natural recovery-state treatment). | Deferrable-to-closeout / V5. **Not** a V4 concern (V4 is the diagram, not the status bar). |
| **N-5** | `AUDIT_PHASE_V1` §4 (D-9) / §5 M-2 | V1 wired `statusBar.ts:74 → "fsm.showOutputChannel"` + the crash-exhaustion buttons to `fsm.restartLanguageServer`/`Show Log` but (correctly per scope) did **not** contribute/register them — inert until V3. | **CLOSED by V3** | None. **Verified closed this audit:** package.json:95-104 contributes both; `clientControl.ts` registers both (`fsm.restartLanguageServer` via V1's unchanged `restartServer()` — clientControl.ts:31-82; `fsm.showOutputChannel` → `outputChannel.show(true)` — clientControl.ts:91-100); `activationEvents` adds `onCommand:fsm.restartLanguageServer`/`onCommand:fsm.showOutputChannel` (package.json:30-31) so they work even before language activation. Record as the **closed N-5 item** in `GATE_VERIFICATION_v1_3`. | None — closed. |
| **N-6** | V2 grammar inline `_note` (tmLanguage.json:6) + the v1.3-V2 completion report (J-1) | Doc 21 §3's literal JSON declares `machine`/`state`/`composite`/`parallel`/`region` as single-line `match` rules with **no begin/end body rule**, so a structural `{ … }` body had no rule to descend into and the bare-`{` `#action-block` greedily swallowed the whole machine body. **Doc 21 §3 is structurally defective as written.** | **CORRECTED-IN-SHIPPED by V2 (J-1, requires reviewer/§11.1 ratification — already ratified per established context)** | The shipped grammar converts those five declarations to **begin/end block rules whose body re-includes the pattern set**, with **every Doc 21 §2 scope NAME preserved verbatim** (verified §3.D of this audit). Fold the §11.1-ratified J-1 correction into Doc 21 §3 at closeout (the grammar's inline `_note`, tmLanguage.json:6, is the durable correction-of-record meanwhile — the DRIFT-2 "fix-where-demonstrably-broken + explain" discipline; Doc 21 is a Status:Deferred DRAFT Doc 27 §4 "consumes"). | Deferrable-to-closeout. **Not** a V4 concern (grammar is passive to the diagram). |
| **JC-3 (V2/V3-batch, NEW)** | This audit §0 / §3.B | `fsmLang.codegen.outputDir`/`codegen.strategy` are **extension-consumed** by V3 `generate.ts:34-57` with the Doc-22-§8 defaults, but are **NOT** in `contributes.configuration.properties` (package.json:180-207 ships only the 5 V1 keys) → IDE Settings-UI discoverability gap (the user cannot discover/set them via the Settings GUI; only via raw `settings.json`). Same class as the Doc 27 §10 / Doc 28 R-6..R-8 / K-6 config ledger, but these two are *live extension inputs*, not server-dead. | **OPEN (a later config-owner wave)** | Add `fsmLang.codegen.outputDir` (string, default `"generated"`) + `fsmLang.codegen.strategy` (enum `["switch","table"]`, default `"switch"`) to `contributes.configuration.properties` per Doc 22 §8:371-384, in the v1.3-closeout config-owner pass that also reconciles the R-6..R-8 server-dead-key ledger. (Whether to *also* surface `codegen.defaultTarget`/`format.*` is that wave's decision — out of this audit's scope.) | **Deferrable-to-closeout.** NOT a V4 blocker: V4 calls `--emit-ir` with a fixed `--target c99` to a *temp* dir (copyIr.ts:58-66 pattern) — it does not read `codegen.outputDir`/`strategy`. Not a correctness bug (defaults == Doc 22 §8). |
| **JC-1 (V2/V3-batch, environmental)** | This audit §0 / established §11.1 context | `@vscode/test-electron` (package.json:225) launches a real Electron Host ⇒ needs DISPLAY; the CI JS-lane + **every** future §11.1 re-run MUST invoke the suite under `xvfb-run`/with a virtual DISPLAY. | **ENVIRONMENTAL (CI-lane wiring, not code; not in this audit's review surface)** | Wire `xvfb-run` into the Doc 28 §2.3 Option-A separate JS CI lane; record it in `GATE_VERIFICATION_v1_3` §"pre-tag JS-quad" so the pre-tag Extension-Host transcript is reproducible. No source change. | Deferrable-to-closeout (CI-lane + gate-doc). Carry on **every** future §11.1 brief. |

**Mandatory-before-V4 (briefing inputs, not code/doc edits):**
- **C-2 (the V4 IR-data-source contract):** §1.3 — the V4 brief MUST pin
  data-source = the V3 `cliBinary`+`cliRunner` `--emit-ir`-to-temp seam
  (NOT a server method, the R-9/R-10 DRIFT bar), and the codegen-gated
  boundary = last-valid-render + Doc 05 §1.5.9 banner **proven by an
  Extension-Host test** (the analogue of V3 `copyIr.ts:91-104`), with the
  R-15 fixture isolation inherited.
- **F-1+F-2 / N-1 carry (M-1 lineage):** the V4 brief MUST carry the
  "never re-add `transport: TransportKind.stdio` / never switch to a
  `NodeModule` server shape" constraint (V4 touches no client options, but
  this rule binds every `editors/vscode/` wave; cite `AUDIT_PHASE_V1` §3.A
  + this §1).

**Deferrable-to-v1.3-closeout (batched Doc-00 / `GATE_VERIFICATION_v1_3`
pass):** the F-1+F-2/N-1, N-2, N-3 doc edits; N-4 tooltip polish (→V5);
N-6 → Doc 21 §3 (J-1 ratified text); **JC-3** (→ config-owner wave);
**JC-1** (→ JS CI-lane + gate-doc). **CLOSED (record only):** **N-5**
(closed by V3).

---

## 3. Lens 3 — New V2/V3 doc-vs-shipped drift sweep

Beyond the catalogued, I spot-checked the brief's named surfaces against
the shipped merged tree at `61cc3f0` (file:line).

### 3.A Doc 22 §4 — command titles / category (V3 JC-4 normalization)

Doc 22 §4 (22:96-153) specs each command `title` as `"FSM: <X>"` with
`category: "FSM Studio"`. The shipped manifest (package.json:63-105) ships
`category: "FSM Studio"` for all 7 and a **bare** title (`"Check File"`,
`"Generate C99 Code"`, `"Copy IR JSON to Clipboard"`, …) **without** the
`"FSM: "` prefix.

**Verdict: ✅ CORRECT — this is the intended JC-4 normalization, and it
matches Doc 22's OWN authoritative directive, not a drift.** Doc 22's
header note (22:13) explicitly mandates the user-facing prefix be
`FSM Studio:` "(not `FSM:`)". VS Code renders a contributed command in the
palette as **`<category>: <title>`** → with `category:"FSM Studio"` and
title `"Check File"` the palette shows **`FSM Studio: Check File`** — the
*exactly desired* string. Had V3 kept Doc 22 §4's literal `"FSM: Check
File"` title under `category:"FSM Studio"`, the palette would double to
`FSM Studio: FSM: Check File`. The shipped form is the correct realization
of the §4 intent via the `category` mechanism. The stale literal is Doc 22
§4's per-command `title` strings (still `"FSM: …"`) — **fold into the same
v1.3-closeout Doc-22 pass** that owns JC-3 (a §4-title re-pin to bare
titles, noting the `category` produces the prefix); **non-load-bearing,
add to the catalogue's closeout bucket as a Doc-22-§4 citation re-pin**
(cosmetic doc hygiene, identical class to N-3 — not separately numbered).

### 3.B Doc 22 §5/§6 — keybindings / menus (V3 JC-5 / JC-6 scoping)

- **Keybindings.** Doc 22 §5 (22:161-180) specs three keybindings:
  `fsm.openDiagram` (`ctrl+shift+d`), `fsm.openSimulator` (`ctrl+alt+s`),
  `fsm.generateC99` (`ctrl+shift+g`). The shipped manifest
  (package.json:106-113) binds **only** `fsm.generateC99` (`ctrl+shift+g` /
  `cmd+shift+g`, `when: editorLangId == fsm-lang`). **Verdict: ✅ CORRECT
  (JC-5).** `fsm.openDiagram` is **V4** scope and `fsm.openSimulator` is
  out-of-v1.3 entirely (Doc 27 §9 — no Doc 13 simulator in v1.3); V3
  contributing only its own command's binding is exactly the disjoint-scope
  bar. **Forward note for V4:** V4 SHOULD add the Doc 22 §5
  `fsm.openDiagram` `ctrl+shift+d`/`cmd+shift+d` `when: editorLangId ==
  fsm-lang` keybinding when it contributes `fsm.openDiagram` — **a V4
  briefing input** (the symmetric "V3 owns its slice" boundary, now V4's).
  No drift in V3.
- **Menus.** Doc 22 §6 specs `editor/title` (openDiagram/openSimulator —
  both non-V3), `editor/context` (openDiagram@1, openSimulator@2,
  generateC99@3, generateCpp17@4), `explorer/context`
  (checkFile, generateC99), `commandPalette` (all, mostly
  `when: editorLangId == fsm-lang`). The shipped manifest:
  **no `editor/title`** (package.json has no `editor/title` key — correct:
  both its entries are non-V3); `editor/context` ships
  generateC99@`fsm@3` / generateCpp17@`fsm@4` / formatDocument@`fsm@5` /
  copyIR@`fsm@6` (package.json:115-136); `explorer/context` ships
  checkFile + generateC99 `when: resourceExtname == .fsm`
  (package.json:137-148 — **byte-exact to Doc 22 §6:223-234**);
  `commandPalette` gates the editor commands `when: editorLangId ==
  fsm-lang` and leaves `fsm.restartLanguageServer`/`fsm.showOutputChannel`
  **ungated** (package.json:149-176). **Verdict: ✅ CORRECT (JC-6).** The
  `editor/context` group **preserves Doc 22 §6's `fsm@N` numbering with
  the V4/sim slots (`fsm@1` openDiagram, `fsm@2` openSimulator)
  deliberately left vacant for V4** — a *correct* forward-compatible
  scoping (V3 starts at `fsm@3`, exactly its first owned command in Doc 22
  §6's ordering), not a drift. The two client-control commands being the
  only `commandPalette` entries **without** an `editorLangId` `when`
  is correct: they must be invocable with no `.fsm` open (a dead server is
  the case you most need "restart"/"show log"), and it matches Doc 22
  §6:263-268 (those two listed with no `when`). **No drift.** **Forward
  note for V4 (briefing input):** V4 must add `fsm.openDiagram` to
  `editor/title` group `navigation`, `editor/context` group `fsm@1`, and
  `commandPalette` `when: editorLangId == fsm-lang` per Doc 22 §6 — the
  vacant slots V3 correctly preserved.

### 3.C Merged `package.json` vs Doc 22 — union integrity

- `contributes.configuration` (package.json:178-207) = the **5 V1 keys
  only** (`compilerPath` + 4 inlay). V2/V3 did **not** touch it
  (V3 commit message asserts "Zero touches to
  contributes.{grammars,languages,snippets,configuration}"; verified — the
  block is byte-identical to V1's). The **JC-3** gap (codegen keys
  consumed but not contributed) is the only config delta — catalogued §2.
- `contributes.{languages,grammars,snippets}` (package.json:34-62) =
  V2's, intact; `languages[].configuration` →
  `./language-configuration.json`, `grammars[].path` →
  `./syntaxes/fsm-lang.tmLanguage.json`, `snippets[].path` →
  `./snippets/fsm-lang.json` — all three asset files present
  (`ls` confirmed: 460 / 35 / 113 lines respectively). ✅
- `activationEvents` (package.json:27-32) = V1's two
  (`onLanguage:fsm-lang`, `workspaceContains:**/*.fsm` — **no `*`**, Doc 22
  §2:57 honoured) **+ V3's two** (`onCommand:fsm.restartLanguageServer`,
  `onCommand:fsm.showOutputChannel`). The two `onCommand:` additions are
  **correct and necessary** (the M-2/N-5 commands must be invocable with no
  `.fsm` open / server down — without the `onCommand:` activation the
  command would not fire pre-activation). ✅ This is the **only** V3 touch
  to a V1-owned manifest region and it is additive + justified.
- `engines` (`vscode ^1.85.0`, `node >=20`) / `dependencies`
  (`vscode-languageclient ^9.0.1`) / `devDependencies` — byte-identical to
  V1 (V2/V3 commits both assert "engines/deps byte-untouched"; verified
  package.json:16-19,219-234 unchanged from the V1-audit baseline). ✅

### 3.D V2 grammar scope names vs Doc 21 §2 (the N-6 corrected-in-shipped delta)

Doc 21 §2 (21:50-117) is the authoritative scope-name table. The shipped
`syntaxes/fsm-lang.tmLanguage.json`:

- **Leaf scope names match Doc 21 §2 verbatim:**
  `keyword.declaration.machine.fsm` (tmLanguage.json:65 ≙ Doc 21:52),
  `keyword.declaration.composite.fsm` (:78 ≙ Doc 21:54),
  `keyword.declaration.parallel.fsm` (:91 ≙ Doc 21:55),
  `keyword.declaration.region.fsm` (:104 ≙ Doc 21:69),
  `keyword.declaration.state.fsm` (:117 ≙ Doc 21:53),
  `entity.name.type.machine.fsm` (:66 ≙ Doc 21:76),
  `entity.name.type.state.fsm` (:79/:92/:105/:118 ≙ Doc 21:77/81),
  `comment.line.documentation.fsm` (:32 ≙ Doc 21:107),
  `comment.line.double-slash.fsm` (:37 ≙ Doc 21:105),
  `comment.block.fsm` (:42). **All Doc-21-§2 verbatim.** ✅
- **The ONE structural deviation (N-6, corrected-in-shipped):** Doc 21 §3's
  literal (`21:175-191`) declares `machine-declaration`/`state-declaration`
  as single-line `"match"` rules with **no `begin`/`end`/body**; the
  shipped grammar (tmLanguage.json:60-123) declares them as `begin`/`end`
  **block rules** (`"begin": "\\b(machine)\\s+…\\s*(\\{)"`, `"end": "\\}"`,
  `"patterns":[{"include":"#machine-body"}]`) **plus** a
  `state-declaration-bare` `match` fallback (tmLanguage.json:125-132) for
  the bodyless form. Scope names on every capture are **unchanged** from
  Doc 21 §2. This is the **J-1/N-6** item: Doc 21 §3's *literal JSON* is
  structurally defective (no body rule ⇒ `#action-block` swallowed the
  machine body — the V2 behavioural gate caught it); the shipped grammar
  fixes the *structure* while keeping the *scope names* Doc-21-verbatim.
  The inline `_note` (tmLanguage.json:6) is the durable correction-of-record
  and explicitly asks for reviewer/§11.1 ratification (already ratified per
  established J-1 context). **CORRECTED-IN-SHIPPED — catalogued §2 N-6.**
  No *new* drift beyond this known item.

**Lens-3 verdict: no NEW code-level drift.** The Doc 22 §4-title and
Doc 21 §3-structure deltas are the *intended* JC-4/N-6 normalizations
(shipped code correct; only the stale literal doc prose folds at closeout);
JC-5/JC-6 scoping is correct disjoint-scope discipline with the V4/sim
slots correctly reserved. The only genuinely actionable new item is
**JC-3** (config-discoverability), catalogued. The Doc-22-§4-title re-pin
is folded into the catalogue's closeout bucket as a non-load-bearing
citation re-pin (N-3 class), not separately numbered.

---

## 4. Lens 4 — V4-readiness verdict

**PROCEED-WITH-NOTES.** The V2/V3 phase boundary is clean enough to
dispatch V4 (the diagram WebviewPanel) per Doc 28's cadence. Reasons:

1. **Zero Rust delta across the whole batch; cargo quad structurally
   safe.** `git diff ceb8efd 61cc3f0 -- crates/ Cargo.toml Cargo.lock
   rust-toolchain.toml` is **empty** (re-confirmed from `git` this audit);
   the entire `61cc3f0` delta (38 files / 9136 ins) is under
   `editors/vscode/`. V4 (also pure `editors/vscode/`) cannot regress the
   workspace the §11.1 / W0-clean certification covers.
2. **The V4 IR-data-source DRIFT (K-3) is RESOLVED, not open (§1).** The
   one real path (`fsm generate --emit-ir`, codegen-gated) exists; V3's
   `copyIr.ts` already drives it end-to-end and **already handles the R-11
   codegen-gated boundary honestly** (copyIr.ts:88-120 — the Webview
   analogue of which is V4's last-valid-render+banner); the reusable seam
   (`cliBinary.ts`+`cliRunner.ts`, optionally an extracted `emitIr` core)
   is identified. V4 builds the Webview on a proven IR producer — **no
   rework**.
3. **V2/V3 are strictly disjoint and additive on V1** (§3): V1's
   client-spawn `Executable` is byte-identical; the only V1-region touches
   are the additive `registerCommands(...)` call (extension.ts:128-133)
   and two justified `onCommand:` activation events; M-1 is preserved by
   construction (V3 never constructs a `ServerOptions`). N-5 is **closed**;
   the manifest union is integrity-checked.
4. **No NEW code-level drift** (§3) — the Doc 22 §4-title and Doc 21
   §3-structure deltas are the intended JC-4/N-6 normalizations (shipped
   correct, stale prose folds at closeout); the only actionable new item
   is **JC-3** (config-discoverability), deferrable and not a V4 blocker.

**Mandatory briefing-inputs for V4 (not code/doc edits):**
- **MV4-1 (C-2 — the V4 IR-data-source contract):** the V4 brief MUST pin
  §1.3: data-source = the **V3 `cliBinary.ts`+`cliRunner.ts`
  `--emit-ir`-to-temp seam** (explicitly **NOT** a new LSP/server method —
  the R-9/R-10 DRIFT bar, the Doc 26 §4.5 "do not smuggle a subsystem"
  rule); the **codegen-gated boundary = keep the last-valid render + show
  the Doc 05 §1.5.9 banner verbatim, proven by an Extension-Host test**
  (the analogue of V3 `copyIr.ts:91-104`); structural fidelity asserted
  against the same `--emit-ir` JSON as the oracle; **R-15 fixture
  isolation inherited** (no `fsm.toml [compiler]` in scope, the
  `extension.test.ts:115-125` pattern). State in the brief whether V4
  *extracts* a shared `emitIr` core out of `copyIr.ts` (cleanest; V4 must
  then re-assert `copyIR` unregressed — the SUBAGENT §10 refactor bar) or
  *calls the existing seam with a small commented duplication* (strict
  file-disjointness).
- **MV4-2 (M-1 lineage, F-1+F-2/N-1 carry):** the V4 brief MUST carry the
  "never re-add `transport: TransportKind.stdio`, never switch to a
  `NodeModule` server shape" constraint (V4 touches no client options, but
  this binds every `editors/vscode/` wave; cite `AUDIT_PHASE_V1` §3.A +
  this audit §1).
- **MV4-3 (the §6 manifest forward-slots V4 owns):** the V4 brief MUST
  include "contribute `fsm.openDiagram`: the command (Doc 22 §4, bare
  title + `category:"FSM Studio"` per the JC-4 pattern), the Doc 22 §5
  keybinding (`ctrl+shift+d`/`cmd+shift+d`, `when: editorLangId ==
  fsm-lang`), and the Doc 22 §6 menu slots V3 correctly reserved
  (`editor/title` group `navigation`; `editor/context` group `fsm@1`;
  `commandPalette` `when: editorLangId == fsm-lang`)". (§3.B forward
  notes.)

**Deferrable-to-v1.3-closeout (the batched-doc / `GATE_VERIFICATION_v1_3`
pass — the §2 catalogue):** F-1+F-2/N-1, N-2, N-3 doc edits; N-4 tooltip
(→V5); N-6 → Doc 21 §3 (J-1 ratified); the Doc-22-§4-title re-pin (N-3
class); **JC-3** (→ config-owner wave); **JC-1** (→ JS CI-lane wiring +
gate-doc). **CLOSED — record only:** **N-5** (closed by V3).

**V4 is unblocked once MV4-1/MV4-2/MV4-3 are in its brief.** No code or doc
edit is a precondition to dispatching V4. Per Doc 28 §3 a further
phase-boundary audit (§11.3) runs **after V4** (the substantive new
subsystem) before V5/V6.

---

## 5. Audit metadata

- **Audit type:** SUBAGENT §11.3 post-V2/V3-batch phase-boundary (the
  Doc 28 `V2/V3 → audit → V4` gate). READ-ONLY judgment. **ZERO build**
  (`cargo`/`npm`/`tsc`/`esbuild` not invoked).
- **HEAD audited:** `61cc3f0` (`main`). Worktree:
  `/root/dev/embeded-fsm-sdk-wt-v2v3audit`, branch
  `phase3.2/v1_3-v2v3-audit`.
- **Merge structure re-confirmed from `git`:** `61cc3f0` parents =
  `1542bfb` (V2) + `3c49d38` (V3); V1 `6707d1d` is the merge-base ancestor
  (`git log --oneline`: `6707d1d → 1542bfb → 3c49d38 → 61cc3f0` with
  `4280406` the V1-audit doc commit between V1 and V2).
- **Byte-identity re-confirmed:** `git diff ceb8efd 61cc3f0 --
  crates/ Cargo.toml Cargo.lock rust-toolchain.toml` → **empty** (Rust
  workspace == W0-clean `ceb8efd`); full `61cc3f0` delta = 38 files /
  9136 insertions, all `editors/vscode/`.
- **Independent re-derivations performed (not trusted from context):**
  the V4 seam — `copyIr.ts:54-134` (the `--emit-ir`-to-temp path + R-11
  codegen-gated honest boundary), `cliRunner.ts:37-76` (honest
  resolve-never-reject + ENOENT/non-zero-exit handling),
  `cliBinary.ts:73-108` (the `fsm`-CLI three-rule resolver, no
  silent fallback); M-1 preservation — `extension.ts:128-176` (additive
  `registerCommands` + byte-unchanged transport-omitted `Executable`),
  `index.ts:17-27` + `clientControl.ts:8-16,31-82` (restart via V1's
  `restartServer()`, no `ServerOptions` constructed); N-2 — re-read
  `extension.test.ts:194-206` (`test("(c) … (UTF-16)")`, asserts
  `negotiatedPositionEncoding`); N-5 — package.json:30-31,95-104 +
  `clientControl.ts`; N-6 — `tmLanguage.json:6` `_note` +
  `:60-132` (begin/end block conversion) vs Doc 21 `§3:175-191` (literal
  `match` rules) with §2 scope names byte-compared; JC-3 —
  `generate.ts:34-57` (reads `codegen.outputDir`/`strategy`) vs
  package.json:180-207 (5 keys only) vs Doc 22 §8:371-384.
- **Toolchain probe (trap-aware, not needed but asserted):** the brief's
  established context fixes the 1.75.0 pin; **no `rustc` shelled** (zero
  build). Per `feedback_embeded_fsm_toolchain_probe_trap`, had a probe
  been needed: `sh -c 'cd /root/dev/embeded-fsm-sdk && rustup show
  active-toolchain'` (=1.75.0 overridden); bare `rustc` 1.95.0 = benign
  box default, NOT pin drift.
- **Verdict:** **PROCEED-WITH-NOTES** — V4 dispatchable; MV4-1
  (the V4 IR-data-source contract), MV4-2 (M-1 lineage), MV4-3 (the §6
  manifest forward-slots) are mandatory briefing inputs; the §2 catalogue
  folds at v1.3-closeout (N-5 closed; N-6/Doc-22-§4 corrected-in-shipped;
  JC-3/JC-1 the only genuinely-open new items, both deferrable).
- **Pattern:** mirrors `AUDIT_PHASE_W0_2026_05_16.md` /
  `AUDIT_PHASE_V1_2026_05_16.md` (verdict TL;DR → Lens-1 V4-seam keystone
  analysis → Lens-2 canonical accumulated catalogue → Lens-3 doc-vs-shipped
  sweep → Lens-4 readiness verdict + required orchestrator/briefing
  actions). §1+§2 are the durable correction-of-record until the
  batched-doc closeout (the W0/V1 §11.3 / N-1 precedent).

*End of AUDIT_PHASE_V2V3_2026_05_16*
