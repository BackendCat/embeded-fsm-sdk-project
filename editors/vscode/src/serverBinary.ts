// `fsm-lang-server` binary resolution — Doc 22 §2.2 / §12, Doc 27 §2.2.
//
// WHY the order is load-bearing: the project's cardinal-sin bar is
// "silent-wrong". The resolver therefore has exactly three outcomes and
// NEVER a silent fallback to PATH or to a stale guess:
//   1. `fsmLang.compilerPath` non-empty  -> use it VERBATIM (Doc 22 §8
//      machine-overridable; the user explicitly pointed at a binary).
//   2. else the bundled host-triple binary (Doc 22 §12 `${platform}-${arch}`
//      scheme). V1 ships host-platform only (Doc 27 risk-2 / §8-V1) — a
//      missing bundled binary is NOT silently downgraded.
//   3. else surface the Doc 22 §12 error notification verbatim and stay
//      inert (the client does not start with a guessed binary).

import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

/** The exact Doc 22 §12 error string (verbatim — do not paraphrase). */
export function noBundledBinaryMessage(platformArch: string): string {
  return (
    `FSM Studio: No bundled binary for ${platformArch}. ` +
    `Please install fsm manually and set fsmLang.compilerPath.`
  );
}

/** `${platform}-${arch}` per the Doc 22 §12 scheme (os.platform/os.arch). */
export function hostTriple(): string {
  return `${os.platform()}-${os.arch()}`;
}

/** Server executable basename — `.exe` only on win32 (Doc 22 §12). */
function serverExeName(): string {
  return os.platform() === "win32" ? "fsm-lang-server.exe" : "fsm-lang-server";
}

export interface ResolvedBinary {
  /** Absolute path to the `fsm-lang-server` executable to launch. */
  readonly command: string;
  /** Which resolution rule won — for the output channel / diagnostics. */
  readonly source: "compilerPath" | "bundled";
}

/**
 * Resolve the server binary per the Doc 22 §2.2 order, or `undefined` if no
 * binary is available (the caller then shows {@link noBundledBinaryMessage}
 * and stays inert — no silent fallback).
 *
 * `extensionPath` is the extension install root; the bundled layout is
 * `<root>/bin/<platform>-<arch>/fsm-lang-server[.exe]` (Doc 22 §12).
 */
export function resolveServerBinary(
  extensionPath: string,
  config: Pick<vscode.WorkspaceConfiguration, "get">,
): ResolvedBinary | undefined {
  // Rule 1: explicit user path wins, used verbatim (Doc 22 §8 / §2.2).
  const compilerPath = (config.get<string>("compilerPath") ?? "").trim();
  if (compilerPath.length > 0) {
    return { command: compilerPath, source: "compilerPath" };
  }

  // Rule 2: bundled host-triple binary (Doc 22 §12). V1 = host-only.
  const bundled = path.join(extensionPath, "bin", hostTriple(), serverExeName());
  if (fs.existsSync(bundled)) {
    return { command: bundled, source: "bundled" };
  }

  // Rule 3: no binary — caller surfaces the verbatim error, stays inert.
  return undefined;
}
