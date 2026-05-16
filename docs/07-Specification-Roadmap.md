# FSM Studio — Specification Roadmap

**Document ID:** FSM-SPEC-ROADMAP
**Version:** 1.0.0
**Status:** Living Document

This document lists every specification needed for the project to be fully implementable.
It tracks status and dependencies between documents.

---

## Current Status

> **Reconciled 2026-05-16 (post-v1.3).** The "Spec status" column is the
> document-authoring state (it was, and remains, "all written"). The
> **"Impl status"** column was added in the post-v1.3 doc-honesty pass so
> this index reflects *shipped reality*, not just doc completeness — and
> the table was extended past Doc 22 (Docs 23–30 existed but were unlisted;
> see "Planning & process docs of record" below). `docs/ROADMAP.md` +
> `CHANGELOG.md` remain the authoritative per-release source; this is a
> navigation index over it.

### Static specification corpus (Docs 01–25)

| ID | Document | Spec status | Impl status | Blocks |
|---|---|---|---|---|
| FSM-SPEC-SCI | 01 — Scientific Justification | ✅ Complete | Foundational | — |
| FSM-REQ-LANG | 02 — Language Requirements | ✅ Complete | Shipped v1.0 | FSM-SPEC-DSL |
| FSM-REQ-INFRA | 03 — Infrastructure Requirements | ✅ Complete | Shipped v1.0 (release-pipeline MUSTs unmet — G9 carry-over; see Doc 03 §3.5/§12.1) | All implementation |
| FSM-SPEC-DSL | 04 — DSL Specification (Grammar) | ✅ Complete | Shipped v1.0 | Parser, LSP |
| FSM-SPEC-UI | 05 — UI/UX Specification | ✅ Complete | Planned v1.6 (Web IDE) | VS Code ext, Web IDE |
| FSM-SPEC-STYLE | 06 — Style Guide & Design System | ✅ Complete | Shipped v1.3 (VS Code surfaces) | All UI work |
| FSM-SPEC-SEM | 08 — Formal Execution Semantics | ✅ Complete | Shipped v1.0 | Code gen, Simulator |
| FSM-SPEC-IR | 09 — Canonical JSON Model (IR Schema) | ✅ Complete | Shipped v1.0 | All toolchain |
| FSM-SPEC-DIAG | 10 — Diagnostic Code Catalog | ✅ Complete | Shipped v1.0 (live count 73) | Parser, LSP, CI |
| FSM-SPEC-GEN-C | 11 — C99 Code Generator Spec | ✅ Complete | Shipped v1.0 | Generated runtime |
| FSM-SPEC-GEN-CPP | 12 — C++17 Code Generator Spec | ✅ Complete | **Planned** — own unscheduled minor (post-v1.3, Doc 00 §11.40) | C++ target |
| FSM-SPEC-SIM | 13 — Simulator Protocol Spec | ✅ Complete | Spec normative; in-process simulator shipped v1.0; **WS server planned v1.5** (Doc 30 §2) | VS Code ext, Web IDE |
| FSM-SPEC-LSP | 14 — Language Server Capability Spec | ✅ Complete | **Shipped v1.2** (`fsm-lang-server`, single-file; cross-file v1.4) | LSP implementation |
| FSM-SPEC-TEST | 15 — Conformance Test Suite Spec | ✅ Complete | Shipped v1.0 (hardened v1.1) | QA, CI |
| FSM-SPEC-HAL | 16 — Hardware Abstraction Layer Spec | ✅ Complete | Shipped v1.0 | Generated runtime |
| FSM-SPEC-ASSY | 17 — Assembly Integration Strategy | ✅ Complete | Reference strategy — not a planned codegen target | Low-level targets |
| FSM-SPEC-CLI | 18 — CLI Specification | ✅ Complete | Shipped v1.0 (`simulate`/`lsp`/`ir`/`completions` spec'd but deliberately not exposed) | All CLI users |
| FSM-SPEC-FMT | 19 — Formatter Specification | ✅ Complete | Shipped v1.0 | `fsm fmt`, CI |
| FSM-ARCH-OVERVIEW | 20 — Internal Code Architecture Overview | ✅ Complete | Living (reconciled per release) | All contributors |
| FSM-SPEC-TM | 21 — TextMate Grammar Specification | ✅ Complete | Shipped v1.3 (VS Code ext) | VS Code ext, Web IDE |
| FSM-SPEC-VSCE | 22 — VS Code Extension Manifest Spec | ✅ Complete | **Shipped v1.3** | VS Code ext |
| FSM-ONBOARD | 23 — Developer Onboarding | ✅ Complete | Living | All contributors |
| FSM-TEST-METHOD | 24 — Testing Methodology | ✅ Complete | Living | QA, CI |
| FSM-INTEG-GUIDE | 25 — Integration Guide | ✅ Complete | Shipped v1.1 | Integrators |

### Planning & process docs of record (Docs 26–30)

Per the Doc 28 precedent, epic-kickoff *architecture extractions* + *wave
plans* are first-class numbered siblings, distinct from the static spec
corpus above and from the per-release `GATE_VERIFICATION_*` / `AUDIT_*`
evidence docs. They are listed here so this index is complete; they are
**not** normative specs.

| ID | Document | Role | Status |
|---|---|---|---|
| FSM-ARCH-LSP | 26 — LSP Architecture | LSP crate shape + wave plan (refines Doc 20 §9) | Of record; v1.2 delivered against it |
| FSM-ARCH-VSCE | 27 — VS Code Extension Architecture | Extension architecture (LSP client + Webview) | Of record; v1.3 delivered against it |
| FSM-PLAN-VSCE-V13 | 28 — v1.3 VS Code Wave Plan | V1–V6 epic plan + tag gate | Of record; v1.3 shipped |
| FSM-PLAN-W0-V13 | 29 — v1.3 W0 CST-Coupling Plan | The §11.49 analyzer→CST debt-paydown plan | Of record; W0 shipped (`ceb8efd`) |
| FSM-PLAN-SIMVER-V14 | 30 — v1.4 Simulation & Verification Wave Plan | v1.4 epic plan + ROADMAP↔shipped reconciliation | Of record; **v1.4 in progress** |

---

## Specification corpus complete; tracked against shipped reality

All 25 static specification documents (Docs 01–25) have been written; the
SDK is fully specifiable for implementation. Docs 26–30 are the planning &
process docs of record. **Doc-authoring completeness ≠ shipped state** —
the "Impl status" column above and `docs/ROADMAP.md` track what has
actually shipped per release. See the Implementation Dependency Graph
below for build order.

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
