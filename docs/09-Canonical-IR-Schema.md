# FSM Studio — Canonical JSON Model (IR Schema)

**Document ID:** FSM-SPEC-IR
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL, FSM-SPEC-SEM

The Intermediate Representation (IR) is the canonical JSON document produced by the
FSM-Lang compiler and consumed by all toolchain components: code generators, simulator,
LSP, diagram renderer, and documentation tools. Every toolchain component reads and
writes the IR; none reads `.fsm` source directly.

---

# 1. Design Principles

1. **Round-trippable.** IR → DSL regeneration (`fsm decompile`) MUST be possible. The
   output need not be byte-for-byte identical to the original source, but MUST be
   semantically equivalent and re-compilable to the same IR.

2. **Source-location preserving.** Every node in the IR MUST carry a `loc` field
   containing its source position. Toolchain components MUST propagate locations when
   transforming the IR.

3. **Schema-versioned.** Every IR document MUST contain `"irVersion": "1.0.0"` at the
   top level. Consumers MUST check the major version and MUST NOT process documents with
   a different major version.

4. **Stable-ID bearing.** Every named entity MUST have a `stableId` field. Stable IDs
   MUST NOT change across non-breaking DSL edits (renaming is a breaking change).
   Toolchain continuity (history persistence, trace correlation) depends on stable IDs.

5. **Diagnostic-embedded.** Diagnostics emitted during compilation are embedded in the
   IR under the top-level `diagnostics` array. The IR MAY be partial when errors are
   present.

6. **Heap-free target.** The IR schema is designed so that a generated C runtime MUST
   be implementable without heap allocation. This influences how arrays are represented
   (bounded, not open-ended growable structures).

---

# 2. Top-Level Schema

```json
{
  "irVersion": "1.0.0",
  "sourceHash": "sha256:a3f9c1...",
  "sourceFiles": ["path/to/device.fsm"],
  "machines": [ ],
  "diagnostics": [ ]
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `irVersion` | string | YES | Semantic version of this IR schema |
| `sourceHash` | string | YES | `sha256:` prefix + hex of concatenated source files |
| `sourceFiles` | string[] | YES | Absolute or workspace-relative paths of source files |
| `machines` | MachineObject[] | YES | All machine declarations (may be empty) |
| `diagnostics` | DiagnosticObject[] | YES | All diagnostics from compilation (may be empty) |

---

# 3. Machine Object Schema

```json
{
  "id": "m-motor",
  "stableId": "Motor",
  "name": "Motor",
  "context": { },
  "events": [ ],
  "externs": [ ],
  "root": { },
  "submachines": [ ],
  "loc": { }
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | YES | Compiler-assigned unique ID within this IR document |
| `stableId` | string | YES | Stable identifier for toolchain continuity |
| `name` | string | YES | Machine name as declared in DSL |
| `context` | ContextSchema | YES | Context field declarations |
| `events` | EventObject[] | YES | Event declarations |
| `externs` | ExternObject[] | YES | Extern function declarations |
| `root` | RegionObject | YES | Root region containing all top-level states |
| `submachines` | MachineObject[] | YES | Submachine definitions (may be empty array) |
| `loc` | SourceLocation | YES | Location of `machine` keyword in source |

---

# 4. State Node Schema

State nodes use a **discriminated union** on the `kind` field.

## 4.1 Simple State

```json
{
  "kind": "simple",
  "id": "s-idle",
  "stableId": "Motor:state:Idle",
  "name": "Idle",
  "entry": [],
  "exit": [],
  "transitions": [],
  "timers": [],
  "defers": [],
  "loc": { }
}
```

## 4.2 Composite State

```json
{
  "kind": "composite",
  "id": "s-operational",
  "stableId": "Motor:state:Operational",
  "name": "Operational",
  "entry": [],
  "exit": [],
  "transitions": [],
  "timers": [],
  "defers": [],
  "regions": [ ],
  "history": null,
  "loc": { }
}
```

Additional fields beyond simple state:

| Field | Type | Required | Description |
|---|---|---|---|
| `regions` | RegionObject[] | YES | MUST contain exactly one region for composite states |
| `history` | HistoryObject \| null | YES | History pseudo-state if declared; null otherwise |

## 4.3 Parallel State

```json
{
  "kind": "parallel",
  "id": "s-monitor",
  "stableId": "Motor:state:Monitor",
  "name": "Monitor",
  "entry": [],
  "exit": [],
  "transitions": [],
  "timers": [],
  "defers": [],
  "regions": [ ],
  "loc": { }
}
```

`regions` MUST contain ≥ 2 region objects for parallel states.

## 4.4 Pseudo-State: Initial

```json
{ "kind": "initial", "id": "ps-initial-0", "target": "s-idle", "loc": { } }
```

## 4.5 Pseudo-State: Final

```json
{ "kind": "final", "id": "ps-final-0", "stableId": "Motor:state:Done", "loc": { } }
```

## 4.6 Pseudo-State: Choice

```json
{
  "kind": "choice",
  "id": "ps-choice-0",
  "stableId": "Motor:choice:SpeedCheck",
  "branches": [ ],
  "loc": { }
}
```

Branches are evaluated at runtime (dynamic choice). Each `ChoiceBranch`:

```json
{
  "guard": { },
  "target": "s-running-fast",
  "actions": [],
  "loc": { }
}
```

One branch MUST have `"guard": { "kind": "else" }`.

## 4.7 Pseudo-State: Junction

Same schema as choice, but branches are evaluated statically at the moment the junction
is entered (after all exit actions, before entry actions of target). This distinction is
semantic, not structural — both use the same JSON shape with `"kind": "junction"`.

## 4.8 Pseudo-State: History

```json
{
  "kind": "history",
  "id": "ps-history-0",
  "stableId": "Motor:history:MainHistory",
  "historyKind": "shallow",
  "defaultTarget": "s-idle",
  "loc": { }
}
```

| Field | Type | Values | Description |
|---|---|---|---|
| `historyKind` | string | `"shallow"` \| `"deep"` | Shallow records direct child; deep records deepest active descendant |
| `defaultTarget` | string \| null | state ID | Target when no history is stored yet; null triggers FSM-W0100 |

## 4.9 Pseudo-State: Fork

```json
{
  "kind": "fork",
  "id": "ps-fork-0",
  "stableId": "Motor:fork:StartAll",
  "targets": ["s-monitor-sensors", "s-monitor-output"],
  "loc": { }
}
```

## 4.10 Pseudo-State: Join

```json
{
  "kind": "join",
  "id": "ps-join-0",
  "stableId": "Motor:join:AllDone",
  "sources": ["s-monitor-sensors", "s-monitor-output"],
  "target": "s-idle",
  "actions": [],
  "loc": { }
}
```

## 4.11 Submachine Reference

```json
{
  "kind": "submachine_ref",
  "id": "s-submachine-ref-0",
  "stableId": "Motor:state:Protocol",
  "name": "Protocol",
  "submachineId": "m-protocol",
  "entryPoints": { "Start": "s-proto-start" },
  "exitPoints": { "Done": "s-idle" },
  "transitions": [],
  "loc": { }
}
```

## 4.12 Entry Point / Exit Point

```json
{ "kind": "entry_point", "id": "ep-start", "name": "Start", "loc": { } }
{ "kind": "exit_point",  "id": "xp-done",  "name": "Done",  "loc": { } }
```

---

# 5. Region Schema

```json
{
  "id": "r-main",
  "name": "Main",
  "initial": "ps-initial-0",
  "states": [ ],
  "priority": 0,
  "loc": { }
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | YES | Unique ID |
| `name` | string | YES | Region name (auto-generated if not declared: `"__region_N"`) |
| `initial` | string | YES | ID of the initial pseudo-state within this region |
| `states` | StateNode[] | YES | All states and pseudo-states in this region |
| `priority` | integer | YES | Region dispatch priority for parallel states; lower = higher priority |
| `loc` | SourceLocation | YES | Location |

---

# 6. Transition Object Schema

```json
{
  "id": "t-idle-running",
  "stableId": "Motor:transition:idle-running-START",
  "source": "s-idle",
  "target": "s-running",
  "trigger": { "kind": "event", "eventId": "ev-start" },
  "guard": null,
  "actions": [],
  "priority": 100,
  "internal": false,
  "loc": { }
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | YES | Unique ID |
| `stableId` | string | YES | Stable ID |
| `source` | string | YES | Source state ID |
| `target` | string | YES | Target state ID |
| `trigger` | TriggerObject \| null | YES | null = completion transition |
| `guard` | GuardExpr \| null | YES | null = unconditional |
| `actions` | Statement[] | YES | Action list (may be empty) |
| `priority` | integer | YES | Priority (lower number = higher priority; default 100) |
| `internal` | boolean | YES | true = internal transition (no exit/entry) |
| `loc` | SourceLocation | YES | Location of transition declaration |

### TriggerObject

```json
{ "kind": "event", "eventId": "ev-start" }
```

or for timer triggers (internal synthetic events):

```json
{ "kind": "timer", "timerId": "tm-after-idle" }
```

---

# 7. Guard Expression Schema

Discriminated on `kind`:

| Kind | Description | Additional Fields |
|---|---|---|
| `"field_cmp"` | Compare field to value | `lhs: FieldRef`, `op: CmpOp`, `rhs: FieldRef | Literal` |
| `"extern_call"` | Call a pure extern | `callee: string` (extern ID), `args: Expr[]` |
| `"not"` | Logical negation | `operand: GuardExpr` |
| `"and"` | Logical and | `left: GuardExpr`, `right: GuardExpr` |
| `"or"` | Logical or | `left: GuardExpr`, `right: GuardExpr` |
| `"else"` | Always-true default | (no additional fields) |

`CmpOp`: `"=="` | `"!="` | `"<"` | `">"` | `"<="` | `">="`

Example:
```json
{
  "kind": "and",
  "left": { "kind": "field_cmp", "lhs": { "kind": "ctx", "field": "speed" }, "op": ">", "rhs": { "kind": "int", "value": 0 } },
  "right": { "kind": "extern_call", "callee": "ext-isSpeedValid", "args": [] }
}
```

---

# 8. Statement Schema (Action Language)

Discriminated on `kind`:

| Kind | Fields |
|---|---|
| `"assign"` | `target: FieldRef`, `value: Expr` |
| `"if"` | `condition: Expr`, `then: Statement[]`, `else_: Statement[]` |
| `"while"` | `condition: Expr`, `body: Statement[]` |
| `"for"` | `init: AssignStatement`, `condition: Expr`, `update: AssignStatement`, `body: Statement[]` |
| `"call"` | `callee: string` (extern ID), `args: Expr[]` |
| `"raise"` | `eventId: string`, `args: Expr[]` |
| `"send"` | `eventId: string`, `args: Expr[]`, `machineId: string` |
| `"defer"` | `eventId: string` |

Example `assign`:
```json
{ "kind": "assign", "target": { "kind": "ctx", "field": "speed" }, "value": { "kind": "literal", "literalKind": "int", "value": 0 } }
```

---

# 9. Expression Schema

Discriminated on `kind`:

| Kind | Fields |
|---|---|
| `"field_ref"` | `ref: FieldRef` |
| `"literal"` | `literalKind: "int"|"bool"|"string"`, `value: number|boolean|string` |
| `"call"` | `callee: string` (extern ID), `args: Expr[]` |
| `"unary"` | `op: "!"|"-"|"~"`, `operand: Expr` |
| `"binary"` | `op: BinOp`, `left: Expr`, `right: Expr` |

`FieldRef`:
```json
{ "kind": "ctx", "field": "speed" }
{ "kind": "payload", "field": "target_speed" }
```

`BinOp`: `"+"` | `"-"` | `"*"` | `"/"` | `"%"` | `"&"` | `"|"` | `"^"` | `"<<"` |
`">>"` | `"&&"` | `"||"` | `"=="` | `"!="` | `"<"` | `">"` | `"<="` | `">="`

---

# 10. Context Schema

```json
{
  "fields": [
    {
      "id": "cf-speed",
      "name": "speed",
      "type": { "kind": "primitive", "name": "u16" },
      "default": { "literalKind": "int", "value": 0 },
      "loc": { }
    }
  ]
}
```

### TypeRef

| Kind | Fields | Example |
|---|---|---|
| `"primitive"` | `name: string` | `"u8"`, `"i32"`, `"bool"`, `"f32"` |
| `"enum"` | `enumId: string` | reference to an event's enum field |
| `"opaque"` | `cType: string` | `"struct MyDevice *"` |

---

# 11. Event Schema

```json
{
  "id": "ev-start",
  "stableId": "Motor:event:START",
  "name": "START",
  "payload": [
    { "id": "pf-target-speed", "name": "target_speed", "type": { "kind": "primitive", "name": "u16" }, "loc": { } }
  ],
  "loc": { }
}
```

---

# 12. Extern Declaration Schema

```json
{
  "id": "ext-is-speed-valid",
  "stableId": "Motor:extern:isSpeedValid",
  "name": "isSpeedValid",
  "pure": true,
  "params": [
    { "name": "ctx",     "type": { "kind": "opaque", "cType": "const Motor_t *" } },
    { "name": "payload", "type": { "kind": "opaque", "cType": "const Motor_StartPayload_t *" } }
  ],
  "returnType": { "kind": "primitive", "name": "bool" },
  "loc": { }
}
```

---

# 13. Timer Schema

```json
{
  "id": "tm-after-idle",
  "stableId": "Motor:timer:AfterIdle",
  "kind": "after",
  "durationMs": { "kind": "int_const", "value": 5000 },
  "ownerStateId": "s-idle",
  "target": "s-error",
  "actions": [],
  "loc": { }
}
```

| Field | Values |
|---|---|
| `kind` | `"after"` (one-shot) \| `"every"` (periodic) \| `"every_internal"` (no transition) |
| `durationMs` | `ConstExpr` — currently only `int_const`; future: identifier referencing context field |

---

# 14. Diagnostic Object Schema

```json
{
  "code": "FSM-E0300",
  "severity": "error",
  "message": "Nondeterministic transition conflict: both transitions may be enabled when event START is received in state Idle.",
  "loc": { "file": "device.fsm", "line": 42, "col": 5, "endLine": 42, "endCol": 30 },
  "relatedLocs": [
    { "message": "conflicting transition here", "loc": { "file": "device.fsm", "line": 45, "col": 5, "endLine": 45, "endCol": 30 } }
  ],
  "fixable": false
}
```

| Field | Type | Values |
|---|---|---|
| `code` | string | `"FSM-E####"`, `"FSM-W####"`, `"FSM-I####"`, `"FSM-H####"` |
| `severity` | string | `"error"` \| `"warning"` \| `"info"` \| `"hint"` |
| `message` | string | Human-readable message |
| `loc` | SourceLocation | Primary location |
| `relatedLocs` | RelatedLocation[] | Secondary locations with contextual message |
| `fixable` | boolean | true = LSP can offer a `codeAction` quick-fix |

---

# 15. Source Location Schema

```json
{ "file": "device.fsm", "line": 12, "col": 5, "endLine": 12, "endCol": 20 }
```

- `line` and `col` are **1-based**.
- `endLine` and `endCol` point to the character **after** the last character (exclusive end).
- `file` is workspace-relative when the compiler is invoked within a project; absolute otherwise.

---

# 16. Stable ID Generation Algorithm

Stable IDs are deterministic strings that survive refactoring (state renaming is a breaking change). The algorithm:

1. **Explicit override.** If the declaration has `@id("X")`, the stable ID is `X`. The compiler MUST verify X is unique within the machine.

2. **Auto-generation.** The stable ID is computed as:
   ```
   stableId = slugify(machineName) + ":" + kind + ":" + slugify(declarationName)
   ```
   Where:
   - `slugify(s)` = lowercase, spaces → `-`, non-alphanumeric (except `-`) removed
   - `kind` = `"state"` | `"event"` | `"extern"` | `"transition"` | `"timer"` | `"choice"` | `"fork"` | `"join"` | `"history"`

3. **Collision resolution.** If the generated ID collides (two states with the same name in the same machine, which is FSM-E0021), a numeric suffix is appended: `_2`, `_3`, etc. FSM-E0021 MUST be emitted regardless.

4. **Transition auto-ID.** Transitions do not have names. Auto-ID:
   ```
   slugify(machineName) + ":transition:" + slugify(sourceName) + "-" + slugify(targetName) + "-" + slugify(eventName)
   ```
   Collision resolution appends `_2`, `_3` etc.

5. **Stability guarantee.** Stable IDs MUST NOT change when:
   - Adding or removing unrelated states, events, or transitions
   - Reordering declarations
   - Adding or removing action/guard content
   - Changing transition priorities

   Stable IDs MUST change (and this constitutes a breaking change) when:
   - Renaming the entity
   - Moving a state to a different parent (path changes)

---

# 17. Versioning Rules

The IR schema uses **semantic versioning** (`MAJOR.MINOR.PATCH`).

| Change Type | Version Impact |
|---|---|
| Add optional field | MINOR bump |
| Add required field | MAJOR bump (breaking) |
| Remove any field | MAJOR bump (breaking) |
| Add a new `kind` discriminant value | MINOR bump (consumers MUST handle unknown kinds gracefully) |
| Change type of existing field | MAJOR bump (breaking) |
| Change semantics of existing field | MAJOR bump (breaking) |
| Fix typo in field name | MAJOR bump (breaking) |

Consumers MUST:
- Reject documents with different MAJOR version
- Accept documents with same MAJOR, higher MINOR (forward-compatible — ignore unknown fields)
- Accept documents with same MAJOR.MINOR, any PATCH

---

# 18. Complete Example

Full IR JSON for a 2-state machine `Switch` with states `Off` and `On`, event `TOGGLE`:

```json
{
  "irVersion": "1.0.0",
  "sourceHash": "sha256:d41d8cd98f00b204e9800998ecf8427e",
  "sourceFiles": ["switch.fsm"],
  "machines": [
    {
      "id": "m-switch",
      "stableId": "Switch",
      "name": "Switch",
      "context": { "fields": [] },
      "events": [
        {
          "id": "ev-toggle",
          "stableId": "Switch:event:TOGGLE",
          "name": "TOGGLE",
          "payload": [],
          "loc": { "file": "switch.fsm", "line": 3, "col": 3, "endLine": 3, "endCol": 15 }
        }
      ],
      "externs": [],
      "root": {
        "id": "r-root",
        "name": "__root",
        "initial": "ps-initial-0",
        "priority": 0,
        "states": [
          { "kind": "initial", "id": "ps-initial-0", "target": "s-off",
            "loc": { "file": "switch.fsm", "line": 4, "col": 3, "endLine": 4, "endCol": 15 } },
          {
            "kind": "simple", "id": "s-off", "stableId": "Switch:state:Off", "name": "Off",
            "entry": [], "exit": [], "defers": [], "timers": [],
            "transitions": [
              {
                "id": "t-off-on", "stableId": "Switch:transition:off-on-TOGGLE",
                "source": "s-off", "target": "s-on",
                "trigger": { "kind": "event", "eventId": "ev-toggle" },
                "guard": null, "actions": [], "priority": 100, "internal": false,
                "loc": { "file": "switch.fsm", "line": 7, "col": 5, "endLine": 7, "endCol": 25 }
              }
            ],
            "loc": { "file": "switch.fsm", "line": 5, "col": 3, "endLine": 9, "endCol": 4 }
          },
          {
            "kind": "simple", "id": "s-on", "stableId": "Switch:state:On", "name": "On",
            "entry": [], "exit": [], "defers": [], "timers": [],
            "transitions": [
              {
                "id": "t-on-off", "stableId": "Switch:transition:on-off-TOGGLE",
                "source": "s-on", "target": "s-off",
                "trigger": { "kind": "event", "eventId": "ev-toggle" },
                "guard": null, "actions": [], "priority": 100, "internal": false,
                "loc": { "file": "switch.fsm", "line": 12, "col": 5, "endLine": 12, "endCol": 25 }
              }
            ],
            "loc": { "file": "switch.fsm", "line": 10, "col": 3, "endLine": 14, "endCol": 4 }
          }
        ],
        "loc": { "file": "switch.fsm", "line": 1, "col": 1, "endLine": 15, "endCol": 2 }
      },
      "submachines": [],
      "loc": { "file": "switch.fsm", "line": 1, "col": 1, "endLine": 15, "endCol": 2 }
    }
  ],
  "diagnostics": []
}
```

---

## Deliverable

The normative JSON Schema file for this IR is located at:

```
schema/ir/1.0.0/model.json
```

It is a JSON Schema draft-07 document. All IR producers MUST validate their output
against this schema before emitting. All IR consumers SHOULD validate documents they
receive before processing.

---

*End of FSM-SPEC-IR v1.0.0*
