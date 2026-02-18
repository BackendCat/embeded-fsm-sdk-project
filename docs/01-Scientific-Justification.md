# Scientific and Technical Justification for the Embedded FSM SDK

## Abstract

This document justifies the development of a next-generation domain-specific language (DSL),
compiler toolchain, runtime SDK, and integrated development environment for industrial hierarchical
finite state machines (HSM) targeting embedded systems and industrial automation controllers.

Existing solutions — including UML Statecharts, SCXML, QP/C, Stateflow, StateSmith,
YAKINDU/Itemis CREATE, XState, and Boost.Statechart — are analyzed with respect to
determinism guarantees, formal execution semantics, resource predictability, static analysis
capabilities, and embedded toolchain integration.

Analysis demonstrates that no existing open-source solution combines all of the following in a
single cohesive platform: a formally specified text-based DSL, compile-time nondeterminism
detection, embedded-safe execution (zero mandatory heap allocation), a unified simulation and
debug protocol, and a first-class IDE integration covering the full development lifecycle.

The proposed **Embedded FSM SDK** addresses this gap and establishes a foundation for an
industrial-grade, deterministic, embedded-first state machine development platform licensed
under MIT.

**Keywords:** finite state machines, hierarchical state machines, DSL, embedded systems,
formal semantics, static analysis, code generation, determinism, statecharts.

---

## 1. Introduction

Finite state machines and their hierarchical extensions (HSMs, statecharts) represent a
fundamental computational model for discrete control systems. They are applied in:

- Industrial automation controllers (PLCs, motion controllers, process controllers)
- Communication protocol implementations (USB, CAN, BLE, Ethernet stacks)
- Automotive safety systems (AUTOSAR, ISO 26262)
- Aerospace and defense embedded systems (DO-178C)
- Medical device firmware (IEC 62304)
- Consumer and IoT embedded firmware

Despite this pervasiveness, FSM implementations in production embedded environments are
predominantly written as ad-hoc imperative code: nested switch statements, flag registers,
and manual transition tables. This practice introduces several systematic risks:

1. **Undocumented implicit semantics** — entry/exit ordering, event priority, and guard
   evaluation order are implicit in code structure rather than formally specified.
2. **Non-reproducible nondeterminism** — conflicting transitions and ambiguous guard
   sets may produce different behaviors across compiler versions or optimization levels.
3. **Untestable logic** — state machines embedded in imperative code resist systematic
   coverage analysis and replay-based debugging.
4. **Maintenance fragility** — changes to one state silently invalidate assumptions in others.

Graphical modeling tools exist but are either visualization-only (PlantUML), proprietary
and runtime-heavy (Stateflow, Rhapsody), or not formally specified (UML Statecharts).
Text-based code generators exist (StateSmith, SMC) but lack formal semantics and simulation.

A purpose-built DSL with a formally defined execution model, static analysis, and a complete
development toolchain is therefore required to fill this gap in the open-source ecosystem.

---

## 2. Analysis of Existing Solutions

### 2.1 UML Statecharts (OMG UML 2.x)

The Object Management Group's UML 2.x specification defines statechart notation, including
hierarchical states, orthogonal regions, history pseudo-states, and entry/exit points.
However:

- The UML specification does not define a normative execution algorithm. Different tools
  implement different conflict resolution strategies.
- The textual interchange format (XMI) is verbose XML and not human-editable.
- No tool provides compile-time verification of determinism.
- Code generation quality varies significantly between tool vendors.

**Verdict:** Notation standard without execution standard. Not suitable as a formal foundation.

### 2.2 SCXML (W3C Recommendation, 2015)

The State Chart XML standard defines a formal execution algorithm (the SCXML algorithm)
based on document-order priority. It provides:

- A normative transition selection algorithm.
- Event queue and invoke semantics.
- ECMAScript or XPath as data and guard languages.

However, SCXML is fundamentally oriented toward browser and server runtimes:

- ECMAScript and XPath require large runtime environments.
- The XML format is not suitable for embedded source control workflows.
- The SCXML algorithm is not bounded in execution steps per event cycle.
- No static analysis tooling exists for SCXML documents.
- No embedded C code generators exist that preserve full SCXML semantics.

**Verdict:** Formal and well-specified, but inherently incompatible with embedded constraints.

### 2.3 QP/C and QP/C++ (Quantum Platform, Quantum Leaps)

QP/C is the most widely deployed embedded HSM framework and is based on Miro Samek's
"Practical UML Statecharts in C/C++" methodology. It provides:

- A proven hierarchical event-driven execution model.
- Active object pattern with bounded event queues.
- Production-tested on bare-metal, RTOS, and Linux targets.
- Extensive community documentation.

However, QP has significant limitations as a DSL-based platform:

- No DSL exists. State machines are expressed as C macros and function pointers.
- No code generation — developers write the state machine manually in C.
- No static analysis of state structure or transition determinism.
- No simulation, replay, or step-debug capability in the IDE.
- Visual tooling (QM tool) is proprietary and limited to diagram editing.
- The QM XML format is diagram-layout-coupled, not a semantic model.

**Verdict:** Excellent framework, but not a DSL-based system. No static guarantees.

### 2.4 StateSmith (Open Source, 2022–present)

StateSmith is an emerging open-source code generator targeting C/C++. It accepts PlantUML
or draw.io diagrams as input and produces readable C code. It has gained community traction:

- Open source (MIT license).
- Produces compact, readable C state machine code.
- Actively maintained with growing adoption in embedded communities.

Limitations:

- Input is diagram-based, not a formal text DSL. No grammar to analyze.
- PlantUML and draw.io do not define formal execution semantics.
- No support for orthogonal/parallel regions.
- No static analysis of guard conditions, priority conflicts, or reachability.
- No simulator or replay mechanism.
- No VS Code extension or IDE integration.
- No formal specification document.

**Verdict:** Good code generator, but lacks formal semantics, parallel regions, static
analysis, simulation, and IDE integration. Closest open-source competitor but incomplete.

### 2.5 YAKINDU Statechart Tools / Itemis CREATE

YAKINDU (now Itemis CREATE) is an Eclipse-based commercial tool with a text DSL for
statecharts. It generates C, C++, Java, and other targets.

- Has a formal text DSL with syntax highlighting and code completion.
- Supports hierarchical states and some static checks.
- Has a graphical editor linked to the DSL.

Limitations:

- Commercial product with licensing cost.
- Eclipse-based, not VS Code native.
- No formal execution semantics document.
- Limited parallel region support.
- Limited embedded static analysis (no bounded memory analysis).
- No WebSocket-based simulation protocol.
- Not focused on embedded-safety (heap allocation permitted).

**Verdict:** Closest feature parity in some areas, but commercial, Eclipse-based, and not
formally specified for embedded-safety.

### 2.6 MATLAB/Simulink Stateflow

Stateflow is the industry-standard hierarchical state machine tool in model-based design
for automotive (AUTOSAR), aerospace, and industrial automation:

- Full hierarchical statechart support including parallel regions, history, junctions.
- Embedded C code generation (via Simulink Coder / Embedded Coder).
- MATLAB scripting as action language.
- Model checking integration.
- Industry adoption in safety-critical systems.

Limitations:

- Requires MATLAB license (extremely expensive for small teams and open source).
- Graphical-only input — no text DSL.
- MATLAB runtime dependencies for simulation.
- Not suitable for general open-source embedded development.
- Closed ecosystem.

**Verdict:** Gold standard for commercial automotive/aerospace. Completely inaccessible for
open-source, cost-sensitive, or non-MATLAB environments.

### 2.7 IBM Rhapsody

Rhapsody is an IBM product based on UML Statecharts with C/C++ code generation:

- Full UML statechart feature set including parallel regions and history.
- Enterprise-grade modeling with design patterns.
- Used in aerospace, defense, and automotive.

Limitations:

- Commercial and expensive.
- UML notation — no formal text DSL.
- Runtime framework dependencies.
- Not embedded-first (assumes RTOS or OS).
- Closed source.

**Verdict:** Enterprise tool, not applicable to open-source embedded projects.

### 2.8 Boost.Statechart and Boost.MSM

Boost provides two C++ template-library FSM frameworks:

- Boost.Statechart: Runtime-polymorphic, hierarchical, event-driven.
- Boost.MSM: Compile-time-checked, table-driven, extremely fast.

Both are C++ library solutions — not DSL-based. They require C++ knowledge, provide no
visual tooling, no simulation, no replay, and are limited to C++ targets.

**Verdict:** Powerful C++ libraries, but not a DSL platform.

### 2.9 SMC (State Machine Compiler)

SMC is one of the oldest text-based FSM compilers (since the 1990s). It accepts a
custom text DSL and generates code in multiple languages including C.

- Text-based DSL (not diagram-based).
- Multi-language output.

Limitations:

- No hierarchical state machine support.
- No orthogonal regions.
- No formal execution semantics.
- No static analysis.
- No VS Code integration.
- Effectively unmaintained.

**Verdict:** Historical reference. Does not meet modern embedded HSM requirements.

### 2.10 BridgePoint (xtUML)

BridgePoint is an open-source UML tool supporting executable UML (xtUML / Action Language):

- Full UML class and state machine modeling.
- OAL (Object Action Language) for actions.
- Translates to C.

Limitations:

- Full UML complexity — heavyweight for embedded use.
- Requires extensive model infrastructure (class diagrams, components).
- Not embedded-safety-focused.
- Eclipse-based.

**Verdict:** Full-stack model-driven engineering, not a focused embedded FSM tool.

---

## 3. Identified Gaps in the Existing Ecosystem

Systematic analysis of the existing tools reveals the following unresolved gaps:

| Requirement | QP/C | StateSmith | YAKINDU | Stateflow | SCXML |
|---|---|---|---|---|---|
| Open source (MIT) | ✓ | ✓ | ✗ | ✗ | N/A |
| Text-based DSL | ✗ | ✗ | ✓ | ✗ | partial |
| Formal execution semantics | ✗ | ✗ | ✗ | partial | ✓ |
| Parallel/orthogonal regions | ✓ | ✗ | ✓ | ✓ | ✓ |
| Compile-time determinism check | ✗ | ✗ | partial | ✗ | ✗ |
| Static guard overlap detection | ✗ | ✗ | ✗ | ✗ | ✗ |
| Embedded-safe (no heap) | ✓ | ✓ | partial | ✗ | ✗ |
| Bounded execution per step | ✓ | ✓ | ✗ | ✗ | ✗ |
| Simulator with replay | ✗ | ✗ | partial | ✓ | ✗ |
| WebSocket simulation protocol | ✗ | ✗ | ✗ | ✗ | ✗ |
| VS Code extension | ✗ | ✗ | ✗ | ✗ | ✗ |
| Canonical JSON model | ✗ | ✗ | ✗ | ✗ | ✗ |
| Stable diagnostic codes | ✗ | ✗ | ✗ | ✗ | ✗ |
| Deferral cycle detection | ✗ | ✗ | ✗ | ✗ | ✗ |

No existing tool covers all requirements simultaneously.

---

## 4. Innovation Statement

The Embedded FSM SDK is distinguished from all existing solutions by combining the following
capabilities in a single open-source platform:

### 4.1 Formally Specified Text DSL

FSM-Lang is a purpose-built textual DSL with a formal EBNF grammar and a normative execution
semantics document. Unlike PlantUML or draw.io diagrams, FSM-Lang files are:

- Human-readable and diffable in version control.
- Formally parseable by a reference parser with a published grammar.
- Round-trippable to a canonical semantic model (JSON) without information loss.

### 4.2 Compile-Time Determinism Guarantee

The compiler statically detects all sources of nondeterminism:

- Guard overlap between transitions on the same event.
- Priority conflicts (equal priorities on non-disjoint guards).
- Ambiguous transition candidates in hierarchical configurations.

No other open-source tool provides this guarantee.

### 4.3 Embedded-Safety by Design

Unlike SCXML or Stateflow, embedded-safety is not an afterthought — it is a primary design
constraint from the specification level:

- Zero mandatory heap allocation.
- Bounded event queue with configurable overflow policy.
- Bounded execution steps per dispatch cycle.
- Static memory layout for context and configuration.

### 4.4 Unified Development Lifecycle

The SDK covers the complete development lifecycle as a single coherent platform:

```
Parse → Validate → Analyze → Compile → Simulate → Debug → Generate → Export
```

This eliminates the fragmentation between separate modeling, code generation, and debug tools.

### 4.5 WebSocket Simulation Protocol

The simulator exposes a formal WebSocket protocol for event injection, state inspection,
trace collection, and deterministic replay. This enables:

- IDE-integrated simulation (VS Code WebView panel).
- Hardware-in-the-Loop (HiL) testing by connecting hardware to the simulator.
- CI/CD pipeline integration via the WebSocket API.
- Third-party tool integration.

### 4.6 First-Class VS Code Integration

The VS Code extension provides:

- Full syntax highlighting via TextMate grammar for `.fsm` files.
- Language Server Protocol (LSP) integration: diagnostics, hover, completion, go-to-definition.
- Live state diagram WebView panel.
- Integrated simulator panel with event injection and state inspection.
- Diagnostic codes navigable from Problems panel.

### 4.7 Canonical JSON Model as Interoperability Hub

All toolchain components operate on a canonical JSON representation of the semantic model.
This enables external tools (linters, documentation generators, custom code generators) to
integrate without re-parsing the DSL.

---

## 5. Motivation for SDK Architecture (Not Just a Language)

A DSL specification alone is insufficient to achieve industrial adoption. The following
observations motivate the full SDK architecture:

**Observation 1:** Industrial embedded teams adopt tooling packages, not language specifications.
A DSL without a VS Code extension, a working parser, and a code generator has no adoption path.

**Observation 2:** Correctness assurance requires simulation and replay. A DSL compiler that
only generates code leaves verification to manual testing. The simulator closes the
verification gap without requiring hardware.

**Observation 3:** Static analysis value is proportional to integration. Diagnostic codes
displayed in the IDE at edit time — before compilation — have 10× the impact of
post-compilation error messages.

**Observation 4:** Industrial teams require documentation integration. The canonical JSON
model enables documentation generators, compliance report generators, and model reviewers
to operate on the semantic model without parsing FSM-Lang.

**Observation 5:** Hardware-in-the-Loop testing is a standard industrial practice. The
WebSocket simulation protocol makes the simulator a first-class HiL integration point,
enabling automated regression of state machine logic against hardware.

These observations collectively justify a full SDK rather than a standalone DSL specification.

---

## 6. Standards and Methodological References

The following standards, methodologies, and publications inform the design:

1. **Harel, D. (1987).** "Statecharts: A Visual Formalism for Complex Systems."
   *Science of Computer Programming*, 8(3), 231–274.
   — Foundational reference for hierarchical statechart semantics.

2. **OMG UML 2.5.1 Specification (2017).** Object Management Group.
   — Defines UML state machine notation (normative reference for notation).

3. **W3C SCXML Recommendation (2015).** "State Chart XML: State Machine Notation for
   Control Abstraction." W3C.
   — Defines the normative SCXML algorithm (reference for transition resolution semantics).

4. **Samek, M. (2008).** *Practical UML Statecharts in C/C++: Event-Driven Programming for
   Embedded Systems.* Newnes/Elsevier.
   — Standard reference for embedded HSM implementation patterns.

5. **IEC 61131-3 (2013).** "Programmable Controllers — Part 3: Programming Languages."
   — Defines SFC (Sequential Function Charts) as an industry reference for FSM specification
   in industrial automation.

6. **ISO 26262 (2018).** "Road Vehicles — Functional Safety."
   — Automotive safety standard requiring deterministic and verifiable state machine behavior.

7. **DO-178C (2011).** "Software Considerations in Airborne Systems and Equipment
   Certification." RTCA.
   — Aviation software standard requiring formal specification and verification.

8. **IEC 62304 (2006/2015).** "Medical Device Software — Software Life Cycle Processes."
   — Medical device software standard requiring documented and testable state machine logic.

9. **Fowler, M. (2010).** *Domain-Specific Languages.* Addison-Wesley.
   — Methodology reference for DSL design.

10. **Watt, D.A. (2004).** *Programming Language Design Concepts.* Wiley.
    — Type system and expression language design reference.

---

## 7. Conclusion

The conducted analysis demonstrates that no existing open-source solution simultaneously
provides:

- A formally specified text-based DSL for hierarchical state machines.
- Compile-time nondeterminism detection.
- Embedded-safe execution semantics (no heap, bounded execution).
- A simulator with a formal WebSocket protocol.
- First-class VS Code integration with live diagram visualization.
- A canonical JSON model for toolchain interoperability.

The Embedded FSM SDK fills this gap and provides a complete industrial-grade platform
that is uniquely positioned at the intersection of formal methods, embedded systems
engineering, and modern developer tooling.

The platform's open-source (MIT) licensing makes it accessible to academic research,
commercial embedded development, safety-critical industries, and the open-source community
simultaneously.
