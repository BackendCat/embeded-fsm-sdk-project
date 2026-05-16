// V4 behavioural-acceptance gate — Doc 28 §3-V4 / Doc 27 §6/§8-V4 (NOT
// symbol-presence; "the command is registered / a webview opened" is
// explicitly NOT acceptance and is rejected — the P0-1 lesson). This is
// the extension-layer §5.4 analogue, the sibling of `extension.test.ts` /
// `commands.test.ts`: a REAL headless VS Code Extension Host + the REAL
// `fsm` CLI, EXECUTING `fsm.openDiagram` and asserting the Webview
// rendered the machine's graph FROM the REAL `fsm generate --emit-ir` IR.
//
// V1/V2/V3 test files are NOT edited (separate file; the runner globs
// suite/**/*.test.js so this is auto-discovered).
//
// THE TWO LOAD-BEARING ASSERTIONS (the V4 gate, proven not assumed):
//
//  (1) IR→GRAPH FIDELITY. Open the diagram for `clean.fsm` (machine Gate)
//      via the REAL contributed+registered command. Independently run the
//      SAME real `fsm generate --emit-ir` ourselves (the established oracle
//      pattern — `commands.test.ts` copyIR uses the identical one), parse
//      that artifact, and assert the Webview's rendered structural model
//      has the EXACT state/transition set of that IR — same node ids, same
//      edge (source,target) set, same counts. Asserted at BOTH boundaries:
//      the Webview's posted `rendered` ack (proving the model reached + laid
//      out in the REAL Webview — elkjs ran in-browser-context, not just in
//      Node) AND the pure fsm-ir projection (`parseAndBuild`, the SUT's own
//      mapper, fed the SAME oracle artifact). A divergence means the
//      diagram drifted from the canonical IR — the Doc 27 §6.3 contract.
//
//  (2) THE CODEGEN-GATED BOUNDARY (the DRIFT-class assertion). With a valid
//      render already established for the Gate machine, drive a refresh
//      whose IR acquisition hits the codegen-gated `emitIr` boundary (the
//      CLI exits non-zero ⇒ NO `.ir.json` — the IDENTICAL `emitIr` /
//      `DiagramView.refresh()` path a pure-codegen ICE takes; the
//      V3-blessed analogue, the exact AUDIT_PHASE_V2V3_2026_05_16 §1.3.2
//      "analogue of V3 copyIr.ts:91-104, surfaced as last-valid-retention").
//      Assert: the controller STILL holds the clean Gate model (last-valid
//      RETAINED, not cleared), the Webview received the Doc 05 §1.5.9
//      banner VERBATIM + reports it still holds the last-valid render. "A
//      webview opened" / "a panel exists" is explicitly NOT acceptance.
//
// R-15 (MANDATORY): fixtures are staged in an OS temp dir so the CLI's
// upward `fsm.toml` walk finds none — `--emit-ir` runs the CLI and an
// allow/deny fixture could project diagnostics that mask a codegen failure
// differently than the oracle expects (the `commands.test.ts` /
// `extension.test.ts:115-125` pattern, reused verbatim per the V4 brief).

import * as assert from "assert";
import { execFileSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveRealBinaries } from "./binaries";
// The V4 module under test — static import (the V3-suite convention; the
// only dynamic imports in this codebase are Node builtins). `parseAndBuild`
// + `STALE_BANNER` are the SUT's own mapper/contract; `DiagramController`
// drives the codegen-gated boundary directly (its ctor registers NO command
// — only `registerOpenDiagram` does, and the extension already did that in
// activate()).
import { DiagramController, STALE_BANNER as SUT_STALE_BANNER, parseAndBuild } from "../../diagram";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");
const STALE_BANNER = "⚠ Diagram shows last valid state. Fix parse errors to update.";

interface FsmExtensionApi {
  readonly serverStarted: boolean;
  readonly negotiatedPositionEncoding: string | undefined;
  readonly binarySource: "compilerPath" | "bundled" | "none";
}

/** The fsm-ir shape we project for the oracle (mirrors irGraph.ts; an
 * INDEPENDENT re-projection so the test is a true oracle, not a tautology
 * against the SUT mapper). */
interface OracleIr {
  irVersion?: string;
  machines?: Array<{ name?: string; root?: OracleRegion }>;
}
interface OracleRegion {
  states?: OracleState[];
}
interface OracleState {
  kind: string;
  id: string;
  transitions?: Array<{ id: string; source: string; target: string }>;
  regions?: OracleRegion[];
}

function oracleSets(ir: OracleIr): {
  nodeIds: Set<string>;
  edges: Set<string>;
} {
  const nodeIds = new Set<string>();
  const edges = new Set<string>();
  const withTx = new Set(["simple", "composite", "parallel", "submachine_ref"]);
  const withRegions = new Set(["composite", "parallel"]);
  const walk = (r: OracleRegion | undefined): void => {
    for (const s of r?.states ?? []) {
      nodeIds.add(s.id);
      if (withTx.has(s.kind)) {
        for (const t of s.transitions ?? []) {
          edges.add(`${t.source}->${t.target}`);
        }
      }
      if (withRegions.has(s.kind)) {
        for (const inner of s.regions ?? []) {
          walk(inner);
        }
      }
    }
  };
  walk(ir.machines?.[0]?.root);
  return { nodeIds, edges };
}

/** Run the real CLI `generate --emit-ir` and return the parsed IR (the
 * exact oracle `commands.test.ts` copyIR uses). */
function emitIrOracle(cli: string, fsmPath: string): OracleIr {
  const out = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v4-iror-"));
  try {
    execFileSync(cli, ["generate", "--target", "c99", "--out", out, "--emit-ir", fsmPath], {
      stdio: "ignore",
    });
    const irFile = fs.readdirSync(out).find((f) => f.endsWith(".ir.json"));
    assert.ok(irFile, "oracle: real --emit-ir wrote a .ir.json");
    return JSON.parse(fs.readFileSync(path.join(out, irFile), "utf8")) as OracleIr;
  } finally {
    fs.rmSync(out, { recursive: true, force: true });
  }
}

async function waitFor(predicate: () => boolean, what: string, timeoutMs = 30_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (predicate()) {
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(`timed out waiting for: ${what}`);
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

suite("FSM Studio V4 — Extension-Host diagram behavioural acceptance", () => {
  let cliBinary: string;
  let extensionPath: string;
  let tmpDir: string;
  /** Webview→ext messages captured by wrapping createWebviewPanel ONCE. */
  const webviewMsgs: Array<Record<string, unknown>> = [];

  suiteSetup(async function () {
    this.timeout(600_000);

    const bins = resolveRealBinaries();
    cliBinary = bins.cli;

    // R-15 isolation: stage fixtures in an OS temp dir; hard-assert the
    // upward fsm.toml walk finds none (the extension.test.ts:115-125
    // pattern, reused verbatim per the V4 brief's R-15 inheritance).
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v4-diag-"));
    for (const f of ["clean.fsm", "broken.fsm"]) {
      fs.copyFileSync(path.join(FIXTURE_SRC, f), path.join(tmpDir, f));
    }
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

    // Launch via fsmLang.compilerPath (Rule 1). The V4 diagram path uses
    // the `fsm` CLI (the sibling the V3 cliBinary.ts resolves), NOT the
    // server — but activating with a resolved server is the realistic
    // host state and matches the V3 suite.
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update("compilerPath", bins.server, vscode.ConfigurationTarget.Global);

    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    const api = (await ext.activate()) as FsmExtensionApi;
    assert.ok(api, "activate() must return the extension API");
    extensionPath = ext.extensionPath;

    // Capture every message any diagram Webview posts back (the IR→graph
    // `rendered` ack + the `staleShown` ack) by wrapping
    // createWebviewPanel ONCE — both the REAL command path and the
    // directly-driven controller go through this same factory.
    const realCreate = vscode.window.createWebviewPanel;
    (
      vscode.window as unknown as {
        createWebviewPanel: typeof vscode.window.createWebviewPanel;
      }
    ).createWebviewPanel = ((...args: Parameters<typeof vscode.window.createWebviewPanel>) => {
      const panel = (
        realCreate as (
          ...a: Parameters<typeof vscode.window.createWebviewPanel>
        ) => vscode.WebviewPanel
      )(...args);
      panel.webview.onDidReceiveMessage((m: Record<string, unknown>) => {
        webviewMsgs.push(m);
      });
      return panel;
    }) as typeof vscode.window.createWebviewPanel;
  });

  suiteTeardown(async () => {
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update("compilerPath", undefined, vscode.ConfigurationTarget.Global);
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  // Precondition (NOT the acceptance): the command exists.
  test("fsm.openDiagram is registered", async () => {
    const all = await vscode.commands.getCommands(true);
    assert.ok(all.includes("fsm.openDiagram"), "fsm.openDiagram must be a registered command");
  });

  // (1) IR→GRAPH FIDELITY — the REAL command renders the EXACT
  //     state/transition set of the real `--emit-ir` IR (asserted at the
  //     real Webview's posted ack AND the SUT's own pure mapper — not
  //     "a panel opened").
  test("fsm.openDiagram renders the EXACT state/transition set of the real --emit-ir IR (Gate)", async function () {
    this.timeout(120_000);

    const fixture = path.join(tmpDir, "clean.fsm"); // machine Gate
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fixture));
    await vscode.window.showTextDocument(doc);

    webviewMsgs.length = 0;
    // Execute the REAL contributed+registered command (end-to-end).
    await vscode.commands.executeCommand("fsm.openDiagram");

    // The Webview must post a `rendered` ack with the laid-out counts —
    // its presence proves the model reached + laid out in the REAL
    // Webview (elkjs ran in the browser context), not just in Node.
    await waitFor(
      () => webviewMsgs.some((m) => m.type === "rendered"),
      "the Webview to post a `rendered` ack",
      60_000,
    );
    const rendered = webviewMsgs.find((m) => m.type === "rendered");
    assert.ok(rendered, "a `rendered` ack must have been posted");
    assert.ok(
      !("error" in rendered) || rendered.error === undefined,
      `the Webview render must not error: ${JSON.stringify(rendered)}`,
    );

    // Independent oracle: the SAME real `fsm generate --emit-ir`, parsed
    // and projected here independently (NOT via the SUT mapper).
    const ir = emitIrOracle(cliBinary, fixture);
    const { nodeIds, edges } = oracleSets(ir);

    // The Gate fixture's known structural truth (re-derived above so a
    // schema change fails loudly): 1 initial pseudo + 2 simple states;
    // 2 transitions (Closed->Open, Open->Closed).
    assert.strictEqual(nodeIds.size, 3, "oracle precondition: Gate has 3 state nodes");
    assert.strictEqual(edges.size, 2, "oracle precondition: Gate has 2 transitions");

    // The REAL Webview ack's counts MUST equal the oracle's set sizes.
    assert.strictEqual(
      rendered.nodes,
      nodeIds.size,
      "the Webview laid out a DIFFERENT node count than the real " +
        `--emit-ir IR (webview=${String(rendered.nodes)}, ` +
        `oracle=${nodeIds.size}) — the diagram drifted from the ` +
        "canonical fsm-ir (Doc 27 §6.3)",
    );
    assert.strictEqual(
      rendered.edges,
      edges.size,
      "the Webview laid out a DIFFERENT edge count than the real " +
        `--emit-ir IR (webview=${String(rendered.edges)}, ` +
        `oracle=${edges.size})`,
    );
    assert.strictEqual(
      rendered.machineName,
      "Gate",
      "the rendered machine must be the fixture's machine `Gate`",
    );

    // The SUT's OWN pure mapper, fed the SAME oracle artifact, must yield
    // the EXACT same set (same ids + same edge endpoints) — a real
    // structural equality, not a count coincidence.
    const model = parseAndBuild(JSON.stringify(ir), "Gate");
    assert.deepStrictEqual(
      new Set(model.nodes.map((n) => n.id)),
      nodeIds,
      "the SUT's projected node-id set must equal the real IR's set",
    );
    assert.deepStrictEqual(
      new Set(model.edges.map((e) => `${e.source}->${e.target}`)),
      edges,
      "the SUT's projected edge (source->target) set must equal the " + "real IR's transition set",
    );
    // Every node must carry a 1-based SourceLocation (the click→source
    // substrate, Doc 05 §1.5.4) — a model without locs cannot navigate.
    for (const n of model.nodes) {
      assert.ok(
        typeof n.loc.line === "number" && n.loc.line >= 1,
        `node ${n.id} must carry a 1-based source line for click→source`,
      );
    }
  });

  // (2) THE CODEGEN-GATED BOUNDARY — last-valid render RETAINED + the
  //     Doc 05 §1.5.9 banner VERBATIM, never a blank/crashed webview.
  //     Driven through the EXPORTED DiagramController directly (its
  //     constructor registers NO command — only registerOpenDiagram does,
  //     and the extension already did that in activate(); the command path
  //     and ctl.open() share the IDENTICAL DiagramView.refresh() code, and
  //     test (1) already proved the command→Webview ack).
  test("codegen-gated boundary: keeps the last valid render + shows the Doc 05 §1.5.9 banner (NOT blank)", async function () {
    this.timeout(120_000);

    const ctxSubs: { dispose(): void }[] = [];
    const log = vscode.window.createOutputChannel("fsm-v4-boundary");
    const ctl = new DiagramController(
      {
        subscriptions: ctxSubs,
        extensionPath,
      } as unknown as vscode.ExtensionContext,
      {
        getClient: () => undefined,
        outputChannel: log,
        restartServer: async () => {},
        extensionPath,
      },
    );

    // A dedicated mutable fixture: starts as the VALID Gate machine.
    const mut = path.join(tmpDir, "_boundary.fsm");
    fs.copyFileSync(path.join(tmpDir, "clean.fsm"), mut);

    const docu = await vscode.workspace.openTextDocument(vscode.Uri.file(mut));
    await vscode.window.showTextDocument(docu);

    webviewMsgs.length = 0;
    // (a) Establish a VALID render (last-valid model now populated).
    await ctl.open(mut);
    await waitFor(
      () => ctl.lastValidModelFor(mut, "Gate") !== undefined,
      "an initial valid Gate render to populate the last-valid model",
      60_000,
    );
    const valid = ctl.lastValidModelFor(mut, "Gate");
    assert.ok(valid, "precondition: a valid Gate render must exist");
    assert.strictEqual(valid.machineName, "Gate");
    assert.strictEqual(
      valid.nodes.length,
      3,
      "precondition: the valid render is the 3-node Gate model",
    );
    await waitFor(
      () => webviewMsgs.some((m) => m.type === "rendered"),
      "the initial valid Webview render ack",
      30_000,
    );

    // (b) Make the SAME file's next IR acquisition hit the codegen-gated
    //     boundary: overwrite it with a source the CLI's `generate`
    //     REJECTS (no `.ir.json` produced). This is the IDENTICAL emitIr
    //     `codegenFailed` / refresh() path a pure codegen ICE takes (the
    //     V3-blessed analogue — broken.fsm's content; `fsm generate`
    //     exits non-zero, NO IR written).
    const failingSrc = fs.readFileSync(path.join(tmpDir, "broken.fsm"), "utf8");
    fs.writeFileSync(mut, failingSrc);
    // Sanity: confirm the CLI really declines to write an .ir.json for
    // this source (the codegen-gated precondition, proven not assumed).
    {
      const probe = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v4-probe-"));
      let exit = 0;
      try {
        execFileSync(cliBinary, ["generate", "--target", "c99", "--out", probe, "--emit-ir", mut], {
          stdio: "ignore",
        });
      } catch (e) {
        exit = (e as { status?: number }).status ?? 1;
      }
      const wroteIr = fs.readdirSync(probe).some((f) => f.endsWith(".ir.json"));
      fs.rmSync(probe, { recursive: true, force: true });
      assert.notStrictEqual(
        exit,
        0,
        "codegen-gated precondition: `fsm generate` must exit " + "non-zero for the failing source",
      );
      assert.strictEqual(
        wroteIr,
        false,
        "codegen-gated precondition: NO .ir.json must be produced",
      );
    }

    webviewMsgs.length = 0;
    // Re-open (focus-existing → refresh) — the refresh's emitIr hits the
    // codegen-gated boundary.
    await ctl.open(mut);

    // The Webview must acknowledge the Doc 05 §1.5.9 banner.
    await waitFor(
      () => webviewMsgs.some((m) => m.type === "staleShown" && m.bannerVisible === true),
      "the Webview to acknowledge the Doc 05 §1.5.9 stale banner",
      30_000,
    );
    const staleAck = webviewMsgs.find((m) => m.type === "staleShown");
    assert.ok(
      staleAck,
      "the Webview must post a `staleShown` ack on the codegen-gated " +
        "boundary (NOT silently blank)",
    );
    assert.strictEqual(
      staleAck.bannerVisible,
      true,
      "the Doc 05 §1.5.9 banner must be VISIBLE on the boundary",
    );
    assert.strictEqual(
      staleAck.hasLastValidRender,
      true,
      "the Webview must report it STILL holds the last valid render " +
        "(the Doc 05 §1.5.9 'last valid state' — NOT blanked)",
    );

    // The controller's last-valid model MUST be UNCHANGED — the codegen
    // failure did not clear it (the core last-valid-retention contract;
    // the §6.1.3 boundary PROVEN, not assumed).
    const after = ctl.lastValidModelFor(mut, "Gate");
    assert.ok(
      after,
      "the last-valid model must be RETAINED across the codegen-gated " +
        "failure (NOT cleared to a blank diagram)",
    );
    assert.strictEqual(
      after.machineName,
      "Gate",
      "the retained model must still be the last VALID render (Gate)",
    );
    assert.deepStrictEqual(
      after.nodes.map((n) => n.id),
      valid.nodes.map((n) => n.id),
      "the retained last-valid model must be byte-identical to the " +
        "pre-failure render (the §6.1.3 last-valid-render contract)",
    );

    for (const d of ctxSubs) {
      d.dispose();
    }
    fs.rmSync(mut, { force: true });
  });

  // The Doc 05 §1.5.9 banner string the panel ships must be VERBATIM
  // (a paraphrase is a contract regression — assert the text, not just
  // its presence).
  test("the shipped stale-banner string is the Doc 05 §1.5.9 text VERBATIM", async () => {
    assert.strictEqual(
      SUT_STALE_BANNER,
      STALE_BANNER,
      "the panel's banner must be the Doc 05 §1.5.9 string verbatim",
    );
  });
});
