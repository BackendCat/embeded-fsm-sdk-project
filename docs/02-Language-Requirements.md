# Embedded FSM SDK — Language Requirements

**Document ID:** FSM-REQ-LANG
**Version:** 4.0
**Status:** Normative Draft
**License:** MIT

---

## 0. Guiding Principle

> FSM-Lang describes **what** the machine does — its structure, wiring, and topology.
> C describes **how** — the logic inside guards and actions.
> This boundary is the product.

FSM-Lang is a **declarative structural DSL**. It is not a programming language.
It does not compute. It does not execute logic. It describes finite state machine
topology and wires it to externally defined behaviour.

---

## 1. Definitions

| Term | Definition |
|---|---|
| **FSM** | Finite State Machine. |
| **HSM** | Hierarchical State Machine (Harel statecharts). |
| **Composite State** | A state containing nested child states or regions. |
| **Orthogonal Region** | A named parallel partition of a composite state. |
| **Pseudo-State** | Structural node without runtime context: initial, final, choice, junction, history, fork, join. |
| **LCA** | Lowest Common Ancestor in the state hierarchy. |
| **Extern Guard** | A side-effect-free C function declared in the DSL and implemented by the user. |
| **Extern Action** | A C function declared in the DSL and called by the generated runtime. |
| **Inline FSM Statement** | A statement expressible directly in the DSL because it is a pure FSM semantic operation (`raise`, `send`, `defer`). |
| **Context** | A typed persistent data record associated with one machine instance. |
| **Payload** | Typed data fields attached to an event instance. |
| **Target Profile** | Named set of code generation and platform constraints. |
| **Canonical Model** | The versioned JSON representation of the semantic model, independent of layout. |
| **Diagnostic Code** | Stable alphanumeric identifier for a specific class of error or warning. |

---

## 2. Product Goals

### G1. Formal Determinism

- Transition selection MUST be deterministic for every conformant model.
- Ambiguity MUST be a compile-time error, never a runtime property.
- Hierarchical and parallel conflict resolution MUST be formally specified.
- No undefined behaviour MUST exist in any conformant model.

### G2. Embedded-Grade Safety

- Generated runtime MUST NOT require dynamic memory allocation.
- Event queue MUST be statically allocated and bounded.
- Execution steps per dispatch cycle MUST be bounded.
- Generated C MUST compile on any C99-conformant compiler with no OS dependencies.
- Worst-case memory footprint MUST be computable from the model at compile time.

### G3. Declarative Purity

- The DSL MUST describe machine structure, not implement logic.
- All computational behaviour (guard logic, action bodies) MUST be implemented
  in C and referenced by name in the DSL.
- The only statements expressible inline in the DSL are pure FSM-semantic operations:
  `raise`, `send`, and `defer`. These are structural, not computational.
- Guards MUST be either extern pure function references or restricted field comparisons
  (see Section 8). No arithmetic. No assignment. No side effects.

### G4. Industrial Completeness

FSM-Lang MUST support every construct required for production HSM modelling:

**Structural:**
- Hierarchical composite states (unlimited nesting)
- Named orthogonal regions (parallel execution)
- Submachine references (machine reuse as composite state)
- Entry points and exit points on composite states

**Pseudo-States:**
- Initial (per region)
- Final (triggers completion event)
- Shallow history
- Deep history
- Choice (dynamic guard evaluation)
- Junction (static guard evaluation)
- Fork (parallel entry)
- Join (parallel synchronisation)

**Transitions:**
- External (exits/enters source state)
- Internal (no configuration change)
- Local (targets descendant without re-entering source)
- Completion (triggered by final state)
- Timed — one-shot (`after`) and periodic (`every`)

**Event model:**
- Explicit typed events with payload fields
- Deferred events
- Raised events (`raise`)
- Scheduled events (`schedule` / `cancel`)
- Completion events (auto-generated)

**Guards and priority:**
- Extern pure function references
- Restricted field comparison expressions
- Explicit numeric priority

### G5. Dual Code Generation Target

- **C99** MUST be supported as the primary code generation target.
- **C++17** MUST be supported as the secondary target (class-based machine).
- Both targets MUST produce semantically identical behaviour.
- Target selection is per-machine via the `target` block.

### G6. Toolchain Completeness

```
Parse → Validate → Analyze → Compile → Simulate → Debug → Generate → Export
```

Each stage independently invocable. All share the canonical JSON model.

### G7. Single Source of Truth

- DSL MUST round-trip losslessly to and from the canonical JSON model.
- The canonical model MUST be independently versioned.
- All toolchain components MUST operate on the canonical model.

---

## 3. Structural Requirements

### 3.1 File Structure

- A file begins with `language fsm MAJOR.MINOR`.
- A file MAY declare `feature` flags, `const` values, `enum` types, `extern` functions,
  and `machine` definitions.
- Each file is a module. Module identity is the path relative to the project root.
- A module MAY import from other modules.
- Circular imports are `FSM-E0700`.

### 3.2 Machine

A machine MUST declare:
- Context (zero or more typed fields)
- Events (zero or more typed events with payload)
- Queue configuration
- State hierarchy (at least one state, one initial declaration)
- One or more target profiles

### 3.3 Hierarchical States

- Composite states MAY contain nested child states to any depth.
- A composite state with named `region` blocks is orthogonal.
- A composite state without `region` blocks is treated as a single implicit region.
- Each region MUST declare exactly one `initial` target.

### 3.4 Orthogonal Regions

- Regions are named; names MUST be unique within the machine.
- All active regions receive each dispatched event.
- Dispatch order across regions is lexicographic by region name unless overridden
  by explicit region priority.
- Cross-region transitions (except via fork/join) are `FSM-E0102`.

### 3.5 Submachine References

- A machine definition MAY be used by reference as a composite state (`state X is M`).
- Entry via initial state or named entry point. Exit via final state or named exit point.
- Submachine has its own separate context.
- Circular submachine references are `FSM-E0106`.

### 3.6 History

- **Shallow history** restores the most recently active direct child of the region.
- **Deep history** restores the most recently active leaf of the full subtree.
- A history pseudo-state MUST declare a default target (used if no history recorded).
- History pseudo-states SHOULD have `@id` annotations (compiler warns `FSM-H0001` otherwise).

### 3.7 Fork and Join

- A fork targets exactly one state per listed region simultaneously.
- A join completes when all listed source states have exited.
- Fork/join MUST be structurally matched. Mismatch is `FSM-E0107`.

---

## 4. Transition Requirements

### 4.1 Transition Types

| Type | Syntax | Description |
|---|---|---|
| External | `on E [g] -> T : a` | Exits source, runs action, enters target. |
| Internal | `on E [g] : a` | Runs action, no configuration change. |
| Local | `on E [g] ~> T : a` | Enters descendant T without re-entering source. |
| Completion | `done [g] -> T : a` | Triggered when region reaches final state. |
| Timed one-shot | `after N ms -> T : a` | Fires once after N ms from state entry. |
| Timed periodic | `every N ms : a` | Fires every N ms while in state. |

### 4.2 Guards

Guards are restricted boolean expressions. They MUST be side-effect free.
Guards MUST be evaluated before any exit actions execute.

Permitted guard forms:

```
[guard_name]                        extern pure function
[!guard_name]                       negation of extern pure function
[ctx.field op literal]              context field comparison
[payload.field op literal]          payload field comparison
[ctx.field op ctx.field2]           two context fields
[payload.field op ctx.field]        payload vs context
[g1 && g2]                          logical AND of two guard expressions
[g1 || g2]                          logical OR of two guard expressions
[(g)]                               grouping
```

Where `op` ∈ `{ ==, !=, <, >, <=, >= }` and `literal` is an integer literal, boolean
literal, or enum variant reference.

No arithmetic. No assignment. No function call in non-extern form.

### 4.3 Actions

Actions are attached to transitions, entry blocks, and exit blocks.
Actions MUST be one of:

| Form | Description |
|---|---|
| `fn_name` | Call declared extern function. |
| `fn_name(payload.f)` | Call extern with payload field as argument. |
| `raise EVENT` | Enqueue event to own queue (FSM-native). |
| `raise EVENT(expr)` | Enqueue event with literal or field payload. |
| `send EVENT to M` | Enqueue event to machine M's queue (FSM-native). |

Multiple actions: comma-separated list.

```
on LINK_DOWN -> Disconnected : close_connection, raise LINK_LOST
```

No assignment. No if/else. No arithmetic. No inline computation.

### 4.4 Priority

- `priority N` — lower N = higher priority. Default is `100`.
- Equal-priority transitions on same event with non-disjoint guards: `FSM-E0101`.
- Hierarchical priority: transitions on inner states take priority over outer states.

### 4.5 LCA Execution Order

External transition from S to T with LCA L:

1. Exit actions of S, then ancestors of S up to (not including) L. Innermost first.
2. Transition actions.
3. Entry actions from L down to T. Outermost first.
4. Enter initial substates of T if composite.

---

## 5. Event Model

### 5.1 Event Declaration

- Events MUST be explicitly declared.
- Events MAY have zero or more typed payload fields.
- Payload field types MUST be from the type system (Section 7).
- Payload is statically bounded (no dynamic fields).

### 5.2 Queue

- Bounded, statically allocated FIFO.
- Capacity: compile-time configurable.
- Overflow policy: `drop_oldest`, `drop_newest`, `assert`, `error`.
- Drain semantics: process one event per dispatch call (run-to-completion per event).

### 5.3 Deferred Events

- `defer EVENT` in state body: hold this event while in this state.
- Deferred events re-inserted at front of queue on transition to non-deferring state.
- Deferral cycle (event deferred in all reachable states): `FSM-E0400`.

### 5.4 Raised Events

- `raise EVENT` enqueues to the current machine's own queue.
- Raised events processed in subsequent dispatch cycles, not current.
- Bounded by queue capacity.

### 5.5 Scheduled Events

```
schedule EVENT after N ms as handle_name   // in action list (extern call form)
cancel handle_name
```

Scheduling and cancellation are expressed via dedicated extern functions declared
by the code generator:

```c
/* generated: */
void M_schedule_MY_EVENT(M_t *m, uint32_t delay_ms, M_timer_handle_t *h);
void M_cancel(M_t *m, M_timer_handle_t *h);
```

These are called from the user's extern action functions, not directly from the DSL.

### 5.6 Completion Events

- Auto-generated when a region's active state reaches a final state.
- Processed before externally queued events.
- Triggers `done` transitions on the enclosing composite state.

---

## 6. Timer Model

- `after N ms -> T` — one-shot. Starts on state entry. Cancelled on state exit.
- `every N ms : a` — periodic. Starts on state entry. Cancelled on state exit.
- `N` MUST be a compile-time constant (integer or `const` reference).
- Timer implementation is provided via a user-supplied HAL adapter.
- Virtual clock in simulator replaces wall time for deterministic replay.

---

## 7. Type System

The type system exists solely for two purposes:
1. Generating correct C struct layouts for context and payload.
2. Type-checking field comparison guards.

It is intentionally minimal.

### 7.1 Permitted Types

| FSM-Lang type | C99 type | Notes |
|---|---|---|
| `bool` | `bool` | |
| `u8` | `uint8_t` | |
| `u16` | `uint16_t` | |
| `u32` | `uint32_t` | |
| `u64` | `uint64_t` | |
| `i8` | `int8_t` | |
| `i16` | `int16_t` | |
| `i32` | `int32_t` | |
| `i64` | `int64_t` | |
| `f32` | `float` | Profile may forbid. |
| `f64` | `double` | Profile may forbid. |
| `enum EnumName` | `typedef enum` | Declared in module. |
| `opaque "C_type"` | verbatim C type | No field-comparison guards allowed. |

### 7.2 Enumeration Types

- Declared at module or machine scope.
- Values auto-assigned from 0, or explicitly assigned.
- Comparable with `==` and `!=` in guards.
- Generated as `typedef enum M_EnumName_t` in C.

### 7.3 No Other Types

- No arrays in context (use `opaque` with a C struct).
- No strings (use `opaque`).
- No pointers.
- No nested structs.

---

## 8. Extern Declaration

All functions used in guards or actions MUST be declared before use:

```
extern      fn_name(param_type param_name, ...) [ : return_type ]
pure extern fn_name(...)                         [ : return_type ]
```

- `extern` — may be called in action lists only.
- `pure extern` — may be called in guard expressions only. MUST be side-effect free.
- Return type omitted = `void`.
- Arguments to `pure extern` guards: the compiler passes `(ctx, payload)` automatically;
  explicit parameters are for documentation only and MUST match the generated signature.

---

## 9. Inline FSM Statements

The only statements expressible inline in the DSL (not extern calls):

### 9.1 Permitted in Action Blocks

| Statement | Description |
|---|---|
| `ctx.field = expression` | Assign value to a context field. |
| `fn(args)` | Call a declared extern function with any argument expressions. |
| `HAL_fn(args)` | Direct tier-1 call — emitted verbatim, args are opaque C tokens or expressions. |
| `if (guard) { } else { }` | Conditional action. Guard follows the same restricted guard grammar. |
| `while (guard) { }` | Loop. See §9.2. |
| `for (id = expr; guard; id = expr) { }` | Counted loop. See §9.2. |
| `raise EVENT` | Enqueue event to own queue (FSM-native). |
| `raise EVENT(args)` | Enqueue event with payload (FSM-native). |
| `send EVENT to M` | Enqueue event to machine M (FSM-native). |
| `defer EVENT` | Defer event while in this state (state body only). |

### 9.2 Loops

Loops are **permitted** in action blocks.

Rationale: prohibiting loops in the DSL while allowing `extern` calls (which may themselves contain loops) is a leaky abstraction that gives no real guarantee. The embedded safety contract (bounded execution, WCET) is the responsibility of the developer and the target toolchain, not the FSM compiler.

The compiler emits `FSM-W0200` (configurable, default: warning) when a loop appears in an action block, as a style nudge — complex looping logic is often clearer in a named extern function.

### 9.3 Expression Language in Actions

Expressions in action argument positions and assignments:

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Bitwise: `&`, `|`, `^`, `~`, `<<`, `>>`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!`
- Context fields: `ctx.field`
- Payload fields: `payload.field`
- Constants and enum variants
- Pure extern function calls (declared `pure extern`)
- Grouping: `()`

This is the **action expression language**. It is richer than the guard expression language (Section 8.2) because actions are not subject to static nondeterminism analysis.

### 9.4 Guard Expression Language (unchanged — restricted)

Guards remain a separate, strictly restricted sublanguage. No arithmetic assignment. No side effects. The compiler must be able to statically analyze guard pairs for overlap.

See Section 8.2 for the guard grammar.

---

## 10. Stable Node IDs

- Any state, region, pseudo-state, or event MAY carry `@id("string")`.
- Stable IDs are preserved in the canonical model across renames.
- Used as primary keys for: history storage, trace records, toolchain references.
- Duplicate `@id` within a machine: `FSM-E0750`.
- Missing `@id` on history pseudo-state: `FSM-H0001` (hint).

---

## 11. Module System

- `import "path/to/module.fsm" as Alias`
- `import "path/to/module.fsm" { EventA, EventB }`
- Imports resolve: event declarations, enum types, extern declarations, machine references.
- All imported identifiers carry their origin module in the canonical model.
- Circular imports: `FSM-E0700`.

---

## 12. Profile System

Profiles are named capability sets for code generation targets.

### 12.1 Built-in Profiles

| Profile | Description |
|---|---|
| `C99` | C99, no OS, no heap, no float by default. Primary target. |
| `C99-RTOS` | C99 with RTOS context. Allows ISR-safe queue variants. |
| `Cpp17` | C++17 class-based machine. Same semantics as C99. |
| `Simulation` | Full features. Virtual clock. Used by simulator. |

### 12.2 Profile Configuration Keys

| Key | Type | Description |
|---|---|---|
| `strategy` | `switch_based` \| `table_driven` | Code gen strategy. |
| `queue_capacity` | integer | Event queue depth. |
| `overflow` | overflow policy | Queue overflow behaviour. |
| `allow_float` | boolean | Whether `f32`/`f64` types are permitted. |
| `max_nesting` | integer | Maximum state nesting depth. |

---

## 13. Static Analysis Requirements

Every diagnostic MUST carry: stable code, severity, file, line, column, span, message.

### 13.1 Structural Errors

| Code | Condition |
|---|---|
| `FSM-E0001` | Region missing `initial`. |
| `FSM-E0002` | Region has multiple `initial`. |
| `FSM-E0003` | Duplicate name in scope. |
| `FSM-E0004` | Transition targets undefined state. |
| `FSM-E0005` | Region contains no states. |

### 13.2 Semantic Errors

| Code | Condition |
|---|---|
| `FSM-E0100` | Nondeterministic: unresolvable transition conflict. |
| `FSM-E0101` | Equal-priority transitions with non-disjoint guards. |
| `FSM-E0102` | Transition crosses region boundary illegally. |
| `FSM-E0103` | LCA computation failure (disconnected graph). |
| `FSM-E0110` | Local transition (`~>`) target is not a descendant. |
| `FSM-E0105` | Internal transition has an explicit target. |
| `FSM-E0106` | Circular submachine reference. |
| `FSM-E0107` | Fork/join structure mismatch. |

### 13.3 Type Errors

| Code | Condition |
|---|---|
| `FSM-E0300` | Type mismatch in guard comparison. |
| `FSM-E0301` | Undefined identifier in guard expression. |
| `FSM-E0302` | Non-`pure` extern used in guard. |
| `FSM-E0303` | `opaque` field used in field-comparison guard. |

### 13.4 Event Model Errors

| Code | Condition |
|---|---|
| `FSM-E0400` | Deferral cycle detected. |
| `FSM-E0401` | Undefined event referenced. |

### 13.5 Reachability Warnings

| Code | Condition |
|---|---|
| `FSM-W0001` | State unreachable from initial configuration. |
| `FSM-W0002` | Transition guard statically always false. |
| `FSM-W0003` | Final state has outgoing transitions. |
| `FSM-W0200` | Loop in action block. Consider extracting to a named extern function. |

### 13.6 Hints

| Code | Condition |
|---|---|
| `FSM-H0001` | History pseudo-state missing `@id`. |
| `FSM-H0002` | Composite state with history child missing `@id`. |

---

## 14. IR Requirements

- The compiler MUST emit a canonical JSON model from the DSL.
- JSON Schema MUST be published separately (`FSM-SPEC-IR`).
- Schema version embedded in every document.
- Same DSL input MUST produce identical IR across builds (deterministic).
- All nodes carry stable IDs and source location spans.

---

## 15. Code Generation Requirements

### 15.1 C99 Target

Generated files per machine `M`:

| File | Contents |
|---|---|
| `M.h` | Machine type, event type, dispatch prototype, init prototype. |
| `M.c` | Dispatch implementation, queue logic, timer callbacks. |
| `M_impl.h` | Extern function declarations (user implements these). |
| `M_conf.h` | Compile-time configuration (`#define` macros). |
| `M_test.c` | Optional: generated test harness covering all transitions. |

Generated dispatch function:

```c
void M_dispatch(M_t *m, const M_event_t *event);
void M_init(M_t *m);
void M_tick(M_t *m, uint32_t elapsed_ms);  /* for bare-metal timer integration */
```

### 15.2 C++17 Target

- Machine as a class: `class M`.
- User overrides virtual methods for extern actions.
- Or: non-virtual with function pointer table (configurable).
- Same dispatch semantics as C99.

### 15.3 Correctness Requirement

Generated code semantics MUST be verified against the simulator for every machine.
A generated test harness MUST exercise all transitions, all history paths, and all
parallel region combinations.

---

## 16. Simulation Requirements

See `FSM-REQ-INFRA` for full simulator specification.
Summary: WebSocket JSON-RPC 2.0 API, virtual clock, deterministic replay, step/run/pause,
event injection, context inspection, trace collection.

---

## 17. Conformance Criteria

A FSM-Lang implementation is conformant when:

1. All constructs in Section 2 (G4) are syntactically and semantically supported.
2. Execution semantics match the normative formal semantics (`FSM-SPEC-SEM`) exactly.
3. No undefined behaviour exists in any conformant model.
4. All diagnostics in Section 13 are reported with the specified codes.
5. IR round-trips without semantic loss.
6. Generated C99 and C++17 code compiles without warnings under:
   `gcc -std=c99 -Wall -Wextra -pedantic` and
   `clang++ -std=c++17 -Wall -Wextra -pedantic`.
7. Simulator produces a trace sufficient for deterministic replay.

---

## 18. Explicitly Out of Scope

- Probabilistic / stochastic state machines
- Distributed multi-node FSM synchronisation
- Formal model checking integration (future plugin)
- Non-deterministic automata
- Inline computation (arithmetic, assignment, if/else) in the DSL
- A general-purpose type system or expression language
