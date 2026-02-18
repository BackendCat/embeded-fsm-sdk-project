# FSM Studio — Formatter Specification

**Document ID:** FSM-SPEC-FMT
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL

Specifies the canonical formatting rules for `fsm fmt`. The formatter is deterministic
and idempotent: running `fsm fmt` twice on the same file MUST produce identical output.

---

# 1. Design Principles

1. **One canonical style.** There is exactly one correct way to format an FSM-Lang file.
   No configuration of individual rules (only `indent-size` and `bracket-style` are
   user-configurable).
2. **Idempotent.** `fsm fmt (fsm fmt FILE)` MUST equal `fsm fmt FILE`.
3. **Comment-preserving.** All `//`, `/* */`, and `///` comments MUST be preserved
   in their original relative position (before or after the node they annotate).
4. **Semantic-neutral.** Formatting MUST NOT change the meaning of any FSM-Lang file.
   A formatted file MUST compile to an identical IR as the unformatted original.
5. **Minimal diff.** The formatter SHOULD make the smallest set of changes that
   produces canonical form. This minimizes noise in version control diffs.

---

# 2. Configurable Options

Only two options are configurable. All other rules are fixed.

| Option | Values | Default | Description |
|---|---|---|---|
| `indent-size` | integer 1–8 | `4` | Spaces per indent level |
| `bracket-style` | `same-line` \| `next-line` | `same-line` | Position of opening `{` |

`bracket-style = "same-line"` (default):
```
machine Motor {
    state Idle {
```

`bracket-style = "next-line"`:
```
machine Motor
{
    state Idle
    {
```

---

# 3. File-Level Rules

1. **Encoding.** Output MUST be UTF-8. BOM MUST NOT be emitted.
2. **Line endings.** LF (`\n`) on all platforms. CRLF in input is accepted and
   normalized to LF.
3. **Trailing newline.** Files MUST end with exactly one `\n`.
4. **Trailing whitespace.** No trailing whitespace on any line.
5. **Maximum line length.** No hard limit enforced. Long transition labels are not
   broken (DSL identifiers cannot contain whitespace).
6. **Top-level separation.** One blank line between top-level declarations
   (`machine`, any top-level `// comment`).
7. **No consecutive blank lines.** At most one consecutive blank line anywhere in
   the file.

---

# 4. Machine Declaration

```fsm
machine Motor {
    context {
        speed:   u16 = 0
        running: bool = false
    }

    event START(target_speed: u16)
    event STOP

    pure extern isSpeedValid(threshold: u16) : bool

    initial Idle

    state Idle { }
    state Running { }
}
```

**Rules:**
- `machine NAME {` on one line (or `machine NAME\n{` for next-line bracket style).
- **Section order inside machine body** (MUST follow this order):
  1. `context { }` block (if present)
  2. `event` declarations (if present)
  3. `extern` declarations (if present)
  4. `initial` declaration
  5. State declarations (in source order — formatter does NOT reorder states)
  6. `submachine` declarations (if present)
- One blank line between each section (context block, events group, externs group, initial, states).
- No blank line between consecutive `event` declarations (they form a group).
- No blank line between consecutive `extern` declarations.

---

# 5. Context Block

```fsm
context {
    speed:   u16  = 0
    running: bool = false
    mode:    u8   = 0
}
```

**Rules:**
- `context {` on one line.
- Each field on its own line, indented by one level.
- **Aligned colons and equals signs** within the context block:
  - Field names left-aligned.
  - `:` aligned to the widest field name + 1 space.
  - Type names left-aligned after `:`.
  - `=` aligned to the widest type name + 1 space.
  - Default values left-aligned after `=`.
- Each field ends with `;`.
- Closing `}` at base indent level.

**Alignment algorithm:**
```
max_name_len  = max(len(field.name) for all fields in block)
max_type_len  = max(len(field.type) for all fields in block)

for each field:
    emit field.name
    emit ' ' * (max_name_len - len(field.name) + 1)
    emit ':'
    emit ' '
    emit field.type
    emit ' ' * (max_type_len - len(field.type) + 1)
    emit '= '
    emit field.default
    emit ';'
```

---

# 6. Event Declarations

```fsm
event START(target_speed: u16)
event STOP
event FAULT(code: u8, severity: u8)
```

**Rules:**
- Each `event` on its own line, no blank lines between events.
- No alignment of event names (unlike context fields).
- If event has payload: `event NAME(field: Type, field: Type);` — one line.
- Payload fields separated by `, ` (comma + space).
- No trailing comma.

---

# 7. Extern Declarations

```fsm
pure extern isSpeedValid(threshold: u16) : bool
extern logEvent(code: u8)
```

**Rules:**
- Each `extern` on one line.
- `pure` keyword before `extern` keyword if present: `pure extern NAME`.
- Return type: `: Type` with spaces around `:`.
- No alignment between extern declarations.
- No blank line between consecutive externs.

---

# 8. State Declarations — Simple State

```fsm
state Idle {
    entry: {
        startTimer();
        ctx.running = false;
    }
    exit: { stopTimer(); }

    on START [isSpeedValid(ctx.speed)] -> Running: { logStart(); }
    on START -> Error

    after 5000ms -> Timeout
    defer PAUSE
}
```

**Rules:**
- `state NAME {` on one line.
- **Section order inside state body** (MUST follow this order):
  1. `entry:` block (if present)
  2. `exit:` block (if present)
  3. `on` transitions (in source order)
  4. `after` / `every` timers (in source order)
  5. `internal` transitions (in source order)
  6. `defer` declarations (in source order)
- One blank line between `entry`/`exit` blocks and transitions.
- No blank line between consecutive `on` transitions.
- No blank line between consecutive `defer` declarations.

---

# 9. Entry and Exit Blocks

**Single-statement entry/exit** — inline on one line:
```fsm
entry: { startTimer(); }
exit:  { stopTimer(); }
```

**Multi-statement entry/exit** — each statement on its own line:
```fsm
entry: {
    startTimer();
    ctx.speed = 0;
    logEntry();
}
```

**Alignment of `entry:` and `exit:`** — if both are present, align the `{`:
```fsm
entry: { startTimer(); }
exit:  { stopTimer(); }
```
(Pad `exit:` with one space to align `{` with `entry:`)

**Rules for action blocks:**
- Each statement on its own line (multi-statement blocks).
- Statements indented one level inside `{ }`.
- `if`/`else` formatted as:
  ```fsm
  if (ctx.speed > 100) {
      highSpeedAction();
  } else {
      normalAction();
  }
  ```
- `while` formatted as:
  ```fsm
  while (ctx.retries > 0) {
      retry();
      ctx.retries = ctx.retries - 1;
  }
  ```
- `for` formatted as:
  ```fsm
  for (ctx.i = 0; ctx.i < 10; ctx.i = ctx.i + 1) {
      step();
  }
  ```
- `raise`, `send`, `defer` on their own line.
- No trailing `;` after the closing `}` of an action block.

---

# 10. Transitions

```fsm
on START                          -> Running
on START [ctx.speed > 0]          -> Running: { logStart(); }
on START [isSpeedValid(ctx.speed)] -> Fast
on STOP                           -> Idle
```

**Rules:**
- `on EVENT` — `on` + space + event name.
- Guard `[expr]` — space before `[`, no space inside `[` and `]`.
- `->` — space before and after.
- Target state name — no space after.
- Action `{ ... }` — `:` immediately after target, then space, then `{ ... }`.
- **Alignment within a group of consecutive `on` transitions:**
  - Align `->` arrows vertically to the widest `on EVENT [guard]` prefix.
  - Align `:` (action separator) vertically if any transition has an action.
  - Alignment resets on any blank line or non-transition declaration.

**Alignment example:**
```fsm
on START [ctx.speed > 0]          -> Running: { logStart(); }
on START [isSpeedValid(ctx.speed)] -> Fast
on STOP                            -> Idle
```

- The longest prefix is `on START [isSpeedValid(ctx.speed)]` (32 chars).
- All `->` are aligned to column 33.

**Internal transition:**
```fsm
internal on PING: { updateTimestamp(); }
```

**Priority clause** (inline after transition):
```fsm
on START [ctx.speed > 0] -> Running priority 50
```
`priority` keyword + space + number, at the end of the line before `;`.

---

# 11. Timer Declarations

```fsm
after  5000ms -> Timeout
every  100ms  -> Update
every  500ms: { poll(); }
```

**Rules:**
- `after`/`every` followed by one space.
- Duration: integer immediately followed by `ms` (no space between number and `ms`).
- One space after `ms`.
- `->` with spaces around it, OR `:` with space before action block.
- **Align** `->` and `ms` across consecutive timer declarations (same as transition alignment).

---

# 12. Composite and Parallel States

```fsm
composite Operational {
    initial Normal
    history shallow

    state Normal { }
    state Degraded { }
}

parallel Monitor {
    region Sensors {
        initial Idle
        state Idle { }
    }

    region Output {
        initial Idle
        state Idle { }
    }
}
```

**Rules:**
- `composite NAME {` / `parallel NAME {` on one line.
- `initial StateName` before any `state` declarations.
- `history shallow` / `history deep` on its own line, before states.
- `history shallow default -> StateName` on its own line.
- **Blank line between regions** in a parallel state.
- Each `region NAME {` / `}` follows the same indent rules as state bodies.

---

# 13. Pseudo-State Declarations

## 13.1 Choice Pseudo-State

```fsm
choice SpeedSelect {
    [ctx.speed > 100] -> Fast
    [ctx.speed > 0]   -> Normal
    [else]            -> Idle
}
```

**Rules:**
- `choice NAME {` on one line (opening brace on same line as `choice`).
- Each branch on its own line, indented by one level (4-space indent at default `indent-size`).
- Guards are left-aligned at the indent level.
- `->` arrows aligned vertically within the choice block (pad guards to the widest guard + 1 space).
- `[else]` branch MUST be last.
- Closing `}` at the same indent level as the `choice` keyword.
- If a branch has an action block: `[guard] -> Target: { action(); }` — colon immediately
  after target, space, then action block (same rules as transition actions in §10).

**Example with actions:**
```fsm
choice SpeedSelect {
    [ctx.speed > 100] -> Fast:   { logFast(); }
    [ctx.speed > 0]   -> Normal: { logNormal(); }
    [else]            -> Idle
}
```

## 13.2 Junction Pseudo-State

Formatting rules are identical to `choice` (§13.1). The only difference is the keyword.

```fsm
junction RouteSelect {
    [ctx.mode == 1] -> ModeA
    [ctx.mode == 2] -> ModeB
    [else]          -> Default
}
```

**Rules:**
- `junction NAME {` on one line (opening brace on same line).
- Each branch on its own line with 4-space indent.
- Guards left-aligned at the indent level.
- `->` arrows aligned vertically within the junction block.
- `[else]` branch MUST be last.
- Closing `}` at the same indent level as the `junction` keyword.

## 13.3 Fork Pseudo-State

```fsm
fork StartAll -> { Monitor.Sensors, Monitor.Output }
```

**Rules:**
- `fork NAME -> { Target1, Target2 }` on a single line if the total line length is <= 80 characters.
- Targets separated by `, ` (comma + space) inside `{ }`.
- Space after `{` and before `}`.
- If the line exceeds 80 characters, targets wrap to separate lines with 4-space indent:
  ```fsm
  fork ActivateAllRegions -> {
      Monitor.Sensors,
      Monitor.Output,
      Controller.Primary,
      Controller.Secondary
  }
  ```
- In the multi-line form, each target on its own line with a trailing comma (except the last target).
- Closing `}` at the same indent level as the `fork` keyword.

## 13.4 Join Pseudo-State

Formatting rules are identical to `fork` (§13.3), with the addition of the `-> Target` suffix.

```fsm
join AllDone { Monitor.Sensors, Monitor.Output } -> Done
```

**Rules:**
- `join NAME { Source1, Source2 } -> Target` on a single line if <= 80 characters.
- Sources separated by `, ` (comma + space) inside `{ }`.
- Space after `{` and before `}`.
- `-> Target` follows immediately after `}` with spaces around `->`.
- If the line exceeds 80 characters, sources wrap to separate lines with 4-space indent:
  ```fsm
  join AllRegionsDone {
      Monitor.Sensors,
      Monitor.Output,
      Controller.Primary,
      Controller.Secondary
  } -> Completed
  ```
- In the multi-line form, `} -> Target` on its own line at the base indent level.

## 13.5 Stable ID and Doc Comment on Pseudo-States

Pseudo-states follow the same annotation ordering as all other declarations (see §15):
`@id(...)` first, then `/// doc comment`, then the declaration.

```fsm
@id("ps-speed-select")
/// Routes to the appropriate speed state.
choice SpeedSelect {
    [ctx.speed > 100] -> Fast
    [ctx.speed > 0]   -> Normal
    [else]            -> Idle
}

@id("ps-start-fork")
/// Activates all parallel regions simultaneously.
fork StartAll -> { Monitor.Sensors, Monitor.Output }
```

---

# 14. Comments

**Line comments** — preserved in place:
```fsm
// This is a comment
state Idle { }
```

**Block comments** — preserved in place:
```fsm
/*
 * Multi-line block comment
 * preserved exactly.
 */
state Idle { }
```

**Rules:**
- Comments are NOT reformatted internally (content preserved verbatim).
- A comment that precedes a declaration has no blank line inserted between the comment
  and the declaration.
- A comment that appears at the end of a line (after a declaration) is preserved on
  the same line with one space before `//`.
- Orphaned comments (not attached to any declaration) are preserved in their relative
  position.

---

# 15. Stable IDs and Annotations

```fsm
@id("motor-idle-state")
/// The idle waiting state.
state Idle {
    @id("t-idle-to-running")
    on START -> Running
}
```

**Rules:**
- `@id(...)` on its own line immediately before the annotated node.
- No blank line between `@id(...)` and its annotated node.
- If `/// doc comment` and `@id(...)` both precede a declaration, order is: `@id` first,
  then `/// doc comment`, then the declaration.

---

# 16. Full Canonical Example

Input (unformatted):
```fsm
machine Motor{context{speed:u16=0;running:bool=false;}event START(target_speed:u16);event STOP;initial->Idle;state Idle{entry:{startTimer();}on START->Running:{ ctx.speed=0; logStart(); }after 5000ms->Error;}state Running{exit:{stopMotor();}on STOP->Idle;}}
```

Output (canonical):
```fsm
machine Motor {
    context {
        speed:   u16  = 0
        running: bool = false
    }

    event START(target_speed: u16)
    event STOP

    initial Idle

    state Idle {
        entry: { startTimer(); }

        on START -> Running: {
            ctx.speed = 0;
            logStart();
        }

        after 5000ms -> Error
    }

    state Running {
        exit: { stopMotor(); }

        on STOP -> Idle
    }
}
```

---

# 17. Idempotency Requirement

The formatter MUST produce identical output when run twice:
```bash
fsm fmt motor.fsm          # First run — formats in place
fsm fmt --check motor.fsm  # Second run — MUST exit 0
```

The CI pipeline MUST verify this:
```bash
fsm fmt --check $(find . -name "*.fsm") || (echo "FAIL: run fsm fmt"; exit 1)
```

---

*End of FSM-SPEC-FMT v1.0.0*
