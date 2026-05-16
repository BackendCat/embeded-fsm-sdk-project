# AUDIT — §11.3 Post-V1 Phase-Boundary Audit (v1.3-V1: VS Code extension MVP spine)

- **Scope:** the Doc-28-mandated `V1 → audit → V2/V3` gate (SUBAGENT §11.3 /
  Doc 28 §3-V1 "A phase-boundary audit runs after V1 before any V2+
  dispatch").
- **Subject:** `editors/vscode/` at `main` HEAD **`6707d1d`** ("v1.3-V1: VS
  Code extension MVP spine — scaffold + LSP client + live diagnostics").
- **Method:** READ-ONLY judgment audit. ZERO `cargo`/`npm`/build. Derived
  by reading shipped source (`editors/vscode/src/*`, the locked
  `vscode-languageclient@9.0.1` in the main worktree's `node_modules`,
  `crates/fsm-lsp/src/main.rs`) + `git -C` + `grep`, cross-read against
  Docs 22/26/27/28.
- **Established context (NOT re-litigated — the §11.1 independent
  re-verification already passed):** V1 is purely additive
  (`editors/vscode/` + the W0 audit doc + `.gitignore`); Rust workspace
  byte-identical to W0-clean `ceb8efd`; §11.1 returned EXTENSION-HOST GATE
  GREEN with the (a)/(d) oracle independently recomputed; toolchain
  correctly pinned 1.75.0. This audit **independently re-derives** Findings
  1+2 from source (verify-the-record) and judges spine-soundness for
  V2–V6 — it does not re-run the §11.1 gate.

---

## 0. Verdict (TL;DR)

**V2-READINESS: PROCEED-WITH-NOTES.**

The phase boundary is **clean enough to dispatch V2/V3** per Doc 28's
cadence. V1 is a correctly-disciplined, depth-first, purely-additive
integration keystone:

- **The Rust workspace is byte-untouched.** `git diff ceb8efd 6707d1d --
  crates/ Cargo.toml Cargo.lock` is **empty**; the full V1 delta is
  `editors/vscode/` (5638 LOC) + the W0 audit doc + `.gitignore`. The
  cargo quad is structurally incapable of regressing from this commit.
- **The two doc-overturning findings are CONFIRMED by independent
  source re-derivation** (this audit re-read the exact line numbers in
  `main.rs` and `client.js`/`main.js` itself — §2). Both the shipped
  code's *behaviour* and the *implementer's disclosure* are correct;
  it is **Doc 27/28 prose** that is wrong-as-written. The shipped
  extension does the right thing for the wrong-documented reason, and
  did so deliberately and with a load-bearing code comment.
- **The V1-implemented-but-unexercised paths the implementer disclosed
  (Doc 22 §2.2 binary-resolution Rule 1/2/3; Doc 22 §13.2
  exponential-backoff `errorHandler`) are correct by reading**, not
  merely "compile" (§1.2, §1.3). Spot-checked line-by-line against the
  Doc 22 prose and the shipped behaviour.
- **The omit-`transport` fix is structurally robust, NOT a fragile
  coincidence of v9 internals** — proven against the actual
  `vscode-languageclient@9.0.1` `Executable`-vs-`NodeModule` dispatch
  (§1.1, the decisive spine spot-check). It rides a *deliberate,
  by-construction* library affordance, not an accident.

The **NOTES** (none are V2 ship-blockers; classified by when each must
be folded):

- **N-1 (CANONICAL RECONCILIATION — MANDATORY-BEFORE-V2 as a *briefing*
  input, fold-text deferrable to v1.3-closeout):** Doc 27 §2.2's
  "`TransportKind.stdio`" and Doc 27 §2.3 / §7-risk-5 / §8-V1(c) +
  Doc 28 §2.2 / R-4 / §3-V1(c) / §5-V1(c)'s "UTF-8 reachable / fast-path
  used" are **both contradicted by shipped-library reality**. The exact
  correction-of-record text is **§3 of this doc** — this doc is the
  durable correction meanwhile (the W0 §11.3 / N-1 pattern). **What is
  mandatory before V2 is not the doc edit but that the V2/V3 briefs cite
  THIS audit's §3, not the stale Doc 27/28 prose** (the Doc 28 §1.1
  "read it through the reconciliation row" discipline, now extended to a
  third finding-pair). The doc edits themselves batch into the
  v1.3-closeout Doc-00 pass alongside the existing R-14/R-15 re-pins.
- **N-2 (DRIFT — new, deferrable-to-closeout):** Doc 28 §3-V1 / §5-V1
  acceptance clause (c) **textually mandates the FALSE assertion**
  (`positionEncoding` == **UTF-8**). The shipped acceptance test
  `extension.test.ts` test (c) correctly asserts **`"utf-16"`** instead
  (extension.test.ts:205-213) — i.e. **the shipped gate already
  diverges, correctly, from its own brief**. This is *good* (the
  implementer caught it), but it means Doc 28 §3/§5 clause (c) is now a
  stale instruction that a future re-derivation or a mechanical
  "does the test match the brief" check would mis-flag. Fold the §3
  correction text into Doc 28 (c) at closeout.
- **N-3 (citation re-pin, non-load-bearing, closeout):** Doc 27 §2.2's
  `main.rs:39-48` for the `--port` branch is **off by start-line**: the
  shipped `--port` arm is `main.rs:39-48`-ish but the *unknown-arg*
  exit-2 arm Finding 1 actually rides is **`main.rs:49-52`** and the
  `None`/stdio arm is **`main.rs:53-65`**. Doc 27 §2.2 conflates the two
  under the `--port` citation; the extension's own code comment
  (extension.ts:140-143) cites these correctly. Doc-hygiene only.

This audit deliberately shows the spine-soundness spot-checks (§1) and
the independent source re-derivation of Findings 1+2 (§2) because "an
audit that finds nothing on the integration keystone that already
surfaced two doc-overturning findings is itself suspect" — the work
*was* checked at the byte/line level, the two findings *are*
independently re-confirmed (not trusted from context), and the surviving
drift (N-1/N-2/N-3) is named precisely with the exact fold-text rather
than papered over.

---

## 1. Lens 1 — V1 spine soundness for V2–V6

V1's job per Doc 27 §8 / Doc 28 §3 is to prove the *hard depth-first
bet* — "does the client wiring + the position-encoding seam actually
work end-to-end against the shipped server" — so that V2 (TextMate
grammar/snippets), V3 (commands), V4 (diagram Webview), V5 (tree views),
V6 (multi-platform bundling) build on a proven spine without rework. The
question is not "does it compile" (the §11.1 gate covered green); it is
**"is each load-bearing seam correct *by reading*, and is anything a
high-fan-out V2–V6 would expose latent?"**

### 1.1 The decisive spot-check — is the omit-`transport` fix robust, or a fragile v9 coincidence?

This is the single highest-leverage spine question (a V3 commands wave,
or any future client-options touch, would be the high-fan-out exposer).
The implementer's disclosure (extension.ts:137-152) claims: setting
`transport: TransportKind.stdio` makes v9 push `--stdio` onto argv → the
strict server exits 2 mid-`initialize`; **omitting** `transport` makes
v9 spawn over stdio *without* the push. **Independently verified against
the locked `vscode-languageclient@9.0.1` source** (installed == locked;
`package.json` `^9.0.1`, `package-lock.json` resolved `9.0.1` +
sha512 integrity — byte-cross-checked):

1. **Type discrimination is unambiguous** (`lib/node/main.js:51-63`):
   - `Executable.is(v)` ≙ `Is.string(v.command)` (`main.js:53-56`)
   - `NodeModule.is(v)` ≙ `Is.string(v.module)` (`main.js:60-63`)
   V1's `ServerOptions.run` is `{ command: resolved.command, args: [] }`
   (extension.ts:153-160) — it has `.command`, **no `.module`**.
2. **The dangerous coercion is in the `NodeModule` branch and is never
   reached.** `main.js:264` `if (NodeModule.is(json) && json.module)`
   is **false** for V1, so `main.js:266`
   `let transport = node.transport || TransportKind.stdio;` — the
   `|| stdio` that *would* coerce `undefined → stdio` and push
   `--stdio` (main.js:288 / :350) — **is structurally unreachable for an
   `Executable`**.
3. **The `Executable` branch deliberately does NOT coerce.**
   `main.js:405` `else if (Executable.is(json) && json.command)` →
   `main.js:409` `const transport = json.transport;` (**verbatim, no
   `|| stdio`**). V1 omits `transport`, so `transport === undefined`.
4. **The `--stdio` push is strictly gated on `=== TransportKind.stdio`.**
   `main.js:410` `if (transport === TransportKind.stdio) {
   args.push('--stdio'); }`. `TransportKind.stdio === 0` (`main.js:38`);
   `undefined !== 0` → **`--stdio` is NOT pushed**. ✓
5. **`undefined` transport is a first-class, explicitly-handled spawn
   path — not a fall-through.** `main.js:425`
   `if (transport === undefined || transport === TransportKind.stdio) {
   const serverProcess = cp.spawn(command.command, args, options); …
   StreamMessageReader(stdout)/StreamMessageWriter(stdin) }`
   (main.js:425-433). The library authors *intentionally* treat
   `transport === undefined` for an `Executable` as "spawn raw, inject
   no transport flag, wire stdio streams" — equivalent to stdio but
   **without the argv mutation**.

**Verdict: ROBUST, by construction.** The fix is not "a coincidence of
v9 internals" — it rides a *deliberate asymmetry* the library authors
designed: the `NodeModule` path coerces `undefined → stdio` (because a
forked node module *needs* a `--stdio`/`--node-ipc` discriminator),
while the `Executable` path deliberately leaves `undefined` as a
distinct "spawn the command exactly as given" mode. This invokes
`fsm-lang-server` with `args: []` — **exactly its stdio contract**
(`main.rs:53-65` `None => run_stdio()`). A V3 wave adding commands, or
any future `clientOptions` refinement, does not endanger this as long as
it does not (a) set `transport` on the `Executable`, or (b) switch to a
`NodeModule` server shape. **Spine-latent risk for V2–V6: NONE,
provided the §3 / N-1 reconciliation is in the briefs so a future wave
does not "fix" the missing `transport` back in.** This is the load-bearing
reason N-1 is mandatory-as-a-briefing-input.

### 1.2 Binary-resolution Rule 1/2/3 (Doc 22 §2.2 / §12) — correct by reading

The V1 launch path in the §11.1 gate is Rule 1 (`fsmLang.compilerPath`
set by the test — extension.test.ts:142-148); Rules 2/3 are
**implemented but unexercised by the gate** — exactly the kind of path
the implementer flagged for audit scrutiny. Read line-by-line
(`serverBinary.ts`):

- **Rule 1 (serverBinary.ts:56-60):** `(config.get<string>("compilerPath")
  ?? "").trim()`; non-empty → `{ command: compilerPath, source:
  "compilerPath" }` **verbatim**. Matches Doc 22 §12 "user-specified path
  takes precedence" + Doc 27 §2.2 Rule 1 + the `scope:
  machine-overridable` in `package.json:54`. `.trim()` correctly treats
  a whitespace-only setting as "unset" (defensible — a stray space must
  not become a bogus argv[0]). ✓
- **Rule 2 (serverBinary.ts:62-71):** `path.join(extensionPath, "bin",
  hostTriple(), serverExeName())`; `hostTriple()` = `${os.platform()}-${
  os.arch()}` (serverBinary.ts:28-30) — **exactly the Doc 22 §12
  `${platform}-${arch}` scheme** (`os.platform()` ∈ {linux,darwin,win32},
  `os.arch()` ∈ {x64,arm64}); `serverExeName()` appends `.exe` **only on
  win32** (serverBinary.ts:33-35) — matches Doc 22 §12 `fsm${ext}`,
  `ext = win32 ? '.exe' : ''`. `fs.existsSync` gate → only returns
  `bundled` if the file is actually present. **No silent downgrade to a
  different triple.** ✓ (V1 ships host-only, Doc 27 risk-2 — Rule 2 is
  the V6 multi-platform consumption point and is already correct for it.)
- **Rule 3 (serverBinary.ts:73-74 + extension.ts:121-132):** returns
  `undefined`; the caller emits the **verbatim Doc 22 §12 string** via
  `noBundledBinaryMessage()` (serverBinary.ts:20-25) — byte-compared to
  Doc 22:617 `"FSM Studio: No bundled binary for {platform}-{arch}.
  Please install fsm manually and set fsmLang.compilerPath."` →
  **exact** (the template substitutes `${platformArch}` =
  `hostTriple()`), sets status `stopped`, logs to the output channel,
  and **returns `buildApi("none")` without starting the client**. This
  is the cardinal-sin bar honoured at the client boundary: **no silent
  fallback to PATH or a guessed binary** — verified there is no
  `which`/PATH lookup, no default-path guess anywhere in
  `serverBinary.ts`. ✓

**Doc-22-§12-prose drift note (pre-existing, already reconciled in Doc
27 §10 ⚠️ row, restated for completeness):** Doc 22 §12's code sample
bundles `fsm${ext}` (the CLI name); the binary the *client* launches is
`fsm-lang-server` (serverBinary.ts:33-34). This is the known Doc 22 §12
under-description (a genuine two-binary need), already flagged ⚠️ in
Doc 27 §10 and Doc 28 R-3 — **not a new finding**; the shipped resolver
correctly uses `fsm-lang-server`.

### 1.3 Doc 22 §13.2 exponential-backoff `errorHandler` — correct by reading

Implemented but **only exercised by the gate via the lifecycle, not via
a forced crash** — another implementer-flagged unexercised path. Read
`crashRecovery.ts` against the Doc 22 §13.2 table (Doc 22:663-698):

| Doc 22 §13.2 param | Doc 22 value | `crashRecovery.ts` | Match |
|---|---|---|---|
| `MAX_RESTARTS` | 3 | `MAX_RESTARTS = 3` (:19) | ✓ |
| `BASE_DELAY_MS` | 3000 | `BASE_DELAY_MS = 3000` (:20) | ✓ |
| Backoff multiplier | 3× | `BACKOFF_MULTIPLIER = 3` (:21) | ✓ |
| Attempt 1/2/3 delay | 3 / 9 / 27 s | `backoffDelayMs(n)=3000·3^(n-1)` (:25-27) → 3000/9000/27000 | ✓ |
| Success-reset | 60 s | `RESET_DELAY_MS = 60_000` (:22), armed in `noteServerReady` (:49-54) | ✓ |
| After 3 failures | stop + manual button | `closed()`: `crashCount<3` → `Restart` else `DoNotRestart` (:81-97) + the exhaustion notification (extension.ts:170-182) with `[Restart Language Server][Show Log]` | ✓ |
| Manual restart resets counter | yes | `resetCrashCount()` (:57-60), called by `restartServer()` (extension.ts:275) | ✓ |
| Crash counter reset on healthy uptime | yes (60 s) | `noteServerReady()` sets a 60 s timer → `crashCount=0`; **cleared the instant the process dies** (`closed()` first line, :80 `this.clearResetTimer()`) | ✓ — and the "clear timer on death" detail is *correct and subtle*: it prevents a crash within the 60 s window from being absolved by a stale reset timer. |
| `error()` (protocol error) | continue | `error()` → `ErrorAction.Continue` (:73-76) — matches Doc 22:646 sketch | ✓ |

The restart-crash message string (crashRecovery.ts:91-94) is
byte-faithful to the Doc 22:654 template
(`FSM Language Server crashed. Restarting in ${delayMs/1000}s...
(attempt ${n}/${MAX_RESTARTS})`). The exhaustion notification copy
(extension.ts:171) `"FSM Language Server has stopped after 3 restart
attempts."` matches Doc 22:687. **Correct by reading, not just
compiling.** One observation, not a defect: the status-bar tooltip on
"restarting" renders the generic `"…starting"` (statusBar.ts:31-35) and
not Doc 22:679's literal `"FSM Language Server restarting (attempt
N/3)..."`. The Doc 22 §10 *icon* (`$(sync~spin)`) and *text* (`FSM`)
**do** match; only the tooltip *string* is generic. This is cosmetic,
inside the unexercised recovery path, and Doc 27 §2.5/§10 already
establishes Doc 22 §10 as intent-shorthand the extension maps — flagging
it as a **deferrable cosmetic polish for V3/V5** (when the status bar
gains the `fsm.currentMachine` second item and a recovery-state
treatment is natural), **not** a V2 blocker.

### 1.4 Other spine seams (V2–V6 build surface) — read & sound

- **`initializationOptions` / `synchronize` (extension.ts:187-204):**
  carries **only** the four inlay keys (`readInlayHintSettings`,
  inlayConfig.ts:31-51) + `synchronize.configurationSection:
  "fsmLang"`. **No `capabilities`/`clientOptions` override** — the
  deliberate Doc 27 §2.3 / Doc 28 R-4 "do not strip the client default"
  constraint, correctly honoured (this is what keeps the
  positionEncoding negotiation at the library default — see §2.2/§3).
  The four keys map exactly to the shipped server's
  `InlayHintConfig::from_settings` four-key reality (R-5; defaults
  master/priorities/timers ON, state-types OFF — inlayConfig.ts:35-49
  matches). ✓ Sound base for V3 (the commands that give the *other*
  Doc 22 §8 keys meaning) — V1 correctly does **not** contribute the
  dead keys (the §2.5 "never silently no-op a user setting" rule).
- **Activation (`package.json:27-30`):** `onLanguage:fsm-lang` +
  `workspaceContains:**/*.fsm` — **byte-exact to Doc 22 §2** (Doc
  22:47-50); **no `*`** (the explicit Doc 22:57 prohibition honoured).
  Sound for V2–V6 (no wave needs `*`; V4 diagram/V5 trees activate via
  the same `.fsm` triggers). ✓
- **`engines.vscode: ^1.85.0` (package.json:17) + `@types/vscode:
  ^1.85.0` (:91) + `vscode-languageclient: ^9.0.1` (:102):** consistent
  with Doc 27 §2.1 (`engines.vscode ^1.85.0`, the LSP-3.17 / client-≥8
  floor) and Doc 28 §2.2 ("`vscode-languageclient ≥ 8`"). `^9.0.1` is
  ≥ 8 — satisfies the floor; locked at 9.0.1. **No drift.** (The
  finding that ≥8/≥9 *cannot* reach UTF-8 is the §2.2/§3 doc-prose
  correction, not a version-floor problem.) ✓
- **esbuild bundle (esbuild.mjs):** `external: ["vscode"]` (correct —
  host-provided), `format: cjs`, `platform: node`, client+deps bundled
  (no `node_modules` runtime dep in the VSIX). Sound for V6 VSIX
  packaging. The `target: "node18"` is conservative vs `engines.node
  >=20` (package.json:18) — harmless (node18 output runs on node20).
  Non-issue; noting for completeness. ✓
- **Test harness (`runTest.ts`/`index.ts`/`oracle.ts`/`binaries.ts`/
  `extension.test.ts`):** the §5.4-analogue is genuine — real headless
  Extension Host (`@vscode/test-electron`), real `fsm-lang-server`
  built **from the repo root** so the 1.75.0 pin applies
  (binaries.ts:48-71 — toolchain-trap-aware, comment cites the memory
  rule), oracle **recomputed** from `fsm check --json`
  (oracle.ts:56-90), never hand-typed, byte-compared
  (extension.test.ts:71-101). R-15 isolation is enforced *and
  hard-asserted*: fixtures staged in an OS temp dir + an explicit
  upward `fsm.toml` walk that **fails loud** if any is found
  (extension.test.ts:118-137). The ASCII-column invariant is asserted
  per fixture (oracle.ts:99-125) so a future fixture that violates the
  scalar→UTF-16 mapping fails loudly rather than mis-mapping silently.
  This is the correct, durable §5.4-analogue scaffold for V2–V6 to
  extend (V2 `vscode-tmgrammar-test`, V3 command-invocation assertions,
  V4 Webview model assertions) — **sound, no rework needed**. ✓

**Lens-1 verdict: the spine is sound for V2–V6 with no rework.** Every
load-bearing seam is correct *by reading*, the two unexercised paths the
implementer disclosed (Rule 2/3, the backoff handler) are correct
line-by-line, and the omit-`transport` fix is robust by construction.
The only latent foot-gun is **social, not structural**: a future wave
that "tidies up the missing `transport`" back to `TransportKind.stdio`
would silently reintroduce the exit-2 crash — which is exactly why N-1
(the §3 reconciliation must be in the V2/V3 briefs) is mandatory.

---

## 2. Lens 2 — Independent source re-derivation of Findings 1 + 2 (verify-the-record)

Per the brief, I re-confirmed both findings **from source myself**, not
from the supplied context (symmetric verify-the-record, the W0 §6
pattern).

### 2.1 Finding 1 — the strict server arg-parser vs the client `--stdio` push

**Re-read `crates/fsm-lsp/src/main.rs` (the shipped binary at
`6707d1d`; Rust workspace byte-identical to `ceb8efd`):**

- **`main.rs:49-52`** —
  `Some(other) => { eprintln!("error: unknown argument {other:?}; try
  --help"); ExitCode::from(2) }`. The arg matcher (`match
  args.next().as_deref()`, :22) handles `--version`/`-V` (:23),
  `--help`/`-h` (:28), `--port` (:39, exits 2), and **every other
  token — including `--stdio` — falls into the `Some(other)` arm and
  exits `2`**. Confirmed: the server has **no `--stdio` handler**; it is
  an unknown arg.
- **`main.rs:53-65`** — `None => { … rt.block_on(fsm_lsp::run_stdio());
  ExitCode::SUCCESS }`. Stdio mode is entered **iff argv has no
  positional token**, i.e. the binary is invoked with **no args**.

**Re-read `vscode-languageclient@9.0.1` `lib/node/main.js` (the
`Executable` branch V1 hits):** §1.1 above is the full re-derivation —
`main.js:409` reads `json.transport` with no coercion; `main.js:410`
pushes `--stdio` **only** if `transport === TransportKind.stdio (=0)`;
`main.js:425` treats `transport === undefined` as a first-class raw
spawn. **Confirmed independently.**

**Conclusion (re-confirmed):** had V1 set `transport:
TransportKind.stdio`, v9 would `args.push('--stdio')` (main.js:411) →
`fsm-lang-server --stdio` → `main.rs:49-52` exits 2 → client stream
destroyed mid-`initialize`. V1 **omits** `transport`
(extension.ts:153-156, `{ command, args: [] }`, no `transport` key) →
`main.js:425` raw `cp.spawn(command, [], options)` → `fsm-lang-server`
with no args → `main.rs:53-65` `run_stdio()`. **The shipped behaviour is
correct.** Doc 27 §2.2's "the `vscode-languageclient` stdio transport
(`TransportKind.stdio`)" is **wrong as a literal round-trip
instruction** — it states the *intent* (use stdio) but names the one
concrete form that breaks against the shipped server's strict parser.

### 2.2 Finding 2 — UTF-8 positionEncoding is unreachable through the official client

**Re-read `vscode-languageclient@9.0.1` `lib/common/client.js`
(installed == locked 9.0.1, sha512-cross-checked):**

- **`client.js:1370`** —
  `generalCapabilities.positionEncodings = ['utf-16'];`. Read the full
  surrounding `fillClientCapabilities`/`computeClientCapabilities`
  context (client.js:1355-1377): `generalCapabilities` is built from
  fixed literals (`staleRequestSupport`, `regularExpressions`,
  `markdown`, then `:1370` `positionEncodings = ['utf-16']`). There is
  **no read of any client option, no middleware hook, no parameter** —
  the array is an **unconditional hardcoded literal**. The only
  client-controlled adjustment in this block is `markdown.allowedTags`
  gated on `markdown.supportHtml` (:1371-1373) — *not* encoding.
  `positionEncodings` is **not overridable** by `initializationOptions`,
  `clientOptions`, or any documented surface.
- **`client.js:835-836`** — `if (result.capabilities.positionEncoding
  !== undefined && result.capabilities.positionEncoding !==
  vscode_languageserver_protocol_1.PositionEncodingKind.UTF16) { throw
  new Error(\`Unsupported position encoding (...) received from server
  ${this.name}\`); }`. Confirmed: the client **throws** if the server
  negotiates anything other than UTF-16 (or omits it).

**Conclusion (re-confirmed):** the official `vscode-languageclient@9`
(a) advertises **only** `general.positionEncodings = ['utf-16']` to the
server, and (b) **throws** on any non-UTF-16 server response. The shipped
server's negotiation (`server.rs:216-226` per Doc 28 R-4: `negotiated =
if offer.contains(UTF8) {Utf8} else {Utf16}`) therefore sees a
**UTF-16-only offer** → negotiates **UTF-16** (the documented `else`
fallback) → the client accepts it (no throw). **UTF-8 is unreachable
through the official client by any in-scope means.** Doc 26 §4.1 is
explicit that the UTF-16 path is **still correct** — it loses only the
performance fast-path, not correctness; and the substantive defect-class
(mis-encoded multibyte columns) is guarded **end-to-end by acceptance
(d)** which runs *under this exact UTF-16 negotiation*
(extension.test.ts:305-341, fixture `non_ascii_doc.fsm` — emoji+Cyrillic
on line 3, FSM-E0100 on line 9, Range byte-compared to the CLI oracle).
The shipped acceptance test **already encodes the correct expectation**:
test (c) asserts `negotiatedPositionEncoding === "utf-16"`
(extension.test.ts:205-213) — i.e. the implementer **correctly
diverged** from Doc 28 §3/§5 clause (c)'s literal "UTF-8" instruction.

**Both findings stand on independent re-derivation. The defect is in
Doc 27/28 prose; the shipped code is correct.**

---

## 3. Findings 1 + 2 — canonical reconciliation text (the orchestrator-owned correction-of-record)

This is the EXACT correction text. Per the W0 §11.3 / N-1 pattern, **this
audit doc is the durable correction-of-record now**; the orchestrator
folds this text into the docs at the **v1.3-closeout batched-doc pass**
(alongside the existing Doc 28 R-14/R-15 re-pins and the §1.1 Doc-00
row). V2/V3 briefs **must cite this §3** rather than the stale prose
(N-1, mandatory-before-V2 as a briefing input).

### 3.A — Doc 27 §2.2 (`TransportKind.stdio` → omit-`transport`)

> **Replace** (Doc 27 §2.2, the "Decision — stdio `ServerOptions`"
> paragraph, the sentence: *"The client launches the binary as a child
> process with the `vscode-languageclient` stdio transport
> (`TransportKind.stdio`)."*) **with:**
>
> "The client launches the binary as a child process over **stdio**, by
> constructing an `Executable` `ServerOptions` and **deliberately
> OMITTING the `transport` field** (`{ command, args: [] }`, no
> `transport`). This is **not** `TransportKind.stdio`: the shipped
> `fsm-lang-server` takes **no arguments** for stdio mode
> (`crates/fsm-lsp/src/main.rs:53-65`: `None => run_stdio()`) and its
> strict parser **exits 2 on ANY unknown argument, including `--stdio`**
> (`main.rs:49-52`, the project's fail-loud-on-unknown-arg bar). Setting
> `transport: TransportKind.stdio` makes `vscode-languageclient@9` push
> `--stdio` onto argv for an `Executable`
> (`lib/node/main.js:410-411`) → the server exits 2 and the client
> stream is destroyed mid-`initialize`. Omitting `transport` is the
> **only** form that round-trips: for an `Executable`, v9 reads
> `json.transport` **without** the `|| TransportKind.stdio` coercion the
> `NodeModule` path applies (`main.js:409` vs `:266`), and
> `transport === undefined` is an **explicitly-handled first-class raw
> spawn path** (`main.js:425`: `cp.spawn(command, args, options)` with
> argv un-augmented, wired to stdio streams). This invokes the binary
> exactly as its contract requires (`fsm-lang-server`, no args). Doc 27's
> original `TransportKind.stdio` was the *intent* (use stdio); the
> shipped server's strict parser makes the no-`transport` form the only
> one that actually works. This is a deliberate, by-construction
> `vscode-languageclient` API affordance (verified against the locked
> `vscode-languageclient@9.0.1`), **not** a fragile internal
> coincidence — confirmed by the §11.3 post-V1 audit
> (`docs/AUDIT_PHASE_V1_2026_05_16.md` §1.1/§2.1). **Constraint for
> V2–V6:** a future wave MUST NOT 're-add' `transport:
> TransportKind.stdio` to the `Executable`, and MUST NOT switch to a
> `NodeModule` server shape, without re-deriving this seam — doing
> either silently reintroduces the exit-2 crash."

(Also re-pin, N-3: Doc 27 §2.2's `main.rs:39-48` citation should read
*"`main.rs:39-48` (`--port`), `main.rs:49-52` (unknown-arg exit-2),
`main.rs:53-65` (`None → run_stdio`)"* — the original conflates the
three arms under the `--port` citation.)

### 3.B — Doc 27 §2.3 + §7-risk-5 + §8-V1(c), and Doc 28 §2.2 + R-4 + §3-V1(c) + §5-V1(c) (UTF-8-unreachable)

> **Replace** the load-bearing assertion shared by Doc 27 §2.3 (*"…
> `vscode-languageclient` ≥ 8 sends `general.positionEncodings`
> automatically … the entire Doc 26 §4.1 / risk-1 transcoding-defect
> deletion rides on the server seeing `utf-8` in the offer"*), Doc 27
> §7-risk-5, Doc 27 §8-V1 clause (c) (*"assert the negotiated
> `positionEncoding` is **UTF-8** under the ≥8 client"*), Doc 28 §2.2
> bullet (*"a client that sends `general.positionEncodings` so the
> shipped server's UTF-8 fast-path (R-4) is reachable"*), Doc 28 R-4's
> "the UTF-8 fast-path is reachable" framing, and Doc 28 §3-V1 / §5-V1
> clause (c) (*"the negotiated `positionEncoding` is **UTF-8**"*),
> **with the following canonical statement:**
>
> "**UTF-8 `positionEncoding` is UNREACHABLE through the official
> `vscode-languageclient`** (verified against v9.0.1; the same holds for
> v8). `lib/common/client.js:1370` **hardcodes**
> `general.positionEncodings = ['utf-16']` as an unconditional literal
> with **no opt-in, override, or middleware**, and `client.js:835-836`
> **throws** `Unsupported position encoding` if the server negotiates
> anything other than UTF-16. The official client is intrinsically
> UTF-16 (VS Code's text model is UTF-16). Consequently the shipped
> server's documented **fallback** runs: the client offers only
> `['utf-16']` → the server's negotiation
> (`server.rs:216-226`: `if offer.contains(UTF8) {Utf8} else {Utf16}`)
> selects **UTF-16** → the client accepts it (no throw). **This is
> CORRECT** (Doc 26 §4.1: the UTF-16 LineIndex path is still correct; it
> loses only the *performance* fast-path, not correctness). The
> substantive Doc 26 §4.1 defect-class (mis-encoded multibyte columns) is
> **NOT** guarded by negotiating UTF-8 (impossible) but is guarded
> **end-to-end by V1 acceptance assertion (d)** — a first error *after*
> a non-ASCII line, Range byte-compared to the `fsm check --json`
> oracle — which runs **under this exact UTF-16 negotiation**. The
> surviving, real constraint (formerly stated as 'do not strip
> `general.positionEncodings`') is now: **do NOT override
> `clientOptions`/`initializationOptions` in a way that suppresses the
> client's default capabilities; `initializationOptions` carries ONLY
> the four inlay keys.** Therefore **V1 acceptance (c) asserts the
> negotiated encoding is `"utf-16"`** (the correct value for the
> official client — a *throw* or `undefined` would be the failure), and
> **(d) is the substantive multibyte-defect guard**. Doc 27 §7-risk-5 /
> Doc 28 K-5 stand as written *except* that the regression they guard is
> 'a `clientOptions` override that suppresses default capabilities or
> mis-handles the UTF-16 response', not 'a stripped
> `general.positionEncodings`'. Confirmed by the §11.3 post-V1 audit
> (`docs/AUDIT_PHASE_V1_2026_05_16.md` §2.2). **R-4's verified
> server-side negotiation logic (`server.rs:216-226/:239`) is unchanged
> and remains exact — only R-4's *client-reachability assumption*
> ('UTF-8 reachable iff the extension does not strip it') is false; the
> server-side code R-4 verified is correct and the UTF-16 branch it
> documented is the one that legitimately runs.**"

**Net (the one-line orchestrator takeaway):** the shipped V1 code is
correct on both seams; the corrections are **pure documentation-of-record
fixes**. No code change is implied or required. The shipped acceptance
test already encodes the corrected (c). These fold at v1.3-closeout; the
binding obligation before V2 is that the V2/V3 briefs reference this §3.

---

## 4. Lens 3 — Other V1 doc-vs-shipped drift sweep

V1 is the integration keystone; per the brief I spot-checked Doc 22
§2.2/§13.2, the four `fsmLang.*` inlay keys (R-5),
`engines.vscode`/client-version floor, and activation vs Doc 22 §2 for
*further* prose contradicted by the shipped `editors/vscode/`. Beyond
the §3 reconciliation (N-1/N-2) and the §0 N-3 citation re-pin:

| # | Area | Shipped (file:line) | Doc prose | Verdict |
|---|---|---|---|---|
| D-1 | Activation events | `package.json:27-30` `onLanguage:fsm-lang` + `workspaceContains:**/*.fsm` | Doc 22 §2 (22:47-50), Doc 27 §2.4, Doc 28 §5 | ✅ **byte-exact; no `*`** (Doc 22:57 honoured) |
| D-2 | Binary-resolution order | `serverBinary.ts:56-74` (Rule 1→2→3, no silent fallback) | Doc 27 §2.2 (1/2/3), Doc 22 §12 | ✅ correct (the `fsm` vs `fsm-lang-server` name imprecision is the **pre-existing** Doc 27 §10 ⚠️ / Doc 28 R-3 row — not new) |
| D-3 | Doc 22 §12 error string | `serverBinary.ts:20-25` | Doc 22:616-617 verbatim string | ✅ **byte-exact** |
| D-4 | Crash-recovery params | `crashRecovery.ts:19-27,49-97` | Doc 22 §13.2 table (22:663-698) | ✅ all 8 params + counter-reset semantics match (§1.3) |
| D-5 | Four inlay keys (R-5) | `inlayConfig.ts:31-51`, `package.json:56-76` | Doc 27 §2.5 / Doc 28 R-5 (exactly four; defaults priorities/timers/master ON, state-types OFF) | ✅ exact; the dead Doc 22 §8 keys correctly **not** contributed (the §2.5 "never silent no-op" rule) |
| D-6 | `engines`/client floor | `package.json:17` `^1.85.0`, `:102` `vscode-languageclient ^9.0.1` | Doc 27 §2.1 (`^1.85.0`, client ≥8), Doc 28 §2.2 (≥8) | ✅ satisfies floor (the UTF-8 unreachability is the §3.B doc correction, not a version problem) |
| D-7 | Deactivation handshake | `extension.ts:282-289` `client.stop()` | Doc 27 §2.2 ("`LanguageClient.stop()` performs the Doc 22 §13.1 shutdown/exit handshake; extension does not hand-roll it"), Doc 22 §13.1 | ✅ delegated to the client as documented |
| D-8 | Status-bar §10 *icons/text* | `statusBar.ts:28-61` | Doc 22 §10 icon/text table (22:565-571) | ✅ icons/text exact; **N-4 (cosmetic, deferrable):** the "restarting" *tooltip* renders generic `"…starting"` not Doc 22:679's literal `"FSM Language Server restarting (attempt N/3)..."` — inside the unexercised recovery path; Doc 27 §2.5/§10 already makes Doc 22 §10 intent-shorthand. **Deferrable to V3/V5.** |
| D-9 | `fsm.restartLanguageServer`/`fsm.showOutputChannel` commands | `statusBar.ts:74` sets `item.command = "fsm.showOutputChannel"`; `restartServer()` (extension.ts:274-280) **not** contributed as a `package.json` command | Doc 22 §10 status-bar `command: fsm.showOutputChannel`; Doc 27 §8 / Doc 28 §3 — **commands are explicitly V3 scope** | ⚠️ **N-5 (latent, NOT a V1 defect — correctly scoped):** `statusBar.ts` wires `command:"fsm.showOutputChannel"` and the exhaustion notification calls `fsm.restartLanguageServer` semantics, but **neither command is contributed in `package.json`** (V1 scope boundary — commands are V3). This is **correct depth-first scoping** (extension.ts:267-273 documents it explicitly) — but it is a **V3 hard dependency**: V3 MUST contribute `fsm.showOutputChannel` and `fsm.restartLanguageServer` or the V1 status-bar click + the exhaustion-notification button are inert. **Flag for the V3 brief** (not a V2 blocker; V2 is static assets only). |

**No new contradiction beyond §3 (N-1/N-2) + N-3 + the cosmetic N-4 +
the V3-dependency N-5.** The Doc 22-§8-config-drift surface (the
debounce/maxProblems/etc. no-op keys) is **out of V1's contributed
surface entirely** (V1 contributes only the 5 real keys —
`compilerPath` + 4 inlay) and is the already-enumerated Doc 27 §10 /
Doc 28 R-6..R-8 / K-6 ledger — **not re-opened by V1** and correctly so.

---

## 5. Lens 4 — V2-readiness verdict

**PROCEED-WITH-NOTES.** The phase boundary is clean enough to dispatch
V2 and V3 per Doc 28's cadence. Reasons:

1. **Zero Rust delta; cargo quad structurally safe.** `git diff
   ceb8efd 6707d1d -- crates/ Cargo.toml Cargo.lock` empty; V1 cannot
   regress the workspace the §11.1 / W0-clean certification covers.
2. **The spine is proven and sound for V2–V6 with no rework** (§1):
   every load-bearing seam correct *by reading*; the two
   implementer-disclosed unexercised paths (binary Rule 2/3, the
   backoff handler) correct line-by-line; the omit-`transport` fix
   robust **by construction** (§1.1).
3. **The two doc-overturning findings are independently re-confirmed
   from source** (§2) and the shipped code is **correct** on both — the
   defect is purely Doc 27/28 *prose*, with the exact fold-text
   produced (§3). The shipped acceptance test already encodes the
   corrected expectation.
4. **No new code-level drift** (§4) — only documentation-of-record
   corrections and two correctly-scoped forward-dependencies.

**Mandatory-before-V2 (briefing inputs, not code/doc edits):**

- **M-1 (N-1):** the V2 **and** V3 implementer briefs MUST cite
  **this audit's §3** for the transport + positionEncoding seams, and
  MUST carry the §3.A constraint *"do not re-add `transport:
  TransportKind.stdio` to the `Executable`; do not switch to a
  `NodeModule` server shape"* (the only structural foot-gun a
  high-fan-out wave could trip). This is the Doc 28 §1.1
  "read-it-through-the-reconciliation-row" discipline extended to this
  third finding-pair.
- **M-2 (N-5):** the **V3** brief MUST include "contribute
  `fsm.showOutputChannel` **and** `fsm.restartLanguageServer` in
  `package.json` — V1 wired the status-bar click + exhaustion-notification
  buttons to them but (correctly, per scope) did not contribute them;
  they are inert until V3." (Not a V2 concern — V2 is static assets.)

**Deferrable-to-v1.3-closeout (batched Doc-00 pass, alongside the
existing R-14/R-15 re-pins):**

- **N-1 / N-2 doc edits:** fold the §3.A and §3.B text into Doc 27
  §2.2/§2.3/§7-risk-5/§8-V1(c) and Doc 28 §2.2/R-4/§3-V1(c)/§5-V1(c).
- **N-3:** the `main.rs:39-48`→`:39-48/:49-52/:53-65` citation re-pin
  in Doc 27 §2.2.
- **N-4:** the Doc 22 §10 "restarting" *tooltip*-string polish — fold
  into the V3/V5 status-bar work (cosmetic; inside the unexercised
  recovery path).

**Not blocking, not deferred — already correct:** the shipped
`extension.test.ts` test (c) asserting `"utf-16"` (the implementer
correctly diverged from the stale Doc 28 (c) instruction — this is the
*desired* behaviour, recorded here so a future "test ≠ brief"
mechanical check does not mis-flag it as a defect; it is N-2's whole
point).

**V2 and V3 are independent of each other and both unblocked** once M-1
(and M-2 for V3) are in their briefs. No code or doc edit is a
precondition to dispatching V2.

---

## 6. Audit metadata

- **Audit type:** SUBAGENT §11.3 post-V1 phase-boundary (the Doc 28
  `V1 → audit → V2/V3` gate). READ-ONLY judgment. **ZERO build**
  (`cargo`/`npm`/`tsc`/`esbuild` not invoked).
- **HEAD audited:** `6707d1d` (`main`). Worktree:
  `/root/dev/embeded-fsm-sdk-wt-v1audit`, branch
  `phase3.2/v1_3-v1-audit`.
- **Byte-identity re-confirmed:** `git diff ceb8efd 6707d1d --
  crates/ Cargo.toml Cargo.lock` → empty (Rust workspace == W0-clean).
- **Independent re-derivations performed (not trusted from context):**
  Finding 1 — `crates/fsm-lsp/src/main.rs:49-52` (unknown-arg exit 2),
  `:53-65` (`None → run_stdio`); `vscode-languageclient@9.0.1`
  `lib/node/main.js:51-63,264-266,405-433` (Executable/NodeModule
  dispatch + the `--stdio`-push gating + the `undefined`-transport
  first-class spawn). Finding 2 — `lib/common/client.js:1355-1377`
  (hardcoded `positionEncodings=['utf-16']` at `:1370`, no override),
  `:835-836` (throw on non-UTF-16). Version: installed == locked
  `9.0.1`, sha512-cross-checked against `package-lock.json`.
- **Toolchain probe (trap-aware):** `cd /root/dev/embeded-fsm-sdk &&
  rustup show active-toolchain` → `1.75.0 … (overridden by
  rust-toolchain.toml)`; bare `rustc` `1.95.0` = benign box default
  (NOT pin drift, per `feedback_embeded_fsm_toolchain_probe_trap`).
- **Fixtures verified non-vacuous:** `broken.fsm` (no `initial`) →
  FSM-E0107 (Doc 10:341 "No initial declaration"), exactly one
  diagnostic; `non_ascii_doc.fsm` (🚀+Cyrillic line 3, undeclared
  `Missing` target line 9) → FSM-E0100 (Doc 10:268 "Unknown state
  reference"), error genuinely *after* the non-ASCII line (the Doc 26
  §4.1 line-counting guard is non-vacuous).
- **Verdict:** **PROCEED-WITH-NOTES** — V2/V3 dispatchable; M-1 (and
  M-2 for V3) are mandatory briefing inputs; N-1..N-4 doc/cosmetic
  edits batch to v1.3-closeout.
- **Pattern:** mirrors `AUDIT_PHASE_W0_2026_05_16.md` (verdict TL;DR →
  Lens-1 spine + spot-checks → independent re-derivation → drift sweep
  → readiness verdict + required orchestrator actions). The
  correction-of-record (§3) is the durable artefact until the
  batched-doc closeout (the W0 §11.3 / N-1 precedent).
