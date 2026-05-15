# FSM-Lang + GNU Make

A copy-this-and-go integration: a `.fsm` source, a `Makefile` that runs
`fsm generate` and compiles the result with a strict C99 toolchain, and a
hand-written HAL — exactly what an embedded project's build looks like.

## One command

```sh
make run
```

That single target:

1. `fsm generate --target c99 motor.fsm --out gen/` — emits the five
   generated-C files (`Motor.c`, `Motor.h`, `Motor_impl.h`,
   `Motor_conf.h`, `fsm_hal.h`). This is the ABI contract documented in
   [`docs/25-Integration-Guide.md`](../../../docs/25-Integration-Guide.md).
2. `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` compiles the generated
   `Motor.c` together with the hand-written `hal.c`, the extern
   implementations (`motor_externs.c`) and the `main.c` driver.
3. Links `./motor_app` and runs it. **Exit code 0 means the FSM was
   driven through its full lifecycle and every transition / context
   mutation / extern call was asserted correct** — not merely that the
   symbols linked.

Expected output:

```
OK Motor FSM: lifecycle verified (count=2 set_speed=2 reset_link=1 last_fault=7)
```

## `fsm` on PATH or an explicit path

`FSM` defaults to the `fsm` binary on your PATH. To point at a build
without installing:

```sh
make FSM=/abs/path/to/fsm run
```

## Files

| File | Role |
|---|---|
| `motor.fsm` | The state machine (3-state motor controller; timer + guard + actions). |
| `Makefile` | `fsm generate` → strict `gcc` → link → run. `FSM ?= fsm`. |
| `hal.c` | Hand-written HAL: `fsm_hal_clock_now_ms` + `fsm_hal_assert`. The single platform-integration point (docs/16). |
| `motor_externs.c` / `.h` | User implementations of the DSL `extern`s (`can_start`, `set_speed`, `reset_link`) + entry/exit hooks. Records side effects so the driver can assert behaviour. |
| `main.c` | Runnable driver: walks the lifecycle, asserts state + context + extern calls at every step. |
| `gen/` | Generated output (git-ignored; recreated by `make`). |

## Porting to real hardware

Replace `hal.c` with your platform's millisecond tick and fault trap (on
Arduino that is `return millis();` and a halt loop — see docs/16 §HAL),
and replace `motor_externs.c` with your real motor-driver calls. Nothing
else changes: the generated runtime is platform-agnostic.

## Clean

```sh
make clean
```
