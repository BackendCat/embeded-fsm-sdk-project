# FSM Studio — LSP Server Architecture

**Document ID:** FSM-ARCH-LSP
**Version:** 1.0.0
**Status:** Living Document — v1.2 epic architecture. Authoritative for the
`fsm-lsp` crate shape and its wave plan. Refines (does not contradict)
Doc 20 §4.4/§4.5/§9; where Doc 20's LSP sketch and this doc disagree on
crate shape or incrementality, **this doc wins** (Doc 20 §9 predates the
deep-extraction pass).
**Depends on:** FSM-SPEC-LSP (14), FSM-SPEC-TM (21), FSM-SPEC-VSCE (22),
FSM-SPEC-UI (05), FSM-SPEC-DIAG (10/`fsm-diagnostics`), FSM-ARCH-OVERVIEW (20),
FSM-PROC-SUBAGENT.

This is the engineering architecture for the v1.2 LSP server. It is the
output of the extraction-first-for-epics discipline (FSM-PROC-SUBAGENT;
the v1.1 retrospective fed it forward): a multi-wave epic gets a
deep-extraction architecture pass scoped against the *current code*, not a
verbal sketch, before any implementer wave. Every reuse claim here cites a
concrete type/function/file verified at HEAD `0a657d4`.

---

# 1. Why LSP-first (v1.2 sequencing)

Both the LSP server (Doc 14) and the C++17 codegen (Doc 12) are in v1.2
scope. The LSP is sequenced **first** because it has the largest downstream
dependency fan-out:

- The **VS Code extension** (Doc 22) is an LSP *client* — its
  `vscode-languageclient` wiring, status bar (`fsm.serverRunning`), crash
  recovery (Doc 22 §13), and most commands are inert without a server.
- The **Web IDE** (Doc 05 §2.5) runs "the LSP client … as a Web Worker"
  and the "Compiler (WASM) … in a separate Web Worker". The same analysis
  core the LSP wraps is what the Web IDE will compile to `wasm32`.

C++17 codegen is self-contained (it wraps the already-correct C emitter
with `extern "C"` + a CRTP shell — no new pipeline semantics) and can land
in parallel or after without blocking anything. Front-loading the
high-fan-out item maximises critical-path progress. (Roadmap v1.1
retrospective feed-forward (b).)

---

# 2. Crate shape — `fsm-lsp`

A **new workspace member** `crates/fsm-lsp`, producing a `fsm-lang-server`
binary (Doc 14 §1: server name `fsm-lang-server`, stdio default, `--port N`
TCP alternate).

## 2.1 Dependency norms — `tower-lsp` is sanctioned, not novel

The workspace `Cargo.toml` already reserves this slot explicitly:

> ```
> # Intentionally NOT included for v1.0:
> #   - tower-lsp  (LSP deferred to v1.1+)
> #   - tokio      (no WebSocket simulator server in v1.0)
> #   - wasm-bindgen (WASM deferred to v1.1+)
> ```

Adding `tower-lsp` + `tokio` to a *new host-side crate* is the anticipated
move, not a norm violation. The embedded G2 promise (heap-free, no
`malloc`, no growable containers in **generated runtime**) is unaffected:
`fsm-lsp` is a developer-host tool, never compiled into firmware. `std` +
`tokio` are fine here exactly as they are fine in `fsm-cli`.

`fsm-lsp` depends on (path deps, same pattern as `fsm-cli`):

| Dep | Why | Feature flags |
|---|---|---|
| `fsm-parser` | CST + typed AST; `parse`, `parse_with_tokens` | default |
| `fsm-analyzer` | `analyze_with_source`, `SymbolTable` | `default` (schema-validate gate is `#[cfg(debug_assertions)]`-cheap; keep on for dev) |
| `fsm-diagnostics` | `Diagnostic`/`Span`/`Severity`/`DiagnosticCode`/`RelatedInfo` | `serde` (LSP serialises) — **not** `miette` (no terminal rendering server-side) |
| `fsm-ir` | `Ir` for hover/symbol enrichment (transition counts, payload types) | default |
| `tower-lsp` | LSP framework + `lsp-types` re-export (Doc 20 §1034 decision) | — |
| `tokio` | async runtime (`rt-multi-thread`, `io-std`, `macros`, `sync`, `time`) | — |
| `serde_json` | already a workspace dep; LSP config parsing | — |

**`fsm-lsp` MUST NOT re-implement analysis.** It calls the *same* two
functions `fsm check` calls (§3 below). Doc 20 §9.4 already mandates this
("No handler re-analyzes the document"); this doc makes the reuse seam
concrete and load-bearing.

## 2.2 `tower-lsp` / `lsp-types` offline-availability risk

`tokio` 1.52.x is in the local cargo cache; **`tower-lsp` and `lsp-types`
are not** (verified: `~/.cargo/registry` has no `tower-lsp*`/`lsp-types*`).
The L1 wave will need network for the first `cargo fetch`, or a vendored
add. This is an L1 *prerequisite to flag in the brief*, not a v1.2 blocker.
Pin exact versions in `[workspace.dependencies]` (Doc 23 §2 MSRV 1.75
governs the choice — pick the newest `tower-lsp` line that still builds on
1.75; record the pin rationale in Doc 00 §11 like the `toml = "0.8"`
precedent).

## 2.3 Module layout

Adopt Doc 20 §9.2's layout (it is sound) with this doc's refinements:

```
crates/fsm-lsp/src/
├── lib.rs            # re-exports; `run_stdio()` / `run_tcp(port)` entry points
├── main.rs           # bin: arg parse (--port, --version), tokio runtime, run
├── server.rs         # Backend: impl tower_lsp::LanguageServer
├── document_store.rs # DocumentStore: Url -> Arc<RwLock<AnalyzedDocument>>
├── analysis.rs       # the reuse seam: parse + analyze_with_source; debounce
├── position.rs       # *** byte-Span <-> LSP-Position; the encoding boundary
├── refs.rs           # reference-index pass (NEW analysis; §4.7)
└── capabilities/
    ├── diagnostics.rs   # publishDiagnostics (L1 MVP)
    ├── document_symbol.rs
    ├── hover.rs
    ├── definition.rs
    ├── completion.rs
    ├── references.rs
    ├── rename.rs
    ├── code_action.rs
    ├── semantic_tokens.rs
    ├── folding.rs
    └── inlay_hints.rs
└── tests/            # in-process tower-lsp client integration tests (§5.4-LSP)
```

Two modules are **new infrastructure** beyond Doc 20 §9.2: `position.rs`
(§4) and `refs.rs` (§4.7) — both justified below.

## 2.4 `AnalyzedDocument` — corrected vs Doc 20 §9.3

Doc 20 §9.3 sketches `ast: Vec<MachineDecl>` and `ir: IrDocument`. Correct
to the real types at HEAD:

```rust
pub struct AnalyzedDocument {
    pub uri: Url,
    pub text: String,            // current full buffer (sync model §4.3)
    pub version: i32,
    pub parse: fsm_parser::ParseResult,        // owns GreenNode + parse errors
    pub analysis: fsm_analyzer::AnalysisResult,// ir + diagnostics + symbol_table
    pub line_index: LineIndex,   // precomputed byte<->position (§4.1)
    pub refs: ReferenceIndex,    // §4.7, derived once per analysis
    pub analyzed_at: Instant,
}
```

`fsm_parser::ParseResult { green, errors }` and
`fsm_analyzer::AnalysisResult { ir: Option<Ir>, diagnostics, symbol_table }`
are the real shapes (verified `crates/fsm-parser/src/parse.rs:25`,
`crates/fsm-analyzer/src/lower/mod.rs:59`). `ast` is **not** stored — it is
a zero-cost `ParseResult::ast()` view recoverable on demand
(`#[repr(transparent)]` over the green tree; `crates/fsm-parser/src/ast/mod.rs`).
`ir` is `Option<Ir>` (partial IR is preferred to none).

---

# 3. The keystone reuse seam — diagnostics pipeline

`fsm-lsp` reuses the *exact* `fsm check` pipeline. Verified at
`crates/fsm-cli/src/cmd/check.rs:46,61`:

```rust
let pr = fsm_parser::parse(&src);                       // CST + parse errors
let result = fsm_analyzer::analyze_with_source(&pr, &label, &src);
// result.diagnostics  -> publishDiagnostics
// result.symbol_table -> hover/def/completion/symbols
// result.ir           -> hover/symbol enrichment
```

`analysis.rs` calls these two functions and nothing else for the analysis
step. The only `fsm check`-specific extra is the import-security pass
(`security_check_imports` → `resolve_import`); the LSP SHOULD run it too so
its diagnostics match the CLI exactly (an `OutsideWorkspace` import must
squiggle in-editor identically to `fsm check`). This guarantees **the LSP
can never disagree with `fsm check`** — same code path, not a parallel
re-implementation. It also means every diagnostic-producing wave that ever
shipped (the §5.4 gcc-RUN corpus, conformance suite) transitively validates
LSP diagnostics for free.

`AnalysisResult.symbol_table`'s own doc-comment
(`crates/fsm-analyzer/src/lower/mod.rs:66`) already says it is "exposed so
downstream tools (**LSP completion**, documentation generators) can re-use
it without rebuilding." The substrate was deliberately built for this.

---

# 4. Position encoding & incrementality (the load-bearing design risk)

## 4.1 Span model and the THREE incompatible line/col implementations

`fsm_diagnostics::Span` is a **byte-offset** half-open range `[start,end)`
(`crates/fsm-diagnostics/src/lib.rs:23-33`). Its doc-comment is explicit:

> Both ends are byte offsets, not char or UTF-16 units — **the LSP layer is
> responsible for converting** to whatever shape the editor requires.

There are already **two divergent** byte→line/col converters in-tree, and
**neither is LSP-correct**:

| Site | Counts | Base | Used by |
|---|---|---|---|
| `fsm_analyzer::util::compute_line_col` (`util.rs:163`) | **bytes** (`col += 1` per byte) | 1-based | IR `SourceLocation` → codegen/sim refs |
| `fsm_cli::cmd::check::line_col` (`check.rs:197`) | **Unicode scalars** (`col += 1` per `char`) | 1-based | `--json`, human render |

LSP 3.17 positions are **0-based line + 0-based `character`**, and the
`character` is measured in **UTF-16 code units** under the default
`positionEncoding`. So `fsm-lsp` MUST own a **third** dedicated converter
in `position.rs`. Conflating any existing one would produce off-by-one
(0 vs 1 base) and, worse, silently wrong columns on any line containing a
non-ASCII or non-BMP character (emoji in a `///` doc comment, Cyrillic in a
string literal). This is a precise, real defect class — not theoretical.

**Decision — negotiate UTF-8, fall back to UTF-16.** Advertise
`positionEncoding: ["utf-8", "utf-16"]` in `initialize` (an LSP 3.17
`general.positionEncodings` client capability; `vscode-languageclient`
≥ 8 supports it). If the client accepts `utf-8`, an LSP `character` is a
**byte** column → it maps *directly* onto `Span` byte offsets with only a
line-start lookup, eliminating the entire UTF-16 transcoding class. If the
client only supports `utf-16` (older clients, raw Monaco), fall back to a
correct byte→UTF-16 converter. `position.rs` owns a `LineIndex` built once
per analysis (`Vec<u32>` of line-start byte offsets; binary-search
byte→line, then encode the intra-line slice in the negotiated unit). This
is the rust-analyzer model and is O(log n) per lookup, O(n) to build.

Risk if mis-scoped: a naive `bytes`-as-`character` shim that "works on the
ASCII examples" and silently corrupts ranges on Unicode — exactly the
symbol-presence-test trap the §5.4 mandate exists to stop. The acceptance
test (§5.4-LSP) MUST include a non-ASCII fixture.

## 4.2 Span → LSP Range

Every `Diagnostic` carries a primary `Span` + `Vec<RelatedInfo>` (each with
its own `Span`). The mapping is total and mechanical:
`Range { start: pos(span.start), end: pos(span.end) }`, `RelatedInfo` →
`DiagnosticRelatedInformation`, `Severity` → `DiagnosticSeverity`
(Error→1, Warning→2, Info→3, Hint→4 — note Doc 05 §1.4.3 wants Hint as a
distinct grey squiggle, so do NOT collapse Info/Hint the way the `miette`
impl does at `lib.rs:505`), `DiagnosticCode` → `code` +
`codeDescription.href` (Doc 05 §1.4.3: codes are clickable links to online
docs). No new analysis — pure projection.

## 4.3 Document sync model — full, not incremental

Doc 14 §2 advertises `textDocumentSync.change = 2` (**Incremental**). This
refers to the *wire* sync (client sends deltas). It does **not** require an
incremental *parser*. Decision for v1.2:

- Accept incremental wire sync (`change = 2`) — apply each
  `TextDocumentContentChangeEvent` to the in-memory buffer string (cheap,
  using `LineIndex` to resolve the edit range to a byte splice).
- **Re-parse + re-analyze the whole buffer** on the debounced trigger.
  This is `parse(&full_text)` → `analyze_with_source(...)`.

This is the pragmatic v1.2 choice. See §4.4 for why it is safe.

## 4.4 Parser-resilience verdict — re-parse-on-change IS viable for v1.2

**Verdict: YES. Incremental parse is NOT a v1.2 prerequisite.** Evidence
from the code, not the prose:

1. The parser is a genuine rust-analyzer-style **resilient, never-panic**
   parser. `crates/fsm-parser/src/parser.rs`: `error_until` (panic-mode
   resync with a *guaranteed-progress* loop, `parser.rs:333-340`),
   `err_and_bump`, all malformed input wrapped in `ERROR_NODE`. DoS-bounded
   (`ParseLimits`: input bytes, token count, recursion depth) — adversarial
   input short-circuits to one `E0010` + an empty-but-well-formed `FILE`
   CST, never hangs (`parse.rs:67-106`).
2. The CST root is **always** a `FILE` node; `ParseResult::ast()` is
   infallible (`parse.rs:41-44`: `.expect("root node is always FILE")`).
   Every editor keystroke yields a usable tree.
3. Trivia is fully preserved (byte-exact round-trip — `lib.rs:13-15`,
   `flush_leading_trivia` + `skip_trivia`), so hover/semantic-tokens over
   comments and `documentSymbol` work on broken input.
4. The grammar driver itself has a **forward-progress guard** for
   zero-consumption rules (`grammar/file.rs:114-119`) and explicitly
   continues past a missing `language` header "so a user editing the body
   of their file **in an LSP setting** still sees structure / completions"
   (`grammar/file.rs:18`, verbatim). The parser was written
   LSP-resilience-aware from day one.
5. `parse_with_tokens(src, tokens)` already exists, documented "Used by the
   **LSP incremental-reparse pipeline**" (`parse.rs:90-106`) — it lets a
   future optimisation skip re-lexing untouched ranges *without* changing
   the v1.2 contract.

Performance: full re-parse+re-analyze of a typical embedded `.fsm`
(hundreds–low-thousands of lines) is sub-millisecond to low-single-digit-ms
in this Rust pipeline. With the 200ms debounce (Doc 14 §14, Doc 22 §8
`fsmLang.debounceMs`), full re-analysis is comfortably within budget. The
50ms target in Doc 20 §4.5 is met by full re-parse for realistic embedded
file sizes; incremental parsing is a *v1.3+ optimisation for pathologically
large files*, not a v1.2 correctness need.

## 4.5 Prose-vs-code drift to flag (do NOT build on the false claim)

Doc 20 **§4.5** ("the affected CST subtree is replaced, and only the
changed portion of the AST is rebuilt") and Doc 20 **line 787**
(`pub fn parse_incremental(old_tree: &SyntaxNode, edit: &TextEdit) -> ParseResult;`)
describe **incremental CST patching that does not exist in code**. Verified:
`fsm-parser` exposes `parse`, `parse_with_limits`, `parse_with_tokens` only
(`crates/fsm-parser/src/lib.rs:52`); `parse_with_tokens` still drives the
*full* grammar (`grammar::parse_file` over the whole token vec,
`parse.rs:102-105`). There is **no** `parse_incremental`, no green-tree
diff/splice. This is aspirational architecture prose (the P0-1
"aspirational-prose" pattern the project has been bitten by before).
**Scope decision: v1.2 implements full re-parse-on-debounce (§4.3/4.4) and
the L-plan does NOT depend on `parse_incremental`.** If a future wave wants
true incremental parsing it is a *separate, self-contained
`fsm-parser` epic* with its own behavioural acceptance — not smuggled into
the LSP waves. Doc 20 §4.5/L787 should be annotated as "v1.3+ aspiration,
not implemented" in a later doc-reconciliation pass (out of scope for this
read-only wave; flagged here per the brief).

## 4.6 Multi-file / `import` projects

Doc 14 §6/§7/§14 wants cross-file goto-definition/references and a
workspace scan on server start. The analyzer's `SymbolTable` is **per-file
/ per-parse** (`SymbolTable::build(&ast::File)`); there is **no
cross-file/project symbol index** in-tree, and `fsm check` itself only
resolves imports for the *security containment* check, explicitly **not**
loading sibling files (`check.rs:184-188`: "sibling file may not exist
yet"). Building a true multi-file project index is a large new subsystem.

**Scope decision: v1.2 LSP is single-file for semantic features.**
- `publishDiagnostics`, `documentSymbol`, `hover`, `completion`,
  `semanticTokens`, `foldingRange`, `inlayHint`, single-file
  `definition`/`references`/`rename` — all in scope (single document).
- Cross-file `definition`/`references` and the startup workspace scan
  (Doc 14 §6 "Cross-file definition requires the server to have indexed
  the full workspace", §14 "Scan all `*.fsm` in workspace") — **explicitly
  deferred to v1.3** (pairs naturally with the Web IDE multi-file story).
  Doc 14 §8 *already* specifies the graceful single-file degradation:
  cross-file rename returns the user-visible error *"Cross-file rename is
  not supported in LSP v1. Rename in each file manually."* — v1.2 honours
  that exact contract, so this is shipping the spec's own v1 boundary, not
  under-delivering.

This is called out so no wave assumes a project index "comes for free."

## 4.7 The new analysis `fsm-lsp` must add — `ReferenceIndex`

`SymbolTable`'s `resolve_*` methods are **name → declaration** only
(`symbol_table.rs:362-433`). There is **no reverse index** (declaration →
all use sites). `hover`/`definition`/`documentSymbol`/`completion` need
*only* the existing forward table + declaration `Span`s — **zero new
analysis**. But `textDocument/references` and `rename` need the reverse
direction, which does not exist anywhere in-tree.

`refs.rs` adds a **single CST walk** (not a re-analysis) producing a
`ReferenceIndex`: for each resolvable identifier token (state ref after
`->`/`~>`/`initial`/fork/join; event in `on`/`raise`/`send`/`defer`; extern
call; `ctx.field`; machine in `send … to M`), record `(symbol-key, Span,
role=decl|ref)`. It reuses `SymbolTable` for the resolution decision (so
"references" means *semantically resolved* references, not text matches)
and the typed AST accessors for token discovery. This is new code but it is
*derived from* existing analysis, not a parallel analyzer — it cannot drift
from `fsm check` because resolution still goes through `SymbolTable`.

---

# 5. Capability set & per-capability reuse design

`initialize`/`initialized`/`shutdown`/`exit` — `tower-lsp` lifecycle.
`initialize` returns the `ServerCapabilities` from Doc 14 §2 (plus the
`positionEncoding` negotiation, §4.1). Lifecycle behaviour per Doc 14 §14 +
Doc 22 §13 (client owns crash-restart; server owns the 200ms debounce).

| Capability | Existing API it reuses (concrete seam) | New code needed |
|---|---|---|
| `publishDiagnostics` **(L1 MVP)** | `parse` + `analyze_with_source` → `result.diagnostics`; `security_check_imports`/`resolve_import` (`check.rs:46,60,61`) | `position.rs` Span→Range; debounce driver; nothing analytical |
| `documentSymbol` | `AnalysisResult.symbol_table` — per-machine ordered `events/externs/consts/enums/states/regions/context_fields`; `StateEntry.container_path`+`shape` build the Doc 14 §13 tree | tree assembly + Span→Range only |
| `hover` | `symbol_table` resolve_* + `Ir` (transition counts, payload types, extern signatures, `@id`) | Markdown templating (Doc 14 §5 / Doc 05 §1.4.4); token-at-position |
| `definition` | `symbol_table.resolve_state/event/extern/context_field/machine` → `Entry.span` / `StateEntry.span` is the decl site | token-at-position; Span→Range; **single-file only** (§4.6) |
| `completion` | `symbol_table` name lists (events/states/externs/`ctx.`/`payload.`); keyword list = **Doc 04 §1.5** (authoritative; Doc 14 §4 forbids inlining) | trigger-context classification from CST cursor; snippet table (Doc 22 §9) |
| `references` | `refs.rs` `ReferenceIndex` (§4.7), resolution via `symbol_table` | the `ReferenceIndex` pass (new, derived) |
| `rename` + `prepareRename` | `ReferenceIndex` → in-file `WorkspaceEdit`; entity-kind gate from `symbol_table` | rename-safety guard (§7); Doc 14 §8 cross-file error string verbatim |
| `codeAction` | `Diagnostic.code` + `.span` drive the Doc 14 §9 qu-fix table; AST for refactor.extract | per-code edit synthesis (text edits) |
| `semanticTokens` | CST token kinds + `symbol_table` to disambiguate decl-vs-ref and pseudo-state-name-as-type (Doc 14 §10 / Doc 21 §6) | delta encoding (LSP relative token format); legend from Doc 14 §10 |
| `foldingRange` | CST node ranges (`machine/state/composite/parallel/region/context` bodies, block comments) — pure tree walk | Doc 14 §12 mapping |
| `inlayHint` | `Ir` (non-default priority, timer durations, state child counts) + Doc 22 §8 toggles | Doc 14 §11 formatting (human duration) |

Two observations that constrain the plan:
- Everything except `references`/`rename` reuses **only** the existing
  forward `SymbolTable` + CST/AST + `Ir`. The only genuinely new *analysis*
  is the §4.7 reference index.
- `semanticTokens` and TextMate (Doc 21) coexist by Doc 21 §6's own rule
  (TextMate during startup/large-file, semantic tokens refine analyzed
  regions). Doc 21 is already drafted; the L-plan consumes it, doesn't
  redesign it.

---

# 6. The pub-surface dependency (cross-reference the v1.2-gate pub-hygiene wave)

The v1.1 retrospective made a **`pub→pub(crate)` sweep + `unreachable_pub`
/`missing_docs` lints** the standing v1.2 gate, BEFORE v1.2 features
(Roadmap retrospective feed-forward (a)). Measured at HEAD: `fsm-parser`
≈ 264 pub items with only 3 `pub(crate)` — it is the dominant offender
exactly as the retrospective states; `fsm-analyzer` ≈ 66, `fsm-diagnostics`
≈ 24, `fsm-ir` ≈ 78.

**The tension:** the pub-hygiene wave narrows surface; the LSP needs a
*specific slice* of that surface as **intended cross-crate API**. These two
waves MUST be coordinated or the hygiene wave will `pub(crate)` something
the LSP legitimately needs (or, worse, the LSP will be built reaching into
genuinely-accidental `pub` internals that *should* be narrowed).

**Seams the LSP needs to remain (or become) intended public API** —
distinct from accidental over-`pub`:

- `fsm-parser`: `parse`, `parse_with_tokens`, `ParseResult` (+ `.green`,
  `.errors`, `.syntax()`, `.ast()`), the whole `ast::*` typed-accessor
  surface, `cst::{SyntaxNode, SyntaxToken, SyntaxKind, FsmLanguage}` and
  `AstNode`/`AstChildren`/`child`/`children`/`child_token`. These are the
  LSP's CST/AST traversal substrate — **intended API**.
- `fsm-analyzer`: `analyze`, `analyze_with_source`, `AnalysisResult` (+
  fields), `SymbolTable` and its `resolve_*` methods + the entry types
  (`Entry`, `StateEntry`, `MachineSymbols`, `EnumEntry`, `StateShape`,
  `SubmachineEntry`). The `symbol_table` field's doc-comment *already*
  declares LSP a consumer — promote these to *documented* intended API.
- `fsm-diagnostics`: already a clean, minimal foundation crate (24 items,
  all genuinely public by design). No action; `fsm-lsp` consumes it with
  the `serde` feature.
- `fsm-ir`: the read accessors `fsm-lsp` uses for hover/inlay (transition
  priority, timer durations, payload types, `@id`). The hygiene wave should
  keep the *read* surface; internal builders may narrow.

**Recommended coordination (orchestrator decides):** run the pub-hygiene
wave **first** (it is bounded, non-behavioural — retrospective (a)), and
its brief MUST cite this §6 list as the "keep public, document as intended
API" allowlist so the narrowing is *informed by the LSP's needs* rather
than blind. `#[doc]` the kept seams as "stable for `fsm-lsp` / external
tooling". The LSP waves then build on a deliberately-shaped surface. This
sequencing is the whole point of extraction-first: the LSP architecture
*informs* the cleanup, instead of the cleanup blindly preceding it.

---

# 7. Risks & open questions

1. **Position-encoding correctness (HIGH).** Two wrong line/col impls
   already exist (§4.1); a third naive one is the likely failure. Mitigation:
   `position.rs` is its own module with its own Unicode test fixture;
   negotiate UTF-8 to delete the transcoding class entirely; the §5.4-LSP
   acceptance test asserts a diagnostic range over a non-ASCII line.
2. **Rename-safety (HIGH).** Doc 14 §8 forbids renaming machine names and
   `@id` strings (breaking changes) and scopes rename to one file. A
   too-broad text-substitution rename corrupts code silently. Mitigation:
   `rename` is **`ReferenceIndex`-driven only** (semantic refs, never text
   match); `prepareRename` hard-rejects machine/`@id`/keyword tokens via
   `symbol_table`; the acceptance test renames a state and asserts only the
   semantically-bound occurrences changed (and a same-spelled string
   literal did NOT).
3. **Parser-resilience under partial input (MEDIUM, assessed → LOW).**
   Mitigated by design (§4.4): the parser is provably never-panic +
   progress-guaranteed + always-`FILE`-root. Residual: capability handlers
   must treat every AST accessor as `Option` (the AST contract already is —
   `ast/mod.rs:10-12`) and never `unwrap()` on a cursor-resolved node.
   Acceptance: a "type half a transition" fixture must still yield hover +
   symbols without panic.
4. **`tower-lsp`/`lsp-types` not cached offline (MEDIUM, logistical).**
   §2.2 — L1 needs a network `cargo fetch` or a vendored add; pin versions
   MSRV-1.75-compatibly. Flag in the L1 brief's prerequisites.
5. **Doc 20 §4.5 / L787 false-incremental-parse prose (MEDIUM).** §4.5 —
   do not let any wave assume `parse_incremental` exists. The plan is
   full-reparse; a later doc pass annotates Doc 20.
6. **Multi-file expectations in Doc 14 (MEDIUM).** §4.6 — Doc 14 §6/§7/§14
   describe workspace-wide indexing that has no in-tree substrate. v1.2 is
   single-file; Doc 14 §8's own cross-file error string is the sanctioned
   degradation. Brief every relevant wave with the single-file boundary.
7. **Testing methodology (MEDIUM).** Symbol-presence assertions are
   forbidden for behaviour (FSM-PROC-SUBAGENT §5.2/§5.4). The LSP analogue
   of "gcc-compile-and-RUN" is an **in-process `tower-lsp` test client that
   issues real JSON-RPC requests and asserts response payloads** (§5.4-LSP).
   Open question the L1 wave resolves concretely: `tower-lsp`'s preferred
   in-process test harness (duplex transport vs `LspService::call`).
8. **Open question — config breadth.** Doc 14 §3 / Doc 22 §8 define a large
   `fsmLang.*` config surface (inlay toggles, style-warning gates, complexity
   threshold). v1.2 should wire the ones that gate *existing* analyzer
   behaviour (debounce, maxProblems, enableStyleWarnings/Hints,
   actionComplexityThreshold) and stub-accept the rest. Final cut is an
   L-wave judgment call recorded in Doc 00 §11.

---

# 8. Scoped implementer-wave breakdown (L1…L7)

Sequenced so **L1 proves the architecture end-to-end** (initialize +
publishDiagnostics reusing the *exact* `fsm check` pipeline) before any
breadth. Each wave: deliverable, reuse seams, and a **§5.4-LSP behavioural
acceptance** (the analogue of the gcc-compile-and-RUN mandate — an
in-process `tower-lsp` client sends a real request and asserts the response
payload; **symbol/route presence is NOT acceptance**).

### L0 (gate prerequisite, NOT an LSP wave) — pub-hygiene + intended-API
Run the standing v1.2 pub-hygiene wave (retrospective (a)) FIRST, briefed
with the §6 allowlist. Deliverable: `pub→pub(crate)` sweep +
`unreachable_pub`/`missing_docs`; the §6 seams kept and `#[doc]`'d as
intended `fsm-lsp` API. Acceptance: workspace quad green; a throwaway
compile-probe in a scratch crate proves every §6 seam is reachable
cross-crate. *This is the existing gate wave, listed here only to pin the
dependency edge; it is not new scope created by this doc.*

### L1 — MVP: server skeleton + `publishDiagnostics` (proves the spine)
- **Deliverable:** `crates/fsm-lsp` crate; `fsm-lang-server` bin; stdio
  transport; `initialize` (Doc 14 §2 capabilities + UTF-8/UTF-16
  `positionEncoding` negotiation, §4.1); `didOpen`/`didChange`(full-buffer
  apply)/`didClose`; 200ms debounce; `publishDiagnostics` via
  `parse`+`analyze_with_source`(+`security_check_imports`); `position.rs`
  `LineIndex` + Span→Range for the negotiated encoding.
- **Reuse seams:** `fsm_parser::parse`; `fsm_analyzer::analyze_with_source`
  → `.diagnostics`; `fsm_parser::import_resolver::resolve_import`;
  `fsm_diagnostics` (serde). New: `position.rs`, debounce, server shell.
- **§5.4-LSP acceptance:** in-process `tower-lsp` client: (a) `initialize`
  → assert advertised capabilities + negotiated encoding; (b) `didOpen` a
  **known-broken** `.fsm` → assert the received `publishDiagnostics`
  contains the **exact** `FSM-Exxxx` code(s) **and the exact LSP Range**
  that `fsm check --json` reports for the same source (cross-checked
  against the CLI, the established oracle — proves identical pipeline);
  (c) `didOpen` a fixture whose first error is **after a non-ASCII line**
  (emoji in a `///`) → assert the range is correct under BOTH negotiated
  encodings (the §4.1 defect guard); (d) `didChange` to fix the error →
  assert diagnostics clear. **Symbol-presence (route exists) is NOT
  acceptance — the asserted Range bytes are.**

### L2 — `documentSymbol` + `foldingRange` (pure forward-table/CST)
- **Deliverable:** Doc 14 §13 hierarchical symbol tree; Doc 14 §12 folding.
- **Reuse:** `AnalysisResult.symbol_table` (ordered tables, `container_path`,
  `shape`); CST node ranges. No new analysis.
- **§5.4-LSP acceptance:** `documentSymbol` on the Motor example → assert
  the full `DocumentSymbol` tree shape (Machine→context/events/externs/
  states with nesting + `SymbolKind`s) equals the Doc 14 §13 structure;
  `foldingRange` → assert one range per machine/state/composite body with
  correct start/end lines.

### L3 — `hover` + `definition` (single-file)
- **Deliverable:** Doc 14 §5 / Doc 05 §1.4.4 hover Markdown; Doc 14 §6
  goto-definition (single-file; cross-file deferred §4.6).
- **Reuse:** `symbol_table.resolve_*` (decl `Span`s); `Ir` for transition
  counts / payload types / signatures; token-at-position via CST.
- **§5.4-LSP acceptance:** position the cursor on a state reference in a
  transition → assert `definition` returns the **exact Range of the
  `state NAME {` declaration**; hover on an event → assert the response
  Markdown contains the payload field list from the IR. Negative: hover in
  whitespace → `None`, no panic.

### L4 — `completion`
- **Deliverable:** Doc 14 §4 / Doc 05 §1.4.5 context-sensitive completion
  (keywords from **Doc 04 §1.5**; events/states/externs/`ctx.`/`payload.`;
  snippets from Doc 22 §9).
- **Reuse:** `symbol_table` name lists; CST cursor → trigger-context.
- **§5.4-LSP acceptance:** `completion` after `on ` in a state body →
  assert the item set equals exactly the machine's declared event names
  (kind 20); after `-> ` → exactly the state names; after `ctx.` → exactly
  the context fields with their type in `detail`. Assert a keyword item's
  snippet text matches Doc 14 §4.

### L5 — `references` + `rename` + `prepareRename` (introduces `ReferenceIndex`)
- **Deliverable:** `refs.rs` `ReferenceIndex` (§4.7); Doc 14 §7 references;
  Doc 14 §8 rename (in-file `WorkspaceEdit`) + `prepareRename` with the
  machine/`@id`/keyword rejection + the verbatim cross-file error string.
- **Reuse:** `symbol_table` for resolution (refs are *semantic*); CST for
  token spans.
- **§5.4-LSP acceptance:** `references` on a state declared once and used in
  3 transitions → assert exactly 4 locations (decl + 3), correct Ranges;
  `rename` that state → assert the `WorkspaceEdit` changes exactly those 4
  Ranges **and a string literal of the same spelling is NOT in the edit**
  (the §7 rename-safety guard); `prepareRename` on a machine name → assert
  it returns the Doc 14 §8 rejection, not a range.

### L6 — `semanticTokens` (consumes Doc 21)
- **Deliverable:** Doc 14 §10 legend; full + range; decl-vs-ref +
  pseudo-state-name-as-`type` disambiguation; LSP delta encoding.
- **Reuse:** CST token kinds; `symbol_table` for the decl/ref +
  pseudo-state distinction; Doc 21 §6 coexistence rule.
- **§5.4-LSP acceptance:** `semanticTokens/full` on a fixture exercising
  every legend index → decode the relative-encoded array and assert each
  token's (line, char, len, type, modifier) matches the Doc 14 §10 mapping
  (e.g. a state name at its declaration = type index 1 + modifier
  `declaration`; the same name after `->` = type 1, no `declaration`).

### L7 — `codeAction` + `inlayHint`
- **Deliverable:** Doc 14 §9 quick-fix table (diagnostic-driven) + the two
  refactor.extract actions; Doc 14 §11 inlay hints with Doc 22 §8 toggles.
- **Reuse:** `Diagnostic.code`/`.span` → fix synthesis; `Ir` for inlay
  values; config plumbing.
- **§5.4-LSP acceptance:** open a file with `FSM-E0107` (no initial) →
  request `codeAction` at the diagnostic → assert the returned action's
  `WorkspaceEdit` inserts `initial <FirstState>` at the correct Range and
  that re-analysing the edited buffer clears `FSM-E0107`; `inlayHint` on a
  transition with `priority 50` → assert a hint `// priority: 50` at the
  right position; toggle `fsmLang.inlayHints.showTransitionPriorities=false`
  → assert it disappears.

**Sequencing rationale.** L0 (gate) → L1 (spine, proves the reuse seam +
position encoding end-to-end — the riskiest architecture bet validated
first) → L2/L3/L4 (breadth on the *existing* forward substrate, no new
analysis, independently parallelisable after L1) → L5 (the one new analysis,
`ReferenceIndex`) → L6 (semantic tokens, consumes the drafted Doc 21) → L7
(actions/hints, depends on diagnostics+IR already proven). A phase-boundary
audit (FSM-PROC-SUBAGENT §11.3) after L1 and after L5 (the analysis-adding
wave) — same discipline that would have caught P0-1 at the boundary.

---

# 9. What this doc deliberately scopes OUT of v1.2 (no implied freebies)

- **Incremental CST parsing** (`parse_incremental`). Does not exist
  (§4.5); full re-parse-on-debounce is the v1.2 contract; incremental is a
  separate v1.3+ `fsm-parser` epic with its own acceptance.
- **Cross-file / workspace-wide** definition, references, rename, and the
  startup workspace scan (Doc 14 §6/§7/§14). No in-tree project-index
  substrate; v1.2 is single-file and ships Doc 14 §8's own cross-file
  degradation message. Deferred to v1.3 (with the Web IDE multi-file story).
- **`wasm32` build of `fsm-lsp`** (Doc 05 §2.5 Web Worker LSP). v1.2 is the
  native stdio/TCP server. The architecture stays WASM-*compatible* (the
  analysis core is `tokio`-free; only `server.rs`/transport touch tokio) so
  v1.3 can add a `wasm32` target without re-architecting — but compiling it
  is explicitly v1.3 scope, not a v1.2 deliverable.
- **`workspaceSymbol`** (Doc 14 §2 advertises it). Requires the same
  project index as cross-file; defer with §4.6. Either drop it from the
  advertised capabilities in v1.2 or back it with single-file results — an
  L2 judgment call recorded in Doc 00 §11.

If Doc 14 implies any of the above is "free", it is not — they are real
subsystems and are deferred explicitly rather than half-built.

---

*End of FSM-ARCH-LSP v1.0.0*
