# FSM-Lang + PlatformIO

The Motor FSM as a PlatformIO project, targeting **two** environments off
one source tree to prove the generated C is genuinely embedded-portable:

- **`native`** — the host-simulation platform. Builds + runs a bare
  `main()` driver on the desktop.
- **`uno`** — a real Arduino Uno (ATmega328P, 8-bit AVR). Builds the
  firmware as an Arduino `setup()`/`loop()` sketch; flashing it blinks
  the on-board LED fast when the FSM lifecycle verified.

## Build & flash

```sh
pio run                     # build every env
pio run -e native           # host build
pio run -e native -t exec   # host build + run (prints the verified banner)
pio run -e uno              # cross-compile firmware for the Uno
pio run -e uno -t upload    # flash a connected Uno
```

On `-e native -t exec` the program prints:

```
OK Motor FSM: lifecycle verified (count=2 set_speed=2 reset_link=1 last_fault=7)
```

and on the Uno the on-board LED fast-blinks (~4 Hz) when every transition
/ context mutation / extern-call assertion held, or stays solid-on if an
assertion failed.

## How `fsm generate` is wired in

`platformio.ini` registers a pre-build hook:

```ini
extra_scripts = pre:scripts/fsm_generate.py
```

`scripts/fsm_generate.py` runs `fsm generate --target c99 src/motor.fsm
--out src/gen/` before every build and adds `src/gen/` to the include
path. The `fsm` binary is `fsm` on PATH unless the `FSM` environment
variable points at one (same contract as the Make/CMake examples).

The only platform-specific source is the single `#if defined(ARDUINO)`
split in `src/hal.c` (POSIX clock on the host, `millis()` on the Uno) —
the generated FSM runtime is byte-for-byte identical on both targets.
`src/main.c` (bare `main()`, host only) and `src/main_arduino.cpp`
(`setup()`/`loop()`, Uno only) are selected per-env via `build_src_filter`.
The C++ TU links the generated C directly because `Motor.h` wraps its API
in `extern "C"` (see [`docs/25`](../../../docs/25-Integration-Guide.md)
§2, the C→C++ recipe).

## If PlatformIO is not installed

PlatformIO is a Python tool (`pip install platformio`) and may not be on
every machine. The guarantee that actually matters is that the **same
generated C + driver compiles under a strict C99 toolchain** — PlatformIO
is just the build/flash wrapper around that portable core. The repo's
`crates/fsm-cli/tests/integration_examples.rs` therefore: runs `pio run`
when `pio` is on PATH; otherwise falls back to compiling this project's
generated C + `hal.c` + `motor_externs.c` + `main.c` with `gcc -std=c99
-Wall -Wextra -Wpedantic -Werror` and running it (a loud, documented
skip-with-reason — never a silent pass). Either way the embedded-portable
C is proven sound.

## Files

| File | Role |
|---|---|
| `platformio.ini` | Two envs (`native`, `uno`); pre-build hook; strict flags. |
| `scripts/fsm_generate.py` | Pre-build: `fsm generate` → `src/gen/`, add include path. |
| `src/motor.fsm` | The state machine. |
| `src/hal.c` | HAL: POSIX clock (host) / `millis()` (Arduino) behind one `#if`. |
| `src/motor_externs.c` / `.h` | DSL `extern` impls + entry/exit hooks + probe. |
| `src/main.c` | Bare-`main()` driver — **`native` env only**. |
| `src/main_arduino.cpp` | Arduino `setup()`/`loop()` sketch — **`uno` env only**. |
| `src/gen/` | Generated output (git-ignored; recreated each build). |
