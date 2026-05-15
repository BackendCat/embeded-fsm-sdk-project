# FSM-Lang + Cargo (Rust)

Drive the FSM-Lang-generated C state machine from Rust over a tiny
`extern "C"` FFI. A `build.rs` runs `fsm generate`, the
[`cc`](https://crates.io/crates/cc) crate compiles the generated C, and
`src/main.rs` links it and exercises the FSM — asserting real
transitions, not just that the symbols resolved.

## Build & run

```sh
cargo run
```

Expected output:

```
OK Rust<->C FSM: lifecycle verified over FFI (set_speed_calls=2, reset_link_calls=1)
```

A non-zero exit means an `assert!` inside `src/main.rs` failed — i.e. the
FSM did **not** behave as the DSL semantics require (a real behavioural
failure, not a build error).

## `fsm` on PATH or an explicit path

`build.rs` takes `fsm` from PATH by default. Point it at a specific
binary with the `FSM` environment variable:

```sh
FSM=/abs/path/to/fsm cargo run
```

## How it wires together

| Stage | Where | What |
|---|---|---|
| 1. Codegen | `build.rs` | `fsm generate --target c99 motor.fsm --out $OUT_DIR/gen/` |
| 2. Compile C | `build.rs` (`cc` crate) | `Motor.c` + `hal.c` + `motor_externs.c` + `ffi_helpers.c` → `libmotor_fsm.a`, strict `-std=c99 -Wall -Wextra -Wpedantic -Werror` |
| 3. Link + drive | `src/main.rs` | `extern "C"` declarations → call the API, assert the lifecycle |

`ffi_helpers.c` is a thin pass-through shim over the generated public API.
Rust never mirrors the private `Motor_t` layout — that is an
implementation detail; wrapping the stable function ABI is the robust FFI
pattern (see [`docs/25`](../../../docs/25-Integration-Guide.md) §3).

## Standalone — not a workspace member

`Cargo.toml` declares an **empty `[workspace]` table**. Cargo otherwise
walks up the directory tree and would absorb this crate into the parent
FSM Studio workspace. The empty table makes it its own workspace root, so
`cargo build` works from this directory whether it lives inside this repo
or is copied anywhere else.

> Note: a `.cargo/config.toml` in a parent directory can still redirect
> the *target directory* (cargo config discovery is independent of
> workspace membership). Inside this repo's worktree the build artifacts
> route to the shared `CARGO_TARGET_DIR`; copied out standalone they go to
> a local `target/`. Behaviour is identical either way.

## Files

| File | Role |
|---|---|
| `Cargo.toml` | Standalone crate (`[workspace]` empty); `cc` build-dep. |
| `build.rs` | `fsm generate` + `cc` compile of the generated C + HAL + externs. |
| `src/main.rs` | `extern "C"` FFI driver; asserts the full lifecycle. |
| `ffi_helpers.c` | Stable pass-through shim over the generated public API. |
| `motor.fsm` | The state machine. |
| `hal.c` | Hand-written HAL (`fsm_hal_clock_now_ms` + `fsm_hal_assert`). |
| `motor_externs.c` / `.h` | DSL `extern` impls + entry/exit hooks + side-effect probe. |
| `Cargo.lock` | Committed — this is a binary crate, so the lockfile pins reproducible builds. |
