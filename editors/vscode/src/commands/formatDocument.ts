// `fsm.formatDocument` — Doc 27 §8-V3: shell the `fsm` CLI formatter (the
// LSP has no formatting; the CLI `fsm fmt` is the only formatter path).
//
// WHY `fsm fmt --stdin` (not `fsm fmt <file>`): the active buffer may be
// DIRTY (unsaved edits). `fsm fmt <file>` formats the ON-DISK file in
// place — it would silently format stale content and desync the editor.
// `fsm fmt --stdin` (verified crates/fsm-cli/src/cmd/fmt.rs:18-20,72-80:
// "read from stdin, write formatted output to stdout") formats the EXACT
// current buffer text with no file mutation; the result is applied as a
// single editor edit (so undo works and the buffer stays the source of
// truth). The formatter is the canonical `fsm-formatter` — no JS
// re-implementation (Doc 27 §5 "thin glue over the `fsm` CLI").

import * as vscode from "vscode";

import { CommandDeps } from "./index";
import { noCliBinaryMessage, resolveCliBinary } from "./cliBinary";
import { runCli } from "./cliRunner";
import { error, errorWithLog, info, warn } from "./notify";

export function registerFormatDocument(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.formatDocument", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor || editor.document.languageId !== "fsm-lang") {
        warn("FSM Studio: open a .fsm file to format.");
        return;
      }

      const cli = resolveCliBinary(
        deps.extensionPath,
        vscode.workspace.getConfiguration("fsmLang"),
      );
      if (!cli) {
        error(noCliBinaryMessage(`${process.platform}-${process.arch}`));
        return;
      }

      const doc = editor.document;
      const original = doc.getText();
      deps.outputChannel.appendLine(
        `[fsm] fsm.formatDocument: ${cli.command} fmt --stdin ` +
          `(${doc.uri.fsPath})`,
      );

      const res = await runCli(cli.command, ["fmt", "--stdin"], {
        stdin: original,
      });

      if (res.spawnError) {
        deps.outputChannel.appendLine(
          `[fsm] format could not spawn the CLI: ${res.stderr}`,
        );
        error(`FSM Studio: could not run fsm fmt — ${res.stderr.trim()}`);
        return;
      }

      // The formatter refuses to format un-parseable input (it exits
      // non-zero and writes the reason to stderr). Surface that honestly
      // (fire-and-handle, not awaited); do NOT replace the buffer with
      // empty/garbage.
      if (res.code !== 0) {
        const detail =
          res.stderr.trim().length > 0
            ? res.stderr.trim().split("\n")[0]
            : `fsm fmt exited with code ${res.code}`;
        errorWithLog(
          `FSM Studio: cannot format — ${detail}`,
          deps.outputChannel,
        );
        return;
      }

      const formatted = res.stdout;
      if (formatted === original) {
        info("FSM Studio: document is already formatted.");
        return;
      }

      // Apply as ONE replace edit over the whole document so a single
      // undo reverts it and the buffer remains the source of truth.
      const fullRange = new vscode.Range(
        doc.positionAt(0),
        doc.positionAt(original.length),
      );
      const ok = await editor.edit((eb) => {
        eb.replace(fullRange, formatted);
      });
      if (ok) {
        deps.outputChannel.appendLine(
          "[fsm] document formatted (single replace edit).",
        );
      } else {
        error("FSM Studio: failed to apply the formatting edit.");
      }
    }),
  );
}
