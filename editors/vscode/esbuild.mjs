// esbuild bundle for the FSM Studio VS Code extension (Doc 28 §2.2:
// "Bundler: esbuild — one fast dev-dep, no webpack tree").
//
// TWO bundles:
//  1. The EXTENSION (Node/CJS, loaded by the VS Code Extension Host).
//     `vscode` is provided by the host at runtime and MUST be external (it
//     is not an npm package). The language client + its deps are bundled so
//     the shipped extension has no node_modules runtime dependency.
//  2. The DIAGRAM WEBVIEW (V4 — browser/IIFE). The Webview runs in a
//     browser context (not Node); it loads ONE nonce'd `<script>` (the
//     strict CSP — Doc 27 §8-V4). `elkjs` (the sanctioned new runtime dep
//     for ELK Layered layout, Doc 27 §6.2 / Doc 28 §3-V4) is bundled INTO
//     this script so the Webview needs no node_modules and no remote
//     resource (CSP `default-src 'none'`). Output: `dist/webview/
//     diagramWebview.js` — the exact path `diagram/diagramPanel.ts`
//     references via `localResourceRoots` + `asWebviewUri`.

import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");

const extensionOptions = {
  entryPoints: ["src/extension.ts"],
  bundle: true,
  outfile: "dist/extension.js",
  external: ["vscode"],
  format: "cjs",
  platform: "node",
  target: "node18",
  sourcemap: true,
  // Production-grade but readable; the VSIX size matters (Doc 27 K-4) but
  // un-minified is easier to debug a wire issue with.
  minify: false,
  logLevel: "info",
};

// The Webview script — a browser IIFE with elkjs bundled in. `platform:
// "browser"` so esbuild does not inject Node shims; `format: "iife"` so it
// runs as a single <script> with no module loader (the CSP forbids
// dynamic import / eval).
const webviewOptions = {
  entryPoints: ["src/diagram/webview/diagramWebview.ts"],
  bundle: true,
  outfile: "dist/webview/diagramWebview.js",
  format: "iife",
  platform: "browser",
  target: "es2020",
  sourcemap: true,
  minify: false,
  logLevel: "info",
};

if (watch) {
  const ctxExt = await esbuild.context(extensionOptions);
  const ctxWv = await esbuild.context(webviewOptions);
  await ctxExt.watch();
  await ctxWv.watch();
  console.log("esbuild: watching extension + webview…");
} else {
  await esbuild.build(extensionOptions);
  console.log("esbuild: bundled dist/extension.js");
  await esbuild.build(webviewOptions);
  console.log("esbuild: bundled dist/webview/diagramWebview.js");
}
