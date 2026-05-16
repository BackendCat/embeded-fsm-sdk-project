// Populate `editors/vscode/bin/<host-triple>/` with the RELEASE `fsm` +
// `fsm-lang-server` binaries, so V1's Doc 22 §2.2 Rule-2 ("bundled
// host-triple") resolution — implemented in `serverBinary.ts` /
// `commands/cliBinary.ts` since V1 but never exercised against a real
// bundle — finally has a real bundle to resolve. Run by `npm run
// populate:bin`, and automatically before `vsce package` via the
// `vscode:prepublish` hook (Doc 28 §3-V6 / Doc 27 §8-V6).
//
// WHY this is a build script and NOT a committed `bin/`:  a ~12 MB pair of
// native binaries in git is a cardinal repo-bloat sin (the brief's hard
// "NO HUGE BINARIES IN GIT" discipline + the project's
// `feedback_agent_no_rsync_prod` lineage). `editors/vscode/bin/` is a
// build artifact: it is `.gitignore`d and produced fresh at package time
// from the pinned-toolchain release build. The committed delta is ONLY
// this script + `.vscodeignore` + `package.json` packaging metadata + the
// acceptance test.
//
// HOST-TRIPLE SCOPE (MV6-1, binding):  this bundles ONLY the host triple.
// The full Doc 22 §12 five-platform cross-compile tail is GATED on G9 (the
// cross-OS CI matrix, which has never run against any commit) — there are
// no cross toolchains on this box and grinding cross-compilation is the
// documented foot-gun. The multi-platform tail is an explicit
// owner/infra-gated action, NOT attempted here (Doc 27 §7 risk-2,
// escalate-don't-grind).
//
// TRIPLE-NAMING (load-bearing, a deliberate judgment call — see the V6
// report):  V1's resolver looks for the bundle at
// `<ext>/bin/<os.platform()>-<os.arch()>/<exe>` (serverBinary.ts
// `hostTriple()` === e.g. `linux-x64`), NOT the Rust target triple
// `x86_64-unknown-linux-gnu`. The brief mandates the bundle "must satisfy
// V1's EXISTING Rule-2 resolver" and "do NOT reimplement resolution", so
// this script populates the directory the resolver ACTUALLY reads. The
// triple is derived from the SAME `os.platform()-os.arch()` expression the
// resolver uses (re-derived here, not hard-coded) so the producer and the
// consumer cannot drift.
//
// TOOLCHAIN TRAP (memory feedback_embeded_fsm_toolchain_probe_trap):  the
// bundled binaries MUST be built on the project-pinned rustc
// (rust-toolchain.toml = 1.75.0), which rustup honours ONLY when `cargo`
// runs from inside the repo. `cargo build` is therefore always invoked
// with cwd = the repo root. We also assert the active toolchain from the
// repo before building (a loud failure beats a silently mis-toolchained
// bundle). The bare `rustc --version` value fluctuates and is irrelevant
// — `rustup show active-toolchain` is the real pin probe.

import { execFileSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { fileURLToPath } from "url";

const here = path.dirname(fileURLToPath(import.meta.url));
const vscodeDir = path.resolve(here, ".."); // editors/vscode
const repoRoot = path.resolve(vscodeDir, "../../"); // repo root

/**
 * The bundle sub-directory name. MUST equal `serverBinary.ts`'s
 * `hostTriple()` (`${os.platform()}-${os.arch()}`) so V1's Rule-2 resolver
 * finds what this script writes. Re-derived from the same expression — the
 * single source of truth is "what the resolver reads".
 */
function hostTriple() {
  return `${os.platform()}-${os.arch()}`;
}

/** `.exe` suffix only on win32 (matches the resolver's exe-name logic). */
function exeSuffix() {
  return os.platform() === "win32" ? ".exe" : "";
}

/**
 * Read the out-of-tree cargo target dir from `.cargo/config.toml` (Doc 28
 * §2: the shared `/root/dev/embeded-fsm-sdk-target` trough). Never
 * hard-codes a path the workspace owns.
 */
function targetDir() {
  const cfg = path.join(repoRoot, ".cargo", "config.toml");
  const txt = fs.readFileSync(cfg, "utf8");
  const m = /^\s*target-dir\s*=\s*"([^"]+)"/m.exec(txt);
  if (!m) {
    throw new Error(`could not read target-dir from ${cfg}`);
  }
  return path.isAbsolute(m[1]) ? m[1] : path.resolve(repoRoot, m[1]);
}

/**
 * Assert the repo's pinned toolchain is the active one (run from the repo
 * so the rust-toolchain.toml override applies). A mis-toolchained bundle
 * must fail loudly here, never ship silently.
 */
function assertPinnedToolchain() {
  const out = execFileSync("rustup", ["show", "active-toolchain"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  // e.g. "1.75.0-x86_64-unknown-linux-gnu (overridden by '…/rust-toolchain.toml')"
  if (!out.startsWith("1.75.0")) {
    throw new Error(
      `populate-bin: refusing to bundle — active toolchain is "${out}", ` +
        `expected the pinned 1.75.0 (rust-toolchain.toml). Run cargo from ` +
        `the repo root so the override applies.`,
    );
  }
  console.log(`populate-bin: pinned toolchain OK — ${out}`);
}

function main() {
  assertPinnedToolchain();

  const target = targetDir();
  const suffix = exeSuffix();
  const serverSrc = path.join(target, "release", `fsm-lang-server${suffix}`);
  const cliSrc = path.join(target, "release", `fsm${suffix}`);

  // Build --release on the pinned toolchain if either binary is absent.
  // cwd = repoRoot so rustup applies rust-toolchain.toml (1.75.0); a
  // --release build keeps the VSIX small (MV6-1).
  if (!fs.existsSync(serverSrc) || !fs.existsSync(cliSrc)) {
    console.log(
      "populate-bin: release binaries absent — building on pinned 1.75…",
    );
    execFileSync(
      "cargo",
      ["build", "--release", "-p", "fsm-lsp", "-p", "fsm-cli"],
      { cwd: repoRoot, stdio: "inherit" },
    );
  }
  if (!fs.existsSync(serverSrc) || !fs.existsSync(cliSrc)) {
    throw new Error(
      `populate-bin: release binaries still missing after build: ` +
        `${serverSrc} / ${cliSrc}`,
    );
  }

  const triple = hostTriple();
  const destDir = path.join(vscodeDir, "bin", triple);
  fs.rmSync(destDir, { recursive: true, force: true });
  fs.mkdirSync(destDir, { recursive: true });

  const serverDest = path.join(destDir, `fsm-lang-server${suffix}`);
  const cliDest = path.join(destDir, `fsm${suffix}`);
  fs.copyFileSync(serverSrc, serverDest);
  fs.copyFileSync(cliSrc, cliDest);
  // Preserve the executable bit (copyFileSync drops mode on some FS).
  fs.chmodSync(serverDest, 0o755);
  fs.chmodSync(cliDest, 0o755);

  const mb = (p) => (fs.statSync(p).size / (1024 * 1024)).toFixed(1);
  console.log(
    `populate-bin: bundled host triple "${triple}" -> bin/${triple}/\n` +
      `  fsm-lang-server  (${mb(serverDest)} MB)\n` +
      `  fsm              (${mb(cliDest)} MB)\n` +
      `populate-bin: multi-platform tail (4 other triples) is G9-GATED — ` +
      `not attempted (MV6-1 / Doc 27 §7 risk-2, owner/infra action).`,
  );
}

main();
