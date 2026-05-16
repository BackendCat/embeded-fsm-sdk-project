// V6 behavioural-acceptance gate — Doc 28 §3-V6 / Doc 27 §8-V6. NOT
// symbol-presence, NOT "vsce exited 0 / a .vsix exists" (the brief
// explicitly excludes that). The two proofs the V6 gate requires:
//
//   (1) PACKAGING — run the `bin/`-population script + `vsce package`,
//       then assert the produced `.vsix` (via `vsce ls`) CONTAINS
//       `extension/bin/<host-triple>/fsm` + `fsm-lang-server` AND
//       `extension/dist/`, and EXCLUDES `node_modules` / `src` / `test`
//       (a real, lean, installable VSIX — the duck-principle bar).
//
//   (2) RULE-2 ROUND-TRIP — this suite runs in a DEDICATED Extension Host
//       launched with a pristine `--user-data-dir` and `fsmLang.compiler-
//       Path` NEVER set, while `editors/vscode/bin/<host-triple>/` is
//       populated. The extension therefore activates down V1's EXISTING
//       Doc 22 §2.2 Rule-2 ("bundled host-triple") path — implemented in
//       `serverBinary.ts` since V1 but, until a real bundle existed,
//       NEVER exercised end-to-end. We assert (a) `api.binarySource ===
//       "bundled"` (Rule-2 won, not Rule-1/3) AND (b) a real diagnostic
//       round-trips from the BUNDLED `fsm-lang-server`, byte-equal to the
//       `fsm check --json` oracle computed with the BUNDLED `fsm` CLI.
//
// This file is V6-owned (NEW). It does NOT touch V1's serverBinary.ts /
// client-spawn nor the V2–V5 suites. It REUSES — does not reimplement —
// V1's resolver (via the running extension) and the established
// oracle.ts (the SUBAGENT §6 disjoint-scope + no-reimplement bars).

import { execFileSync } from "child_process";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import {
  OracleDiag,
  assertAsciiColumnInvariant,
  cliOracle,
} from "../suite/oracle";

// __dirname = <vscodeDir>/out/test/suiteV6  →  <vscodeDir>
const VSCODE_DIR = path.resolve(__dirname, "../../../");
const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");

/** MUST equal serverBinary.ts `hostTriple()` (`platform-arch`). */
function hostTriple(): string {
  return `${os.platform()}-${os.arch()}`;
}
function exeSuffix(): string {
  return os.platform() === "win32" ? ".exe" : "";
}

interface FsmExtensionApi {
  readonly serverStarted: boolean;
  readonly negotiatedPositionEncoding: string | undefined;
  readonly binarySource: "compilerPath" | "bundled" | "none";
}

async function waitForDiagnostics(
  uri: vscode.Uri,
  predicate: (d: readonly vscode.Diagnostic[]) => boolean,
  timeoutMs = 20_000,
): Promise<readonly vscode.Diagnostic[]> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const diags = vscode.languages.getDiagnostics(uri);
    if (predicate(diags)) {
      return diags;
    }
    if (Date.now() > deadline) {
      throw new Error(
        `timed out waiting for diagnostics on ${uri.fsPath}; ` +
          `last = ${JSON.stringify(diags)}`,
      );
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

function assertRangeMatchesOracle(
  got: vscode.Diagnostic,
  want: OracleDiag,
  label: string,
): void {
  assert.strictEqual(got.code, want.code, `[${label}] code == CLI oracle`);
  assert.strictEqual(
    got.range.start.line,
    want.startLine,
    `[${label}] start.line byte-matches fsm check --json`,
  );
  assert.strictEqual(
    got.range.start.character,
    want.startChar,
    `[${label}] start.character byte-matches fsm check --json`,
  );
  assert.strictEqual(
    got.range.end.line,
    want.endLine,
    `[${label}] end.line byte-matches fsm check --json`,
  );
  assert.strictEqual(
    got.range.end.character,
    want.endChar,
    `[${label}] end.character byte-matches fsm check --json`,
  );
}

suite("FSM Studio V6 — bundled-binary VSIX + Rule-2 acceptance", () => {
  let api: FsmExtensionApi;
  let bundledCli: string;
  let bundledServer: string;
  let tmpDir: string;

  suiteSetup(async function () {
    this.timeout(600_000); // a cold release build + vsce package is slow.

    // 1. Populate editors/vscode/bin/<triple>/ with the pinned-1.75
    //    RELEASE binaries (the build artifact V1's Rule-2 resolves). The
    //    script asserts the pinned toolchain itself and builds --release
    //    from the repo root if absent (MV6-1). We do NOT reimplement it
    //    here — we run the SAME script the package lane runs.
    execFileSync("node", ["scripts/populate-bin.mjs"], {
      cwd: VSCODE_DIR,
      stdio: "inherit",
    });

    const triple = hostTriple();
    const sfx = exeSuffix();
    bundledServer = path.join(
      VSCODE_DIR,
      "bin",
      triple,
      `fsm-lang-server${sfx}`,
    );
    bundledCli = path.join(VSCODE_DIR, "bin", triple, `fsm${sfx}`);
    assert.ok(
      fs.existsSync(bundledServer),
      `populate-bin must have produced ${bundledServer}`,
    );
    assert.ok(
      fs.existsSync(bundledCli),
      `populate-bin must have produced ${bundledCli}`,
    );

    // 2. R-15 isolation: stage fixtures in an OS temp dir (NOT under the
    //    repo) so the bundled CLI's upward fsm.toml walk finds none —
    //    apply_allow_deny is a guaranteed no-op and the oracle equals the
    //    (allow/deny-unaware) bundled-server squiggle.
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v6-accept-"));
    for (const f of ["broken.fsm", "non_ascii_doc.fsm"]) {
      fs.copyFileSync(path.join(FIXTURE_SRC, f), path.join(tmpDir, f));
    }
    let dir = tmpDir;
    for (;;) {
      assert.ok(
        !fs.existsSync(path.join(dir, "fsm.toml")),
        `R-15 violated: fsm.toml found at ${dir}`,
      );
      const parent = path.dirname(dir);
      if (parent === dir) {
        break;
      }
      dir = parent;
    }

    // 3. PROVE Rule-2 is the active path: fsmLang.compilerPath MUST be
    //    empty (this Host is launched with a pristine --user-data-dir, so
    //    it already is — assert it loudly rather than trust it).
    const cfg = vscode.workspace
      .getConfiguration("fsmLang")
      .get<string>("compilerPath");
    assert.ok(
      cfg === undefined || cfg.trim() === "",
      `V6 Rule-2 precondition: fsmLang.compilerPath must be UNSET so the ` +
        `resolver falls through Rule-1 to Rule-2; got ${JSON.stringify(cfg)}`,
    );

    // 4. Activate the extension. With compilerPath empty + bin/<triple>/
    //    populated, V1's resolveServerBinary returns {source:"bundled"}
    //    and the client spawns the BUNDLED server (Rule-2, first time
    //    ever exercised against a real bundle).
    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    api = (await ext.activate()) as FsmExtensionApi;

    const startDeadline = Date.now() + 30_000;
    while (!api.serverStarted) {
      if (Date.now() > startDeadline) {
        throw new Error(
          "bundled language server never reached Running state",
        );
      }
      await new Promise((r) => setTimeout(r, 150));
    }
  });

  suiteTeardown(() => {
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  // (1) PACKAGING — populate-bin already ran in suiteSetup; bundle the
  // extension and `vsce package`, then assert the .vsix file list.
  test("(1) vsce package yields a lean installable VSIX with bundled bin/ + dist/, excluding node_modules/src/test", function () {
    this.timeout(300_000);

    // esbuild the extension so dist/ exists (the runtime payload).
    execFileSync("npm", ["run", "bundle"], {
      cwd: VSCODE_DIR,
      stdio: "inherit",
    });

    const vsixPath = path.join(VSCODE_DIR, "fsm-lang-v6-accept.vsix");
    if (fs.existsSync(vsixPath)) {
      fs.rmSync(vsixPath);
    }
    // The same flags the committed `package` script uses
    // (--no-dependencies: the extension is esbuild-bundled, so the VSIX
    // ships ZERO runtime node_modules — see .vscodeignore rationale).
    const vsceBin = path.join(
      VSCODE_DIR,
      "node_modules",
      ".bin",
      "vsce",
    );
    execFileSync(
      vsceBin,
      ["package", "--no-dependencies", "--out", vsixPath],
      { cwd: VSCODE_DIR, stdio: "inherit" },
    );
    assert.ok(fs.existsSync(vsixPath), "vsce package must emit the .vsix");

    // The authoritative content listing — `vsce ls` prints exactly the
    // files vsce put in the package (NOT a guess about .vscodeignore).
    const listing = execFileSync(
      vsceBin,
      ["ls", "--no-dependencies"],
      { cwd: VSCODE_DIR, encoding: "utf8" },
    );
    const files = listing
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);

    const triple = hostTriple();
    const sfx = exeSuffix();
    const wantServer = `bin/${triple}/fsm-lang-server${sfx}`;
    const wantCli = `bin/${triple}/fsm${sfx}`;

    // INCLUDED: the bundled host-triple binaries + the esbuild runtime.
    assert.ok(
      files.includes(wantServer),
      `VSIX must contain ${wantServer}; got:\n${files.join("\n")}`,
    );
    assert.ok(
      files.includes(wantCli),
      `VSIX must contain ${wantCli}; got:\n${files.join("\n")}`,
    );
    assert.ok(
      files.some((f) => f === "dist/extension.js"),
      `VSIX must contain dist/extension.js (the extension main); ` +
        `got:\n${files.join("\n")}`,
    );

    // EXCLUDED: source, tests, node_modules (lean — duck-principle).
    const bad = files.filter(
      (f) =>
        f.startsWith("node_modules/") ||
        f.startsWith("src/") ||
        f.startsWith("out/") ||
        f.includes("/test/") ||
        f.startsWith("test/") ||
        f.endsWith(".ts"),
    );
    assert.deepStrictEqual(
      bad,
      [],
      `VSIX must EXCLUDE node_modules/src/out/test/*.ts; leaked:\n` +
        bad.join("\n"),
    );

    // The .vsix is a real zip whose `extension/` prefix carries the same
    // payload (the on-disk artifact, not just `vsce ls`'s view).
    const zipList = execFileSync("unzip", ["-l", vsixPath], {
      encoding: "utf8",
    });
    assert.ok(
      zipList.includes(`extension/${wantServer}`) &&
        zipList.includes(`extension/${wantCli}`) &&
        zipList.includes("extension/dist/extension.js"),
      `the .vsix zip must hold extension/${wantServer} + ` +
        `extension/${wantCli} + extension/dist/extension.js`,
    );
    assert.ok(
      !/(\s|\/)node_modules\//.test(zipList) &&
        !/extension\/src\//.test(zipList),
      "the .vsix zip must NOT contain node_modules/ or src/",
    );

    const sizeMb = (
      fs.statSync(vsixPath).size /
      (1024 * 1024)
    ).toFixed(2);
    // eslint-disable-next-line no-console
    console.log(
      `V6: installable .vsix = ${sizeMb} MB; ` +
        `${files.length} entries; bundled host triple "${triple}".`,
    );

    // The .vsix is a build artifact (gitignored) — clean it up so the
    // worktree stays binary-free; the listing assertions above are the
    // durable proof.
    fs.rmSync(vsixPath, { force: true });
  });

  // (2a) — Rule-2 actually won (NOT Rule-1 compilerPath, NOT Rule-3
  // none). This is the assertion the brief pins: api.binarySource must
  // be exactly "bundled", proving V1's never-exercised Rule-2 path fired
  // against the real bundle.
  test("(2a) the extension resolved the BUNDLED binary via V1 Rule-2 (binarySource === 'bundled')", () => {
    assert.strictEqual(
      api.binarySource,
      "bundled",
      "with fsmLang.compilerPath unset and bin/<host-triple>/ populated, " +
        "V1's resolveServerBinary MUST return source:'bundled' (Rule-2) — " +
        "the path implemented since V1 but never exercised until a real " +
        "bundle existed (Doc 22 §2.2 / Doc 27 §8-V6)",
    );
    assert.strictEqual(
      api.serverStarted,
      true,
      "the BUNDLED fsm-lang-server must have reached Running",
    );
  });

  // (2b) — a real diagnostic round-trips from the BUNDLED server, byte-
  // equal to the `fsm check --json` oracle computed with the BUNDLED
  // CLI. This proves the bundled binary actually WORKS end-to-end (the
  // SEC-P0-1 path-canon seam included), not merely that it was resolved.
  test("(2b) a known-broken .fsm round-trips the exact CLI-oracle diagnostic THROUGH the bundled server", async () => {
    const fixture = path.join(tmpDir, "broken.fsm");
    const source = fs.readFileSync(fixture, "utf8");
    // Oracle source = the BUNDLED `fsm` CLI (same bundle as the server).
    const oracle = cliOracle(bundledCli, fixture);
    assertAsciiColumnInvariant(source, oracle);
    assert.strictEqual(
      oracle.length,
      1,
      `oracle precondition: broken.fsm has exactly one diagnostic via the ` +
        `bundled CLI, got ${JSON.stringify(oracle)}`,
    );

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    const diags = await waitForDiagnostics(
      doc.uri,
      (d) => d.length === 1,
    );
    assertRangeMatchesOracle(diags[0], oracle[0], "broken.fsm (bundled)");
  });

  // (2c) — the Doc 26 §4.1 multibyte-line defect-class guard, re-run
  // end-to-end THROUGH the bundled server (the path-canon / position
  // seam is the sharpest per-OS risk the bundle must not regress —
  // Doc 27 §7 risk-2).
  test("(2c) error after a non-ASCII line has the exact bundled-CLI-oracle Range", async () => {
    const fixture = path.join(tmpDir, "non_ascii_doc.fsm");
    const source = fs.readFileSync(fixture, "utf8");
    const oracle = cliOracle(bundledCli, fixture);
    assertAsciiColumnInvariant(source, oracle);
    assert.ok(
      oracle.length >= 1,
      "oracle precondition: non_ascii_doc.fsm has >= 1 diagnostic",
    );
    const first = oracle[0];
    assert.strictEqual(
      first.code,
      "FSM-E0100",
      "first diagnostic is the unknown-state-reference error",
    );
    assert.ok(
      first.startLine > 2,
      `the guarded error must be AFTER the non-ASCII line ` +
        `(got 0-based startLine ${first.startLine})`,
    );

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    const diags = await waitForDiagnostics(
      doc.uri,
      (d) => d.some((x) => x.code === "FSM-E0100"),
    );
    const got = diags.find((d) => d.code === "FSM-E0100");
    assert.ok(got, "the FSM-E0100 diagnostic must be present");
    assertRangeMatchesOracle(got, first, "non_ascii_doc.fsm (bundled)");
  });
});
