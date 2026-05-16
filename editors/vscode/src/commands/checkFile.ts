// `fsm.checkFile` — Doc 27 §5: "the live publishDiagnostics already covers
// 'check' continuously; the explicit command is a convenience that runs
// `fsm check <file>` (the CLI) and surfaces its exit/summary. It MUST NOT
// spawn a second analyzer in JS — the squiggle is already the server's
// `fsm check`; the command is a user-invoked ECHO of the same pipeline,
// not a parallel one." This handler therefore shells the real `fsm` CLI
// and reports its exit/summary; it never parses/analyses in JS.

import * as path from "path";

import * as vscode from "vscode";

import { resolveTargetFsm } from "./activeFsm";
import { CommandDeps } from "./index";
import { noCliBinaryMessage, resolveCliBinary } from "./cliBinary";
import { runCli } from "./cliRunner";
import { error, errorWithLog, info, warn, warnWithLog } from "./notify";

export function registerCheckFile(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.checkFile", async (arg) => {
      const fsmPath = resolveTargetFsm(arg);
      if (!fsmPath) {
        warn("FSM Studio: open a .fsm file to check.");
        return;
      }

      const cli = resolveCliBinary(
        deps.extensionPath,
        vscode.workspace.getConfiguration("fsmLang"),
      );
      if (!cli) {
        // No silent no-op — surface the verbatim missing-CLI message.
        error(noCliBinaryMessage(`${process.platform}-${process.arch}`));
        return;
      }

      const file = path.basename(fsmPath);
      deps.outputChannel.appendLine(
        `[fsm] fsm.checkFile: ${cli.command} check ${fsmPath}`,
      );
      const res = await runCli(cli.command, ["check", fsmPath]);

      if (res.spawnError) {
        deps.outputChannel.appendLine(
          `[fsm] check could not spawn the CLI: ${res.stderr}`,
        );
        error(
          `FSM Studio: could not run fsm check — ${res.stderr.trim()}`,
        );
        return;
      }

      // Echo the CLI's own human output verbatim into the log so the user
      // sees the exact same pipeline result the squiggle already shows.
      if (res.stdout.trim().length > 0) {
        deps.outputChannel.appendLine(res.stdout.trimEnd());
      }
      if (res.stderr.trim().length > 0) {
        deps.outputChannel.appendLine(res.stderr.trimEnd());
      }

      // `fsm check` exits 0 = clean, 1 = diagnostics present (NOT a
      // command failure — surface it as the real result, honestly).
      // Notifications are fire-and-handle (never awaited): the check has
      // already run; the toast must not pin the command "in progress".
      if (res.code === 0) {
        info(`FSM Studio: ${file} — no problems found.`);
      } else if (res.code === 1) {
        warnWithLog(
          `FSM Studio: ${file} — problems found (see the Problems panel ` +
            "and the FSM Language Server log).",
          deps.outputChannel,
        );
      } else {
        errorWithLog(
          `FSM Studio: fsm check exited with code ${res.code}.`,
          deps.outputChannel,
        );
      }
    }),
  );
}
