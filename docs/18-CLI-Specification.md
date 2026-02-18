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

| Code | Meaning |
|---|---|
| `0` | Success — no errors |
| `1` | User error — parse/semantic/validation errors present in input |
| `2` | Tool error — internal compiler bug, I/O error, invalid flags |
| `3` | Not found — input file does not exist |
| `4` | Configuration error — invalid `fsm.toml` or conflicting flags |

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
| `--target <TARGET>` | `-t` | `c99` | `c99` \| `cpp17` — code generation target |
| `--out <DIR>` | `-o` | `./generated/` | Output directory |
| `--strategy <S>` | | `switch` | `switch` \| `table` — codegen strategy |
| `--queue-size <N>` | | `8` | Event queue capacity (must be power of 2) |
| `--queue-overflow <P>` | | `assert` | `assert` \| `drop-oldest` \| `drop-newest` |
| `--isr-safe` | | off | Enable ISR-safe queue |
| `--no-stl` | | off | Disable STL (C++ target only) |
| `--impl-style <S>` | | `crtp` | `crtp` \| `vtable` (C++ target only) |
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

Start the simulator daemon. Clients connect via WebSocket (FSM-SPEC-SIM).

```bash
fsm simulate [OPTIONS] [FILE]...
```

**Options:**
| Flag | Default | Description |
|---|---|---|
| `--port <N>` | `7842` | WebSocket listen port |
| `--host <ADDR>` | `127.0.0.1` | Listen address (`0.0.0.0` for remote access) |
| `--virtual-clock` | off | Start in virtual clock mode |
| `--load <FILE>` | | Pre-load machine(s) from `.fsm` file |
| `--load-ir <FILE>` | | Pre-load machine(s) from IR JSON file |
| `--once` | off | Exit after first client disconnects |

**Examples:**
```bash
fsm simulate motor.fsm                     # Start daemon, pre-load Motor machine
fsm simulate --port 8080 --virtual-clock   # Custom port, virtual clock
```

The daemon prints its listen address to stderr:
```
info: FSM simulator listening on ws://127.0.0.1:7842
info: loaded machine 'Motor' (instanceId: default)
```

---

## `fsm lsp`

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

Run the conformance test suite (FSM-SPEC-TEST).

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

*End of FSM-SPEC-CLI v1.0.0*
