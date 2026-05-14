# Motor Controller

A canonical 3-state motor controller demonstrating the core FSM-Lang surface
the v1.0 SDK ships:

- **Context fields** — `count` (run counter), `last_fault_code` (latched fault).
- **Events with payloads** — `FAULT(code : u8)` carries the diagnostic byte.
- **Pure guards** — `[can_start]` gates the start-up transition.
- **Actions** — assignment + extern call sequences (`set_speed(100)`).
- **One-shot timer** — `after 5000 ms -> Faulted` auto-faults a hung motor.

## States

| State    | Meaning                                          |
| -------- | ------------------------------------------------ |
| Idle     | Motor stopped. Awaiting START.                   |
| Running  | Motor spinning. 5 s watchdog primed for hangs.   |
| Faulted  | Hardware fault latched. Operator must `RESET`.   |

## Transitions

| From    | Event      | Guard         | Target   | Actions                                                  |
| ------- | ---------- | ------------- | -------- | -------------------------------------------------------- |
| Idle    | START      | `[can_start]` | Running  | `ctx.count = ctx.count + 1; set_speed(100)`              |
| Running | STOP       | —             | Idle     | `set_speed(0)`                                           |
| Running | FAULT(code)| —             | Faulted  | `ctx.last_fault_code = payload.code; reset_link()`       |
| Running | (5 s)      | —             | Faulted  | timer-driven watchdog                                    |
| Faulted | RESET      | —             | Idle     | —                                                        |

## Target use case

Embedded motor-control loop: an operator command FSM driving a separate
field-oriented-control task. The host implements the externs:

```c
bool Motor_can_start(const Motor_t *m);   // checks supply rail / brake state
void Motor_set_speed(uint16_t rpm);       // pushes to FOC pipeline
void Motor_reset_link(void);              // re-arms the comm channel
```

## Build

```sh
fsm check examples/motor/motor.fsm
fsm generate --target c99 examples/motor/motor.fsm --out examples/motor/generated/
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror -c examples/motor/generated/*.c
```

## Replay a trace

```sh
fsm test examples/motor/
```

The included `.trace` exercises every state and every transition.
