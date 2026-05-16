# Embedded FSM Studio

Formal embedded-first DSL and toolchain for deterministic hierarchical finite state machines.

## What is this?

A production-grade platform for industrial FSM development.

### What ships today

The compiler core below shipped in **v1.0.0** and is stable across every
release since. The LSP server (v1.2.0) and VS Code extension (v1.3.0)
shipped on top of it — see [Project Status](#project-status) and
[`docs/ROADMAP.md`](docs/ROADMAP.md) for the per-release breakdown.

- **FSM-Lang DSL** — formal text grammar for hierarchical state machines (parser + analyzer + IR emitter)
- **C99 code generator** — deterministic, heap-free output safe for embedded targets.
  Two dispatch strategies via `fsm generate --strategy {switch,table,auto}`
  (`auto` picks `switch` when state count < 64, else `table`).
- **In-process simulator** — virtual-clock interpreter for trace-based testing.
  Emits `StepRecord` JSON (Doc 13 §11) — byte-deterministic. Runs as a
  library backing `fsm test` (no standalone `simulate` subcommand in
  v1.x — the WebSocket server is deferred to v1.5, Doc 13).
- **Canonical formatter** — `fsm fmt` for idempotent `.fsm` source layout.
- **LSP server** — `fsm-lang-server` (shipped v1.2.0; Docs 14/26).
- **VS Code extension** — syntax highlighting, LSP client, read-only ELK
  diagram panel, activity-bar views (shipped v1.3.0; Docs 21/22/27/28).
- **CLI** — `fsm check`, `fsm generate`, `fsm fmt`, `fsm test`, `fsm parse`,
  `fsm doc`, `fsm decompile`, `fsm init`.

### Generated code

Every emitted `.c` / `.h` carries an SPDX header (default `MIT`, override with
`fsm generate --license <SPDX>`). The user MUST supply the HAL contract:

```c
// fsm_hal.h provided by the user once per project
uint32_t fsm_hal_clock_now_ms(void);                     // wall-clock or virtual
void     fsm_hal_assert(const char *file, int line,      // assertion handler
                        const char *msg);
```

On Arduino:

```c
uint32_t fsm_hal_clock_now_ms(void) { return millis(); }
void fsm_hal_assert(const char *f, int l, const char *m) {
    (void)f; (void)l; (void)m; for (;;) {}
}
```

The HAL is **mandatory** in v1.0 (Doc 00 §10.3): codegen unconditionally emits
`#include "fsm_hal.h"`. Trivial integration is ≤ 10 lines per project.

### Roadmap (post-v1.3)

`docs/ROADMAP.md` is the live, per-release source of truth — this list is a
pointer, not a duplicate.

Shipped since v1.0 (tagged, local):

- **LSP server** (`fsm-lang-server`, Docs 14/26) — shipped in **v1.2.0**
- **VS Code extension** (Docs 21/22/27) — shipped in **v1.3.0**
- **`defer EVENT` runtime support** — shipped in **v1.1.0** (the v1.0
  `FSM-E0903` hard-fail was removed)

Not yet shipped (current deferred set, per `docs/ROADMAP.md` + Doc 30):

- **`fsm verify` verification core** — bounded reachability + deadlock
  detection + trace differential replay — **v1.4, in progress**
- **Simulator WebSocket / JSON-RPC server** (Doc 13) — deferred to **v1.5**
- **Web IDE** — browser editor + diagram + `wasm32` toolchain (Doc 05) —
  deferred to **v1.6**
- **C++17 code generator** (Doc 12) — its own unscheduled minor between
  v1.3 and v2.0

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
| [25 — Integration Guide](docs/25-Integration-Guide.md) | Generated-C ABI, C/C++/Rust recipes, Make/CMake/PlatformIO fragments, troubleshooting; worked examples in [`examples/integration/`](examples/integration/) |

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

<a id="project-status"></a>

## Project Status

**Latest tagged release: `v1.3.0` (2026-05-16, local). v1.4 in progress.**

All tags below are annotated and **local** (the remote/CI is owner-controlled;
see `docs/GATE_VERIFICATION_v1_*.md` §"owner action"). `docs/ROADMAP.md` is
the authoritative, per-release source — this is a summary of it.

| Release | Theme | Status |
|---|---|---|
| `v1.0.0` | Correct, tested, embedded-ready core (DSL → C99 → CLI + in-process simulator + formatter) | ✅ Shipped (tagged, local) |
| `v1.1.0` | Integration ergonomics + UML completion (`defer`, submachines, `--import-header`) | ✅ Shipped (tagged, local) |
| `v1.2.0` | LSP server (`fsm-lang-server`, Docs 14/26) | ✅ Shipped (tagged, local) |
| `v1.3.0` | VS Code extension (Docs 21/22/27/28) | ✅ Shipped (tagged, local) |
| `v1.4`   | Verification core — `fsm verify` (bounded reachability + deadlock) + trace differential replay (Doc 30) | 🚧 In progress |

- **Deferred to a later minor** (per `docs/ROADMAP.md` + Doc 30, owner-confirmed):
  Simulator WebSocket / JSON-RPC server → **v1.5**; Web IDE + `wasm32`
  toolchain (Doc 05) → **v1.6**; C++17 codegen (Doc 12) → its own
  unscheduled minor between v1.3 and v2.0.
- **Why X?** — read [`docs/00-Decisions-And-Reconciliation.md`](docs/00-Decisions-And-Reconciliation.md).
  The 14 blocker resolutions (§2), TL escalation decisions (§10), and the
  implementation-time decisions table (§11) cover every "why is it
  this way" question.
- **What changed when?** — see [`CHANGELOG.md`](CHANGELOG.md) (per-release
  entries) and the `docs/GATE_VERIFICATION_v1_*.md` gate-evidence docs.
- **Developer entry:** [`docs/23-Developer-Onboarding.md`](docs/23-Developer-Onboarding.md).
