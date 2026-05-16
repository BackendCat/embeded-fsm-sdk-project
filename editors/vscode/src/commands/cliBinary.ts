// `fsm` CLI binary resolution for the V3 CLI-wrapper commands — the
// sibling of V1's `serverBinary.ts` (Doc 27 §8-V3 / §5; Doc 22 §4).
//
// WHY a SECOND resolver (and not a reuse of V1's serverBinary.ts): Doc 27
// §2.2 (lines 107-111) explicitly records a *genuine two-binary need* —
// the language client needs `fsm-lang-server` (V1's serverBinary.ts), but
// the V3 codegen / check / fmt / IR commands shell the SEPARATE `fsm` CLI
// ("see §6/§10 for why the diagram path *also* needs the `fsm` CLI, a
// genuine two-binary need Doc 22 §12 under-describes"). `fsmLang.compilerPath`
// points at the SERVER binary (Doc 27 §2.2: "NOT the `fsm` CLI, despite the
// word 'compiler'"). This resolver therefore mirrors V1's exact three-rule
// Doc 22 §2.2 shape but resolves `fsm`, deriving Rule 1 from the SIBLING of
// the configured server path (the two binaries are produced into the same
// directory by one build, and the bundled layout puts them side-by-side per
// the Doc 22 §12 `bin/<triple>/` scheme). The cardinal-sin bar is preserved
// verbatim: exactly three outcomes, NEVER a silent PATH/guess fallback.
//
// This file is V3-owned (editors/vscode/src/commands/**). It does NOT touch
// V1's serverBinary.ts (the SUBAGENT §6 disjoint-scope bar).

import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

import { hostTriple } from "../serverBinary";

/** CLI executable basename — `.exe` only on win32 (Doc 22 §12 scheme). */
function cliExeName(): string {
  return os.platform() === "win32" ? "fsm.exe" : "fsm";
}

/** Server executable basename — to derive the CLI sibling from Rule 1. */
function serverExeName(): string {
  return os.platform() === "win32" ? "fsm-lang-server.exe" : "fsm-lang-server";
}

/**
 * The exact Doc 22 §12-shaped error for a missing CLI (verbatim shape of
 * V1's `noBundledBinaryMessage`, retargeted to the `fsm` CLI — the V3
 * commands need the CLI, not the server). Never paraphrase into a silent
 * no-op (the cardinal-sin bar at the command boundary).
 */
export function noCliBinaryMessage(platformArch: string): string {
  return (
    `FSM Studio: No bundled fsm CLI for ${platformArch}. ` +
    `Please install fsm manually and set fsmLang.compilerPath to the ` +
    `fsm-lang-server next to it.`
  );
}

export interface ResolvedCli {
  /** Absolute path to the `fsm` CLI executable to shell. */
  readonly command: string;
  /** Which resolution rule won — for the output channel / diagnostics. */
  readonly source: "compilerPath-sibling" | "bundled";
}

/**
 * Resolve the `fsm` CLI per the Doc 22 §2.2 order (mirrored from
 * `serverBinary.ts`), or `undefined` if none is available (the caller then
 * shows {@link noCliBinaryMessage} and does NOT shell a guessed binary).
 *
 *   1. `fsmLang.compilerPath` set → the `fsm` CLI is the sibling of that
 *      `fsm-lang-server` path (same build output dir; Doc 27 §2.2's
 *      two-binary need). Used only if that sibling actually exists.
 *   2. else the bundled host-triple CLI
 *      `<extensionPath>/bin/<platform>-<arch>/fsm[.exe]` (Doc 22 §12).
 *   3. else `undefined` → caller surfaces the verbatim error, no fallback.
 */
export function resolveCliBinary(
  extensionPath: string,
  config: Pick<vscode.WorkspaceConfiguration, "get">,
): ResolvedCli | undefined {
  // Rule 1: derive the CLI from the explicit server path's directory.
  const compilerPath = (config.get<string>("compilerPath") ?? "").trim();
  if (compilerPath.length > 0) {
    const dir = path.dirname(compilerPath);
    // Prefer the exact `fsm` sibling; if the user pointed compilerPath at
    // something not named `fsm-lang-server`, still look for `fsm` in the
    // same dir (the two binaries always co-build into one directory).
    const sibling = path.join(dir, cliExeName());
    if (path.basename(compilerPath) === serverExeName() || fs.existsSync(sibling)) {
      if (fs.existsSync(sibling)) {
        return { command: sibling, source: "compilerPath-sibling" };
      }
    }
  }

  // Rule 2: bundled host-triple CLI (Doc 22 §12 `bin/<triple>/` scheme).
  const bundled = path.join(extensionPath, "bin", hostTriple(), cliExeName());
  if (fs.existsSync(bundled)) {
    return { command: bundled, source: "bundled" };
  }

  // Rule 3: no CLI — caller surfaces the verbatim error, stays inert.
  return undefined;
}
