# Printer — Deferred Events

A printer that goes into maintenance. Demonstrates:

- **`defer EVENT`** (`feature deferred`) — the v1.1 UML 2.5.1 §14.2.3.9.1
  deferred-event runtime.
- **Hold-then-replay** — a `PRINT_JOB` that arrives during `Maintenance`
  is not lost; it is held and replayed when the printer returns to `Idle`.
- **Transition-wins** — from `Idle` (which consumes `PRINT_JOB`) the event
  transitions immediately and is never deferred, even though `Maintenance`
  declares `defer PRINT_JOB`.

## State chart

```
Printer
├── initial Idle
├── Idle         — on PRINT_JOB   -> Printing
│                  on START_MAINT -> Maintenance
├── Printing     — on JOB_DONE    -> Idle
└── Maintenance  { defer PRINT_JOB }
                  — on MAINT_DONE -> Idle
```

## Why this example

`Maintenance` cannot service a print job, but a job submitted while the
printer is mid-maintenance should still print once maintenance finishes —
not error, not vanish. `defer PRINT_JOB` expresses exactly that: while
`Maintenance` is in the active configuration and no transition there
consumes `PRINT_JOB`, the event is **held** (not discarded). On
`MAINT_DONE` the machine leaves `Maintenance`; the held `PRINT_JOB` is
released to the **front** of the event queue (FIFO) and reprocessed in
`Idle`, where `Idle —PRINT_JOB→ Printing` finally consumes it (Doc 08
§10.2 / §10.3).

The trace also proves **transition-wins** (UML 2.5.1 §14.2.3.9.1): the
first `PRINT_JOB` is dispatched from `Idle`, which has an enabled
transition for it, so it transitions straight to `Printing` and is never
deferred — deferral is decided only *after* leaf-to-root transition
selection fails.

## Build

```sh
fsm check examples/deferred/deferred.fsm
fsm generate --target c99 examples/deferred/deferred.fsm \
    --out examples/deferred/generated/
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -c examples/deferred/generated/*.c
```

## Replay

```sh
fsm test examples/deferred/
```

The trace exercises transition-wins (`PRINT_JOB` consumed directly from
`Idle`) and the full defer round trip (`PRINT_JOB` held in `Maintenance`,
released and replayed on `MAINT_DONE`, then drained — verifiable by the
`event_deferred` and `event_redispatched` step kinds in `deferred.trace`).

## Codegen note

The C99 emitter ships the deferred-event runtime: a per-state defer-mask
table, a fixed-capacity `_deferred[]` buffer in the machine struct
(`PRINTER_DEFER_CAPACITY`, heap-free per Doc 02 G2), and the
release-to-queue-front drain. The simulator (`fsm test`) and the generated
C are byte-identical on this trace — the codegen behavioural-acceptance
test `crates/fsm-codegen-c/tests/defer_codegen_runs.rs` compiles and runs
the generated Printer and asserts the same hold/replay/exactly-once
behaviour the simulator records here.
