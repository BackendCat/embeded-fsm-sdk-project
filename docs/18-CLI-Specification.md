# FSM Studio — CLI Specification

**Document ID:** FSM-SPEC-CLI
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL, FSM-SPEC-IR, FSM-SPEC-GEN-C, FSM-SPEC-GEN-CPP

Specifies the `fsm` command-line binary: all subcommands, flags, exit codes, stdout/stderr
conventions, and configuration file format.

---

# 1. Binary Name and Discovery

The binary is named `fsm`. It MUST be a single statically-linked (or self-contained)
executable with no mandatory runtime dependencies beyond the OS.

Installation locations (in PATH search order):
```
$HOME/.fsm/bin/fsm      (user install)
/usr/local/bin/fsm      (system install)
/opt/fsm/bin/fsm        (bundle install)
```

Version discovery:
```bash
fsm --version
# Output: fsm 0.1.0 (2025-02-18)
```

---

# 2. Global Flags

These flags apply to every subcommand:

| Flag | Short | Description |
|---|---|---|
| `--help` | `-h` | Print help for the current subcommand |
| `--version` | `-V` | Print version string and exit |
| `--no-color` | | Disable ANSI color codes in all output |
| `--json` | | Output diagnostics as JSON (machine-readable) |
| `--quiet` | `-q` | Suppress all output except errors |
| `--verbose` | `-v` | Increase log verbosity (stackable: `-vvv`) |
| `--config <path>` | | Override configuration file path |

---

# 3. Exit Codes

> _Updated 2026-05-14 in v1.0 doc reconciliation; see CHANGELOG._
>
> **This table is the single authoritative source for exit codes.** Other
> docs (Doc 03, Doc 23) cross-reference here and MUST NOT redefine.

| Code | Meaning |
|---|---|
| `0` | Success — no errors |
| `1` | User error — parse/semantic/validation errors present in input |
| `2` | Tool error — internal compiler bug, I/O error, invalid flags |
| `3` | Not found — input file does not exist |
| `4` | Configuration error — invalid `fsm.toml` or conflicting flags |

`fsm generate` panics in the codegen layer are converted to `EmitError`
(Doc 00 §11.9 / P1-8 wave) and surface as exit code 2; user-fixable input
problems remain at exit code 1.

---

# 4. Stdout / Stderr Convention

- **stdout** — machine-readable output: generated files content (when using `--stdout`),
  IR JSON, structured reports
- **stderr** — human-readable output: diagnostic messages, progress, warnings, info
- **`--json` flag** — all diagnostics go to **stdout** as a JSON array; progress goes
  to stderr; errors that would normally go to stderr appear in the JSON array instead

Diagnostic format (human-readable, default):
```
error[FSM-E0300]: nondeterministic transition conflict
  --> motor.fsm:42:5
   |
42 |     on START [ctx.speed > 5]  -> Medium;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
43 |     on START [ctx.speed > 10] -> Fast;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ also matches
   |
   = note: both transitions may be enabled simultaneously
   = help: add `priority` clauses or make guards mutually exclusive
```

Diagnostic format (`--json` flag):
```json
[
  {
    "code": "FSM-E0300",
    "severity": "error",
    "message": "nondeterministic transition conflict",
    "file": "motor.fsm",
    "line": 42,
    "col": 5,
    "endLine": 42,
    "endCol": 37,
    "relatedLocs": [
      { "file": "motor.fsm", "line": 43, "col": 5, "endLine": 43, "endCol": 36,
        "message": "also matches" }
    ],
    "fixable": false
  }
]
```

---

# 5. Subcommands

---

## `fsm parse`

Parse one or more `.fsm` files and report diagnostics. Does not perform semantic analysis.

```bash
fsm parse [OPTIONS] <FILE>...
```

**Options:**
| Flag | Description |
|---|---|
| `--emit-cst` | Print the Concrete Syntax Tree to stdout as JSON |
| `--emit-ast` | Print the Abstract Syntax Tree to stdout as JSON |

**Examples:**
```bash
fsm parse motor.fsm                 # Check syntax only
fsm parse src/**/*.fsm              # Check all .fsm files
fsm parse motor.fsm --emit-cst      # Dump CST (for debugging)
```

---

## `fsm check`

Full compilation check: parse + semantic analysis + nondeterminism detection.
Does NOT emit any output files. Use this in CI to verify correctness.

```bash
fsm check [OPTIONS] <FILE>...
```

**Options:**
| Flag | Description |
|---|---|
| `--max-errors <N>` | Stop after N errors (default: 100) |
| `--warn-as-error` | Treat all warnings as errors (exit code 1) |
| `--allow <CODE>` | Suppress specific diagnostic code globally (stackable) |
| `--deny <CODE>` | Treat specific diagnostic code as error (stackable) |

**Examples:**
```bash
fsm check motor.fsm
fsm check src/**/*.fsm --warn-as-error
fsm check motor.fsm --deny FSM-W0200   # loops-in-actions must be fixed
```

---

## `fsm generate`

Compile and generate code. Produces all output files for the selected target.

```bash
fsm generate [OPTIONS] <FILE>...
```

**Options:**
| Flag | Short | Default | Description |
|---|---|---|---|
| `--target <TARGET>` | `-t` | `c99` | `c99` (C++17 deferred to v1.1) |
| `--out <DIR>` | `-o` | `./generated/` | Output directory |
| `--strategy <S>` | | `auto` | `switch` \| `table` \| `auto` — dispatch strategy (Doc 11 §8). `auto` picks `switch` when state count < 64, else `table`. Per Doc 00 §11.14. |
| `--license <SPDX>` | | `MIT` | SPDX license identifier emitted in generated `.c` / `.h` header. Validated against the SPDX list. Per Doc 00 §10.4. |
| `--report-memory` | | off | Print computed per-machine memory budget (`sizeof(M_t)` breakdown) to stderr. Per Doc 00 §G-01. |
| `--queue-size <N>` | | `8` | Event queue capacity (must be power of 2) |
| `--queue-overflow <P>` | | `assert` | `assert` \| `drop_oldest` \| `drop_newest` |
| `--isr-safe` | | off | Enable ISR-safe queue |
| `--no-stl` | | off | Disable STL (C++ target only — DEFERRED v1.1) |
| `--impl-style <S>` | | `crtp` | `crtp` \| `vtable` (C++ target only — DEFERRED v1.1) |
| `--emit-offsets` | | off | Emit `M_asm_offsets.h` with struct byte offsets |
| `--stdout` | | off | Write generated source to stdout instead of files |
| `--emit-ir` | | off | Also write the IR JSON to `--out` directory |

**Examples:**
```bash
# C99, switch strategy, output to generated/
fsm generate motor.fsm

# C99, table strategy, custom output dir
fsm generate motor.fsm --target c99 --strategy table --out build/fsm/

# C++17, CRTP, no STL (Arduino)
fsm generate motor.fsm --target cpp17 --no-stl --out src/

# Generate and also emit IR
fsm generate motor.fsm --emit-ir --out build/
```

**Output files for `Motor` machine:**
```
generated/
├── Motor.h (or Motor.hpp)
├── Motor.c (or Motor.cpp)
├── Motor_impl.h (or Motor_impl.hpp)
└── Motor_conf.h (or Motor_conf.hpp)
```

---

## `fsm compile`

Alias for `fsm generate`. Kept for familiarity. Identical behavior.

---

## `fsm fmt`

Format `.fsm` source files according to the canonical style (FSM-SPEC-FMT).

```bash
fsm fmt [OPTIONS] [FILE]...
```

**Options:**
| Flag | Description |
|---|---|
| `--check` | Do not modify files; exit with code 1 if any file is unformatted |
| `--indent-size <N>` | Spaces per indent level (default: 4) |
| `--bracket-style <S>` | `same-line` \| `next-line` (default: `same-line`) |
| `--stdin` | Read from stdin, write formatted output to stdout |

**Examples:**
```bash
fsm fmt motor.fsm              # Format in place
fsm fmt src/**/*.fsm           # Format all
fsm fmt --check motor.fsm      # CI check — exit 1 if unformatted
fsm fmt --stdin < motor.fsm    # Pipe usage
```

**Exit codes for `fsm fmt --check`:**
- `0` — all files are correctly formatted
- `1` — one or more files need formatting
- `2` — I/O or parse error

---

## `fsm simulate`

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §B-02; see CHANGELOG._

Launch the in-process simulator and print active states / step traces to
stdout. **v1.0 does NOT start a WebSocket server.** The WebSocket transport
is deferred to v1.1; the network-facing flags below are reserved for that
revival and currently a no-op (passing `--port` produces a deprecation
notice on stderr).

```bash
fsm simulate [OPTIONS] [FILE]...
```

**Options (v1.0):**
| Flag | Default | Description |
|---|---|---|
| `--virtual-clock` | on | Use virtual clock (only mode in v1.0) |
| `--load <FILE>` | | Pre-load machine(s) from `.fsm` file |
| `--load-ir <FILE>` | | Pre-load machine(s) from IR JSON file |
| `--trace` | off | Emit `StepRecord` JSON per step (Doc 13 §11 schema) |

**Reserved for v1.1 (Doc 13 revival):**
| Flag | Default | Description |
|---|---|---|
| `--port <N>` | — | WebSocket listen port (v1.1) |
| `--host <ADDR>` | — | Listen address (v1.1) |
| `--once` | — | Exit after first client disconnects (v1.1) |

**Examples:**
```bash
fsm simulate motor.fsm                     # Load and print initial state
fsm simulate motor.fsm --trace             # Print StepRecord JSON per step
```

---

## `fsm lsp`

> _DEFERRED to v1.1 — no `fsm-lang-server` ships in v1.0 (Doc 00 §6 D-03)._

Start the Language Server in stdio mode. Called by VS Code extension automatically.

```bash
fsm lsp [OPTIONS]
```

**Options:**
| Flag | Default | Description |
|---|---|---|
| `--stdio` | (default) | stdio transport |
| `--port <N>` | | TCP transport, listen on port N |
| `--log-file <PATH>` | | Write log to file (useful for debugging) |

This subcommand is not intended to be called by users directly. The VS Code extension
invokes it via `vscode-languageclient` with stdio transport.

---

## `fsm doc`

Generate HTML or Markdown documentation from `/// doc comments` in `.fsm` files.

```bash
fsm doc [OPTIONS] <FILE>...
```

**Options:**
| Flag | Default | Description |
|---|---|---|
| `--format <F>` | `html` | `html` \| `markdown` |
| `--out <DIR>` | `./docs-out/` | Output directory |
| `--title <T>` | machine name | HTML page title |

---

## `fsm test`

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §11.15 / §11.16;
> see CHANGELOG._

Run the conformance test suite. Walks `MANIFEST.json` files under the test
suite root and executes each fixture.

```bash
fsm test [OPTIONS]
```

**Options:**
| Flag | Description |
|---|---|
| `--suite <DIR>` | Path to test suite root (required) |
| `--category <C>` | Filter: `parser` \| `validator` \| `semantic` \| `codegen-c` \| `formatter` |
| `--id <ID>` | Run single test by MANIFEST ID |
| `--tag <TAG>` | Filter by tag (stackable) |
| `--failing-only` | Print only failing tests |
| `--update-golden` | Regenerate golden files for formatter and codegen tests |
| `--format <F>` | `human` (default) \| `junit` \| `json` |
| `--verbose` | Show diff for every failing test |
| `--allow-empty-expected` | Opt-in: accept empty `expected` arrays in `.trace` files as a passing trace. Default is **hard fail** (Doc 00 §11.15 / §G6) so silently-empty fixtures cannot accidentally claim conformance. |

**Environment variables:**
| Variable | Description |
|---|---|
| `FSM_SKIP_GCC_TESTS` | If set to `1`, skip integration tests that invoke `gcc -Werror` (gates Doc 11 G4 in CI). Use only when gcc is unavailable; the default is to **fail** the run with a clear "missing gcc" diagnostic, not silently skip. Per Doc 00 §11.16. |

---

## `fsm ir`

Dump the compiled IR JSON for a `.fsm` file.

```bash
fsm ir [OPTIONS] <FILE>
```

**Options:**
| Flag | Default | Description |
|---|---|---|
| `--pretty` | on | Pretty-print JSON |
| `--compact` | off | Compact single-line JSON |
| `--machine <NAME>` | all | Only dump the named machine |

**Example:**
```bash
fsm ir motor.fsm --machine Motor | jq '.machines[0].root.states | length'
```

---

# 6. Configuration File — `fsm.toml`

The `fsm` binary searches for `fsm.toml` starting from the input file's directory
and walking up to the filesystem root (similar to `.gitignore` / `cargo.toml`).
`--config <path>` overrides the search.

```toml
# fsm.toml — project configuration

[compiler]
max_errors = 100
warn_as_error = false
allow = ["FSM-W0500"]     # Suppress globally
deny  = []

[generate]
target   = "c99"
strategy = "switch"
out      = "generated/"
queue_size     = 8
queue_overflow = "assert"
isr_safe       = false

[generate.cpp]
stl_profile = "no-stl"
impl_style  = "crtp"

[format]
indent_size    = 4
bracket_style  = "same-line"

[simulate]
port          = 7842
host          = "127.0.0.1"
virtual_clock = false

[lsp]
log_file = ""      # empty = no log file
debounce_ms = 200
```

**Search order for configuration:**
1. `--config <path>` flag (highest priority)
2. `$FSM_CONFIG` environment variable
3. `fsm.toml` in the directory of the input file(s)
4. `fsm.toml` in each parent directory up to root
5. `$HOME/.fsm/config.toml` (user-global defaults)
6. Built-in defaults (lowest priority)

### 6.1 Configuration Merge Semantics

When multiple configuration sources exist, they are merged from lowest to highest
priority (built-in defaults → user-global → project → env → CLI flag). Each key is
resolved independently using **last-writer-wins** semantics.

**Per-key merge behavior:**

| Key category | Merge strategy | Example |
|---|---|---|
| Scalar values (`target`, `strategy`, `indent_size`, `port`, etc.) | **Replace** — higher-priority value overwrites lower | CLI `--target cpp17` overwrites `fsm.toml` `target = "c99"` |
| Boolean flags (`warn_as_error`, `isr_safe`, `virtual_clock`) | **Replace** | `--warn-as-error` overrides `warn_as_error = false` in config |
| Array values (`allow`, `deny`) | **Append** — higher-priority arrays are concatenated after lower | User-global `allow = ["FSM-W0500"]` + project `allow = ["FSM-W0200"]` → `["FSM-W0500", "FSM-W0200"]` |
| `out` (output directory) | **Replace** with path resolution: relative paths resolve relative to the config file that declares them | Project `fsm.toml` with `out = "generated/"` resolves to `<project-root>/generated/` |

**Precedence rules:**

1. CLI flags ALWAYS override all config file values and environment variables.
2. Environment variables override all config file values.
3. The nearest `fsm.toml` (by directory walk) overrides more distant ones.
4. User-global `$HOME/.fsm/config.toml` provides defaults only — never overrides
   project-level config.
5. If no configuration source sets a key, the built-in default is used.

**Multiple `fsm.toml` files in the directory hierarchy:**

Only the **nearest** `fsm.toml` to the input file is loaded. Parent `fsm.toml` files
are NOT merged — the nearest one shadows all parents. This prevents unexpected
inheritance from distant ancestor directories.

```
project/
├── fsm.toml              ← project-level (used for files in project/)
├── src/
│   ├── fsm.toml          ← subdirectory-level (used for files in src/)
│   └── motor.fsm         ← uses src/fsm.toml, NOT project/fsm.toml
└── lib/
    └── sensor.fsm        ← uses project/fsm.toml (no lib/fsm.toml)
```

**Diagnostic for conflicting config:**

If a CLI flag contradicts a config file value, the CLI flag wins silently. If two
config keys are mutually exclusive (e.g., `--stdout` with `--out`), the tool MUST emit
exit code `4` with message: `"conflicting options: --stdout and --out cannot be used
together"`.

---

# 7. Environment Variables

| Variable | Description |
|---|---|
| `FSM_CONFIG` | Path to configuration file (overrides `fsm.toml` search) |
| `FSM_LOG` | Log filter string (e.g., `fsm_lsp=debug,fsm_analyzer=info`) |
| `FSM_NO_COLOR` | Disable color output if set to any value |
| `FSM_CACHE_DIR` | Override cache directory (default: `$HOME/.fsm/cache/`) |
| `FSM_TOOLCHAIN_DIR` | Override built-in toolchain path |

---

# 8. Shell Completions

```bash
fsm completions bash   > ~/.bash_completion.d/fsm
fsm completions zsh    > ~/.zsh/completions/_fsm
fsm completions fish   > ~/.config/fish/completions/fsm.fish
fsm completions pwsh   > ~/Documents/PowerShell/fsm.ps1
```

---

# 9. CI Integration Patterns

**GitHub Actions:**
```yaml
- name: Check FSM sources
  run: fsm check src/**/*.fsm --warn-as-error

- name: Generate C99 code
  run: fsm generate src/**/*.fsm --target c99 --out generated/

- name: Verify formatting
  run: fsm fmt --check src/**/*.fsm

- name: Run conformance tests
  run: fsm test --suite tests/ --format junit > test-results.xml
```

**Makefile:**
```makefile
FSM_SOURCES := $(wildcard src/*.fsm)
GENERATED   := $(FSM_SOURCES:src/%.fsm=generated/%.h)

check:
	fsm check $(FSM_SOURCES)

generated/%.h generated/%.c: src/%.fsm
	fsm generate $< --out generated/

fmt:
	fsm fmt $(FSM_SOURCES)

fmt-check:
	fsm fmt --check $(FSM_SOURCES)
```

---

# 10. Security

> _Added 2026-05-14 in v1.0 doc reconciliation per Doc 00 §G-02 / §11.12 /
> §11.13; see CHANGELOG._

The compiler runs in CI environments. The DSL has two surfaces an attacker
could exploit: `import "path"` (path traversal) and `opaque "C_type"` (code
injection). v1.0 hardens both at parse time.

## 10.1 Import Path Resolution

- Paths are resolved with `Path::canonicalize` (symlinks resolved before the
  check, not after).
- The resolved canonical path MUST start with the workspace root prefix.
  `..` segments that escape the workspace are rejected as `FSM-E0700`-class
  errors (canonicalization failure / out-of-workspace).
- Per Doc 00 §11.12 / commit `bccc46f`.

## 10.2 Opaque Type Validation

- Content matched against `^[A-Za-z_][A-Za-z0-9_ *]*$`. Semicolons,
  parentheses, brackets, and string literals are rejected at parse time.
- The validated string is the only thing emitted into generated C, so a
  rejected input cannot escape the regex into the compiled `.c`.

## 10.3 Parser DoS Limits

Default limits, applied at `parse()` entry:

| Limit | Value | Rationale |
|---|---|---|
| `max_input_bytes` | 1 MiB | Bounds peak parser RAM |
| `max_recursion_depth` | 256 | RAII guard, prevents stack overflow on deeply nested input |
| `max_token_count` | ~262k | Bounds the lexer's token buffer |

Per Doc 00 §11.13. Configurable via `fsmLang.parser.*` in `fsm.toml`.

## 10.4 Simulator WebSocket — Not in v1.0

The Doc 03 / Doc 13 simulator WebSocket is **not running** in v1.0. The
security gap from "server on `0.0.0.0` with no auth" is closed by deletion
(per Doc 00 §B-02). When v1.1 revives Doc 13, transport security is a
release-blocker for that revival.

---

*End of FSM-SPEC-CLI v1.0.0*
