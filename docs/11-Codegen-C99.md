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
bool Motor_dequeue (Motor_t *m, Motor_Event_t *out);
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
typedef void (*Motor_EntryExitFn_t)(Motor_t *);

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
static const Motor_EntryExitFn_t Motor_exit_fns[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_IDLE]    = Motor_exit_Idle,
    [MOTOR_STATE_RUNNING] = Motor_exit_Running,
    [MOTOR_STATE_ERROR]   = Motor_exit_Error,
};
static const Motor_EntryExitFn_t Motor_entry_fns[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_IDLE]    = Motor_entry_Idle,
    [MOTOR_STATE_RUNNING] = Motor_entry_Running,
    [MOTOR_STATE_ERROR]   = Motor_entry_Error,
};

void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    for (uint8_t i = 0; i < MOTOR_TRANS_TABLE_SIZE; i++) {
        const Motor_TransRow_t *row = &Motor_trans_table[i];
        if (row->source != m->_state)   continue;
        if (row->trigger != ev->id)     continue;
        if (row->guard && !row->guard(m, ev)) continue;

        /* Execute: exit → action → enter */
        Motor_exit_fns[m->_state](m);
        if (row->action) row->action(m, ev);
        m->_state = row->target;
        Motor_entry_fns[m->_state](m);
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

On history (`-> History`) entry:

```c
Motor_StateId_t restore = m->_history_Operational;
if (restore == MOTOR_STATE_ROOT) {
    restore = MOTOR_STATE_RUNNING_NORMAL;   /* default */
}
m->_state = restore;
Motor_entry_fns[restore](m);
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
- Entry/exit function pointer tables: `static const Motor_EntryExitFn_t Motor_entry_fns[]` (`.rodata`)

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

# 19. Choice Pseudo-State Codegen

A `choice` pseudo-state generates a chain of `if/else if/else` evaluating guards in declaration order. The `[else]` branch maps to the final `else`.

```c
/* Choice pseudo-state: SpeedSelect */
/* Guards evaluated in declaration order; first true branch taken */
static void Motor_choice_SpeedSelect(Motor_t *m, const Motor_Event_t *ev) {
    if (m->speed > 100u) {
        /* [ctx.speed > 100] -> Fast */
        m->_state = MOTOR_STATE_FAST;
        Motor_entry_Fast(m);
    } else if (m->speed > 0u) {
        /* [ctx.speed > 0] -> Normal */
        m->_state = MOTOR_STATE_NORMAL;
        Motor_entry_Normal(m);
    } else {
        /* [else] -> Stopped */
        m->_state = MOTOR_STATE_STOPPED;
        Motor_entry_Stopped(m);
    }
}
```

The choice function is called from the dispatch path wherever a transition targets the choice pseudo-state. The `[else]` branch is mandatory (absence is `FSM-E0100`).

---

# 20. Junction Pseudo-State Codegen

A `junction` pseudo-state is semantically similar to `choice` but branches MUST be provably disjoint and exhaustive at compile time. When all guards are compile-time constants, the compiler MAY optimize to a single unconditional branch. When guards are not all static, the junction falls back to a choice-like runtime chain.

**Static junction (all guards constant):**
```c
/* Junction: ModeSelect — all guards are constant, resolved at compile time */
/* Only the matching branch is emitted */
static void Motor_junction_ModeSelect(Motor_t *m, const Motor_Event_t *ev) {
    /* Compile-time evaluation: MODE == 1, so only this branch is emitted */
    m->_state = MOTOR_STATE_FAST_MODE;
    Motor_entry_FastMode(m);
}
```

**Dynamic junction (not all guards static — falls back to runtime chain):**
```c
/* Junction: TypeRoute — runtime evaluation (guards are not all constant) */
static void Motor_junction_TypeRoute(Motor_t *m, const Motor_Event_t *ev) {
    if (m->mode == MOTOR_MODE_A) {
        m->_state = MOTOR_STATE_HANDLER_A;
        Motor_entry_HandlerA(m);
    } else if (m->mode == MOTOR_MODE_B) {
        m->_state = MOTOR_STATE_HANDLER_B;
        Motor_entry_HandlerB(m);
    } else {
        /* [else] -> DefaultHandler */
        m->_state = MOTOR_STATE_DEFAULT_HANDLER;
        Motor_entry_DefaultHandler(m);
    }
}
```

---

# 21. Deep History Codegen

Deep history stores the **leaf** state (not just the direct child) of a composite state. On deep history entry, the machine restores directly to that leaf, executing entry actions for all intermediate states from LCA down.

```c
typedef struct {
    /* ... user context fields ... */
    Motor_StateId_t _state;
    Motor_StateId_t _deep_history_Operational;  /* stores the leaf state */
} Motor_t;
```

**Recording deep history on exit:**
```c
/* Before exiting Operational composite state: record the active leaf */
m->_deep_history_Operational = m->_state;  /* _state always holds the leaf */
```

**Restoring deep history on entry:**
```c
/* Transition -> DeepHistory of Operational */
static void Motor_enter_deep_history_Operational(Motor_t *m) {
    Motor_StateId_t restore = m->_deep_history_Operational;
    if (restore == MOTOR_STATE_ROOT) {
        /* No history recorded — use default */
        restore = MOTOR_STATE_OPERATIONAL_NORMAL;
    }

    /* Enter all states from Operational down to the leaf */
    /* The entry path is pre-computed as a static table */
    static const Motor_StateId_t path_to_Normal[] = {
        MOTOR_STATE_OPERATIONAL, MOTOR_STATE_OPERATIONAL_NORMAL
    };
    static const Motor_StateId_t path_to_Fast[] = {
        MOTOR_STATE_OPERATIONAL, MOTOR_STATE_OPERATIONAL_FAST
    };
    static const Motor_StateId_t path_to_Fast_Boost[] = {
        MOTOR_STATE_OPERATIONAL, MOTOR_STATE_OPERATIONAL_FAST,
        MOTOR_STATE_OPERATIONAL_FAST_BOOST
    };

    const Motor_StateId_t *path = NULL;
    uint8_t path_len = 0;

    switch (restore) {
    case MOTOR_STATE_OPERATIONAL_NORMAL:
        path = path_to_Normal; path_len = 2; break;
    case MOTOR_STATE_OPERATIONAL_FAST:
        path = path_to_Fast; path_len = 2; break;
    case MOTOR_STATE_OPERATIONAL_FAST_BOOST:
        path = path_to_Fast_Boost; path_len = 3; break;
    default:
        FSM_ASSERT(0 && "Motor: invalid deep history state");
        return;
    }

    for (uint8_t i = 0; i < path_len; i++) {
        Motor_entry_fns[path[i]](m);
    }
    m->_state = restore;
}
```

Contrast with **shallow history** (§14): shallow history stores only the direct child of the composite state, then expands that child's initial substates normally. Deep history skips initial expansion and restores the exact leaf.

---

# 22. Deferred Event Codegen

Deferred events use a per-state bitmask and a circular buffer for re-queued events.

**Defer mask:**
```c
/* Per-state defer bitmask — bit N = event ID N is deferred in this state */
static const uint32_t Motor_defer_mask[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_IDLE]       = 0x00000000u,   /* defers nothing */
    [MOTOR_STATE_CONNECTING] = (1u << MOTOR_EVENT_DATA_RECEIVED),  /* defers DATA_RECEIVED */
    [MOTOR_STATE_CONNECTED]  = 0x00000000u,
};
```

**Deferred event queue:**
```c
/* Circular buffer for deferred events */
typedef struct {
    Motor_Event_t buffer[MOTOR_DEFER_QUEUE_CAPACITY];
    uint8_t head;
    uint8_t tail;
    uint8_t count;
} Motor_DeferQueue_t;

static void Motor_defer_enqueue(Motor_DeferQueue_t *q, const Motor_Event_t *ev) {
    FSM_ASSERT(q->count < MOTOR_DEFER_QUEUE_CAPACITY && "defer queue overflow");
    q->buffer[q->tail] = *ev;
    q->tail = (q->tail + 1u) & (MOTOR_DEFER_QUEUE_CAPACITY - 1u);
    q->count++;
}

static bool Motor_defer_dequeue(Motor_DeferQueue_t *q, Motor_Event_t *out) {
    if (q->count == 0u) return false;
    *out = q->buffer[q->head];
    q->head = (q->head + 1u) & (MOTOR_DEFER_QUEUE_CAPACITY - 1u);
    q->count--;
    return true;
}
```

**Integration with dispatch loop:**
```c
void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    /* Check if event is deferred in the current state */
    if (Motor_defer_mask[m->_state] & (1u << ev->id)) {
        Motor_defer_enqueue(&m->_defer_queue, ev);
        return;  /* event deferred — not dispatched */
    }

    Motor_StateId_t prev_state = m->_state;
    /* ... normal dispatch logic ... */

    /* On state change: release deferred events if new state does not defer them */
    if (m->_state != prev_state) {
        Motor_release_deferred(m);
    }
}

static void Motor_release_deferred(Motor_t *m) {
    Motor_Event_t ev;
    Motor_DeferQueue_t tmp = {0};  /* temporary queue for events still deferred */

    while (Motor_defer_dequeue(&m->_defer_queue, &ev)) {
        if (Motor_defer_mask[m->_state] & (1u << ev.id)) {
            /* Still deferred in new state — keep in queue */
            Motor_defer_enqueue(&tmp, &ev);
        } else {
            /* Released — prepend to external queue for processing */
            Motor_queue_push_front(m, &ev);
        }
    }
    m->_defer_queue = tmp;
}
```

---

# 23. Internal Transition Codegen

Internal transitions do NOT call exit/entry actions. They are generated as a separate case in the dispatch that executes actions without a state change.

```c
void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    switch (m->_state) {

    case MOTOR_STATE_HB_ACTIVE:
        switch (ev->id) {
        case MOTOR_EVENT_HEARTBEAT_ACK:
            /* Internal transition — no state change, no exit/entry */
            Motor_action_onHbReceived(m, ev);
            break;
        case MOTOR_EVENT_HEARTBEAT_TIMEOUT:
            /* External transition: HBActive -> HBFailed */
            Motor_exit_HBActive(m);
            m->_state = MOTOR_STATE_HB_FAILED;
            Motor_entry_HBFailed(m);
            break;
        default:
            break;
        }
        break;

    /* ... other states ... */
    }
}
```

Key difference from external self-transition: an external self-transition (`on E -> SameState`) calls `exit_SameState()`, then `entry_SameState()`, restarting timers. An internal transition (`on E : action()`) does none of this.

---

# 24. Parallel State Encoding

For machines with parallel regions, the context struct contains one `_state_regionN` field per region instead of a single `_state` field.

**Struct definition:**
```c
typedef struct {
    /* User context fields */
    uint16_t speed;
    bool running;

    /* One state field per region */
    Motor_StateId_t _state_DataPath;          /* Region: DataPath */
    Motor_StateId_t _state_HeartbeatMonitor;  /* Region: HeartbeatMonitor */

    /* Parent composite state (for LCA computation) */
    Motor_StateId_t _state_parent;  /* e.g., MOTOR_STATE_CONNECTED */

    /* ... timers, queue, etc. */
} Motor_t;
```

**Dispatch loop — iterate all regions:**
```c
void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    /* If we are inside the Connected parallel state, dispatch to all regions */
    if (m->_state_parent == MOTOR_STATE_CONNECTED) {
        /* Region 1: DataPath */
        Motor_dispatch_region_DataPath(m, ev);
        /* Region 2: HeartbeatMonitor */
        Motor_dispatch_region_HeartbeatMonitor(m, ev);
        return;
    }

    /* Non-parallel dispatch (outer states) */
    switch (m->_state_parent) {
    case MOTOR_STATE_DISCONNECTED:
        /* ... */
        break;
    case MOTOR_STATE_CONNECTING:
        /* ... */
        break;
    /* ... */
    }
}

static void Motor_dispatch_region_DataPath(Motor_t *m, const Motor_Event_t *ev) {
    switch (m->_state_DataPath) {
    case MOTOR_STATE_IDLE_DATA:
        switch (ev->id) {
        case MOTOR_EVENT_DATA_RECEIVED:
            Motor_exit_IdleData(m);
            m->_state_DataPath = MOTOR_STATE_PROCESSING;
            Motor_entry_Processing(m);
            break;
        /* ... */
        }
        break;
    /* ... */
    }
}
```

---

# 25. LCA (Least Common Ancestor) Lookup Table

The LCA is pre-computed as a static lookup table indexed by `(source, target)`. This avoids runtime tree traversal.

**Table generation algorithm:**
```
for each pair (S, T) in states × states:
    lca_table[S][T] = compute_lca(S, T)    // using path_to_root algorithm from §5.2
```

**Generated code:**
```c
/* LCA lookup table: lca_table[source][target] = LCA state ID */
static const Motor_StateId_t Motor_lca_table[MOTOR_STATE__COUNT][MOTOR_STATE__COUNT] = {
    /*                   ROOT   IDLE   RUNNING  ERROR  OP_NORMAL  OP_FAST */
    /* ROOT */        {  ROOT,  ROOT,  ROOT,    ROOT,  ROOT,      ROOT    },
    /* IDLE */        {  ROOT,  IDLE,  ROOT,    ROOT,  ROOT,      ROOT    },
    /* RUNNING */     {  ROOT,  ROOT,  RUNNING, ROOT,  ROOT,      ROOT    },
    /* ERROR */       {  ROOT,  ROOT,  ROOT,    ERROR, ROOT,      ROOT    },
    /* OP_NORMAL */   {  ROOT,  ROOT,  ROOT,    ROOT,  OP_NORMAL, OPERATIONAL },
    /* OP_FAST */     {  ROOT,  ROOT,  ROOT,    ROOT,  OPERATIONAL, OP_FAST },
};
```

**Lookup function:**
```c
static Motor_StateId_t Motor_lca(Motor_StateId_t source, Motor_StateId_t target) {
    FSM_ASSERT(source < MOTOR_STATE__COUNT);
    FSM_ASSERT(target < MOTOR_STATE__COUNT);
    return Motor_lca_table[source][target];
}
```

The LCA table is used in the dispatch function to determine which exit and entry actions to fire when transitioning between nested states. For flat machines (no composite states), the LCA is always ROOT and the table is trivial.

---

# 26. Fork Entry Sequence Codegen

A fork pseudo-state enters multiple regions simultaneously. The generated code calls entry sequences for each target region in declaration order.

```c
/* Fork: StartAll -> {Sensors.Idle, Output.Off} */
static void Motor_fork_StartAll(Motor_t *m) {
    /* Enter parent parallel state if not already active */
    if (m->_state_parent != MOTOR_STATE_MONITOR) {
        m->_state_parent = MOTOR_STATE_MONITOR;
        Motor_entry_Monitor(m);
    }

    /* Enter Region 1: Sensors -> Idle (declaration order) */
    m->_state_DataPath = MOTOR_STATE_SENSORS_IDLE;
    Motor_entry_SensorsIdle(m);

    /* Enter Region 2: Output -> Off (declaration order) */
    m->_state_HeartbeatMonitor = MOTOR_STATE_OUTPUT_OFF;
    Motor_entry_OutputOff(m);
}
```

Fork targets MUST be in different orthogonal regions of the same parallel state (`FSM-E0750` if not). Each target MUST be an initial state of its region (`FSM-E0302` if not).

---

# 27. Complete Example — Motor Machine

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
