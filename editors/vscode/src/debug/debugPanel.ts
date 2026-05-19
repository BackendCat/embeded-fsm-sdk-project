// The debug `WebviewPanel` (Doc 33 §W2; DBGUX §2 layout) — the v1.3
// statechart diagram REUSED VERBATIM + an additive overlay + the control
// rail, driven entirely over the W1 `fsm/simulate` LSP layer.
//
// THE KEYSTONE (Doc 33 §2 — the load-bearing structural invariant): this
// panel adds NO FSM semantics. Every debug verb (Init / Inject(+payload) /
// Advance-clock / Inspect) marshals to exactly one `fsm/simulate` op via
// `simulateClient.ts` and RENDERS the response. It never computes which
// transition fires, the active configuration, which timers fired, or
// whether an event was discarded — those are 100% the W1 `Interpreter`'s
// answers, read straight off the response. The W4 keystone audit will
// negative-grep this surface for transition/guard/step/active-config logic
// and expect ∅. A second simulator semantics is the cardinal regression.
//
// REUSE, NOT REINVENT (Doc 33 §W2 reuse ledger; DBGUX §4.1 "no second
// diagram"): the statechart is the v1.3 `emitIr` (the codegen-gated IR
// boundary) + the v1.3 `irGraph.ts` `parseAndBuild` model + the v1.3
// `diagramWebview.ts` ELK/SVG `renderModel` (reused VERBATIM by the debug
// webview bundle — see `webview/debugWebview.ts`) + the v1.3
// `diagramPanel.ts` panel-identity / stale-banner / click→source contract
// (mirrored here: keyed by RESOLVED FILE PATH so a transient parse error
// does not orphan the panel; the VERBATIM Doc 05 §1.5.9 banner; transport
// disabled-with-inline-reason; never blanks, never fakes a session — the
// v1.3 cardinal-sin bar extended to the transport, DBGUX §2.3).
//
// SCOPE (Doc 33 §W2, MVP-core, Item-3 DEFERRED ⇒ NO queue/data-plane
// surface): transport rail (Init/Run/Pause/Step), the event+payload
// injector (payload form revealed inline under the picker — the
// proximity principle, DBGUX §2.2), the clock-advancer + pending-timer
// list, the context Δ-inspector, the StepRecord timeline DISPLAY.
// Breakpoint glyphs are PLACEHOLDERS only; breakpoint behaviour +
// time-travel rewind are W3. NO breakpoint predicate, NO rewind here.
//
// SECURITY: the v1.3 strict-CSP + per-render CSPRNG nonce contract,
// reused verbatim (the bundled debug webview is the only script;
// `default-src 'none'`; `postMessage` JSON only).

import { randomBytes } from "crypto";
import * as path from "path";

import * as vscode from "vscode";

import { CommandDeps } from "../commands/index";
import { emitIr } from "../diagram/emitIr";
import { DiagramModel, IrGraphError, parseAndBuild } from "../diagram/irGraph";
import { simulate, SimResponse, SimTransportError } from "./simulateClient";

/** The Doc 05 §1.5.9 stale banner — VERBATIM, reused from the v1.3
 * contract (a regression of this string fails acceptance — the text is
 * asserted, not paraphrased; the same bar `diagramPanel.ts` holds). */
export const STALE_BANNER = "⚠ Diagram shows last valid state. Fix parse errors to update.";

/** Messages the debug webview posts back to the extension (the only
 * inbound surface — a tight, typed boundary, the v1.3 discipline). The
 * webview decides NO FSM semantics; each is a user gesture the extension
 * marshals to a W1 `fsm/simulate` op. */
type DebugWebviewToExt =
  | { readonly type: "ready" }
  | { readonly type: "revealSource"; readonly line: number; readonly column: number }
  | { readonly type: "init" }
  | {
      readonly type: "dispatch";
      readonly event: string;
      readonly payload?: Record<string, unknown>;
    }
  | { readonly type: "advanceClock"; readonly deltaMs: number }
  // Test-observability acks (NOT acceptance on their own — the §W2 gate is
  // the panel-vs-oracle byte-match below; these let the ExtHost E2E await
  // a deterministic point, the v1.3 `rendered`/`staleShown` ack pattern).
  | { readonly type: "renderedDiagram"; readonly nodes: number; readonly edges: number }
  | { readonly type: "transportApplied"; readonly enabled: boolean }
  | { readonly type: "stateApplied"; readonly stamp: number }
  | { readonly type: "staleShown"; readonly hasLastValidRender: boolean };

/** Messages the extension posts into the debug webview. The webview
 * RENDERS these; it computes nothing (the keystone). */
type ExtToDebugWebview =
  | { readonly type: "render"; readonly model: DiagramModel }
  | { readonly type: "staleBanner"; readonly text: string }
  | {
      readonly type: "transport";
      readonly enabled: boolean;
      readonly reason: string;
      readonly events: string[];
    }
  // The VERBATIM W1 `fsm/simulate` response — the webview displays the
  // active config / context / StepRecord timeline EXACTLY as carried here
  // (zero recomputation; the byte-fidelity the §W2 gate proves).
  | { readonly type: "simState"; readonly resp: SimResponse; readonly stamp: number }
  | { readonly type: "simError"; readonly message: string };

/** Pull the declared event names out of the IR JSON for the injector's
 * event picker. This is a PURE structural read of the same `--emit-ir`
 * document the diagram renders (NOT a semantics decision — it does not
 * choose which event is enabled; the user picks, the W1 oracle decides the
 * effect). Mirrors the `irGraph.ts` "read the canonical IR" discipline. */
function eventNamesFromIr(irJson: string): string[] {
  try {
    const doc = JSON.parse(irJson) as {
      machines?: Array<{ events?: Array<{ name?: string }> }>;
    };
    const names = new Set<string>();
    for (const m of doc.machines ?? []) {
      for (const e of m.events ?? []) {
        if (typeof e.name === "string" && e.name.length > 0) {
          names.add(e.name);
        }
      }
    }
    return [...names].sort();
  } catch {
    // A malformed IR is handled by the diagram path's honest stale-banner;
    // the picker just shows no events (never a fabricated list).
    return [];
  }
}

/**
 * Owns one `.fsm` file's debug panel + its live session.
 *
 * IDENTITY (the v1.3 §1.5.9 contract, reused): keyed by the RESOLVED FILE
 * PATH, not the machine name — a transient parse/codegen error removes the
 * machine from the IR; a machine-name key would orphan the panel and spawn
 * a blank one instead of focusing the existing one and showing
 * last-valid + banner. The `instanceId` for the W1 session is the resolved
 * path (one debug session per panel — the W1 session map is keyed by it).
 */
class DebugView {
  private lastValidModel: DiagramModel | undefined;
  private machineName: string | undefined;
  private disposed = false;
  private webviewReady = false;
  /** Monotonic stamp so the ExtHost E2E can await the EXACT applied
   * response (not a stale one). Never carries semantics. */
  private stamp = 0;
  /** `true` once a `load`+`init` succeeded — gates the inject/clock verbs
   * (an honest precondition, never a faked step). */
  private initialized = false;

  constructor(
    readonly fsmPath: string,
    private readonly panel: vscode.WebviewPanel,
    private readonly deps: CommandDeps,
  ) {
    panel.webview.onDidReceiveMessage((m: DebugWebviewToExt) => {
      void this.onMessage(m);
    });
    panel.onDidDispose(() => {
      this.disposed = true;
      // Best-effort: drop the W1 session (pure plumbing; no semantics).
      void simulate(this.deps.getClient(), {
        op: "unload",
        instanceId: this.instanceId(),
      }).catch(() => undefined);
    });
  }

  /** The W1 session key — the resolved file path (one session per panel). */
  private instanceId(): string {
    return path.resolve(this.fsmPath);
  }

  reveal(): void {
    this.panel.reveal(vscode.ViewColumn.Beside, false);
  }

  isDisposed(): boolean {
    return this.disposed;
  }

  /** Test observability — the last valid rendered model (the v1.3 bar). */
  getLastValidModel(): DiagramModel | undefined {
    return this.lastValidModel;
  }

  getMachineName(): string | undefined {
    return this.machineName;
  }

  private post(m: ExtToDebugWebview): void {
    if (!this.disposed) {
      void this.panel.webview.postMessage(m);
    }
  }

  private async onMessage(m: DebugWebviewToExt): Promise<void> {
    if (!m || typeof m.type !== "string") {
      return;
    }
    switch (m.type) {
      case "ready":
        this.webviewReady = true;
        await this.refresh();
        break;
      case "revealSource":
        await this.revealSource(m.line, m.column);
        break;
      case "init":
        await this.doInit();
        break;
      case "dispatch":
        await this.doDispatch(m.event, m.payload);
        break;
      case "advanceClock":
        await this.doAdvanceClock(m.deltaMs);
        break;
      default:
        // Acks (renderedDiagram/stateApplied/staleShown) are observed by
        // the ExtHost test via the panel message stream; nothing to do.
        break;
    }
  }

  /**
   * Re-acquire the IR (the v1.3 codegen-gated `emitIr` boundary, REUSED)
   * and (re-)render the diagram + (re-)compute the transport-enable state.
   * On a codegen-gated failure: keep the last valid render + the VERBATIM
   * stale banner + DISABLE the transport with the reason inline — NEVER
   * blank, NEVER fake a session (the v1.3 cardinal-sin bar, extended to
   * the transport per DBGUX §2.3). This decides NO FSM semantics.
   */
  async refresh(): Promise<void> {
    if (this.disposed || !this.webviewReady) {
      return;
    }
    const res = await emitIr(
      this.fsmPath,
      this.deps.extensionPath,
      vscode.workspace.getConfiguration("fsmLang"),
      this.deps.outputChannel,
    );

    if (!res.ok) {
      // No valid IR: the honest stale state, transport disabled WITH the
      // reason inline (DBGUX §2.3; the v1.3 contract extended to the rail).
      this.deps.outputChannel.appendLine(
        `[fsm] debug: IR unavailable (${res.reason}: ${res.detail}) — ` +
          "keeping last valid render + stale banner; transport disabled.",
      );
      this.post({ type: "staleBanner", text: STALE_BANNER });
      this.post({
        type: "transport",
        enabled: false,
        reason: "Cannot simulate: fix parse errors",
        events: [],
      });
      this.initialized = false;
      return;
    }

    let model: DiagramModel;
    try {
      model = parseAndBuild(res.json);
    } catch (e) {
      const detail = e instanceof IrGraphError ? e.message : String(e);
      this.deps.outputChannel.appendLine(
        `[fsm] debug: IR projection failed (${detail}) — keeping last ` +
          "valid render + stale banner; transport disabled.",
      );
      this.post({ type: "staleBanner", text: STALE_BANNER });
      this.post({
        type: "transport",
        enabled: false,
        reason: "Cannot simulate: fix parse errors",
        events: [],
      });
      this.initialized = false;
      return;
    }

    this.lastValidModel = model;
    if (this.machineName !== model.machineName) {
      this.machineName = model.machineName;
      this.panel.title = `⬡ ${model.machineName} — Debug`;
    }
    this.post({ type: "render", model });

    // Register / refresh the W1 session for this buffer (the `load` op —
    // its ONLY semantics is `Interpreter::new`; the rest is plumbing). A
    // load `error` means the model does not compile → honest disabled
    // transport, never a faked session.
    const doc = await this.readDoc();
    let loadResp: SimResponse;
    try {
      loadResp = await simulate(this.deps.getClient(), {
        op: "load",
        instanceId: this.instanceId(),
        text: doc.text,
        uri: doc.uri,
        machineName: model.machineName,
      });
    } catch (e) {
      const msg = e instanceof SimTransportError ? e.message : String(e);
      this.post({
        type: "transport",
        enabled: false,
        reason: `Cannot simulate: ${msg}`,
        events: [],
      });
      this.initialized = false;
      return;
    }
    if (loadResp.error) {
      this.post({
        type: "transport",
        enabled: false,
        reason: `Cannot simulate: ${loadResp.error}`,
        events: [],
      });
      this.initialized = false;
      return;
    }

    // Transport enabled — Init is the gate (DBGUX §2.3 "Press Init to
    // instantiate the machine"). The event picker is a PURE structural
    // read of the IR (the user picks; the W1 oracle decides the effect).
    this.post({
      type: "transport",
      enabled: true,
      reason: this.initialized ? "" : "Press Init to instantiate the machine.",
      events: eventNamesFromIr(res.json),
    });
    this.deps.outputChannel.appendLine(
      `[fsm] debug: rendered ${model.machineName} ` +
        `(${model.nodes.length} nodes, ${model.edges.length} edges); ` +
        "W1 session ready.",
    );
  }

  /** Read the panel's `.fsm` text + uri (the `load`/re-`load` input). */
  private async readDoc(): Promise<{ text: string; uri: string }> {
    const uri = vscode.Uri.file(this.fsmPath);
    const doc = await vscode.workspace.openTextDocument(uri);
    return { text: doc.getText(), uri: uri.toString() };
  }

  /** Apply a VERBATIM W1 `fsm/simulate` response to the webview. The
   * panel RENDERS it; it recomputes nothing (the keystone — the §W2 gate
   * asserts the rendered config/context/timeline byte-equals this). */
  private applyResponse(resp: SimResponse): void {
    if (resp.error) {
      // Honest: the StepError / invalid reason VERBATIM (never a
      // fabricated clean end-of-run — the "inconclusive ≠ done" bar).
      this.post({ type: "simError", message: resp.error });
      this.deps.outputChannel.appendLine(`[fsm] debug: fsm/simulate error: ${resp.error}`);
      return;
    }
    this.stamp += 1;
    this.post({ type: "simState", resp, stamp: this.stamp });
  }

  private async doInit(): Promise<void> {
    let resp: SimResponse;
    try {
      // ── the keystone in one place: the panel verb → ONE fsm/simulate
      // op → render. The panel does NOT compute the initial config or
      // the StepKind::Init record — the W1 Interpreter::init does; we
      // render `resp` verbatim.
      resp = await simulate(this.deps.getClient(), {
        op: "init",
        instanceId: this.instanceId(),
      });
    } catch (e) {
      this.post({
        type: "simError",
        message: e instanceof SimTransportError ? e.message : String(e),
      });
      return;
    }
    if (!resp.error) {
      this.initialized = true;
    }
    this.applyResponse(resp);
  }

  private async doDispatch(event: string, payload?: Record<string, unknown>): Promise<void> {
    if (!this.initialized) {
      this.post({
        type: "simError",
        message: "press Init to instantiate the machine before injecting an event.",
      });
      return;
    }
    let resp: SimResponse;
    try {
      // panel verb → ONE fsm/simulate op → render. Which transition (if
      // any) fires, whether the event is discarded, what timers chain —
      // ALL the W1 oracle's answers; we render `resp` verbatim.
      resp = await simulate(this.deps.getClient(), {
        op: "dispatch",
        instanceId: this.instanceId(),
        event:
          payload && Object.keys(payload).length > 0 ? { name: event, payload } : { name: event },
      });
    } catch (e) {
      this.post({
        type: "simError",
        message: e instanceof SimTransportError ? e.message : String(e),
      });
      return;
    }
    this.applyResponse(resp);
  }

  private async doAdvanceClock(deltaMs: number): Promise<void> {
    if (!this.initialized) {
      this.post({
        type: "simError",
        message: "press Init to instantiate the machine before advancing the clock.",
      });
      return;
    }
    let resp: SimResponse;
    try {
      // panel verb → ONE fsm/simulate op → render. Which timers fired is
      // the W1 oracle's answer (read off `resp.steps`); we compute nothing.
      resp = await simulate(this.deps.getClient(), {
        op: "advanceClock",
        instanceId: this.instanceId(),
        deltaMs,
      });
    } catch (e) {
      this.post({
        type: "simError",
        message: e instanceof SimTransportError ? e.message : String(e),
      });
      return;
    }
    this.applyResponse(resp);
  }

  /** Reveal a clicked node's declaration (the v1.3 click→source contract,
   * reused verbatim — the IR `SourceLocation` is 1-based, VS Code 0-based). */
  private async revealSource(line1Based: number, column1Based: number): Promise<void> {
    const uri = vscode.Uri.file(this.fsmPath);
    const doc = await vscode.workspace.openTextDocument(uri);
    const editor = await vscode.window.showTextDocument(doc, {
      viewColumn: vscode.ViewColumn.One,
      preserveFocus: false,
    });
    const line = Math.max(0, line1Based - 1);
    const col = Math.max(0, column1Based - 1);
    const pos = new vscode.Position(line, col);
    editor.selection = new vscode.Selection(pos, pos);
    editor.revealRange(
      new vscode.Range(pos, pos),
      vscode.TextEditorRevealType.InCenterIfOutsideViewport,
    );
  }
}

/**
 * The `fsm.openDebug` controller — opens/focuses one debug panel per `.fsm`
 * and keeps it live (re-render + re-load the W1 session on save, the v1.3
 * §1.5.9 contract). The webview HTML is CSP-locked with a per-render
 * CSPRNG nonce; the only resource is the bundled `debugWebview.js`.
 */
export class DebugController {
  /** resolved-file-path → its live view. Keyed by the FILE (NOT the
   * machine name) so the panel + its last-valid render + W1 session
   * survive a transient codegen failure (the v1.3 §1.5.9 contract). */
  private readonly views = new Map<string, DebugView>();

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly deps: CommandDeps,
  ) {
    context.subscriptions.push(
      vscode.workspace.onDidSaveTextDocument((doc) => {
        const view = this.views.get(path.resolve(doc.uri.fsPath));
        if (view && !view.isDisposed()) {
          void view.refresh();
        }
      }),
    );
  }

  /** Open (or focus the existing) debug panel for the given `.fsm`. Panel
   * identity is the RESOLVED FILE PATH (the v1.3 §1.5.1/§1.5.9 contract). */
  async open(fsmPath: string): Promise<void> {
    const key = path.resolve(fsmPath);

    const existing = this.views.get(key);
    if (existing && !existing.isDisposed()) {
      existing.reveal();
      await existing.refresh();
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      "fsmDebug",
      `⬡ ${path.basename(fsmPath)} — Debug`,
      { viewColumn: vscode.ViewColumn.Beside, preserveFocus: false },
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [
          vscode.Uri.file(path.join(this.context.extensionPath, "dist", "webview")),
        ],
      },
    );
    panel.webview.html = this.html(panel.webview);

    const view = new DebugView(fsmPath, panel, this.deps);
    this.views.set(key, view);
    panel.onDidDispose(() => {
      const cur = this.views.get(key);
      if (cur === view) {
        this.views.delete(key);
      }
    });
    // The webview posts `ready` → the view drives the first refresh
    // (the v1.3 race-free first-render contract).
  }

  /** Test-observable: the last-valid model for a file's panel (the v1.3
   * bar — proves the diagram REUSE rendered, not "a panel opened"). */
  lastValidModelFor(fsmPath: string): DiagramModel | undefined {
    return this.views.get(path.resolve(fsmPath))?.getLastValidModel();
  }

  /** Strict CSP webview shell (the v1.3 `diagramPanel.ts` contract, reused
   * verbatim): `default-src 'none'`; the ONLY script is the nonce'd
   * bundled `debugWebview.js`; styles nonce'd inline; no remote anything. */
  private html(webview: vscode.Webview): string {
    const nonce = makeNonce();
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "dist", "webview", "debugWebview.js"),
      ),
    );
    const csp =
      `default-src 'none'; ` +
      `img-src ${webview.cspSource} data:; ` +
      `style-src 'nonce-${nonce}'; ` +
      `script-src 'nonce-${nonce}';`;
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>FSM Debug</title>
<style nonce="${nonce}">
  html,body{margin:0;padding:0;height:100%;overflow:hidden;
    font-family:var(--vscode-font-family);
    color:var(--vscode-foreground);
    background:var(--vscode-editor-background);
    font-size:12px;}
  #shell{display:flex;flex-direction:column;height:100%;}
  /* The stale banner uses a DEBUG-UNIQUE id (#debugBanner), NOT #banner:
     the debug webview inlines the v1.3 diagram renderer, whose standalone
     bootstrap is gated on (#svg && #banner); a #banner here would
     re-trigger that v1.3 auto-bind. See webview/debugWebview.ts. */
  #debugBanner{display:none;padding:6px 12px;
    background:var(--vscode-inputValidation-warningBackground,#5a4a00);
    color:var(--vscode-inputValidation-warningForeground,#e8d36b);
    border-bottom:1px solid var(--vscode-inputValidation-warningBorder,#a8862b);}
  #debugBanner.show{display:block;}
  #transport{padding:6px 12px;display:flex;gap:8px;align-items:center;
    flex-wrap:wrap;border-bottom:1px solid var(--vscode-panel-border,#333);}
  #transport button{font:inherit;cursor:pointer;
    color:var(--vscode-button-foreground);
    background:var(--vscode-button-background);
    border:none;padding:3px 10px;border-radius:3px;}
  #transport button:disabled{opacity:.5;cursor:not-allowed;}
  #clock{color:var(--vscode-descriptionForeground);margin-left:auto;}
  #status{padding:4px 12px;color:var(--vscode-descriptionForeground);
    border-bottom:1px solid var(--vscode-panel-border,#333);
    min-height:16px;}
  #status.err{color:var(--vscode-errorForeground,#f48771);}
  #main{flex:1;display:flex;min-height:0;}
  #left{flex:2;display:flex;flex-direction:column;min-width:0;
    border-right:1px solid var(--vscode-panel-border,#333);}
  #right{flex:1;display:flex;flex-direction:column;min-width:220px;}
  .region-title{padding:4px 10px;font-weight:600;
    color:var(--vscode-descriptionForeground);
    border-bottom:1px solid var(--vscode-panel-border,#333);}
  #diagramWrap{flex:2;position:relative;overflow:auto;min-height:0;}
  #svg{width:100%;height:100%;}
  svg{width:100%;height:100%;}
  .node rect{fill:var(--vscode-editorWidget-background,#252526);
    stroke:var(--vscode-focusBorder,#007fd4);stroke-width:1.5;}
  .node.active rect{stroke:var(--vscode-charts-green,#89d185);
    stroke-width:3;}
  .node text{fill:var(--vscode-foreground);font-size:13px;cursor:pointer;}
  .pseudo circle{fill:var(--vscode-focusBorder,#007fd4);}
  .pseudo.active circle{fill:var(--vscode-charts-green,#89d185);}
  .edge path{fill:none;stroke:var(--vscode-foreground);stroke-width:1.5;
    opacity:.7;}
  .bp-glyph{fill:var(--vscode-descriptionForeground);opacity:.35;
    cursor:default;}
  .edge text{fill:var(--vscode-descriptionForeground);font-size:11px;}
  #ctx{flex:1;overflow:auto;border-top:1px solid
    var(--vscode-panel-border,#333);}
  table{border-collapse:collapse;width:100%;font-size:12px;}
  th,td{text-align:left;padding:2px 10px;
    border-bottom:1px solid var(--vscode-panel-border,#2a2a2a);}
  th{color:var(--vscode-descriptionForeground);font-weight:600;}
  .delta{color:var(--vscode-charts-green,#89d185);}
  #inject{padding:8px 10px;
    border-bottom:1px solid var(--vscode-panel-border,#333);}
  #inject label{display:block;color:var(--vscode-descriptionForeground);
    margin-bottom:3px;}
  #inject select,#inject input{font:inherit;width:100%;box-sizing:border-box;
    color:var(--vscode-input-foreground);
    background:var(--vscode-input-background);
    border:1px solid var(--vscode-input-border,#3c3c3c);
    padding:3px 6px;border-radius:2px;margin-bottom:6px;}
  #inject button{font:inherit;cursor:pointer;
    color:var(--vscode-button-foreground);
    background:var(--vscode-button-background);
    border:none;padding:3px 10px;border-radius:3px;}
  #payloadForm{margin:4px 0;padding:6px;border-radius:3px;
    background:var(--vscode-editorWidget-background,#252526);
    display:none;}
  #payloadForm.show{display:block;}
  #clockBox{padding:8px 10px;
    border-bottom:1px solid var(--vscode-panel-border,#333);}
  #clockBox input{width:90px;font:inherit;
    color:var(--vscode-input-foreground);
    background:var(--vscode-input-background);
    border:1px solid var(--vscode-input-border,#3c3c3c);
    padding:3px 6px;border-radius:2px;}
  #clockBox button{font:inherit;cursor:pointer;
    color:var(--vscode-button-foreground);
    background:var(--vscode-button-background);
    border:none;padding:3px 10px;border-radius:3px;margin-left:6px;}
  #timeline{flex:1;overflow:auto;}
  .tl-cur{color:var(--vscode-charts-green,#89d185);}
  .empty{padding:8px 10px;color:var(--vscode-descriptionForeground);
    font-style:italic;}
</style>
</head>
<body>
<div id="shell">
  <div id="debugBanner"></div>
  <div id="transport">
    <button id="btnInit" disabled>⟲ Init</button>
    <button id="btnRun" disabled title="Step-reveal is W3">▶ Run</button>
    <button id="btnPause" disabled title="Pause is W3">⏸ Pause</button>
    <button id="btnStep" disabled title="Single-step is W3">⏭ Step</button>
    <span id="clock">⏱ virtual clock: — ms</span>
  </div>
  <div id="status">Loading…</div>
  <div id="main">
    <div id="left">
      <div class="region-title">Statechart (live)</div>
      <div id="diagramWrap"><svg id="svg" xmlns="http://www.w3.org/2000/svg"></svg></div>
      <div id="ctx">
        <div class="region-title">Context</div>
        <div id="ctxBody" class="empty">No context yet — press Init.</div>
      </div>
    </div>
    <div id="right">
      <div id="inject">
        <div class="region-title" style="padding-left:0;border:none;">Inject</div>
        <label for="evtPick">event</label>
        <select id="evtPick"></select>
        <div id="payloadForm"></div>
        <button id="btnDispatch" disabled>Dispatch</button>
      </div>
      <div id="clockBox">
        <div class="region-title" style="padding-left:0;border:none;">⏱ Advance clock</div>
        by <input id="clockDelta" type="number" min="0" value="500" /> ms
        <button id="btnAdvance" disabled>⏩</button>
        <div id="timers" class="empty">pending timers: —</div>
      </div>
      <div class="region-title">Trace / Timeline</div>
      <div id="timeline"><div class="empty">No steps yet.</div></div>
    </div>
  </div>
</div>
<script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }
}

/** Cryptographically-unpredictable CSP nonce per webview render — Node's
 * CSPRNG (`crypto.randomBytes`), NOT `Math.random()` (the v1.3 GT-9
 * contract, reused verbatim; 24 bytes → ≈192 bits, exceeds the W3C
 * ≥128-bit CSP-nonce recommendation). */
function makeNonce(): string {
  return randomBytes(24).toString("base64url");
}
