# Embedded FSM SDK — Infrastructure Requirements

**Document ID:** FSM-REQ-INFRA
**Version:** 1.0
**Status:** Normative Draft
**Scope:** Toolchain + Runtime + IDE + Web Services
**License:** MIT (specification + reference implementation)

This document defines the infrastructure-level requirements for the Embedded FSM SDK.
It covers the compiler toolchain architecture, CLI interface, language server, IR
specification, code generator, simulator protocol, VS Code extension, Web IDE, build
system integration, packaging, and test harness.

This document is **normative**. Implementations MUST satisfy all MUST-level requirements.

---

## 0. Architecture Overview

The SDK is structured as a layered pipeline of independently invocable components,
all sharing the canonical JSON model (IR) as the interchange format.

```
┌─────────────────────────────────────────────────────────────┐
│                    Source Files (.fsm)                      │
└────────────────────────────┬────────────────────────────────┘
                             │
                     ┌───────▼──────┐
                     │    Parser    │  (fsm-parse)
                     └───────┬──────┘
                             │  AST
                     ┌───────▼──────┐
                     │  Validator   │  (fsm-validate)
                     └───────┬──────┘
                             │  Validated AST
                     ┌───────▼──────┐
                     │   Analyzer   │  (fsm-analyze)
                     └───────┬──────┘
                             │  Canonical IR (JSON)
            ┌────────────────┼──────────────────────┐
            │                │                      │
     ┌──────▼──────┐  ┌──────▼──────┐   ┌──────────▼──────┐
     │  Code Gen   │  │  Simulator  │   │  Documentation  │
     │ (fsm-gen)   │  │  (fsm-sim)  │   │  (fsm-doc)      │
     └──────┬──────┘  └──────┬──────┘   └─────────────────┘
            │                │
     ┌──────▼──────┐  ┌──────▼──────────────────────┐
     │ C Source    │  │  WebSocket Debug Protocol   │
     │ Output      │  │  (IDE / HiL / CI)           │
     └─────────────┘  └─────────────────────────────┘
```

All components MUST be invocable independently via CLI.
All components MUST be usable as library APIs (no mandatory subprocess spawning for IDE integration).

---

# 1. CLI Interface Requirements

## 1.1 Command Structure

The SDK MUST provide a single unified CLI binary: `fsm`.

Subcommands:

```
fsm parse      [OPTIONS] <file>...          Parse and print AST / check syntax
fsm validate   [OPTIONS] <file>...          Validate + semantic check
fsm analyze    [OPTIONS] <file>...          Full static analysis
fsm compile    [OPTIONS] <file>...          Full pipeline to IR
fsm generate   [OPTIONS] <ir>              Generate code from IR
fsm simulate   [OPTIONS] <ir>              Launch simulator
fsm doc        [OPTIONS] <file>...          Generate documentation
fsm fmt        [OPTIONS] <file>...          Format source files
fsm check      [OPTIONS] <file>...          Parse + validate + analyze (CI usage)
fsm version                                 Print version info
fsm help       [SUBCOMMAND]                 Print help
```

## 1.2 Input/Output

- Input files MUST be accepted as positional arguments.
- Standard input MAY be accepted via `-` as filename.
- Output path MUST be configurable via `--output` or `-o`.
- Diagnostic output MUST go to stderr.
- Generated artifacts MUST go to stdout or the specified output path.
- Exit codes: `0` = success, `1` = error (parse/validate), `2` = internal error.

## 1.3 Diagnostic Output Formats

CLI MUST support `--format` with:

- `text` (default): Human-readable, with file:line:col prefix.
- `json`: Machine-readable JSON array of diagnostic objects.
- `sarif`: SARIF 2.1 format for CI/CD integration (GitHub Actions, Azure DevOps).

## 1.4 Verbosity and Logging

- `--verbose` / `-v`: Enable verbose output (pipeline stages, timing).
- `--quiet` / `-q`: Suppress all output except errors.
- `--no-color`: Disable ANSI color in text output.

## 1.5 Configuration File

- The SDK MUST support a project configuration file: `fsm.toml` (or `fsm.json`).
- Configuration MUST include: module root, target profile, output directories, feature flags.
- CLI flags MUST override configuration file values.

---

# 2. Parser Requirements

## 2.1 Grammar

- The parser MUST implement the grammar defined in the DSL Specification document exactly.
- The grammar MUST be formally specified in EBNF notation with no ambiguity.
- The parser MUST be a recursive-descent parser or equivalent deterministic parser.
- The parser MUST NOT use operator precedence parsing without formal documentation.

## 2.2 Error Recovery

- The parser MUST implement error recovery to continue parsing after syntax errors.
- After a syntax error, the parser MUST attempt to resynchronize at the next statement or
  declaration boundary.
- The parser MUST not produce false secondary errors caused by incomplete recovery.
- All recovered errors MUST be reported with stable diagnostic codes.

## 2.3 Output: Concrete Syntax Tree (CST)

- The parser MUST produce a Concrete Syntax Tree preserving all tokens including whitespace
  and comments.
- The CST MUST be the basis for the formatter (`fsm fmt`).
- The CST MUST include precise source location spans for every node.

## 2.4 Output: Abstract Syntax Tree (AST)

- The parser MUST produce an AST by lowering the CST.
- The AST MUST preserve all semantically significant structure.
- The AST MUST strip formatting but retain documentation comments.
- AST node types MUST be formally documented.

## 2.5 Incremental Parsing

- The parser SHOULD support incremental parsing for LSP performance.
- Incremental parsing MUST correctly handle insertions, deletions, and replacements.
- After an incremental update, unchanged AST subtrees MUST be reused.

## 2.6 Performance

- A file of 10,000 lines MUST parse in under 100ms on a modern desktop CPU.
- Incremental re-parse of a single line change MUST complete in under 10ms.

---

# 3. Language Server Protocol (LSP) Requirements

## 3.1 Protocol Compliance

The FSM-Lang Language Server MUST implement LSP version 3.17 or later.

## 3.2 Required Capabilities

| Capability | Requirement |
|---|---|
| `textDocument/publishDiagnostics` | MUST — all parser/validator diagnostics |
| `textDocument/completion` | MUST — keywords, state names, event names, types |
| `textDocument/hover` | MUST — type info, docstring, diagnostic on hover |
| `textDocument/definition` | MUST — go to state/event/type declaration |
| `textDocument/references` | MUST — find all uses of a state/event |
| `textDocument/rename` | MUST — rename state/event/field (updates stable ID if needed) |
| `textDocument/formatting` | MUST — full document formatting |
| `textDocument/rangeFormatting` | SHOULD |
| `textDocument/codeLens` | SHOULD — transition count, reachability status |
| `textDocument/signatureHelp` | SHOULD — event payload field completion |
| `textDocument/inlayHints` | SHOULD — type annotations, priority display |
| `textDocument/semanticTokens` | MUST — semantic highlighting |
| `workspace/symbol` | SHOULD — search states/machines by name |

## 3.3 Completion Quality

- Completion MUST be context-sensitive: offer only valid identifiers in the current scope.
- Completion MUST include documentation strings from `///` doc comments.
- Keyword completion MUST include a brief description of each keyword's purpose.
- State name completion after `->` MUST list only reachable states.

## 3.4 Diagnostics

- Diagnostics MUST be published on every document change (debounced, max 200ms delay).
- Each diagnostic MUST include: stable code, severity, range, message.
- Related information (e.g., conflicting transition location) MUST be included as diagnostic
  related locations.
- Quick-fix actions (code actions) MUST be provided for common errors:
  - `FSM-E0001`: Insert missing `initial` declaration.
  - `FSM-W0001`: Delete unreachable state (with confirmation).
  - `FSM-H0001`: Add `@id` annotation to history pseudo-state.

## 3.5 Language Server Binary

- The language server MUST be distributed as a standalone binary: `fsm-lsp`.
- The binary MUST accept `--stdio` flag for LSP over stdin/stdout.
- The binary MUST accept `--socket PORT` for LSP over TCP.

---

# 4. Semantic Validator Requirements

## 4.1 Validation Phases

The semantic validator operates in two separate phases:

1. **Structural Validation** (fast, always enabled):
   - Name resolution.
   - Type checking.
   - Basic structural rules (missing initial, duplicate names).

2. **Semantic Analysis** (slower, full pass):
   - Reachability analysis.
   - Nondeterminism detection.
   - Guard overlap analysis.
   - Deferral cycle detection.
   - Profile constraint validation.

Phase 1 MUST complete before Phase 2 begins.
Phase 1 errors MUST suppress Phase 2 to avoid false positives.

## 4.2 Guard Overlap Analysis

- The analyzer MUST detect pairs of transitions on the same event with equal priority and
  potentially overlapping guards.
- For constant guard expressions: exact evaluation required.
- For non-constant guard expressions: conservative analysis (assume overlap possible).
- Provably disjoint guards (e.g., `x < 10` and `x >= 10`) MUST NOT be flagged as overlapping.
- The analysis MUST report all conflicting transition pairs, not just the first.

## 4.3 Reachability Analysis

- Reachability is computed from the initial state forward.
- All state nodes, regions, and pseudo-states must be reachable.
- Transitions leading to unreachable targets MUST be reported.
- History pseudo-states with no path back to their region MUST be reported.

## 4.4 Deferral Cycle Detection

- For each event, the analyzer MUST verify that there exists at least one reachable state
  that does NOT defer the event.
- A deferral cycle exists if an event is deferred in all states reachable from the initial
  configuration.

## 4.5 Incremental Analysis

- The analyzer MUST support incremental re-analysis on document change.
- Only affected scopes (changed machine or its dependencies) MUST be re-analyzed.

---

# 5. Intermediate Representation (IR) / Canonical JSON Model

## 5.1 Format

- The canonical model MUST be a JSON object.
- The schema MUST be formally published as a JSON Schema (draft-07 or later).
- The schema version MUST be embedded in the document: `"$schema"`, `"version"`.

## 5.2 Required Top-Level Fields

```json
{
  "$schema": "https://fsm-sdk.dev/schemas/ir/1.0.0/model.json",
  "version": "1.0.0",
  "generated": "<ISO 8601 timestamp>",
  "source": "<source file path>",
  "sourceHash": "<SHA-256 of source>",
  "machines": [ ... ]
}
```

## 5.3 Machine Object

Each machine object MUST include:

- `id`: Stable machine ID.
- `name`: Machine name.
- `context`: Array of typed field definitions.
- `events`: Array of event definitions with payload schema.
- `profile`: Target profile name.
- `queue`: Queue configuration object.
- `states`: Array of state/region/pseudo-state node definitions.
- `transitions`: Array of transition definitions.
- `timers`: Array of timer definitions.
- `diagnostics`: Array of diagnostic objects from the analysis phase.
- `metadata`: Source location metadata for all nodes.

## 5.4 Node IDs

- Every node in the model MUST have a unique `nodeId` (UUID or hash-based stable ID).
- If the author provides an `@id` annotation, it MUST be used as the stable ID.
- Generated IDs (no `@id`) MUST be reproducible: derived from the qualified path of the node.

## 5.5 Stability

- Given the same DSL source, the canonical model MUST be identical across runs.
- Timestamps and random elements MUST NOT be included in the stable model fields.
- The `generated` timestamp is excluded from stability scope.

## 5.6 Schema Versioning

- The schema version MUST follow semantic versioning (MAJOR.MINOR.PATCH).
- A MAJOR version bump indicates breaking changes to the schema.
- A MINOR version bump adds fields in a backward-compatible way.
- Toolchain consumers MUST check `version` before deserializing.
- The IR schema MUST be published alongside the DSL specification.

---

# 6. Code Generator Requirements

## 6.1 Target Languages

The reference implementation MUST support C (C99) as the primary target.
Additional targets (C++17, Rust) MAY be provided as separate generator plugins.

## 6.2 Generation Strategies

Two strategies MUST be supported and selectable per profile:

### 6.2.1 Switch-Based Strategy

- Each state is represented as an enumeration value.
- A dispatch function uses nested switch statements.
- Hierarchical execution is achieved via explicit fallthrough or function chain.
- More readable, easier to debug manually.
- Suitable for small to medium machines (<64 states).

### 6.2.2 Table-Driven Strategy

- Transitions are represented as a static table of `(state, event) → action, target` entries.
- Hierarchical state entry/exit is achieved via parent arrays.
- More compact, better for large machines or ROM-constrained targets.
- Less human-readable but identical semantics to switch-based.

## 6.3 Generated File Structure

For each machine `M`, the generator MUST produce:

- `M.h`: Public API header (machine type, event type, dispatch function prototype).
- `M.c`: Implementation (context initialization, dispatch function, timer callbacks).
- `M_impl.h`: External function declarations (user must implement).
- `M_conf.h`: Compile-time configuration macros (queue size, feature toggles).

## 6.4 HAL (Hardware Abstraction Layer)

The generated code MUST depend on a timer HAL with a documented interface:

```c
/* User must provide: */
void fsm_timer_start(uint32_t timer_id, uint32_t ms, bool periodic);
void fsm_timer_cancel(uint32_t timer_id);
uint32_t fsm_timer_now_ms(void);
```

The HAL MUST be documented and a reference implementation for common RTOS/bare-metal
configurations MUST be provided (FreeRTOS, Zephyr, POSIX).

## 6.5 Thread Safety

- Generated code MUST be reentrant-safe per machine instance.
- No global state MUST be used in generated dispatch functions.
- Thread safety between multiple instances or ISR/thread dispatch MAY be provided via
  user-supplied locking in the HAL.

## 6.6 Correctness

- Generated code semantics MUST be verified against the simulator for each generated machine.
- A test harness MUST exercise all transitions, entry/exit sequences, and history restore paths.
- The test harness MUST be generated alongside the code.

---

# 7. Simulator Requirements

## 7.1 Architecture

The simulator MUST be a self-contained process (`fsm-sim`) that:

- Loads a canonical IR (JSON model).
- Executes the machine using the formally defined semantics.
- Exposes a WebSocket API for external control.
- Uses a virtual clock (not wall time) for deterministic behavior.

## 7.2 WebSocket Protocol

The simulator MUST expose a JSON-RPC 2.0 API over WebSocket on a configurable port (default: 7400).

### 7.2.1 Required Methods

| Method | Description |
|---|---|
| `sim.init` | Load a machine IR and initialize to initial state. |
| `sim.step` | Execute one event dispatch cycle. |
| `sim.run` | Execute until breakpoint, final state, or `sim.pause`. |
| `sim.pause` | Pause execution. |
| `sim.reset` | Reset to initial state. |
| `sim.inject` | Inject an event with payload. |
| `sim.getConfig` | Get active state configuration (all active states). |
| `sim.getContext` | Get current context field values. |
| `sim.setContext` | Set a context field value. |
| `sim.getTimers` | Get active timers and remaining time. |
| `sim.advanceClock` | Advance virtual clock by N milliseconds. |
| `sim.setBreakpoint` | Set a breakpoint (state entry, transition, guard). |
| `sim.clearBreakpoint` | Remove a breakpoint. |
| `sim.getTrace` | Retrieve the execution trace. |
| `sim.replay` | Load and replay a trace. |
| `sim.getSchema` | Return the protocol schema version. |

### 7.2.2 Required Notifications (Server → Client)

| Notification | Description |
|---|---|
| `sim.onStateEntry` | A state was entered. Includes state ID and timestamp. |
| `sim.onStateExit` | A state was exited. |
| `sim.onTransition` | A transition was executed. Includes source, target, event, guard. |
| `sim.onBreakpoint` | Execution paused at a breakpoint. |
| `sim.onFinalState` | A region reached its final state. |
| `sim.onCompletion` | A completion event was generated. |
| `sim.onTimerFired` | A timer event was generated. |
| `sim.onQueueOverflow` | Event queue overflow with policy applied. |

### 7.2.3 Protocol Version

- The WebSocket protocol MUST carry a version in the handshake.
- Protocol version MUST follow semantic versioning.
- The `sim.getSchema` method MUST return the protocol version and method catalog.

## 7.3 Trace Format

Traces MUST be serializable as JSON arrays of event records. Each record MUST include:

- Record type (event_dispatched, state_entered, state_exited, transition_executed,
  timer_fired, context_mutated, queue_overflow).
- Virtual clock timestamp (milliseconds).
- Node IDs (stable IDs of involved states/transitions).
- Event data (event name, payload fields).
- Guard evaluation result (for transition records).

A trace MUST be sufficient for deterministic replay without access to the original IR.

## 7.4 Virtual Clock

- The virtual clock starts at 0.
- `sim.step` and `sim.run` advance the clock by the minimum timer expiry or no time if
  no timer is active.
- `sim.advanceClock(N)` advances the clock by N milliseconds, firing all expired timers.
- Wall time MUST NOT be used in any simulation computation.

## 7.5 Hardware-in-the-Loop (HiL) Mode

- The simulator MAY operate in HiL mode where events come from a real hardware source
  via a serial/CAN/USB bridge.
- HiL mode MUST replace `sim.inject` with an external event source driver.
- HiL mode MUST still allow context inspection and trace collection.

---

# 8. VS Code Extension Requirements

## 8.1 Extension Identity

- Extension ID: `fsm-sdk.fsm-lang`
- Display name: `FSM-Lang`
- File extension: `.fsm`
- Language ID: `fsm-lang`
- Extension MUST be publishable to the VS Code Marketplace (Open VSX Registry for open source).

## 8.2 Syntax Highlighting

- Syntax highlighting MUST be provided via a TextMate grammar (`syntaxes/fsm-lang.tmLanguage.json`).
- The grammar MUST highlight:
  - Keywords (machine, state, on, guard, etc.)
  - State names and event names (separate scopes for semantic coloring)
  - Type names
  - Literals (integers, booleans, strings)
  - Comments (single-line, multi-line, doc comments)
  - Operators
  - Diagnostic annotations (`@id`, etc.)
  - Stable ID strings
- The grammar MUST be usable independently of the language server for basic syntax coloring.

## 8.3 Language Server Integration

- The extension MUST launch `fsm-lsp` as a subprocess with `--stdio`.
- The LSP client MUST support all capabilities listed in Section 3.2.
- LSP restart MUST be available via command palette: `FSM-Lang: Restart Language Server`.

## 8.4 Diagram Panel (WebView)

The extension MUST provide a live state diagram WebView panel.

Requirements:
- Command: `FSM-Lang: Open Diagram` (also available as editor title button).
- The panel renders the current file's state machine as an interactive diagram.
- **Live update**: The diagram MUST update within 500ms of a source file change.
- **Bidirectional navigation**: Clicking a state in the diagram MUST navigate to its
  declaration in the editor. Hovering over a state in the editor MUST highlight it in the diagram.
- **Active configuration overlay**: When the simulator is running, the diagram MUST overlay
  the currently active states with a distinct visual indicator.
- **Transition overlay**: Executed transitions MUST be briefly animated in the diagram.
- The diagram renderer MUST use deterministic layout (not randomized force-directed).
- The renderer MUST support zoom, pan, and fit-to-view.
- The renderer MUST support export to SVG and PNG.

## 8.5 Simulator Panel (WebView)

The extension MUST provide an integrated simulator panel.

Requirements:
- Command: `FSM-Lang: Open Simulator`.
- **Start/Stop/Reset controls**.
- **Step button**: Execute one dispatch cycle.
- **Event injection form**: Select event from dropdown, fill payload fields, inject.
- **Active configuration display**: Show all currently active states.
- **Context inspector**: Show all context fields with current values; allow editing.
- **Timer inspector**: Show all active timers with remaining time.
- **Trace log**: Scrollable list of all simulator events with timestamps and stable IDs.
- **Breakpoint management**: List, add, and remove breakpoints.
- The simulator panel MUST connect to `fsm-sim` via the WebSocket protocol.
- The panel MUST handle `fsm-sim` reconnection transparently.

## 8.6 Commands

The extension MUST register the following commands:

| Command ID | Title |
|---|---|
| `fsm.openDiagram` | FSM-Lang: Open Diagram |
| `fsm.openSimulator` | FSM-Lang: Open Simulator |
| `fsm.generateCode` | FSM-Lang: Generate Code |
| `fsm.compileToIR` | FSM-Lang: Compile to IR |
| `fsm.formatDocument` | FSM-Lang: Format Document |
| `fsm.restartServer` | FSM-Lang: Restart Language Server |
| `fsm.showDiagnostics` | FSM-Lang: Show All Diagnostics |
| `fsm.exportDiagram` | FSM-Lang: Export Diagram as SVG |

## 8.7 Code Snippets

The extension MUST provide snippets for:

- New machine declaration skeleton.
- New state with entry/exit.
- External transition with guard.
- Internal transition.
- Timer declaration.
- Context declaration.
- Event declaration.
- Parallel region structure.

## 8.8 Configuration

The extension MUST expose settings under `fsm-lang.*`:

- `fsm-lang.lspPath`: Path to the `fsm-lsp` binary.
- `fsm-lang.simulatorPath`: Path to the `fsm-sim` binary.
- `fsm-lang.simulatorPort`: WebSocket port for simulator.
- `fsm-lang.diagramLayout`: Layout algorithm (`hierarchical`, `dot`, `elk`).
- `fsm-lang.generateOnSave`: Auto-generate code on save.
- `fsm-lang.profile`: Default target profile.

## 8.9 Problems Integration

- All diagnostics from the language server MUST appear in the VS Code Problems panel.
- Each diagnostic MUST be clickable to navigate to the source location.
- Diagnostic codes MUST appear as links to the online documentation.

---

# 9. Web IDE Requirements

## 9.1 Architecture

- The Web IDE MUST be a standalone single-page application (SPA).
- It MUST be deployable as a static site (no server-side rendering required).
- The backend MUST be the WebSocket-based simulator and the FSM compiler CLI (via a thin bridge).

## 9.2 Required Features

| Feature | Requirement |
|---|---|
| Code editor (Monaco) | MUST — syntax highlighting, LSP via web worker |
| File tree | MUST — create, rename, delete `.fsm` files |
| Diagram panel | MUST — same as VS Code extension diagram panel |
| Simulator panel | MUST — same as VS Code extension simulator panel |
| Diagnostics panel | MUST — Problems view with diagnostic codes |
| Code generation | MUST — generate and download C code from browser |
| Share URL | SHOULD — serialize current state to URL-safe string |
| Project import/export | MUST — zip archive of project files |
| Keyboard shortcuts | MUST — standard editor shortcuts |

## 9.3 Monaco Editor Integration

- The Web IDE MUST embed Monaco Editor (VS Code's editor component).
- Syntax highlighting MUST be provided via a Monaco language registration with the FSM-Lang grammar.
- The LSP client MUST run as a Web Worker communicating with the language server over a
  message channel.
- Completion, hover, diagnostics, and go-to-definition MUST be supported.

## 9.4 Deployment

- The Web IDE MUST be deployable to any static hosting provider (Netlify, GitHub Pages, S3).
- The simulator backend MUST be separately deployable as a Docker container.
- The Web IDE MUST support connecting to a user-provided simulator URL.

---

# 10. Build System Integration Requirements

## 10.1 CMake Integration

- The SDK MUST provide a CMake module (`FindFSMLang.cmake` or `fsm-lang-config.cmake`).
- The CMake module MUST provide a `fsm_generate_code(TARGET <name> SOURCES <file>...)` function.
- Generated files MUST be added to the target's sources automatically.
- Re-generation MUST be triggered only when source `.fsm` files change.
- The module MUST support `PROFILE`, `STRATEGY`, and `OUTPUT_DIR` parameters.

## 10.2 Make Integration

- A `fsm.mk` Makefile include MUST be provided.
- Pattern rules MUST be defined: `%.c %.h: %.fsm`.
- Dependency tracking MUST be included (`.d` files or explicit deps).

## 10.3 Other Build Systems

- Meson build system integration SHOULD be provided as `meson/fsm-lang.wrap`.
- Bazel rules SHOULD be provided as `fsm_library` and `fsm_generate` rules.

## 10.4 CI/CD Pipeline Integration

- The `fsm check` command MUST be usable in CI pipelines as a lint/validate step.
- SARIF output format MUST be supported for GitHub Actions annotation.
- Exit code conventions MUST be documented.
- A GitHub Actions composite action MUST be provided: `.github/actions/fsm-check`.

---

# 11. Runtime SDK Requirements

## 11.1 Dispatch Function Contract

The generated dispatch function MUST:

```c
/* Signature (switch-based strategy): */
void M_dispatch(M_t *m, const M_event_t *event);

/* Thread safety: reentrant per instance, not thread-safe across instances. */
/* Execution time: bounded. */
/* Memory: no allocation. All state in m. */
```

## 11.2 Machine Instance Structure

Each machine instance MUST be a single statically allocated struct containing:

- Context data.
- Active state configuration (state index or bitfield for parallel regions).
- History storage (one entry per history pseudo-state).
- Event queue (inline bounded ring buffer).
- Timer handles (one per declared timer).

Total size MUST be computable at compile time.

## 11.3 Initialization

```c
void M_init(M_t *m);              /* Initialize to initial state, execute initial entry actions. */
void M_reset(M_t *m);             /* Reset to initial state without re-initialization. */
bool M_is_final(const M_t *m);   /* Returns true if machine has reached a top-level final state. */
```

## 11.4 Event Enqueue

```c
bool M_enqueue(M_t *m, const M_event_t *event); /* Returns false on overflow if policy = error. */
```

## 11.5 Tick Function

For timer integration without RTOS:

```c
void M_tick(M_t *m, uint32_t elapsed_ms); /* Advance timers by elapsed time, fire expired timers. */
```

## 11.6 Portability

- Runtime MUST compile with `-std=c99 -Wall -Wextra -pedantic` without warnings.
- No dependency on OS headers beyond `<stdint.h>`, `<stdbool.h>`, `<string.h>`.
- No VLAs, no alloca, no `setjmp`/`longjmp`.

---

# 12. Packaging and Distribution Requirements

## 12.1 CLI Binary Distribution

- Pre-built binaries MUST be provided for:
  - Linux x86_64 (musl static)
  - Linux aarch64 (musl static)
  - macOS x86_64
  - macOS aarch64 (Apple Silicon)
  - Windows x86_64 (MSVC runtime)
- Binaries MUST be distributed as GitHub Releases assets.
- Checksums (SHA-256) MUST be published alongside binaries.

## 12.2 Package Managers

| Package Manager | Package Name | MUST/SHOULD |
|---|---|---|
| npm (Node.js) | `@fsm-sdk/cli` | MUST (VS Code extension dependency) |
| Homebrew | `fsm-sdk` | SHOULD |
| Cargo (Rust, if implemented in Rust) | `fsm-lang` | SHOULD |
| pip (Python wheels) | `fsm-sdk` | SHOULD |
| winget / Chocolatey | `fsm-sdk` | SHOULD |

## 12.3 VS Code Extension Distribution

- The extension MUST be published to the VS Code Marketplace.
- The extension MUST be published to the Open VSX Registry (for open-source IDEs).
- The extension MUST bundle the language server binary for the user's platform.
- Extension version MUST follow semantic versioning aligned with SDK version.

## 12.4 Docker Image

- A Docker image MUST be provided: `fsm-sdk/compiler:latest`.
- The image MUST include: `fsm` CLI, `fsm-lsp`, `fsm-sim`.
- Base image MUST be `alpine` or `distroless` for minimal size.
- Multi-arch images (linux/amd64, linux/arm64) MUST be supported.

---

# 13. Test Harness Requirements

## 13.1 Unit Tests for Generated Code

For each machine, the code generator SHOULD optionally produce a test harness:

- A `M_test.c` file with a `main()` function.
- Test scenarios: one per transition, one per history path, one per parallel region.
- Test infrastructure: event injection, context assertion, state assertion.
- Compatible with any C test framework (Unity, CTest, custom).

## 13.2 Conformance Test Suite

A conformance test suite MUST be published alongside the specification:

- A set of FSM-Lang source files covering all language constructs.
- Expected canonical IR output for each source file.
- Expected diagnostics for invalid source files (error injection tests).
- Expected simulator traces for behavioral tests.
- Any conformant implementation MUST pass all conformance tests.

## 13.3 Regression Tests

- All reported bugs MUST have a corresponding regression test before closing.
- Regression tests MUST be part of the CI pipeline.

## 13.4 Coverage Requirements

- Parser: 100% of grammar productions tested (positive and negative cases).
- Validator: 100% of diagnostic codes tested.
- Code generator: 100% of IR constructs exercised in generated code.
- Simulator: 100% of WebSocket methods tested.
