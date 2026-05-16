// `fsm.copyIR` — Doc 27 §5/§6/§10 + Doc 28 §3-V3: the ONLY IR-JSON path is
// `fsm generate --emit-ir` writing `<out>/<first-machine>.ir.json` (there
// is NO `fsm ir` subcommand and NO LSP IR method — R-9/R-10). The command
// runs it to a TEMP dir and copies the file's content to the clipboard.
//
// R-11 CODEGEN-GATED BOUNDARY (the DRIFT-class assertion, Doc 28 §3-V3 /
// Doc 27 §6.1.3): verified against the shipped CLI
// (crates/fsm-cli/src/cmd/generate.rs:165-209) — `--emit-ir` writes the
// `.ir.json` ONLY AFTER codegen `emit()` + the generated-file `fs::write`
// succeed; if codegen fails the CLI `return ExitCode::from(2)` BEFORE the
// IR write, so NO `.ir.json` is produced. This command MUST surface that
// honestly: if the CLI failed / no `.ir.json` exists, it tells the user
// (codegen-gated) and copies NOTHING — never a stale/empty clipboard
// presented as success (the cardinal-sin bar; this is the named V3 gate
// "the test fixture is one that codegen-succeeds … the command must
// surface that honestly, asserted here").

import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveTargetFsm } from "./activeFsm";
import { CommandDeps } from "./index";
import { noCliBinaryMessage, resolveCliBinary } from "./cliBinary";
import { runCli } from "./cliRunner";
import { error, errorWithLog, info, warn } from "./notify";

export function registerCopyIr(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.copyIR", async (arg) => {
      const fsmPath = resolveTargetFsm(arg);
      if (!fsmPath) {
        warn("FSM Studio: open a .fsm file to copy its IR.");
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

      // Generate into an isolated temp dir so we never pollute the
      // workspace just to read the IR (Doc 27 §5: "runs it to a temp
      // dir and copies the file's content").
      const tmpDir = fs.mkdtempSync(
        path.join(os.tmpdir(), "fsm-copyir-"),
      );
      try {
        const args = [
          "generate",
          "--target",
          "c99",
          "--out",
          tmpDir,
          "--emit-ir",
          fsmPath,
        ];
        deps.outputChannel.appendLine(
          `[fsm] fsm.copyIR: ${cli.command} ${args.join(" ")}`,
        );
        const res = await runCli(cli.command, args, {
          cwd: path.dirname(fsmPath),
        });

        if (res.spawnError) {
          deps.outputChannel.appendLine(
            `[fsm] copyIR could not spawn the CLI: ${res.stderr}`,
          );
          error(
            `FSM Studio: could not run fsm generate — ` +
              `${res.stderr.trim()}`,
          );
          return;
        }
        if (res.stderr.trim().length > 0) {
          deps.outputChannel.appendLine(res.stderr.trimEnd());
        }

        // R-11 codegen-gated boundary, surfaced honestly: a non-zero
        // exit means codegen failed and NO .ir.json was written. Do NOT
        // copy anything; tell the user exactly why.
        if (res.code !== 0) {
          const detail =
            res.stderr.trim().length > 0
              ? res.stderr.trim().split("\n")[0]
              : `fsm generate exited with code ${res.code}`;
          // Fire-and-handle: the command is DONE (correctly declining to
          // copy); the toast must not pin it "in progress".
          errorWithLog(
            "FSM Studio: cannot copy IR — codegen failed, so no IR was " +
              `produced (the IR is codegen-gated). ${detail}`,
            deps.outputChannel,
          );
          return;
        }

        // Locate the single `*.ir.json` the CLI wrote (named after the
        // first machine — crates/fsm-cli/src/cmd/generate.rs:189-209).
        const irFiles = fs
          .readdirSync(tmpDir)
          .filter((f) => f.endsWith(".ir.json"));
        if (irFiles.length === 0) {
          // Defensive: exit 0 but no IR file (should not happen given the
          // verified CLI contract) — still never fake a clipboard.
          errorWithLog(
            "FSM Studio: cannot copy IR — the CLI reported success but " +
              "wrote no .ir.json file.",
            deps.outputChannel,
          );
          return;
        }

        const irPath = path.join(tmpDir, irFiles[0]);
        const irJson = fs.readFileSync(irPath, "utf8");
        await vscode.env.clipboard.writeText(irJson);
        deps.outputChannel.appendLine(
          `[fsm] copied IR (${irFiles[0]}, ${irJson.length} bytes) ` +
            "to the clipboard.",
        );
        info(
          `FSM Studio: IR JSON copied to the clipboard (${irFiles[0]}).`,
        );
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    }),
  );
}
