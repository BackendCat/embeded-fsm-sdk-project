# Embedded FSM Studio

Formal embedded-first DSL and toolchain for deterministic hierarchical finite state machines.

## What is this?

A production-grade platform for industrial FSM development:

- **FSM-Lang** — a formal text DSL with EBNF grammar and normative execution semantics
- **Compiler** — parser, semantic validator, static analyzer, IR emitter
- **Code generator** — deterministic C99 output, switch-based and table-driven strategies
- **Simulator** — WebSocket-based, virtual clock, deterministic replay
- **VS Code extension** — syntax highlighting, LSP, live diagram panel, simulator panel
- **Web IDE** — browser-based editor, diagram, and simulator

## Getting Started

See [23 — Developer Onboarding](docs/23-Developer-Onboarding.md) for the full tech stack,
build commands, repository layout, and role-based reading guides.

## Documentation

### Foundations

| Document | Description |
|---|---|
| [01 — Scientific Justification](docs/01-Scientific-Justification.md) | Competitive analysis, research basis, innovation statement |
| [02 — Language Requirements](docs/02-Language-Requirements.md) | Normative language and system requirements |
| [03 — Infrastructure Requirements](docs/03-Infrastructure-Requirements.md) | Toolchain, CLI, LSP, simulator, VS Code, packaging |
| [04 — DSL Specification](docs/04-DSL-Specification.md) | Full EBNF grammar, formal algorithm, industrial example |
| [05 — UI/UX Specification](docs/05-UI-Specification.md) | VS Code extension + Web IDE — all panels, menus, interactions |
| [06 — Style Guide & Design System](docs/06-Style-Guide.md) | Color tokens, typography, component library, diagram shapes |
| [07 — Specification Roadmap](docs/07-Specification-Roadmap.md) | Master index of all specification documents |

### Execution & Semantics

| Document | Description |
|---|---|
| [08 — Formal Execution Semantics](docs/08-Formal-Execution-Semantics.md) | RTC step algorithm, LCA, exit/entry order, history, timers |
| [09 — Canonical IR Schema](docs/09-Canonical-IR-Schema.md) | JSON Intermediate Representation — full schema reference |
| [10 — Diagnostic Code Catalog](docs/10-Diagnostic-Code-Catalog.md) | All FSM-E/W/I/H codes with examples and fix suggestions |

### Code Generators

| Document | Description |
|---|---|
| [11 — C99 Code Generator Spec](docs/11-Codegen-C99.md) | Generated file layout, struct layout, dispatch strategies, HAL integration |
| [12 — C++17 Code Generator Spec](docs/12-Codegen-CPP17.md) | CRTP/vtable patterns, no-STL profile, Arduino integration |
| [16 — HAL Specification](docs/16-HAL-Specification.md) | Clock HAL, assertion HAL, ISR safety, reference implementations |
| [17 — Assembly Integration](docs/17-Assembly-Integration.md) | AAPCS, AVR, RISC-V, MSP430 calling conventions; pure-assembly strategy |

### Toolchain & Protocols

| Document | Description |
|---|---|
| [13 — Simulator WebSocket Protocol](docs/13-Simulator-Protocol.md) | JSON-RPC 2.0 protocol, all methods, notifications, multi-machine |
| [14 — LSP Capability Spec](docs/14-LSP-Capability-Spec.md) | All LSP 3.17 capabilities, completion items, hover, code actions |
| [15 — Conformance Test Suite](docs/15-Conformance-Test-Suite.md) | Test categories, trace format, coverage requirements, CI runner |
| [18 — CLI Specification](docs/18-CLI-Specification.md) | All `fsm` subcommands, flags, exit codes, `fsm.toml` config |
| [19 — Formatter Specification](docs/19-Formatter-Specification.md) | Canonical formatting rules, alignment algorithm, idempotency |

### Editor Integration

| Document | Description |
|---|---|
| [21 — TextMate Grammar](docs/21-TextMate-Grammar.md) | `source.fsm` scope tree, all regex patterns, `language-configuration.json` |
| [22 — VS Code Extension Manifest](docs/22-VSCode-Extension-Manifest.md) | Full `package.json` contributions: commands, views, config, snippets, bundled binaries |

### Architecture & Development

| Document | Description |
|---|---|
| [20 — Internal Code Architecture](docs/20-Architecture-Overview.md) | Repository layout, crate internals, data flow, crate dependency graph, ADRs |
| [23 — Developer Onboarding](docs/23-Developer-Onboarding.md) | Tech stack, build commands, role-based reading guide |
| [24 — Testing Methodology](docs/24-Testing-Methodology.md) | Test tiers, acceptance checklists, automation coverage, CI setup |

## Key Properties

- **Deterministic** — compile-time nondeterminism detection; no runtime ambiguity
- **Embedded-safe** — zero heap allocation, bounded queue, bounded execution per step
- **Industrially complete** — all UML statechart constructs including parallel regions,
  history, deferred events, submachines, fork/join, entry/exit points
- **Open source** — MIT license (specification + reference implementation)

## File Extension

FSM-Lang source files use the `.fsm` extension.

## License

MIT
