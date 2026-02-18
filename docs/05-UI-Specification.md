# FSM Studio — UI/UX Specification

**Document ID:** FSM-SPEC-UI
**Version:** 1.0.0
**Status:** Normative Draft
**Applies to:** VS Code Extension v1.x · Web IDE v1.x

All measurements are in logical pixels (CSS px / dp). All timing values are in milliseconds.
All UI behaviour described in this document is normative for implementation.

---

# Part 1 — VS Code Extension

## 1.1 Overview

The VS Code extension integrates FSM Studio into the VS Code editor. It consists of:

- **FSM Explorer** — activity bar + sidebar panel (file tree, machine outline, event list)
- **Diagram Panel** — WebView editor tab, live state machine diagram
- **Simulator Panel** — WebView panel in the bottom area, interactive simulation
- **Editor integration** — syntax highlighting, diagnostics, completion, hover, go-to-definition
- **Status bar item** — machine name + diagnostic count

All WebView UI MUST use VS Code CSS variables for colour, font, and spacing.
All icons MUST use [Codicons](https://microsoft.github.io/vscode-codicons/).

---

## 1.2 Activity Bar Entry

| Property | Value |
|---|---|
| Icon | Custom SVG: stylised statechart node graph (see Style Guide §4.1) |
| Tooltip | `FSM Studio` |
| Badge | Number of diagnostics in the active file. Hidden if 0. |
| Order | After "Source Control", before "Extensions" (configurable). |

Clicking the activity bar icon opens the **FSM Explorer** sidebar.

---

## 1.3 FSM Explorer (Sidebar)

### 1.3.1 Layout

```
┌─ FSM STUDIO ──────────────────────── [⟳] [+] ─┐
│ ▾ MACHINES                                      │
│   ▾ DeviceManager                   [◉] [→]     │
│     ▾ States                                    │
│       ● Disconnected                            │
│       ▾ Connecting                              │
│       ▾ Connected                               │
│         ▾ DataPath (region)                     │
│           ● IdleData                            │
│           ● Processing                          │
│           ● Sending                             │
│         ▾ HeartbeatMonitor (region)             │
│           ● HBActive                            │
│       ● Disconnecting                           │
│     ▾ Events (8)                                │
│       ○ CMD_CONNECT(host:u32, port:u16)         │
│       ○ LINK_UP                                 │
│       ○ ...                                     │
│     ▾ Externs                                   │
│       ƒ can_retry [pure]                        │
│       ƒ reset_link                              │
│ ▾ OPEN DOCUMENTS                                │
│   device_manager.fsm                   [◉] [→]  │
└─────────────────────────────────────────────────┘
```

### 1.3.2 Toolbar Buttons

| Button | Icon | Action |
|---|---|---|
| Refresh | `$(refresh)` | Re-parse and refresh the entire tree. |
| New machine file | `$(add)` | Create a new `.fsm` file in the workspace. |

### 1.3.3 Machine Tree Item

Each machine in the tree shows:
- Machine name (bold)
- `[◉]` button (`$(debug-start)`) — Start simulator for this machine.
- `[→]` button (`$(arrow-right)`) — Open diagram for this machine.
- Badge: diagnostic count if > 0, colored by severity.

Click on machine name → navigate to `machine` declaration in editor.

### 1.3.4 State Tree Items

- Simple state: `●` prefix, monospace name.
- Composite state: `▾` / `▸` chevron prefix.
- Region: displayed as `name (region)` in italic.
- Active state (when simulator running): highlighted with accent background.
- Hover: shows stable ID if present, and full qualified path.
- Click: navigates to state declaration in editor.

### 1.3.5 Event Tree Items

- `○` prefix, event name, payload fields in muted colour.
- Click: navigates to event declaration.
- Hover: shows full payload type information.

### 1.3.6 Context Menu (right-click on state)

- "Go to Definition" — navigate to state in editor.
- "Find All References" — show all transitions targeting this state.
- "Set Breakpoint" — add simulator breakpoint on entry.
- "Copy Stable ID" — copy `@id` value to clipboard.
- "Rename…" — trigger LSP rename.

---

## 1.4 Editor Integration

### 1.4.1 File Association

Files with `.fsm` extension are automatically associated with `fsm-lang`.
Language mode displayed in status bar: `FSM-Lang`.

### 1.4.2 Syntax Highlighting

Provided via TextMate grammar. The following token scopes MUST be defined:

| Token | TextMate scope | Typical colour |
|---|---|---|
| Keywords (`machine`, `state`, `on`, etc.) | `keyword.control.fsm` | Blue |
| Type keywords (`u8`, `bool`, etc.) | `keyword.type.fsm` | Cyan |
| State names (declaration) | `entity.name.type.state.fsm` | Green/Yellow |
| Event names (declaration) | `entity.name.type.event.fsm` | Orange |
| Extern function names | `entity.name.function.extern.fsm` | Yellow |
| Enum names | `entity.name.type.enum.fsm` | Cyan |
| `@id` annotation | `keyword.other.annotation.fsm` | Muted yellow |
| String contents | `string.quoted.double.fsm` | Orange |
| Integers | `constant.numeric.fsm` | Green |
| Booleans | `constant.language.boolean.fsm` | Blue |
| Line comments | `comment.line.fsm` | Muted green |
| Doc comments (`///`) | `comment.documentation.fsm` | Muted green, italic |
| Operators (`->`, `~>`, `==`) | `keyword.operator.fsm` | Light |
| Transition arrow (`->`) | `keyword.operator.arrow.fsm` | Accent |
| Guard brackets (`[`, `]`) | `punctuation.definition.guard.fsm` | Muted |
| `ctx.` prefix | `variable.language.ctx.fsm` | Light purple |
| `payload.` prefix | `variable.language.payload.fsm` | Light purple |
| Diagnostic codes in messages | — | Rendered by VS Code |

### 1.4.3 Diagnostics

- Displayed as squiggles: red (error), yellow (warning), blue (info), grey (hint).
- Appears in the Problems panel with the stable code as prefix: `[FSM-E0001]`.
- Diagnostic codes are clickable links to online documentation.
- Related information locations (e.g., conflicting transition) shown as secondary squiggles.
- Diagnostics published within 200ms of the last keystroke (debounced).

### 1.4.4 Hover

Hovering over a state name shows:
```
state Connecting
─────────────────────────────
Stable ID: "s-connecting"
Transitions: 5 outgoing, 1 incoming
Deferred: DATA_RECEIVED
Active timers: after 10000ms
```

Hovering over an extern function:
```
extern initiate_connect()
─────────────────────────────
Called on entry of: Connecting
Called on LINK_DOWN [can_retry] transition
Generated signature: void M_initiate_connect(void)
```

Hovering over a guard expression `[can_retry]`:
```
pure extern can_retry(ctx) : bool
─────────────────────────────
Side-effect free. Used as guard.
Generated: bool M_can_retry(const M_ctx_t *ctx)
```

### 1.4.5 Code Completion

Triggered at character trigger or `Ctrl+Space`:

| Context | Completions offered |
|---|---|
| After `on ` | All declared event names. |
| After `-> ` | All state names in the machine. |
| After `~> ` | Descendant state names only. |
| After `entry :` | All declared extern (non-pure) functions, `raise`, `send`. |
| After `[` | All declared `pure extern` functions, `ctx.`, `payload.`, `else`. |
| After `ctx.` | All context field names with types. |
| After `payload.` | All payload fields of the current event. |
| After `raise ` | All declared event names. |
| After `send ` | All declared event names. |
| After `to ` | All declared machine names (imported). |
| After `feature ` | All feature flag names. |
| After `target ` | All built-in target profile names. |
| Type position | All type keywords + declared enum names. |

Completion items include documentation from `///` doc comments.

### 1.4.6 Go to Definition

`F12` / `Ctrl+Click` on:
- State name → jumps to state declaration.
- Event name → jumps to event declaration.
- Extern function → jumps to extern declaration (if in DSL) or shows `M_impl.h` location.
- Enum variant → jumps to enum declaration.
- `@id` string → stays, shows canonical model entry.

### 1.4.7 Find All References

`Shift+F12` on a state or event name: shows all transitions, entry/exit usages.

### 1.4.8 Rename

`F2` on any declared identifier: renames across the file and updates all references.
If the identifier has a `@id` annotation, a prompt appears:
> "This state has a stable ID. Renaming preserves the ID. Continue?"

### 1.4.9 Code Snippets

| Prefix | Expansion |
|---|---|
| `machine` | Machine skeleton with context, events, queue, target, initial, first state. |
| `state` | State with entry/exit. |
| `cstate` | Composite state with nested state. |
| `ostate` | Orthogonal state with two regions. |
| `on` | External transition. |
| `oni` | Internal transition. |
| `onl` | Local transition. |
| `after` | One-shot timer. |
| `every` | Periodic timer. |
| `choice` | Choice pseudo-state with two branches + else. |
| `hist` | Shallow history declaration. |
| `extern` | Extern function declaration. |
| `pure` | Pure extern function declaration. |

### 1.4.10 Document Formatting

`Shift+Alt+F` — formats the document:
- Normalises indentation (4 spaces).
- One blank line between top-level declarations.
- Consistent spacing around `:`, `->`, `~>`, `[`, `]`.
- Aligns action list commas vertically when action list > 2 items.
- Preserves all comments and blank lines within blocks.

---

## 1.5 Diagram Panel

### 1.5.1 Opening

- Command: `FSM Studio: Open Diagram` (palette + editor title button).
- Editor title button icon: `$(type-hierarchy)`.
- Opens as an editor tab to the **right** of the source file (split right).
- Tab title: `⬡ MachineName — Diagram`.
- One diagram panel per machine. Re-using the same command focuses existing panel.

### 1.5.2 Layout

```
┌─ ⬡ DeviceManager — Diagram ─────────────────── [⟳] [⊞] [↙↗] [⤓SVG] ─┐
│                                                                          │
│  ┌─────────────┐                                                         │
│  │ ● initial   │──────────────────────────────────────────────────────── │
│  └─────────────┘    on CMD_CONNECT                                       │
│                          │                                               │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │ Connecting                                                       │    │
│  │   entry: initiate_connect   defer: DATA_RECEIVED                 │    │
│  │   after 10000ms ─────────────────────────────────┐              │    │
│  └──────────────────────────────────────────────────│──────────────┘    │
│                 on LINK_UP │         on LINK_DOWN │  │                   │
│                            ▼                      ▼  │                   │
│  ┌──────────────────────────────────────────────────────────────────┐    │
│  │ Connected                          ║ DataPath      ║ Heartbeat   │    │
│  │                                    ║ ○ IdleData    ║ ○ HBActive  │    │
│  │                                    ║ ○ Processing  ║             │    │
│  │                                    ║ ○ Sending     ║             │    │
│  └──────────────────────────────────────────────────────────────────┘    │
│                                                                          │
│  ─────────────────────────── MINIMAP ─────────────────────────────────  │
│  [zoom: 85%] [▾ Layout: Hierarchical]  [Legend ▾]                       │
└──────────────────────────────────────────────────────────────────────────┘
```

### 1.5.3 Toolbar Buttons

| Button | Icon | Tooltip | Action |
|---|---|---|---|
| Refresh | `$(refresh)` | Re-parse and re-render | Force re-render from current file. |
| Fit view | `$(screen-full)` | Fit diagram to window | Zoom to fit all states. |
| Reset zoom | `$(zoom-out)` | Zoom: 100% | Reset to 100% zoom. |
| Export SVG | `$(save)` | Export as SVG | Save diagram as `.svg`. |

### 1.5.4 Canvas Behaviour

| Interaction | Behaviour |
|---|---|
| Scroll wheel | Zoom in/out (0.25× to 4×), centred on cursor. |
| Click + drag canvas | Pan. |
| Click on state | Highlight state + highlight corresponding source line (editor scroll). |
| Double-click on state | Navigate to state declaration in editor (go-to-definition). |
| Hover on state | Show tooltip: stable ID, incoming/outgoing transition count, active timers. |
| Hover on transition arrow | Show tooltip: event, guard expression, action list. |
| Right-click on state | Context menu (see §1.5.7). |

### 1.5.5 State Rendering

**Simple state box:**
```
┌─────────────────────────┐
│ StateName               │
│ entry: fn_name          │  ← rendered in muted colour, smaller font
│ exit:  fn_name          │
└─────────────────────────┘
```
- Border radius: 6px.
- Min width: 140px.
- Padding: 8px 12px.
- State name: 13px, semibold.
- Entry/exit annotations: 11px, muted colour, italic.
- Border: 1.5px, `--color-state-border`.

**Composite state box:**
```
┌─ CompositeName ──────────────────────────────────┐
│  entry: fn                                        │
│  ┌── RegionA ──────────────┐┌── RegionB ────────┐ │
│  │  ○ Child1              ││  ○ Child2          │ │
│  └────────────────────────┘└───────────────────┘ │
└───────────────────────────────────────────────────┘
```
- Border radius: 10px.
- Region divider: dashed vertical line, `--color-region-divider`.
- Region label: 10px, uppercase, `--color-text-tertiary`.

**Pseudo-states:**

| Pseudo-state | Shape | Size |
|---|---|---|
| Initial | Filled circle | 12px |
| Final | Double circle | 16px |
| History (shallow) | Circle with `H` | 20px |
| History (deep) | Circle with `H*` | 20px |
| Choice | Diamond | 20×20px |
| Junction | Filled circle (small) | 10px |
| Fork | Filled rectangle (horizontal) | 8×40px |
| Join | Filled rectangle (horizontal) | 8×40px |

### 1.5.6 Transition Arrow Rendering

- Stroke: 1.5px, `--color-transition`.
- Arrowhead: filled triangle, 6×8px.
- Label position: midpoint of arc, background fill to ensure legibility.
- Label format: `EVENT [guard] : action` — guard and action shown if present.
- Guard shown in `[ ]` brackets, muted colour.
- Action shown after `:`, even more muted.
- Self-transition: rendered as a small arc on the right side of the state.
- Timed transition: dashed line with clock icon (⏱) in label.
- Completion transition: dotted line with label `done`.

### 1.5.7 Diagram Context Menu

Right-click on state:
- "Go to Source" — navigate to declaration.
- "Set Entry Breakpoint" — add breakpoint (active if simulator running).
- "Set Exit Breakpoint"
- "Expand/Collapse" — toggle composite state expansion.
- ─────
- "Copy State Name"
- "Copy Stable ID"

Right-click on canvas:
- "Fit View"
- "Reset Zoom"
- "Export as SVG"
- "Export as PNG"

### 1.5.8 Simulator Overlay (when simulator is running)

- **Active states**: box border changes to `--color-accent` (2.5px), inner glow effect.
- **Executed transition**: the arrow animates (moves dash pattern) for 600ms.
- **Recently exited state**: border briefly changes to `--color-warning`, fades in 400ms.
- **Breakpoint state**: red dot badge in the top-right corner of the state box.

### 1.5.9 Live Update

- When the source `.fsm` file changes: diagram re-renders within 500ms.
- Re-render is incremental: only changed subtrees are re-laid-out.
- If re-parse fails (syntax error): diagram shows last valid state + error banner:
  ```
  ⚠ Diagram shows last valid state. Fix parse errors to update.
  ```
- Layout algorithm: ELK Layered (hierarchical). Stable between renders (same input → same layout).
- Zoom and pan position are preserved across re-renders if the machine identity is the same.

### 1.5.10 Legend Panel

Collapsible legend in the bottom-left corner of the canvas:

```
○ Initial state        ◎ Final state
─── External transition    ─ ─ Completion transition
⏱ ─── Timed transition    [H] Shallow history
```

Toggle via "Legend ▾" button in toolbar.

---

## 1.6 Simulator Panel

### 1.6.1 Opening

- Command: `FSM Studio: Open Simulator`.
- Opens in the **bottom panel area** (same row as Terminal, Problems).
- Panel title: `⬡ FSM Simulator`.
- One simulator panel per workspace. Multiple machines can be loaded sequentially.

### 1.6.2 Full Panel Layout

```
┌─ ⬡ FSM Simulator ────────────────────────────────────────────────────────────────┐
│ Machine: DeviceManager   [▶ Run] [⏸ Pause] [⏮ Step] [⏹ Stop] [↺ Reset]          │
│ Status: ● RUNNING   Clock: 10250 ms                                               │
├──────────────────────────────┬───────────────────────────────┬────────────────────┤
│ ACTIVE CONFIGURATION         │ CONTEXT INSPECTOR             │ EVENT INJECTION     │
│                              │                               │                     │
│ ▾ DeviceManager              │  Field         Value          │ Event: [PACKET   ▾] │
│   ▾ Connected                │  retry_count   0        [✎]  │ kind:  [DATA     ▾] │
│     ● DataPath               │  alarm_active  false    [✎]  │ len:   [12         ]│
│       ● Processing           │                               │                     │
│     ● HeartbeatMonitor       │                               │ [  Inject Event  ]  │
│       ● HBActive             │                               │                     │
│                              │  TIMER INSPECTOR              │                     │
│                              │  heartbeat   4250 ms left     │                     │
│                              │  send_to     ─ (idle)         │                     │
├──────────────────────────────┴───────────────────────────────┴────────────────────┤
│ TRACE LOG                                                           [⤓] [🗑] [⏸]  │
│ ────────────────────────────────────────────────────────────────────────────────── │
│  10250  ▶ PACKET dispatched  {kind: DATA, len: 12}                                 │
│  10250  ⤵ Processing → exit                                                       │
│  10250  ⟶ transition Processing → IdleData  [payload.kind == PacketType.DATA]     │
│  10250  ⤴ IdleData → entry                                                        │
│   5000  ⏱ timer: heartbeat fired → send_heartbeat()                               │
│   5000  ⤴ HBActive → on HEARTBEAT_ACK (internal)                                  │
│ ────────────────────────────────────────────────────────────────────────────────── │
│ [  ▶ BREAKPOINTS (2)  ]  s-connecting: entry    Connected: LINK_DOWN transition   │
└───────────────────────────────────────────────────────────────────────────────────┘
```

### 1.6.3 Toolbar

| Button | Icon | Behaviour |
|---|---|---|
| Run | `$(debug-start)` `▶ Run` | Start/continue execution until breakpoint or stop. |
| Pause | `$(debug-pause)` `⏸ Pause` | Pause at current state. |
| Step | `$(debug-step-over)` `⏮ Step` | Dispatch one event from the queue. |
| Stop | `$(debug-stop)` `⏹ Stop` | Stop simulator, clear state. |
| Reset | `$(debug-restart)` `↺ Reset` | Reset to initial state, clear trace. |

Status badge colours:
- `● IDLE` — `--color-text-tertiary`
- `● RUNNING` — `--color-success`
- `● PAUSED` — `--color-warning`
- `● STOPPED` — `--color-error`
- `● BREAKPOINT` — `--color-accent`

### 1.6.4 Active Configuration Panel

- Shows the full active state tree in a collapsible tree view.
- Active states shown with `●` (filled circle, accent colour).
- Inactive composite states shown with `○`.
- Regions shown as indented group labels.
- Refreshes after every dispatch cycle (< 16ms, one animation frame).

### 1.6.5 Context Inspector

- Two-column table: `Field Name` | `Current Value`.
- Values are displayed in monospace font.
- `[✎]` button on each row: click to open inline editor for that field.
  - Inline editor: text input with type-appropriate validation (integer range, boolean toggle, enum dropdown).
  - `Enter` or `Tab` to confirm. `Escape` to cancel.
  - Editing context during a running simulation is permitted; value takes effect immediately.
- Values that changed in the last dispatch cycle briefly flash accent background (300ms).

### 1.6.6 Timer Inspector

- One row per active timer.
- Shows: timer name, remaining time (ms), progress bar.
- Idle timer slots show `─ (idle)`.
- `[⏩ Fire]` button: immediately fire the timer (for testing).

### 1.6.7 Event Injection

- Dropdown selector: all declared events.
- When an event with payload is selected, payload field inputs appear below.
  - Integer fields: number input with min/max from type.
  - Boolean fields: toggle switch.
  - Enum fields: dropdown of enum variants.
- `[ Inject Event ]` button: enqueue the event.
- Keyboard shortcut: `Enter` when focus is in event injection form.
- Injected events appear immediately in the trace log as `⊕ EVENT injected`.

### 1.6.8 Trace Log

Each entry has:
- Virtual clock timestamp (ms), left-aligned, monospace, muted.
- Icon indicating type:
  - `▶` — event dispatched from queue.
  - `⊕` — event injected manually.
  - `⤵` — state exit.
  - `⤴` — state entry.
  - `⟶` — transition executed.
  - `⏱` — timer fired.
  - `⚠` — queue overflow.
- Description text.
- Guard expression shown for transitions in muted colour.

Toolbar:
- `[⤓]` — export trace as JSON.
- `[🗑]` — clear trace.
- `[⏸]` — pause auto-scroll.

Clicking any trace entry navigates the diagram to the state/transition involved.
Clicking a transition entry highlights the corresponding arrow in the diagram for 2s.

### 1.6.9 Breakpoints Bar

Collapsed by default. Expanding shows a list of all active breakpoints:
- Each breakpoint shows: type (entry/exit/transition), state/transition name, `[✕]` remove button.
- "Add Breakpoint…" button opens a dropdown to select state or transition.
- Breakpoints persist across simulator resets within the session.

---

## 1.7 Status Bar

| Element | Content | Position |
|---|---|---|
| Language mode | `FSM-Lang` | Right side, standard VS Code language indicator. |
| Machine indicator | `⬡ DeviceManager` | Left side, shows machine of the current `.fsm` file. |
| Diagnostic count | `$(error) 2  $(warning) 1` | Left side, next to machine indicator. |
| Simulator status | `$(debug-start) RUNNING` | Left side when simulator is active. |

Clicking machine indicator → opens diagram panel.
Clicking diagnostic count → opens Problems panel filtered to current file.
Clicking simulator status → opens simulator panel.

---

## 1.8 Command Palette Commands

All commands prefixed with `FSM Studio:`.

| Command | Default Keybinding | Description |
|---|---|---|
| `FSM Studio: Open Diagram` | `Ctrl+Shift+D` | Open diagram for current file. |
| `FSM Studio: Open Simulator` | `Ctrl+Alt+S` | Open simulator panel. |
| `FSM Studio: Generate Code` | `Ctrl+Shift+G` | Run code generator for current file. |
| `FSM Studio: Compile to IR` | — | Emit canonical JSON model. |
| `FSM Studio: Format Document` | `Shift+Alt+F` | Format current `.fsm` file. |
| `FSM Studio: Restart Language Server` | — | Kill and restart `fsm-lsp`. |
| `FSM Studio: Export Diagram as SVG` | — | Export active diagram. |
| `FSM Studio: Export Diagram as PNG` | — | Export active diagram. |
| `FSM Studio: Show All Diagnostics` | — | Open Problems filtered to FSM-Lang. |
| `FSM Studio: Inject Event…` | `Ctrl+Shift+E` | Open event injection quick-pick. |
| `FSM Studio: Step Simulator` | `F10` | Single step (when simulator active). |
| `FSM Studio: Run Simulator` | `F5` | Run (when simulator active). |
| `FSM Studio: Pause Simulator` | `F6` | Pause (when simulator active). |
| `FSM Studio: Reset Simulator` | `Shift+F5` | Reset (when simulator active). |

---

# Part 2 — Web IDE

## 2.1 Overview

The Web IDE is a browser-based development environment for FSM-Lang. It targets:
- Users without VS Code.
- Sharable, embeddable demos.
- CI/CD integration (run in a headless browser).
- Team collaboration workflows.

Technology: React + Monaco Editor + Vite. Compiler runs as WASM.

---

## 2.2 Application Shell Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ HEADER: [⬡ FSM Studio]   [File ▾] [Edit ▾] [Run ▾] [Help ▾]   [🌙] [⤓] [Share]│
├──────┬──────────────────────────┬───────────────────────────────────────────┤
│      │                          │                                           │
│ SIDE │     CODE EDITOR          │          DIAGRAM / SIMULATOR              │
│ BAR  │     (Monaco)             │                                           │
│      │                          │                                           │
│  ↕   │                          │   [Diagram] [Simulator]  ← tab switcher  │
│      │                          │                                           │
│      │                          │                                           │
├──────┴──────────────────────────┴───────────────────────────────────────────┤
│ BOTTOM PANEL: [Problems] [Output] [Trace]                    [✕ close panel]│
└─────────────────────────────────────────────────────────────────────────────┘
```

### Panel proportions (default, resizable)

| Panel | Default width/height |
|---|---|
| Sidebar | 220px, resizable 160–360px. |
| Code editor | ~45% of remaining width. |
| Diagram/Simulator | ~55% of remaining width. |
| Bottom panel | 200px when open, collapsible. |

All panel splitters are draggable. Double-click a splitter to reset to default.

---

## 2.3 Header

```
┌────────────────────────────────────────────────────────────────────────────┐
│  ⬡ FSM Studio    File ▾   Edit ▾   Run ▾   Help ▾          🌙  ⤓  Share  │
└────────────────────────────────────────────────────────────────────────────┘
```

Height: 48px.
Background: `--bg-surface`.
Border-bottom: 1px `--color-border`.

### File menu

| Item | Shortcut | Action |
|---|---|---|
| New File | `Ctrl+N` | Create blank `.fsm` file. |
| Open File… | `Ctrl+O` | Open file picker, loads `.fsm` or project zip. |
| Open Project… | — | Open `.zip` project archive. |
| Save | `Ctrl+S` | Save active file (browser storage or download). |
| Save All | `Ctrl+Shift+S` | Save all open files. |
| Download File | — | Download current file as `.fsm`. |
| Download Project | — | Download project as `.zip`. |
| ──── | | |
| Close File | — | Close current file tab. |

### Edit menu

| Item | Shortcut | Action |
|---|---|---|
| Undo | `Ctrl+Z` | |
| Redo | `Ctrl+Shift+Z` | |
| Find | `Ctrl+F` | |
| Find & Replace | `Ctrl+H` | |
| Format Document | `Shift+Alt+F` | |
| ──── | | |
| Go to Definition | `F12` | |
| Find References | `Shift+F12` | |

### Run menu

| Item | Shortcut | Action |
|---|---|---|
| Compile | `Ctrl+B` | Parse + validate + analyze. |
| Generate C99 | — | Generate and show C code. |
| Start Simulator | `F5` | Load into simulator, go to Simulator tab. |
| Step | `F10` | One dispatch step. |
| Pause | `F6` | |
| Reset | `Shift+F5` | |

### Toolbar icons

| Icon | Action |
|---|---|
| 🌙 / ☀ | Toggle dark / light theme. |
| ⤓ | Download project zip. |
| Share | Copy share URL to clipboard. Encodes project state in URL. |

---

## 2.4 Sidebar

```
┌──────────────────────┐
│ 🗂 FILES             │
│ ▾ project/           │
│   ▾ src/             │
│     device_mgr.fsm ● │
│     common/          │
│       events.fsm     │
├──────────────────────┤
│ ⬡ FSM EXPLORER      │
│ ▾ DeviceManager      │
│   ▾ States           │
│     ...              │
│   ▾ Events           │
│     ...              │
└──────────────────────┘
```

Two sections, separated by a divider. Each section is independently collapsible.

**Files section:**
- Shows project directory tree.
- `.fsm` files shown with `⬡` icon.
- Active (focused) file has bold name.
- Modified but unsaved file: `●` badge (accent colour) after name.
- Right-click → New File, Rename, Delete, Duplicate.

**FSM Explorer section:**
- Same content as VS Code extension sidebar §1.3.
- Synced to the active editor file.

---

## 2.5 Code Editor (Monaco)

- Monaco Editor, full VS Code editor experience.
- FSM-Lang language registered: syntax highlighting, completion, hover, diagnostics.
- LSP client runs as a Web Worker.
- Compiler (WASM) runs in a separate Web Worker.
- Editor tabs: one tab per open file. Tabs shown above editor.
- Tab: filename, `●` if unsaved, `✕` to close.
- Minimap enabled by default (can be toggled via menu).

**Editor settings exposed in UI (via ⚙ Settings panel):**
- Tab size (default: 4).
- Font size.
- Word wrap.
- Theme override (respects header toggle).

---

## 2.6 Diagram Tab

Identical feature set to VS Code extension diagram panel (§1.5) with adaptations:
- No VS Code-specific CSS variables → uses Web IDE design tokens.
- Export uses browser download API instead of VS Code save dialog.
- Toolbar integrated into the panel tab bar (not in VS Code editor title area).

---

## 2.7 Simulator Tab

Identical feature set to VS Code simulator panel (§1.6) with adaptations:
- Layout is vertical (stacked sections) instead of horizontal to fit narrower panel.
- WebSocket connects to `fsm-sim` at configured URL (default: `ws://localhost:7842`).
- If `fsm-sim` is not running, shows a connection error banner with setup instructions.

### Simulator tab layout (vertical stacking)

```
┌─ Simulator ────────────────────────────────────────────────────────────────┐
│ [▶ Run] [⏸] [⏮ Step] [⏹] [↺]     Status: ● RUNNING   Clock: 5000ms      │
├──────────────────────────────────────────────────────────────────────────── │
│ EVENT INJECTION                                                             │
│ Event: [PACKET ▾]   kind: [DATA ▾]   len: [12]   [ Inject ]               │
├────────────────────────────────┬───────────────────────────────────────────┤
│ ACTIVE CONFIGURATION           │ CONTEXT INSPECTOR                         │
│  ▾ Connected                   │  retry_count  0   [✎]                    │
│    ● DataPath > Processing     │  alarm_active false [✎]                   │
│    ● HeartbeatMonitor > HBActive│                                          │
├────────────────────────────────┴───────────────────────────────────────────┤
│ TRACE LOG                                              [⤓ Export] [🗑 Clear]│
│ 5000  ▶ HEARTBEAT_ACK dispatched                                           │
│ 5000  ⤵ HBActive → on HEARTBEAT_ACK : on_hb_received                      │
│ ...                                                                        │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 2.8 Bottom Panel

Three tabs: **Problems**, **Output**, **Trace**.

### Problems tab

- Lists all diagnostics from the last compile.
- Columns: Severity icon | Code | Message | File | Line.
- Click row → jump to location in editor.
- Filterable by severity (errors / warnings / hints).

### Output tab

- Shows raw output from compiler CLI invocations.
- Monospace font, auto-scrolls.
- Colour-coded: errors red, warnings yellow, info default.

### Trace tab

- Full simulator trace log (same as trace section in Simulator tab).
- Available even when Simulator tab is not open.

---

## 2.9 Settings Panel

Accessible via `⚙` icon (bottom-left of sidebar, above status bar).

```
┌─ Settings ──────────────────────────────────────────────────────┐
│ EDITOR                                                           │
│ Tab size          [4]                                            │
│ Font size         [13px]                                         │
│ Word wrap         [○ Off  ● On]                                  │
│                                                                  │
│ CODE GENERATION                                                  │
│ Default profile   [C99 ▾]                                        │
│ Strategy          [switch_based ▾]                               │
│                                                                  │
│ SIMULATOR                                                        │
│ Server URL        [ws://localhost:7842          ]                 │
│ Auto-connect      [● On  ○ Off]                                  │
│                                                                  │
│ APPEARANCE                                                       │
│ Theme             [Dark ▾]                                       │
│ Diagram layout    [Hierarchical ▾]                               │
└─────────────────────────────────────────────────────────────────┘
```

Settings are persisted in `localStorage`.

---

## 2.10 Share Feature

`Share` button in header:
1. Serialises all open files into a compressed, base64-encoded URL fragment.
2. Copies the URL to clipboard.
3. Shows toast: "Link copied to clipboard".

When opening a share URL:
- Files are decoded and loaded into the editor automatically.
- A banner shows: "Opened from share link. [Save to local project]".

Maximum share payload: 64KB uncompressed. If exceeded, shows error:
"Project too large to share as URL. Use Download Project instead."

---

## 2.11 Responsive Behaviour

| Viewport width | Adaptation |
|---|---|
| > 1200px | Full three-column layout as described. |
| 900–1200px | Sidebar collapses to icon-only strip by default. |
| 600–900px | Diagram and Editor shown as tabs (not side-by-side). |
| < 600px | Not supported. Shows "Please use a larger screen." |

---

# Part 3 — Interaction Patterns

## 3.1 Error States

### Parse error in editor

- Red squiggle on offending token.
- Stable code in Problems panel.
- Diagram shows last valid state + yellow banner.
- Simulator cannot be started; "Compile errors must be fixed first" tooltip on Run button.

### Simulator disconnected

- Status badge: `⚠ DISCONNECTED` in warning colour.
- Retry button shown.
- All simulator controls disabled.
- Trace log preserved.

### Queue overflow

- Toast notification: "⚠ Queue overflow: event dropped (drop_oldest policy)."
- Trace log entry: `⚠ OVERFLOW: EVENT_NAME dropped`.
- Diagram: queue overflow indicator badge on machine name.

## 3.2 Toast Notifications

- Bottom-right corner.
- Stack vertically (newest on top).
- Auto-dismiss after 4 seconds.
- Manual dismiss via `✕` button.
- Max 3 visible at once (oldest dismissed if exceeded).

| Type | Background | Icon |
|---|---|---|
| Success | `--color-success` + 15% opacity bg | `✓` |
| Warning | `--color-warning` + 15% opacity bg | `⚠` |
| Error | `--color-error` + 15% opacity bg | `✕` |
| Info | `--color-accent` + 15% opacity bg | `ℹ` |

## 3.3 Loading States

When the compiler is running (WASM or CLI):
- Status bar (VS Code) / header (Web IDE) shows spinner: `⟳ Compiling…`.
- Diagram tab shows spinner overlay if re-rendering.
- Simulator controls disabled during compilation.

## 3.4 Keyboard Navigation

All interactive elements reachable by `Tab`.
All modal dialogs trap focus.
`Escape` closes dropdowns, tooltips, and modals.
All toolbar buttons have `aria-label`.

---

# Part 4 — Accessibility

- All interactive elements MUST have accessible names (`aria-label` or visible label).
- Colour MUST NOT be the sole distinguisher of information (icons + text + colour).
- Minimum contrast ratio: 4.5:1 for normal text, 3:1 for large text (WCAG 2.1 AA).
- Diagram: active states MUST be indicated by border + icon, not colour alone.
- All keyboard shortcuts MUST be reachable without a mouse.
- Simulator trace: screen reader announces new trace entries (ARIA live region).
