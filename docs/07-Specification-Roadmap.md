# FSM Studio — Specification Roadmap

**Document ID:** FSM-SPEC-ROADMAP
**Version:** 1.0.0
**Status:** Living Document

This document lists every specification needed for the project to be fully implementable.
It tracks status and dependencies between documents.

---

## Current Status

| ID | Document | Status | Blocks |
|---|---|---|---|
| FSM-SPEC-SCI | 01 — Scientific Justification | ✅ Complete | — |
| FSM-REQ-LANG | 02 — Language Requirements | ✅ Complete | FSM-SPEC-DSL |
| FSM-REQ-INFRA | 03 — Infrastructure Requirements | ✅ Complete | All implementation |
| FSM-SPEC-DSL | 04 — DSL Specification (Grammar) | ✅ Complete | Parser, LSP |
| FSM-SPEC-UI | 05 — UI/UX Specification | ✅ Complete | VS Code ext, Web IDE |
| FSM-SPEC-STYLE | 06 — Style Guide & Design System | ✅ Complete | All UI work |
| FSM-SPEC-SEM | 08 — Formal Execution Semantics | ✅ Complete | Code gen, Simulator |
| FSM-SPEC-IR | 09 — Canonical JSON Model (IR Schema) | ✅ Complete | All toolchain |
| FSM-SPEC-DIAG | 10 — Diagnostic Code Catalog | ✅ Complete | Parser, LSP, CI |
| FSM-SPEC-GEN-C | 11 — C99 Code Generator Spec | ✅ Complete | Generated runtime |
| FSM-SPEC-GEN-CPP | 12 — C++17 Code Generator Spec | ✅ Complete | C++ target |
| FSM-SPEC-SIM | 13 — Simulator Protocol Spec | ✅ Complete | VS Code ext, Web IDE |
| FSM-SPEC-LSP | 14 — Language Server Capability Spec | ✅ Complete | LSP implementation |
| FSM-SPEC-TEST | 15 — Conformance Test Suite Spec | ✅ Complete | QA, CI |
| FSM-SPEC-HAL | 16 — Hardware Abstraction Layer Spec | ✅ Complete | Generated runtime |
| FSM-SPEC-ASSY | 17 — Assembly Integration Strategy | ✅ Complete | Low-level targets |
| FSM-SPEC-CLI | 18 — CLI Specification | ✅ Complete | All CLI users |
| FSM-SPEC-FMT | 19 — Formatter Specification | ✅ Complete | `fsm fmt`, CI |
| FSM-ARCH-OVERVIEW | 20 — Internal Code Architecture Overview | ✅ Complete | All contributors |
| FSM-SPEC-TM | 21 — TextMate Grammar Specification | ✅ Complete | VS Code ext, Web IDE |
| FSM-SPEC-VSCE | 22 — VS Code Extension Manifest Spec | ✅ Complete | VS Code ext |

---

## All Specifications Complete

All 22 specification documents have been written. The FSM Studio SDK is fully specifiable
for implementation. See the Implementation Dependency Graph below for build order.

---

## Recommended Writing Order

```
1. FSM-SPEC-SEM     ← Foundation for everything else
2. FSM-SPEC-IR      ← Schema needed by all toolchain components
3. FSM-SPEC-DIAG    ← Needed for LSP and parser implementation
4. FSM-SPEC-HAL     ← Needed before code gen can be tested on hardware
5. FSM-SPEC-GEN-C   ← Primary product deliverable
6. FSM-SPEC-SIM     ← Needed for VS Code extension and Web IDE
7. FSM-SPEC-LSP     ← Needed for editor integration
8. FSM-SPEC-GEN-CPP ← Secondary code gen target
9. FSM-SPEC-TEST    ← Can be written in parallel with implementation
10. FSM-SPEC-ASSY   ← Last, lowest priority
```

---

## Implementation Dependency Graph

```
FSM-SPEC-SEM ──────────────────────────────────────────────┐
FSM-SPEC-IR  ──────────────┐                               │
FSM-SPEC-DIAG ─────────────┤                               │
                            ▼                               ▼
FSM-SPEC-DSL ──────► Parser ──► Validator ──► Analyzer ──► IR
                                                            │
                    ┌───────────────────────────────────────┤
                    ▼                   ▼                   ▼
            FSM-SPEC-GEN-C      FSM-SPEC-SIM       FSM-SPEC-LSP
                    │                   │                   │
                    ▼                   ▼                   ▼
           C99 Code Output      WebSocket API        VS Code LSP
                    │                   │                   │
            FSM-SPEC-HAL       FSM-SPEC-UI           FSM-SPEC-UI
                    │                   │                   │
                    ▼                   ▼                   ▼
            Hardware Target    VS Code Simulator     Diagram Panel
```

---

## Document Template

All specification documents MUST follow this header:

```markdown
# FSM Studio — [Document Title]

**Document ID:** FSM-SPEC-XXX
**Version:** 1.0.0
**Status:** Draft | Normative Draft | Normative | Deprecated
**Depends on:** [list of prerequisite document IDs]

[one-paragraph summary]

---
```

All normative requirements use RFC 2119 language: MUST, MUST NOT, SHOULD, MAY.
