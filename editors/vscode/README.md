# FSM Studio for VS Code

Language support, diagrams, **formal verification**, and C codegen for
**FSM-Lang** — a deterministic, embedded-first DSL for hierarchical finite
state machines.

This extension is the editor frontend for the FSM-Lang toolchain
(`fsm-lang-server` + the `fsm` CLI). The compiler, verifier, and code
generator are bundled with the extension — no separate install, no network
access required.

---

## Features

### Language support

- **Syntax highlighting** for `.fsm` files (TextMate grammar) and snippets
  (`machine`, `state`, `transition`, …).
- **Language server** (`fsm-lang-server`): diagnostics as you type, document
  symbols, hover, and configurable inlay hints (transition priorities, timer
  durations, state kinds).
- **Machines** and **Events** activity-bar explorers, kept in sync with the
  language server's live document symbols.

### Diagram

- A read-only, CSP-locked **diagram panel** (ELK Layered layout). Click a
  state or transition to jump to its source. Open with `Ctrl+Shift+D` (macOS
  `Cmd+Shift+D`) or the editor-title icon. The diagram keeps the last valid
  render and shows an honest banner when a source no longer compiles — it
  never blanks or fakes a stale view.

### Verification

The extension surfaces the toolchain's formal verifier — **deadlock-freedom**
and **reachability** analysis — in two equivalent ways. Both render the
*same* `fsm-verify` verdict; the editor result is byte-equal to the CLI.

- **`FSM Studio: Verify (Deadlock-Freedom + Reachability)`** (`fsm.verify`)
  — runs the bundled `fsm` CLI and renders its verdict, exploration stats,
  the deadlock **counterexample witness** as a navigable list, and
  reachability findings as editor diagnostics.
- **`FSM Studio: Verify Live (via Language Server)`** (`fsm.verifyLive`) —
  the same verification driven through the language server (debounced,
  large-FSM-guarded; opt-in, never auto-run on every keystroke).
- **`FSM Studio: Baseline Regression Check`** (`fsm.baseline`) — runs the
  CLI's trace-baseline regression check over a suite directory.

An **inconclusive** result (e.g. a bound was hit before the state space was
exhausted) is always shown as *inconclusive* — never as "verified". A
"verified" verdict means the verifier proved the property, not that the
analysis ran out of budget.

### Code generation

- **`FSM Studio: Generate C99 Code`** (`fsm.generateC99`, `Ctrl+Shift+G` /
  `Cmd+Shift+G`) and **`Generate C++17 Code`** (`fsm.generateCpp17`). The
  emitted C is deterministic and heap-free; every file carries an SPDX
  header. The command surfaces the compiler's own error verbatim when a
  target is unavailable — it never reports a fake success.
- **`Check File`** (`fsm.checkFile`), **`Copy IR JSON`** (`fsm.copyIR`),
  **`Format Document`** (`fsm.formatDocument`).

---

## Settings

| Setting | Default | Effect |
|---|---|---|
| `fsmLang.compilerPath` | *(bundled)* | Path to a custom `fsm-lang-server`. Empty uses the binary bundled for your platform. |
| `fsmLang.enableInlayHints` | `true` | Master toggle for inlay hints. |
| `fsmLang.inlayHints.showTransitionPriorities` | `true` | Priority values on non-default transitions. |
| `fsmLang.inlayHints.showStateTypes` | `false` | State-kind labels (composite, parallel). |
| `fsmLang.inlayHints.showTimerDurations` | `true` | Human-readable timer durations. |
| `fsmLang.codegen.outputDir` | `generated` | Output directory for the Generate commands (`fsm generate --out`). |
| `fsmLang.codegen.strategy` | `switch` | Dispatch strategy: `switch`, `table`, or `auto` (`fsm generate --strategy`). |
| `fsmLang.codegen.queueSize` | *(compiler default)* | Event-queue capacity; must be a power of two (`fsm generate --queue-size`). |
| `fsmLang.codegen.license` | `MIT` | SPDX identifier embedded in generated file headers (`fsm generate --license`). |
| `fsmLang.codegen.emitIr` | `false` | Also write IR JSON next to generated sources (`fsm generate --emit-ir`). |
| `fsmLang.codegen.reportMemory` | `false` | Log the computed memory budget after emission (`fsm generate --report-memory`). |

A per-project `fsm.toml` is also honored by the bundled CLI (e.g. per-machine
`[machine.<Name>] strategy`, `[generate] import_headers`); project config
takes precedence where it applies.

---

## The HAL contract (generated code)

Generated C unconditionally includes `fsm_hal.h`, which you supply once per
project:

```c
uint32_t fsm_hal_clock_now_ms(void);                 // wall-clock or virtual
void     fsm_hal_assert(const char *file, int line,  // assertion handler
                        const char *msg);
```

On Arduino, this is typically ≤ 10 lines:

```c
uint32_t fsm_hal_clock_now_ms(void) { return millis(); }
void fsm_hal_assert(const char *f, int l, const char *m) {
    (void)f; (void)l; (void)m; for (;;) {}
}
```

---

## Requirements

- VS Code `^1.85.0`.
- No external toolchain: the `fsm` CLI and `fsm-lang-server` for your host
  platform are bundled in the extension package.

---

## Versioning

This extension is versioned independently of the FSM-Lang Rust toolchain
crates. The extension's version (this `1.0.0` is its first Marketplace
release) tracks editor-surface changes; the bundled compiler/verifier
evolve on the toolchain's own release line. See the
[CHANGELOG](CHANGELOG.md).

## License

MIT. See the [project repository](https://github.com/BackendCat/embeded-fsm-sdk-project).
