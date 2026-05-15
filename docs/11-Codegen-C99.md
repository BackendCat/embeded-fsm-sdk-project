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

The user includes `Motor.h`, provides the HAL (`fsm_hal.h` — Doc 16), and
implements the functions declared in `Motor_impl.h`.

> _Updated 2026-05-14: every generated `.c` and `.h` carries an SPDX license
> header (`SPDX-License-Identifier: MIT` by default; user-overridable via
> `fsm generate --license <SPDX>`) per Doc 00 §10.4. See Doc 18 for the
> flag, Doc 11 §27 for the header shape._

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
/* _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §11.3 (multi-
   active-leaf representation); see CHANGELOG._ */
typedef struct {
    /* User context fields */
    uint16_t speed;
    bool     running;
    /* Internal FSM state — DO NOT access directly.

       Multi-active-leaf representation: `_active[]` holds one StateId per
       active region. For non-parallel machines, `_active_count == 1`. This
       replaces the older `_state` + `_state_region_N` slot scheme so
       non-parallel and parallel machines share one dispatch path. */
    Motor_StateId_t _active[MOTOR_MAX_PARALLEL_REGIONS];
    uint8_t         _active_count;
    /* Per-history-pseudo-state storage (direct-child slot; per-region when
       parent is Parallel — see §14 / §21). */
    Motor_StateId_t _history_Main;
    /* Per-join-pseudo-state bit-vector (one bit per join source). */
    uint8_t         _join_bits;
    /* Per-timer arm-on-entry countdown (one per declared timer). */
    uint32_t        _timer_after_idle_remaining_ms;
    /* Per-instance completion-depth counter (was file-scope; now isolated). */
    uint8_t         _completion_depth;
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

# 8. Dispatch Strategies — Overview

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §B-10 / §B-11 /
> §5.1 / §11.14; see CHANGELOG._

Two strategies are normative for v1.0. The `--strategy` CLI flag (Doc 18) and
the `MachineObject.target.strategy` IR field (Doc 09 §3) both can select.

| Strategy | When to pick | Cost — ROM | Cost — dispatch latency | Mechanism |
|---|---|---|---|---|
| `switch` | Small-to-medium machines (≲ 64 states); single best when the C compiler can fold the nested switch into a jump table | Lower fixed overhead; one `Motor_try_transitions_in_state` per state | O(depth × per-state-events) | B-10: nested `switch` + leaf-to-root walk on a static `parent_table[]` |
| `table` | Larger machines, parallel-region heavy code, or ROM-vs-flash tradeoffs that favour `.rodata` | Single `Motor_TransRow_t[]` table; cheaper-per-extra-state | O(N transitions × regions) — linear scan | B-11: collect-then-execute, one selected row per region |
| `auto` (default) | The CLI default when neither IR nor flag picks. Heuristic: `state count < 64` ⇒ `switch`, else `table`. | — | — | Same emit/transition.rs shared semantics either way; pick is structural. |

Both strategies share the same `emit/transition.rs` exit/action/entry
sequencer so behaviour is bit-equivalent across strategies (verified by the
CGEN-002 conformance fixture).

## 8.1 Switch Strategy — `--strategy=switch`

> Implements Doc 00 §B-10 leaf-to-root ancestor walk on a static
> `parent_table[]`. Without this walk, transitions declared on a composite
> parent silently fail to fire from nested leaves (classic HSM codegen bug).

```c
/* Motor.c — switch-based dispatch with ancestor walk */
#include "Motor.h"
#include "Motor_impl.h"
#include "fsm_hal.h"

/* parent_table[s] = direct parent of state s, or ROOT_SENTINEL for top-level.
   Populated by walking IR.MachineObject.root at codegen time. */
static const Motor_StateId_t Motor_parent_table[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_OPERATIONAL]         = MOTOR_STATE__ROOT_SENTINEL,
    [MOTOR_STATE_OPERATIONAL_RUNNING] = MOTOR_STATE_OPERATIONAL,
    [MOTOR_STATE_ERROR]               = MOTOR_STATE__ROOT_SENTINEL,
    /* ... */
};

static bool Motor_try_transitions_in_state(
    Motor_t *m, Motor_StateId_t s, const Motor_Event_t *ev)
{
    switch (s) {
    case MOTOR_STATE_OPERATIONAL:
        switch (ev->id) {
        case MOTOR_EVENT_FAULT:
            /* Composite transition fires regardless of which descendant is active. */
            Motor_execute_transition(m, /*src=*/MOTOR_STATE_OPERATIONAL,
                                         /*tgt=*/MOTOR_STATE_ERROR,
                                         /*kind=*/MOTOR_TKIND_EXTERNAL, ev);
            return true;
        default: break;
        }
        break;
    case MOTOR_STATE_OPERATIONAL_RUNNING:
        switch (ev->id) {
        /* ... per-state generated cases ... */
        default: break;
        }
        break;
    default: break;
    }
    return false;
}

void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    /* For each active leaf (one per region), walk leaf -> ... -> root. */
    for (uint8_t r = 0; r < m->_active_count; r++) {
        Motor_StateId_t s = m->_active[r];
        while (s != MOTOR_STATE__ROOT_SENTINEL) {
            if (Motor_try_transitions_in_state(m, s, ev)) {
                goto next_region;
            }
            s = Motor_parent_table[s];
        }
        /* No ancestor of m->_active[r] defined a transition for ev. */
    next_region:;
    }
}
```

The `parent_table[]` costs ≤ 1 byte per state on ≤256-state machines. Per
`MachineObject.root` walk happens at codegen time. Priority and document-order
within a single state are resolved inside `Motor_try_transitions_in_state` by
emitting cases in `(priority, document_order)` order. The leaf-tried-first
rule handles "inner-beats-outer" implicitly. Confirms Doc 00 §B-10
implementation per P0-2/P0-3 wave.

## 8.2 Table Strategy — `--strategy=table`

> Implements Doc 00 §B-11 collect-then-execute. The early-return form (first
> match wins) loses transitions in parallel composites where the second
> region never gets a turn.

```c
/* Motor.c — table-driven dispatch, two-phase collect-then-execute */

typedef bool (*Motor_GuardFn_t)(const Motor_t *, const Motor_Event_t *);
typedef void (*Motor_ActionFn_t)(Motor_t *, const Motor_Event_t *);

typedef enum {
    MOTOR_TKIND_EXTERNAL = 0,
    MOTOR_TKIND_LOCAL    = 1,
    MOTOR_TKIND_INTERNAL = 2,
    MOTOR_TKIND_COMPLETION = 3,
} Motor_TKind_t;

typedef struct {
    Motor_StateId_t  source;
    Motor_EventId_t  trigger;
    Motor_GuardFn_t  guard;       /* NULL = unconditional */
    Motor_ActionFn_t action;      /* NULL = no action */
    Motor_StateId_t  target;
    Motor_TKind_t    kind;
    uint8_t          priority;
    uint16_t         action_idx;  /* deterministic per-row id; doc order */
} Motor_TransRow_t;

/* Sorted by (source, priority asc, document_order asc) at codegen time. */
static const Motor_TransRow_t Motor_trans_table[] = {
    /* ... rows ... */
};
#define MOTOR_TRANS_TABLE_SIZE  (sizeof(Motor_trans_table) / sizeof(Motor_trans_table[0]))

void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    /* Phase 1 — collect: one matching row per active region. */
    const Motor_TransRow_t *selected[MOTOR_MAX_PARALLEL_REGIONS];
    uint8_t selected_count = 0;

    for (uint8_t r = 0; r < m->_active_count; r++) {
        Motor_StateId_t s = m->_active[r];
        const Motor_TransRow_t *best = NULL;
        while (s != MOTOR_STATE__ROOT_SENTINEL) {
            for (uint16_t i = 0; i < MOTOR_TRANS_TABLE_SIZE; i++) {
                const Motor_TransRow_t *row = &Motor_trans_table[i];
                if (row->source  != s)      continue;
                if (row->trigger != ev->id) continue;
                if (row->guard && !row->guard(m, ev)) continue;
                if (!best || row->priority < best->priority) best = row;
            }
            if (best) break;
            s = Motor_parent_table[s];
        }
        if (best) selected[selected_count++] = best;
    }

    /* Phase 2 — execute: sequence the selected transitions deterministically. */
    for (uint8_t i = 0; i < selected_count; i++) {
        Motor_execute_transition(m, selected[i], ev);
    }
}
```

`Motor_execute_transition` sequences `exit / action / entry` per Doc 08 §6/§7,
consuming the row's `kind` to apply the correct exit/entry-set rules (B-09).
This confirms Doc 00 §B-11 per P0-2/P0-3 wave.

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

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §10.3 (HAL
> mandatory) and §11.5/§11.6 (per-timer event IDs, arm-on-entry); see CHANGELOG._

**HAL is mandatory.** Generated `Motor.c` unconditionally includes
`fsm_hal.h` (Doc 16). The user MUST provide
`uint32_t fsm_hal_clock_now_ms(void)` and `void fsm_hal_assert(...)` symbols
at link time. The codegen uses `fsm_hal_clock_now_ms()` (not wall time, not a
hand-rolled tick counter) to read elapsed milliseconds; the simulator emulates
the same surface deterministically. Zero-duration timers were rejected
upstream by the analyzer with `FSM-E0410` per Doc 00 §B-13.

**Per-timer event IDs.** Every declared timer receives a distinct synthetic
event ID `MOTOR_EVENT_TIMER_<TIMER_ID>_FIRED` so that two timers — and the
shared completion event ID — never collide. Per Doc 00 §11.5 / P0-4 wave.

**Arm-on-entry, disarm-on-exit.** A timer is armed when its owner state is
entered and cleared when the state is exited. Previously the timer was armed
at `Motor_init`, which broke for any timer-owning state that wasn't the
initial state. Per Doc 00 §11.6.

```c
void Motor_tick(Motor_t *m, uint32_t elapsed_ms) {
    /* Decrement after_idle timer (armed on entry to Idle). */
    if (m->_timer_after_idle_remaining_ms > 0) {
        if (elapsed_ms >= m->_timer_after_idle_remaining_ms) {
            m->_timer_after_idle_remaining_ms = 0;
            /* Fire: distinct per-timer event ID, NOT shared completion. */
            Motor_Event_t ev = { .id = MOTOR_EVENT_TIMER_AFTER_IDLE_FIRED };
            Motor_dispatch(m, &ev);
        } else {
            m->_timer_after_idle_remaining_ms -= elapsed_ms;
        }
    }
}

/* Inside Motor_entry_Idle, the timer is armed: */
static void Motor_entry_Idle(Motor_t *m) {
    m->_timer_after_idle_remaining_ms = 5000;  /* arm on entry */
    /* ... entry actions ... */
}

/* Inside Motor_exit_Idle, the timer is disarmed: */
static void Motor_exit_Idle(Motor_t *m) {
    m->_timer_after_idle_remaining_ms = 0;     /* disarm on exit */
    /* ... exit actions ... */
}
```

Timer synthetic events use the dedicated `MOTOR_EVENT_TIMER_<ID>_FIRED` IDs
which are part of the public `Motor_EventId_t` enum (so users can match on
them in `internal on` handlers) but are synthesized only by `Motor_tick()`.

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

> _Updated 2026-05-14 in v1.0 doc reconciliation; see CHANGELOG._

Parallel composites are handled by the multi-active-leaf representation
described in §11 (Doc 00 §11.3 / P0-2/P0-3 wave) — `m->_active[N]` stores one
active leaf per region. Dispatch is delegated to §8.2's collect-then-execute
loop, which selects one matching transition per active region before
executing any of them. The earlier "fall-through across cases" sketch is
retired; behaviour for non-parallel machines is the single-slot
(`_active_count == 1`) special case of the same code path.

---

# 14. History State Implementation

> _Updated 2026-05-14 in v1.0 doc reconciliation; see CHANGELOG._

History is stored as one `Motor_StateId_t` field per history pseudo-state.
The stored value is the **direct child of the composite region** (the
two-level abstraction, not the deepest leaf), so the deepest descendant is
re-expanded via the initial-substate walk on entry. The `default ->` target
is **mandatory** per Doc 00 §B-14; the analyzer rejects defaultless history
declarations with `FSM-E0111`. The historical "no default at runtime" branch
is therefore unreachable in production.

```c
typedef struct {
    /* ... */
    Motor_StateId_t _history_Operational;   /* shallow history: direct child slot */
} Motor_t;
```

On exit from a composite state with history, the implementation records the
**direct child of the composite region** before calling exit actions. The
direct-child resolution helper is generated at codegen time:

```c
/* Before calling exit_Running() when exiting Operational: */
m->_history_Operational = Motor_get_direct_child_of_Operational(m);
```

On history (`-> History`) entry, the default is unconditionally available
(FSM-E0111 ensures it):

```c
Motor_StateId_t restore = (m->_history_Operational != MOTOR_STATE__ROOT_SENTINEL)
                          ? m->_history_Operational
                          : MOTOR_STATE_RUNNING_NORMAL;   /* declared default */
m->_active[region_idx] = restore;
Motor_entry_fns[restore](m);
```

For Deep history whose parent is a Parallel state, one storage slot is
emitted **per region** (not a single `StateId`), per the per-region history
slots rule (Doc 00 §I-26).

---

# 15. Completion Event Handling

> _Updated 2026-05-14 in v1.0 doc reconciliation; see CHANGELOG._

After each entry sequence, the dispatcher invokes `maybe_enqueue_completion`
implementing the Doc 08 §9.1 algorithm: Simple states auto-fire on entry;
Composite states fire when their single-region active leaf is Final;
**Parallel states fire only when every region's active leaf is Final** (the
all-regions-done rule per Doc 00 §B-08). The B-08 simulator and codegen now
match.

```c
static void Motor_handle_completion(Motor_t *m, Motor_StateId_t just_entered) {
    /* Simple = auto-fire on entry */
    if (Motor_state_is_simple[just_entered]) {
        Motor_Event_t comp = { .id = MOTOR_EVENT__COMPLETION,
                               .completion_from = just_entered };
        Motor_dispatch(m, &comp);
        return;
    }
    /* Composite / parallel handled by maybe_enqueue_completion checking all
       regions per Doc 08 §9.1; only enqueues completion when the all-regions
       rule holds. */
}
```

`_completion_depth` is a **per-instance field on `Motor_t`** (not a file-scope
`static`), so two machine instances can run independently without sharing
state. Loop protection unchanged at 100 (Doc 10 FSM-E0900):

```c
static void Motor_dispatch_completion(Motor_t *m, Motor_StateId_t from) {
    m->_completion_depth++;
    FSM_ASSERT(m->_completion_depth <= 100 && "FSM-E0900: completion chain depth exceeded");
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

> _Updated 2026-05-14 in v1.0 doc reconciliation per Doc 00 §11.7 (option-b
> downgrade); see CHANGELOG._

**v1.0 STATUS — DEFERRED.** The analyzer rejects every `defer EVENT`
declaration at compile time with `FSM-E0903` ("`defer` not yet supported;
v1.0 limitation, lands in v1.1"). **No runtime defer path is emitted in
v1.0.** The defer-queue codegen sketch below is the v1.1 reference and is
kept for forward planning only.

The decision to reject-rather-than-silent-drop preserves Doc 02 G1 (no
undefined behaviour). A silent drop would have shipped a violating but
plausible-looking compile, which the audit found and which Doc 00 §11.7
specifies must be honest-instead.

## 22.1 v1.1 Reference Sketch (informative)

When `defer` ships in v1.1, per-region bitmask arrays (one bitmask per active
region, NOT a flat per-machine mask) drive the gate; codegen picks the bitmask
width based on declared event count (`uint32_t` / `uint64_t` /
`uint8_t[ceil(N/8)]`); if `N > 256`, the analyzer raises `FSM-E0903`
("too many event types for defer bitmask — reduce or split machine") per
Doc 00 §G-08.

```c
/* v1.1 reference — not emitted in v1.0. */
static const uint32_t Motor_defer_mask[MOTOR_STATE__COUNT] = {
    [MOTOR_STATE_CONNECTING] = (1u << MOTOR_EVENT_DATA_RECEIVED),
    /* ... */
};
/* Per-region defer queue, release-on-exit semantics per Doc 08 §10. */
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
