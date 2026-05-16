// FSM Studio VS Code extension — V1 MVP spine (Doc 28 §5 / Doc 27 §8-V1).
//
// SCOPE (V1, depth-first PD-7): extension scaffold + language client to the
// SHIPPED `fsm-lang-server` over stdio + live diagnostics round-tripping
// in-editor. Grammar/snippets/commands/diagram/trees are explicitly OUT of
// V1 (Doc 28 §5 DO-NOT) and live in V2..V6.
//
// THE positionEncoding seam — a load-bearing V1-SPINE FINDING that
// corrects Doc 27 §2.3 / Doc 28 R-4 against shipped-`vscode-languageclient`
// reality (the depth-first V1 gate exists precisely to surface this):
//
//   Doc 27 §2.3 / R-4 assume `vscode-languageclient` >= 8 advertises
//   `general.positionEncodings` such that the shipped server's UTF-8
//   fast-path (server.rs:216-239) is reachable "iff the extension does not
//   strip it". VERIFIED FALSE against vscode-languageclient v8.0.2 / v8.1.0
//   / v9.0.1: ALL of them HARDCODE
//   `generalCapabilities.positionEncodings = ['utf-16']`
//   (lib/common/client.js) with no opt-in / override / middleware, AND
//   THROW `Unsupported position encoding` if the server negotiates
//   anything other than UTF-16 (client.js:~835). The official client is
//   intrinsically UTF-16 (VS Code's text model is UTF-16); UTF-8 is NOT
//   reachable through it by any in-scope means.
//
//   CONSEQUENCE (correct + safe): the shipped server's documented FALLBACK
//   path runs — client offers only `['utf-16']` -> server negotiates
//   UTF-16 (server.rs:216-226, R-4's explicit "else UTF-16"). This is
//   still CORRECT (Doc 26 §4.1: "still correct, loses the UTF-8
//   fast-path"); the squiggle round-trips and the multibyte-range
//   defect-class guard is intact end-to-end under UTF-16 (the server's
//   UTF-16 LineIndex path is itself tested — lsp_client_acceptance.rs
//   `non_ascii_line_range_correct_under_utf8_AND_utf16`). What is lost is
//   only the perf fast-path, NOT correctness. The Doc 26 §4.1 defect
//   CLASS (mis-encoded multibyte columns) is still guarded — see the V1
//   acceptance (d) assertion, which runs under this UTF-16 negotiation.
//
// The one thing we MUST still NOT do (the actual surviving R-4 guard): do
// NOT override `clientOptions`/`initializationOptions` in a way that
// suppresses the client's default capabilities. `initializationOptions`
// carries ONLY the four inlay keys; the capability set is the client
// default. (Surfaced to the orchestrator per Doc 28 §5 / SUBAGENT §8 — a
// doc-vs-shipped-library drift the V1 spine caught, exactly as intended.)

import * as path from "path";
import * as vscode from "vscode";
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  State,
} from "vscode-languageclient/node";

import { registerCommands } from "./commands";
import { registerOpenDiagram } from "./diagram";
import { FsmExplorerHandle, registerFsmExplorer } from "./tree";
import { FsmErrorHandler } from "./crashRecovery";
import { readInlayHintSettings } from "./inlayConfig";
import {
  noBundledBinaryMessage,
  hostTriple,
  resolveServerBinary,
} from "./serverBinary";
import { FsmStatusBar } from "./statusBar";

const LANGUAGE_ID = "fsm-lang";
const CONFIG_SECTION = "fsmLang";
const OUTPUT_CHANNEL_NAME = "FSM Language Server";

let client: LanguageClient | undefined;
let statusBar: FsmStatusBar | undefined;
let errorHandler: FsmErrorHandler | undefined;
let outputChannel: vscode.OutputChannel | undefined;

/**
 * Test-observable extension API returned from {@link activate}. This is the
 * standard VS Code pattern for making an extension's internal state
 * assertable from an Extension-Host test WITHOUT leaking test concerns into
 * the runtime path. It exposes only read-only facts the V1 acceptance gate
 * needs (Doc 28 §5 (c): the negotiated positionEncoding must be observable).
 */
export interface FsmExtensionApi {
  /** `true` once the language client reached the Running state. */
  readonly serverStarted: boolean;
  /**
   * The `positionEncoding` the server advertised back in `initialize`
   * (Doc 27 §2.3 / Doc 28 R-4) — `"utf-8"` under a >= 8 client that did
   * not strip `general.positionEncodings`. `undefined` if the client never
   * started (no binary).
   */
  readonly negotiatedPositionEncoding: string | undefined;
  /** Which binary-resolution rule won (Doc 22 §2.2). */
  readonly binarySource: "compilerPath" | "bundled" | "none";
  /**
   * V5 test-observability (additive — V1's three fields above are
   * unchanged; structural typing keeps V1's/V2's/V3's/V4's tests, which
   * declare their own narrower `FsmExtensionApi`, byte-unaffected). Lets
   * the V5 §5.4 Extension-Host acceptance read the EXACT projected tree
   * forest each registered provider rendered and force a deterministic
   * `documentSymbol` refresh — so the gate is "the tree's node structure
   * deep-equals the `executeDocumentSymbolProvider` oracle", NOT "a
   * provider is registered" (the P0-1 bar). `undefined` only if V5
   * registration was skipped (it never is — it runs before the no-binary
   * return).
   */
  readonly fsmExplorer: FsmExplorerHandle | undefined;
}

function buildApi(
  binarySource: "compilerPath" | "bundled" | "none",
  fsmExplorer: FsmExplorerHandle | undefined,
): FsmExtensionApi {
  return {
    get serverStarted(): boolean {
      return client?.state === State.Running;
    },
    get negotiatedPositionEncoding(): string | undefined {
      // `initializeResult` is populated by vscode-languageclient after the
      // `initialize` round-trip; `positionEncoding` is the negotiated value.
      const enc =
        client?.initializeResult?.capabilities?.positionEncoding;
      return typeof enc === "string" ? enc : undefined;
    },
    binarySource,
    fsmExplorer,
  };
}

export async function activate(
  context: vscode.ExtensionContext,
): Promise<FsmExtensionApi> {
  outputChannel = vscode.window.createOutputChannel(OUTPUT_CHANNEL_NAME);
  context.subscriptions.push(outputChannel);

  statusBar = new FsmStatusBar();
  context.subscriptions.push(statusBar);

  // V3: register the CLI-wrapper + client-control commands (Doc 28 §3-V3).
  // Done HERE — before the no-binary early-return — so the M-2 commands
  // (`fsm.showOutputChannel`, `fsm.restartLanguageServer`) are functional
  // even with no server binary (V1's status-bar click wires to
  // `fsm.showOutputChannel` and is shown in the stopped state too;
  // `fsm.restartLanguageServer` degrades honestly when `client` is
  // undefined). `getClient` reads the module-level `client` set by V1's
  // (unaltered) spawn path; this call adds command registrations only.
  registerCommands(context, {
    getClient: () => client,
    outputChannel,
    restartServer,
    extensionPath: context.extensionPath,
  });

  // V4: register the read-only diagram WebviewPanel (Doc 28 §3-V4).
  // Done HERE — before the no-binary early-return — so `fsm.openDiagram`
  // is functional even with no language-SERVER binary: the diagram's data
  // source is the `fsm` CLI (`generate --emit-ir`), NOT `fsm-lang-server`
  // (the genuine two-binary need Doc 27 §2.2 records; resolved by the V3
  // `cliBinary.ts` seam, not the server resolver). An additive call site
  // identical in shape to V3's `registerCommands` — it adds one command
  // registration only and does NOT touch V1's (unaltered) client-spawn /
  // `serverOptions` / `positionEncoding` (MV4-2 / M-1 lineage).
  registerOpenDiagram(context, {
    getClient: () => client,
    outputChannel,
    restartServer,
    extensionPath: context.extensionPath,
  });

  // V5: register the activity-bar tree views + context-key chrome
  // (Doc 28 §3-V5 / Doc 27 §5/§8-V5; Doc 22 §7/§11). Done HERE — before
  // the no-binary early-return — so the FSM-explorer container, its
  // `fsm.machineExplorer`/`fsm.eventExplorer` providers, the `view/title`
  // refresh commands, and the Doc 22 §11 context keys exist even with no
  // server binary (the views then show their `viewsWelcome` / honest
  // empty state — never a fabricated or IR-sourced tree). The tree's data
  // is the FREE V1-client `documentSymbol` capability
  // (`executeDocumentSymbolProvider`, server.rs:251) — NOT the `fsm`
  // CLI's `--emit-ir` path (MV5-1). An additive call site identical in
  // shape to V3's `registerCommands` / V4's `registerOpenDiagram` — it
  // adds tree/command/context-key registrations only and does NOT touch
  // V1's (unaltered) client-spawn / `serverOptions` / `clientOptions` /
  // `positionEncoding` (MV5-2 / M-1 lineage).
  const fsmExplorer = registerFsmExplorer(context, {
    getClient: () => client,
  });

  const config = vscode.workspace.getConfiguration(CONFIG_SECTION);
  const resolved = resolveServerBinary(context.extensionPath, config);
  if (!resolved) {
    // Rule 3 (Doc 22 §2.2 / §12): no binary -> verbatim error, stay inert.
    // NO silent fallback (the cardinal-sin bar at the client boundary).
    const triple = hostTriple();
    outputChannel.appendLine(
      `[fsm] no fsm-lang-server binary: fsmLang.compilerPath is empty and ` +
        `no bundled binary at bin/${triple}/. Client not started.`,
    );
    statusBar.set("stopped");
    void vscode.window.showErrorMessage(noBundledBinaryMessage(triple));
    return buildApi("none", fsmExplorer);
  }
  outputChannel.appendLine(
    `[fsm] launching fsm-lang-server (${resolved.source}): ${resolved.command}`,
  );

  // stdio transport — but NOTE the load-bearing client-binding subtlety
  // (a real V1-spine finding): the shipped `fsm-lang-server` takes NO
  // arguments for stdio mode (`main.rs:53-65`: `None => run_stdio()`) and
  // its strict parser exits 2 on ANY unknown argument, INCLUDING `--stdio`
  // (`main.rs:49-52`, the project's fail-loud-on-unknown-arg bar). If we
  // set `transport: TransportKind.stdio`, vscode-languageclient v9 pushes
  // `--stdio` onto argv (lib/node/main.js:408-410) — the server then exits
  // 2 and the client stream is destroyed mid-`initialize`. The correct
  // wiring is to OMIT `transport`: for an `Executable`, v9 spawns over
  // stdio when `transport === undefined` (main.js:423) WITHOUT injecting
  // `--stdio` (the push is gated on an explicit stdio kind, not the
  // undefined default). This invokes the binary exactly as its contract
  // requires: `fsm-lang-server` with no args. Doc 27 §2.2's
  // "`TransportKind.stdio`" is the intent; the shipped server's arg parser
  // makes the no-transport form the only one that actually round-trips —
  // exactly the kind of seam the V1 depth-first gate exists to surface.
  const executable: Executable = {
    command: resolved.command,
    args: [],
  };
  const serverOptions: ServerOptions = {
    run: executable,
    debug: executable,
  };

  errorHandler = new FsmErrorHandler((state) => {
    if (!statusBar) {
      return;
    }
    if (state.kind === "restarting") {
      // N-4 (the audit-sanctioned V5 status-bar fold-in, Doc 22:679):
      // route the restarting state to the dedicated tooltip
      // (`FSM Language Server restarting (attempt N/3)...`) instead of the
      // shared `set("starting")` whose generic tooltip the V1 audit
      // flagged. The `attempt` is read off the error-handler state object
      // (FsmErrorHandler already supplies it — crashRecovery.ts:84-88); no
      // client-spawn / serverOptions / clientOptions touched (M-1 intact).
      statusBar.setRestarting(state.attempt);
    } else {
      statusBar.set("stopped");
      void vscode.window
        .showErrorMessage(
          "FSM Language Server has stopped after 3 restart attempts.",
          "Restart Language Server",
          "Show Log",
        )
        .then((choice) => {
          if (choice === "Restart Language Server") {
            void restartServer();
          } else if (choice === "Show Log") {
            outputChannel?.show(true);
          }
        });
    }
  });
  context.subscriptions.push({ dispose: () => errorHandler?.dispose() });

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: LANGUAGE_ID }],
    outputChannel,
    // The four real inlay keys flow BOTH at initialize
    // (initializationOptions) AND live (synchronize.configurationSection ->
    // workspace/didChangeConfiguration). This is exactly the channel pair
    // the shipped server round-trips (server.rs:233-235 + :855-857).
    initializationOptions: readInlayHintSettings(
      vscode.workspace.getConfiguration(CONFIG_SECTION),
    ),
    synchronize: {
      configurationSection: CONFIG_SECTION,
    },
    errorHandler,
    // NOTE: no `capabilities` / `clientOptions` override here — that is
    // deliberate (Doc 27 §2.3 / Doc 28 R-4). Touching it risks stripping
    // `general.positionEncodings` and silently regressing to UTF-16.
  };

  client = new LanguageClient(
    "fsmLanguageServer",
    "FSM Language Server",
    serverOptions,
    clientOptions,
  );

  // Status bar follows the client lifecycle (Doc 22 §10 / §13.2).
  context.subscriptions.push(
    client.onDidChangeState((e) => {
      if (!statusBar) {
        return;
      }
      if (e.newState === State.Running) {
        statusBar.set("running");
        errorHandler?.noteServerReady();
      } else if (e.newState === State.Starting) {
        statusBar.set("starting");
      } else if (e.newState === State.Stopped) {
        statusBar.set("stopped");
      }
    }),
  );

  // Refine the running icon by the latest diagnostics severity — a passive
  // read of what the client already received (Doc 27 §5; no new analysis).
  context.subscriptions.push(
    vscode.languages.onDidChangeDiagnostics(() => {
      refreshSeverityIndicator();
    }),
  );

  await client.start();
  context.subscriptions.push(client);
  refreshSeverityIndicator();
  return buildApi(resolved.source, fsmExplorer);
}

function refreshSeverityIndicator(): void {
  if (!statusBar) {
    return;
  }
  let worst: "error" | "warning" | "clean" = "clean";
  for (const [uri, diags] of vscode.languages.getDiagnostics()) {
    if (path.extname(uri.fsPath) !== ".fsm") {
      continue;
    }
    for (const d of diags) {
      if (d.severity === vscode.DiagnosticSeverity.Error) {
        worst = "error";
      } else if (
        d.severity === vscode.DiagnosticSeverity.Warning &&
        worst !== "error"
      ) {
        worst = "warning";
      }
    }
  }
  statusBar.refineBySeverity(worst);
}

/**
 * `fsm.restartLanguageServer` semantics reused by the crash-recovery
 * exhaustion notification (Doc 22 §13.2 step 3): restart + reset the
 * crash counter. The standalone command is V3 scope; V1 only needs the
 * recovery-path restart, so this is intentionally not contributed as a
 * command in package.json.
 */
async function restartServer(): Promise<void> {
  errorHandler?.resetCrashCount();
  if (client) {
    await client.restart();
    statusBar?.set("running");
  }
}

export async function deactivate(): Promise<void> {
  // `LanguageClient.stop()` performs the Doc 22 §13.1 shutdown/exit
  // handshake (the extension does not hand-roll it — Doc 27 §2.2).
  if (client) {
    await client.stop();
    client = undefined;
  }
}
