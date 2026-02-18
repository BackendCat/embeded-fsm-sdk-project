# FSM Studio — VS Code Extension Manifest Specification

**Document ID:** FSM-SPEC-VSCE
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-LSP, FSM-SPEC-UI, FSM-SPEC-TM

Specifies the exact `package.json` contributions for the VS Code extension: language
registration, commands, keybindings, configuration schema, menus, and activation events.

---

# 1. Extension Identity

```json
{
  "name": "fsm-lang",
  "displayName": "FSM Studio",
  "description": "FSM-Lang language support, diagrams, and simulator for embedded state machines.",
  "version": "0.1.0",
  "publisher": "fsmstudio",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/fsmstudio/sm-sdk"
  },
  "icon": "assets/icon.png",
  "galleryBanner": { "color": "#0D1117", "theme": "dark" },
  "engines": { "vscode": "^1.85.0" },
  "categories": ["Programming Languages", "Linters", "Visualization", "Debuggers"]
}
```

---

# 2. Activation Events

```json
"activationEvents": [
  "onLanguage:fsm-lang",
  "workspaceContains:**/*.fsm"
]
```

The extension activates when:
1. A `.fsm` file is opened (via `onLanguage:fsm-lang`).
2. The workspace contains any `.fsm` file at startup (via `workspaceContains`).

The extension MUST NOT activate on VS Code startup without a `.fsm` file present.

---

# 3. Language Contribution

```json
"contributes": {
  "languages": [
    {
      "id": "fsm-lang",
      "aliases": ["FSM-Lang", "fsm"],
      "extensions": [".fsm"],
      "mimetypes": ["text/x-fsm-lang"],
      "icon": {
        "light": "./assets/file-icon-light.svg",
        "dark":  "./assets/file-icon-dark.svg"
      },
      "configuration": "./language-configuration.json"
    }
  ],
  "grammars": [
    {
      "language": "fsm-lang",
      "scopeName": "source.fsm",
      "path": "./syntaxes/fsm-lang.tmLanguage.json"
    }
  ],
  "snippets": [
    {
      "language": "fsm-lang",
      "path": "./snippets/fsm-lang.json"
    }
  ]
}
```

---

# 4. Commands

```json
"commands": [
  {
    "command": "fsm.openDiagram",
    "title": "FSM: Open Diagram",
    "icon": "$(graph)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.openSimulator",
    "title": "FSM: Open Simulator",
    "icon": "$(debug-alt)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.generateC99",
    "title": "FSM: Generate C99 Code",
    "icon": "$(export)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.generateCpp17",
    "title": "FSM: Generate C++17 Code",
    "icon": "$(export)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.formatDocument",
    "title": "FSM: Format Document",
    "icon": "$(symbol-misc)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.checkFile",
    "title": "FSM: Check File",
    "icon": "$(check-all)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.copyIR",
    "title": "FSM: Copy IR JSON to Clipboard",
    "icon": "$(clippy)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.restartLanguageServer",
    "title": "FSM: Restart Language Server",
    "icon": "$(refresh)",
    "category": "FSM Studio"
  },
  {
    "command": "fsm.showOutputChannel",
    "title": "FSM: Show Language Server Log",
    "category": "FSM Studio"
  }
]
```

---

# 5. Keybindings

```json
"keybindings": [
  {
    "command": "fsm.openDiagram",
    "key": "ctrl+shift+d",
    "mac": "cmd+shift+d",
    "when": "editorLangId == fsm-lang"
  },
  {
    "command": "fsm.openSimulator",
    "key": "ctrl+alt+s",
    "mac": "cmd+alt+s",
    "when": "editorLangId == fsm-lang"
  },
  {
    "command": "fsm.generateC99",
    "key": "ctrl+shift+g",
    "mac": "cmd+shift+g",
    "when": "editorLangId == fsm-lang"
  }
]
```

---

# 6. Menus

```json
"menus": {
  "editor/title": [
    {
      "command": "fsm.openDiagram",
      "when": "editorLangId == fsm-lang",
      "group": "navigation"
    },
    {
      "command": "fsm.openSimulator",
      "when": "editorLangId == fsm-lang",
      "group": "navigation"
    }
  ],
  "editor/context": [
    {
      "command": "fsm.openDiagram",
      "when": "editorLangId == fsm-lang",
      "group": "fsm@1"
    },
    {
      "command": "fsm.openSimulator",
      "when": "editorLangId == fsm-lang",
      "group": "fsm@2"
    },
    {
      "command": "fsm.generateC99",
      "when": "editorLangId == fsm-lang",
      "group": "fsm@3"
    },
    {
      "command": "fsm.generateCpp17",
      "when": "editorLangId == fsm-lang",
      "group": "fsm@4"
    }
  ],
  "explorer/context": [
    {
      "command": "fsm.checkFile",
      "when": "resourceExtname == .fsm",
      "group": "fsm"
    },
    {
      "command": "fsm.generateC99",
      "when": "resourceExtname == .fsm",
      "group": "fsm"
    }
  ],
  "commandPalette": [
    {
      "command": "fsm.openDiagram",
      "when": "editorLangId == fsm-lang"
    },
    {
      "command": "fsm.openSimulator",
      "when": "editorLangId == fsm-lang"
    },
    {
      "command": "fsm.generateC99",
      "when": "editorLangId == fsm-lang"
    },
    {
      "command": "fsm.generateCpp17",
      "when": "editorLangId == fsm-lang"
    },
    {
      "command": "fsm.formatDocument",
      "when": "editorLangId == fsm-lang"
    },
    {
      "command": "fsm.checkFile"
    },
    {
      "command": "fsm.copyIR",
      "when": "editorLangId == fsm-lang"
    },
    {
      "command": "fsm.restartLanguageServer"
    },
    {
      "command": "fsm.showOutputChannel"
    }
  ]
}
```

---

# 7. Views and View Containers

```json
"viewsContainers": {
  "activitybar": [
    {
      "id": "fsm-explorer",
      "title": "FSM Studio",
      "icon": "./assets/activitybar-icon.svg"
    }
  ]
},
"views": {
  "fsm-explorer": [
    {
      "id": "fsm.machineExplorer",
      "name": "Machines",
      "when": "fsm.hasOpenFsmFile"
    },
    {
      "id": "fsm.eventExplorer",
      "name": "Events",
      "when": "fsm.hasOpenFsmFile"
    }
  ]
}
```

---

# 8. Configuration Schema

```json
"configuration": {
  "title": "FSM Studio",
  "properties": {
    "fsmLang.compilerPath": {
      "type": "string",
      "default": "",
      "description": "Path to the fsm binary. Leave empty to use the bundled binary.",
      "scope": "machine-overridable"
    },
    "fsmLang.maxProblems": {
      "type": "integer",
      "default": 100,
      "minimum": 1,
      "maximum": 10000,
      "description": "Maximum number of diagnostics reported per file."
    },
    "fsmLang.enableInlayHints": {
      "type": "boolean",
      "default": true,
      "description": "Show inlay hints (priority values, timer durations, state counts)."
    },
    "fsmLang.inlayHints.showTransitionPriorities": {
      "type": "boolean",
      "default": true,
      "description": "Show priority values on non-default priority transitions."
    },
    "fsmLang.inlayHints.showStateTypes": {
      "type": "boolean",
      "default": false,
      "description": "Show state kind labels (composite, parallel) next to state names."
    },
    "fsmLang.inlayHints.showTimerDurations": {
      "type": "boolean",
      "default": true,
      "description": "Show human-readable duration next to timer ms values."
    },
    "fsmLang.diagnostics.enableStyleWarnings": {
      "type": "boolean",
      "default": true,
      "description": "Emit FSM-W02xx style warnings (e.g., loop in action block)."
    },
    "fsmLang.diagnostics.enableHints": {
      "type": "boolean",
      "default": true,
      "description": "Show FSM-H0xxx editor hints."
    },
    "fsmLang.diagnostics.actionComplexityThreshold": {
      "type": "integer",
      "default": 10,
      "minimum": 3,
      "maximum": 100,
      "description": "Number of statements in an action block before FSM-W0201 is emitted."
    },
    "fsmLang.codegen.defaultTarget": {
      "type": "string",
      "enum": ["c99", "cpp17"],
      "default": "c99",
      "description": "Default code generation target for FSM: Generate commands.",
      "enumDescriptions": [
        "Generate ANSI C99 code (embedded-safe, no STL).",
        "Generate C++17 code with CRTP or vtable implementation pattern."
      ]
    },
    "fsmLang.codegen.outputDir": {
      "type": "string",
      "default": "generated",
      "description": "Relative path from workspace root for generated code output."
    },
    "fsmLang.codegen.strategy": {
      "type": "string",
      "enum": ["switch", "table"],
      "default": "switch",
      "enumDescriptions": [
        "Switch-case dispatch (smaller ROM footprint, simpler code).",
        "Table-driven dispatch (larger ROM, cache-friendly, preferred for many transitions)."
      ]
    },
    "fsmLang.format.indentSize": {
      "type": "integer",
      "default": 4,
      "minimum": 1,
      "maximum": 8,
      "description": "Number of spaces per indentation level."
    },
    "fsmLang.format.bracketStyle": {
      "type": "string",
      "enum": ["same-line", "next-line"],
      "default": "same-line",
      "description": "Position of opening braces."
    },
    "fsmLang.simulator.port": {
      "type": "integer",
      "default": 7842,
      "minimum": 1024,
      "maximum": 65535,
      "description": "Port for the FSM simulator WebSocket daemon."
    },
    "fsmLang.debounceMs": {
      "type": "integer",
      "default": 200,
      "minimum": 50,
      "maximum": 2000,
      "description": "Debounce delay in milliseconds before re-analysis on text change."
    }
  }
}
```

---

# 9. Snippets — `snippets/fsm-lang.json`

```json
{
  "Machine": {
    "prefix": "machine",
    "body": [
      "machine ${1:Name} {",
      "    context {",
      "        ${2:field}: ${3:u32} = ${4:0};",
      "    }",
      "",
      "    events { ${5:EVENT} }",
      "",
      "    initial ${6:Idle}",
      "",
      "    state ${6:Idle} {",
      "        on ${5:EVENT} -> ${7:Next}",
      "    }",
      "",
      "    state ${7:Next} {",
      "        $0",
      "    }",
      "}"
    ],
    "description": "New FSM machine declaration"
  },
  "State": {
    "prefix": "state",
    "body": [
      "state ${1:Name} {",
      "    entry: { ${2:onEntry();}  }",
      "    exit:  { ${3:onExit();} }",
      "",
      "    on ${4:EVENT} -> ${5:Target};",
      "    $0",
      "}"
    ],
    "description": "New simple state"
  },
  "Composite State": {
    "prefix": "composite",
    "body": [
      "composite ${1:Name} {",
      "    initial ${2:Sub}",
      "",
      "    state ${2:Sub} {",
      "        $0",
      "    }",
      "}"
    ],
    "description": "New composite state"
  },
  "Parallel State": {
    "prefix": "parallel",
    "body": [
      "parallel ${1:Name} {",
      "    region ${2:First} {",
      "        initial ${3:Idle}",
      "        state ${3:Idle} { $0 }",
      "    }",
      "",
      "    region ${4:Second} {",
      "        initial ${5:Idle}",
      "        state ${5:Idle} { }",
      "    }",
      "}"
    ],
    "description": "New parallel state with two regions"
  },
  "Transition with guard": {
    "prefix": "on",
    "body": [
      "on ${1:EVENT} [${2:guard()}] -> ${3:Target};"
    ],
    "description": "Transition with guard"
  },
  "After timer": {
    "prefix": "after",
    "body": [
      "after ${1:1000}ms -> ${2:Timeout};"
    ],
    "description": "One-shot timer transition"
  },
  "Every timer": {
    "prefix": "every",
    "body": [
      "every ${1:100}ms: { ${2:poll();} }"
    ],
    "description": "Periodic timer with inline action"
  },
  "Guard with else": {
    "prefix": "choice",
    "body": [
      "choice ${1:Name} {",
      "    [${2:condition()}] -> ${3:Branch1};",
      "    [else]             -> ${4:Branch2};",
      "}"
    ],
    "description": "Choice pseudo-state with branches"
  },
  "Pure extern guard": {
    "prefix": "pure extern",
    "body": [
      "pure extern ${1:checkCondition}(${2:threshold: u32}) : bool"
    ],
    "description": "Pure extern guard declaration"
  },
  "Extern action": {
    "prefix": "extern",
    "body": [
      "extern ${1:doAction}(${2:param: u32})"
    ],
    "description": "Extern action declaration"
  }
}
```

---

# 10. Status Bar Items

The extension contributes two status bar items (rightmost area):

```json
"statusBarItems": [
  {
    "id": "fsm.serverStatus",
    "text": "$(check) FSM",
    "tooltip": "FSM Language Server: Running",
    "command": "fsm.showOutputChannel",
    "alignment": "right",
    "priority": 100
  },
  {
    "id": "fsm.currentMachine",
    "text": "$(graph) Motor",
    "tooltip": "Active machine: Motor — click to open diagram",
    "command": "fsm.openDiagram",
    "alignment": "right",
    "priority": 99
  }
]
```

Status bar server states:

| Icon | Text | Meaning |
|---|---|---|
| `$(sync~spin)` | `FSM` | Server starting |
| `$(check)` | `FSM` | Server running, no errors |
| `$(warning)` | `FSM` | Server running, warnings present |
| `$(error)` | `FSM` | Server running, errors present |
| `$(circle-slash)` | `FSM (stopped)` | Server stopped or crashed |

---

# 11. Extension Context Keys

```json
"when": "fsm.hasOpenFsmFile"       // true when any .fsm file is open
"when": "fsm.serverRunning"        // true when LSP server is connected
"when": "fsm.simulatorConnected"   // true when simulator WebSocket is open
"when": "editorLangId == fsm-lang" // standard VS Code condition for .fsm editor
```

---

# 12. Bundled Binary

The extension bundles the `fsm` binary for supported platforms:

```
editors/vscode/
└── bin/
    ├── linux-x64/fsm
    ├── linux-arm64/fsm
    ├── darwin-x64/fsm
    ├── darwin-arm64/fsm
    └── win32-x64/fsm.exe
```

Platform detection in extension activation:
```typescript
import * as os from 'os';
import * as path from 'path';

function getBundledBinaryPath(): string {
    const platform = os.platform();     // 'linux', 'darwin', 'win32'
    const arch     = os.arch();         // 'x64', 'arm64'
    const ext      = platform === 'win32' ? '.exe' : '';
    return path.join(__dirname, '..', 'bin',
        `${platform}-${arch}`, `fsm${ext}`);
}
```

If `fsmLang.compilerPath` is set, the user-specified path takes precedence.
If the bundled binary is not found for the current platform, the extension MUST show
an error notification: `"FSM Studio: No bundled binary for {platform}-{arch}. Please
install fsm manually and set fsmLang.compilerPath."`

---

# 13. Language Server Lifecycle and Crash Recovery

The extension manages the `fsm-lang-server` process and implements automatic crash
recovery with exponential backoff.

### 13.1 Normal Lifecycle

| Event | Extension behavior |
|---|---|
| Extension activates | Spawn `fsm-lang-server` with stdio transport; connect `vscode-languageclient` |
| Server ready | Update status bar to `$(check) FSM`; publish capabilities |
| File opened | Forward `textDocument/didOpen` to server |
| File changed | Forward `textDocument/didChange` to server (server debounces internally) |
| File closed | Forward `textDocument/didClose` to server |
| Extension deactivates | Send `shutdown` request; wait 2 seconds; send `exit` notification; kill process if still alive |

### 13.2 Crash Detection and Recovery

The extension monitors the server process via `vscode-languageclient`'s built-in crash
detection. When the server process exits unexpectedly (non-zero exit code or signal):

```typescript
const clientOptions: LanguageClientOptions = {
    errorHandler: {
        error: (error, message, count) => {
            return { action: ErrorAction.Continue };
        },
        closed: () => {
            if (crashCount < MAX_RESTARTS) {
                crashCount++;
                const delay = BASE_DELAY_MS * Math.pow(3, crashCount - 1);
                return {
                    action: CloseAction.Restart,
                    message: `FSM Language Server crashed. Restarting in ${delay / 1000}s... (attempt ${crashCount}/${MAX_RESTARTS})`
                };
            }
            return { action: CloseAction.DoNotRestart };
        }
    }
};
```

**Recovery parameters:**

| Parameter | Value |
|---|---|
| `MAX_RESTARTS` | 3 |
| `BASE_DELAY_MS` | 3000 (3 seconds) |
| Backoff multiplier | 3× (exponential) |
| Attempt 1 delay | 3 seconds |
| Attempt 2 delay | 9 seconds |
| Attempt 3 delay | 27 seconds |
| After 3 failures | Stop auto-restart; show manual restart button |

**Status bar during recovery:**

| State | Icon | Text | Tooltip |
|---|---|---|---|
| Restarting (attempt N) | `$(sync~spin)` | `FSM` | `FSM Language Server restarting (attempt N/3)...` |
| Failed (all attempts exhausted) | `$(circle-slash)` | `FSM (stopped)` | `FSM Language Server stopped. Click to restart.` |

**After all restart attempts fail:**

1. Status bar shows `$(circle-slash) FSM (stopped)`.
2. A notification is shown:
   ```
   "FSM Language Server has stopped after 3 restart attempts.
   [Restart Language Server] [Show Log] [Dismiss]"
   ```
3. Clicking "Restart Language Server" invokes `fsm.restartLanguageServer` command,
   resets the crash counter, and attempts a fresh start.
4. Clicking "Show Log" opens the Output Channel for `FSM Language Server`.

**Crash counter reset:**

The crash counter resets to 0 after the server has been running successfully for 60
seconds. This prevents a single transient crash from permanently consuming a restart
attempt.

```typescript
const RESET_DELAY_MS = 60_000;
let resetTimer: NodeJS.Timeout | undefined;

function onServerReady(): void {
    resetTimer = setTimeout(() => {
        crashCount = 0;
    }, RESET_DELAY_MS);
}

function onServerCrash(): void {
    if (resetTimer) {
        clearTimeout(resetTimer);
        resetTimer = undefined;
    }
}
```

---

*End of FSM-SPEC-VSCE v1.0.0*
