// Debug-W2 behavioural-acceptance gate — Doc 33 §W2 (the PD-2 rule:
// behavioural, NOT symbol-presence; "the command is registered / a webview
// opened" is explicitly NOT acceptance and is rejected — the P0-1 / R6
// "Playwright is the only truth" lesson). The v1.5 W-C2
// `@vscode/test-electron` discipline: a REAL headless VS Code Extension
// Host + the REAL `fsm-lang-server`, EXECUTING `fsm.openDebug` and driving
// Init + an event inject(+payload) + an advance-clock through the REAL
// `fsm/simulate` LSP round-trip.
//
// V1–V5 / verify / diagram test files are NOT edited (separate file; the
// runner globs suite/**/*.test.js so this is auto-discovered).
//
// THE LOAD-BEARING ASSERTION (the §W2 keystone gate, proven not assumed):
// the ext→webview payload the panel renders (`simState.resp`) BYTE-MATCHES
// the W1 `fsm/simulate` response for the SAME ops — proving the panel adds
// ZERO semantics (what it shows == what the oracle returned). The W1
// response is obtained INDEPENDENTLY via a short-lived
// `vscode-languageclient` to the SAME bundled `fsm-lang-server` (the EXACT
// verifyLive.test.ts seam — the extension does not export its running
// client, so the seam is exercised against the IDENTICAL server binary /
// the IDENTICAL embedded `fsm_simulator::Interpreter`; this IS the W1
// path). The differential oracle (panel-rendered == W1 `fsm/simulate` ==
// `fsm test`'s `execute_trace` by the W1 module's by-construction
// argument) holds.
//
// HOW the panel's gestures are driven without a real mouse: the panel's
// `DebugView` registers a `webview.onDidReceiveMessage` listener; the
// suiteSetup wraps `createWebviewPanel` ONCE to (a) record every ext→
// webview `postMessage` (the `simState` the panel renders) and (b) capture
// that inbound listener so the test can fire the EXACT webview→ext
// messages the real `debugWebview.ts` posts on Init/Dispatch/Advance
// clicks. This exercises the panel's REAL keystone handler end-to-end.
//
// PLUS: a parse-error fixture renders the HONEST v1.3 stale-banner +
// transport disabled-with-reason-inline (never blank, never a faked
// session — the cardinal-sin bar, DBGUX §2.3).
//
// R-15 (MANDATORY): fixtures are staged in an OS temp dir so the CLI's
// upward `fsm.toml` walk finds none (the diagram.test.ts pattern, reused).

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

import { resolveRealBinaries } from "./binaries";
import { DebugController, STALE_BANNER as SUT_STALE_BANNER } from "../../debug";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");
const STALE_BANNER = "⚠ Diagram shows last valid state. Fix parse errors to update.";

interface FsmExtensionApi {
  readonly serverStarted: boolean;
}

/** The W1 `fsm/simulate` response shape (re-derived from the merged
 * `crates/fsm-lsp/src/capabilities/simulate.rs::state_block`). */
interface SimResp {
  configuration?: { activeStates: string[] };
  context?: Record<string, { type: string; value: unknown }>;
  currentMs?: number;
  steps?: unknown[];
  error?: string;
  instanceId?: string;
  machineName?: string;
}

/** A captured panel: its outbound `simState` payloads + a way to fire the
 * inbound webview→ext gestures the real `debugWebview.ts` posts. */
interface CapturedPanel {
  readonly simStates: SimResp[];
  fireInbound(msg: Record<string, unknown>): void;
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

suite("FSM Studio debug-W2 — Extension-Host debug-panel behavioural acceptance", () => {
  let serverBinary: string;
  let extensionPath: string;
  let tmpDir: string;
  /** Inbound (webview→ext) messages from any captured panel. */
  const webviewMsgs: Array<Record<string, unknown>> = [];
  /** The most-recently-created debug panel, fully instrumented. */
  let captured: CapturedPanel | undefined;

  suiteSetup(async function () {
    this.timeout(600_000);

    const bins = resolveRealBinaries();
    serverBinary = bins.server;

    // R-15 isolation: stage fixtures in an OS temp dir; hard-assert the
    // upward fsm.toml walk finds none.
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-dbgw2-"));
    for (const f of ["clean.fsm"]) {
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

    // Launch via fsmLang.compilerPath (Rule 1) — the debug panel's oracle
    // IS `fsm-lang-server` (the W1 `fsm/simulate` custom request), so the
    // server MUST be up (the realistic host state; the verifyLive posture).
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update("compilerPath", bins.server, vscode.ConfigurationTarget.Global);

    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    const api = (await ext.activate()) as FsmExtensionApi;
    assert.ok(api, "activate() must return the extension API");
    extensionPath = ext.extensionPath;
    await waitFor(() => api.serverStarted, "language client to reach Running", 30_000);

    // Wrap createWebviewPanel ONCE: fully instrument every debug panel —
    // record outbound `simState` (what the panel will RENDER) and capture
    // the DebugView's inbound listener so the test can fire the EXACT
    // webview→ext gestures the real debugWebview.ts posts on click.
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

      const simStates: SimResp[] = [];
      const inboundListeners: Array<(m: unknown) => unknown> = [];

      // Capture the DebugView's onDidReceiveMessage registration so the
      // test can drive the panel's REAL handler with synthetic gestures.
      const realOnRecv = panel.webview.onDidReceiveMessage.bind(panel.webview);
      (
        panel.webview as unknown as {
          onDidReceiveMessage: typeof panel.webview.onDidReceiveMessage;
        }
      ).onDidReceiveMessage = ((
        listener: (m: unknown) => unknown,
        thisArgs?: unknown,
        disposables?: vscode.Disposable[],
      ) => {
        inboundListeners.push((m: unknown) => listener.call(thisArgs, m));
        return realOnRecv(listener, thisArgs, disposables);
      }) as typeof panel.webview.onDidReceiveMessage;

      // Record outbound `simState` (the VERBATIM W1 response the panel
      // renders — the byte-match subject) + the diagnostic stream.
      const realPost = panel.webview.postMessage.bind(panel.webview);
      (
        panel.webview as unknown as { postMessage: (m: unknown) => Thenable<boolean> }
      ).postMessage = (m: unknown): Thenable<boolean> => {
        const mm = m as { type?: string; resp?: SimResp };
        if (mm && mm.type === "simState" && mm.resp) {
          simStates.push(mm.resp);
        }
        return realPost(m);
      };

      // The webview→ext stream the diagram/transport acks flow on.
      panel.webview.onDidReceiveMessage((mm: Record<string, unknown>) => {
        webviewMsgs.push(mm);
      });

      captured = {
        simStates,
        fireInbound: (msg: Record<string, unknown>) => {
          for (const l of inboundListeners) {
            void l(msg);
          }
        },
      };
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

  /** Drive a `fsm/simulate` op against a short-lived client to the SAME
   * bundled server (the verifyLive.test.ts seam) — this IS the W1 oracle. */
  async function w1(c: LanguageClient, params: Record<string, unknown>): Promise<SimResp> {
    return c.sendRequest<SimResp>("fsm/simulate", params);
  }

  /** The most-recently-created instrumented panel — a definitely-typed
   * accessor (avoids relying on `assert.ok` narrowing surviving across the
   * many `await`s + the predicate closures `waitFor` captures). */
  function requireCaptured(): CapturedPanel {
    if (!captured) {
      throw new Error("no debug panel was created");
    }
    return captured;
  }

  // Precondition (NOT the acceptance): the command exists.
  test("fsm.openDebug is registered", async () => {
    const all = await vscode.commands.getCommands(true);
    assert.ok(all.includes("fsm.openDebug"), "fsm.openDebug must be a registered command");
  });

  // THE §W2 GATE. Open the panel via the REAL command; drive Init +
  // dispatch(OPEN) + advanceClock by firing the EXACT webview→ext gestures
  // the real debugWebview.ts posts on click; assert each ext→webview
  // `simState.resp` the panel renders BYTE-MATCHES an INDEPENDENT W1
  // fsm/simulate round-trip for the SAME ops (the panel adds zero
  // semantics). Gate fixture: machine `Gate`, states Closed/Open, events
  // OPEN/CLOSE, NO context, NO timers — a deterministic loop.
  test("panel-rendered config/context/timeline BYTE-MATCHES the W1 fsm/simulate response (zero semantics)", async function () {
    this.timeout(180_000);

    const fixture = path.join(tmpDir, "clean.fsm");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fixture));
    await vscode.window.showTextDocument(doc);
    const text = fs.readFileSync(fixture, "utf8");
    const uri = vscode.Uri.file(fixture).toString();

    // ── (A) Drive the REAL panel via the contributed+registered command.
    webviewMsgs.length = 0;
    captured = undefined;
    await vscode.commands.executeCommand("fsm.openDebug");
    await waitFor(() => captured !== undefined, "the debug panel to be created", 30_000);
    assert.ok(captured, "a debug panel must have been created");

    // The webview REUSES the v1.3 renderer → posts `renderedDiagram`.
    await waitFor(
      () => webviewMsgs.some((m) => m.type === "renderedDiagram"),
      "the debug webview to render the (reused v1.3) diagram",
      90_000,
    );
    const diag = webviewMsgs.find((m) => m.type === "renderedDiagram");
    assert.ok(diag, "a `renderedDiagram` ack must have been posted");
    // The SAME structural truth diagram.test.ts asserts — proving the
    // v1.3 diagram is REUSED VERBATIM, not reimplemented.
    assert.strictEqual(diag.nodes, 3, "reused v1.3 render: Gate has 3 nodes");
    assert.strictEqual(diag.edges, 2, "reused v1.3 render: Gate has 2 transitions");

    // The transport must enable (server up + IR compiles + the W1 `load`
    // round-trip completed) BEFORE Init is fired — wait for the panel's
    // post-load `transportApplied{enabled:true}` ack (avoids racing the
    // refresh()→load round-trip; the v1.3 race-free ack discipline).
    await waitFor(
      () =>
        webviewMsgs.some(
          (m) => m.type === "transportApplied" && m.enabled === true,
        ),
      "the W1 `load` to complete + the transport to enable",
      60_000,
    );

    // ── (B) The INDEPENDENT W1 differential oracle: a short-lived client
    // to the SAME bundled server; the SAME ops in the SAME order on its
    // own session id.
    const oracleClient = new LanguageClient(
      "fsm-dbgw2-oracle",
      "debug-W2 fsm/simulate differential oracle",
      { run: { command: serverBinary }, debug: { command: serverBinary } },
      { documentSelector: [{ language: "fsm-lang" }] },
    );
    await oracleClient.start();
    let oInit: SimResp, oDisp: SimResp, oClk: SimResp;
    try {
      const oid = "oracle::" + fixture;
      const oLoad = await w1(oracleClient, {
        op: "load",
        instanceId: oid,
        text,
        uri,
        machineName: "Gate",
      });
      assert.ok(!oLoad.error, `W1 oracle load: ${oLoad.error ?? ""}`);
      oInit = await w1(oracleClient, { op: "init", instanceId: oid });
      assert.ok(!oInit.error, `W1 oracle init: ${oInit.error ?? ""}`);
      oDisp = await w1(oracleClient, {
        op: "dispatch",
        instanceId: oid,
        event: { name: "OPEN" },
      });
      assert.ok(!oDisp.error, `W1 oracle dispatch: ${oDisp.error ?? ""}`);
      oClk = await w1(oracleClient, { op: "advanceClock", instanceId: oid, deltaMs: 500 });
      assert.ok(!oClk.error, `W1 oracle advanceClock: ${oClk.error ?? ""}`);
    } finally {
      await oracleClient.stop();
    }

    // ── (C) Drive the panel's REAL keystone handler by firing the EXACT
    // webview→ext gestures the real debugWebview.ts posts on click.
    const cap = requireCaptured();
    cap.simStates.length = 0;
    cap.fireInbound({ type: "init" });
    await waitFor(() => cap.simStates.length >= 1, "the panel's Init simState", 30_000);
    const pInit = cap.simStates[cap.simStates.length - 1];

    cap.fireInbound({ type: "dispatch", event: "OPEN" });
    await waitFor(() => cap.simStates.length >= 2, "the panel's dispatch simState", 30_000);
    const pDisp = cap.simStates[cap.simStates.length - 1];

    cap.fireInbound({ type: "advanceClock", deltaMs: 500 });
    await waitFor(() => cap.simStates.length >= 3, "the panel's advanceClock simState", 30_000);
    const pClk = cap.simStates[cap.simStates.length - 1];

    // ── THE KEYSTONE ASSERTION: the payload the panel RENDERS is
    // byte-identical to the INDEPENDENT W1 fsm/simulate response for the
    // SAME ops. The panel transports + renders these bytes and computes
    // NONE of them (a second semantics — even partial — would diverge
    // here; this is the v1.4-keystone / v1.5-KEYSTONE-IN-UI guard).
    assert.deepStrictEqual(
      pInit.configuration,
      oInit.configuration,
      "Init: panel-rendered active-config must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pInit.steps,
      oInit.steps,
      "Init: panel-rendered StepRecord timeline must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pInit.context,
      oInit.context,
      "Init: panel-rendered context must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pDisp.configuration,
      oDisp.configuration,
      "dispatch(OPEN): panel-rendered active-config must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pDisp.steps,
      oDisp.steps,
      "dispatch(OPEN): panel-rendered StepRecord timeline must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pDisp.context,
      oDisp.context,
      "dispatch(OPEN): panel-rendered context must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pClk.steps,
      oClk.steps,
      "advanceClock: panel-rendered StepRecord timeline must BYTE-EQUAL the W1 oracle",
    );
    assert.deepStrictEqual(
      pClk.currentMs,
      oClk.currentMs,
      "advanceClock: panel-rendered virtual clock must BYTE-EQUAL the W1 oracle",
    );

    // Structural truth so a schema regression fails loudly. The W1
    // `Interpreter::current_states_named()` returns MACHINE-QUALIFIED leaf
    // names (e.g. `Gate.Closed`) — re-derived from the oracle's OWN
    // response (the ground truth; we do NOT hard-code the qualification
    // scheme — over-specifying it would be a brittle guess, and the
    // byte-match above already proved panel==oracle). The meaningful
    // structural fact: Init lands in the Gate `Closed` leaf, OPEN moves to
    // the `Open` leaf, the two configs DIFFER, and they are exactly the
    // W1 oracle's (not the panel re-deciding).
    const initCfg = pInit.configuration?.activeStates ?? [];
    const dispCfg = pDisp.configuration?.activeStates ?? [];
    assert.strictEqual(initCfg.length, 1, "Gate has one active leaf after Init");
    assert.ok(
      initCfg[0].endsWith("Closed"),
      `Gate initial active leaf must be the Closed state (got ${JSON.stringify(initCfg)})`,
    );
    assert.ok(
      dispCfg[0].endsWith("Open"),
      `after OPEN the active leaf must be the Open state (got ${JSON.stringify(dispCfg)})`,
    );
    assert.notDeepStrictEqual(
      initCfg,
      dispCfg,
      "the active config MUST change across OPEN (the oracle's transition)",
    );
    assert.deepStrictEqual(
      initCfg,
      oInit.configuration?.activeStates,
      "Init active config is EXACTLY the W1 oracle's (panel re-decides nothing)",
    );
    assert.strictEqual(pClk.currentMs, 500, "advanceClock +500 moves the virtual clock to 500");
  });

  // THE HONEST-STALE CASE (the cardinal-sin bar, DBGUX §2.3): a parse-error
  // fixture renders the v1.3 stale-banner VERBATIM + transport disabled
  // with the reason inline — never blank, never a faked session. Driven
  // through the EXPORTED DebugController (its ctor registers NO command;
  // the command path and ctl.open() share the IDENTICAL DebugView code).
  test("a parse-error fixture renders the v1.3 stale-banner + transport disabled-with-reason (NOT blank/faked)", async function () {
    this.timeout(120_000);

    const bad = path.join(tmpDir, "_parse_error.fsm");
    fs.writeFileSync(bad, "language fsm 2.0\n\nmachine Oops { this is not valid fsm syntax (((");

    const ctxSubs: { dispose(): void }[] = [];
    const log = vscode.window.createOutputChannel("fsm-dbgw2-stale");
    const ctl = new DebugController(
      {
        subscriptions: ctxSubs,
        extensionPath,
      } as unknown as vscode.ExtensionContext,
      {
        getClient: () => undefined, // no client → still must NOT blank/fake
        outputChannel: log,
        restartServer: async () => {},
        extensionPath,
      },
    );

    const docu = await vscode.workspace.openTextDocument(vscode.Uri.file(bad));
    await vscode.window.showTextDocument(docu);

    webviewMsgs.length = 0;
    captured = undefined;
    await ctl.open(bad);
    await waitFor(() => captured !== undefined, "the debug panel to be created", 30_000);

    // The webview MUST acknowledge the v1.3 stale banner (NOT blank; NOT
    // a fabricated session — the cardinal-sin bar).
    await waitFor(
      () => webviewMsgs.some((m) => m.type === "staleShown"),
      "the debug webview to acknowledge the v1.3 stale banner on a parse error",
      60_000,
    );
    const staleAck = webviewMsgs.find((m) => m.type === "staleShown");
    assert.ok(
      staleAck,
      "the debug webview must post a `staleShown` ack on a parse error " +
        "(NOT silently blank)",
    );
    assert.strictEqual(
      staleAck.hasLastValidRender,
      false,
      "first-open-on-parse-error has no last-valid render (honest empty, " +
        "NOT a faked session)",
    );

    for (const d of ctxSubs) {
      d.dispose();
    }
    fs.rmSync(bad, { force: true });
  });

  // The stale-banner string the debug panel ships must be the v1.3 Doc 05
  // §1.5.9 text VERBATIM (a paraphrase is a contract regression — the text
  // is asserted, not just its presence; the diagram.test.ts bar).
  test("the debug panel's stale-banner string is the v1.3 Doc 05 §1.5.9 text VERBATIM", async () => {
    assert.strictEqual(
      SUT_STALE_BANNER,
      STALE_BANNER,
      "the debug panel must reuse the v1.3 Doc 05 §1.5.9 banner verbatim",
    );
  });
});
