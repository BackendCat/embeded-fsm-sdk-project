// V5 behavioural-acceptance gate — Doc 28 §3-V5 / Doc 27 §5/§8-V5; Doc 22
// §7/§10/§11 (NOT symbol-presence; "a view container exists / a provider
// is registered" is explicitly NOT acceptance and is rejected — the P0-1
// lesson). This is the extension-layer §5.4 analogue, the sibling of
// `extension.test.ts` / `diagram.test.ts`: a REAL headless VS Code
// Extension Host + the REAL `fsm-lang-server`, EXERCISING the FSM
// activity-bar tree and asserting its rendered node STRUCTURE deep-equals
// the `documentSymbol` hierarchy the running server returns for that exact
// file.
//
// V1/V2/V3/V4 test files are NOT edited (separate file; the runner globs
// suite/**/*.test.js so this is auto-discovered).
//
// THE FOUR LOAD-BEARING ASSERTIONS (the V5 gate, proven not assumed):
//
//  (1) TREE-FIDELITY vs documentSymbol (the MV5-1 keystone). Open the
//      multi-machine fixture. INDEPENDENTLY obtain the symbol oracle via
//      `vscode.executeDocumentSymbolProvider` (the SAME free V1-client
//      capability the SUT consumes — server.rs:251), and deep-compare the
//      Machines tree's projected node forest against it: every machine
//      root, the Doc 14 §13 `events`/`states` group nodes, the nested
//      statechart hierarchy (composite `Locked` → `Idle`), names, kinds,
//      and BOTH ranges must be EQUAL — re-projected, not re-analyzed. A
//      divergence means the tree drifted from the canonical symbol
//      projection (or — the forbidden MV5-1 foot-gun — was built from the
//      codegen-gated `--emit-ir` path, which has a DIFFERENT shape and
//      would vanish for a non-codegen'able machine). The Events tree is
//      asserted the same way against the hoisted `events` groups.
//
//  (2) CLICK→DECLARATION. Drive the SUT's real `fsm.revealSymbol` command
//      with a tree node's `selectionRange` and assert the editor's
//      selection lands EXACTLY on that range (the authoritative name span
//      the LSP computed — Doc 05 §1.3.6; no new location math).
//
//  (3) CONTEXT-KEY GATING. The Doc 22 §7 views are `when:
//      fsm.hasOpenFsmFile`. Toggle a `.fsm` active vs a non-`.fsm`
//      document and assert the OBSERVABLE consequence of the
//      context-key-gated tree: populated when a `.fsm` is active, empty
//      when not (the `when:`-clause behaviour Doc 28 §3-V5:348-350
//      mandates — not "the manifest contains a when string").
//
//  (4) N-4 STATUS-BAR TOOLTIP. A REAL `FsmStatusBar.setRestarting(N)`
//      must render the Doc 22:679 literal `FSM Language Server restarting
//      (attempt N/3)...` VERBATIM on the actual status-bar item (the
//      audit-sanctioned V5 fold-in; the same verbatim-contract assertion
//      diagram.test.ts used for the Doc 05 §1.5.9 banner).
//
// R-15 (MANDATORY, inherited): the fixture is staged in an OS temp dir so
// the analyzer's upward `fsm.toml` walk finds none — `documentSymbol` runs
// the analyzer and an `fsm.toml [compiler] allow/deny` could project a
// different symbol set than the oracle expects (the
// `extension.test.ts:115-125` / `diagram.test.ts:177-195` pattern, reused
// verbatim per the V5 brief's R-15 inheritance).

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveRealBinaries } from "./binaries";
// The V5 modules under test — static import (the suite convention; the
// only dynamic imports in this codebase are Node builtins). `projectMachineTree`
// / `projectEventTree` are the SUT's OWN pure mappers (fed the SAME oracle
// artifact independently — a true structural equality, not a tautology);
// `FsmStatusBar` drives the N-4 fix directly; `restartingTooltip` is the
// shipped Doc 22:679 literal builder.
import type { FsmTreeNode } from "../../tree/symbolTree";
import {
  projectEventTree,
  projectMachineTree,
} from "../../tree/symbolTree";
import { FsmStatusBar, restartingTooltip } from "../../statusBar";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");

interface FsmExplorerHandle {
  refresh(): Promise<void>;
  machineRoots(): FsmTreeNode[];
  eventRoots(): FsmTreeNode[];
  syncContextKeys(): void;
}
interface FsmExtensionApi {
  readonly serverStarted: boolean;
  readonly negotiatedPositionEncoding: string | undefined;
  readonly binarySource: "compilerPath" | "bundled" | "none";
  readonly fsmExplorer: FsmExplorerHandle | undefined;
}

/** A serialisable structural view of a tree node — names + kinds + the
 * two ranges + the recursive child shape. The deep-compare key: a
 * divergence here is a real fidelity break. */
interface NodeShape {
  name: string;
  detail: string | undefined;
  kind: number;
  range: [number, number, number, number];
  selectionRange: [number, number, number, number];
  children: NodeShape[];
}

function rng(r: vscode.Range): [number, number, number, number] {
  return [
    r.start.line,
    r.start.character,
    r.end.line,
    r.end.character,
  ];
}

/** Project a SUT {@link FsmTreeNode} to its comparable shape. */
function shapeOfTreeNode(n: FsmTreeNode): NodeShape {
  return {
    name: n.name,
    detail: n.detail,
    kind: n.kind,
    range: rng(n.range),
    selectionRange: rng(n.selectionRange),
    children: n.children.map(shapeOfTreeNode),
  };
}

/** Project a raw `vscode.DocumentSymbol` (the INDEPENDENT oracle) to the
 * SAME comparable shape — NOT via the SUT mapper, so the equality is a
 * true oracle, not a tautology against the SUT. */
function shapeOfSymbol(s: vscode.DocumentSymbol): NodeShape {
  return {
    name: s.name,
    detail: s.detail === "" ? undefined : s.detail,
    kind: s.kind,
    range: rng(s.range),
    selectionRange: rng(s.selectionRange),
    children: (s.children ?? []).map(shapeOfSymbol),
  };
}

/** The Events-tree oracle, derived INDEPENDENTLY from the raw symbols:
 * each machine's `events` group hoisted, machines with no events dropped
 * (mirrors the SUT contract but computed here from the oracle, not the
 * SUT). */
function eventOracleShape(
  symbols: vscode.DocumentSymbol[],
): NodeShape[] {
  const out: NodeShape[] = [];
  for (const m of symbols) {
    const ev = (m.children ?? []).find((c) => c.name === "events");
    const leaves = (ev?.children ?? []).map(shapeOfSymbol);
    if (leaves.length === 0) {
      continue;
    }
    out.push({
      name: m.name,
      detail: m.detail === "" ? undefined : m.detail,
      kind: m.kind,
      range: rng(m.range),
      selectionRange: rng(m.selectionRange),
      children: leaves,
    });
  }
  return out;
}

async function getSymbolOracle(
  uri: vscode.Uri,
): Promise<vscode.DocumentSymbol[]> {
  // Poll: the server may not have published the analysis the instant the
  // doc opens. The oracle is the SAME free capability the SUT uses.
  const deadline = Date.now() + 30_000;
  for (;;) {
    const got = await vscode.commands.executeCommand<
      vscode.DocumentSymbol[]
    >("vscode.executeDocumentSymbolProvider", uri);
    if (got && got.length > 0) {
      return got;
    }
    if (Date.now() > deadline) {
      throw new Error(
        "timed out waiting for executeDocumentSymbolProvider oracle",
      );
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

async function waitFor(
  predicate: () => boolean | Promise<boolean>,
  what: string,
  timeoutMs = 30_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (await predicate()) {
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(`timed out waiting for: ${what}`);
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

suite("FSM Studio V5 — Extension-Host tree + context-key acceptance", () => {
  let api: FsmExtensionApi;
  let explorer: FsmExplorerHandle;
  let tmpDir: string;
  let fixture: string;
  let plainFile: string;

  suiteSetup(async function () {
    this.timeout(600_000); // a cold cargo build can be slow on a busy box.

    const bins = resolveRealBinaries();

    // R-15 isolation: stage the multi-machine fixture in an OS temp dir;
    // hard-assert the upward fsm.toml walk finds none (the
    // extension.test.ts:115-125 pattern, reused verbatim per the V5 brief).
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v5-tree-"));
    fs.copyFileSync(
      path.join(FIXTURE_SRC, "multi_machine.fsm"),
      path.join(tmpDir, "multi_machine.fsm"),
    );
    fixture = path.join(tmpDir, "multi_machine.fsm");
    // A NON-.fsm file in the SAME dir for the context-key toggle test.
    plainFile = path.join(tmpDir, "notes.txt");
    fs.writeFileSync(plainFile, "not a state machine\n");
    let dir = tmpDir;
    for (;;) {
      assert.ok(
        !fs.existsSync(path.join(dir, "fsm.toml")),
        `R-15 violated: fsm.toml found at ${dir}`,
      );
      const parent = path.dirname(dir);
      if (parent === dir) {
        break;
      }
      dir = parent;
    }

    // Launch the real server via fsmLang.compilerPath (Rule 1) — the SAME
    // un-overridden V1 client auto-registers the documentSymbol provider
    // the tree consumes (MV5-1: zero new extension code at the LSP layer).
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update(
        "compilerPath",
        bins.server,
        vscode.ConfigurationTarget.Global,
      );

    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    api = (await ext.activate()) as FsmExtensionApi;
    assert.ok(api, "activate() must return the extension API");
    assert.ok(
      api.fsmExplorer,
      "V5 must register the FSM explorer (the handle is the §5.4 " +
        "test-observability surface — NOT the acceptance itself)",
    );
    explorer = api.fsmExplorer;

    // Wait for the language client to actually reach Running so the
    // documentSymbol round-trip has a live server behind it.
    const startDeadline = Date.now() + 30_000;
    while (!api.serverStarted) {
      if (Date.now() > startDeadline) {
        throw new Error("language client never reached Running state");
      }
      await new Promise((r) => setTimeout(r, 150));
    }
  });

  suiteTeardown(async () => {
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update(
        "compilerPath",
        undefined,
        vscode.ConfigurationTarget.Global,
      );
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  // Precondition (NOT the acceptance): the tree providers + view container
  // are contributed/registered. "A provider is registered" is explicitly
  // NOT acceptance — the structural deep-compare below is.
  test("the FSM explorer views + refresh commands are registered", async () => {
    const cmds = await vscode.commands.getCommands(true);
    assert.ok(
      cmds.includes("fsm.refreshMachineExplorer"),
      "fsm.refreshMachineExplorer must be a registered command",
    );
    assert.ok(
      cmds.includes("fsm.refreshEventExplorer"),
      "fsm.refreshEventExplorer must be a registered command",
    );
    assert.ok(
      cmds.includes("fsm.revealSymbol"),
      "fsm.revealSymbol must be a registered command",
    );
    // The Doc 22 §7 view container + views are in the manifest (a
    // precondition; the acceptance is the fidelity deep-compare).
    const pkg = ext_pkg();
    const containers =
      pkg.contributes.viewsContainers.activitybar as Array<{
        id: string;
      }>;
    assert.ok(
      containers.some((c) => c.id === "fsm-explorer"),
      "Doc 22 §7 fsm-explorer activity-bar container must be contributed",
    );
    const views = pkg.contributes.views["fsm-explorer"] as Array<{
      id: string;
      when: string;
    }>;
    assert.deepStrictEqual(
      views.map((v) => v.id).sort(),
      ["fsm.eventExplorer", "fsm.machineExplorer"],
      "Doc 22 §7 fsm.machineExplorer + fsm.eventExplorer must be contributed",
    );
    for (const v of views) {
      assert.strictEqual(
        v.when,
        "fsm.hasOpenFsmFile",
        `view ${v.id} must be when: fsm.hasOpenFsmFile (Doc 22 §7/§11)`,
      );
    }
  });

  // (1) TREE-FIDELITY vs documentSymbol — the Machines tree's projected
  //     node forest deep-equals the INDEPENDENT executeDocumentSymbolProvider
  //     result EXACTLY (re-projected, not re-analyzed — the MV5-1 keystone).
  test("the Machines tree's node structure deep-equals the documentSymbol hierarchy EXACTLY", async function () {
    this.timeout(120_000);

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    // INDEPENDENT oracle: the SAME free V1-client capability the SUT uses,
    // queried here directly (not via the SUT mapper).
    const oracle = await getSymbolOracle(doc.uri);

    // Sanity: the oracle is the known multi-machine truth (so a server
    // schema change fails loudly, not silently). 2 machine roots: Traffic
    // (3 simple states + 2 events) and Door (Closed/Open simple + Locked
    // composite→Idle; 3 events).
    assert.strictEqual(
      oracle.length,
      2,
      `oracle precondition: 2 machine roots, got ${oracle
        .map((s) => s.name)
        .join(",")}`,
    );
    assert.deepStrictEqual(
      oracle.map((s) => s.name),
      ["Traffic", "Door"],
      "oracle precondition: machine roots are Traffic, Door",
    );
    const door = oracle.find((s) => s.name === "Door");
    assert.ok(door, "oracle: Door machine present");
    const doorStates = (door.children ?? []).find(
      (c) => c.name === "states",
    );
    const locked = (doorStates?.children ?? []).find(
      (s) => s.name === "Locked",
    );
    assert.ok(
      locked && (locked.children ?? []).some((c) => c.name === "Idle"),
      "oracle precondition: composite Locked nests Idle (the nesting " +
        "the fidelity compare must preserve)",
    );

    // Force the SUT to re-acquire the documentSymbol projection + repaint
    // (deterministic — not waiting on an editor event).
    await explorer.refresh();
    await waitFor(
      () => explorer.machineRoots().length === 2,
      "the Machines tree to project the 2 machine roots",
      30_000,
    );

    // THE ACCEPTANCE: the SUT's rendered forest deep-equals the
    // INDEPENDENT oracle's full hierarchy — every root, group node,
    // nested state, name, kind, AND both ranges. Not "a tree appeared".
    const sutShape = explorer.machineRoots().map(shapeOfTreeNode);
    const oracleShape = oracle.map(shapeOfSymbol);
    assert.deepStrictEqual(
      sutShape,
      oracleShape,
      "the Machines tree's projected node structure MUST deep-equal the " +
        "executeDocumentSymbolProvider hierarchy EXACTLY (re-projected, " +
        "not re-analyzed). A divergence means the tree drifted from the " +
        "canonical symbol projection OR was built from the forbidden " +
        "codegen-gated --emit-ir path (MV5-1).",
    );

    // Independent cross-check: the SUT's OWN pure mapper, fed the SAME
    // oracle artifact, must yield the SAME shape (a real structural
    // equality at the mapper boundary too — the diagram.test.ts pattern).
    assert.deepStrictEqual(
      projectMachineTree(oracle).map(shapeOfTreeNode),
      oracleShape,
      "projectMachineTree(oracle) must be a faithful 1:1 re-projection",
    );
  });

  // (1b) The Events tree is the SAME documentSymbol response, lensed to
  //      the hoisted per-machine `events` groups — also deep-equal.
  test("the Events tree's node structure deep-equals the hoisted documentSymbol events groups", async function () {
    this.timeout(60_000);

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);
    const oracle = await getSymbolOracle(doc.uri);

    await explorer.refresh();
    await waitFor(
      () => explorer.eventRoots().length === 2,
      "the Events tree to project both machines' event groups",
      30_000,
    );

    const sutShape = explorer.eventRoots().map(shapeOfTreeNode);
    const oracleShape = eventOracleShape(oracle);
    assert.deepStrictEqual(
      sutShape,
      oracleShape,
      "the Events tree must deep-equal the hoisted documentSymbol " +
        "events groups EXACTLY (same data source, a lens — not a " +
        "second analysis)",
    );
    // Cross-check at the SUT mapper boundary: projectEventTree fed the
    // SAME oracle artifact must yield the SAME shape (a true structural
    // equality, not a tautology — the projectMachineTree pattern).
    assert.deepStrictEqual(
      projectEventTree(oracle).map(shapeOfTreeNode),
      oracleShape,
      "projectEventTree(oracle) must be a faithful hoisted re-projection",
    );
    // Concrete sanity: Traffic has TICK/RESET, Door OPEN/CLOSE/LOCK.
    const traffic = sutShape.find((m) => m.name === "Traffic");
    assert.deepStrictEqual(
      traffic?.children.map((c) => c.name),
      ["TICK", "RESET"],
      "Traffic's event leaves must be TICK, RESET (verbatim symbols)",
    );
    const doorE = sutShape.find((m) => m.name === "Door");
    assert.deepStrictEqual(
      doorE?.children.map((c) => c.name),
      ["OPEN", "CLOSE", "LOCK"],
      "Door's event leaves must be OPEN, CLOSE, LOCK",
    );
  });

  // (2) CLICK→DECLARATION — the SUT's real reveal command navigates the
  //     editor selection to the EXACT documentSymbol selectionRange (the
  //     authoritative name span; no new location math — Doc 05 §1.3.6).
  test("clicking a tree node reveals the editor at the symbol's exact selectionRange", async function () {
    this.timeout(60_000);

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);
    const oracle = await getSymbolOracle(doc.uri);

    // Target a deeply-nested node so the navigation is non-trivial: the
    // composite Door → Locked → Idle state. Its selectionRange is the
    // "Idle" name token the LSP computed.
    const door = oracle.find((s) => s.name === "Door");
    const states = (door?.children ?? []).find(
      (c) => c.name === "states",
    );
    const locked = (states?.children ?? []).find(
      (s) => s.name === "Locked",
    );
    const idle = (locked?.children ?? []).find((s) => s.name === "Idle");
    assert.ok(idle, "oracle: Door→Locked→Idle present");
    const want = idle.selectionRange;

    // Move the cursor somewhere else first so the reveal is a real move.
    const ed0 = vscode.window.activeTextEditor;
    assert.ok(ed0, "an editor must be active");
    ed0.selection = new vscode.Selection(0, 0, 0, 0);

    // Drive the SUT's REAL contributed reveal command (the exact command
    // a TreeItem click invokes — FsmTreeItem.command).
    await vscode.commands.executeCommand(
      "fsm.revealSymbol",
      doc.uri,
      want,
    );

    await waitFor(() => {
      const ed = vscode.window.activeTextEditor;
      return (
        ed !== undefined &&
        ed.document.uri.fsPath === fixture &&
        ed.selection.start.line === want.start.line &&
        ed.selection.start.character === want.start.character
      );
    }, "the editor selection to land on the symbol's selectionRange");

    const ed = vscode.window.activeTextEditor;
    assert.ok(ed, "an editor must be active after reveal");
    assert.strictEqual(
      ed.document.uri.fsPath,
      fixture,
      "reveal must focus the declaring document",
    );
    assert.strictEqual(
      ed.selection.start.line,
      want.start.line,
      "reveal selection start.line must equal the symbol selectionRange",
    );
    assert.strictEqual(
      ed.selection.start.character,
      want.start.character,
      "reveal selection start.character must equal the selectionRange",
    );
    assert.strictEqual(
      ed.selection.end.line,
      want.end.line,
      "reveal selection end.line must equal the selectionRange",
    );
    assert.strictEqual(
      ed.selection.end.character,
      want.end.character,
      "reveal selection end.character must equal the selectionRange",
    );
    // It really IS the "Idle" token (the authoritative name span), not
    // the whole state decl — selectionRange ⊊ range, the LSP contract.
    const selText = ed.document.getText(
      new vscode.Range(want.start, want.end),
    );
    assert.strictEqual(
      selText,
      "Idle",
      "the revealed selectionRange must be the 'Idle' name token " +
        "(the authoritative declaration span — Doc 05 §1.3.6)",
    );
  });

  // (3) CONTEXT-KEY GATING — the Doc 22 §7 views are `when:
  //     fsm.hasOpenFsmFile`. Assert the OBSERVABLE consequence: the tree
  //     projects content when a `.fsm` is active and is EMPTY when a
  //     non-`.fsm` document is active (the `when:` behaviour Doc 28
  //     §3-V5:348-350 mandates — not "the manifest has a when string").
  test("context-key-gated tree: populated for an active .fsm, empty for a non-.fsm", async function () {
    this.timeout(60_000);

    // (a) A .fsm is active → fsm.hasOpenFsmFile true → tree populated.
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);
    await getSymbolOracle(doc.uri); // ensure the server has analyzed.
    explorer.syncContextKeys();
    await explorer.refresh();
    await waitFor(
      () => explorer.machineRoots().length === 2,
      "the tree to populate while a .fsm is the active editor",
    );
    assert.strictEqual(
      explorer.machineRoots().length,
      2,
      "with a .fsm active the gated Machines tree shows its machines",
    );

    // (b) Switch to a NON-.fsm document → fsm.hasOpenFsmFile false → the
    //     gated tree has no FSM content (the views would be hidden by the
    //     `when:` clause; the observable data consequence is an empty
    //     projection — the SUT must NOT fabricate or IR-source nodes).
    const plainDoc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(plainFile),
    );
    await vscode.window.showTextDocument(plainDoc);
    explorer.syncContextKeys();
    await explorer.refresh();
    await waitFor(
      () => explorer.machineRoots().length === 0,
      "the gated tree to clear when no .fsm is the active editor",
    );
    assert.strictEqual(
      explorer.machineRoots().length,
      0,
      "with a non-.fsm active the fsm.hasOpenFsmFile-gated tree must be " +
        "empty (no fabricated / IR-sourced nodes — the cardinal-sin bar)",
    );
    assert.strictEqual(
      explorer.eventRoots().length,
      0,
      "the Events tree must clear too when no .fsm is active",
    );

    // (c) Back to the .fsm → re-populated (the gate is reversible, not a
    //     one-way latch).
    await vscode.window.showTextDocument(doc);
    explorer.syncContextKeys();
    await explorer.refresh();
    await waitFor(
      () => explorer.machineRoots().length === 2,
      "the gated tree to re-populate when the .fsm is active again",
    );
    assert.strictEqual(
      explorer.machineRoots().length,
      2,
      "the fsm.hasOpenFsmFile gate must be reversible",
    );
  });

  // (4) N-4 STATUS-BAR TOOLTIP — a REAL FsmStatusBar.setRestarting(N)
  //     renders the Doc 22:679 literal VERBATIM on the actual status-bar
  //     item (the audit-sanctioned V5 fold-in; the diagram.test.ts
  //     verbatim-contract pattern).
  test("N-4: the restarting status-bar tooltip is the Doc 22:679 literal VERBATIM", () => {
    // The shipped string builder is the Doc 22:679 text exactly (a
    // paraphrase is a contract regression — assert the text, not just
    // that a function exists).
    assert.strictEqual(
      restartingTooltip(1),
      "FSM Language Server restarting (attempt 1/3)...",
      "restartingTooltip(1) must be the Doc 22:679 literal verbatim",
    );
    assert.strictEqual(
      restartingTooltip(3),
      "FSM Language Server restarting (attempt 3/3)...",
      "restartingTooltip(3) must carry the dynamic attempt N verbatim",
    );

    // BEHAVIOURAL: a real FsmStatusBar.setRestarting(2) must render that
    // exact tooltip + the Doc 22:679 $(sync~spin)/FSM icon+text on the
    // ACTUAL status-bar item (not merely that a constant equals a string
    // — the rendered item is the contract the user sees).
    const sb = new FsmStatusBar();
    try {
      sb.setRestarting(2);
      const r = sb.rendered();
      assert.strictEqual(
        r.tooltip,
        "FSM Language Server restarting (attempt 2/3)...",
        "FsmStatusBar.setRestarting(2) must RENDER the Doc 22:679 " +
          "tooltip verbatim (N-4 — previously the generic '…starting')",
      );
      assert.strictEqual(
        r.text,
        "$(sync~spin) FSM",
        "the recovery icon/text must stay $(sync~spin)/FSM (Doc 22:679)",
      );
    } finally {
      sb.dispose();
    }
  });
});

/** The shipped extension manifest (read once for the §7 precondition
 * checks — a precondition, NOT the acceptance, which is the deep-compare). */
function ext_pkg(): {
  contributes: {
    viewsContainers: { activitybar: unknown };
    views: Record<string, unknown>;
  };
} {
  const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
  assert.ok(ext, "extension must be present to read its manifest");
  return ext.packageJSON as ReturnType<typeof ext_pkg>;
}
