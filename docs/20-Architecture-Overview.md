# FSM Studio — Internal Code Architecture Overview

**Document ID:** FSM-ARCH-OVERVIEW
**Version:** 1.0.0
**Status:** Living Document
**Depends on:** FSM-SPEC-IR, FSM-SPEC-SEM

This document describes the **internal code architecture** of each SDK subsystem —
not system-level data flow (see FSM-SPEC-IR for that), but the module structure,
key data types, internal pipelines, and design patterns within each crate or package.

This is the entry point for contributors wanting to understand, modify, or extend
a specific subsystem.

---

# 1. Repository Layout

```
sm-sdk/
├── crates/
│   ├── fsm-lexer/          # Tokenizer — no_std, zero-copy
│   ├── fsm-parser/         # CST + AST construction
│   ├── fsm-analyzer/       # Symbol resolution, type-check, semantic validation
│   ├── fsm-ir/             # IR types, JSON serialization, visitor API
│   ├── fsm-codegen-c/      # C99 code emitter
│   ├── fsm-codegen-cpp/    # C++17 code emitter
│   ├── fsm-simulator/      # In-process FSM interpreter + WebSocket server
│   ├── fsm-lsp/            # Language Server (tower-lsp)
│   ├── fsm-formatter/      # Canonical formatter (fsm fmt)
│   ├── fsm-cli/            # CLI binary (clap)
│   └── fsm-wasm/           # WASM build for Web IDE (wasm-bindgen)
├── editors/
│   ├── vscode/             # VS Code extension (TypeScript)
│   └── web-ide/            # Web IDE (Vite + React + Monaco)
├── schema/
│   └── ir/1.0.0/model.json # Normative JSON Schema for IR (FSM-SPEC-IR)
├── docs/                   # All specification documents
└── tests/                  # Conformance test suite (FSM-SPEC-TEST)
    ├── 01-parser/
    ├── 02-validator/
    ├── 03-semantic/
    ├── 04-codegen-c/
    └── 05-formatter/
```

---

# 2. Compilation Pipeline

```
.fsm source
     │
     ▼
 fsm-lexer          → token stream (zero-copy, &str slices)
     │
     ▼
 fsm-parser         → CST (lossless green tree) → AST (typed)
     │
     ▼
 fsm-analyzer       → SymbolTable + SemanticDiagnostics
     │
     ▼
 fsm-ir             → IR document (MachineObject[]) + DiagnosticObject[]
     │
     ├──► fsm-codegen-c    → Motor.h, Motor.c, Motor_impl.h, Motor_conf.h
     ├──► fsm-codegen-cpp  → Motor.hpp, Motor.cpp, Motor_impl.hpp
     ├──► fsm-simulator    → live interpreter (no code gen needed)
     ├──► fsm-lsp          → hover, completion, diagnostics, inlay hints
     └──► fsm-formatter    → canonical .fsm output
```

---

# 3. Lexer — `fsm-lexer`

## 3.1 Design Philosophy

The lexer is **zero-copy** and `no_std`-compatible. All tokens are `(TokenKind, Span)`
pairs where `Span` is a `(start: usize, end: usize)` byte range into the source `&str`.
No strings are allocated; the caller slices the source to recover token text.

## 3.2 Module Layout

```
fsm-lexer/src/
├── lib.rs          # Public API: Lexer<'src>, Token, TokenKind, Span
├── lexer.rs        # Main Lexer struct and next_token() implementation
├── token.rs        # TokenKind enum (~55 variants)
└── tests.rs        # Unit tests
```

## 3.3 Key Types

```rust
pub struct Span { pub start: usize, pub end: usize }

pub struct Token { pub kind: TokenKind, pub span: Span }

pub enum TokenKind {
    // Keywords
    KwMachine, KwState, KwParallel, KwComposite, KwEvent, KwExtern, KwPure,
    KwContext, KwInitial, KwFinal, KwHistory, KwShallow, KwDeep,
    KwChoice, KwJunction, KwFork, KwJoin, KwEntry, KwExit,
    KwOn, KwAfter, KwEvery, KwDefer, KwRaise, KwSend, KwTo,
    KwIf, KwElse, KwWhile, KwFor, KwPriority, KwMs,
    // Punctuation
    Arrow, HistoryArrow, Colon, Semicolon, Comma, Dot, Eq,
    LBrace, RBrace, LParen, RParen, LBracket, RBracket,
    // Literals
    IntLiteral, BoolLiteral, StringLiteral,
    // Identifiers and specials
    Ident, AtId, DocComment, LineComment, BlockComment, Whitespace, Newline,
    // Error recovery
    Error, Eof,
}

pub struct Lexer<'src> {
    src: &'src str,
    pos: usize,
}
impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self;
    pub fn next_token(&mut self) -> Token;
    pub fn tokenize(src: &'src str) -> Vec<Token>;   // convenience
}
```

## 3.4 Error Recovery

Unknown characters emit `TokenKind::Error` and advance by one byte. Lexing always
continues — the parser handles error propagation.

---

# 4. Parser — `fsm-parser`

## 4.1 Design Philosophy

Two-phase parsing:
1. **CST (Concrete Syntax Tree)** — lossless; every token including whitespace and
   comments is stored. Used by the formatter and (as a v1.3+ aspiration —
   *not* implemented; LSP ships full re-parse, see §4.5 / Doc 00 §11.39)
   incremental reparsing for LSP.
2. **AST (Abstract Syntax Tree)** — typed, trivia-free. Used by the analyzer and
   code generators.

The CST uses a [rowan](https://github.com/rust-analyzer/rowan)-style green tree:
an arena-allocated, reference-counted, immutable tree. AST nodes are thin typed wrappers
over CST nodes, providing a typed API with no allocation.

## 4.2 Module Layout

> **DRIFT-3 — reality check.** The tree below was reconciled to the
> *actual* `crates/fsm-parser/src/` at this HEAD (verified read-only).
> The earlier listing named files that do not exist (`grammar/action.rs`,
> `cst/green.rs`, `ast/nodes.rs`, `ast/token_ext.rs`, `ast/generated.rs`,
> `error.rs`, `tests.rs`) and omitted modules that do. Notable corrections:
> the action sublanguage lives in `grammar/stmt.rs` (not `action.rs`);
> there is **no `cst/green.rs`** — the green tree comes from the upstream
> `rowan` crate, `cst/mod.rs` is the lossless-CST integration layer; the
> typed AST is a multi-file `ast/` view, not `nodes.rs`/`generated.rs`;
> and there is **no `error.rs` / `ParseError`** — parser diagnostics are
> `fsm_diagnostics::Diagnostic` (see §4.3 DRIFT-3). Recorded per Doc 00
> §11 (prose-vs-code reconciliation; cf. §11.19/§11.24/§11.39).

```
fsm-parser/src/
├── lib.rs                  # Crate root + public re-exports
├── parse.rs                # Entry points: parse / parse_with_limits / parse_with_tokens → ParseResult
├── parser.rs               # Recursive-descent driver (DepthGuard, ExprContext)
├── expr.rs                 # Pratt operator-precedence parser for FSM-Lang expressions
├── token_set.rs            # 128-bit TokenKind bitmask used by the parser
├── limits.rs               # Parser DoS limits (ParseLimits)
├── import_resolver.rs      # Compile-time validation of `import "path"`
├── opaque_type_validator.rs# Compile-time validation of `opaque "C_type"` payloads
├── grammar/
│   ├── mod.rs              # Grammar module root (one file per top-level area)
│   ├── file.rs             # Top-level grammar entry — parse_file
│   ├── top_level.rs        # imports, features, consts, enums, externs
│   ├── machine.rs          # Machine body — inside of `machine NAME { … }`
│   ├── state.rs            # State declarations and pseudo-state forms
│   ├── transition.rs       # Transition declarations (Doc 04 §8)
│   ├── guard.rs            # Guard clauses: `[ guard_expr ]`
│   └── stmt.rs             # Action sublanguage: assign, if, while, for, raise, …
├── cst/
│   ├── mod.rs              # Lossless CST built on the upstream `rowan` green-tree library
│   └── kinds.rs            # SyntaxKind — union of every token + every grammar node
└── ast/
    ├── mod.rs              # Typed AST view layered on the rowan CST
    ├── top_level.rs        # Typed accessors for top-level declarations
    ├── state.rs            # Typed accessors for state declarations / pseudo-states
    ├── transition.rs       # Typed accessors for transition / guard / priority / action-block
    ├── stmt.rs             # Typed accessors for action-sublanguage statements
    └── expr.rs             # Typed accessors for expressions + the guard-else marker
```

## 4.3 Key Types

> **DRIFT-3 — reality check.** The *shipped* `ParseResult`
> (`crates/fsm-parser/src/parse.rs:25`) holds a `rowan` `GreenNode`, not a
> materialised `SyntaxNode`, and a `Vec<fsm_diagnostics::Diagnostic>`, not
> a bespoke `ParseError`. The CST is projected lazily on demand via
> `ParseResult::syntax()` (`parse.rs:35`), `ast()` (`:41`) and
> `reconstructed_text()` (`:48`). **There is no `ParseError` type
> anywhere in `fsm-parser/src`** (verified read-only); parser, lexer and
> recovery diagnostics are all the unified `fsm_diagnostics::Diagnostic`
> (re-exported `crates/fsm-parser/src/lib.rs:45`/`:52`). The §12.2 sketch
> carries the same DRIFT-1 reality-check note. Recorded per Doc 00 §11
> (prose-vs-code reconciliation; cf. §11.19/§11.24/§11.39).

```rust
// Shipped (crates/fsm-parser/src/parse.rs:25):
pub struct ParseResult {
    pub green: GreenNode,            // rowan green tree — round-trips source
    pub errors: Vec<Diagnostic>,     // fsm_diagnostics::Diagnostic; may be
                                     // non-empty even on a recovered parse
}

impl ParseResult {
    pub fn syntax(&self) -> SyntaxNode;        // parse.rs:35 — rowan CST view
    pub fn ast(&self) -> ast::File;            // parse.rs:41 — typed AST root
    pub fn reconstructed_text(&self) -> String;// parse.rs:48 — byte-exact source
}

// Diagnostics are the unified `fsm_diagnostics::Diagnostic` (span + code +
// severity + message); there is no parser-local `ParseError` type.

// AST node example:
pub struct MachineDecl(SyntaxNode);
impl MachineDecl {
    pub fn name(&self) -> Option<&str>;
    pub fn context(&self) -> Option<ContextBlock>;
    pub fn events(&self) -> impl Iterator<Item = EventDecl>;
    pub fn states(&self) -> impl Iterator<Item = StateDecl>;
    // ...
}
```

## 4.4 Error Recovery Strategy

The parser uses **panic-mode recovery**: on an unexpected token, it skips tokens
until a synchronization point is found. Synchronization points are the start tokens
of top-level declarations: `machine`, `state`, `composite`, `parallel`, `event`,
`extern`. This allows the parser to recover and parse subsequent valid declarations
even when one is malformed.

## 4.5 Incremental Reparsing (LSP)

> **NOT IMPLEMENTED in v1.x — deferred to v1.3+ (DRIFT-1).** The
> green-tree-diff/subtree-splice scheme described in this section, and the
> `parse_incremental(old_tree, edit)` signature in §12.2, **do not exist in
> code**. The shipped `fsm-parser` does a **full re-parse** only:
> `parse` / `parse_with_limits` / `parse_with_tokens` (`crates/fsm-parser/
> src/lib.rs:52`); all three drive the *whole* grammar via
> `grammar::parse_file` over the entire token vector (`crates/fsm-parser/
> src/parse.rs:59`/`:67`/`:94`, `grammar/file.rs:50`). `parse_with_tokens`
> only skips **re-lexing** untouched ranges — it still re-parses the full
> grammar; there is no `parse_incremental`, no CST diff, no subtree reuse.
> The v1.2 LSP epic **deliberately built on full re-parse-on-debounce**
> (Doc 26 §4.3–§4.5; the L1–L6 waves do not depend on `parse_incremental`):
> a full re-parse + re-analyze of a realistic embedded `.fsm` is
> sub-millisecond to low-single-digit-ms, comfortably inside the 200ms
> change-debounce, so the 50ms target below is met *without* incremental
> parsing. True incremental CST patching is a tracked **v1.3+
> optimisation** for pathologically large files — a separate, self-contained
> `fsm-parser` epic with its own behavioural acceptance — **not a current
> capability**. Recorded per Doc 00 §11.39 (the prose-vs-code / P0-1
> "aspirational-prose" reconciliation discipline; cf. §11.19/§11.24); the
> design intent below is preserved as the v1.3+ target, not as shipped
> behaviour.

For LSP `textDocument/didChange` events, the CST supports incremental reparsing:
the changed range is re-tokenized, the affected CST subtree is replaced, and only
the changed portion of the AST is rebuilt. This keeps LSP response times under 50ms
for typical file sizes.

---

# 5. Analyzer — `fsm-analyzer`

## 5.1 Design Philosophy

The analyzer runs in two sequential phases. Each phase has a single responsibility and
produces a clean output type.

## 5.2 Module Layout

> **DRIFT-4 — reality check.** The tree below was reconciled to the
> *actual* `crates/fsm-analyzer/src/` at this HEAD (verified read-only,
> `find crates/fsm-analyzer/src -name '*.rs'`). DRIFT-3 corrected the
> §4.2 parser layout and the §12.3 analyzer API and flagged that this
> §5.2/§5.3 analyzer listing was stale the same way; this is that fix.
> The earlier listing named a **two-`phase{1,2}_*` directory structure
> that does not exist** (`phase1_structure/{symbol_table,builder,resolver}`,
> `phase2_semantic/{type_checker,determinism,reachability,history,submachine}`)
> plus `diagnostics.rs`, `ir_builder.rs` and `tests.rs` — none of which
> are present. The shipped analyzer is **flat**: a `SymbolTable` build
> pass (`symbol_table.rs`), a `checks/` family of semantic passes, and a
> `lower/` AST→IR lowering tree (AD-3, 2026-05-15, split out of the
> former `lower.rs`/`LoweringCtx` god-object). There is **no
> `DiagnosticAccumulator`** — diagnostics are a plain
> `Vec<fsm_diagnostics::Diagnostic>` threaded through the pipeline (see
> §5.3 DRIFT-4); IR-building is the `lower/` tree, not an `ir_builder.rs`.
> The two-phase split described in §5.1/§5.4/§5.5 survives as the
> *conceptual* model (symbol-build then checks then lower), not as an
> on-disk `phase{1,2}_*` directory layout. Recorded per Doc 00 §11
> (prose-vs-code reconciliation; cf. §11.19/§11.24/§11.39).

```
fsm-analyzer/src/
├── lib.rs              # Crate root + public re-exports: analyze / analyze_with_source / AnalysisResult / SymbolTable
├── symbol_table.rs     # First pass: collect every named entity per machine (FSM-E0020..E0024 duplicate detection)
├── scope.rs            # Lexical scope stack consulted during name resolution / checks
├── lca.rs              # Lowest-Common-Ancestor utilities (Doc 00 §7.7 B-09) — incl. external-self-transition LCA corner case
├── util.rs             # Small utilities shared across the analyzer pipeline
├── checks/             # Semantic-check family — one module per check, run in dependency order
│   ├── mod.rs          # Check orchestrator: name resolution first, then the rest
│   ├── name_resolution.rs # Every reference resolves to an in-scope symbol (FSM-E0100..E0109)
│   ├── type_check.rs   # Lightweight type checks: guards boolean, literals fit, assignments agree
│   ├── determinism.rs  # Same-(source,event) transitions must be statically exclusive (FSM-E0300)
│   ├── completion.rs   # Completion-transition semantics (Doc 00 §7.5 B-07): FSM-E0401 / FSM-W0101
│   ├── history.rs      # shallow_history / deep_history must carry `default ->` (FSM-E0111)
│   ├── timer.rs        # Timer durations: 0/negative → FSM-E0410, >24h → FSM-W0601
│   ├── parallel.rs     # Parallel-region structure: FSM-E0600 / FSM-H0004 / FSM-W0600
│   ├── submachine.rs   # Submachine reference checks over real SUBMACHINE_DECL / SUBMACHINE_REF nodes
│   ├── defer.rs        # defer+explicit-on conflict (FSM-E0310); v1.0 unsupported-defer FSM-E0903
│   └── import.rs       # Import sanity — parser already enforces G-02 traversal; light for v1.0
└── lower/              # AST → IR lowering (AD-3: split out of the former LoweringCtx god-object)
    ├── mod.rs          # Lowering entry point + `analyze` / `analyze_with_source` / `AnalysisResult`
    ├── ids.rs          # Monotonic ID factory — the only mutable participant in lowering
    ├── loc.rs          # Immutable source-location helpers (file / src → SourceLocation)
    ├── machine.rs      # Machine-level lowering: context fields, events, externs, consts, queue, types
    ├── state.rs        # State-tree lowering: states, regions, pseudo-states, transitions, timers, defers
    ├── expr.rs         # Action / guard sub-language lowering → IR Statement / GuardExpr / IrExpr
    └── hash.rs         # `sourceHash` label + a tiny dep-free in-tree SHA-256 (RFC 6234)
```

## 5.3 Key Types

> **DRIFT-4 — reality check.** This block is an independent stale copy of
> `AnalysisResult` (DRIFT-3 already corrected the *§12.3* copy; §5.3 is a
> separate one this fix reconciles). The *shipped* `AnalysisResult`
> (`crates/fsm-analyzer/src/lower/mod.rs:59`, re-exported
> `crates/fsm-analyzer/src/lib.rs:37`) carries `ir: Option<Ir>` (an
> `fsm_ir::Ir`, not an `IrDocument`; `None` only on catastrophic errors —
> partial IR is preferred) and `diagnostics: Vec<Diagnostic>` (the
> unified `fsm_diagnostics::Diagnostic`, not a bespoke
> `AnalysisDiagnostic`). **There is no `IrDocument`, `AnalysisDiagnostic`,
> `ScopeId`, `ScopeKind`, `SymbolId`, or `TypeEnv` type anywhere in
> `crates/fsm-analyzer/src`** (verified read-only). The shipped
> `SymbolTable` (`crates/fsm-analyzer/src/symbol_table.rs:155`) is **not**
> a `HashMap<ScopeId, Scope>` — it is a flat `Vec<MachineSymbols>` (one
> per declared machine) plus file-level decl vectors and name→index
> lookups; duplicate-name detection uses those ordered tables, not a
> scope map. `Scope` (`crates/fsm-analyzer/src/scope.rs:12`) is the
> lexical-scope-stack cursor — `{ machine_idx: Option<usize>,
> container_path: Vec<String> }` — not a `{ kind, symbols, parent }`
> node; it identifies *which container declared a name* and is consulted
> by `SymbolTable` during resolution, it does not itself store symbols.
> Type information is checked structurally by `checks/type_check.rs`
> (conservative v1.0 scope, no HM-style `TypeEnv`). The original block is
> retained below only as the historical design sketch; the shipped shapes
> are authoritative. Recorded per Doc 00 §11 (prose-vs-code
> reconciliation; cf. §11.19/§11.24/§11.39).

```rust
// Shipped (crates/fsm-analyzer/src/lower/mod.rs:59; re-export lib.rs:37):
pub struct AnalysisResult {
    pub ir: Option<Ir>,                 // fsm_ir::Ir; None only on
                                        // catastrophic errors (partial IR preferred)
    pub diagnostics: Vec<Diagnostic>,   // fsm_diagnostics::Diagnostic — analyzer +
                                        // lowerer + parser, in source-emission order
    pub symbol_table: SymbolTable,      // exposed for downstream re-use (LSP, docs)
}

// Shipped (crates/fsm-analyzer/src/symbol_table.rs:155):
pub struct SymbolTable {
    pub machines: Vec<MachineSymbols>,  // one per declared `machine`
    pub file_consts: Vec<Entry>,        // decls outside any machine block
    pub file_enums: Vec<EnumEntry>,
    pub file_externs: Vec<Entry>,
    pub machine_index: HashMap<String, usize>,    // name → index in `machines`
    pub submachines: Vec<SubmachineEntry>,        // `submachine NAME { … }` templates
    pub submachine_index: HashMap<String, usize>,
    // (plus file_extern_pure — see source for the full set)
}

// Shipped (crates/fsm-analyzer/src/scope.rs:12) — a scope-stack CURSOR,
// not a symbol-holding node; consulted by SymbolTable during resolution:
pub struct Scope {
    pub machine_idx: Option<usize>,     // index into SymbolTable::machines
    pub container_path: Vec<String>,    // state/region IDs machine-root → here;
                                        // last element is the immediate parent
}

// There is no separate `TypeEnv`; v1.0 type checking is structural and
// lives in `checks::type_check` (guards boolean, literals fit, assignments
// agree) — not a Hindley-Milner environment.
```

> **DRIFT-5 — reality check (§5.4 + §5.5 phase-narrative).** DRIFT-4
> reconciled the §5.2/§5.3 *layout/types* and noted the §5.1/§5.4/§5.5
> phase-narrative survives only as the *conceptual* model, not an on-disk
> `phase{1,2}_*` split; this is that follow-up, plus the stale per-step
> code claims (verified read-only against the analyzer source at this
> HEAD, annotate-not-delete; cf. §11.45/§11.46 and Doc 00 §11.47). The
> **shipped pipeline is a single pass**, not a strict two-phase one:
> `SymbolTable::build` (`crates/fsm-analyzer/src/symbol_table.rs:179`,
> emitting the duplicate-name FSM-E0020..E0024/E0025) → `checks::run_all`
> (`crates/fsm-analyzer/src/checks/mod.rs` — name resolution first, then
> the `checks/*` family in dependency order) → `lower_file`
> (`crates/fsm-analyzer/src/lower/mod.rs:153`, AST→IR). The §5.4/§5.5
> step-list below is the *design intent / conceptual grouping*, kept for
> orientation; **two specific per-step claims are stale**:
> - **§5.5 step 2 does NOT emit `FSM-E0301`.** Per Doc 00 **B-07** (and
>   Doc 00 G-11) guarded completion transitions are *explicitly allowed*;
>   `crates/fsm-analyzer/src/checks/completion.rs` states verbatim "the
>   analyzer DOES NOT emit FSM-E0301" — that module instead emits
>   `FSM-E0401` (composite external self-transition) and `FSM-W0101`
>   (dead completion transition). Determinism (`checks/determinism.rs`)
>   emits `FSM-E0300`/`W0300` as described; only the E0301 implication is
>   wrong.
> - **§5.5 step 3 (forward reachability → `FSM-E0400`) is NOT
>   implemented.** `FSM-E0400` ("unreachable state") is catalogued in
>   `crates/fsm-diagnostics/src/lib.rs` but has **zero emission sites**
>   in `crates/fsm-analyzer/` and is `UNTESTED` in
>   `tests/conformance/COVERAGE_MAP.md` (there is no `checks/reachability`
>   module; `util.rs`'s "reachable" helper is just an AST state iterator,
>   not a reachability analysis). It is a documented-but-unimplemented
>   step, recorded here (the prose-vs-code / P0-1 honesty discipline) —
>   not silently presented as shipped. Step 1 (type checking,
>   `checks/type_check.rs`) and step 4 (history/fork/join invariants,
>   `checks/history.rs` + `checks/parallel.rs`) are accurate.

## 5.4 Phase 1 — Structural Validation

Walk the AST in declaration order. For each declaration:
1. Check for duplicate names in the current scope → FSM-E0020–E0025.
2. Register the symbol in the current scope's symbol table.
3. After full walk, resolve all name references (transition targets, event names,
   extern names) → FSM-E0100–E0109.

## 5.5 Phase 2 — Semantic Validation

1. **Type checking.** For each guard expression and action statement, walk the expression
   tree and infer types bottom-up. Report FSM-E0200–E0205 on mismatches.

2. **Determinism analysis.** For each state, collect all outgoing transitions per event.
   Build a transition interference graph: two transitions interfere if their guard
   conditions are not provably mutually exclusive. The solver uses a conservative
   static analysis (not a full SMT solver). Report FSM-E0300 if interference is detected
   and no priority resolution is in place.

3. **Reachability.** Run a forward reachability analysis from the initial state.
   States not reachable from any configuration are FSM-E0400.

4. **History/join invariants.** Verify history targets exist; verify join sources are
   all in the same parallel state's regions; verify fork targets are all initial states
   of distinct regions.

---

# 6. IR Crate — `fsm-ir`

## 6.1 Design Philosophy

Pure data types. No business logic. Serde-derived JSON serialization. Visitor API for
toolchain components to traverse the IR without pattern matching.

## 6.2 Module Layout

```
fsm-ir/src/
├── lib.rs          # Re-exports all public types
├── model.rs        # All IR structs and enums (mirrors FSM-SPEC-IR §3–§16)
├── builder.rs      # MachineBuilder, StateBuilder — fluent construction API
├── visitor.rs      # IrVisitor trait with default no-op implementations
├── stable_id.rs    # Stable ID generation algorithm
├── schema.rs       # Embeds schema/ir/1.0.0/model.json for validation
└── tests.rs
```

## 6.3 Visitor Pattern

```rust
pub trait IrVisitor {
    fn visit_machine(&mut self, m: &MachineObject) { self.walk_machine(m); }
    fn visit_state(&mut self, s: &StateNode)        { self.walk_state(s); }
    fn visit_transition(&mut self, t: &TransitionObject) {}
    fn visit_guard(&mut self, g: &GuardExpr)        {}
    fn visit_statement(&mut self, s: &Statement)    {}
    fn visit_expr(&mut self, e: &Expr)              {}
    // walk_* methods provide default traversal
    fn walk_machine(&mut self, m: &MachineObject)   { /* visits all children */ }
    // ...
}
```

Code generators, analyzers, and the formatter all implement `IrVisitor` to traverse
the IR without knowing its internal structure.

---

# 7. C99 Code Generator — `fsm-codegen-c`

## 7.1 Design Philosophy

Template-free. All output is produced via direct `write!()` calls into an
`IndentWriter`. Two strategies share the same planning phase but use different emission
modules.

## 7.2 Module Layout

```
fsm-codegen-c/src/
├── lib.rs              # Public API: generate(ir, config) → EmittedFiles
├── plan.rs             # CodegenPlan: pre-computes naming, indices, orderings
├── naming.rs           # All C identifier generation (slugify, prefix, etc.)
├── emit/
│   ├── mod.rs          # EmissionContext, IndentWriter
│   ├── header.rs       # Motor.h emission
│   ├── source.rs       # Motor.c emission (dispatcher, entry/exit fns)
│   ├── impl_header.rs  # Motor_impl.h emission (extern declarations)
│   └── conf_header.rs  # Motor_conf.h emission (macros)
├── strategy/
│   ├── switch.rs       # Switch-based dispatch emission
│   └── table.rs        # Table-driven dispatch emission
└── tests.rs
```

## 7.3 CodegenPlan

```rust
pub struct CodegenPlan {
    pub machine_name: String,
    pub prefix: String,                 // e.g., "Motor"
    pub states: Vec<StateRecord>,
    pub events: Vec<EventRecord>,
    pub transitions: Vec<TransitionRecord>,
    pub timers: Vec<TimerRecord>,
    pub context_fields: Vec<FieldRecord>,
    pub strategy: CodegenStrategy,
}

pub struct StateRecord {
    pub id: String,          // IR node ID
    pub c_enum_name: String, // e.g., "MOTOR_STATE_IDLE"
    pub c_entry_fn: String,  // e.g., "Motor_entry_Idle"
    pub c_exit_fn: String,   // e.g., "Motor_exit_Idle"
    pub depth: usize,        // nesting depth (for LCA computation)
    pub parent_idx: Option<usize>,
}
```

The plan is computed in a single pass over the IR before any emission begins.
This separates naming concerns from output concerns.

## 7.4 IndentWriter

```rust
pub struct IndentWriter<W: Write> {
    inner: W,
    indent: usize,
    indent_str: &'static str,   // "    " (4 spaces)
    at_line_start: bool,
}
impl<W: Write> IndentWriter<W> {
    pub fn indent(&mut self);
    pub fn dedent(&mut self);
    pub fn writeln(&mut self, s: &str);
}
```

---

# 8. Simulator — `fsm-simulator`

## 8.1 Design Philosophy

Pure Rust interpreter of the IR. Does NOT execute generated C code. Implements
FSM-SPEC-SEM semantics directly. Two modes:
- **Embedded mode** — used by the VS Code extension via the `fsm-lsp` process (in-process).
- **Daemon mode** — standalone process exposing the WebSocket JSON-RPC API (FSM-SPEC-SIM).

## 8.2 Module Layout

```
fsm-simulator/src/
├── lib.rs              # Public API
├── interpreter.rs      # Core RTC step engine — implements FSM-SPEC-SEM
├── configuration.rs    # ActiveConfiguration — HashSet<StateId> + helpers
├── context.rs          # ContextValues — HashMap<FieldName, JsonValue>
├── queue.rs            # EventQueue — internal + external ring buffers
├── clock.rs            # VirtualClock | WallClock (trait)
├── trace.rs            # TraceRecorder — collects StepRecord[]
├── breakpoints.rs      # BreakpointRegistry
├── timer.rs            # TimerRegistry — tracks active timers, fires on tick
├── ws_server.rs        # tokio + tungstenite WebSocket server
└── rpc/
    ├── mod.rs          # JSON-RPC 2.0 dispatcher
    ├── methods.rs      # sim/load, sim/dispatch, sim/init, etc.
    └── notifications.rs # sim/stateChanged, sim/timerFired, etc.
```

## 8.3 Interpreter Core

```rust
pub struct Interpreter {
    machine: Arc<MachineObject>,   // from fsm-ir
    config: ActiveConfiguration,
    context: ContextValues,
    internal_queue: EventQueue,
    external_queue: EventQueue,
    defer_set: Vec<EventId>,
    timer_reg: TimerRegistry,
    trace: TraceRecorder,
    breakpoints: BreakpointRegistry,
    loop_counter: usize,            // completion event chain depth (FSM-E0900)
}

impl Interpreter {
    pub fn init(&mut self, initial_context: Option<ContextValues>) -> StepResult;
    pub fn dispatch(&mut self, event: SimEvent) -> Vec<StepRecord>;
    pub fn tick(&mut self, elapsed_ms: u32) -> Vec<StepRecord>;
    fn rtc_step(&mut self, event: SimEvent) -> StepRecord;
    fn select_transitions(&self, event: &SimEvent) -> Vec<&TransitionObject>;
    fn compute_lca(&self, source: &StateId, target: &StateId) -> StateId;
    fn execute_exit_sequence(&mut self, exit_set: &[StateId]);
    fn execute_entry_sequence(&mut self, entry_set: &[StateId]);
    fn handle_completion(&mut self);
}
```

## 8.4 WebSocket Server Architecture

```
tokio runtime
    │
    ├── WsServer task (accepts connections)
    │       │
    │       └── per-connection task
    │               │
    │               ├── recv loop → JSON-RPC dispatch → method handler
    │               └── notification sender (mpsc channel)
    │
    └── InterpreterManager
            └── HashMap<InstanceId, Interpreter>
```

The `InterpreterManager` is behind an `Arc<Mutex<>>`. Method handlers lock it,
call interpreter methods, release lock, then serialize and send responses.

---

# 9. LSP Server — `fsm-lsp`

## 9.1 Design Philosophy

Built on `tower-lsp`. All computation is async. Document analysis is debounced (200ms)
to avoid thrashing during fast typing. The LSP server holds a `DocumentStore` that maps
URI → analyzed document.

## 9.2 Module Layout

```
fsm-lsp/src/
├── lib.rs              # Public API + server entry point
├── server.rs           # Backend struct implementing tower_lsp::LanguageServer
├── document_store.rs   # DocumentStore — stores and retrieves analyzed documents
├── analysis.rs         # Triggers pipeline for a document, debounced
├── capabilities/
│   ├── completion.rs   # textDocument/completion handler
│   ├── hover.rs        # textDocument/hover handler
│   ├── goto_def.rs     # textDocument/definition handler
│   ├── references.rs   # textDocument/references handler
│   ├── rename.rs       # textDocument/rename + prepareRename
│   ├── code_action.rs  # textDocument/codeAction handler
│   ├── inlay_hints.rs  # textDocument/inlayHint handler
│   ├── sem_tokens.rs   # textDocument/semanticTokens handler
│   ├── folding.rs      # textDocument/foldingRange handler
│   └── doc_symbols.rs  # textDocument/documentSymbol handler
└── tests.rs
```

## 9.3 DocumentStore

```rust
pub struct AnalyzedDocument {
    pub uri: Url,
    pub source: String,
    pub version: i32,
    pub cst: SyntaxNode,
    pub ast: Vec<MachineDecl>,
    pub ir: IrDocument,
    pub symbol_table: SymbolTable,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub analyzed_at: Instant,
}

pub struct DocumentStore {
    docs: HashMap<Url, Arc<RwLock<AnalyzedDocument>>>,
    analysis_debouncer: Debouncer,
}
```

## 9.4 Request Handling Pattern

Each capability handler:
1. Receives LSP request parameters.
2. Locks the document read-lock from `DocumentStore`.
3. Walks the CST or IR to find the relevant node at the cursor position.
4. Constructs and returns the LSP response.

No handler re-analyzes the document; analysis is done separately by the debouncer.

---

# 10. VS Code Extension — `editors/vscode/`

## 10.1 Directory Layout

```
editors/vscode/
├── src/
│   ├── extension.ts        # Activation / deactivation; LanguageClient setup
│   ├── lsp-client.ts       # Wraps vscode-languageclient with FSM middleware
│   ├── commands.ts         # All palette command registrations
│   ├── statusbar.ts        # Status bar items
│   ├── diagram/
│   │   ├── DiagramPanel.ts # WebView panel lifecycle management
│   │   └── renderer/       # Runs inside WebView (bundled separately)
│   │       ├── index.ts    # Entry point
│   │       ├── elk.ts      # ELK layout integration
│   │       └── svg.ts      # SVG diagram rendering
│   └── simulator/
│       ├── SimulatorPanel.ts   # Simulator WebView lifecycle
│       ├── SimulatorClient.ts  # WebSocket JSON-RPC client (FSM-SPEC-SIM)
│       └── renderer/           # Simulator UI (runs in WebView)
├── package.json            # Extension manifest, grammar contribution, commands
├── syntaxes/
│   └── fsm-lang.tmLanguage.json # TextMate grammar for syntax highlighting
└── tsconfig.json
```

## 10.2 Communication Patterns

**Extension process ↔ WebView:**
```
DiagramPanel.ts        WebView (renderer/index.ts)
     │                         │
     │  postMessage({type:'ir', payload: IrDocument})
     │ ──────────────────────────►
     │                         │ layout + render SVG
     │  postMessage({type:'stateClick', stateId})
     │ ◄──────────────────────────
     │
     │ → command: fsm.gotoDefinition(stateId)
```

**Extension process ↔ Simulator daemon:**
```
SimulatorClient.ts   fsm-simulator (daemon process)
       │                     │
       │  WebSocket connect  │
       │ ──────────────────► │
       │  sim/load (IR)      │
       │ ──────────────────► │
       │  sim/dispatch(ev)   │
       │ ──────────────────► │
       │  sim/stateChanged   │
       │ ◄────────────────── │
```

## 10.3 Extension State

The extension process owns all state. The WebView is stateless and purely presentational
— it renders whatever the extension sends. This simplifies WebView reloads and avoids
state desync.

---

# 11. Web IDE — `editors/web-ide/`

## 11.1 Directory Layout

```
editors/web-ide/
├── src/
│   ├── main.tsx            # React entry point
│   ├── App.tsx             # Root layout
│   ├── store/
│   │   ├── fileStore.ts    # Zustand: open files, active file
│   │   ├── sessionStore.ts # Zustand: compiler output, IR, diagnostics
│   │   └── simStore.ts     # Zustand: simulator state
│   ├── workers/
│   │   ├── CompilerWorker.ts  # Web Worker: calls WASM compiler
│   │   └── LspWorker.ts       # Web Worker: in-process LSP
│   ├── editor/
│   │   ├── Editor.tsx      # Monaco Editor wrapper
│   │   ├── FsmLanguage.ts  # Monaco language registration
│   │   └── LspBridge.ts    # Bridges Monaco events to LspWorker
│   ├── diagram/            # Same renderer package as VS Code WebView
│   ├── simulator/
│   │   ├── Simulator.tsx   # Simulator panel UI
│   │   └── InProcessSim.ts # Direct call to WASM interpreter (no WebSocket)
│   └── components/         # Shared UI components
├── public/
│   └── fsm_compiler.wasm   # Compiled WASM binary
├── vite.config.ts
└── package.json
```

## 11.2 Worker Architecture

All heavy computation runs off the main thread:

```
Main thread (React UI)
       │
       ├── CompilerWorker (Web Worker)
       │     WASM compiler
       │     Input: source text
       │     Output: IR document + diagnostics
       │
       └── LspWorker (Web Worker)
             in-process LSP (Rust → WASM)
             Input: LSP requests (hover, completion, etc.)
             Output: LSP responses
```

Workers communicate via `postMessage`/`onmessage`. The Zustand stores are updated
from worker responses via the main thread.

## 11.3 In-Process Simulator

Unlike the VS Code extension (which connects to a daemon via WebSocket), the Web IDE
uses the WASM-compiled simulator directly in a Web Worker:

```typescript
// InProcessSim.ts (runs in Web Worker)
import { WasmSimulator } from '../wasm-bridge';

const sim = new WasmSimulator();
sim.load(irJson);
sim.init({});

function dispatch(eventName: string, payload: object) {
    const result = sim.dispatch(eventName, payload);
    self.postMessage({ type: 'stepResult', result });
}
```

This means the Web IDE has zero backend dependency — it runs fully in the browser.

---

# 11.5 Crate Dependency Graph

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §11.1; see CHANGELOG._
>
> The foundation crate `fsm-diagnostics` (zero workspace deps; owns `Span`,
> `SourceLocation`, `Severity`, `DiagnosticCode` — 75 variants *at Wave
> 1.0 / `db5ef83`* — and `Diagnostic`) was added in Wave 1.0 / commit
> `db5ef83` to break cyclic dep risk. Every pipeline crate consumes it
> instead of redefining diagnostic types locally (rust-analyzer / Roslyn
> precedent). _(Forward note: the **live** `DiagnosticCode` count is now
> **73** — `FSM-E0903` retired v1.1, `FSM-W0500` retired v1.2 to
> `deprecated::DeprecatedCode`; the historical "75 at `db5ef83`" above is
> left intact as a pinned statement. See `tests/conformance/COVERAGE_MAP.md`
> + Doc 00 §11.47; the `all_codes_matches_expected_count` lock test is the
> live source of truth.)_
>
> v1.0 ships **9 crates** (added `fsm-diagnostics`). The crates
> `fsm-lsp`, `fsm-codegen-cpp`, and `fsm-wasm` previously listed are
> **not present in v1.0** — LSP/C++17/WASM are deferred to v1.1 per Doc 00
> §6 D-01..D-05.

```
fsm-diagnostics    (no workspace deps; owns Span, SourceLocation,
                    Severity, DiagnosticCode, Diagnostic)
        │
        ├──► fsm-lexer        (depends on: fsm-diagnostics)
        │
        ├──► fsm-parser       (depends on: fsm-diagnostics, fsm-lexer)
        │
        ├──► fsm-ir           (depends on: fsm-diagnostics, serde, serde_json)
        │
        ├──► fsm-analyzer     (depends on: fsm-diagnostics, fsm-parser, fsm-ir)
        │
        ├──► fsm-codegen-c    (depends on: fsm-diagnostics, fsm-ir)
        │
        ├──► fsm-simulator    (depends on: fsm-diagnostics, fsm-ir,
        │                                  fsm-analyzer)
        │
        ├──► fsm-formatter    (depends on: fsm-diagnostics, fsm-lexer,
        │                                  fsm-parser)
        │
        └──► fsm-cli          (binary; depends on: ALL crates above)

Not in v1.0 (deferred):
  - fsm-codegen-cpp   (v1.1 — Doc 12 deferred)
  - fsm-lsp           (v1.1 — Doc 14 deferred)
  - fsm-wasm          (v1.1 — Web IDE deferred)
  - fsm-rpc-server    (v1.1 — WebSocket simulator deferred)
```

**Key rules:**
- `fsm-diagnostics` is the foundation crate; everyone else consumes it for
  `Span`, `SourceLocation`, and `Diagnostic`. No crate redefines these
  types locally.
- `fsm-codegen-c` depends on `fsm-ir` only (not on parser or analyzer).
- `fsm-simulator` depends on `fsm-ir` and `fsm-analyzer` (for semantic
  validation at load time).
- `fsm-formatter` depends only on the CST (lexer + parser); it does NOT
  need analyzer or IR.

**Build system: Cargo**
```bash
# Build all crates (9 crates ship in v1.0)
cargo build --workspace

# Run the full test suite
cargo test --workspace

# Clippy / fmt gates
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

# 12. Public API Surface — Per-Crate Contracts

Each crate exposes a minimal public API. Crates MUST NOT depend on types from
other crates beyond what is listed in the dependency graph (§11.5).

## 12.1 `fsm-lexer` Public API

```rust
// Re-exported types
pub struct Span { pub start: usize, pub end: usize }
pub struct Token { pub kind: TokenKind, pub span: Span }
pub enum TokenKind { /* ~60 variants — see §3.3 */ }

// Public functions
pub struct Lexer<'src> { /* ... */ }
impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self;
    pub fn next_token(&mut self) -> Token;
    pub fn tokenize(src: &'src str) -> Vec<Token>;
}
```

## 12.2 `fsm-parser` Public API

> **DRIFT-1 — reality check.** The *shipped* `fsm-parser` public API is
> `parse` / `parse_with_limits` / `parse_with_tokens` returning a
> `ParseResult { green, errors }` (`crates/fsm-parser/src/lib.rs:52`,
> `parse.rs:59`/`:67`/`:94`). `parse_incremental` below **does not exist in
> v1.x** — it is a deferred v1.3+ aspiration (full re-parse only today; see
> §4.5 and Doc 00 §11.39). The signature is kept as the future-API sketch,
> not as a present contract.

```rust
// Shipped (crates/fsm-parser/src/parse.rs:59/:67/:94):
pub fn parse(src: &str) -> ParseResult;
pub fn parse_with_limits(src: &str, limits: &ParseLimits) -> ParseResult;
pub fn parse_with_tokens(src: &str, tokens: Vec<Token>) -> ParseResult;

// v1.3+ aspiration — NOT implemented (see §4.5 / Doc 00 §11.39):
pub fn parse_incremental(old_tree: &SyntaxNode, edit: &TextEdit) -> ParseResult;

// Shipped shape (crates/fsm-parser/src/parse.rs:25) — DRIFT-3, see §4.3:
pub struct ParseResult {
    pub green: GreenNode,            // rowan green tree
    pub errors: Vec<Diagnostic>,     // fsm_diagnostics::Diagnostic; no ParseError type
}
// Projections: ParseResult::syntax() / ast() / reconstructed_text()
// (parse.rs:35/:41/:48).

// AST node types (thin wrappers over CST nodes)
pub struct MachineDecl(SyntaxNode);
pub struct StateDecl(SyntaxNode);
pub struct EventDecl(SyntaxNode);
pub struct ExternDecl(SyntaxNode);
pub struct TransitionDecl(SyntaxNode);
pub struct GuardExpr(SyntaxNode);
pub struct ActionList(SyntaxNode);
pub struct ContextBlock(SyntaxNode);
pub struct RegionDecl(SyntaxNode);
// ... (one per grammar rule)
```

## 12.3 `fsm-analyzer` Public API

> **DRIFT-3 — reality check.** The *shipped* `fsm-analyzer` public API is
> `analyze` / `analyze_with_source`, both taking a `&ParseResult` (not a
> `&[MachineDecl]`) and returning an `AnalysisResult { ir: Option<Ir>,
> diagnostics: Vec<Diagnostic>, symbol_table: SymbolTable }`
> (`crates/fsm-analyzer/src/lower/mod.rs:59`/`:72`/`:79`, re-exported
> `crates/fsm-analyzer/src/lib.rs:37`). `analyze_incremental` below **does
> not exist in v1.x** — it is the analyzer-side analogue of the deferred
> `parse_incremental` (§4.5 / §12.2 / DRIFT-1) and is itself a deferred
> v1.3+ aspiration. The shipped analyzer does a **full re-analysis** on
> every call: `analyze_with_source` rebuilds the symbol table
> (`SymbolTable::build`, `lower/mod.rs:82`), re-runs every check
> (`checks::run_all`, `:86`) and re-lowers the whole AST (`lower_file`,
> `:89`) from a fresh `ParseResult` — there is no `prev`/`changed` reuse,
> no incremental diff. The v1.2 LSP epic **deliberately built on full
> re-analyze-on-debounce** (Doc 26 §4.3–§4.5; cross-ref §4.5 / Doc 00
> §11.39): a full re-parse + re-analyze of a realistic embedded `.fsm` is
> sub-millisecond to low-single-digit-ms, comfortably inside the change
> debounce, so no LSP wave depends on `analyze_incremental`. True
> incremental re-analysis is a tracked **v1.3+ optimisation** for
> pathologically large files — a separate, self-contained `fsm-analyzer`
> epic with its own behavioural acceptance — **not a current capability**.
> Recorded per Doc 00 §11 (the prose-vs-code / P0-1 "aspirational-prose"
> reconciliation discipline; cf. §11.19/§11.24/§11.39); the design intent
> below is preserved as the v1.3+ target, not as shipped behaviour.

```rust
// Shipped (crates/fsm-analyzer/src/lower/mod.rs:72/:79):
pub fn analyze(parse_result: &ParseResult) -> AnalysisResult;
pub fn analyze_with_source(
    parse_result: &ParseResult, file: &str, src: &str
) -> AnalysisResult;

// v1.3+ aspiration — NOT implemented (see §4.5 / Doc 00 §11.39):
// full re-analysis only today; no incremental diff.
pub fn analyze_incremental(
    prev: &AnalysisResult, changed: &MachineDecl
) -> AnalysisResult;

// Shipped shape (crates/fsm-analyzer/src/lower/mod.rs:59):
pub struct AnalysisResult {
    pub ir: Option<Ir>,                 // None only on catastrophic errors
    pub diagnostics: Vec<Diagnostic>,   // fsm_diagnostics::Diagnostic
    pub symbol_table: SymbolTable,
}
```

## 12.4 `fsm-ir` Public API

```rust
// Types
pub struct IrDocument { pub version: String, pub machines: Vec<MachineObject> }
pub struct MachineObject { pub id: String, pub name: String, /* ... */ }
pub struct StateNode { pub id: String, pub name: String, pub kind: StateKind, /* ... */ }
pub struct TransitionObject { pub id: String, pub source: String, pub target: String, /* ... */ }
pub enum StateKind { Simple, Composite, Parallel, Final, Choice, Junction, Fork, Join }

// Serialization
pub fn to_json(doc: &IrDocument) -> Result<String, IrError>;
pub fn from_json(json: &str) -> Result<IrDocument, IrError>;

// Builder API
pub struct MachineBuilder { /* ... */ }
impl MachineBuilder {
    pub fn new(name: &str) -> Self;
    pub fn add_state(&mut self, state: StateNode) -> &mut Self;
    pub fn add_transition(&mut self, trans: TransitionObject) -> &mut Self;
    pub fn build(self) -> MachineObject;
}

// Visitor — see §12.8
pub trait IrVisitor { /* ... */ }
```

## 12.5 `fsm-codegen-c` Public API

```rust
pub fn generate(ir: &IrDocument, config: &CodegenConfig) -> Result<EmittedFiles, CodegenError>;

pub struct CodegenConfig {
    pub strategy: CodegenStrategy,
    pub queue_capacity: u8,
    pub queue_overflow: OverflowPolicy,
    pub isr_safe: bool,
}

pub struct EmittedFiles {
    pub header: String,       // Motor.h content
    pub source: String,       // Motor.c content
    pub impl_header: String,  // Motor_impl.h content
    pub conf_header: String,  // Motor_conf.h content
}
```

## 12.6 `fsm-codegen-cpp` Public API

```rust
pub fn generate(ir: &IrDocument, config: &CppCodegenConfig) -> Result<EmittedFiles, CodegenError>;

pub struct CppCodegenConfig {
    pub strategy: CodegenStrategy,
    pub impl_style: ImplStyle,       // Crtp | VtableClassic
    pub stl_profile: StlProfile,     // NoStl | UseStl
    pub queue_capacity: u8,
    pub queue_overflow: OverflowPolicy,
    pub isr_safe: bool,
}
```

## 12.7 `fsm-simulator` Public API

```rust
pub struct Interpreter { /* ... */ }
impl Interpreter {
    pub fn new(machine: &MachineObject) -> Self;
    pub fn init(&mut self, context: Option<ContextValues>) -> StepResult;
    pub fn dispatch(&mut self, event: SimEvent) -> Vec<StepRecord>;
    pub fn tick(&mut self, elapsed_ms: u32) -> Vec<StepRecord>;
    pub fn configuration(&self) -> &ActiveConfiguration;
    pub fn context(&self) -> &ContextValues;
    pub fn set_context_field(&mut self, name: &str, value: JsonValue) -> Result<(), SimError>;
    pub fn set_breakpoint(&mut self, bp: Breakpoint) -> BreakpointId;
    pub fn clear_breakpoint(&mut self, id: BreakpointId);
    pub fn trace(&self) -> &[StepRecord];
}
```

## 12.8 `fsm-ir` Visitor Pattern

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §8 (Doc 20 §12.8
> patch) and §B-06 (new IR types); see CHANGELOG._

The `IrVisitor` trait provides a default traversal of the IR tree.
Implementations override specific `visit_*` methods. Crucially: `walk_machine`
descends into `machine.root` (a `RegionObject`) — NOT a flat `m.states` /
`m.transitions` list. The Doc 09 schema has a region-tree shape; walking the
flat fields would miss every nested state.

```rust
pub trait IrVisitor {
    fn visit_document(&mut self, doc: &IrDocument)        { self.walk_document(doc); }
    fn visit_machine(&mut self, m: &MachineObject)        { self.walk_machine(m); }
    fn visit_state(&mut self, s: &StateNode)              { self.walk_state(s); }
    fn visit_transition(&mut self, t: &TransitionObject)  {}
    fn visit_guard(&mut self, g: &GuardExpr)              {}
    fn visit_action(&mut self, a: &ActionStatement)       {}
    fn visit_statement(&mut self, s: &Statement)          {}
    fn visit_expr(&mut self, e: &Expr)                    {}
    fn visit_timer(&mut self, t: &TimerDecl)              {}
    fn visit_context_field(&mut self, f: &FieldDecl)      {}
    fn visit_event(&mut self, e: &EventDecl)              {}
    fn visit_extern(&mut self, x: &ExternDecl)            {}
    fn visit_const(&mut self, c: &ConstDecl)              {}     // NEW per B-06
    fn visit_import(&mut self, i: &ImportDecl)            {}     // NEW per B-06
    fn visit_queue(&mut self, q: &QueueConfig)            {}     // NEW per B-06
    fn visit_target(&mut self, t: &TargetConfig)          {}     // NEW per B-06
    fn visit_region(&mut self, r: &RegionObject)          { self.walk_region(r); }

    // Default traversal methods — descend the region tree, NOT flat lists.
    fn walk_document(&mut self, doc: &IrDocument) {
        for i in &doc.imports  { self.visit_import(i); }
        for m in &doc.machines { self.visit_machine(m); }
    }
    fn walk_machine(&mut self, m: &MachineObject) {
        for c in &m.consts            { self.visit_const(c); }
        self.visit_queue(&m.queue);
        self.visit_target(&m.target);
        for f in &m.context.fields    { self.visit_context_field(f); }
        for e in &m.events            { self.visit_event(e); }
        for x in &m.externs           { self.visit_extern(x); }
        self.visit_region(&m.root);   // ← descend the root region
        for sm in &m.submachines      { self.visit_machine(sm); }
    }
    fn walk_state(&mut self, s: &StateNode) {
        for t in s.transitions()      { self.visit_transition(t); }
        for tmr in s.timers()         { self.visit_timer(tmr); }
        for r in s.regions()          { self.visit_region(r); }
    }
    fn walk_region(&mut self, r: &RegionObject) {
        for s in &r.states { self.visit_state(s); }
    }
}
```

Used by: `fsm-codegen-c` (CodegenPlan builder), `fsm-analyzer` (reachability
analysis), `fsm-simulator` (interpreter traversal). C++17 codegen reuse is
deferred to v1.1.

## 12.9 Formatter CST API

The formatter operates on the **CST** (Concrete Syntax Tree), NOT the AST or IR.
This preserves comments, whitespace, and original token positions.

```rust
// CST node types (from fsm-parser, used by fsm-formatter)
pub enum SyntaxKind {
    // Composite nodes (non-terminal)
    SourceFile, MachineDecl, StateDecl, RegionDecl, ContextBlock,
    EventsBlock, EventDecl, ExternDecl, TransitionDecl, InternalDecl,
    LocalDecl, CompletionDecl, AfterDecl, EveryDecl, DeferDecl,
    GuardExpr, ActionList, Statement, Expr, ChoiceDecl, JunctionDecl,
    ForkDecl, JoinDecl, HistoryDecl, FinalDecl, InitialDecl,
    FieldDecl, ParamDecl, TypeRef, StableId, DocComment,
    IfStmt, WhileStmt, ForStmt, AssignStmt, CallExpr, CastExpr,

    // Token nodes (terminal) — one per TokenKind
    KwMachine, KwState, /* ... all token kinds from fsm-lexer ... */
    Whitespace, Newline, LineComment, BlockComment,
}

// fsm-parser → fsm-formatter interface
pub struct SyntaxNode { /* rowan GreenNode wrapper */ }
impl SyntaxNode {
    pub fn kind(&self) -> SyntaxKind;
    pub fn text_range(&self) -> TextRange;
    pub fn children(&self) -> impl Iterator<Item = SyntaxNode>;
    pub fn children_with_tokens(&self) -> impl Iterator<Item = SyntaxElement>;
    pub fn first_token(&self) -> Option<SyntaxToken>;
    pub fn last_token(&self) -> Option<SyntaxToken>;
}

pub struct SyntaxToken { /* rowan GreenToken wrapper */ }
impl SyntaxToken {
    pub fn kind(&self) -> SyntaxKind;
    pub fn text(&self) -> &str;
    pub fn text_range(&self) -> TextRange;
}

pub enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxToken),
}
```

The formatter walks the CST using `children_with_tokens()` to access both structural
nodes and trivia (whitespace, comments). It rebuilds the output string by applying
formatting rules to the structure while preserving comment content verbatim.

---

# 13. Architecture Decision Records (Renumbered from §12)

## ADR-001: Why Rust for the Toolchain

**Decision:** All crates (`fsm-lexer` through `fsm-simulator`, `fsm-lsp`) are written
in Rust.

**Rationale:**
- Memory safety without GC — critical for a `no_std` target (WASM, bare-metal hosting)
- Excellent `no_std` support for the lexer and IR crates
- First-class WASM compilation via `wasm-bindgen`
- Rich parsing ecosystem (rowan, tower-lsp)
- Zero-cost abstractions for zero-overhead visitor pattern

**Trade-offs:** Longer onboarding time for contributors; no dynamic dispatch for
plugin-style extensibility.

## ADR-002: Why tower-lsp

**Decision:** Use `tower-lsp` as the async LSP framework.

**Rationale:** Battle-tested (used by rust-analyzer, taplo, marksman). Async-native.
Composable via Tower middleware. Handles stdio and TCP transports transparently.

## ADR-003: Why Recursive Descent (Not Parser Combinators)

**Decision:** Hand-written recursive-descent parser instead of nom/winnow/chumsky.

**Rationale:** Recursive descent gives full control over error recovery. Parser
combinator libraries make custom synchronization points difficult. The grammar is
small enough that a hand-written parser is maintainable.

## ADR-004: Why Rowan / Lossless CST

**Decision:** Use rowan-style green trees for the CST.

**Rationale:** Lossless tree enables formatter (must preserve all whitespace and
comments) and — as a future v1.3+ optimisation, *not* implemented in v1.x
(LSP ships full re-parse-on-debounce; see §4.5 / Doc 00 §11.39) —
incremental reparsing for LSP (only re-parse changed subtrees).

## ADR-005: Why Not Reuse an SCXML Runtime

**Decision:** Build a custom simulator; do not adapt an existing SCXML runtime.

**Rationale:** SCXML runtimes are too general (event I/O, HTTP binding, datamodel
variety). We need: (1) IR-native interpretation (no DOM), (2) virtual clock control
for testing, (3) determinism verification at interpret time, (4) WASM compilation.

## ADR-006: Why WebSocket (Not stdio) for Simulator Protocol

**Decision:** Simulator daemon exposes a WebSocket API, not a stdio pipe.

**Rationale:** WebSocket enables the Web IDE to connect from the browser without a
local process. It also enables HiL (Hardware-in-the-Loop) scenarios where the
simulator runs on a remote machine. stdio is simpler but not portable to browser.

## ADR-007: Why TypeScript for Editors (Not Rust + WASM)

**Decision:** VS Code extension and Web IDE are TypeScript; they call WASM only for
the compiler and LSP cores.

**Rationale:** VS Code extension API is TypeScript-native. React/Monaco ecosystem is
TypeScript. The editor UX logic (panel management, tree views, status bar) is best
expressed in TypeScript. Heavy computation is offloaded to WASM workers.

---

# 13. Cross-Cutting Concerns

## 13.1 Error Handling

All public APIs return `Result<T, FsmError>`. Each crate defines its own error type
that converts into the top-level `FsmError`.

### Per-Crate Error Types

```rust
// fsm-lexer — no errors (emits TokenKind::Error tokens instead)

// fsm-parser
pub enum ParseError {
    UnexpectedToken { span: Span, expected: Vec<TokenKind>, found: TokenKind },
    UnterminatedString { span: Span },
    UnterminatedComment { span: Span },
    UnexpectedEof { span: Span },
}

// fsm-analyzer
pub enum AnalysisError {
    DuplicateDeclaration { name: String, first: Span, second: Span, code: &'static str },
    UnresolvedReference { name: String, span: Span, code: &'static str },
    TypeMismatch { expected: TypeRef, found: TypeRef, span: Span, code: &'static str },
    DeterminismConflict { transitions: Vec<TransitionId>, code: &'static str },
    ReachabilityViolation { state: String, code: &'static str },
}

// fsm-ir
pub enum IrError {
    SchemaVersionMismatch { expected: String, found: String },
    InvalidNodeId { id: String },
    SerializationError(serde_json::Error),
}

// fsm-codegen-c / fsm-codegen-cpp
pub enum CodegenError {
    UnsupportedConstruct { construct: String, reason: String },
    NamingConflict { name: String },
    IoError(std::io::Error),
}

// fsm-simulator
pub enum SimError {
    InstanceNotFound { id: String },
    InvalidEvent { name: String },
    CompletionLoopDetected,
    ClockViolation { msg: String },
}

// fsm-formatter
pub enum FormatError {
    ParseError(Vec<ParseError>),  // formatter needs a valid CST
    IoError(std::io::Error),
}
```

### Error Propagation Path

```
fsm-lexer    → TokenKind::Error (no Result)
    │
fsm-parser   → Result<ParseResult, Vec<ParseError>>
    │
fsm-analyzer → Result<AnalysisResult, Vec<AnalysisError>>
    │
fsm-ir       → Result<IrDocument, IrError>
    │
    ├── fsm-codegen-c   → Result<EmittedFiles, CodegenError>
    ├── fsm-codegen-cpp  → Result<EmittedFiles, CodegenError>
    ├── fsm-simulator    → Result<StepResult, SimError>
    └── fsm-formatter    → Result<String, FormatError>
```

Top-level `FsmError` aggregates all error types:

```rust
#[non_exhaustive]
pub enum FsmError {
    Parse(Vec<ParseError>),
    Analysis(Vec<AnalysisError>),
    Ir(IrError),
    Codegen(CodegenError),
    Simulator(SimError),
    Format(FormatError),
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

## 13.2 Logging

All crates use the `tracing` crate with structured fields. Log level is controlled by
the `FSM_LOG` environment variable (e.g., `FSM_LOG=fsm_lsp=debug,fsm_analyzer=info`).

## 13.3 Testing Strategy

- **Unit tests:** `tests.rs` in each crate module. Run with `cargo test`.
- **Integration tests:** `tests/` directory at root. Each test is a `.fsm` file +
  expected output. Run with `cargo test --test conformance`.
- **Golden file tests:** Generated C files are compared against expected files using
  `insta` snapshot testing. Update with `cargo insta review`.
- **Property tests:** `fsm-analyzer` determinism detection uses `proptest` to generate
  random guard expressions and verify analysis consistency.

## 13.4 Feature Flags

| Crate | Feature Flag | Effect |
|---|---|---|
| `fsm-lexer` | `no_std` | Removes `std` dependency; requires `alloc` |
| `fsm-ir` | `no_std` | Removes `std`; uses `alloc` only |
| `fsm-simulator` | `wasm` | Enables `wasm-bindgen` exports; disables WebSocket server |
| `fsm-lsp` | `stdio` / `tcp` | Transport selection (default: stdio) |

## 13.5 WASM Build

The `fsm-wasm` crate re-exports the full pipeline with `#[wasm_bindgen]` annotations:

```rust
#[wasm_bindgen]
pub fn compile(source: &str) -> JsValue { /* returns IrDocument as JSON */ }

#[wasm_bindgen]
pub fn simulate_init(ir_json: &str, context_json: &str) -> JsValue { ... }

#[wasm_bindgen]
pub fn simulate_dispatch(handle: usize, event_json: &str) -> JsValue { ... }
```

Build: `wasm-pack build crates/fsm-wasm --target web --out-dir ../../editors/web-ide/public`

---

*End of FSM-ARCH-OVERVIEW v1.0.0*
