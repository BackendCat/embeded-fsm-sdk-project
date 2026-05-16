// The read-only diagram `WebviewPanel` (Doc 27 §6.2; Doc 05 §1.5; Doc 28
// §3-V4). CSP-locked, `postMessage`-only, IR-sourced, ELK-laid-out in the
// Webview. Read-only — interactive graphical editing + simulator overlay
// are explicitly OUT of v1.3 (Doc 27 §9).
//
// THE CODEGEN-GATED BOUNDARY (the V4 DRIFT-class contract — proven, not
// assumed; MV4-1): the IR comes from `emitIr` (the ONE real `fsm generate
// --emit-ir`-to-temp path; no LSP/server method, no `fsm ir` subcommand).
// When codegen rejects (e.g. a source that PARSES/ANALYZES clean but a
// codegen edge fails — `crates/fsm-cli/src/cmd/generate.rs` exits BEFORE
// the IR write, so NO `.ir.json`), `emitIr` returns `codegenFailed`. This
// panel then KEEPS THE LAST VALID RENDER and shows the Doc 05 §1.5.9 banner
// VERBATIM (`⚠ Diagram shows last valid state. Fix parse errors to
// update.`) — it NEVER blanks or crashes the webview (the symmetric
// Webview analogue of V3 `copyIr.ts` refusing to fake a clipboard; the
// cardinal-sin bar). The very first open with no valid IR shows an honest
// "could not produce a diagram" state with the same banner — never a blank
// panel presented as success.
//
// SECURITY (Doc 27 §8-V4): a strict Content-Security-Policy with a
// per-render `nonce`; `localResourceRoots` pinned to the bundled webview
// dir; NO remote resources (`default-src 'none'`); the only script is the
// nonce'd bundled `diagramWebview.js`. The Webview ↔ extension boundary is
// `postMessage` JSON only — the structural model in, click/navigate events
// out (mapped back to editor reveals via the IR `SourceLocation`).

import { randomBytes } from "crypto";
import * as path from "path";

import * as vscode from "vscode";

import { CommandDeps } from "../commands/index";
import { emitIr } from "./emitIr";
import { DiagramModel, IrGraphError, parseAndBuild } from "./irGraph";

/** The Doc 05 §1.5.9 banner — VERBATIM. A regression of this string fails
 * the V4 acceptance (the contract text is asserted, not paraphrased). */
export const STALE_BANNER = "⚠ Diagram shows last valid state. Fix parse errors to update.";

/** Messages the Webview posts back to the extension (the only inbound
 * surface — a tight, typed boundary). */
interface WebviewToExt {
  /** A rendered node was clicked → reveal its declaration in the editor. */
  readonly type: "revealSource";
  readonly line: number; // 1-based (IR SourceLocation), mapped to 0-based.
  readonly column: number;
}

/** Messages the extension posts into the Webview. */
type ExtToWebview =
  | { readonly type: "render"; readonly model: DiagramModel }
  | { readonly type: "staleBanner"; readonly text: string };

/**
 * Owns one `.fsm` file's diagram panel + its live state.
 *
 * IDENTITY (Doc 05 §1.5.1/§1.5.9 — the load-bearing design point the V4
 * codegen-gated boundary depends on): the panel is keyed by the RESOLVED
 * FILE PATH, NOT the machine name. The diagram shows the file's first
 * machine; the machine name is mutable DISPLAY state (the tab title) that
 * the view re-derives on each successful render. Keying on the machine name
 * would be a correctness bug: a parse/codegen error transiently removes the
 * machine from the IR, so a machine-name key would make a re-open after an
 * error spawn a FRESH blank panel instead of focusing the existing one and
 * showing last-valid+banner — exactly the §1.5.9 "preserved across
 * re-renders if the machine identity is the same" contract, and the precise
 * scenario the V4 acceptance exercises. File-path identity keeps the panel
 * (and its last-valid render + zoom/pan) stable across the codegen-gated
 * boundary. `DiagramController` focuses an existing one rather than
 * duplicating.
 */
class DiagramView {
  /** The last model that rendered successfully — the Doc 05 §1.5.9
   * "last valid state" the panel falls back to on a codegen failure. */
  private lastValidModel: DiagramModel | undefined;
  /** The machine name of the last successful render (mutable DISPLAY state
   * — the tab title; NOT part of the panel identity). `undefined` until the
   * first valid render. */
  private machineName: string | undefined;
  private disposed = false;

  constructor(
    readonly fsmPath: string,
    private readonly panel: vscode.WebviewPanel,
    private readonly deps: CommandDeps,
  ) {
    panel.webview.onDidReceiveMessage((m: WebviewToExt) => {
      if (m && m.type === "revealSource") {
        void this.revealSource(m.line, m.column);
      }
    });
    panel.onDidDispose(() => {
      this.disposed = true;
    });
  }

  /** The machine name of the last valid render (test observability). */
  getMachineName(): string | undefined {
    return this.machineName;
  }

  reveal(): void {
    this.panel.reveal(vscode.ViewColumn.Beside, false);
  }

  isDisposed(): boolean {
    return this.disposed;
  }

  /**
   * Re-acquire the IR and (re-)render. On a codegen-gated failure: keep the
   * last valid render + show the Doc 05 §1.5.9 banner — NEVER blank/crash.
   */
  async refresh(): Promise<void> {
    if (this.disposed) {
      return;
    }
    const res = await emitIr(
      this.fsmPath,
      this.deps.extensionPath,
      vscode.workspace.getConfiguration("fsmLang"),
      this.deps.outputChannel,
    );

    if (!res.ok) {
      // The codegen-gated boundary (or no-CLI / spawn / no-IR): DO NOT
      // refresh the diagram. Keep the last valid render and show the
      // verbatim Doc 05 §1.5.9 banner. If there is no last-valid render
      // yet (first open failed), the webview shows the honest
      // "no diagram yet" placeholder WITH the same banner — never a blank
      // panel pretending success.
      this.deps.outputChannel.appendLine(
        `[fsm] diagram: IR unavailable (${res.reason}: ${res.detail}) — ` +
          "keeping last valid render + showing stale banner (Doc 05 " +
          "§1.5.9).",
      );
      this.post({ type: "staleBanner", text: STALE_BANNER });
      return;
    }

    let model: DiagramModel;
    try {
      // No machine name passed → project the FIRST machine (Doc 05
      // §1.5.1 "one diagram panel per machine"; v1.3 shows the file's
      // first machine — the panel identity is the file, §1.5.9).
      model = parseAndBuild(res.json);
    } catch (e) {
      // A malformed/empty IR is a hard failure — same honest fallback as
      // the codegen-gated boundary (never a blank diagram as "success").
      const detail = e instanceof IrGraphError ? e.message : String(e);
      this.deps.outputChannel.appendLine(
        `[fsm] diagram: IR projection failed (${detail}) — keeping last ` +
          "valid render + stale banner.",
      );
      this.post({ type: "staleBanner", text: STALE_BANNER });
      return;
    }

    this.lastValidModel = model;
    // Update the mutable DISPLAY state (tab title) — NOT the identity.
    if (this.machineName !== model.machineName) {
      this.machineName = model.machineName;
      this.panel.title = `⬡ ${model.machineName} — Diagram`;
    }
    this.post({ type: "render", model });
    this.deps.outputChannel.appendLine(
      `[fsm] diagram: rendered ${model.machineName} ` +
        `(${model.nodes.length} nodes, ${model.edges.length} edges).`,
    );
  }

  /** The last successfully-rendered model (test observability + the
   * "last valid state" the banner path preserves). */
  getLastValidModel(): DiagramModel | undefined {
    return this.lastValidModel;
  }

  private post(m: ExtToWebview): void {
    if (!this.disposed) {
      void this.panel.webview.postMessage(m);
    }
  }

  /** Reveal a clicked node's declaration in the source editor (Doc 05
   * §1.5.4 click → editor line; the IR `SourceLocation` is 1-based, VS Code
   * is 0-based). */
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
 * The `fsm.openDiagram` controller — opens/focuses one read-only diagram
 * panel per machine and keeps it live (re-render on save / on the document
 * becoming clean, Doc 05 §1.5.9). The webview HTML is CSP-locked with a
 * per-panel nonce; the only resource is the bundled `diagramWebview.js`.
 */
export class DiagramController {
  /** resolved-file-path → its live view. Keyed by the FILE (NOT the
   * machine name) so the panel + its last-valid render survive a transient
   * codegen failure that removes the machine from the IR (Doc 05
   * §1.5.1/§1.5.9 — the V4 codegen-gated-boundary contract). */
  private readonly views = new Map<string, DiagramView>();

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly deps: CommandDeps,
  ) {
    // Live re-render: on save of the .fsm the panel re-acquires the IR;
    // a codegen failure trips the last-valid + banner path (Doc 05 §1.5.9
    // "diagram re-renders … if re-parse fails: last valid + banner").
    context.subscriptions.push(
      vscode.workspace.onDidSaveTextDocument((doc) => {
        const view = this.views.get(path.resolve(doc.uri.fsPath));
        if (view && !view.isDisposed()) {
          void view.refresh();
        }
      }),
    );
  }

  /** Open (or focus the existing) diagram for the given `.fsm`. The panel
   * identity is the RESOLVED FILE PATH (Doc 05 §1.5.1 "re-using the same
   * command focuses existing panel"; §1.5.9 identity-stable-across-renders)
   * — re-opening after a parse/codegen error focuses the SAME panel and
   * shows last-valid+banner, never a fresh blank one. */
  async open(fsmPath: string): Promise<void> {
    const key = path.resolve(fsmPath);

    const existing = this.views.get(key);
    if (existing && !existing.isDisposed()) {
      existing.reveal(); // Doc 05 §1.5.1 — focus the existing panel.
      await existing.refresh();
      return;
    }

    // The tab title starts from the filename; the view re-titles it to
    // `⬡ <Machine> — Diagram` on the first successful render (the machine
    // name is mutable DISPLAY state, not the identity).
    const panel = vscode.window.createWebviewPanel(
      "fsmDiagram",
      `⬡ ${path.basename(fsmPath)} — Diagram`,
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

    const view = new DiagramView(fsmPath, panel, this.deps);
    this.views.set(key, view);
    panel.onDidDispose(() => {
      const cur = this.views.get(key);
      if (cur === view) {
        this.views.delete(key);
      }
    });

    await view.refresh();
  }

  /** Test-observable: the last-valid model for a file's panel. The
   * `machineName` arg is accepted for call-site readability but identity
   * is the file path (one panel per file — §1.5.9). */
  lastValidModelFor(fsmPath: string, _machineName?: string): DiagramModel | undefined {
    void _machineName;
    return this.views.get(path.resolve(fsmPath))?.getLastValidModel();
  }

  /** Strict CSP webview shell. `default-src 'none'`; the ONLY script is the
   * nonce'd bundled `diagramWebview.js`; styles are nonce'd inline; no
   * remote anything (Doc 27 §8-V4 security). */
  private html(webview: vscode.Webview): string {
    const nonce = makeNonce();
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.file(
        path.join(this.context.extensionPath, "dist", "webview", "diagramWebview.js"),
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
<title>FSM Diagram</title>
<style nonce="${nonce}">
  html,body{margin:0;padding:0;height:100%;overflow:hidden;
    font-family:var(--vscode-font-family);
    color:var(--vscode-foreground);
    background:var(--vscode-editor-background);}
  #banner{display:none;padding:6px 12px;font-size:12px;
    background:var(--vscode-inputValidation-warningBackground,#5a4a00);
    color:var(--vscode-inputValidation-warningForeground,#e8d36b);
    border-bottom:1px solid var(--vscode-inputValidation-warningBorder,#a8862b);}
  #banner.show{display:block;}
  #status{padding:6px 12px;font-size:12px;
    color:var(--vscode-descriptionForeground);}
  #canvas{position:absolute;top:0;left:0;width:100%;height:100%;}
  svg{width:100%;height:100%;}
  .node rect{fill:var(--vscode-editorWidget-background,#252526);
    stroke:var(--vscode-focusBorder,#007fd4);stroke-width:1.5;rx:6;}
  .node text{fill:var(--vscode-foreground);font-size:13px;
    cursor:pointer;}
  .pseudo circle{fill:var(--vscode-focusBorder,#007fd4);}
  .edge path{fill:none;stroke:var(--vscode-foreground);stroke-width:1.5;
    opacity:.7;}
  .edge text{fill:var(--vscode-descriptionForeground);font-size:11px;}
</style>
</head>
<body>
<div id="banner"></div>
<div id="status">Loading diagram…</div>
<div id="canvas"><svg id="svg" xmlns="http://www.w3.org/2000/svg"></svg></div>
<script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }
}

/**
 * Cryptographically-unpredictable CSP nonce (per webview render).
 *
 * Sourced from Node's CSPRNG (`crypto.randomBytes`, the extension host is
 * Node) — NOT `Math.random()`, whose output is a predictable non-crypto
 * PRNG and is unfit to seed a Content-Security-Policy nonce (the v1.3
 * carried GT-9 nit). 24 random bytes → a 32-char base64url token (≈192
 * bits of entropy; comfortably exceeds the W3C "at least 128 bits"
 * CSP-nonce recommendation).
 */
function makeNonce(): string {
  return randomBytes(24).toString("base64url");
}
