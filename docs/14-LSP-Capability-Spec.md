# FSM Studio — Language Server Capability Specification

**Document ID:** FSM-SPEC-LSP
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL, FSM-SPEC-DIAG, FSM-SPEC-IR

Specifies all LSP 3.17 capabilities provided by the `fsm-lang-server`. Each section
defines the exact shape of requests, responses, and the behavior expected from a
conformant implementation.

---

# 1. Server Identification

| Attribute | Value |
|---|---|
| Server name | `fsm-lang-server` |
| Protocol version | LSP 3.17 |
| Default transport | stdio |
| Alternate transport | TCP (via `--port N` flag) |
| File types served | `*.fsm` |
| Language ID | `fsm-lang` |

---

# 2. Initialization

### Client `initializationOptions`

```json
{
  "fsmLang": {
    "compilerPath":             "/usr/local/bin/fsm",
    "maxDiagnosticCount":       100,
    "enableExperimentalFeatures": false
  }
}
```

### Server `ServerCapabilities` response

```json
{
  "textDocumentSync": {
    "openClose": true,
    "change": 2
  },
  "completionProvider": {
    "triggerCharacters": [".", ":", "@", "[", " "]
  },
  "hoverProvider": true,
  "definitionProvider": true,
  "referencesProvider": true,
  "documentHighlightProvider": true,
  "documentSymbolProvider": true,
  "workspaceSymbolProvider": true,
  "codeActionProvider": {
    "codeActionKinds": ["quickfix", "refactor"]
  },
  "renameProvider": {
    "prepareProvider": true
  },
  "foldingRangeProvider": true,
  "semanticTokensProvider": {
    "legend": {
      "tokenTypes":     ["namespace","type","enum","function","variable","keyword","string","number","operator","comment","decorator"],
      "tokenModifiers": ["declaration","readonly","deprecated","static"]
    },
    "full": true,
    "range": true
  },
  "inlayHintProvider": true,
  "documentFormattingProvider": true,
  "diagnosticProvider": {
    "interFileDependencies": false,
    "workspaceDiagnostics": false
  }
}
```

---

# 3. Workspace Configuration

The server reads the following keys via `workspace/configuration` on startup and on
`workspace/didChangeConfiguration`:

| Key | Type | Default | Description |
|---|---|---|---|
| `fsmLang.maxProblems` | integer | 100 | Max diagnostics reported per file |
| `fsmLang.enableInlayHints` | boolean | true | Toggle all inlay hints |
| `fsmLang.inlayHints.showTransitionPriorities` | boolean | true | Show non-default priority values |
| `fsmLang.inlayHints.showStateTypes` | boolean | false | Show `(composite)` / `(parallel)` labels |
| `fsmLang.inlayHints.showTimerDurations` | boolean | true | Show human-readable duration next to ms values |
| `fsmLang.diagnostics.enableStyleWarnings` | boolean | true | Emit FSM-W02xx style warnings |
| `fsmLang.diagnostics.enableHints` | boolean | true | Emit FSM-H0xxx hints |
| `fsmLang.diagnostics.actionComplexityThreshold` | integer | 10 | Statement count for FSM-W0201 |
| `fsmLang.codegen.defaultTarget` | string | `"c99"` | `"c99"` \| `"cpp17"` |
| `fsmLang.format.indentSize` | integer | 4 | Spaces per indent level |
| `fsmLang.format.bracketStyle` | string | `"same-line"` | `"same-line"` \| `"next-line"` |

---

# 4. `textDocument/completion`

### Trigger Characters

| Character | Completion context |
|---|---|
| `.` | After `ctx` or `payload` — offer field names |
| `:` | After `->` or timer duration — context-sensitive |
| `@` | Offer `@id(` snippet |
| `[` | Start of guard expression — offer field names and extern names |
| ` ` (space) | After `on`, `raise`, `send`, `defer` — offer event names |

### Completion Item Categories

**Keywords** — `kind: 14` (Keyword)
```json
{
  "label": "machine",
  "kind": 14,
  "detail": "(keyword)",
  "insertTextFormat": 1,
  "insertText": "machine ${1:Name} {\n    $0\n}"
}
```
Full keyword list: `machine`, `state`, `parallel`, `composite`, `event`, `extern`,
`pure`, `context`, `initial`, `final`, `history`, `shallow`, `deep`, `choice`,
`junction`, `fork`, `join`, `entry`, `exit`, `on`, `after`, `every`, `defer`,
`raise`, `send`, `to`, `if`, `else`, `while`, `for`, `priority`, `internal`.

**State names** — `kind: 7` (Class), triggered after `->`, `~>`, `initial ->`,
`history default ->`
```json
{
  "label": "Running",
  "kind": 7,
  "detail": "composite state",
  "documentation": { "kind": "markdown", "value": "The Running state manages motor operation." }
}
```

**Event names** — `kind: 20` (Event), triggered after `on `, `raise `, `send `, `defer `
```json
{
  "label": "START",
  "kind": 20,
  "detail": "(target_speed: u16)",
  "documentation": "Payload: target_speed: u16"
}
```

**Context fields** — `kind: 5` (Field), triggered after `ctx.`
```json
{
  "label": "speed",
  "kind": 5,
  "detail": "u16",
  "labelDetails": { "description": "context field" }
}
```

**Payload fields** — `kind: 5` (Field), triggered after `payload.`

**Extern functions** — `kind: 3` (Function), triggered in action position or guard position
```json
{
  "label": "isSpeedValid",
  "kind": 3,
  "detail": "pure (ctx: Motor_t*, payload: Motor_StartPayload_t*) → bool",
  "documentation": "Guard: checks that the target speed is within safe range."
}
```

**Snippets** — offered on empty lines within state bodies
```json
[
  { "label": "on EVENT -> TARGET", "insertText": "on ${1:EVENT} -> ${2:Target};", "kind": 15 },
  { "label": "after Xms -> TARGET", "insertText": "after ${1:1000}ms -> ${2:Timeout};", "kind": 15 },
  { "label": "entry block", "insertText": "entry: {\n    $0\n}", "kind": 15 },
  { "label": "exit block",  "insertText": "exit: {\n    $0\n}", "kind": 15 }
]
```

---

# 5. `textDocument/hover`

Hover responses MUST use GitHub-flavoured Markdown.

### State hover

```markdown
## state `Idle` *(simple)*

**Transitions out:** 2
**Entry actions:** `startTimer()`
**Exit actions:** `stopTimer()`

*Motor.fsm:12:3*
```

### Transition hover

```markdown
## transition `Idle → Running`

**Trigger:** `START(target_speed: u16)`
**Guard:** `ctx.speed > 0`
**Actions:** `logStart()`
**Priority:** 100 *(default)*
```

### Event hover

```markdown
## event `START`

**Payload fields:**
- `target_speed: u16`

*Used on 2 transitions*
```

### Extern hover

```markdown
## `pure` extern `isSpeedValid`

**Signature:** `(Motor_t *ctx, Motor_StartPayload_t *payload) → bool`
**Used as guard on:** 2 transitions
```

### Context field hover

```markdown
## context field `speed: u16`

**Default value:** `0`
**Referenced in:** 4 guards, 3 action assignments
```

---

# 6. `textDocument/definition`

| Hover target | Go-to location |
|---|---|
| State name in transition target | State declaration (the `state NAME {` line) |
| Event name in `on E`, `raise E`, `send E`, `defer E` | Event declaration |
| Extern name in guard or action | Extern declaration |
| `ctx.field` | Field in `context { }` block |
| Machine name in `send E to M` | Machine declaration |
| `@id("X")` stable ID string | First declaration with that stable ID |

If the target is in a different file (same project), the definition MUST jump to
that file. Cross-file definition requires the server to have indexed the full
workspace at startup.

---

# 7. `textDocument/references`

Returns all reference locations for the entity under cursor.

| Entity | References include |
|---|---|
| State | All `->` transition targets, all `initial ->`, all `history default ->`, all fork targets, all join sources |
| Event | All `on E`, `raise E`, `send E`, `defer E` |
| Extern | All call sites in guards and action blocks |
| Context field | All `ctx.field` reads and writes |

Cross-file references are supported within the same workspace.

---

# 8. `textDocument/rename` + `prepareRename`

### `prepareRename`

Returns the range of the identifier under the cursor. The client shows this as the
pre-filled rename input.

Supported entities: state names, event names, extern names, context field names.

NOT supported: machine names (too broad — changes generated file names), `@id`
stable ID strings (renaming stable IDs is a breaking change).

### `rename`

Returns a `WorkspaceEdit` that updates all references in the same file atomically.

```json
{
  "changes": {
    "file:///path/to/motor.fsm": [
      { "range": { "start": {...}, "end": {...} }, "newText": "NewName" },
      { "range": { "start": {...}, "end": {...} }, "newText": "NewName" }
    ]
  }
}
```

Cross-file rename: NOT supported in v1. The server MUST emit a user-visible error
message if the entity is referenced in a different file: `"Cross-file rename is not
supported in LSP v1. Rename in each file manually."`

---

# 9. `textDocument/codeAction`

Code actions are offered when the cursor is within a diagnostic span.

| Diagnostic code | Action kind | Title | Effect |
|---|---|---|---|
| FSM-E0100 (Unknown state) | `quickfix` | "Create state `X`" | Inserts skeleton `state X { }` |
| FSM-E0107 (No initial) | `quickfix` | "Add initial declaration" | Inserts `initial -> FirstState;` |
| FSM-E0106 (Non-pure extern in guard) | `quickfix` | "Add `pure` to extern `X`" | Inserts `pure` modifier |
| FSM-W0200 (Loop in action) | `quickfix` | "Suppress with comment" | Inserts `// fsm-lint:disable FSM-W0200` |
| FSM-W0500 (Unused extern) | `quickfix` | "Remove unused extern `X`" | Deletes the extern declaration |
| FSM-E0300 (Nondeterminism) | `quickfix` | "Add priority clauses" | Adds `priority 1` / `priority 2` to conflicting transitions |
| FSM-E0022 (Duplicate event) | `quickfix` | "Remove duplicate event `X`" | Deletes the second declaration |

### Refactor actions (cursor-position, not tied to diagnostics)

| Trigger | Action | Effect |
|---|---|---|
| Cursor inside action block with > 5 statements | `refactor.extract` "Extract to extern function" | Moves block to `extern` declaration + call site |
| Cursor on `while` or `for` in action block | `refactor.extract` "Extract loop to extern" | Moves loop body to extern function |

---

# 10. `textDocument/semanticTokens`

### Token Type Legend (index = legend position)

| Index | Type | Applied to |
|---|---|---|
| 0 | `namespace` | Machine names |
| 1 | `type` | State names (in declarations and references) |
| 2 | `enum` | Event names |
| 3 | `function` | Extern function names |
| 4 | `variable` | Context fields (`ctx.X`), payload fields (`payload.X`) |
| 5 | `keyword` | All FSM-Lang keywords |
| 6 | `string` | Stable ID strings, doc comment text |
| 7 | `number` | Integer literals, timer duration values |
| 8 | `operator` | `->`, `~>`, `:`, `=`, `[`, `]`, `&&`, `\|\|`, comparison operators |
| 9 | `comment` | Line comments, block comments |
| 10 | `decorator` | `@id(...)` annotation |

### Token Modifier Legend

| Index | Modifier | Applied when |
|---|---|---|
| 0 | `declaration` | Token is the declaration site (not a reference) |
| 1 | `readonly` | Payload fields (read-only in action blocks) |
| 2 | `deprecated` | Entity has a deprecated stable ID (reserved for future use) |
| 3 | `static` | Enum variant names within event payload type context |

---

# 11. `textDocument/inlayHints`

Inlay hints are rendered inline in the editor as grayed-out text after the relevant token.

| Condition | Hint text | Example |
|---|---|---|
| Non-default priority on transition | `// priority: 50` | After `on START [...]-> Fast;` |
| Composite/parallel state type | `// 3 substates` | After `composite Operational {` |
| Timer duration > 1000ms | `// 1.5 s` | After `after 1500ms` |
| Timer duration > 60000ms | `// 1 min 30 s` | After `after 90000ms` |
| Extern with inferrable param names | `// (ctx, payload)` | After `isSpeedValid` call in guard |

---

# 12. `textDocument/foldingRange`

Folding regions:

| Start token | End token | Description |
|---|---|---|
| `machine NAME {` | matching `}` | Full machine body |
| `state NAME {` | matching `}` | State body |
| `composite NAME {` | matching `}` | Composite state body |
| `parallel NAME {` | matching `}` | Parallel state body |
| `region NAME {` | matching `}` | Region body |
| `context {` | matching `}` | Context block |
| `/*` | matching `*/` | Block comment |
| First `on E` of a contiguous group | Last `on E` of group | Transition group (collapsed to `on ... (N transitions)`) |

---

# 13. `textDocument/documentSymbol`

Returns a hierarchical symbol tree:

```
Machine: Motor
  ├── context
  │     ├── speed: u16
  │     └── running: bool
  ├── events
  │     ├── START (payload: target_speed: u16)
  │     └── STOP
  ├── externs
  │     └── isSpeedValid (pure)
  └── states
        ├── Idle (simple)
        │     └── after 5000ms -> Error
        ├── Running (composite)
        │     ├── Normal (simple)
        │     └── Fast (simple)
        └── Error (simple, final)
```

---

# 14. Server Lifecycle

| Event | Behavior |
|---|---|
| Server start | Scan all `*.fsm` in workspace; analyze all files; cache results |
| `textDocument/didOpen` | Analyze the opened file immediately; publish diagnostics |
| `textDocument/didChange` | Buffer change; reset 200ms debounce timer |
| Debounce timer fires | Re-lex, re-parse, re-analyze; publish updated diagnostics |
| `textDocument/didClose` | Remove from active document set; retain analysis cache |
| Server crash | VS Code extension detects exit; restarts after 3s; up to 3 attempts |
| `shutdown` | Graceful stop; flush any pending analysis |

The 200ms debounce SHOULD be configurable via `fsmLang.debounceMs` (range: 50–2000ms).

---

*End of FSM-SPEC-LSP v1.0.0*
