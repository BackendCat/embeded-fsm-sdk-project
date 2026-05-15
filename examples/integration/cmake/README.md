# FSM-Lang + CMake

The same Motor FSM as the Make example, wired into a CMake project. Copy
this directory into a CMake codebase and the FSM builds as a normal target.

## Build & run

```sh
cmake -S . -B build
cmake --build build
./build/motor_app          # exit 0 == lifecycle verified
```

Or run via the convenience target:

```sh
cmake --build build --target run
```

Expected output:

```
OK Motor FSM: lifecycle verified (count=2 set_speed=2 reset_link=1 last_fault=7)
```

## `fsm` on PATH or an explicit path

`fsm` is taken from PATH by default. Override at configure time:

```sh
cmake -S . -B build -DFSM=/abs/path/to/fsm
```

## How it wires together

`CMakeLists.txt` uses the standard `add_custom_command(OUTPUT ...)`
pattern (CMake >= 3.13):

- A custom command runs `fsm generate --target c99 motor.fsm --out
  build/gen/` and declares the five generated files
  (`Motor.c`/`.h`/`Motor_impl.h`/`Motor_conf.h`/`fsm_hal.h` — the ABI
  contract, [`docs/25`](../../../docs/25-Integration-Guide.md)) as build
  outputs.
- `add_executable(motor_app ...)` compiles the generated `Motor.c` with
  the hand-written `hal.c`, the extern impls and the `main.c` driver
  under `-Wall -Wextra -Wpedantic -Werror`, then links a runnable binary.
- Because the executable depends on the generated `Motor.c`, CMake
  re-runs `fsm generate` exactly when `motor.fsm` changes — never
  otherwise.

The C sources are identical to the Make example (the generated-C contract
is build-system agnostic). See that README and `docs/25` for the porting
story (swap `hal.c` + `motor_externs.c` for your platform).

## Files

| File | Role |
|---|---|
| `motor.fsm` | The state machine. |
| `CMakeLists.txt` | `add_custom_command` → `fsm generate`; executable over generated C + HAL + driver. |
| `hal.c` | Hand-written HAL (`fsm_hal_clock_now_ms` + `fsm_hal_assert`). |
| `motor_externs.c` / `.h` | DSL `extern` implementations + entry/exit hooks + side-effect probe. |
| `main.c` | Runnable driver asserting the full lifecycle. |
| `build/` | CMake build tree, incl. `build/gen/` (git-ignored). |
