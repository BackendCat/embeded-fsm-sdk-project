// V5 — activity-bar tree views + context-key chrome (Doc 28 §3-V5 / Doc 27
// §5/§8-V5; Doc 22 §7/§11).
//
// SCOPE (V5, strictly disjoint from V1's client-spawn, V2's grammar/
// snippets, V3's CLI-wrapper/client-control commands, and V4's diagram
// Webview): this module registers the two `TreeDataProvider`s
// (`fsm.machineExplorer` / `fsm.eventExplorer`) for the Doc 22 §7
// activity-bar container, the two tree-refresh commands
// (`fsm.refreshMachineExplorer` / `fsm.refreshEventExplorer` — the Doc 05
// §1.3.2 `view/title` refresh affordance), and the Doc 22 §11 context-key
// plumbing (`fsm.hasOpenFsmFile` / `fsm.serverRunning`). It adds NO
// runtime to the language client (MV5-2 / M-1 lineage: V5 touches no
// client options; V1's omit-transport `Executable` + the deliberate
// no-`capabilities`-override stay byte-unchanged).
//
// THE V5 DATA SOURCE (MV5-1, the keystone): the tree consumes the FREE
// V1-client `documentSymbol` capability via
// `vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider",
// uri)`. That round-trips to the SHIPPED `fsm-lang-server` (advertised
// free at crates/fsm-lsp/src/server.rs:251; the SAME single-analysis
// `analyze_with_source` projection the squiggle uses —
// crates/fsm-lsp/src/capabilities/document_symbol.rs:1-9) with ZERO new
// extension code, because V1's client has no `capabilities` override
// (extension.ts:234-236). It does NOT reach for V4's
// `emitIr.ts`/`irGraph.ts` (`fsm generate --emit-ir`) — that codegen-gated
// IR is the wrong shape for an outline AND would make the tree vanish for
// a machine that parses but would not codegen (the symmetric analogue of
// V1's omit-transport foot-gun; re-importing the K-3 DRIFT into chrome is
// explicitly forbidden). If the live symbols are momentarily unavailable
// (server not yet running / no `.fsm` active) the tree shows an empty/
// welcome state, NEVER the IR path.
//
// Context keys are driven by VS CODE STATE + the V1 client lifecycle, NOT
// a new analysis: `fsm.hasOpenFsmFile` from the active editor's
// languageId, `fsm.serverRunning` mirrored from the SAME
// `client.onDidChangeState` signal V1 already tracks for the status bar
// (extension.ts:248-261) — V5 only forwards it into `setContext`.

import * as vscode from "vscode";
import {
  LanguageClient,
  State,
} from "vscode-languageclient/node";

import {
  FsmTreeNode,
  projectEventTree,
  projectMachineTree,
} from "./symbolTree";

const LANGUAGE_ID = "fsm-lang";

/** Doc 22 §11 context keys this module owns (set via `setContext`). */
const CK_HAS_OPEN_FSM = "fsm.hasOpenFsmFile";
const CK_SERVER_RUNNING = "fsm.serverRunning";

/**
 * The V1-owned state V5 needs, passed in by `extension.ts` WITHOUT V5
 * reaching into V1's module internals or altering its client-spawn — the
 * exact dependency-injection shape V3's `CommandDeps` / V4's
 * `registerOpenDiagram` already established.
 */
export interface ExplorerDeps {
  /**
   * The language client `extension.ts` already built (V1's omit-transport
   * `Executable`). May be `undefined` if no binary resolved — the tree
   * then shows its welcome/empty state (no fabricated nodes, no IR
   * fallback; the cardinal-sin bar at this boundary).
   */
  readonly getClient: () => LanguageClient | undefined;
}

/**
 * Map an {@link FsmTreeNode}'s LSP `SymbolKind` to a ThemeIcon. Purely
 * cosmetic (the tree's *data* is the `documentSymbol` projection; the icon
 * is chrome). The `SymbolKind` numeric values are the LSP/`vscode`
 * constants (`Module=1`, `Namespace=2`, `Class=4`, `Enum=9`,
 * `Function=11`, `Field=7`, `Constant=13`, `Event=23`,
 * `EnumMember=21`, `Operator=25` — the kinds
 * `document_symbol.rs:classify_state`/group nodes emit).
 */
function iconForKind(kind: number): vscode.ThemeIcon {
  switch (kind) {
    case vscode.SymbolKind.Module:
      return new vscode.ThemeIcon("symbol-class");
    case vscode.SymbolKind.Namespace:
      return new vscode.ThemeIcon("symbol-namespace");
    case vscode.SymbolKind.Class:
      return new vscode.ThemeIcon("symbol-class");
    case vscode.SymbolKind.Enum:
      return new vscode.ThemeIcon("symbol-enum");
    case vscode.SymbolKind.EnumMember:
      return new vscode.ThemeIcon("symbol-enum-member");
    case vscode.SymbolKind.Function:
      return new vscode.ThemeIcon("symbol-function");
    case vscode.SymbolKind.Field:
      return new vscode.ThemeIcon("symbol-field");
    case vscode.SymbolKind.Constant:
      return new vscode.ThemeIcon("symbol-constant");
    case vscode.SymbolKind.Event:
      return new vscode.ThemeIcon("symbol-event");
    case vscode.SymbolKind.Operator:
      return new vscode.ThemeIcon("symbol-operator");
    default:
      return new vscode.ThemeIcon("symbol-misc");
  }
}

/**
 * A `vscode.TreeItem` wrapping one projected node. Carries the `node` so
 * the reveal command (and the Extension-Host acceptance) can read the
 * authoritative `selectionRange`/`uri` back off the clicked item.
 */
class FsmTreeItem extends vscode.TreeItem {
  constructor(
    public readonly node: FsmTreeNode,
    public readonly uri: vscode.Uri,
  ) {
    super(
      node.name,
      node.children.length > 0
        ? vscode.TreeItemCollapsibleState.Expanded
        : vscode.TreeItemCollapsibleState.None,
    );
    // The Doc 14 §13 parenthetical (`Idle (simple)` / `count: u32` / …)
    // shown dimmed after the name — the symbol's verbatim, never re-derived.
    this.description = node.detail;
    this.iconPath = iconForKind(node.kind);
    // Click → reveal the declaration at the authoritative `selectionRange`
    // (Doc 05 §1.3.6). No new location math — the LSP already guarantees
    // `selectionRange ⊆ range` and that it is the name span.
    this.command = {
      command: "fsm.revealSymbol",
      title: "Reveal in Editor",
      arguments: [uri, node.selectionRange],
    };
    this.contextValue = "fsmSymbol";
  }
}

/**
 * The shared `TreeDataProvider` for both FSM explorers. Its data is the
 * FREE `documentSymbol` projection for the active `.fsm` editor
 * (re-projected, not re-analyzed — MV5-1). A `project` strategy makes the
 * SAME provider class serve both the Machines (full hierarchy) and Events
 * (flat per-machine events) views without duplicating the
 * documentSymbol acquisition.
 */
class FsmSymbolTreeProvider
  implements vscode.TreeDataProvider<FsmTreeItem>
{
  private readonly emitter =
    new vscode.EventEmitter<FsmTreeItem | undefined>();
  readonly onDidChangeTreeData = this.emitter.event;

  /** Cache of the last projection so `getChildren(element)` can recurse
   * without re-querying the server for every expand. Re-acquired on
   * refresh / active-editor change (the data, not the chrome, is the
   * source of truth). */
  private roots: FsmTreeNode[] = [];
  private activeUri: vscode.Uri | undefined;

  constructor(
    private readonly project: (
      symbols: readonly vscode.DocumentSymbol[],
    ) => FsmTreeNode[],
  ) {}

  /** Re-acquire the `documentSymbol` projection for the active editor and
   * repaint. Called on activation, active-editor change, document save,
   * and the explicit `view/title` refresh command (Doc 05 §1.3.2). */
  async refresh(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== LANGUAGE_ID) {
      this.roots = [];
      this.activeUri = undefined;
      this.emitter.fire(undefined);
      return;
    }
    this.activeUri = editor.document.uri;
    // THE V5 DATA SOURCE: the free V1-client `documentSymbol` capability.
    // `executeDocumentSymbolProvider` round-trips to the shipped server
    // (server.rs:251) via V1's un-overridden client — NO new extension
    // code, NO `fsm --emit-ir`, NO custom LSP method (MV5-1). If the
    // server has nothing yet (just opened / not running) it returns
    // undefined/[] → an HONEST empty tree, never the IR path.
    let symbols: vscode.DocumentSymbol[] | undefined;
    try {
      symbols = await vscode.commands.executeCommand<
        vscode.DocumentSymbol[]
      >("vscode.executeDocumentSymbolProvider", this.activeUri);
    } catch {
      // A momentarily-unavailable provider is an empty tree, not a crash
      // and never a fabricated/IR-sourced one (the cardinal-sin bar).
      symbols = undefined;
    }
    this.roots = this.project(symbols ?? []);
    this.emitter.fire(undefined);
  }

  getTreeItem(element: FsmTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: FsmTreeItem): FsmTreeItem[] {
    if (!this.activeUri) {
      return [];
    }
    const uri = this.activeUri;
    const nodes = element ? element.node.children : this.roots;
    return nodes.map((n) => new FsmTreeItem(n, uri));
  }

  /**
   * The projected root forest as last refreshed — the test-observable
   * surface the §5.4 acceptance deep-compares against an INDEPENDENTLY
   * obtained `executeDocumentSymbolProvider` oracle (a real structural
   * equality, not "a provider is registered"). Read-only by contract.
   */
  projectedRoots(): FsmTreeNode[] {
    return this.roots;
  }

  dispose(): void {
    this.emitter.dispose();
  }
}

/**
 * Recompute the Doc 22 §11 context keys from VS Code state + the V1 client
 * lifecycle. `fsm.hasOpenFsmFile` ← the active editor is a `.fsm`;
 * `fsm.serverRunning` ← the SAME `client.state === Running` signal V1's
 * status bar already follows (NOT a new analysis). Driven via `setContext`
 * so the Doc 22 §7 `when: fsm.hasOpenFsmFile` views show/hide and any
 * `fsm.serverRunning`-gated chrome reacts.
 */
function updateContextKeys(deps: ExplorerDeps): void {
  const editor = vscode.window.activeTextEditor;
  const hasFsm =
    editor !== undefined && editor.document.languageId === LANGUAGE_ID;
  void vscode.commands.executeCommand(
    "setContext",
    CK_HAS_OPEN_FSM,
    hasFsm,
  );
  const client = deps.getClient();
  void vscode.commands.executeCommand(
    "setContext",
    CK_SERVER_RUNNING,
    client?.state === State.Running,
  );
}

/**
 * Test-observable handle (the standard VS Code pattern V1's
 * `FsmExtensionApi` established): lets the Extension-Host §5.4 acceptance
 * read back the EXACT projected node forest each provider rendered and
 * force a deterministic refresh — WITHOUT leaking test concerns into the
 * runtime path or asserting mere "a provider is registered".
 */
export interface FsmExplorerHandle {
  /** Force-refresh both providers (await the documentSymbol round-trip). */
  refresh(): Promise<void>;
  /** The Machines tree's projected root forest (post-refresh). */
  machineRoots(): FsmTreeNode[];
  /** The Events tree's projected root forest (post-refresh). */
  eventRoots(): FsmTreeNode[];
  /** Recompute the Doc 22 §11 context keys now (for the gating test). */
  syncContextKeys(): void;
}

/**
 * Register the V5 activity-bar tree views + context-key chrome. Called
 * once from `activate()` AFTER V1 has built the client — an additive call
 * site, no change to V1's spawn path (the exact pattern V3's
 * `registerCommands` / V4's `registerOpenDiagram` already established).
 * Returns the {@link FsmExplorerHandle} so the acceptance test can observe
 * the projected forests + drive a deterministic refresh.
 */
export function registerFsmExplorer(
  context: vscode.ExtensionContext,
  deps: ExplorerDeps,
): FsmExplorerHandle {
  const machineProvider = new FsmSymbolTreeProvider(projectMachineTree);
  const eventProvider = new FsmSymbolTreeProvider(projectEventTree);
  context.subscriptions.push(machineProvider, eventProvider);

  context.subscriptions.push(
    vscode.window.registerTreeDataProvider(
      "fsm.machineExplorer",
      machineProvider,
    ),
    vscode.window.registerTreeDataProvider(
      "fsm.eventExplorer",
      eventProvider,
    ),
  );

  const refreshAll = async (): Promise<void> => {
    await Promise.all([
      machineProvider.refresh(),
      eventProvider.refresh(),
    ]);
  };

  // The Doc 05 §1.3.2 `view/title` refresh affordance — one command per
  // explorer (the manifest wires each to its own view's title bar).
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fsm.refreshMachineExplorer",
      () => machineProvider.refresh(),
    ),
    vscode.commands.registerCommand(
      "fsm.refreshEventExplorer",
      () => eventProvider.refresh(),
    ),
  );

  // Click→declaration: reveal the authoritative `selectionRange` (Doc 05
  // §1.3.6). The same "navigate via the authoritative source location"
  // discipline V4 used for click→source — no new location math, the LSP
  // already guarantees this range is the name span ⊆ the decl range.
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fsm.revealSymbol",
      async (uri: vscode.Uri, range: vscode.Range) => {
        const doc = await vscode.workspace.openTextDocument(uri);
        const editor = await vscode.window.showTextDocument(doc);
        editor.selection = new vscode.Selection(
          range.start,
          range.end,
        );
        editor.revealRange(
          range,
          vscode.TextEditorRevealType.InCenterIfOutsideViewport,
        );
      },
    ),
  );

  const sync = (): void => {
    updateContextKeys(deps);
    void refreshAll();
  };

  // React to VS Code state: which editor is active (drives both the
  // context keys AND which file the tree projects) and document saves
  // (the symbol set may have changed). NO polling, NO new analysis.
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(() => sync()),
    vscode.workspace.onDidSaveTextDocument((d) => {
      if (d.languageId === LANGUAGE_ID) {
        void refreshAll();
      }
    }),
  );

  // The server-liveness signal WITHOUT a V1 cross-touch or a client-spawn
  // ordering dependency: `onDidChangeDiagnostics` is the SAME passive read
  // V1's status-bar severity refinement uses (extension.ts:265-269 — "a
  // passive read of what the client already received; no new analysis").
  // A diagnostics publish for a `.fsm` means the server is running AND has
  // produced the single analysis `documentSymbol` projects from — the
  // exact moment to re-acquire the symbols and re-evaluate
  // `fsm.serverRunning`. This avoids subscribing to `client.onDidChangeState`
  // here (which would either force registration AFTER V1's spawn or
  // duplicate V1's status-bar handler — the N-4 precedent sanctions only
  // the statusBar.ts string touch, nothing more in V1's lifecycle block).
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => sync()),
  );

  // Belt-and-braces: if a client already exists at registration time
  // (V5 registered after V1's spawn), also mirror its `onDidChangeState`
  // — the audit's named signal (extension.ts:248). Harmless + a no-op
  // when no binary resolved (the tree stays in its honest empty state).
  const client = deps.getClient();
  if (client) {
    context.subscriptions.push(
      client.onDidChangeState(() => sync()),
    );
  }

  // Initial paint + context-key seed (covers the file-already-open case).
  sync();

  return {
    refresh: refreshAll,
    machineRoots: () => machineProvider.projectedRoots(),
    eventRoots: () => eventProvider.projectedRoots(),
    syncContextKeys: () => updateContextKeys(deps),
  };
}
