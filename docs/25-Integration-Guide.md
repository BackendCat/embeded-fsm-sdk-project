# FSM Studio — Integration Guide

**Document ID:** FSM-DEV-INTEGRATION
**Version:** 1.0.0
**Status:** Informative — practitioner guide. The normative contracts it
summarizes live in FSM-SPEC-GEN-C (11), FSM-SPEC-HAL (16) and
FSM-SPEC-CLI (18); where this doc and those disagree, those win.
**Depends on:** FSM-SPEC-GEN-C (11), FSM-SPEC-HAL (16), FSM-SPEC-CLI (18)

This guide is for engineers dropping a `.fsm` state machine into a real
codebase: what files `fsm generate` produces, the ABI you link against,
how to call the generated runtime from C / C++ / Rust, and ready-to-copy
build-system fragments for Make, CMake and PlatformIO.

Every fragment here is exercised by a self-contained, genuinely-building
example under [`examples/integration/`](../examples/integration/) — copy
one and you have a working build, not a snippet to debug. Each example's
acceptance test compiles the generated C with `gcc -std=c99 -Wall -Wextra
-Wpedantic -Werror` **and runs it**, asserting real transitions
(`crates/fsm-cli/tests/integration_examples.rs`).

---

## 1. The generated-C ABI contract

`fsm generate --target c99 <machine>.fsm --out <dir>` emits exactly five
files for a machine named `Motor` (the file basename follows the `machine`
name in the DSL, not the source filename):

| File | Edit? | Role |
|---|---|---|
| `Motor.h` | No | **Public API.** State/event id enums, payload structs, the event tagged union, the `Motor_Context_t`, the `Motor_t` instance struct, and the function prototypes you call. Wrapped in `extern "C"` for C++. |
| `Motor.c` | No | The one generated **translation unit**. Compile + link this. |
| `Motor_impl.h` | No | The **externs you must provide**: every DSL `extern`, every guard, and per-state `Motor_entry_*` / `Motor_exit_*` hooks. Prototypes only — you write the bodies. |
| `Motor_conf.h` | No | Compile-time configuration (queue capacity, overflow policy, strategy tag, timer type). Tunable indirectly via CLI flags / `fsm.toml`, not by hand-editing. |
| `fsm_hal.h` | No | The **HAL contract** (Doc 16). Declares `fsm_hal_clock_now_ms()` + `fsm_hal_assert()` that you must implement. |

"No" means *do not hand-edit* — the header carries `Do not edit by hand`
and is overwritten on every `generate`. Re-generate instead.

### 1.1 The public API surface (`Motor.h`)

```c
void            Motor_init          (Motor_t *m);
void            Motor_dispatch      (Motor_t *m, const Motor_Event_t *ev);
void            Motor_post          (Motor_t *m, const Motor_Event_t *ev);
bool            Motor_dequeue       (Motor_t *m, Motor_Event_t *out);
void            Motor_advance_clock (Motor_t *m, uint32_t elapsed_ms);
Motor_StateId_t Motor_current_state (const Motor_t *m);
```

- **You allocate `Motor_t`** — stack, BSS, or static. `Motor_init`
  zero-initializes it and enters the initial state. There is no malloc in
  the generated runtime.
- **Events** are an id + an optional payload union. Payload-less:
  `Motor_Event_t e = { .id = MOTOR_EVENT_START };`. With payload:
  `e.__payload.FAULT.code = 7;` after setting `e.id = MOTOR_EVENT_FAULT;`.
- **`Motor_dispatch`** runs one run-to-completion step synchronously.
  `Motor_post` enqueues for a later `Motor_dequeue`+dispatch loop
  (ISR-friendly with `MOTOR_QUEUE_ISR_SAFE`).
- **`Motor_advance_clock(m, elapsed_ms)`** drives `after N ms` timers; the
  caller owns the time base (see Doc 16 + §5 here).
- The `Motor_t` *fields* (`_active`, `_queue`, …) are an implementation
  detail, **not** the ABI. Read state via `Motor_current_state`; never
  hand-mirror the struct layout across an FFI boundary (see §3.2).

### 1.2 The externs you provide (`Motor_impl.h`)

For the canonical motor (`pure extern can_start() : bool`, `extern
set_speed(u16 rpm)`, `extern reset_link()`):

```c
bool can_start(void);              /* the [can_start] pure guard       */
void set_speed(uint16_t rpm);      /* START / STOP transition action   */
void reset_link(void);             /* FAULT transition action          */

void Motor_entry_IDLE(Motor_t *m); /* per-state hooks — codegen emits  */
void Motor_entry_RUNNING(Motor_t *m);
void Motor_entry_FAULTED(Motor_t *m);
void Motor_exit_IDLE(Motor_t *m);
void Motor_exit_RUNNING(Motor_t *m);
void Motor_exit_FAULTED(Motor_t *m);
```

Entry/exit prototypes are emitted for **every** state from the analyzer's
state list, even when the DSL declares no entry/exit action — provide
empty bodies for those (a missing symbol is a link error, by design: the
contract is explicit). `pure` externs (guards) **must not** mutate state.

---

## 2. C and C++ (`extern "C"`) integration

**C** is the native case: compile `Motor.c` + your `*_impl` definitions +
a HAL impl with a C99 compiler. Nothing special.

**C++**: `Motor.h` already guards its declarations with

```c
#ifdef __cplusplus
extern "C" {
#endif
/* ... */
#ifdef __cplusplus
}
#endif
```

so a `.cpp` translation unit includes it directly and links the C object
with no name-mangling mismatch. If you also provide the externs / HAL from
C++, wrap *those definitions* in `extern "C"` too so their symbols match
the C prototypes:

```cpp
extern "C" {
#include "Motor.h"
#include "Motor_impl.h"
}

extern "C" void set_speed(uint16_t rpm) { motor_driver.set_rpm(rpm); }
extern "C" bool can_start(void)         { return rail.ok(); }
```

The PlatformIO example's `src/main_arduino.cpp` is a real C++ TU driving
the generated C this way.

---

## 3. Rust integration (`cc` + FFI)

The pattern: a `build.rs` runs `fsm generate`, the
[`cc`](https://crates.io/crates/cc) crate compiles the generated C (plus
your HAL + extern impls) into a static lib, and `extern "C"` in Rust calls
the public API. Full worked crate:
[`examples/integration/cargo-rust/`](../examples/integration/cargo-rust/).

### 3.1 `build.rs`

```rust
use std::{env, path::PathBuf, process::Command};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let gen = out.join("gen");
    let fsm = env::var("FSM").unwrap_or_else(|_| "fsm".into());

    println!("cargo:rerun-if-changed=motor.fsm");
    println!("cargo:rerun-if-env-changed=FSM");
    std::fs::create_dir_all(&gen).unwrap();

    let st = Command::new(&fsm)
        .args(["generate", "--target", "c99"])
        .arg(manifest.join("motor.fsm"))
        .arg("--out").arg(&gen)
        .status().expect("spawn fsm");
    assert!(st.success(), "fsm generate failed");

    cc::Build::new()
        .file(gen.join("Motor.c"))
        .file(manifest.join("hal.c"))
        .file(manifest.join("motor_externs.c"))
        .include(&gen).include(&manifest)
        .flag("-std=c99").flag("-Wall").flag("-Wextra")
        .flag("-Wpedantic").flag("-Werror")
        .warnings(false)               // our explicit flags own warnings
        .compile("motor_fsm");
}
```

### 3.2 The FFI surface

Treat `Motor_t` as **opaque**. Mirroring the generated struct's private
fields in Rust is brittle — the layout is an implementation detail that
can change between versions; the *function ABI* is the stable contract.
The robust pattern is a thin C shim of stable pass-throughs
(`motor_new`/`motor_send`/`motor_state`/…) compiled alongside, called from
Rust:

```rust
#[repr(C)]
struct Motor { _opaque: [u8; 0] }

extern "C" {
    fn motor_new() -> *mut Motor;
    fn motor_send(m: *mut Motor, event_id: c_int);
    fn motor_state(m: *const Motor) -> c_int;
    fn motor_free(m: *mut Motor);
}
```

State/event integer ids come from the `Motor.h` enums and are stable:
`IDLE=2, RUNNING=3, FAULTED=4`; `START=0, STOP=1, FAULT=2, RESET=3`.

### 3.3 Workspace isolation

If the crate lives inside another Cargo workspace's directory tree, cargo
will absorb it unless it declares its own root. Put an empty `[workspace]`
table in its `Cargo.toml`:

```toml
[workspace]

[package]
name = "fsm-cargo-integration"
# ...
```

The example crate does exactly this so `cargo build` works standalone
regardless of where it is copied. (Note: a separate `.cargo/config.toml`
in a parent dir can still redirect the *target directory* — that is
config discovery, orthogonal to workspace membership.)

---

## 4. Build-system fragments

### 4.1 Make

```make
FSM    ?= fsm
CC     ?= cc
CFLAGS ?= -std=c99 -Wall -Wextra -Wpedantic -Werror -Igen -I.

gen/.stamp: motor.fsm
	$(FSM) generate --target c99 motor.fsm --out gen
	@touch $@

gen/Motor.c: gen/.stamp

motor_app: gen/Motor.c hal.c motor_externs.c main.c motor_externs.h
	$(CC) $(CFLAGS) gen/Motor.c hal.c motor_externs.c main.c -o motor_app

run: motor_app
	./motor_app
```

`FSM ?= fsm` means it works with an installed `fsm` or `make
FSM=/path/to/fsm run`. The `.stamp` file re-generates only when the
`.fsm` changes. Full project:
[`examples/integration/make/`](../examples/integration/make/).

### 4.2 CMake (≥ 3.13)

```cmake
cmake_minimum_required(VERSION 3.13)
project(fsm_cmake_integration C)
set(CMAKE_C_STANDARD 99)
set(CMAKE_C_EXTENSIONS OFF)            # -std=c99, not -std=gnu99
set(FSM "fsm" CACHE STRING "Path to the fsm binary")

set(GEN "${CMAKE_CURRENT_BINARY_DIR}/gen")
add_custom_command(
    OUTPUT  ${GEN}/Motor.c ${GEN}/Motor.h ${GEN}/Motor_impl.h
            ${GEN}/Motor_conf.h ${GEN}/fsm_hal.h
    COMMAND ${CMAKE_COMMAND} -E make_directory "${GEN}"
    COMMAND ${FSM} generate --target c99
            "${CMAKE_CURRENT_SOURCE_DIR}/motor.fsm" --out "${GEN}"
    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/motor.fsm"
    VERBATIM)

add_executable(motor_app ${GEN}/Motor.c hal.c motor_externs.c main.c)
target_include_directories(motor_app PRIVATE ${GEN} ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_options(motor_app PRIVATE -Wall -Wextra -Wpedantic -Werror)
```

The `add_custom_command(OUTPUT …)` form makes the generated `Motor.c` a
real build dependency, so CMake re-runs `fsm generate` exactly when the
`.fsm` changes. Override the binary with `-DFSM=/path/to/fsm`. Full
project: [`examples/integration/cmake/`](../examples/integration/cmake/).

### 4.3 PlatformIO

`fsm generate` runs from a pre-build hook:

```ini
[env]
extra_scripts = pre:scripts/fsm_generate.py
build_flags = -Wall -Wextra -Wpedantic -Werror

[env:native]
platform = native
build_src_filter = +<*> -<main_arduino.cpp>
build_flags = ${env.build_flags} -std=c99

[env:uno]
platform = atmelavr
board = uno
framework = arduino
build_src_filter = +<*> -<main.c>
build_flags = -Wall -Wextra -Werror -std=gnu99
```

```python
# scripts/fsm_generate.py
import os, subprocess
Import("env")
gen = os.path.join(env["PROJECT_DIR"], "src", "gen")
src = os.path.join(env["PROJECT_DIR"], "src", "motor.fsm")
os.makedirs(gen, exist_ok=True)
subprocess.check_call([os.environ.get("FSM", "fsm"),
                       "generate", "--target", "c99", src, "--out", gen])
env.Append(CPPPATH=[gen])
```

PlatformIO compiles `src/` recursively, so generating into `src/gen/` +
appending it to `CPPPATH` is enough. Full project (incl. the Arduino
sketch entry point):
[`examples/integration/platformio/`](../examples/integration/platformio/).

---

## 5. The HAL requirement (mandatory)

Every generated `Motor.c` unconditionally `#include "fsm_hal.h"`. The
firmware will **not link** until you provide:

```c
uint32_t fsm_hal_clock_now_ms(void);     /* monotonic ms; wrap OK       */
void     fsm_hal_assert(bool cond, const char *msg);
```

`fsm_hal_clock_now_ms` must be monotonically non-decreasing, callable from
any context, ≤1 ms precision for sub-100 ms timers; 32-bit wraparound
(~49.7 days) is handled by unsigned subtraction. On Arduino the whole HAL
is ~10 lines (`return millis();` + a trap loop) — see Doc 16 and the
PlatformIO example's `src/hal.c`.

`fsm_hal.h` also ships an optional POSIX reference impl gated by
`-DFSM_HAL_PROVIDE_POSIX_REFERENCE` (host test runs only); embedded
targets provide their own. The worked examples deliberately hand-write
`hal.c` instead — that is what a real port looks like.

> **POSIX gotcha (host builds).** `_POSIX_C_SOURCE` must be defined
> *before any system header is parsed* (it gates `clock_gettime` /
> `struct timespec`). Put `#define _POSIX_C_SOURCE 200809L` as the very
> first line of `hal.c`, ahead of every `#include` — including
> `<stdint.h>`. (This bites the conditional-HAL pattern: the macro inside
> an `#else` after the top-of-file includes is already too late.)

---

## 6. Integration knobs (CLI flags / `fsm.toml`)

| Knob | Effect |
|---|---|
| `--strategy switch\|table\|auto` | Dispatch codegen shape. `switch` = nested `switch` (small, debuggable); `table` = transition table (flat, predictable I-cache). `auto` (default) picks per machine. Per-machine override in `fsm.toml` `[machine.X] strategy = "switch"`. |
| `--license <SPDX>` | The `SPDX-License-Identifier` stamped into every emitted file (default `MIT`). Set to your project's license so generated TUs carry the right header in your tree. |
| `--import-header <path.h>` | Derive `extern`s from an existing C header instead of hand-writing them in the `.fsm` (v1.1 W5). Repeatable; also `fsm.toml` `[generate] import_headers`. A `.fsm` `extern` of the same name wins. Lets you point the FSM straight at your existing HAL/driver headers — see [`examples/import-header/`](../examples/import-header/). |
| `--queue-size <N>` | Event-queue capacity (power of two). Surfaces as `MOTOR_QUEUE_CAPACITY`. |
| `--report-memory` | Print the computed static memory budget after emission. |

Strategy and license are integration-relevant because they change the
*shape* and *header* of code that lands in your repo, not the behaviour —
the FSM semantics are identical across strategies (proven by the
conformance suite running both).

---

## 7. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `undefined reference to 'fsm_hal_clock_now_ms'` / `'fsm_hal_assert'` | HAL not provided. | Add a `hal.c` implementing both (§5), or compile one TU with `-DFSM_HAL_PROVIDE_POSIX_REFERENCE` for host runs. |
| `undefined reference to 'Motor_entry_RUNNING'` (etc.) | Entry/exit hook prototype emitted but no body. | Provide a body for **every** `Motor_entry_*`/`Motor_exit_*` in `Motor_impl.h`, empty `{ (void)m; }` if the DSL has no entry/exit action. |
| `undefined reference to 'can_start'` / `'set_speed'` | A DSL `extern` has no implementation. | Implement every prototype in `Motor_impl.h`; or import it via `--import-header` if it already exists in a C header. |
| `'struct timespec' has no member` / `implicit declaration of 'clock_gettime'` under `-Werror` | `_POSIX_C_SOURCE` defined too late. | Make it the first line of the file, before all `#include`s (§5 gotcha). |
| `stray '\342' in program` / comment-looking text compiled as code | A non-ASCII char (em-dash, `§`) in a C comment **after** the comment was accidentally closed (e.g. a `*/` sequence inside it). | Keep generated-adjacent C ASCII-only and don't put `*/` inside block comments. (The generated files are already ASCII-clean.) |
| Arduino build: `multiple definition of 'main'` | A bare `main()` TU compiled into an Arduino-framework env (the core owns `main`). | Use `setup()`/`loop()` for the firmware env and `build_src_filter` to exclude the bare-`main` TU there (and vice-versa). |
| `-Wpedantic` errors from Arduino core headers | Third-party headers, not your contract. | Keep `-Wall -Wextra -Werror` on the firmware; drop `-Wpedantic` for the Arduino env only. The generated C itself is `-Wpedantic`-clean (host gate proves it). |
| Rust: this crate got pulled into a parent workspace | No own workspace root. | Add an empty `[workspace]` table to its `Cargo.toml` (§3.3). |
| `fsm: command not found` from a build | `fsm` not on PATH. | Install it, or pass `FSM=/abs/path/to/fsm` (Make/CMake `-DFSM=`/PlatformIO `FSM` env all support this). |

---

## 8. Reference

- **Doc 11** — C99 codegen: exact file layout, struct layout, dispatch
  strategies (normative).
- **Doc 16** — HAL spec: the clock/assert contract, ISR safety, reference
  impls (normative; HAL is mandatory for v1.0).
- **Doc 18** — CLI spec: every flag, `fsm.toml`, exit codes (normative).
- **Doc 12** — C++17 codegen (the future first-class C++ target; today
  C++ consumes the C ABI via `extern "C"` as in §2).
- [`examples/integration/`](../examples/integration/) — the four worked,
  genuinely-building projects this guide documents.
