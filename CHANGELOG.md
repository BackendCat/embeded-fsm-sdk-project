# Changelog

All notable changes to FSM Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] — 2026-05-16

**Theme: Developer tooling — the `fsm-lsp` Language Server.**

> **v1.2 re-scoping (user-endorsed; `docs/00-Decisions-And-Reconciliation.md` §11.40):** `v1.2.0` ships **the LSP server only**. The VS Code extension moved to **v1.3** (its natural LSP-client home; architecture in `docs/27-VSCode-Extension-Architecture.md`), the old v1.3 "Simulation & verification" theme to **v1.4**, and C++17 codegen (Doc 12) to its **own minor**. This keeps the validated small-tight-tagged cadence (v1.0/v1.1) and the post-v1.1 retrospective's LSP-first feed-forward; a mega-v1.2 would contradict the shippable-increment pattern. Reversible via the ROADMAP's own re-versioning mechanism.

### Added

- **`fsm-lsp` — Language Server (LSP) spine** (v1.2-LSP-L1): a new
  `fsm-lang-server` binary speaking LSP over stdio with `initialize`
  (UTF-8/UTF-16 `positionEncoding` negotiation, UTF-8-preferred),
  full-document sync, a 200ms debounce, and `textDocument/publishDiagnostics`
  that reuses the **exact** `fsm check` analysis pipeline (parser +
  analyzer + import-security) — editor squiggles can never disagree with
  `fsm check --json` (byte-exact Range + code parity proven by an
  in-process `tower-lsp` client test under both encodings, incl. a
  multibyte-line fixture). `#![forbid(unsafe_code)]` (workspace now
  10/10). See `docs/26-LSP-Architecture.md` §8 L1 and
  `docs/00-Decisions-And-Reconciliation.md` §11.32.
- **`fsm-lsp` — `documentSymbol` + `foldingRange`** (v1.2-LSP-L2): the
  Doc 14 §13 hierarchical symbol tree (machine → context/events/externs/
  states with composite/region nesting + `SymbolKind`s, name
  `selectionRange`s) and Doc 14 §12 folding regions, both advertised in
  `initialize`. `documentSymbol` reuses the **same** single `fsm check`
  analysis the diagnostics path runs (the `symbol_table` is now threaded
  through — one analysis feeds both, no second pass / no second position
  converter, diagnostics byte-identical to L1); `foldingRange` is a pure
  parse-tree walk. Proven by an in-process `tower-lsp` client test
  asserting the full tree + every range + folds, byte-equal under both
  UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L2 and
  `docs/00-Decisions-And-Reconciliation.md` §11.33.
- **`fsm-lsp` — `hover` + `definition`** (v1.2-LSP-L3, single-file): Doc
  14 §5 GitHub-Markdown hover (symbol kind + IR-sourced detail — event
  payload field types, `pure` extern signatures, context-field
  type/default, state kind + transitions-out) and Doc 14 §6
  goto-definition, both advertised in `initialize` and built on a shared
  token-at-cursor seam whose resolution mirrors the analyzer's own
  name-resolution dispatch and goes through the **same**
  `SymbolTable::resolve_*` `fsm check` uses (a goto/hover can never
  disagree with a squiggle). Cross-file/unresolved symbols return
  `null`/`None` — the spec-correct single-file graceful degradation
  (cross-file is v1.3), never a fabricated location. Hover's structured
  detail comes from the lowered `Ir`, threaded **additively** through the
  single analysis (the exact behaviour-neutral pattern L2 used for
  `symbol_table` — one analysis feeds all, no second lowering, no second
  position converter; L1 diagnostics + L2 symbols/folds byte-identical).
  Proven by in-process `tower-lsp` client tests asserting definition
  ranges byte-equal to the analysis oracle, hover Markdown structured
  content, the cross-file-`null` case, and correct position mapping under
  **both** UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L3 and
  `docs/00-Decisions-And-Reconciliation.md` §11.34.
- **`fsm-lsp` — context-aware `completion`** (v1.2-LSP-L4, single-file):
  Doc 14 §4 `textDocument/completion` advertised with the exact Doc 14 §2
  trigger characters `[".", ":", "@", "[", " "]`; a CST trigger-context
  classifier that **reuses L3's `resolve` substrate** (the same ancestry
  walk + `in_guard` predicate + a shared `prev_significant_token`
  primitive — not a parallel detector) offers ONLY context-correct
  candidates (transition target → state names; after `on `/`raise`/`defer`
  → events; guard → context fields + `pure` externs; `ctx.` → context
  fields; action → fields + all externs + statement keywords; structural
  starts → Doc 04 §1.5 keywords + Doc 14 §4 snippets), every name sourced
  from the single threaded `symbol_table` (one analysis, no second
  converter) and keywords pinned verbatim to Doc 04 §1.5 by a test.
  Wrong-context candidates are structurally impossible; an unclassifiable
  cursor returns an empty list, never a symbol dump. Proven by in-process
  `tower-lsp` client tests asserting, per context, the served set (labels
  + kinds) equals the analysis oracle AND a wrong-context candidate is
  absent, under **both** UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L4 and
  `docs/00-Decisions-And-Reconciliation.md` §11.35.
- **`fsm-lsp` — `references` + `prepareRename`/`rename`** (v1.2-LSP-L5,
  single-file): Doc 14 §7 `textDocument/references` and Doc 14 §8
  `textDocument/prepareRename` + `textDocument/rename` advertised
  (`rename_provider.prepareProvider = true`). Powered by the new
  `ReferenceIndex` — the **one genuinely-new analysis** of the LSP epic,
  *derived* from the single `fsm check` analysis (a single CST walk over
  its `symbol_table` + parse — no second analysis pass, no parallel
  resolver) and **semantic-only**: a token is a reference iff the SAME L3
  `resolve` classifier (mirroring `checks::name_resolution`, resolving
  through the SAME `SymbolTable::resolve_*` `fsm check` uses) resolves it
  to *exactly that declaration* — never an identifier-string/text match.
  A same-spelled token inside a string literal, a comment, or trivia is
  not a CST `Ident` in a classified position, and one in a different
  scope resolves to a different declaration; both are excluded **by
  construction**, so a `rename`'s `WorkspaceEdit` can never silently
  corrupt the user's source (Doc 26 risk-2, the cardinal silent-data-loss
  risk in its most acute form). `references` honours `includeDeclaration`;
  `prepareRename` hard-rejects (up-front, the LSP contract — never a
  silent allow) machine names (codegen/ABI blast radius, out of v1.2
  scope), `@id`/state-id annotation strings, keywords/contextual keywords,
  non-identifier cursors, and any string/comment/trivia position; `rename`
  additionally rejects an invalid new identifier and an in-scope name
  collision with a clear message and **no edit**. Cross-file
  references/rename remain explicitly v1.3 (Doc 26 §4.6/§9). Still one
  analysis, one position converter, one resolver — reused, not
  duplicated; L1–L4 diagnostics/symbols/hover/goto/completion proven
  byte-unchanged. Proven by the most exhaustive in-process `tower-lsp`
  client safety matrix of any wave — the headline risk-2 test asserts a
  safe rename's edit set is **exactly** the semantic references and that a
  same-spelled string-literal substring, comment word, AND different-scope
  symbol are **NONE of them** in the `WorkspaceEdit` (applying the edits
  leaves the other machine + the comment + the string byte-preserved and
  the buffer still parses clean), plus a six-case `prepareRename`
  negative-rejection matrix and all reference/rename ranges correct under
  **both** UTF-8 and UTF-16 on a multibyte fixture. See
  `docs/26-LSP-Architecture.md` §8 L5 and
  `docs/00-Decisions-And-Reconciliation.md` §11.36.
- **`fsm-lsp` — `semanticTokens` (`full` + `range`)** (v1.2-LSP-L6,
  single-file): Doc 14 §10 `textDocument/semanticTokens/full` and
  `textDocument/semanticTokens/range` advertised in `initialize` with the
  Doc 14 §2/§10 `legend` (11 token types + 4 modifiers) declared **once**
  and reused for both the advertisement and the encoder, so the advertised
  indices and the encoded `tokenType`/`tokenModifiers` can never diverge.
  Semantic tokens are *more precise* than the Doc 21 TextMate grammar: a
  bare `Ident` is one TextMate scope everywhere, but this layer knows —
  from the **same** single `fsm check` analysis the LSP already runs —
  whether it is a state / event / extern / context field / machine and
  whether it is a *declaration* or a *reference*. The decl-vs-ref +
  entity-type split **reuses** L3's `resolve` classifier (use sites) and
  L5's `ReferenceIndex` declaration-name discovery / `SymbolKey` taxonomy
  (declaration sites) through one shared entity model — **no new analysis,
  no parallel classifier** (Doc 26 §8 L6); a declaration and a use of the
  same symbol get the identical legend type and differ only by the
  `declaration` modifier. Non-`Ident` tokens get their lexical `fsm-lexer`
  `SyntaxKind` type (keyword / operator / number / string / comment / `@id`
  decorator); structural punctuation Doc 14 §10 has no legend slot for is
  not emitted (the client uses the Doc 21 TextMate scope — Doc 21 §6
  coexistence). Emitted as the LSP relative delta array
  (`[deltaLine, deltaStartChar, length, tokenType, tokenModifiers]`,
  sorted, deltas relative to the previous token, `deltaStartChar` reset on
  a new line) with `deltaStartChar`/`length` in the negotiated
  `positionEncoding` via L1's one authoritative `LineIndex` (no second
  converter); a multi-line comment is split into one token per line (LSP
  `multilineTokenSupport` defaults off), and `range` is a self-contained
  substream (its first token's deltas relative to the response start).
  Cross-file remains explicitly v1.3 (Doc 26 §4.6/§9). Still one analysis,
  one position converter, one classifier — reused, not duplicated; L1–L5
  diagnostics/symbols/hover/goto/completion/references/rename proven
  byte-unchanged. Proven by in-process `tower-lsp` client tests that
  **decode the relative delta array back to absolute positions** and assert
  the full decoded stream equals the reused-pipeline oracle (symbol
  presence is NOT acceptance — the decoded bytes are), with hard-coded
  per-token cross-checks (a state declaration carries the `declaration`
  modifier, a state use does not, a `ctx.field` ref is the field type, a
  comment is `comment`, a keyword is `keyword`, the `@id` decorator + its
  string), the advertised legend asserted equal to Doc 14 §2/§10 order and
  to the encoder's, and a non-ASCII fixture under **both** UTF-8 and UTF-16
  with the decoded positions/lengths asserted correct in each. See
  `docs/26-LSP-Architecture.md` §8 L6 and
  `docs/00-Decisions-And-Reconciliation.md` §11.37.
- **`fsm-lsp` — `codeAction` + `inlayHint`** (v1.2-LSP-L7, single-file —
  the **final** LSP capability of the v1.2 epic): Doc 14 §9
  `textDocument/codeAction` and Doc 14 §11 `textDocument/inlayHint`
  advertised in `initialize` (`codeActionProvider.codeActionKinds =
  ["quickfix","refactor"]`, `inlayHintProvider`). `codeAction` is
  **edit-producing**, so it inherits L5's risk-2 silent-corruption
  discipline: a `quickfix` is offered **only** when the fix is provably
  mechanical — **FSM-E0107** (insert `initial <FirstState>` after the
  machine's `{`, the first state taken from the same parse the diagnostic
  came from) and **FSM-E0022** (delete the duplicate event declaration,
  whose span the analyzer already pins exactly; **withheld** if the
  duplicate carries its own `@id` annotation, where the deletion would
  stop being mechanical). The other five Doc 14 §9 codes
  (`FSM-E0100`/`E0106`/`W0200`/`W0500`/`E0300`) **and both
  `refactor.extract` actions** are deliberately **scoped out and flagged**
  (Doc 00 §11.38): `W0200`/`W0500` are never emitted by the toolchain at
  all, and `E0100`/`E0106`/`E0300`/extract cannot be produced without a
  heuristic or a forbidden parallel re-analysis — a missing quick-fix is a
  minor UX gap, a wrong edit is the cardinal sin. The edit is matched on
  the server-authoritative diagnostic code (not the client-supplied
  context), tied back to the diagnostic it resolves, and every range goes
  through the one `LineIndex` (no string munging). `inlayHint` is
  **read-only display** sourced from the lowered `Ir` of the **same**
  single `fsm check` analysis — the three Doc 26 §5 families
  (non-default transition priority `// priority: 50`, human timer
  durations `// 1.5 s` / `// 1 min 30 s`, composite/parallel substate
  count `// 5 substates`), each gated by its Doc 22 §8 toggle plus the
  master `enableInlayHints`, read from `initialize`
  `initializationOptions` and live `workspace/didChangeConfiguration`
  (only the behaviour-gating inlay keys are consumed; every other
  `fsmLang.*` key is stub-accepted, never a panic). Doc 14 §11's
  extern-parameter-name row is scoped out (no Doc 22 §8 toggle, not in Doc
  26 §5's IR-sourced set) and flagged. Still one analysis, one position
  converter — reused, not duplicated; L1–L6
  diagnostics/symbols/folding/hover/goto/completion/references/rename/
  semantic-tokens proven byte-unchanged (785 workspace tests, the `fsm`
  examples 5/5 and conformance 25/25 suites green). Proven by in-process
  `tower-lsp` client tests applying L5's apply-and-verify rigor to the
  edit-producer — the headline test requests the FSM-E0107 quick-fix,
  asserts the `WorkspaceEdit` byte-equals the reused-pipeline oracle,
  **applies it**, and asserts the diagnostic is resolved, no new
  diagnostic appears, every other byte is unchanged, and the buffer still
  parses; plus a no-bogus-action negative (a scoped-out diagnostic yields
  no action), an inlayHint oracle with hard-coded Doc 14 §11 family
  cross-checks and a `didChangeConfiguration` toggle round-trip, and a
  non-ASCII fixture under **both** UTF-8 and UTF-16 with the codeAction
  edit range and the inlayHint positions asserted correct in each. After
  this wave the v1.2 LSP capability set is **feature-complete**. See
  `docs/26-LSP-Architecture.md` §8 L7 and
  `docs/00-Decisions-And-Reconciliation.md` §11.38.
- **`FSM-W0200` ("loop in action block") is now emitted** (FU-DEAD-CODES):
  the code was catalogued but had **zero emission sites** anywhere in the
  toolchain (a documented diagnostic that could never fire — surfaced
  writing the L7 `codeAction`). Four normative docs mandate it (Doc 02
  §9.2, Doc 04 §8.7.2, Doc 11 §18, Doc 10's W0200 catalog entry), so a
  single emission site was added (`fsm-analyzer` `checks/action_lint.rs`:
  one `FSM-W0200` per `while`/`for` loop whose ancestor chain contains an
  `ACTION_BLOCK`) — a style nudge, not an error; loops remain permitted.
  Formal conformance fixture `tests/conformance/semantic/neg/004_loop_in_action/`
  added (suite 25→26). See
  `docs/00-Decisions-And-Reconciliation.md` §11.47.

### Removed

- **`FSM-W0500` ("extern declared but never used") retired** (FU-DEAD-CODES):
  it was vestigial — no normative spec mandates it and it was **emitted
  nowhere**. Moved to `fsm_diagnostics::deprecated::DeprecatedCode::W0500`;
  it is **not a behavioural break** — it still parses in suppression
  annotations and `fsm check --allow FSM-W0500` / `fsm.toml [compiler]
  allow` (Doc 10 §14 rule 2 — the FU#67 allow/deny contract is preserved,
  test-pinned). The live `DiagnosticCode` count moves **74 → 73** (the
  `all_codes_matches_expected_count` lock test). See
  `docs/00-Decisions-And-Reconciliation.md` §11.47.

### Security

- **RUSTSEC-2026-0009 eliminated** (SEC-FU; `time` ≤0.3.36 DoS via stack
  exhaustion, CVSS 6.8) — found by a pre-tag SCA scan (itself a
  *self-dropped* recommendation from the v1.0 `AUDIT_2026_05_14` pass, now
  operationalised; see *Known limitations* and Doc 00 §11.48). `time` was
  transitive-only via `jsonschema 0.17 ← fsm-ir` (the W0 IR-schema gate).
  The advisory's literal remedy (`time ≥0.3.47`) was infeasible — every
  such `time` needs rustc ≥1.88 / Cargo `edition2024`, which the
  deliberate load-bearing **1.75** toolchain pin cannot parse. Fixed at
  the **root** by bumping `jsonschema` 0.17 → 0.22 in
  `crates/fsm-ir/Cargo.toml` (0.22.0 dropped the `time` edge outright;
  MSRV 1.70 still satisfies the 1.75 pin; the deprecated `JSONSchema::compile`
  shim migrated to `jsonschema::options()/.build()/Validator`). W0
  schema-gate behaviour byte-identical; re-scan reports **0
  vulnerabilities**; the 1.75 pin is unchanged (the stable toolchain is
  additive — it builds `cargo-audit` only). See
  `docs/00-Decisions-And-Reconciliation.md` §11.42.

### Fixed

- **`fsm.toml [compiler] allow/deny` was parsed but never applied** (FU#67,
  mild P0-1 silent-no-op class — a documented option that did nothing):
  `CompilerSection.{allow,deny}` are now finalized in `cmd::check` and
  projected post-analysis by `diagnostics::apply_allow_deny` (allow →
  suppressed/exit 0; deny → error/exit 1; allow+deny → allow wins; an
  unknown code → loud exit-4; a retired code → accepted; a control code →
  unchanged), matched on the parsed `DiagnosticCode` (never a substring
  match on source). +13 tests. Doc 18 §6/§6.1. See
  `docs/00-Decisions-And-Reconciliation.md` §11.41.

### Changed

- Internal API hygiene: 170 accidentally-`pub` items across `fsm-parser`,
  `fsm-formatter`, and `fsm-cli` `src/` (in private modules / the binary
  crate) narrowed to `pub(crate)`; no public API or behaviour change
  (673/0 tests, 5/5 examples, 25/25 conformance, IR fingerprints all
  unchanged). The `unreachable_pub` lint wiring is deferred — residual
  warnings are confined to shared integration-test helpers, not `src/`
  (see `docs/00-Decisions-And-Reconciliation.md` §11.31).
- **`unreachable_pub` is now wired workspace-wide and self-enforcing**
  (FU#68): §11.31's manual sweep narrowed the 170 over-`pub` `src/` items
  but deferred the lint. `[workspace.lints.rust] unreachable_pub = "warn"`
  is now set with every crate opting in (`[lints] workspace = true`) and
  the shared per-test-binary helpers explicitly `#![allow(unreachable_pub)]`'d
  (the legitimate test-only idiom); the 14 residuals it surfaced (the
  `fsm-lsp` `semantic_tokens.rs` legend/modifier consts) were downgraded
  `pub → pub(crate)` (no external consumer, behaviour-inert). Combined with
  the quad's `clippy -D warnings`, accidental `src/` over-`pub` now fails
  the gate instead of silently re-accreting. See
  `docs/00-Decisions-And-Reconciliation.md` §11.43.
- **The two divergent byte→line/col implementations converged** (DRIFT-2):
  `fsm_analyzer::util::compute_line_col` (bytes, 1-based) and
  `fsm_cli::cmd::check::line_col` (Unicode scalars, 1-based) now both
  delegate to ONE
  `fsm_diagnostics::compute_line_col(src, pos, LineColUnit::{Byte|Scalar})`
  core (each behaviour-identical to its old self — proven byte-for-byte on
  all three contracts incl. multibyte, plus a cold-verified quad). The
  LSP's `position.rs::LineIndex` is a structurally different algorithm
  (precomputed line-start table, 0-based, encoding-negotiated) and is
  **deliberately left-and-explained, not folded in** (forcing it into the
  linear-scan core would worsen clarity for zero behaviour gain — the new
  `SUBAGENT_CONVENTIONS` §10 refactor-to-number anti-pattern, which
  DRIFT-2 codifies by practising). No new deps. See
  `docs/00-Decisions-And-Reconciliation.md` §11.44.
- Docs: `docs/20-Architecture-Overview.md` reconciled to shipped reality
  (DRIFT-1/-3/-4/-5; annotate-not-delete, design intent labelled
  historical, every claim cited to `file:line`). §4.5 / §12.2 advertised
  an incremental-CST-reparse pipeline and a `parse_incremental(old_tree,
  edit)` API that **do not exist** — `fsm-parser` does a **full** re-parse
  only (`parse`/`parse_with_limits`/`parse_with_tokens`); these (plus §4.1
  + ADR-004 and the §12.3 `analyze_incremental` analogue) are annotated as
  a **v1.3+ deferred optimisation, not implemented**, design intent
  preserved as the future target (DRIFT-1 §11.39, DRIFT-3 §11.45). §4.2
  module layout / §4.3 `ParseResult` and §5.2 analyzer layout / §5.3
  `AnalysisResult`/`SymbolTable`/`Scope` types reconciled — the §5.2
  listing's two-`phase{1,2}_*` directory tree, `ir_builder.rs`, `tests.rs`
  and `DiagnosticAccumulator` do not exist; the shipped analyzer is flat
  (`symbol_table.rs` + a `checks/` family + the AD-3 `lower/` tree;
  diagnostics are a plain `Vec<Diagnostic>`), the two-phase split kept as
  the *conceptual* model (DRIFT-3 §11.45, DRIFT-4 §11.46). §5.4/§5.5's
  strict-2-phase narrative + stale per-step code claims reconciled the
  same way (DRIFT-5): the real pipeline is `SymbolTable::build` →
  `checks::run_all` → `lower_file`; **`FSM-E0301` is not emitted** (Doc 00
  B-07 explicitly allows guarded completions — `checks/completion.rs`) and
  the §5.5 forward-reachability/`FSM-E0400` step is **not implemented**
  (catalogued, zero emission sites, `COVERAGE_MAP` `UNTESTED`). The v1.2
  LSP epic deliberately built on full re-parse-on-debounce (Doc 26
  §4.3–§4.5). No code change. See
  `docs/00-Decisions-And-Reconciliation.md` §11.39 / §11.45 / §11.46.
- **`docs/processes/SUBAGENT_CONVENTIONS.md` §10** gains the
  *refactor-to-number anti-pattern* row (the one design take-away of the
  external-kit evaluation, Doc 00 §11.48): do not split/merge code purely
  to hit a metric if it worsens readability/clarity; if no clean
  behaviour-safe restructuring exists, leave the violation and **explain
  why** — exactly what DRIFT-2's left-and-explained LSP `LineIndex`
  practised.
- **`.github/workflows/ci.yml`** gains an SCA (software-composition-analysis)
  job running `cargo audit` on a **separate recent stable toolchain**
  (`rustup toolchain install stable` → `cargo +stable install cargo-audit
  --locked` → `cargo audit`) — the 1.75-pinned `cargo-audit 0.21.1`
  cannot parse the modern CVSS-4.0 advisory DB, so it is a distinct job
  that does **not** gate the 1.75 build matrix. Operationalises the
  self-dropped v1.0 audit recommendation that caught RUSTSEC-2026-0009
  pre-tag (Doc 00 §11.48 / §11.42).

### Known limitations (v1.2) — documented, diagnosed, tracked / accepted debt

- **`analyzer → parser-CST coupling` (~15 files) is deferred — accepted,
  tracked debt.** The architecture audit rated it explicitly
  *ship-acceptable, non-behavioural* P1. A ~15-file refactor of the most
  behaviourally-critical crate (`fsm-analyzer`) **at a release boundary**
  is worse-EV than doing it cleanly post-tag (a tracked v1.2.1/early-v1.3
  wave). **v1.1.0 shipped the same arch-debt class tracked + documented in
  its `GATE_VERIFICATION_v1_1.md`** — deferring here is precedent-consistent
  and lower-risk; it will be recorded in `GATE_VERIFICATION_v1_2.md` as
  accepted-tracked-debt. Doc 00 §11.49.
- **Live diagnostic-code count is 73** (not 75 — `FSM-E0903` retired in
  v1.1 when the `defer` runtime shipped, `FSM-W0500` retired in v1.2 as
  vestigial; each −1). The frozen `GATE_VERIFICATION_v1_0.md` /
  `_v1_1.md` correctly cite "75" / "36/75" *at their pinned commits* (a
  frozen attestation is not rewritten); `GATE_VERIFICATION_v1_2.md`
  (authored at tag-time) will cite 73. Doc 00 §11.47.
- **Conformance coverage:** 26/73 codes have formal `tests/conformance/`
  fixtures (W0200 added one this release); the rest are exercised by
  crate-level negative tests. No behavioural gap; formal-suite closure
  remains tracked.
- **CI matrix not yet exercised:** `.github/workflows/ci.yml` (now incl.
  the SCA job) is configured but has not run against v1.2 commits (the
  repo is unpushed by design — the user controls the remote). Pushing the
  tag to exercise the matrix is the documented post-tag action.
- **v1.2 re-scope:** the VS Code extension is **v1.3** (Doc 27), not part
  of `v1.2.0`; C++17 codegen is its own minor; the old v1.3 "Simulation &
  verification" theme is **v1.4**. Doc 00 §11.40.

## [1.1.0] — 2026-05-15

### Corrected — v1.0.0 scope statement (audit integrity)

The v1.0.0 entry below described scope as "full UML statechart semantics." This was **overstated**: **submachine support is entirely absent** across the pipeline (grammar has no `submachine`/`state … is X` production; `KwIs`/`KwSubmachine` tokens are orphans; `lower.rs` hard-codes `submachines: Vec::new()`; IR `SubmachineRef` is only built by a unit-test fixture; codegen/simulator arms are defensive-only for a never-produced variant). Earlier ROADMAP/backlog claims that "IR + analyzer already lower submachine references" were aspirational prose never reconciled against code — the same class as the P0-1 finding. The three shipped examples (motor, traffic-light, vending-machine) do not use submachines, so `GATE_VERIFICATION_v1_0.md` was accurate for what it tested; the defect was the *scope wording*, not the gate evidence. Correct reading of v1.0.0: **UML statecharts excluding submachines**.

**Resolution (v1.1):** the submachine epic landed across four waves — W2a parser/CST/AST (`7a69612`), W2b analyzer/IR lowering + FSM-E0103/E0500/E0501/E0502/E0610 (`2384608`), W2c simulator runtime semantics Doc 08 §12 (`8b66dc2`), W2d C99 codegen both dispatch strategies (`af8c300`). Submachine is implemented **end-to-end for top-level `state X is Sub`** — parser → analyzer/IR → simulator → codegen, `gcc -std=c99 -Werror`-verified (both strategies) and sim≡codegen behaviour-matched, exercised by `examples/submachine/`. A submachine reference **nested inside a composite or parallel state** is **rejected at analysis with `FSM-E0502`** and a clear actionable message ("only supported on a top-level state in v1.1; … move the `state … is X` to the machine's top level"); the lowerer additionally refuses it (defence-in-depth) — so codegen never receives it and **no broken C is emitted** (the clean-reject-over-broken-output / retired-E0903 precedent, G1 no-undefined-behaviour). Implemented by P1-2 (`02d4ded`); test-pinned by `crates/fsm-analyzer/tests/nested_submachine_rejected.rs`. Proper nested *support* is the tracked **SUB-FU-2** follow-up (→ v1.1.x). Accurate v1.1 scope: **full UML for top-level `state X is Sub`; a nested submachine reference is a documented, diagnosed limitation (FSM-E0502)** — NOT a blanket "full-UML coverage" claim. (History/lesson: this statement was over- then mis-corrected several times — "inert leaf" then "emits broken C / fix in progress"; both were stale. The shipped reality is the clean E0502 reject above. Recorded so the lesson — verify the *current* code, conservative wording, applies to audit claims too — is not lost: Doc 00 §11.29.)

### Added
- **Submachine support end-to-end** (W2a-d) — top-level `state X is Sub` references (parser→analyzer/IR→simulator→codegen, both strategies, gcc -Werror + sim≡codegen); a submachine ref nested in a composite/parallel state is rejected with `FSM-E0502` (proper nested support tracked SUB-FU-2 → v1.1.x).
- `defer EVENT` runtime (removes the v1.0 FSM-E0903 limitation) — bounded buffer + FIFO replay on state exit; sim≡codegen verified.
- IR-schema-validation gate (debug-mode, zero release cost) — malformed IR caught at the analyzer boundary; proven load-bearing.
- Shared `CARGO_TARGET_DIR` + warm-cache build policy (wave-speed + disk-safety).
- TD-BUG-1 + 3 sibling table-strategy degenerate-input codegen bugs fixed (zero-transition machine now gcc -Werror-clean, both strategies).
- Architecture-debt paydown: unified LCA (one generic `ParentResolver`), `LoweringCtx` god-object eliminated, `fsm-simulator` dead-dep removed.
- `likely` / `rare` transition annotations → portable `__builtin_expect` codegen (`<PFX>_LIKELY/_UNLIKELY` macros, GNU/clang builtin + standard-C fallback + opt-out), both dispatch strategies, gcc -Werror -pedantic clean. Contextual keyword — lexer unchanged, fully back-compatible.
- `fsm generate --import-header <path.h>` — auto-`extern` declarations harvested from an existing C header, removing hand-written extern boilerplate for users with established C codebases.
- `examples/integration/{make,cmake,cargo-rust,platformio}/` — four worked end-to-end integrations, each genuinely building the generated C under `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (make/cmake/cargo-rust run+assert the full lifecycle; platformio ships the real `pio` recipe + a verified host-gcc fallback). Plus `docs/25-Integration-Guide.md` (ABI contract, C/C++/Rust recipes, build-system fragments, troubleshooting matrix).
- **Per-machine dispatch strategy override** (W7) — `fsm.toml [machine.<Name>] strategy = "switch"|"table"|"auto"` selects dispatch per machine, enabling MIXED dispatch in one project (machine A switch, machine B table from a single `fsm generate`). Effective precedence per machine: `[machine.M].strategy` > CLI `--strategy` > default `auto`. Invalid value = clean exit-4 config error naming the machine; a `[machine.X]` for a machine not generated this run is a WARN (project-wide fsm.toml generating a subset stays valid). Behaviourally accepted: both strategies gcc -Werror compiled + run with per-machine lifecycle assertions.

### Fixed
- **`--import-header` / `fsm.toml import_headers` bypassed the v1.0 G-02 file-read hardening** (**security, SEC-P0-1**; the one remaining v1.1 tag-blocker, found by the pre-tag security audit): the W5 header-import surface returned absolute paths verbatim, joined relative paths with **no shape-check / no `canonicalize` / no workspace-root containment**, and read the header with **no size cap** — so a crafted `fsm.toml` on shared CI could path-traverse to an arbitrary file (`import_headers = ["../../../etc/shadow"]`) and `--import-header /dev/zero` (or a multi-GB file) could OOM the host before the parser's 1 MiB cap ran. The DSL `import "..."` path was already correctly hardened; the root cause was a **divergent parallel code path**. **Fixed by CONVERGING, not duplicating:** the attacker-controlled `fsm.toml import_headers` surface is routed through `fsm_parser::import_resolver::resolve_import` *directly* — the exact same primitive (shape-reject `..`/NUL/absolute → `canonicalize` (symlink-resolved) → workspace-root prefix) the DSL path uses; **no third copy of the containment logic exists**. A small shared `fsm-cli/src/safe_io.rs` owns the ONE bounded-read helper (capped at the same `ParseLimits::DEFAULT.max_input_bytes` the parser enforces on `.fsm` source, so the caps cannot drift) and the ONE `workspace_root_for` resolver (moved out of `cmd::check` so `fsm check` and `fsm generate` share a single definition). **Deliberate absolute-path / trust decision:** `fsm.toml` travels with the (possibly hostile) repo → contained by default; `--import-header` is invocation-supplied at the same trust level as the `.fsm` path argument and the G-02 threat model scopes the danger to a *source/config posted to a shared host*, not the CI job's own argv — and an absolute vendored-HAL path is the documented normal use of the flag (hard-rejecting it would be both wrong for the threat model and a functional regression) → the CLI flag stays absolute-capable with NUL-shape-reject + the **universal DoS size cap still applied**; the genuine vendored-HAL-via-config case is served by an **explicit, named, default-OFF** opt-in `[generate] allow_unscoped_import_headers = true` (never a silent allow; the DoS cap is non-negotiable regardless). The same size cap is also applied to the `fsm.toml` read itself (REL-P2-1, folded in per the audit — same defect class, same shared helper): an unsized stream (`/dev/zero`, FIFO) is `take`-bounded so it rejects instead of OOMing. Oversized header → clean exit-3, oversized `fsm.toml` → clean exit-4, containment escape → exit-1 (each the established mapping; never an OOM/panic). §5.4 behaviourally accepted: a new `import_header_security.rs` (mirroring `fsm-parser/tests/import_security.rs`) proves traversal / absolute-from-config / **symlink-escape** / NUL / >1 MiB / `/dev/zero` / oversized-`fsm.toml` are each rejected with the asserted exit code and the target is provably never read / never OOMs; the pre-existing W5 end-to-end gcc -Werror + RUN acceptance (both dispatch strategies) **still passes unchanged** (the hardening did not break legitimate use). Doc 00 §11.28.
- **Guard-disambiguated same-event transitions mis-lowered by BOTH dispatch strategies** (W7-FU-1, pre-existing core dispatch defect, P0-1 silent-data-loss class; surfaced by W7's §5.4 behavioural-acceptance discipline, NOT introduced by W7): a state with two or more transitions on the same event disambiguated by guards (`on E [g1] -> A` / `on E [g2] -> B`, incl. an unguarded `on E -> C` fallback — bread-and-butter UML the DSL/analyzer/IR accept and preserve) was independently mis-lowered: the **switch** strategy emitted one C `case <EVENT>:` per transition → `gcc` hard error `duplicate case value` (even without `-Werror`), the 2nd+ guarded transition unreachable; the **table** strategy's `select_for_region` returned the first `(source,trigger)` row ignoring its guard, the executor then silently no-op'd if the guard was false → the event was dropped and the later eligible transition never fired. Both now honour Doc 08 §4.1/§4.2: the first guard-enabled candidate in (priority, document-order) wins, an unguarded/`[else]` transition is the always-enabled catch-all, no enabled candidate ⇒ the event bubbles up (no consume). The simulator was independently verified spec-correct and is the oracle; the fix is codegen-only and makes both strategies match it. Behaviourally accepted on BOTH strategies (gcc -Werror compiled + RUN-asserted: priority-tiebreak when >1 guard is simultaneously true, unguarded fallback, no-match discard, sim==codegen). The honest tripwire was converted to a positive correctness test; conformance CGEN-004 added.
- **Default transition priority lowered as `0`, contradicting the spec's `100`** (W7-FU-2, pre-existing doc-vs-impl divergence; surfaced by W7-FU-1's §5.4 discipline, NOT introduced by it): a transition with **no `priority` clause** was lowered by the analyzer's four `lower_*` arms with `.unwrap_or(0)` → IR priority `0`. **Doc 04 §8.6** ("Lower number = higher priority. Default: `100`.") and **Doc 09 §6** ("default 100", canonical example `"priority": 100`) both specify **100**; the IR `model.rs` field comment already said "Default 100"; no corpus source intends `0`. **Decision: default = `100`** (the impl was the bug, not the docs). Rationale (Doc 08 §4.2 min-wins on `(priority, document_order)` — lower number = higher priority): an unprioritized transition must sit at LOW priority so assigning a small explicit number FLOATS a specific/guarded transition above the default herd; a `0` default made an unprioritized transition out-prioritise any explicitly-deprioritised one and no transition could ever be placed below the default — the priority feature was half-useless. The discrepancy was applied IDENTICALLY by the simulator and both codegen strategies (each only READS the IR `priority`), so sim≡codegen and W7-FU-1's *relative* ordering held regardless — hence P1 (consistent, well-defined) not P0 (silent miscompile). Reconciled to a single source of truth: `fsm_ir::DEFAULT_TRANSITION_PRIORITY = 100`, referenced by the four `lower_*` arms and a serde `default`; the wrong `lower/mod.rs` doc-comment and the IR JSON-schema `priority` annotation were aligned (the W0 IR-schema gate stays green — `priority` is `required`/always-serialized so a JSON-Schema `default` annotation does not affect validation). Behaviourally accepted per §5.4 on BOTH strategies (a hand-built IR — the analyzer's FSM-E0300 forbids a clause-less transition in same-source same-event runtime competition, so the default's *observable* effect is only demonstrable at the codegen/sim contract layer below the compile-time gate: gcc -Werror compiled + RUN-asserted the spec-correct min-wins transition fired, sim≡codegen). The pre-existing `ir_default_priority_is_zero` unit test was pinning the *bug* — corrected to assert `100` (the FAILS-on-old / PASSES-on-new regression anchor + §5.4 unit pin). No shipped example's behaviour changed (no example `.fsm` uses `priority`; the W7-FU-1 fixture/CGEN-004 uses explicit `priority 1/2/3`; FSM-E0300/W0300 keys off `priority.is_some()`, not the materialized default). Doc 00 §11.27.
- **Opaque extern params silently dropped** (OPAQUE-BUG-1, pre-existing core defect, P0-1 silent-data-loss class): `extern` declarations using the documented `opaque "C_type"` form for params/returns were silently lost in lowering (`lower_type_ref` returned `None` for `OPAQUE_TYPE_REF`). Now modelled as `Type::Opaque { c_type }` end-to-end (context field, extern param, extern return, `as`-cast) — gcc-RUN verified. Unblocks passing C handles/pointers/structs to HAL functions from FSM actions.
- **Parser panic on a comment before `language fsm`** (PARSE-BUG-1, pre-existing G1-robustness defect): a `//` license header atop a source file (an extremely common real-world input) tripped the rowan single-root assertion. Leading trivia is now flushed after the FILE node opens — the toolchain parses/diagnoses instead of panicking.

### Known limitations (v1.1) — documented, diagnosed, tracked

- **Submachine nested in a composite/parallel state** is rejected at analysis with `FSM-E0502` + an actionable message (move it to the machine's top level); proper nested *support* is tracked as **SUB-FU-2** (→ v1.1.x). Top-level `state X is Sub` is fully supported.
- **Diagnostic conformance coverage:** 36/75 codes have formal `tests/conformance/` fixtures; the remaining 39 are exercised by crate-level negative tests (not the formal harness). No behavioural gap — closure to 75/75 in the formal suite is tracked for v1.1.x.
- **CI matrix not yet exercised:** `.github/workflows/ci.yml` (linux/macos/windows × fmt/clippy/build/test) is configured; the local-equivalent quad is green but the matrix has not run against v1.1 commits (the repo is unpushed by design — the user controls the remote). Pushing the tag to exercise CI is the documented post-tag action; SEC-P0-1's path-canonicalization is the most platform-divergent surface.
- **Tracked architecture debt → gate before v1.2** (non-behavioural, ship-acceptable per the pre-tag audit): `pub` over-exposure (no `unreachable_pub`/`missing_docs` lint; ~765 items) and analyzer→parser-CST coupling (~15 files reaching into CST rather than typed AST).

### Audit trail (v1.1 pre-tag) — frozen evidence

3-lens read-only audit at `docs/AUDIT_PRE_TAG_v1_1_{CORRECTNESS,ARCH,RELIABILITY}_2026-05-15.md`:
- **Architecture** — 0 P0; AD-1/AD-2/AD-3 debt-paydown each verified *true in source* (one `ParentResolver` LCA; `fsm-simulator` analyzer-dep demoted to dev-dep; no `LoweringCtx` god-object); 9/9 crates `#![forbid(unsafe_code)]`.
- **Correctness** — 0 P0; all Doc 00 §11.19–§11.28 rows: commit-exists + code-present + **0 overstated** (the P0-1/submachine prose-vs-code class is absent from the v1.1 surface).
- **Reliability/Security** — test-integrity strong (every sampled §5.4 acceptance test is a real gcc-compile-and-RUN, not symbol-presence); found **SEC-P0-1** (the one tag-blocker — fixed, see *Fixed* above). That report's **REL-P1-1 was stale vs current code** (it traced the older `AUDIT_PHASE_SUBMACHINE` pre-P1-2 state and claimed nested-submachine "emits broken C"); the nested-reject (`FSM-E0502`) had in fact already shipped in P1-2 `02d4ded` and is test-pinned — the release record was corrected to the shipped reality, **not** regressed to the audit's stale claim (verify-vs-code applies to audit findings too). Doc 00 §11.29.

## [1.0.0] — 2026-05-14

### Added
- DSL: FSM-Lang grammar (Doc 04) — hierarchical UML statechart support.
- Compiler pipeline: lexer + parser (Rowan CST + AST) + analyzer (semantic
  checks + AST→IR lowering) + IR (with JSON Schema).
- C99 code generator with two dispatch strategies
  (`--strategy {switch,table,auto}`).
- In-process simulator (RTC interpreter, virtual clock, deterministic JSON
  traces in `StepRecord` format).
- Canonical formatter (`fsm fmt` — idempotent).
- CLI: `fsm check / generate / fmt / parse / test / doc / decompile / init`.
- HAL contract (`fsm_hal_clock_now_ms`, `fsm_hal_assert`) — mandatory for
  codegen.
- 3 worked examples (motor, traffic-light, vending-machine) — all pass
  `check → generate → gcc -Werror → fmt --check → simulator-trace-match`.
- Conformance test suite scaffold (`tests/conformance/` + `MANIFEST.json`).
- License header in generated C (`--license <SPDX>`, default MIT).
- Memory budget reporting (`fsm generate --report-memory`).
- Parser security: import path canonicalize + DoS limits (depth 256,
  1 MiB input cap).
- Determinism: `BTreeMap` in all serialized public types (Doc 13 §11
  byte-exact).
- Foundation crate `fsm-diagnostics` (Wave 1.0) — zero workspace deps;
  owns `Span`, `SourceLocation`, `Severity`, `DiagnosticCode` (75 variants),
  `Diagnostic`.

### v1.0 scope decisions
- Full UML statechart semantics (composite, parallel, history, fork/join,
  submachines, deferred events except `defer EVENT` itself which lands v1.1).
- C99 codegen only (C++17 codegen → v1.1).
- CLI only (LSP → v1.1; VS Code extension → v1.1; Web IDE → v1.1;
  simulator WebSocket protocol → v1.1).

### Implementation notes (selected — full list in `docs/00-Decisions-And-Reconciliation.md` §11)
- B-10 hierarchical dispatch implemented via leaf-to-root walk on a
  `parent_table[]`.
- B-11 collect-then-execute table dispatch implemented (per-region
  selection then sequential execute).
- B-08 parallel-state completion fires only when all regions reach Final.
- B-09 self-transition LCA: external = parent; local = self.
- B-14 history default mandatory (`FSM-E0111`).
- `after 0 ms` rejected at analyzer (`FSM-E0410`).
- `defer EVENT` rejected at analyzer in v1.0 (`FSM-E0903`,
  "v1.0 limitation; lands v1.1").
- Per-timer event IDs (`MOTOR_EVENT_TIMER_<ID>_FIRED`) — timers never
  collide with each other or with the shared completion event ID.
- Multi-active-leaf representation `m->_active[N]` — non-parallel and
  parallel machines share one dispatch path.
- Context field defaults applied in both `M_init` and the simulator's
  `Interpreter::init`.

### Verified — MVP gate (Doc 23 §9)
- G1–G5 pass (`fsm check` / `fsm generate` / `gcc -std=c99 -Werror` /
  `fmt` idempotent).
- G6 all 3 examples through full chain with simulator-trace-match.
- G7 75 diagnostic codes catalogued; 36 covered by formal conformance
  fixtures + remaining covered at crate-test level.
- G8 `cargo build / test / clippy / fmt` all clean on workspace.
- G9 GitHub Actions CI matrix configured (linux/macos/windows); local-run
  verified.
- ~491 tests passing across 9 crates.

### Deferred to v1.1+
- WebSocket simulator protocol (Doc 13).
- VS Code extension (Doc 22).
- LSP server (Doc 14).
- Web IDE (Doc 05 portions, Doc 03 simulator section).
- C++17 code generator (Doc 12).
- TextMate grammar publishing (Doc 21).
- `defer EVENT` runtime support (currently rejected at analysis with
  `FSM-E0903`).
- Submachine support. _(Correction: the original "IR + analyzer present;
  codegen silent no-op" wording was inaccurate — submachine was **absent
  end-to-end** in v1.0; see the "Corrected — v1.0.0 scope statement" note
  under [1.1.0]. The full epic landed in v1.1.)_

### Audit trail
Pre-tag audits at `docs/AUDIT_*_2026_05_14.md`:
- `AUDIT_2026_05_14.md` — correctness (5 P0 + 9 P1 + 12 P2 + 7 P3; all P0
  + critical P1 resolved).
- `AUDIT_B_ARCHITECTURE_2026_05_14.md` — architecture and coupling (0 P0;
  "architecturally sound").
- `AUDIT_C_QUALITY_2026_05_14.md` — complexity and maintainability (2 P0
  quality-class — not release blockers; cleanup tracked for v1.0.1).
- `AUDIT_D_RELIABILITY_AI_2026_05_14.md` — reliability and
  AI-friendliness (`#![forbid(unsafe_code)]` everywhere; 14/14 Doc 00
  blockers traceable in code).

[Unreleased]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/BackendCat/embeded-fsm-sdk-project/releases/tag/v1.0.0
