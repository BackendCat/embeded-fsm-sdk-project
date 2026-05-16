// esbuild bundle for the FSM Studio VS Code extension (Doc 28 §2.2:
// "Bundler: esbuild — one fast dev-dep, no webpack tree").
//
// The extension is a CommonJS Node module loaded by the VS Code Extension
// Host; `vscode` is provided by the host at runtime and MUST be external
// (it is not an npm package). The language client + its deps are bundled
// so the shipped extension has no node_modules runtime dependency.

import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");

const options = {
  entryPoints: ["src/extension.ts"],
  bundle: true,
  outfile: "dist/extension.js",
  external: ["vscode"],
  format: "cjs",
  platform: "node",
  target: "node18",
  sourcemap: true,
  // Production-grade but readable; the VSIX size matters (Doc 27 K-4) but
  // V1 is host-only and un-minified is easier to debug a wire issue with.
  minify: false,
  logLevel: "info",
};

if (watch) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
  console.log("esbuild: watching…");
} else {
  await esbuild.build(options);
  console.log("esbuild: bundled dist/extension.js");
}
