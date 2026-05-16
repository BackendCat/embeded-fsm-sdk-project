// `fsm.generateC99` / `fsm.generateCpp17` — Doc 27 §5/§8-V3: shell the
// `fsm` CLI `generate --target {c99|cpp17} --out <fsmLang.codegen.outputDir>`
// (the CLI is the only codegen path; the LSP has none). `fsmLang.codegen.*`
// (Doc 22 §8: outputDir / strategy) feed these — the keys the SERVER
// ignores but are meaningful HERE (Doc 27 §2.5: "consumed by the
// extension's own commands where they are actually meaningful").
//
// GRACEFUL DEGRADATION (Doc 27 §5): "C++17 codegen is itself a v1.2
// deliverable; the command degrades gracefully (clear error) if the
// bundled `fsm` lacks the target." Verified against the shipped CLI
// (crates/fsm-cli/src/cmd/generate.rs:28-36): `fsm generate --target
// cpp17` prints `error: unknown target 'cpp17' — v1.0 only ships 'c99'`
// and exits 2. This handler surfaces that CLI error VERBATIM and honestly
// (never a silent success — the cardinal-sin bar).

import * as path from "path";

import * as vscode from "vscode";

import { resolveTargetFsm } from "./activeFsm";
import { CommandDeps } from "./index";
import { noCliBinaryMessage, resolveCliBinary } from "./cliBinary";
import { runCli } from "./cliRunner";
import { error, errorWithLog, infoWithLog, warn } from "./notify";

type Target = "c99" | "cpp17";

/**
 * Resolve the codegen output directory (Doc 22 §8
 * `fsmLang.codegen.outputDir`, default "generated"). A relative value is
 * rooted at the .fsm file's workspace folder (or, with no folder, the
 * file's own directory) — so the command works for a single loose file.
 */
function resolveOutputDir(fsmPath: string): string {
  const cfg = vscode.workspace.getConfiguration("fsmLang");
  const configured = (cfg.get<string>("codegen.outputDir") ?? "generated").trim();
  if (path.isAbsolute(configured)) {
    return configured;
  }
  const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(fsmPath));
  const root = folder ? folder.uri.fsPath : path.dirname(fsmPath);
  return path.join(root, configured);
}

/** Doc 22 §8 `fsmLang.codegen.strategy` (switch|table; CLI also `auto`). */
function resolveStrategy(): string {
  const s = (
    vscode.workspace.getConfiguration("fsmLang").get<string>("codegen.strategy") ?? "switch"
  ).trim();
  return s.length > 0 ? s : "switch";
}

/**
 * The remaining `fsm generate` knobs the CLI honours, surfaced as the
 * v1.5 W-B1 JC-3 settings (`fsmLang.codegen.{queueSize,license,emitIr,
 * reportMemory}`). Declaring a setting the handler then ignores would be
 * a discoverability lie in the other direction — these are passed to the
 * CLI exactly when set, never invented (`--import-header` is deliberately
 * NOT a setting: it is a repeatable per-invocation path list with
 * documented trusted-invoker security semantics whose proper home is
 * `fsm.toml [generate] import_headers`, not a global editor setting).
 */
function appendOptionalCodegenFlags(args: string[]): void {
  const cfg = vscode.workspace.getConfiguration("fsmLang");

  const queueSize = cfg.get<number | null>("codegen.queueSize");
  if (typeof queueSize === "number" && Number.isInteger(queueSize)) {
    args.push("--queue-size", String(queueSize));
  }

  // Only pass --license when it diverges from the CLI's own "MIT"
  // default, so an unchanged setting never alters the invocation.
  const license = (cfg.get<string>("codegen.license") ?? "MIT").trim();
  if (license.length > 0 && license !== "MIT") {
    args.push("--license", license);
  }

  if (cfg.get<boolean>("codegen.emitIr") === true) {
    args.push("--emit-ir");
  }
  if (cfg.get<boolean>("codegen.reportMemory") === true) {
    args.push("--report-memory");
  }
}

async function runGenerate(target: Target, deps: CommandDeps, arg: unknown): Promise<void> {
  const fsmPath = resolveTargetFsm(arg);
  if (!fsmPath) {
    warn("FSM Studio: open a .fsm file to generate code.");
    return;
  }

  const cli = resolveCliBinary(deps.extensionPath, vscode.workspace.getConfiguration("fsmLang"));
  if (!cli) {
    error(noCliBinaryMessage(`${process.platform}-${process.arch}`));
    return;
  }

  const outDir = resolveOutputDir(fsmPath);
  const strategy = resolveStrategy();
  const args = ["generate", "--target", target, "--out", outDir, "--strategy", strategy];
  appendOptionalCodegenFlags(args);
  // The CLI requires the .fsm path(s) as the trailing positional arg(s)
  // (`#[arg(required = true)] files`), so it goes last, after any flags.
  args.push(fsmPath);
  deps.outputChannel.appendLine(
    `[fsm] fsm.generate${target === "c99" ? "C99" : "Cpp17"}: ` +
      `${cli.command} ${args.join(" ")}`,
  );

  const res = await runCli(cli.command, args, {
    cwd: path.dirname(fsmPath),
  });

  if (res.spawnError) {
    deps.outputChannel.appendLine(`[fsm] generate could not spawn the CLI: ${res.stderr}`);
    error(`FSM Studio: could not run fsm generate — ${res.stderr.trim()}`);
    return;
  }

  // The CLI writes progress ("wrote <path>") + errors to stderr.
  if (res.stdout.trim().length > 0) {
    deps.outputChannel.appendLine(res.stdout.trimEnd());
  }
  if (res.stderr.trim().length > 0) {
    deps.outputChannel.appendLine(res.stderr.trimEnd());
  }

  if (res.code === 0) {
    // Fire-and-handle: the files are already written; the toast must not
    // pin the command "in progress" waiting for the user to dismiss it.
    infoWithLog(
      `FSM Studio: ${target.toUpperCase()} code written to ${outDir}.`,
      deps.outputChannel,
    );
    return;
  }

  // Non-zero exit — surface the CLI's OWN error verbatim (this is the
  // graceful-degradation path for cpp17 on a v1.0 CLI: the user sees
  // exactly `error: unknown target 'cpp17' — v1.0 only ships 'c99'`,
  // never a fake success). Fire-and-handle, not awaited.
  const detail =
    res.stderr.trim().length > 0
      ? res.stderr.trim().split("\n")[0]
      : `fsm generate exited with code ${res.code}`;
  errorWithLog(
    `FSM Studio: ${target.toUpperCase()} generation failed — ${detail}`,
    deps.outputChannel,
  );
}

export function registerGenerate(context: vscode.ExtensionContext, deps: CommandDeps): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.generateC99", (arg) => runGenerate("c99", deps, arg)),
    vscode.commands.registerCommand("fsm.generateCpp17", (arg) => runGenerate("cpp17", deps, arg)),
  );
}
