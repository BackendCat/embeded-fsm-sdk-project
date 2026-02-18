# FSM Studio — Diagnostic Code Catalog

**Document ID:** FSM-SPEC-DIAG
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL

Complete listing of all diagnostic codes emitted by the FSM-Lang compiler and toolchain.
Codes are permanent — once assigned, a code number is never reused, even if the
diagnostic is retired. Retired codes are listed with status "Deprecated".

---

# 1. Code Format

```
FSM-[SEVERITY][NNNN]
```

| Severity | Prefix | Meaning |
|---|---|---|
| Error | `E` | Compilation cannot succeed; code generation blocked |
| Warning | `W` | Compilation continues; output may have quality issues |
| Info | `I` | Informational; no action required |
| Hint | `H` | Editor-only suggestion; never shown in CLI output by default |

Errors may be **recoverable** (compilation continues and further diagnostics are
collected, but no output is emitted) or **fatal** (compilation halts immediately).

---

# 2. Suppression Syntax

Line-level suppression (applied to the following line):
```
// fsm-lint:disable FSM-W0200
while (ctx.retries > 0) { retryConnect(); }
// fsm-lint:enable FSM-W0200
```

Block-level suppression:
```
// fsm-lint:disable-block FSM-W0201
entry: {
    initA(); initB(); initC(); initD(); initE();
    initF(); initG(); initH(); initI(); initJ();
}
// fsm-lint:enable-block FSM-W0201
```

File-level suppression (must appear within the first 5 lines of the file):
```
// fsm-lint:disable-file FSM-W0500
```

Suppressing a fatal error code (FSM-E0300 and above for non-recoverable errors) has
no effect — the diagnostic is still emitted but marked as suppressed.

---

# 3. Lexer / Parser Errors (FSM-E0001 – FSM-E0099)

---

### FSM-E0001 — Unexpected character

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes — skips the character and continues |
| **Title** | Unexpected character |
| **Description** | The lexer encountered a character that is not part of FSM-Lang syntax. |
| **Cause** | Typically a stray punctuation mark, Unicode character, or encoding error. |
| **Fix** | Remove the invalid character. |

**Example:**
```
state Idle$ {}   // FSM-E0001: unexpected character '$'
```

---

### FSM-E0002 — Unterminated string literal

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No |
| **Description** | A string literal opened with `"` was not closed before end of line. |
| **Fix** | Add the closing `"`. |

---

### FSM-E0003 — Unterminated block comment

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No |
| **Description** | A block comment opened with `/*` was not closed with `*/` before end of file. |

---

### FSM-E0004 — Invalid integer literal

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | An integer literal has an invalid suffix, leading zeros, or overflows u64. |

**Example:**
```
after 999999999999999999999ms -> Timeout   // FSM-E0004: integer overflow
```

---

### FSM-E0005 — Float literal in integer context

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A float literal (e.g., `3.14`) was used where an integer is required. Float literals are only valid for context fields of type `f32` or `f64`. Timer durations, queue capacities, and priority values require integer literals. |
| **Fix** | Use an integer literal, or change the target field type to `f32`/`f64`. |

**Example:**
```
after 1.5 ms -> Timeout   // FSM-E0005: float literal in integer context (timer duration)
```

---

### FSM-E0006 — Write to read-only payload field

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | An action block attempted to assign a value to a payload field. Payload fields are read-only in all contexts — they represent the incoming event data and MUST NOT be modified. |
| **Fix** | Copy the payload value to a context field first: `ctx.field = payload.field`, then modify the context field. |

**Example:**
```
on START -> Running: {
    payload.target_speed = 0;   // FSM-E0006: cannot write to read-only payload field
}
```

---

### FSM-E0010 — Expected token

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes (panic-mode recovery to next declaration) |
| **Message format** | `expected '{expected}', found '{actual}'` |
| **Description** | The parser expected a specific token but found something else. |

**Example:**
```
state Idle {
    on START Idle   // FSM-E0010: expected '->', found identifier
```

---

### FSM-E0011 — Unexpected end of file

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No |
| **Description** | Source ends while the parser is still inside an open declaration. |

---

### FSM-E0012 — Invalid `as` cast

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | An `as` cast was used between incompatible types. Permitted casts: integer-to-integer, integer-to-float, `f32`↔`f64`. Boolean, opaque, and enum types cannot be cast. |
| **Fix** | Remove the invalid cast or use an intermediate type. |

**Example:**
```
ctx.flag = ctx.speed as bool   // FSM-E0012: cannot cast u16 to bool
```

---

# 4. Duplicate Declaration Errors (FSM-E0020 – FSM-E0029)

---

### FSM-E0020 — Duplicate machine name

Trigger: Two `machine` declarations share the same name in the same file.

```
machine Motor { }
machine Motor { }   // FSM-E0020
```

Fix: Rename one of the machines.

---

### FSM-E0021 — Duplicate state name

Trigger: Two states within the same parent scope share a name.

```
state Idle { }
state Idle { }   // FSM-E0021
```

---

### FSM-E0022 — Duplicate event name

```
events {
    START
    START(speed : u16)   // FSM-E0022
}
```

---

### FSM-E0023 — Duplicate context field name

```
context {
    speed : u16
    speed : u32   // FSM-E0023: duplicate 'speed'
}
```

---

### FSM-E0024 — Duplicate extern name

```
pure extern isReady() : bool
extern isReady() : bool   // FSM-E0024
```

---

### FSM-E0025 — Duplicate stable ID

```
@id("state-idle") state Idle { }
@id("state-idle") state Standby { }   // FSM-E0025
```

---

# 5. Symbol Resolution Errors (FSM-E0100 – FSM-E0109)

---

### FSM-E0100 — Unknown state reference

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A transition target, `initial` declaration, or history `default` references a state name that is not declared in scope. |

**Example:**
```
initial Idle
state Running { on STOP -> Idle }
// FSM-E0100: 'Idle' not found in machine scope (forgot to declare it)
```

**Fix:** Declare the missing state, or correct the name.

---

### FSM-E0101 — Unknown event reference

Trigger: An `on`, `raise`, `send`, or `defer` references an event name not declared
in the machine's `event` block.

```
on LAUNCH -> Space   // FSM-E0101: 'LAUNCH' is not declared
```

---

### FSM-E0102 — Unknown extern reference

Trigger: A guard or action calls an extern function not declared in the machine's
`extern` block.

---

### FSM-E0103 — Unknown machine reference

Trigger: `send EVENT to MachineName` where `MachineName` is not declared in the
same project.

---

### FSM-E0104 — Unknown context field reference

Trigger: `ctx.fieldName` where `fieldName` is not declared in the `context` block.

---

### FSM-E0105 — State reference crosses machine boundary

Trigger: A transition target refers to a state in a different machine (not via a
submachine reference).

---

### FSM-E0106 — Non-pure extern used as guard

| | |
|---|---|
| **Description** | An `extern` function without the `pure` modifier is used in a guard expression. Guards must be side-effect free; only `pure extern` functions are permitted. |
| **Fix** | Add `pure` to the extern declaration if the function is side-effect free. If it has side effects, move it to an action instead. |

**Example:**
```
extern logAndCheck() : bool          // no 'pure'

on START [logAndCheck()] -> Running   // FSM-E0106
```

---

### FSM-E0107 — No initial declaration

Trigger: A composite state or the machine root has no `initial` declaration.

```
machine Motor {
    state Idle { }
    state Running { }
    // FSM-E0107: missing 'initial Idle' (or similar)
}
```

---

### FSM-E0108 — Multiple initial declarations

Trigger: Two or more `initial` declarations in the same region.

---

### FSM-E0109 — History default references non-existent state

---

# 6. Type and Semantic Errors (FSM-E0200 – FSM-E0299)

---

### FSM-E0200 — Type mismatch in guard expression

```
context {
    running : bool
}
on START [ctx.running > 10] -> Fast   // FSM-E0200: cannot compare bool with integer
```

---

### FSM-E0201 — Type mismatch in assignment

```
context {
    speed : u16
}
entry : ctx.speed = true   // FSM-E0201: cannot assign bool to u16
```

---

### FSM-E0202 — Type mismatch in extern call argument

---

### FSM-E0203 — Wrong number of arguments to extern

```
pure extern isReady(threshold : u16) : bool
on START [isReady()] -> Running   // FSM-E0203: expected 1 argument, found 0
```

---

### FSM-E0204 — Assignment to read-only field

Trigger: `payload.field = value` — payload fields are read-only.

---

### FSM-E0205 — Invalid left-hand side of assignment

Trigger: Left side of `=` is not a `ctx.field` reference.

---

### FSM-E0206 — Integer overflow in constant expression

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A compile-time constant expression overflows the target integer type. For signed types, overflow is always an error. For unsigned types, this error is only emitted for constant expressions in `const` declarations (runtime unsigned overflow wraps per §3.2). |
| **Fix** | Use a wider type or reduce the expression value. |

**Example:**
```
const MAX = 256 * 256 * 256 * 256   // FSM-E0206: overflow in u32 constant (4294967296 > u32 max)
```

---

### FSM-E0207 — Division by zero in constant expression

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No |
| **Description** | A compile-time constant expression divides by zero. |

**Example:**
```
const RATIO = 100 / 0   // FSM-E0207: division by zero in constant expression
```

---

### FSM-E0208 — Negative value in unsigned context

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A negative literal or provably negative constant expression is assigned to an unsigned integer field. |

**Example:**
```
context {
    count: u16 = -1   // FSM-E0208: negative value -1 in unsigned context (u16)
}
```

---

# 7. Determinism Errors (FSM-E0300 – FSM-E0399)

---

### FSM-E0300 — Nondeterministic transition conflict

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No (code generation is blocked) |
| **Title** | Nondeterministic transition conflict |
| **Description** | Two or more transitions from the same source state with the same event trigger have guards that are not statically provable to be mutually exclusive. The compiler cannot guarantee deterministic behavior. |

**Example:**
```
on START [ctx.speed > 5]  -> Medium
on START [ctx.speed > 10] -> Fast   // FSM-E0300: overlaps with above guard
```

**Fix options:**
1. Make guards mutually exclusive: `[ctx.speed > 5 && ctx.speed <= 10]` vs `[ctx.speed > 10]`
2. Use `else` as the fallback guard.
3. Add explicit `priority` clauses and acknowledge with `FSM-W0300`.

---

### FSM-E0301 — Guard on completion transition

Trigger: A completion transition (no event trigger) has a guard. Completion transitions
MUST be unconditional.

---

### FSM-E0302 — Fork target is not a region initial state

---

### FSM-E0303 — Join source is not in a parallel state region

---

### FSM-E0304 — Parallel state region has no initial declaration

---

### FSM-E0310 — Deferred event conflicts with explicit transition

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A state declares both `defer E` and an explicit `on E -> ...` transition for the same event E. Deferral and explicit handling are mutually exclusive for the same event in the same state. |
| **Fix** | Remove either the `defer` or the `on` transition. |

**Example:**
```
state Processing {
    defer DATA_RECEIVED
    on DATA_RECEIVED -> Handling   // FSM-E0310: conflicts with defer above
}
```

---

### FSM-E0600 — Parallel region has no `initial` declaration

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A `region` block inside a parallel state has no `initial` declaration. Every region MUST declare exactly one initial state. |
| **Fix** | Add `initial StateName` inside the region block. |

**Example:**
```
parallel Monitor {
    region Sensors {
        state Idle { }       // FSM-E0600: region 'Sensors' has no initial declaration
        state Active { }
    }
}
```

---

### FSM-E0750 — Fork target is not a parallel region

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | Yes |
| **Description** | A `fork` pseudo-state lists a target that is not an initial state within a region of a parallel state, or the targets are not in distinct regions of the same parallel state. |
| **Fix** | Ensure all fork targets are initial states in different regions of the same parallel state. |

**Example:**
```
fork StartAll -> { Idle, Running }   // FSM-E0750: Idle and Running are not in parallel regions
```

---

# 8. Reachability Errors (FSM-E0400 – FSM-E0499)

---

### FSM-E0400 — Unreachable state

| | |
|---|---|
| **Severity** | Error (if compiler can prove statically), Warning (FSM-W0400 alias if only suspected) |
| **Description** | A state is declared but cannot be reached from the initial configuration via any sequence of transitions. |

---

### FSM-E0401 — External self-transition on composite state without exit

Trigger: A self-transition on a composite state using `->` (external) instead of
`internal on E:` — the user probably intended an internal transition.

**Severity:** Warning (FSM-W0401 alias)

---

# 9. Submachine Errors (FSM-E0500 – FSM-E0599)

---

### FSM-E0500 — Submachine entry point not declared

### FSM-E0501 — Submachine exit point not declared

### FSM-E0502 — Submachine instantiation cycle detected

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No |
| **Description** | Machine A includes a submachine reference to B, and B (directly or transitively) includes a submachine reference to A. Cycles are forbidden. |

---

# 10. Runtime Safety Errors (FSM-E0900 – FSM-E0999)

---

### FSM-E0900 — Completion event chain too deep

| | |
|---|---|
| **Severity** | Error |
| **Recoverable** | No |
| **Description** | More than 100 consecutive completion events fired without processing an external event. This indicates an infinite loop in the completion chain. |
| **Compile-time detection** | The compiler SHOULD detect statically detectable completion cycles (e.g., a final state with a completion transition back to itself). |
| **Runtime behavior** | The generated runtime MUST call `FSM_ASSERT(false)` when the counter exceeds 100. |

---

# 11. Warning Codes (FSM-W0xxx)

---

### FSM-W0100 — History state has no stored value and no default

| | |
|---|---|
| **Severity** | Warning |
| **Description** | A transition targeting a history pseudo-state (`-> History`) is reachable before any exit from the parent state has been recorded. Behavior on first entry is unspecified unless a `default ->` is declared. |
| **Fix** | Add `default -> StateName` to the history declaration. |

---

### FSM-W0101 — Dead transition

| | |
|---|---|
| **Severity** | Warning |
| **Description** | A transition can never be taken because a higher-priority (or document-order-prior) transition from the same source with the same trigger is always enabled. |

---

### FSM-W0200 — Loop in action block

| | |
|---|---|
| **Severity** | Warning |
| **Suppressible** | Yes |
| **Title** | Loop in inline action block |
| **Description** | A `while` or `for` statement was found in an action block. Loops are permitted but may obscure bounded-execution analysis. |
| **Message** | `Loop in action block. Consider extracting to a named extern function for bounded-execution analysis.` |
| **Fix** | Extract the loop body to an `extern` function, or suppress with `// fsm-lint:disable FSM-W0200`. |

---

### FSM-W0201 — Action block complexity

| | |
|---|---|
| **Severity** | Warning |
| **Description** | An action block contains more than 10 statements. Complex inline logic should live in an `extern` function. |
| **Threshold** | 10 statements (configurable via `fsmLang.diagnostics.actionComplexityThreshold`) |

---

### FSM-W0300 — Transition priority used to resolve conflict

| | |
|---|---|
| **Severity** | Warning |
| **Description** | A transition conflict was present but resolved via explicit `priority` clauses. The compiler accepted this but warns that priority-based resolution is intentional and should be reviewed. |

---

### FSM-W0400 — Timer duration is zero

```
after 0ms -> Timeout   // FSM-W0400: zero-duration timer fires immediately on entry
```

---

### FSM-W0401 — Timer duration unusually large

```
after 999999999ms -> Timeout   // FSM-W0401: ~11.5 days — verify units
```

---

### FSM-W0500 — Extern declared but never used

---

### FSM-W0501 — Event declared but never used

Trigger: An event is declared in the `event` block but never appears on any transition
(`on`), never raised (`raise`/`send`), and never deferred (`defer`).

---

### FSM-W0600 — Region with single state

| | |
|---|---|
| **Severity** | Warning |
| **Suppressible** | Yes |
| **Description** | A parallel region contains only one state. A region with a single state provides no behavioral benefit and suggests the parallel structure may be unnecessary. |
| **Fix** | Consider removing the region or adding additional states. Suppress with `// fsm-lint:disable FSM-W0600` if intentional. |

**Example:**
```
parallel Trivial {
    region OnlyOne {
        initial Sole
        state Sole { }   // FSM-W0600: region 'OnlyOne' has only one state
    }
}
```

---

### FSM-W0601 — Timer duration exceeds 24 hours

| | |
|---|---|
| **Severity** | Warning |
| **Suppressible** | Yes |
| **Description** | A timer duration exceeds 86,400,000 milliseconds (24 hours). Very long timers may indicate a unit error (e.g., seconds used where milliseconds expected) and may exceed platform timer resolution. |
| **Fix** | Verify the duration is correct. Suppress with `// fsm-lint:disable FSM-W0601` if intentional. |

**Example:**
```
after 100000000 ms -> Timeout   // FSM-W0601: duration 100000000ms (~27.8 hours) exceeds 24 hours
```

---

### FSM-W0602 — Unreachable state (no incoming transitions, not initial)

| | |
|---|---|
| **Severity** | Warning |
| **Suppressible** | Yes |
| **Description** | A state has no incoming transitions and is not declared as the `initial` state of any scope. The state can never become active. |
| **Fix** | Add a transition targeting this state, mark it as `initial`, or remove it. |

**Example:**
```
state Orphan { }   // FSM-W0602: 'Orphan' has no incoming transitions and is not initial
```

---

### FSM-W0603 — Guard always evaluates to true/false (constant guard)

| | |
|---|---|
| **Severity** | Warning |
| **Suppressible** | Yes |
| **Description** | A guard expression can be statically proven to always evaluate to the same boolean value. A guard that is always true makes the transition unconditional; a guard that is always false makes the transition dead. |
| **Fix** | Remove the guard if always true, or remove the transition if always false. |

**Example:**
```
on START [true] -> Running        // FSM-W0603: guard always evaluates to true
on STOP [ctx.x > 0 && ctx.x < 0] -> Error  // FSM-W0603: guard always evaluates to false
```

---

# 12. Info Codes (FSM-I0xxx)

---

### FSM-I0001 — Compilation successful

Emitted when `fsm compile` completes with zero errors. Shown in CLI output.

### FSM-I0002 — Code generation complete

Emitted by `fsm generate` when output files are written.

### FSM-I0003 — Simulator ready

Emitted by the simulator daemon when it has loaded a machine and is ready for dispatch.

### FSM-I0004 — IR schema version mismatch (forward-compatible)

Trigger: An IR document with a higher minor version is loaded by a tool. The tool can
process it but may ignore unknown fields.

---

# 13. Hint Codes (FSM-H0xxx) — Editor Only

Hints are displayed only in the editor (as grayed-out annotations) and are never shown
in CLI output unless `--hints` flag is passed.

---

### FSM-H0001 — State has no entry action

```
// Hint: "Consider adding an entry action to document state initialization."
state Idle { }
```

### FSM-H0002 — State has no exit action

### FSM-H0003 — Unconditional transition (no guard)

```
// Hint: "This transition is always enabled when event RESET is received."
on RESET -> Error
```

### FSM-H0004 — Single-region parallel state

```
// Hint: "A parallel state with one region is equivalent to a composite state."
parallel Redundant {
    region Main { ... }
}
```

### FSM-H0005 — Prefer `every` over periodic self-transition

```
// Hint: "Use 'every Xms: action()' instead of a self-transition with 'after Xms'."
after 100ms -> Self
```

---

# 14. Deprecation Policy

1. Diagnostic codes are **never reused**. A retired code retains its number with
   status "Deprecated — no longer emitted."

2. Deprecated codes MUST still be parsed and silently accepted in suppression
   annotations for backwards compatibility. A file with
   `// fsm-lint:disable FSM-E0300` will not produce an error if FSM-E0300 is later
   retired.

3. Message text of a diagnostic may change between minor versions.

4. Code numbers and severity levels MUST NOT change after a code is published as
   Normative.

5. New codes may be added in any minor version. Consumers MUST handle unknown codes
   gracefully (display as unknown, do not crash).

---

*End of FSM-SPEC-DIAG v1.0.0*
