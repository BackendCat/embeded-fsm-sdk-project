// Locate (build-if-absent) the REAL `fsm-lang-server` + `fsm` binaries the
// V1 acceptance gate launches (Doc 28 §5: "the real fsm-lang-server
// binary", not a stub / not a re-impl).
//
// TOOLCHAIN TRAP (Doc 28 §5 / memory feedback_embeded_fsm_toolchain_probe_
// trap): the server MUST be built on the project-pinned rustc (rust-
// toolchain.toml), which rustup honours ONLY when `cargo` runs from inside
// the repo. We therefore always invoke `cargo build` with cwd = the repo
// root so the override applies. The CI step also asserts the active
// toolchain from the repo before invoking this.

import { execFileSync } from "child_process";
import * as fs from "fs";
import * as path from "path";

/** Repo root = three levels up from `editors/vscode/out/test/suite`. */
function repoRoot(): string {
  // __dirname = <repo>/editors/vscode/out/test/suite
  return path.resolve(__dirname, "../../../../../");
}

/**
 * The out-of-tree cargo target dir (Doc 28 §2: `.cargo/config.toml`
 * `target-dir`). Read it from the config so this never hard-codes a path
 * the workspace owns.
 */
function targetDir(root: string): string {
  const cfg = path.join(root, ".cargo", "config.toml");
  const txt = fs.readFileSync(cfg, "utf8");
  const m = /^\s*target-dir\s*=\s*"([^"]+)"/m.exec(txt);
  if (!m) {
    throw new Error(`could not read target-dir from ${cfg}`);
  }
  return path.isAbsolute(m[1]) ? m[1] : path.resolve(root, m[1]);
}

export interface RealBinaries {
  readonly server: string;
  readonly cli: string;
  readonly repoRoot: string;
}

/**
 * Return absolute paths to the built `fsm-lang-server` + `fsm`, building
 * them on the pinned toolchain (cwd = repo root → rust-toolchain.toml
 * override applies) if either is missing.
 */
export function resolveRealBinaries(): RealBinaries {
  const root = repoRoot();
  const target = targetDir(root);
  const server = path.join(target, "debug", "fsm-lang-server");
  const cli = path.join(target, "debug", "fsm");

  if (!fs.existsSync(server) || !fs.existsSync(cli)) {
    // Build from the repo root so rustup applies rust-toolchain.toml
    // (1.75.0). Building from editors/vscode/ would NOT pick up the pin.
    execFileSync(
      "cargo",
      ["build", "-p", "fsm-lsp", "--bin", "fsm-lang-server", "-p", "fsm-cli", "--bin", "fsm"],
      { cwd: root, stdio: "inherit" },
    );
  }
  if (!fs.existsSync(server) || !fs.existsSync(cli)) {
    throw new Error(`real binaries missing after build: server=${server} cli=${cli}`);
  }
  return { server, cli, repoRoot: root };
}
