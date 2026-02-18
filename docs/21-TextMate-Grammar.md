# FSM Studio — TextMate Grammar & Syntax Highlighting Specification

**Document ID:** FSM-SPEC-TM
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL, FSM-SPEC-LSP

Specifies the TextMate grammar for `.fsm` files and the mapping of FSM-Lang constructs
to VS Code theme colors. The grammar file lives at:
`editors/vscode/syntaxes/fsm-lang.tmLanguage.json`

---

# 1. Language Registration

**VS Code `package.json` contribution:**
```json
"languages": [
  {
    "id": "fsm-lang",
    "aliases": ["FSM-Lang", "fsm"],
    "extensions": [".fsm"],
    "mimetypes": ["text/x-fsm-lang"],
    "configuration": "./language-configuration.json"
  }
],
"grammars": [
  {
    "language": "fsm-lang",
    "scopeName": "source.fsm",
    "path": "./syntaxes/fsm-lang.tmLanguage.json"
  }
]
```

---

# 2. Scope Naming Convention

All scopes use the prefix `source.fsm`. Standard scope suffixes follow TextMate
naming conventions to ensure compatibility with VS Code themes.

| FSM construct | TextMate scope |
|---|---|
| Machine keyword | `keyword.declaration.machine.fsm` |
| State keyword | `keyword.declaration.state.fsm` |
| Composite keyword | `keyword.declaration.composite.fsm` |
| Parallel keyword | `keyword.declaration.parallel.fsm` |
| Event keyword | `keyword.declaration.event.fsm` |
| Extern keyword | `keyword.declaration.extern.fsm` |
| Pure keyword | `keyword.modifier.pure.fsm` |
| Context keyword | `keyword.declaration.context.fsm` |
| Control flow keywords (`if`, `else`, `while`, `for`) | `keyword.control.fsm` |
| FSM-semantic keywords (`on`, `after`, `every`, `raise`, `send`, `defer`) | `keyword.operator.fsm` |
| Pseudo-state keywords (`initial`, `final`, `history`, `choice`, `junction`, `fork`, `join`) | `keyword.type.pseudo.fsm` |
| History type (`shallow`, `deep`) | `keyword.other.history.fsm` |
| `to` keyword (in `send E to M`) | `keyword.operator.to.fsm` |
| `as` keyword | `keyword.operator.as.fsm` |
| `priority` keyword | `keyword.other.priority.fsm` |
| `internal` keyword | `keyword.modifier.internal.fsm` |
| Submachine keyword | `keyword.declaration.submachine.fsm` |
| `region` keyword | `keyword.declaration.region.fsm` |
| `events` keyword | `keyword.declaration.events.fsm` |
| `feature` keyword | `keyword.declaration.feature.fsm` |
| `language` keyword | `keyword.declaration.language.fsm` |
| `entry` keyword | `keyword.control.entry.fsm` |
| `exit` keyword | `keyword.control.exit.fsm` |
| `ms` unit | `keyword.other.unit.fsm` |
| Machine name (declaration) | `entity.name.type.machine.fsm` |
| State name (declaration) | `entity.name.type.state.fsm` |
| Event name (declaration) | `entity.name.type.event.fsm` |
| Extern name (declaration) | `entity.name.function.extern.fsm` |
| Context field name | `variable.other.field.fsm` |
| State name (reference in transition) | `entity.name.type.state.fsm` |
| Event name (reference) | `support.type.event.fsm` |
| Extern call (reference) | `support.function.extern.fsm` |
| `ctx` keyword | `variable.language.ctx.fsm` |
| `payload` keyword | `variable.language.payload.fsm` |
| `.` field access | `punctuation.accessor.fsm` |
| `->` arrow | `keyword.operator.arrow.fsm` |
| `~>` local arrow | `keyword.operator.local-arrow.fsm` |
| `[` `]` guard delimiters | `punctuation.definition.guard.fsm` |
| Guard expression | `meta.guard.fsm` |
| `@id(...)` annotation | `meta.annotation.fsm` |
| `@id` keyword | `storage.modifier.annotation.fsm` |
| Stable ID string | `string.quoted.double.annotation.fsm` |
| Float literal | `constant.numeric.float.fsm` |
| Integer literal | `constant.numeric.integer.fsm` |
| Boolean literal (`true`, `false`) | `constant.language.boolean.fsm` |
| String literal | `string.quoted.double.fsm` |
| String escape sequences | `constant.character.escape.fsm` |
| Enum qualified name (`Type.Member`) | `variable.other.enummember.fsm` |
| `=` assignment operator | `keyword.operator.assignment.fsm` |
| Arithmetic operators (`+`, `-`, `*`, `/`, `%`) | `keyword.operator.arithmetic.fsm` |
| Bitwise operators (`&`, `\|`, `^`, `~`, `<<`, `>>`) | `keyword.operator.bitwise.fsm` |
| Logical operators (`&&`, `\|\|`, `!`) | `keyword.operator.logical.fsm` |
| Comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) | `keyword.operator.comparison.fsm` |
| Line comment `//` | `comment.line.double-slash.fsm` |
| Block comment `/* */` | `comment.block.fsm` |
| Doc comment `///` | `comment.line.documentation.fsm` |
| `{` `}` braces | `punctuation.definition.block.fsm` |
| `(` `)` parens | `punctuation.definition.parameters.fsm` |
| `;` semicolon | `punctuation.terminator.fsm` |
| `,` comma | `punctuation.separator.fsm` |
| `:` colon (action separator) | `punctuation.separator.colon.fsm` |
| `else` guard keyword | `keyword.control.else.guard.fsm` |

---

# 3. Grammar File — `fsm-lang.tmLanguage.json`

```json
{
  "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  "name": "FSM-Lang",
  "scopeName": "source.fsm",
  "fileTypes": ["fsm"],
  "patterns": [
    { "include": "#doc-comment" },
    { "include": "#block-comment" },
    { "include": "#line-comment" },
    { "include": "#annotation" },
    { "include": "#machine-declaration" },
    { "include": "#state-declaration" },
    { "include": "#event-declaration" },
    { "include": "#extern-declaration" },
    { "include": "#context-block" },
    { "include": "#transition" },
    { "include": "#timer" },
    { "include": "#action-block" },
    { "include": "#pseudo-states" },
    { "include": "#keywords" },
    { "include": "#literals" },
    { "include": "#operators" },
    { "include": "#punctuation" }
  ],
  "repository": {

    "doc-comment": {
      "name": "comment.line.documentation.fsm",
      "match": "///.*$"
    },

    "line-comment": {
      "name": "comment.line.double-slash.fsm",
      "match": "//(?!/).*$"
    },

    "block-comment": {
      "name": "comment.block.fsm",
      "begin": "/\\*",
      "end": "\\*/",
      "beginCaptures": { "0": { "name": "punctuation.definition.comment.begin.fsm" } },
      "endCaptures":   { "0": { "name": "punctuation.definition.comment.end.fsm" } }
    },

    "annotation": {
      "name": "meta.annotation.fsm",
      "begin": "@id\\s*\\(",
      "end": "\\)",
      "beginCaptures": { "0": { "name": "storage.modifier.annotation.fsm" } },
      "endCaptures":   { "0": { "name": "punctuation.definition.annotation.end.fsm" } },
      "patterns": [
        { "include": "#string" }
      ]
    },

    "machine-declaration": {
      "name": "meta.declaration.machine.fsm",
      "match": "\\b(machine)\\s+([A-Za-z_][A-Za-z0-9_]*)\\b",
      "captures": {
        "1": { "name": "keyword.declaration.machine.fsm" },
        "2": { "name": "entity.name.type.machine.fsm" }
      }
    },

    "state-declaration": {
      "name": "meta.declaration.state.fsm",
      "match": "\\b(state|composite|parallel)\\s+([A-Za-z_][A-Za-z0-9_]*)\\b",
      "captures": {
        "1": { "name": "keyword.declaration.state.fsm" },
        "2": { "name": "entity.name.type.state.fsm" }
      }
    },

    "event-declaration": {
      "name": "meta.declaration.event.fsm",
      "match": "\\b(event)\\s+([A-Z_][A-Z0-9_]*)\\b",
      "captures": {
        "1": { "name": "keyword.declaration.event.fsm" },
        "2": { "name": "entity.name.type.event.fsm" }
      }
    },

    "extern-declaration": {
      "name": "meta.declaration.extern.fsm",
      "match": "\\b(extern)\\s+(pure\\s+)?([A-Za-z_][A-Za-z0-9_]*)\\b",
      "captures": {
        "1": { "name": "keyword.declaration.extern.fsm" },
        "2": { "name": "keyword.modifier.pure.fsm" },
        "3": { "name": "entity.name.function.extern.fsm" }
      }
    },

    "context-block": {
      "name": "meta.block.context.fsm",
      "begin": "\\b(context)\\s*\\{",
      "end": "\\}",
      "beginCaptures": { "1": { "name": "keyword.declaration.context.fsm" } },
      "patterns": [
        { "include": "#doc-comment" },
        { "include": "#line-comment" },
        { "include": "#context-field" },
        { "include": "#type-primitive" }
      ]
    },

    "context-field": {
      "match": "\\b([A-Za-z_][A-Za-z0-9_]*)\\s*:",
      "captures": {
        "1": { "name": "variable.other.field.fsm" }
      }
    },

    "transition": {
      "name": "meta.transition.fsm",
      "begin": "\\b(on)\\s+([A-Z_][A-Z0-9_]*)\\b",
      "end": ";",
      "beginCaptures": {
        "1": { "name": "keyword.operator.fsm" },
        "2": { "name": "support.type.event.fsm" }
      },
      "endCaptures": { "0": { "name": "punctuation.terminator.fsm" } },
      "patterns": [
        { "include": "#guard-expression" },
        { "include": "#arrow" },
        { "include": "#state-reference" },
        { "include": "#action-block" },
        { "include": "#priority-clause" }
      ]
    },

    "timer": {
      "match": "\\b(after|every)\\s+([0-9]+)(ms)\\b",
      "captures": {
        "1": { "name": "keyword.operator.fsm" },
        "2": { "name": "constant.numeric.integer.fsm" },
        "3": { "name": "keyword.other.unit.fsm" }
      }
    },

    "guard-expression": {
      "name": "meta.guard.fsm",
      "begin": "\\[",
      "end": "\\]",
      "beginCaptures": { "0": { "name": "punctuation.definition.guard.begin.fsm" } },
      "endCaptures":   { "0": { "name": "punctuation.definition.guard.end.fsm" } },
      "patterns": [
        { "include": "#field-access" },
        { "include": "#extern-call" },
        { "include": "#literals" },
        { "include": "#operators" },
        { "include": "#keywords-control" },
        { "name": "keyword.control.else.guard.fsm", "match": "\\belse\\b" }
      ]
    },

    "action-block": {
      "name": "meta.block.action.fsm",
      "begin": ":\\s*\\{|\\{",
      "end": "\\}",
      "patterns": [
        { "include": "#doc-comment" },
        { "include": "#line-comment" },
        { "include": "#field-access" },
        { "include": "#extern-call" },
        { "include": "#fsm-operations" },
        { "include": "#keywords-control" },
        { "include": "#literals" },
        { "include": "#operators" },
        { "include": "#action-block" }
      ]
    },

    "field-access": {
      "match": "\\b(ctx|payload)(\\.([A-Za-z_][A-Za-z0-9_]*))",
      "captures": {
        "1": { "name": "variable.language.ctx.fsm" },
        "2": { "name": "punctuation.accessor.fsm" },
        "3": { "name": "variable.other.field.fsm" }
      }
    },

    "extern-call": {
      "match": "\\b([A-Za-z_][A-Za-z0-9_]*)(?=\\s*\\()",
      "captures": {
        "1": { "name": "support.function.extern.fsm" }
      }
    },

    "fsm-operations": {
      "patterns": [
        {
          "match": "\\b(raise)\\s+([A-Z_][A-Z0-9_]*)\\b",
          "captures": {
            "1": { "name": "keyword.operator.fsm" },
            "2": { "name": "support.type.event.fsm" }
          }
        },
        {
          "match": "\\b(send)\\s+([A-Z_][A-Z0-9_]*)\\s+(to)\\s+([A-Za-z_][A-Za-z0-9_]*)\\b",
          "captures": {
            "1": { "name": "keyword.operator.fsm" },
            "2": { "name": "support.type.event.fsm" },
            "3": { "name": "keyword.operator.to.fsm" },
            "4": { "name": "entity.name.type.machine.fsm" }
          }
        },
        {
          "match": "\\b(defer)\\s+([A-Z_][A-Z0-9_]*)\\b",
          "captures": {
            "1": { "name": "keyword.operator.fsm" },
            "2": { "name": "support.type.event.fsm" }
          }
        }
      ]
    },

    "arrow": {
      "patterns": [
        { "name": "keyword.operator.local-arrow.fsm", "match": "~>" },
        { "name": "keyword.operator.arrow.fsm",         "match": "->" }
      ]
    },

    "state-reference": {
      "match": "(?<=-\\>|~\\>)\\s*([A-Za-z_][A-Za-z0-9_.]*)",
      "captures": {
        "1": { "name": "entity.name.type.state.fsm" }
      }
    },

    "priority-clause": {
      "match": "\\b(priority)\\s+([0-9]+)\\b",
      "captures": {
        "1": { "name": "keyword.other.priority.fsm" },
        "2": { "name": "constant.numeric.integer.fsm" }
      }
    },

    "pseudo-states": {
      "patterns": [
        { "name": "keyword.type.pseudo.fsm",
          "match": "\\b(initial|final|history|choice|junction|fork|join|entry_point|exit_point|region)\\b" },
        { "name": "keyword.other.history.fsm",
          "match": "\\b(shallow|deep)\\b" }
      ]
    },

    "keywords": {
      "patterns": [
        { "include": "#keywords-declaration" },
        { "include": "#keywords-control" },
        { "include": "#keywords-operator" }
      ]
    },

    "keywords-declaration": {
      "name": "keyword.declaration.fsm",
      "match": "\\b(machine|state|composite|parallel|events|event|extern|context|pure|submachine|region|feature|language)\\b"
    },

    "keywords-control": {
      "name": "keyword.control.fsm",
      "match": "\\b(if|else|while|for|entry|exit|internal)\\b"
    },

    "keywords-operator": {
      "name": "keyword.operator.fsm",
      "match": "\\b(on|after|every|raise|send|defer|to|as|priority|ms)\\b"
    },

    "type-primitive": {
      "name": "support.type.primitive.fsm",
      "match": "\\b(bool|u8|u16|u32|u64|i8|i16|i32|i64|f32|f64|opaque)\\b"
    },

    "string": {
      "name": "string.quoted.double.fsm",
      "begin": "\"",
      "end": "\"",
      "beginCaptures": { "0": { "name": "punctuation.definition.string.begin.fsm" } },
      "endCaptures":   { "0": { "name": "punctuation.definition.string.end.fsm" } },
      "patterns": [
        {
          "name": "constant.character.escape.fsm",
          "match": "\\\\(n|r|t|\\\\\"|\\\\\\\\)"
        }
      ]
    },

    "literals": {
      "patterns": [
        { "include": "#string" },
        { "name": "constant.numeric.float.fsm",    "match": "\\b\\d+\\.\\d+\\b" },
        { "name": "constant.numeric.integer.fsm",  "match": "\\b[0-9]+\\b" },
        { "name": "constant.language.boolean.fsm", "match": "\\b(true|false)\\b" },
        {
          "name": "variable.other.enummember.fsm",
          "match": "\\b([A-Z][A-Za-z0-9_]*)\\.([A-Z_][A-Z0-9_]*)\\b"
        }
      ]
    },

    "operators": {
      "patterns": [
        { "name": "keyword.operator.assignment.fsm",  "match": "=" },
        { "name": "keyword.operator.comparison.fsm",  "match": "==|!=|<=|>=|<|>" },
        { "name": "keyword.operator.logical.fsm",     "match": "&&|\\|\\||!" },
        { "name": "keyword.operator.bitwise.fsm",     "match": "&|\\||\\^|~|<<|>>" },
        { "name": "keyword.operator.arithmetic.fsm",  "match": "\\+|-|\\*|/|%" }
      ]
    },

    "punctuation": {
      "patterns": [
        { "name": "punctuation.definition.block.begin.fsm", "match": "\\{" },
        { "name": "punctuation.definition.block.end.fsm",   "match": "\\}" },
        { "name": "punctuation.definition.parameters.begin.fsm", "match": "\\(" },
        { "name": "punctuation.definition.parameters.end.fsm",   "match": "\\)" },
        { "name": "punctuation.terminator.fsm",  "match": ";" },
        { "name": "punctuation.separator.fsm",   "match": "," },
        { "name": "punctuation.accessor.fsm",    "match": "\\." },
        { "name": "punctuation.separator.colon.fsm", "match": ":" }
      ]
    }

  }
}
```

---

# 4. Language Configuration — `language-configuration.json`

```json
{
  "comments": {
    "lineComment": "//",
    "blockComment": ["/*", "*/"]
  },
  "brackets": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"]
  ],
  "autoClosingPairs": [
    { "open": "{", "close": "}", "notIn": ["string", "comment"] },
    { "open": "[", "close": "]", "notIn": ["string", "comment"] },
    { "open": "(", "close": ")", "notIn": ["string", "comment"] },
    { "open": "\"", "close": "\"", "notIn": ["string", "comment"] },
    { "open": "/*", "close": "*/", "notIn": ["string", "comment"] }
  ],
  "surroundingPairs": [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
    ["\"", "\""]
  ],
  "indentationRules": {
    "increaseIndentPattern": "\\{[^}]*$",
    "decreaseIndentPattern": "^\\s*\\}"
  },
  "wordPattern": "[A-Za-z_][A-Za-z0-9_]*",
  "folding": {
    "markers": {
      "start": "^\\s*//\\s*#region",
      "end":   "^\\s*//\\s*#endregion"
    }
  }
}
```

---

# 5. Theme Color Recommendations

The following recommendations guide theme authors and help ensure good readability
in both dark and light themes.

| Scope | Recommended role | Dark theme | Light theme |
|---|---|---|---|
| `keyword.declaration.*` | Declaration keywords | Blue (`#569cd6`) | Dark blue |
| `keyword.operator.fsm` | FSM semantic ops (`on`, `raise`, `send`) | Purple (`#c586c0`) | Dark purple |
| `keyword.control.fsm` | Control flow (`if`, `while`) | Magenta (`#c586c0`) | Dark magenta |
| `entity.name.type.machine.fsm` | Machine name | Light yellow (`#dcdcaa`) | Brown |
| `entity.name.type.state.fsm` | State name | Cyan (`#4ec9b0`) | Teal |
| `entity.name.type.event.fsm` | Event name (declaration) | Orange (`#ce9178`) | Dark orange |
| `support.type.event.fsm` | Event name (reference) | Orange italic | |
| `support.function.extern.fsm` | Extern function call | Yellow-green (`#dcdcaa`) | Olive |
| `variable.language.ctx.fsm` | `ctx` keyword | Light blue (`#9cdcfe`) | Blue |
| `variable.other.field.fsm` | Context/payload field | Light blue (`#9cdcfe`) | Blue |
| `keyword.operator.arrow.fsm` | `->` arrow | White (`#d4d4d4`) | Dark gray |
| `keyword.operator.local-arrow.fsm` | `~>` local arrow | Cyan | Teal |
| `meta.guard.fsm` | Guard expression background | Subtle highlight | |
| `constant.numeric.float.fsm` | Float numbers | Light green (`#b5cea8`) | Dark green |
| `constant.numeric.integer.fsm` | Integer numbers | Light green (`#b5cea8`) | Dark green |
| `constant.language.boolean.fsm` | true/false | Blue | Dark blue |
| `string.quoted.double.fsm` | String | Orange-brown (`#ce9178`) | Red-brown |
| `comment.*` | Comments | Green (`#6a9955`) | Dark green |
| `comment.line.documentation.fsm` | Doc comments | Green italic | |
| `storage.modifier.annotation.fsm` | `@id` | Yellow | Brown |
| `support.type.primitive.fsm` | Primitive types | Blue | Dark blue |

---

# 6. Semantic Token Override

When the LSP server is running, semantic tokens (textDocument/semanticTokens) override
TextMate scopes for higher precision. See FSM-SPEC-LSP §10 for the semantic token
legend. Semantic tokens take precedence when available.

VS Code merges TextMate scopes with semantic tokens: TextMate handles tokens the
LSP hasn't analyzed yet (during startup, in large files), semantic tokens provide
refinement for analyzed regions.

---

*End of FSM-SPEC-TM v1.0.0*
