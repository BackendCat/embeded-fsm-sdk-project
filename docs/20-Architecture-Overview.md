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
   comments is stored. Used by the formatter and incremental reparsing for LSP.
2. **AST (Abstract Syntax Tree)** — typed, trivia-free. Used by the analyzer and
   code generators.

The CST uses a [rowan](https://github.com/rust-analyzer/rowan)-style green tree:
an arena-allocated, reference-counted, immutable tree. AST nodes are thin typed wrappers
over CST nodes, providing a typed API with no allocation.

## 4.2 Module Layout

```
fsm-parser/src/
├── lib.rs              # Public API
├── parse.rs            # Entry point: parse(src) → ParseResult
├── parser.rs           # Recursive-descent parser implementation
├── grammar/
│   ├── top_level.rs    # machine_decl, event_decl, extern_decl
│   ├── state.rs        # state_decl, composite, parallel, pseudo-states
│   ├── transition.rs   # transition, guard_expr, action_list
│   ├── action.rs       # statement, expr (action sublanguage)
│   └── guard.rs        # guard_expr (restricted sublanguage)
├── cst/
│   ├── kinds.rs        # SyntaxKind enum (one variant per grammar rule + token)
│   └── green.rs        # GreenNode, GreenToken
├── ast/
│   ├── nodes.rs        # Typed AST node structs
│   ├── token_ext.rs    # Helper methods on tokens
│   └── generated.rs    # Auto-generated node accessor methods
├── error.rs            # ParseError type
└── tests.rs
```

## 4.3 Key Types

```rust
pub struct ParseResult {
    pub tree: SyntaxNode,          // CST root
    pub errors: Vec<ParseError>,
}

pub struct ParseError {
    pub span: Span,
    pub expected: Vec<TokenKind>,
    pub found: TokenKind,
    pub code: &'static str,        // "FSM-E0010" etc.
}

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

```
fsm-analyzer/src/
├── lib.rs              # Public API: analyze(ast) → AnalysisResult
├── phase1_structure/
│   ├── mod.rs          # Phase 1 entry point
│   ├── symbol_table.rs # Scoped symbol registry
│   ├── builder.rs      # Walk AST, build symbol table
│   └── resolver.rs     # Resolve name references to IDs
├── phase2_semantic/
│   ├── mod.rs          # Phase 2 entry point
│   ├── type_checker.rs # Bidirectional type inference for expressions
│   ├── determinism.rs  # Conflict detection algorithm (FSM-E0300)
│   ├── reachability.rs # Dead state and unreachable transition detection
│   ├── history.rs      # History invariant checks (FSM-W0100)
│   └── submachine.rs   # Submachine cycle detection (FSM-E0502)
├── diagnostics.rs      # DiagnosticAccumulator
├── ir_builder.rs       # Converts resolved AST + symbol table → IR
└── tests.rs
```

## 5.3 Key Types

```rust
pub struct AnalysisResult {
    pub ir: IrDocument,                    // from fsm-ir
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub symbol_table: SymbolTable,         // retained for LSP queries
}

pub struct SymbolTable {
    scopes: HashMap<ScopeId, Scope>,
}

pub struct Scope {
    pub kind: ScopeKind,      // Machine | State | Region
    pub symbols: HashMap<String, SymbolId>,
    pub parent: Option<ScopeId>,
}

pub struct TypeEnv {
    pub fields: HashMap<String, TypeRef>,  // context + payload fields
}
```

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

```
fsm-lexer          (no_std, no deps)
    │
    └──► fsm-parser        (depends on: fsm-lexer)
              │
              └──► fsm-analyzer     (depends on: fsm-parser, fsm-ir)
                        │
                        └──► fsm-ir          (depends on: serde, serde_json)
                                  │
                        ┌─────────┴──────────────────────┐
                        │                                │
                   fsm-codegen-c                  fsm-codegen-cpp
                   (depends on: fsm-ir)           (depends on: fsm-ir)
                        │                                │
                        └───────────┬────────────────────┘
                                    │
                              fsm-simulator
                              (depends on: fsm-ir)
                              (WASM feature: fsm-wasm wraps this)
                                    │
                              fsm-lsp
                              (depends on: fsm-lexer, fsm-parser,
                               fsm-analyzer, fsm-ir, fsm-codegen-c,
                               fsm-simulator [embedded mode])
                                    │
                              fsm-formatter
                              (depends on: fsm-lexer, fsm-parser)

                              fsm-cli (binary)
                              (depends on: ALL crates above via clap)

                              fsm-wasm (WASM lib)
                              (depends on: fsm-parser, fsm-analyzer,
                               fsm-ir, fsm-codegen-c, fsm-simulator)
```

**Key rules:**
- `fsm-ir` MUST NOT depend on `fsm-parser` or `fsm-analyzer` (avoid cycles).
- `fsm-codegen-*` MUST depend on `fsm-ir` only — not on the parser or analyzer.
- `fsm-simulator` takes IR as input — it MUST NOT depend on the parser.
- `fsm-lsp` is the only crate that imports the full pipeline.
- `fsm-formatter` depends only on the CST — it does NOT need `fsm-analyzer` or IR.
- `fsm-wasm` re-exports the full pipeline with `#[wasm_bindgen]` attributes.

**Build system: Cargo**
```bash
# Build all crates
cargo build --workspace

# Build WASM
wasm-pack build crates/fsm-wasm --target web --out-dir ../../editors/web-ide/public

# Build VS Code extension
cd editors/vscode && npm install && npm run compile

# Package VS Code extension
cd editors/vscode && npx vsce package
```

---

# 12. Architecture Decision Records

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
comments) and incremental reparsing for LSP (only re-parse changed subtrees).

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

All public APIs return `Result<T, FsmError>`. `FsmError` is a non-exhaustive enum:

```rust
#[non_exhaustive]
pub enum FsmError {
    ParseError(Vec<ParseError>),
    AnalysisError(Vec<AnalysisDiagnostic>),
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    SchemaVersionMismatch { expected: String, found: String },
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
