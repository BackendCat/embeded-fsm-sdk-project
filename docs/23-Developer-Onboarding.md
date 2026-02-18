# FSM Studio SDK — Developer Onboarding and Technology Stack

**Document ID:** FSM-DEV-ONBOARD
**Version:** 1.0.0
**Status:** Normative Draft

This is the primary entry point for developers and AI agents starting implementation.
It covers the tech stack, repository structure, build commands, and role-based reading guides.

---

# 1. Project Purpose

FSM Studio is an embedded-first toolchain for hierarchical finite state machines. It takes
`.fsm` source files (FSM-Lang DSL), compiles them through a Rust pipeline, and produces:

- **(a)** Deterministic C99 or C++17 code safe for use on microcontrollers without heap allocation
- **(b)** A VS Code extension with syntax highlighting, LSP diagnostics, live diagram, and simulator
- **(c)** A browser-based Web IDE using the same Rust pipeline compiled to WebAssembly

The project has no runtime dependencies on any OS or standard library beyond what the HAL provides.

---

# 2. Technology Stack

| Component | Technology | Version Constraint | Notes |
|---|---|---|---|
| Core compiler/toolchain | Rust | ≥ 1.75 (edition 2021) | All crates in a single Cargo workspace |
| Parser / CST | `rowan` crate | latest stable | Lossless concrete syntax tree; enables formatter and incremental LSP |
| LSP server | `tower-lsp` crate | latest stable | Async LSP 3.17 server; stdio transport |
| Async runtime | `tokio` | ≥ 1.35 | Used by tower-lsp and simulator WebSocket server |
| WebSocket server | `tokio-tungstenite` | latest stable | Simulator JSON-RPC 2.0 over WebSocket |
| Serialization | `serde` + `serde_json` | latest stable | IR schema, simulator protocol, CLI `--json` output |
| CLI argument parsing | `clap` | ≥ 4.0 | Builder API; generates shell completions |
| Error reporting | `miette` | latest stable | Rust-style diagnostic output with source snippets |
| WASM compilation | `wasm-bindgen` + `wasm-pack` | latest stable | Produces `fsm-wasm` NPM package for Web IDE |
| VS Code extension | TypeScript | ≥ 5.3 | Uses `vscode-languageclient` for LSP; `esbuild` for bundling |
| Diagram rendering | `@elklayout/core` + custom SVG | latest stable | ELK hierarchical layout; SVG rendered in VS Code WebviewPanel |
| Extension packaging | `@vscode/vsce` | latest stable | Produces `.vsix` for marketplace publish |
| Web IDE | TypeScript + WASM | — | CodeMirror 6 editor; same WASM package as above |
| Testing (Rust) | `cargo test` + `insta` | latest stable | `insta` for snapshot/golden-file tests |
| Testing (conformance) | Custom Rust harness | — | Reads `MANIFEST.json`; described in FSM-SPEC-TEST |
| CI | GitHub Actions | — | `rust-toolchain.toml` pins Rust version; matrix: linux, macos, windows |

---

# 3. Repository Layout

```
sm-sdk/
├── Cargo.toml               # Workspace root
├── rust-toolchain.toml      # Pinned Rust version
├── fsm.toml                 # Project configuration (for dogfooding)
├── crates/
│   ├── fsm-lexer/           # Tokenizer — no workspace dependencies
│   ├── fsm-parser/          # Recursive-descent parser → Rowan CST
│   ├── fsm-analyzer/        # Semantic analysis, type checker, determinism
│   ├── fsm-ir/              # Canonical JSON IR types (serde derive)
│   ├── fsm-codegen-c/       # C99 code generator (IR → .h + .c)
│   ├── fsm-codegen-cpp/     # C++17 code generator (IR → .hpp + .cpp)
│   ├── fsm-simulator/       # WebSocket JSON-RPC 2.0 simulator server
│   ├── fsm-lsp/             # Language server (tower-lsp)
│   ├── fsm-formatter/       # Canonical formatter (CST → formatted text)
│   ├── fsm-cli/             # `fsm` binary entry point (clap)
│   └── fsm-wasm/            # WASM bindings (wasm-bindgen)
├── editors/
│   ├── vscode/              # VS Code extension (TypeScript)
│   │   ├── package.json     # Extension manifest (see FSM-SPEC-VSCE)
│   │   ├── src/
│   │   │   ├── extension.ts # Activation, LSP client, commands
│   │   │   ├── diagram.ts   # Diagram WebviewPanel
│   │   │   ├── simulator.ts # Simulator panel + WebSocket client
│   │   │   └── explorer.ts  # Machine/Event tree views
│   │   ├── syntaxes/        # fsm-lang.tmLanguage.json
│   │   ├── snippets/        # fsm-lang.json
│   │   └── bin/             # Bundled fsm binaries (5 platforms)
│   └── web/                 # Web IDE (TypeScript + WASM)
│       ├── src/
│       │   ├── editor.ts    # CodeMirror 6 integration
│       │   ├── compiler.ts  # WASM bridge
│       │   └── diagram.ts   # ELK + SVG diagram
│       └── public/
├── schema/
│   └── ir/1.0.0/model.json  # JSON Schema draft-07 for IR (FSM-SPEC-IR deliverable)
├── tests/
│   └── conformance/         # FSM-SPEC-TEST test suite
│       ├── MANIFEST.json
│       ├── parser/
│       ├── validator/
│       ├── semantic/
│       ├── codegen-c/
│       └── formatter/
├── examples/
│   ├── motor/               # Motor controller (3-state simple example)
│   ├── traffic-light/       # Traffic light (composite state example)
│   └── vending-machine/     # Vending machine (parallel regions example)
└── docs/                    # All specification documents
    └── RU/                  # Russian translations (where available)
```

---

# 4. Crate Dependency Rules

These rules are **strict** and must not be violated. They enforce a clean, acyclic dependency
graph and prevent build-time coupling between pipeline stages.

```
fsm-lexer      ← no workspace deps
fsm-parser     ← fsm-lexer
fsm-ir         ← fsm-parser (SourceLocation only); MUST NOT import analysis types
fsm-analyzer   ← fsm-parser, fsm-ir
fsm-codegen-c  ← fsm-ir only (NOT parser, NOT analyzer)
fsm-codegen-cpp← fsm-ir only (NOT parser, NOT analyzer)
fsm-formatter  ← fsm-lexer, fsm-parser only (no analysis needed)
fsm-simulator  ← fsm-ir, fsm-analyzer
fsm-lsp        ← all pipeline crates
fsm-cli        ← all crates
fsm-wasm       ← all pipeline crates; no std features incompatible with WASM
```

**Key rule:** `fsm-codegen-c` and `fsm-codegen-cpp` MUST only see `fsm-ir`. They must never
import the parser or analyzer. This keeps generated code independent of parse tree changes.

---

# 5. Build Commands

```bash
# Install Rust (reads rust-toolchain.toml automatically)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build all Rust crates
cargo build --workspace

# Run all Rust tests
cargo test --workspace

# Build the CLI binary (release)
cargo build --release -p fsm-cli

# Build WASM package
wasm-pack build crates/fsm-wasm --target bundler --out-dir ../../editors/web/pkg

# Install VS Code extension dependencies
cd editors/vscode && npm install

# Build VS Code extension
cd editors/vscode && npm run compile

# Package VS Code extension (.vsix)
cd editors/vscode && npx vsce package

# Run conformance tests
cargo run -p fsm-cli -- test tests/conformance/

# Format all .fsm examples (dogfood — must exit 0)
cargo run -p fsm-cli -- fmt --check examples/**/*.fsm
```

---

# 6. Specification Index — Role-Based Reading Guide

| Role | Read First | Then | Reference |
|---|---|---|---|
| Parser implementer | 04 — DSL Spec | 08 — Semantics, 09 — IR Schema | 10 — Diagnostic Catalog |
| Semantic analyzer | 08 — Semantics | 09 — IR Schema, 10 — Diagnostics | 04 — DSL Spec §11 |
| C99 codegen implementer | 09 — IR Schema | 11 — C99 Codegen | 16 — HAL, 15 — Test Suite |
| C++17 codegen implementer | 09 — IR Schema | 12 — C++17 Codegen | 15 — Test Suite |
| Simulator implementer | 08 — Semantics | 13 — Simulator Protocol | 09 — IR Schema |
| LSP implementer | 14 — LSP Capabilities | 04 — DSL Spec, 10 — Diagnostics | 09 — IR Schema |
| Formatter implementer | 04 — DSL Spec | 19 — Formatter Spec | — |
| VS Code extension developer | 22 — VS Code Manifest | 21 — TextMate Grammar, 05 — UI Spec | 14 — LSP, 13 — Simulator |
| Web IDE developer | 05 — UI Spec | 20 — Architecture (§9 WASM) | 13 — Simulator Protocol |
| QA / test engineer | 15 — Test Suite | 10 — Diagnostic Catalog | 08 — Semantics |

---

# 7. Complete Specification Document List

| # | File | ID | Description |
|---|---|---|---|
| 01 | `01-Scientific-Justification.md` | FSM-SPEC-SCI | Competitive analysis, research basis, innovation statement |
| 02 | `02-Language-Requirements.md` | FSM-REQ-LANG | Normative language and system requirements |
| 03 | `03-Infrastructure-Requirements.md` | FSM-REQ-INFRA | Toolchain, CLI, LSP, simulator, VS Code, packaging |
| 04 | `04-DSL-Specification.md` | FSM-SPEC-DSL | Full EBNF grammar, operator precedence, formal algorithm, industrial example |
| 05 | `05-UI-Specification.md` | FSM-SPEC-UI | VS Code extension + Web IDE — all panels, menus, interactions |
| 06 | `06-Style-Guide.md` | FSM-SPEC-STYLE | Color tokens, typography, component library, diagram shapes |
| 07 | `07-Specification-Roadmap.md` | FSM-SPEC-ROADMAP | Master index of all specification documents |
| 08 | `08-Formal-Execution-Semantics.md` | FSM-SPEC-SEM | RTC step algorithm, LCA, exit/entry order, history, timers |
| 09 | `09-Canonical-IR-Schema.md` | FSM-SPEC-IR | JSON Intermediate Representation — full schema reference |
| 10 | `10-Diagnostic-Code-Catalog.md` | FSM-SPEC-DIAG | All FSM-E/W/I/H codes with examples and fix suggestions |
| 11 | `11-Codegen-C99.md` | FSM-SPEC-GEN-C | Generated file layout, struct layout, dispatch strategies, HAL integration |
| 12 | `12-Codegen-CPP17.md` | FSM-SPEC-GEN-CPP | CRTP/vtable patterns, no-STL profile, Arduino integration |
| 13 | `13-Simulator-Protocol.md` | FSM-SPEC-SIM | JSON-RPC 2.0 protocol, all methods, notifications, multi-machine |
| 14 | `14-LSP-Capability-Spec.md` | FSM-SPEC-LSP | All LSP 3.17 capabilities, completion items, hover, code actions |
| 15 | `15-Conformance-Test-Suite.md` | FSM-SPEC-TEST | Test categories, trace format, coverage requirements, CI runner |
| 16 | `16-HAL-Specification.md` | FSM-SPEC-HAL | Clock HAL, assertion HAL, ISR safety, reference implementations |
| 17 | `17-Assembly-Integration.md` | FSM-SPEC-ASSY | AAPCS, AVR, RISC-V, MSP430 calling conventions; pure-assembly strategy |
| 18 | `18-CLI-Specification.md` | FSM-SPEC-CLI | All `fsm` subcommands, flags, exit codes, `fsm.toml` config |
| 19 | `19-Formatter-Specification.md` | FSM-SPEC-FMT | Canonical formatting rules, alignment algorithm, idempotency |
| 20 | `20-Architecture-Overview.md` | FSM-ARCH-OVERVIEW | Repository layout, crate internals, data flow, crate dependency graph, ADRs |
| 21 | `21-TextMate-Grammar.md` | FSM-SPEC-TM | `source.fsm` scope tree, all regex patterns, `language-configuration.json` |
| 22 | `22-VSCode-Extension-Manifest.md` | FSM-SPEC-VSCE | Full `package.json` contributions: commands, views, config, snippets, bundled binaries |
| 23 | `23-Developer-Onboarding.md` | FSM-DEV-ONBOARD | This document — tech stack, build commands, role guide |
| 24 | `24-Testing-Methodology.md` | FSM-DEV-TEST-METH | Test tiers, acceptance checklists, automation coverage, CI setup |

---

# 8. Development Workflow

Standard feature development cycle:

1. Write/modify a `.fsm` test case in `tests/conformance/` that exercises the new behavior
2. Update the relevant specification section in `docs/` to reflect the intended behavior
3. Implement in the appropriate Rust crate, respecting the dependency rules in §4
4. Run `cargo test -p <crate>` — unit and snapshot tests for the modified crate
5. Run `cargo run -p fsm-cli -- test tests/conformance/` — cross-crate conformance
6. Run `cargo run -p fsm-cli -- fmt --check examples/**/*.fsm` — formatter idempotency
7. Open VS Code with extension loaded — verify LSP behavior interactively
8. Submit PR — CI runs all steps above on linux, macos, and windows

---

# 9. What "Done" Looks Like

Observable outcomes that define completion for each major deliverable:

| Deliverable | Command | Expected Result |
|---|---|---|
| Compiler (clean file) | `fsm check motor.fsm` | Exit 0, no output |
| Compiler (broken file) | `fsm check broken.fsm` | Exit 1, Rust-style error with `-->` file:line:col |
| C99 codegen | `fsm generate --target c99 motor.fsm` | `Motor.h`, `Motor.c`, `Motor_impl.h`, `Motor_conf.h` in `generated/` |
| Formatter | `fsm fmt f.fsm && fsm fmt --check f.fsm` | Second invocation exits 0 |
| LSP hover | Hover over state name in VS Code | Hover card with doc comment + state kind |
| Diagram | Save `.fsm` file in VS Code | Diagram panel updates within 500ms |
| Simulator | Dispatch event in simulator panel | Active state highlighted in diagram |
| C99 compile | `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror Motor.c` | Exits 0 with zero warnings |
| C++17 compile | `g++ -std=c++17 -Wall -Wextra -Wpedantic -Werror -fno-rtti Motor.cpp` | Exits 0 with zero warnings |

---

*End of FSM-DEV-ONBOARD v1.0.0*
