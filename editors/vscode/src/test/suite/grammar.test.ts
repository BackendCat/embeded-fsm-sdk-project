// V2 behavioural-acceptance gate — Doc 28 §3 (the V2 row) / Doc 27 §8-V2
// (NOT symbol-presence, the §5.4 analogue). The TextMate-grammar +
// language-configuration + snippets layer is a static-asset contribution;
// its acceptance is that the *contributed* grammar ACTUALLY TOKENIZES a
// real `.fsm` into the scope names the Doc 21 §2/§3 grammar intends —
// asserted inside a REAL headless VS Code Extension Host through the
// EXACT tokenization engine VS Code itself uses.
//
// Why this engine, loaded this way:
//   VS Code colours buffers with `vscode-textmate` + `vscode-oniguruma`;
//   it exposes no stable public API to read a buffer's TextMate scopes
//   (`vscode.executeDocumentHighlights` is explicitly NOT a scope
//   assertion per the brief, and the old internal `_executeScopeProvider`
//   command was removed in modern VS Code — verified absent in the test
//   build). The robust, version-stable, ZERO-extra-dependency path is to
//   drive the very same engine VS Code ships
//   (`${vscode.env.appRoot}/node_modules/vscode-{textmate,oniguruma}`)
//   over the grammar the extension actually CONTRIBUTES (read back from
//   the installed extension's `contributes.grammars`, NOT a hand-picked
//   file) — a real tokenization assertion, not "the file exists / is
//   contributed" (the P0-1 symbol-presence trap at the grammar layer).
//   This also proves Doc 21 §6 "TextMate colours with the server OFF":
//   the tokenization here runs with NO language server at all.
//
// Asserted (Doc 28 §3 V2 gate — "keywords, identifiers, comments (incl.
// the `///` doc-comment), strings/numbers, and at least one nested
// construct"):
//   - keyword scopes (declaration / operator / pseudo-state)
//   - identifier scopes (machine/state/event/extern/context names)
//   - comments: the `///` doc-comment scope is DISTINCT from the `//`
//     line-comment scope and from the `/* */` block-comment scope
//   - string + numeric literal scopes
//   - a NESTED construct: a guard `[...]` nested inside an `on … ->`
//     transition, and an `entry: { … }` action block, resolve their
//     inner scopes under the enclosing scope
//   - the contributed `language-configuration.json` drives real host
//     comment-toggle behaviour (the lang-config half of the V2
//     deliverable — observable host behaviour, not a manifest read)
//   - snippet bodies match the Doc 22 §9 text (the snippets half)
//
// "The .tmLanguage.json file exists / package.json has a grammars entry"
// is explicitly NOT acceptance here (rejected per the brief / P0-1).
//
// V1-SPINE NOTE (carried per the §11.3 post-V1 audit §3 / M-1): V2 is
// STATIC ASSETS ONLY. This suite adds ZERO runtime code and does NOT
// touch any language-client `transport`/`Executable`/`NodeModule` /
// positionEncoding config (§3.A constraint). It sets `fsmLang.compilerPath`
// to the real server ONLY so that the unavoidable `onLanguage:fsm-lang`
// activation the lang-config sub-test triggers resolves fast+clean
// (Rule 1, exactly as the V1 suite does) instead of churning on a failed
// bundled-binary resolution and contending with the V1 suite's
// server-start window in the shared Extension Host.

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveRealBinaries } from "./binaries";

// `vscode-textmate` / `vscode-oniguruma` are not extension deps (and the
// V2 scope forbids adding deps); they are loaded at runtime from the
// running VS Code's own `node_modules` — the exact engine VS Code uses to
// colour buffers, so a scope this asserts is a scope VS Code would apply.
/* eslint-disable @typescript-eslint/no-var-requires */
/* eslint-disable @typescript-eslint/no-explicit-any */

const EXT_ID = "fsmstudio.fsm-lang";

interface Token {
  readonly text: string;
  readonly scopes: string[];
}

type TokenizedLine = Token[];

interface Grammar {
  tokenizeLine(
    line: string,
    ruleStack: any,
  ): {
    tokens: { startIndex: number; endIndex: number; scopes: string[] }[];
    ruleStack: any;
  };
}

/**
 * Build a `vscode-textmate` Registry wired to `vscode-oniguruma`, both
 * loaded from the running VS Code install, and resolve the grammar by the
 * scopeName the extension CONTRIBUTES for `.fsm`.
 */
async function loadContributedGrammar(): Promise<{
  grammar: Grammar;
  INITIAL: any;
}> {
  const ext = vscode.extensions.getExtension(EXT_ID);
  assert.ok(ext, `extension ${EXT_ID} must be present in the Extension Host`);

  const contributes: any = ext.packageJSON.contributes;
  assert.ok(
    contributes && Array.isArray(contributes.grammars),
    "package.json contributes.grammars must be an array",
  );
  const langEntry = (contributes.languages as any[]).find((l) => l.id === "fsm-lang");
  assert.ok(langEntry, "contributes.languages must declare id 'fsm-lang'");
  assert.strictEqual(
    langEntry.configuration,
    "./language-configuration.json",
    "contributes.languages.fsm-lang must reference the language-configuration",
  );
  const grammarEntry = (contributes.grammars as any[]).find((g) => g.language === "fsm-lang");
  assert.ok(grammarEntry, "contributes.grammars must declare a grammar for 'fsm-lang'");
  assert.strictEqual(
    grammarEntry.scopeName,
    "source.fsm",
    "the contributed scopeName must be 'source.fsm' (Doc 21 §1/§3)",
  );

  const grammarPath = path.join(ext.extensionPath, grammarEntry.path);
  assert.ok(fs.existsSync(grammarPath), `contributed grammar path must resolve: ${grammarPath}`);

  const appRoot = vscode.env.appRoot;
  const onigPath = path.join(appRoot, "node_modules", "vscode-oniguruma");
  const tmPath = path.join(appRoot, "node_modules", "vscode-textmate");
  const oniguruma = require(onigPath);
  const tm = require(tmPath);

  const wasmBin = fs.readFileSync(path.join(onigPath, "release", "onig.wasm")).buffer;
  await oniguruma.loadWASM(wasmBin);

  const onigLib = Promise.resolve({
    createOnigScanner: (patterns: string[]) => oniguruma.createOnigScanner(patterns),
    createOnigString: (s: string) => oniguruma.createOnigString(s),
  });

  const registry = new tm.Registry({
    onigLib,
    loadGrammar: async (scopeName: string) => {
      if (scopeName !== grammarEntry.scopeName) {
        return null;
      }
      const raw = fs.readFileSync(grammarPath, "utf8");
      // `parseRawGrammar` chooses JSON vs PLIST by the path extension;
      // the contributed path ends in `.tmLanguage.json`, so JSON.
      return tm.parseRawGrammar(raw, grammarPath);
    },
  });

  const grammar = await registry.loadGrammar(grammarEntry.scopeName);
  assert.ok(grammar, `the contributed grammar (${grammarEntry.scopeName}) must parse and load`);
  return { grammar, INITIAL: tm.INITIAL };
}

/** Tokenize a full source string into per-line (text, scopes) spans. */
function tokenizeSource(grammar: Grammar, INITIAL: any, source: string): TokenizedLine[] {
  const lines = source.split(/\r\n|\r|\n/);
  let ruleStack = INITIAL;
  const out: TokenizedLine[] = [];
  for (const line of lines) {
    const r = grammar.tokenizeLine(line, ruleStack);
    out.push(
      r.tokens.map((t) => ({
        text: line.substring(t.startIndex, t.endIndex),
        scopes: t.scopes,
      })),
    );
    ruleStack = r.ruleStack;
  }
  return out;
}

/** First token whose text (trimmed) === `text`. */
function findToken(lines: TokenizedLine[], text: string): Token | undefined {
  for (const ln of lines) {
    for (const tok of ln) {
      if (tok.text.trim() === text) {
        return tok;
      }
    }
  }
  return undefined;
}

/** All tokens whose text (trimmed) === `text`. */
function findAllTokens(lines: TokenizedLine[], text: string): Token[] {
  const acc: Token[] = [];
  for (const ln of lines) {
    for (const tok of ln) {
      if (tok.text.trim() === text) {
        acc.push(tok);
      }
    }
  }
  return acc;
}

/** Assert some token's scope list contains `scope`, else fail with detail. */
function assertScope(tok: Token | undefined, scope: string, label: string): void {
  assert.ok(tok, `${label}: token not found in tokenized output`);
  assert.ok(
    tok.scopes.includes(scope),
    `${label}: expected scope '${scope}' on token ${JSON.stringify(
      tok.text,
    )}, got ${JSON.stringify(tok.scopes)}`,
  );
}

const FIXTURE_DIR = path.resolve(__dirname, "../fixtures");

suite("V2 — TextMate grammar tokenization (real Extension Host)", () => {
  let grammar: Grammar;
  let INITIAL: any;
  let lines: TokenizedLine[];

  suiteSetup(async function () {
    this.timeout(60_000);
    const loaded = await loadContributedGrammar();
    grammar = loaded.grammar;
    INITIAL = loaded.INITIAL;
    const corpus = fs.readFileSync(path.join(FIXTURE_DIR, "grammar_corpus.fsm"), "utf8");
    lines = tokenizeSource(grammar, INITIAL, corpus);
    // Sanity: every span carries the root scope (proves the grammar
    // actually drove tokenization, not an empty/failed parse).
    const everySpanHasRoot = lines.every((ln) => ln.every((t) => t.scopes.includes("source.fsm")));
    assert.ok(everySpanHasRoot, "every tokenized span must carry the 'source.fsm' root scope");
  });

  test("(keywords) declaration + operator + pseudo-state keyword scopes", () => {
    assertScope(findToken(lines, "machine"), "keyword.declaration.machine.fsm", "machine keyword");
    assertScope(findToken(lines, "state"), "keyword.declaration.state.fsm", "state keyword");
    assertScope(findToken(lines, "extern"), "keyword.declaration.extern.fsm", "extern keyword");
    assertScope(findToken(lines, "pure"), "keyword.modifier.pure.fsm", "pure modifier");
    assertScope(findToken(lines, "events"), "keyword.declaration.events.fsm", "events keyword");
    assertScope(
      findToken(lines, "on"),
      "keyword.operator.fsm",
      "on operator keyword (transition begin)",
    );
    assertScope(findToken(lines, "after"), "keyword.operator.fsm", "after timer keyword");
    assertScope(
      findToken(lines, "initial"),
      "keyword.type.pseudo.fsm",
      "initial pseudo-state keyword",
    );
    assertScope(findToken(lines, "entry"), "keyword.control.fsm", "entry control keyword");
  });

  test("(identifiers) declaration name scopes", () => {
    assertScope(findToken(lines, "Sample"), "entity.name.type.machine.fsm", "machine name");
    const idleDecl = findAllTokens(lines, "Idle").find((t) =>
      t.scopes.includes("entity.name.type.state.fsm"),
    );
    assertScope(idleDecl, "entity.name.type.state.fsm", "state name (Idle declaration)");
    assertScope(
      findToken(lines, "GO"),
      "entity.name.type.event.fsm",
      "event name (GO inside the events block)",
    );
    assertScope(
      findToken(lines, "checkReady"),
      "entity.name.function.extern.fsm",
      "extern function name",
    );
    assertScope(findToken(lines, "count"), "variable.other.field.fsm", "context field name");
  });

  test("(comments) ///, //, /* */ are DISTINCT scopes", () => {
    const docTok = lines.flat().find((t) => t.text.includes("Doc comment line"));
    assert.ok(docTok, "the /// doc-comment line must tokenize");
    assert.ok(
      docTok.scopes.includes("comment.line.documentation.fsm"),
      `/// must be 'comment.line.documentation.fsm', got ${JSON.stringify(docTok.scopes)}`,
    );
    assert.ok(
      !docTok.scopes.includes("comment.line.double-slash.fsm"),
      "the /// doc-comment must NOT collapse to the // line-comment scope",
    );

    const lineTok = lines.flat().find((t) => t.text.includes("Plain line comment"));
    assert.ok(lineTok, "the // line-comment must tokenize");
    assert.ok(
      lineTok.scopes.includes("comment.line.double-slash.fsm"),
      `// must be 'comment.line.double-slash.fsm', got ${JSON.stringify(lineTok.scopes)}`,
    );

    const blockTok = lines.flat().find((t) => t.scopes.includes("comment.block.fsm"));
    assert.ok(blockTok, "the /* */ block comment must carry 'comment.block.fsm'");
  });

  test("(literals) string + numeric scopes", () => {
    // The timer integer `1000` (after the `#timer` rule) is the
    // integer-literal proof. (Note: a `context { … = 0; }` default value
    // is NOT scoped — a pre-existing Doc 21 §3 limitation: the
    // `context-block` rule omits `#literals`/`#operators`; flagged as an
    // observation in the V2 report, out of the V2 gate's required
    // coverage and deliberately NOT "fixed" to keep the verbatim surface
    // minimal to the load-bearing swallow defect only.)
    const timerInt = findAllTokens(lines, "1000").find((t) =>
      t.scopes.includes("constant.numeric.integer.fsm"),
    );
    assertScope(timerInt, "constant.numeric.integer.fsm", "timer integer literal (1000)");
    // A focused integer + float snippet through the same contributed
    // grammar (isolated from the context-block limitation above).
    const numLines = tokenizeSource(grammar, INITIAL, "state S { after 250 ms -> T; }\n");
    const isoInt = numLines.flat().find((t) => t.scopes.includes("constant.numeric.integer.fsm"));
    assertScope(isoInt, "constant.numeric.integer.fsm", "isolated integer literal (250)");

    // A quoted string: a focused snippet (the corpus is keyword-dense)
    // still tokenized through the contributed grammar.
    const strLines = tokenizeSource(grammar, INITIAL, 'machine M { @id("stable-name") }\n');
    const strTok = strLines.flat().find((t) => t.scopes.includes("string.quoted.double.fsm"));
    assert.ok(
      strTok,
      `a quoted string must carry 'string.quoted.double.fsm', got ${JSON.stringify(
        strLines.flat().map((t) => [t.text, t.scopes]),
      )}`,
    );
  });

  test("(nested) guard [...] + entry action { … } resolve nested scopes", () => {
    // The transition rule is a begin/end block; its inner guard must
    // apply NESTED scopes — a real nested-construct assertion, not a
    // flat keyword match.
    const transitionSpans = lines.flat().filter((t) => t.scopes.includes("meta.transition.fsm"));
    assert.ok(
      transitionSpans.length > 0,
      "the `on … ->` transition must open the meta.transition.fsm scope",
    );

    const guardOpen = lines
      .flat()
      .find(
        (t) =>
          t.text === "[" &&
          t.scopes.includes("meta.transition.fsm") &&
          t.scopes.includes("punctuation.definition.guard.begin.fsm"),
      );
    assert.ok(
      guardOpen,
      "the guard '[' must be punctuation.definition.guard.begin.fsm AND " +
        "be nested under meta.transition.fsm",
    );
    const guardBody = lines
      .flat()
      .find((t) => t.scopes.includes("meta.guard.fsm") && t.scopes.includes("meta.transition.fsm"));
    assert.ok(guardBody, "the guard body must carry meta.guard.fsm nested in meta.transition.fsm");
    const arrow = lines
      .flat()
      .find(
        (t) =>
          t.text === "->" &&
          t.scopes.includes("keyword.operator.arrow.fsm") &&
          t.scopes.includes("meta.transition.fsm"),
      );
    assert.ok(
      arrow,
      "the transition arrow '->' must be keyword.operator.arrow.fsm " +
        "nested in meta.transition.fsm",
    );

    // The `entry: { … }` action block: its inner call must resolve a
    // nested action scope (the action-block half of the nested gate).
    const actionInner = lines
      .flat()
      .find((t) => t.text.trim() === "resetCount" && t.scopes.includes("meta.block.action.fsm"));
    assert.ok(
      actionInner,
      "the `entry: { resetCount(); }` body must carry the nested " + "meta.block.action.fsm scope",
    );
  });

  test("(language-configuration) the host applies the contributed lang-config", async function () {
    this.timeout(40_000);
    // The lang-config half of the V2 deliverable: assert the host
    // actually applied `comments.lineComment = //` by exercising the
    // built-in comment-toggle on a real `.fsm` document — observable
    // host behaviour driven by language-configuration.json, not a
    // manifest read.
    //
    // Opening a `.fsm` doc triggers `onLanguage:fsm-lang` activation;
    // point the resolver at the real server first (Rule 1, as the V1
    // suite does) so that activation resolves fast+clean and does NOT
    // contend with the V1 suite's server-start window. V2 adds no
    // runtime code; this only configures the existing V1 resolver.
    const bins = resolveRealBinaries();
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update("compilerPath", bins.server, vscode.ConfigurationTarget.Global);

    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v2-langcfg-"));
    const filePath = path.join(tmpDir, "langcfg.fsm");
    fs.writeFileSync(filePath, "state Idle\n", "utf8");
    const doc = await vscode.workspace.openTextDocument(filePath);
    const editor = await vscode.window.showTextDocument(doc);
    editor.selection = new vscode.Selection(0, 0, 0, 0);
    await vscode.commands.executeCommand("editor.action.commentLine");
    await new Promise((r) => setTimeout(r, 250));
    const firstLine = doc.lineAt(0).text;
    assert.ok(
      firstLine.startsWith("// "),
      `comment-toggle must use the contributed lineComment '//' ` +
        `(language-configuration.json); got ${JSON.stringify(firstLine)}`,
    );
    await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  test("(snippets) contributed snippet bodies match Doc 22 §9 verbatim", () => {
    const ext = vscode.extensions.getExtension(EXT_ID);
    assert.ok(ext, "extension must be present");
    const snippetsContrib: any[] = ext.packageJSON.contributes.snippets;
    assert.ok(
      Array.isArray(snippetsContrib) && snippetsContrib.length === 1,
      "contributes.snippets must be a single-entry array (Doc 22 §3)",
    );
    const snippetsPath = path.join(ext.extensionPath, snippetsContrib[0].path);
    const snippets = JSON.parse(fs.readFileSync(snippetsPath, "utf8"));

    assert.strictEqual(snippets["After timer"].prefix, "after");
    assert.deepStrictEqual(snippets["After timer"].body, ["after ${1:1000}ms -> ${2:Timeout};"]);
    assert.strictEqual(snippets["Transition with guard"].prefix, "on");
    assert.deepStrictEqual(snippets["Transition with guard"].body, [
      "on ${1:EVENT} [${2:guard()}] -> ${3:Target};",
    ]);
    assert.strictEqual(snippets["Machine"].prefix, "machine");
    assert.strictEqual(snippets["Machine"].body[0], "machine ${1:Name} {");
    assert.deepStrictEqual(snippets["Every timer"].body, ["every ${1:100}ms: { ${2:poll();} }"]);
    assert.deepStrictEqual(snippets["Pure extern guard"].body, [
      "pure extern ${1:checkCondition}(${2:threshold: u32}) : bool",
    ]);
    const names = Object.keys(snippets).sort();
    assert.deepStrictEqual(names, [
      "After timer",
      "Composite State",
      "Every timer",
      "Extern action",
      "Guard with else",
      "Machine",
      "Parallel State",
      "Pure extern guard",
      "State",
      "Transition with guard",
    ]);
  });
});
