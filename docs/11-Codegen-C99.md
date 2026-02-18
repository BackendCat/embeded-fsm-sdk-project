# FSM Studio — C99 Code Generator Specification

**Document ID:** FSM-SPEC-GEN-C
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-SEM, FSM-SPEC-IR, FSM-SPEC-HAL

Specifies the exact C99 code emitted by the FSM compiler for each IR construct.
This is the primary deliverable of the toolchain — the generated C runtime that runs
on embedded targets.

---

# 1. Design Goals

1. **Heap-free.** Generated code MUST NOT call `malloc`, `calloc`, `realloc`, `free`,
   or `new`/`delete`. All storage is static or stack-allocated.
2. **Portable C99.** Generated code MUST compile with
   `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` and with
   `arm-none-eabi-gcc -std=c99` with no warnings or errors.
3. **No global state.** All machine state lives in the user-allocated `M_t` struct.
   Multiple independent instances are supported.
4. **Deterministic.** The dispatch function is fully deterministic given the same
   inputs (state, event, context). No randomness, no threading dependencies.
5. **Auditable.** Generated code is human-readable, commented, and maps directly to
   the DSL source. Line numbers from the source are referenced in generated comments.

---

# 2. Generated File Layout

For a machine named `Motor`, the compiler generates exactly four files:

| File | Contents |
|---|---|
| `Motor.h` | Public API: types, `Motor_dispatch()`, `Motor_init()`, `Motor_post()`, `Motor_tick()` |
| `Motor.c` | Implementation: dispatch logic, entry/exit functions, state tables |
| `Motor_impl.h` | User contract: extern declarations the user MUST implement |
| `Motor_conf.h` | Compile-time configuration macros |

The user includes `Motor.h` and implements the functions declared in `Motor_impl.h`.

---

# 3. Context Struct

The context struct is defined in `Motor.h`. It contains:
1. All context fields declared in the DSL `context` block.
2. Internal FSM bookkeeping fields (prefixed with `_`).

```c
/* Motor.h — generated, do not edit */
#ifndef MOTOR_H
#define MOTOR_H

#include <stdint.h>
#include <stdbool.h>
#include "Motor_conf.h"

/* ── State IDs ─────────────────────────────────────────────────────────── */
typedef enum {
    MOTOR_STATE_ROOT        = 0,
    MOTOR_STATE_IDLE        = 1,
    MOTOR_STATE_RUNNING     = 2,
    MOTOR_STATE_ERROR       = 3,
    MOTOR_STATE__COUNT      = 4
} Motor_StateId_t;

/* ── Event IDs ──────────────────────────────────────────────────────────── */
typedef enum {
    MOTOR_EVENT_START = 0,
    MOTOR_EVENT_STOP  = 1,
    MOTOR_EVENT_FAULT = 2,
    MOTOR_EVENT__COUNT = 3
} Motor_EventId_t;

/* ── Payload types ──────────────────────────────────────────────────────── */
typedef struct {
    uint16_t target_speed;
} Motor_StartPayload_t;

/* ── Event union ────────────────────────────────────────────────────────── */
typedef union {
    Motor_EventId_t id;
    struct { Motor_EventId_t id; Motor_StartPayload_t data; } start;
    struct { Motor_EventId_t id; }                            stop;
    struct { Motor_EventId_t id; }                            fault;
} Motor_Event_t;

/* ── Context struct ─────────────────────────────────────────────────────── */
typedef struct {
    /* User context fields */
    uint16_t speed;
    bool     running;
    /* Internal FSM state — DO NOT access directly */
    Motor_StateId_t _state;
    Motor_StateId_t _history_Main;   /* one per history pseudo-state */
    uint8_t         _join_bits;      /* one per join pseudo-state */
    uint32_t        _timer_AfterIdle_remaining_ms;
    /* Internal event queue */
    Motor_Event_t   _queue[MOTOR_QUEUE_CAPACITY];
    uint8_t         _queue_head;
    uint8_t         _queue_tail;
    uint8_t         _queue_count;
} Motor_t;

/* ── Public API ─────────────────────────────────────────────────────────── */
void Motor_init    (Motor_t *m);
void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev);
void Motor_post    (Motor_t *m, const Motor_Event_t *ev);
void Motor_tick    (Motor_t *m, uint32_t elapsed_ms);

#endif /* MOTOR_H */
```

---

# 4. State Enumeration

Naming convention: `{MACHINE}_{STATE}_{NAME}` — all uppercase, non-alphanumeric
characters replaced with `_`, consecutive underscores collapsed to one.

| DSL name | C enum value |
|---|---|
| `ROOT` (synthetic) | `MOTOR_STATE_ROOT` |
| `Idle` | `MOTOR_STATE_IDLE` |
| `Running.Normal` (substate) | `MOTOR_STATE_RUNNING_NORMAL` |

`MOTOR_STATE__COUNT` is always last. It enables static array sizing.

---

# 5. Event Union

The event union uses a tagged union pattern. The `id` field is always at offset 0
(guaranteed by C99 §6.7.2.1). Reading `ev->id` is always safe regardless of which
member of the union was written.

Payload fields are accessed via the named member: `ev->start.data.target_speed`.

---

# 6. Motor_conf.h — Compile-Time Configuration

```c
/* Motor_conf.h — generated skeleton, user may edit */
#ifndef MOTOR_CONF_H
#define MOTOR_CONF_H

/* Queue capacity — must be power of 2 for efficient modular arithmetic */
#define MOTOR_QUEUE_CAPACITY    8

/* Overflow policy: FSM_QUEUE_DROP_OLDEST | FSM_QUEUE_DROP_NEWEST | FSM_QUEUE_ASSERT */
#define MOTOR_QUEUE_OVERFLOW    FSM_QUEUE_ASSERT

/* Code generation strategy: FSM_STRATEGY_SWITCH | FSM_STRATEGY_TABLE */
#define MOTOR_CODEGEN_STRATEGY  FSM_STRATEGY_SWITCH

/* Timer counter type (must hold max timer duration in ms) */
#define MOTOR_TIMER_TYPE        uint32_t

/* Define to enable ISR-safe posting (requires FSM_ENTER_CRITICAL / FSM_EXIT_CRITICAL) */
/* #define MOTOR_QUEUE_ISR_SAFE */

/* Include user HAL */
#include "fsm_hal.h"

#endif /* MOTOR_CONF_H */
```

---

# 7. Motor_impl.h — User Contract

The user MUST provide implementations for every function declared in `Motor_impl.h`.
Linking fails if any symbol is missing.

```c
/* Motor_impl.h — generated, do not edit */
#ifndef MOTOR_IMPL_H
#define MOTOR_IMPL_H

#include "Motor.h"

/* ── Guards (pure — no side effects) ───────────────────────────────────── */
bool Motor_guard_isSpeedValid(const Motor_t *m, const Motor_Event_t *ev);

/* ── Entry actions ──────────────────────────────────────────────────────── */
void Motor_entry_Idle   (Motor_t *m);
void Motor_entry_Running(Motor_t *m);
void Motor_entry_Error  (Motor_t *m);

/* ── Exit actions ───────────────────────────────────────────────────────── */
void Motor_exit_Idle   (Motor_t *m);
void Motor_exit_Running(Motor_t *m);
void Motor_exit_Error  (Motor_t *m);

/* ── Transition actions ─────────────────────────────────────────────────── */
void Motor_action_startMotor(Motor_t *m, const Motor_Event_t *ev);
void Motor_action_stopMotor (Motor_t *m, const Motor_Event_t *ev);

#endif /* MOTOR_IMPL_H */
```

---

# 8. Switch-Based Strategy — `MOTOR_CODEGEN_STRATEGY FSM_STRATEGY_SWITCH`

The dispatch function uses a nested `switch` on current state × event ID.

```c
/* Motor.c — switch-based dispatch (excerpt) */
#include "Motor.h"
#include "Motor_impl.h"

void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    switch (m->_state) {

    case MOTOR_STATE_IDLE:
        switch (ev->id) {
        case MOTOR_EVENT_START:
            if (Motor_guard_isSpeedValid(m, ev)) {
                /* Transition: Idle -> Running */
                /* LCA: ROOT — exit Idle, enter Running */
                Motor_exit_Idle(m);
                Motor_action_startMotor(m, ev);
                m->_state = MOTOR_STATE_RUNNING;
                Motor_entry_Running(m);
                m->_timer_AfterIdle_remaining_ms = 0;  /* cancel Idle timer */
            }
            break;
        default:
            break;
        }
        break;

    case MOTOR_STATE_RUNNING:
        switch (ev->id) {
        case MOTOR_EVENT_STOP:
            Motor_exit_Running(m);
            Motor_action_stopMotor(m, ev);
            m->_state = MOTOR_STATE_IDLE;
            Motor_entry_Idle(m);
            break;
        case MOTOR_EVENT_FAULT:
            Motor_exit_Running(m);
            m->_state = MOTOR_STATE_ERROR;
            Motor_entry_Error(m);
            break;
        default:
            break;
        }
        break;

    case MOTOR_STATE_ERROR:
        /* No transitions from Error */
        break;

    default:
        FSM_ASSERT(0 && "Motor: invalid state");
        break;
    }
}
```

---

# 9. Table-Driven Strategy — `MOTOR_CODEGEN_STRATEGY FSM_STRATEGY_TABLE`

```c
/* Motor.c — table-driven dispatch (excerpt) */

typedef bool (*Motor_GuardFn_t)(const Motor_t *, const Motor_Event_t *);
typedef void (*Motor_ActionFn_t)(Motor_t *, const Motor_Event_t *);

typedef struct {
    Motor_StateId_t  source;
    Motor_EventId_t  trigger;
    Motor_GuardFn_t  guard;     /* NULL = unconditional */
    Motor_ActionFn_t action;    /* NULL = no action */
    Motor_StateId_t  target;
    uint8_t          priority;
    /* LCA depth pre-computed: how many levels to exit/enter */
    uint8_t          exit_depth;
    uint8_t          enter_depth;
} Motor_TransRow_t;

static const Motor_TransRow_t Motor_trans_table[] = {
    /* source             trigger              guard                       action                       target              pri  exit enter */
    { MOTOR_STATE_IDLE,   MOTOR_EVENT_START,   Motor_guard_isSpeedValid,   Motor_action_startMotor,     MOTOR_STATE_RUNNING, 100, 1,   1 },
    { MOTOR_STATE_RUNNING, MOTOR_EVENT_STOP,   NULL,                       Motor_action_stopMotor,      MOTOR_STATE_IDLE,    100, 1,   1 },
    { MOTOR_STATE_RUNNING, MOTOR_EVENT_FAULT,  NULL,                       NULL,                        MOTOR_STATE_ERROR,   100, 1,   1 },
};
#define MOTOR_TRANS_TABLE_SIZE  (sizeof(Motor_trans_table) / sizeof(Motor_trans_table[0]))

/* Exit/entry function tables (indexed by StateId) */
static const Motor_ActionFn_t Motor_exit_fns[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_IDLE]    = (Motor_ActionFn_t)Motor_exit_Idle,
    [MOTOR_STATE_RUNNING] = (Motor_ActionFn_t)Motor_exit_Running,
    [MOTOR_STATE_ERROR]   = (Motor_ActionFn_t)Motor_exit_Error,
};
static const Motor_ActionFn_t Motor_entry_fns[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_IDLE]    = (Motor_ActionFn_t)Motor_entry_Idle,
    [MOTOR_STATE_RUNNING] = (Motor_ActionFn_t)Motor_entry_Running,
    [MOTOR_STATE_ERROR]   = (Motor_ActionFn_t)Motor_entry_Error,
};

void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    for (uint8_t i = 0; i < MOTOR_TRANS_TABLE_SIZE; i++) {
        const Motor_TransRow_t *row = &Motor_trans_table[i];
        if (row->source != m->_state)   continue;
        if (row->trigger != ev->id)     continue;
        if (row->guard && !row->guard(m, ev)) continue;

        /* Execute: exit → action → enter */
        Motor_exit_fns[m->_state](m, ev);
        if (row->action) row->action(m, ev);
        m->_state = row->target;
        Motor_entry_fns[m->_state](m, ev);
        return;
    }
    /* No transition matched — event discarded */
}
```

The table-driven strategy places the transition table in `.rodata` (constant data
section), making it suitable for ROM-constrained targets.

---

# 10. Motor_init Function

```c
void Motor_init(Motor_t *m) {
    /* Zero the entire struct */
    /* NOTE: caller may have zero-initialized via BSS — this is redundant but safe */
    uint8_t *p = (uint8_t *)m;
    for (uint16_t i = 0; i < (uint16_t)sizeof(Motor_t); i++) p[i] = 0;

    /* Set initial state */
    m->_state = MOTOR_STATE_ROOT;

    /* Execute initial entry sequence: ROOT -> Idle */
    m->_state = MOTOR_STATE_IDLE;
    Motor_entry_Idle(m);

    /* Start any timers owned by the initial state */
    m->_timer_AfterIdle_remaining_ms = 5000;
}
```

---

# 11. Timer Integration

```c
void Motor_tick(Motor_t *m, uint32_t elapsed_ms) {
    /* Decrement AfterIdle timer */
    if (m->_state == MOTOR_STATE_IDLE && m->_timer_AfterIdle_remaining_ms > 0) {
        if (elapsed_ms >= m->_timer_AfterIdle_remaining_ms) {
            m->_timer_AfterIdle_remaining_ms = 0;
            /* Fire: inject internal timer event */
            Motor_Event_t timer_ev = { .id = MOTOR_EVENT__TIMER_AFTER_IDLE };
            Motor_dispatch(m, &timer_ev);
        } else {
            m->_timer_AfterIdle_remaining_ms -= elapsed_ms;
        }
    }
}
```

Timer events use a reserved internal event ID range (`MOTOR_EVENT__TIMER_*`) that is
never exposed in the public `Motor_EventId_t` enum. They are synthesized only by
`Motor_tick()`.

---

# 12. Queue Implementation

```c
/* Inline in Motor.c — not exposed in header */

static void Motor_queue_push(Motor_t *m, const Motor_Event_t *ev) {
    if (m->_queue_count >= MOTOR_QUEUE_CAPACITY) {
#if MOTOR_QUEUE_OVERFLOW == FSM_QUEUE_ASSERT
        FSM_ASSERT(0 && "Motor: event queue overflow");
#elif MOTOR_QUEUE_OVERFLOW == FSM_QUEUE_DROP_OLDEST
        m->_queue_head = (m->_queue_head + 1) & (MOTOR_QUEUE_CAPACITY - 1);
        m->_queue_count--;
#else   /* DROP_NEWEST */
        return;
#endif
    }
    m->_queue[m->_queue_tail] = *ev;
    m->_queue_tail = (m->_queue_tail + 1) & (MOTOR_QUEUE_CAPACITY - 1);
    m->_queue_count++;
}

void Motor_post(Motor_t *m, const Motor_Event_t *ev) {
#ifdef MOTOR_QUEUE_ISR_SAFE
    FSM_ENTER_CRITICAL();
#endif
    Motor_queue_push(m, ev);
#ifdef MOTOR_QUEUE_ISR_SAFE
    FSM_EXIT_CRITICAL();
#endif
}
```

The queue capacity MUST be a power of 2 to enable efficient modulo via bitwise AND.
The compiler MUST emit FSM-E0004 (or a configuration error) if `MOTOR_QUEUE_CAPACITY`
is not a power of 2.

---

# 13. Parallel Region Dispatch

When the active state is inside a parallel state, the event is delivered to ALL active
regions:

```c
case MOTOR_STATE_MONITOR_REGION_A_IDLE:
    /* handle event in region A */
    /* FALL THROUGH to also dispatch to region B */
case MOTOR_STATE_MONITOR_REGION_B_IDLE:
    /* handle event in region B */
    break;
```

In the table-driven strategy, all rows matching the current event with source states
in parallel regions are executed in order.

---

# 14. History State Implementation

History is stored as one `Motor_StateId_t` field per history pseudo-state:

```c
typedef struct {
    /* ... */
    Motor_StateId_t _history_Operational;   /* shallow history of Operational */
} Motor_t;
```

On exit from a composite state with history, the implementation records the current
active child before calling exit actions:

```c
/* Before calling exit_Running() when exiting Operational: */
m->_history_Operational = m->_state;   /* record the leaving substate */
```

On `~>` (history transition) entry:

```c
Motor_StateId_t restore = m->_history_Operational;
if (restore == MOTOR_STATE_ROOT) {
    restore = MOTOR_STATE_RUNNING_NORMAL;   /* default */
}
m->_state = restore;
Motor_entry_fns[restore](m, NULL);
```

---

# 15. Completion Event Handling

After each entry sequence, the dispatcher checks whether the new state is `final`:

```c
static void Motor_handle_completion(Motor_t *m) {
    if (m->_state == MOTOR_STATE_DONE) {   /* DONE is final */
        /* Generate completion event — process immediately */
        Motor_Event_t comp = { .id = MOTOR_EVENT__COMPLETION };
        Motor_dispatch(m, &comp);
    }
}
```

The completion event is an internal synthetic event processed immediately (not queued
in the user-facing queue). Infinite loop protection:

```c
static uint8_t _completion_depth = 0;

static void Motor_handle_completion(Motor_t *m) {
    _completion_depth++;
    FSM_ASSERT(_completion_depth <= 100 && "FSM-E0900: completion chain depth exceeded");
    /* ... */
    _completion_depth--;
}
```

---

# 16. Fork/Join Implementation

Fork and join use a bitfield in the context struct:

```c
/* One bit per join pseudo-state's source region */
uint8_t _join_AllDone_bits;    /* bits: 0=SensorsRegion, 1=OutputRegion */
```

On entry to each join source:

```c
/* Entering Monitor.Sensors.Done (join source, bit 0): */
m->_join_AllDone_bits |= (1u << 0);
if (m->_join_AllDone_bits == 0x03u) {   /* all 2 bits set */
    m->_join_AllDone_bits = 0;
    /* fire join transition */
    Motor_dispatch_join_AllDone(m);
}
```

---

# 17. Heap-Free Guarantee

The following patterns MUST NOT appear in any generated file:

- `malloc`, `calloc`, `realloc`, `free`
- `new`, `delete`, `new[]`, `delete[]`
- Variable-length arrays (VLAs): `int arr[n]` where `n` is not a compile-time constant
- `alloca`

The compiler MUST statically verify this by inspecting its own output before emitting.
The CI pipeline MUST run `nm generated/Motor.o | grep -E 'malloc|calloc|free'` and fail
if any such symbol is referenced.

All arrays are fixed-size:
- Queue: `Motor_Event_t _queue[MOTOR_QUEUE_CAPACITY]`
- State table: `static const Motor_TransRow_t Motor_trans_table[]` (`.rodata`)
- Entry/exit function pointer tables: `static const Motor_ActionFn_t Motor_entry_fns[]` (`.rodata`)

---

# 18. Inline Action Code Generation

When the DSL action block contains inline statements (not just extern calls), the
compiler generates a helper function:

DSL:
```
on START: {
    ctx.speed = payload.target_speed;
    if (ctx.speed > 3000) { ctx.speed = 3000; }
    startMotor();
}
```

Generated:
```c
/* Auto-generated action function for Idle->Running:START transition */
static void Motor_action__t_idle_running_START(Motor_t *m, const Motor_Event_t *ev) {
    m->speed = ev->start.data.target_speed;
    if (m->speed > 3000u) { m->speed = 3000u; }
    Motor_action_startMotor(m, ev);
}
```

Inline while/for loops generate directly as C loops inside the action function.
`FSM-W0200` is emitted at compile time (warning only; code is still generated).

---

# 19. Complete Example — Motor Machine

Full 3-state machine (`Idle → Running → Error`) source and generated files are
provided in the conformance test suite at:

```
tests/04-codegen-c/sources/motor-3state.fsm
tests/04-codegen-c/expected/Motor.h
tests/04-codegen-c/expected/Motor.c
tests/04-codegen-c/expected/Motor_impl.h
tests/04-codegen-c/expected/Motor_conf.h
```

The test harness at `tests/04-codegen-c/harness/motor_test.c` compiles and runs the
generated code with a POSIX HAL implementation and verifies the state sequence
`Idle → Running → Idle → Error` for the event sequence
`START(1500) → STOP → START(1500) → FAULT`.

---

*End of FSM-SPEC-GEN-C v1.0.0*
