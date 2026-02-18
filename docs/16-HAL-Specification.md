# FSM Studio — Hardware Abstraction Layer Specification

**Document ID:** FSM-SPEC-HAL
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-GEN-C

Defines all C functions the user must provide to port the generated FSM runtime to
their target platform. The generated runtime is platform-agnostic; the HAL is the
only point of integration with hardware or RTOS services.

---

# 1. Purpose and Scope

The generated C runtime requires two platform services:
1. **A clock** — to support timer-based transitions (`after X ms`, `every X ms`).
2. **An assertion handler** — to report internal contract violations.

Optionally, ISR-safe event posting requires critical section macros.

All HAL symbols are declared in a single user-provided header: `fsm_hal.h`. The
generated `Motor_conf.h` includes this header. The user creates `fsm_hal.h` once per
project and shares it across all generated machines.

---

# 2. HAL Header Contract

The user MUST create a file named `fsm_hal.h` (or configure the include path so the
compiler finds it). This file MUST define or forward-declare:

- `uint32_t fsm_hal_clock_ms(void)` — wall-clock query
- `void fsm_hal_assert_fail(const char *file, int line, const char *msg)` — assertion handler
- `FSM_ASSERT(cond)` macro — optional override (auto-defined if absent)
- `FSM_ENTER_CRITICAL()` / `FSM_EXIT_CRITICAL()` — required only if `MOTOR_QUEUE_ISR_SAFE` is defined

---

# 3. Timer HAL

```c
/**
 * Returns the current time in milliseconds since system startup.
 *
 * Requirements:
 *   - MUST be monotonically non-decreasing.
 *   - MUST be callable from any context (main loop, task, ISR).
 *   - 32-bit wraparound after ~49.7 days is permitted; the generated
 *     runtime handles wrapping correctly using unsigned subtraction.
 *   - Precision MUST be ≤ 1 ms for timers shorter than 100 ms to work
 *     reliably. For longer timers, lower resolution is acceptable.
 */
uint32_t fsm_hal_clock_ms(void);
```

The generated `Motor_tick(m, elapsed_ms)` function accepts a pre-computed elapsed
interval instead of reading the clock directly. This gives the application full control
over tick scheduling:

```c
/* Typical usage in main loop or RTOS task */
uint32_t last = fsm_hal_clock_ms();
while (1) {
    uint32_t now = fsm_hal_clock_ms();
    Motor_tick(&motor, now - last);   /* safe: unsigned subtraction handles wrap */
    last = now;
    /* ... dispatch posted events, do other work ... */
}
```

---

# 4. Assertion HAL

```c
/**
 * Called when an internal FSM contract is violated.
 *
 * This function MUST NOT return. Typical implementations:
 *   - while(1) — freeze for debugger attach
 *   - NVIC_SystemReset() — hard reset on Cortex-M
 *   - abort() — on POSIX hosts (test/simulation)
 *   - k_panic() — on Zephyr
 *   - vTaskSuspendAll(); for(;;); — on FreeRTOS (freezes scheduler)
 *
 * Parameters:
 *   file — source file name of the assertion site (FSM compiler internals)
 *   line — source line number of the assertion site
 *   msg  — human-readable assertion expression string
 */
void fsm_hal_assert_fail(const char *file, int line, const char *msg);

/**
 * Assertion macro. Override in fsm_hal.h to use a custom handler.
 * The default definition calls fsm_hal_assert_fail().
 */
#ifndef FSM_ASSERT
#define FSM_ASSERT(cond) \
    do { \
        if (!(cond)) fsm_hal_assert_fail(__FILE__, __LINE__, #cond); \
    } while (0)
#endif
```

---

# 5. ISR Safety

By default, `Motor_post()` is NOT ISR-safe. If events must be posted from interrupt
handlers, define `MOTOR_QUEUE_ISR_SAFE` in `Motor_conf.h` and provide critical section
macros in `fsm_hal.h`.

```c
/* Required if MOTOR_QUEUE_ISR_SAFE is defined: */

/**
 * FSM_ENTER_CRITICAL() — enter a critical section.
 * MUST disable the interrupt source(s) that can call Motor_post().
 * MUST be paired with FSM_EXIT_CRITICAL().
 * MUST be usable as a statement (no dangling else).
 */
#define FSM_ENTER_CRITICAL()  /* platform-specific, see reference implementations */

/**
 * FSM_EXIT_CRITICAL() — exit the critical section entered by FSM_ENTER_CRITICAL().
 * MUST restore interrupt state to what it was before FSM_ENTER_CRITICAL().
 */
#define FSM_EXIT_CRITICAL()   /* platform-specific */
```

The macros are used only inside `Motor_post()`:

```c
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

---

# 6. Reference Implementations

## 6.1 Bare-Metal ARM Cortex-M (SysTick)

```c
/* fsm_hal.h — Cortex-M, SysTick at 1 kHz */
#include <stdint.h>
#include "cmsis_device.h"   /* CMSIS header for your MCU family */

static volatile uint32_t _fsm_tick_ms = 0u;

/* Call this from SysTick_Handler every 1ms */
static inline void fsm_systick_1ms(void) { _fsm_tick_ms++; }

uint32_t fsm_hal_clock_ms(void) {
    return _fsm_tick_ms;
}

__attribute__((noreturn))
void fsm_hal_assert_fail(const char *file, int line, const char *msg) {
    (void)file; (void)line; (void)msg;
    __disable_irq();
    for (;;) { __WFI(); }
}

/* ISR-safe critical section via PRIMASK */
#define FSM_ENTER_CRITICAL() \
    uint32_t _fsm_primask = __get_PRIMASK(); __disable_irq()

#define FSM_EXIT_CRITICAL() \
    __set_PRIMASK(_fsm_primask)
```

SysTick configuration (in `main.c`):
```c
SysTick_Config(SystemCoreClock / 1000u);   /* 1ms tick */

void SysTick_Handler(void) {
    fsm_systick_1ms();
}
```

---

## 6.2 FreeRTOS

```c
/* fsm_hal.h — FreeRTOS */
#include "FreeRTOS.h"
#include "task.h"

uint32_t fsm_hal_clock_ms(void) {
    return (uint32_t)(xTaskGetTickCount() * portTICK_PERIOD_MS);
}

__attribute__((noreturn))
void fsm_hal_assert_fail(const char *file, int line, const char *msg) {
    (void)file; (void)line; (void)msg;
    taskDISABLE_INTERRUPTS();
    for (;;);
}

/* FreeRTOS critical sections (uses scheduler suspension, not PRIMASK) */
#define FSM_ENTER_CRITICAL()  taskENTER_CRITICAL()
#define FSM_EXIT_CRITICAL()   taskEXIT_CRITICAL()
```

FSM dispatch task (recommended pattern):
```c
static void fsm_task(void *arg) {
    Motor_t *motor = (Motor_t *)arg;
    TickType_t last_wake = xTaskGetTickCount();

    for (;;) {
        /* Dispatch all queued events */
        Motor_Event_t ev;
        while (Motor_dequeue(motor, &ev)) {
            Motor_dispatch(motor, &ev);
        }
        /* Tick at 10ms intervals */
        vTaskDelayUntil(&last_wake, pdMS_TO_TICKS(10));
        Motor_tick(motor, 10u);
    }
}
```

---

## 6.3 Zephyr RTOS

```c
/* fsm_hal.h — Zephyr */
#include <zephyr/kernel.h>

uint32_t fsm_hal_clock_ms(void) {
    return (uint32_t)k_uptime_get();
}

FUNC_NORETURN void fsm_hal_assert_fail(const char *file, int line, const char *msg) {
    (void)file; (void)line; (void)msg;
    k_panic();
    CODE_UNREACHABLE;
}

#define FSM_ENTER_CRITICAL()  unsigned int _fsm_key = irq_lock()
#define FSM_EXIT_CRITICAL()   irq_unlock(_fsm_key)
```

Zephyr timer-driven tick:
```c
K_TIMER_DEFINE(fsm_timer, fsm_timer_expiry, NULL);

static void fsm_timer_expiry(struct k_timer *t) {
    static uint32_t last = 0;
    uint32_t now = (uint32_t)k_uptime_get();
    Motor_tick(&motor, now - last);
    last = now;
}

/* In application start: */
k_timer_start(&fsm_timer, K_MSEC(10), K_MSEC(10));
```

---

## 6.4 POSIX — Host Simulation and Testing

```c
/* fsm_hal.h — POSIX (Linux/macOS) for unit tests and host simulation */
#include <time.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>

uint32_t fsm_hal_clock_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)((uint64_t)ts.tv_sec * 1000u + (uint64_t)ts.tv_nsec / 1000000u);
}

_Noreturn void fsm_hal_assert_fail(const char *file, int line, const char *msg) {
    fprintf(stderr, "[FSM ASSERT] %s:%d: %s\n", file, line, msg);
    abort();
}

/* No ISR on POSIX — critical section is a no-op for single-threaded tests */
#define FSM_ENTER_CRITICAL()  ((void)0)
#define FSM_EXIT_CRITICAL()   ((void)0)
```

---

## 6.5 Arduino

```c
/* fsm_hal.h — Arduino */
#include <Arduino.h>

uint32_t fsm_hal_clock_ms(void) {
    return millis();
}

void fsm_hal_assert_fail(const char *file, int line, const char *msg) {
    (void)file; (void)line; (void)msg;
    /* No serial available reliably in all contexts; freeze */
    noInterrupts();
    for (;;);
}

/* AVR / ARM Arduino critical section */
#if defined(ARDUINO_ARCH_AVR)
#  define FSM_ENTER_CRITICAL()  uint8_t _sreg = SREG; cli()
#  define FSM_EXIT_CRITICAL()   SREG = _sreg
#elif defined(ARDUINO_ARCH_SAMD) || defined(ARDUINO_ARCH_STM32)
#  define FSM_ENTER_CRITICAL()  noInterrupts()
#  define FSM_EXIT_CRITICAL()   interrupts()
#else
#  define FSM_ENTER_CRITICAL()  ((void)0)
#  define FSM_EXIT_CRITICAL()   ((void)0)
#endif
```

---

# 7. Tick Call Patterns

## 7.1 Bare-Metal Super Loop

```c
int main(void) {
    Motor_t motor;
    Motor_init(&motor);

    uint32_t last = fsm_hal_clock_ms();

    while (1) {
        /* 1. Process all queued events */
        Motor_Event_t ev;
        while (Motor_dequeue(&motor, &ev)) {
            Motor_dispatch(&motor, &ev);
        }

        /* 2. Advance timers */
        uint32_t now = fsm_hal_clock_ms();
        Motor_tick(&motor, now - last);
        last = now;

        /* 3. Other application work */
        HAL_GPIO_TogglePin(LED_PORT, LED_PIN);
        HAL_Delay(1);
    }
}
```

## 7.2 RTOS Periodic Task

```c
void motor_fsm_task(void *arg) {
    (void)arg;
    Motor_t motor;
    Motor_init(&motor);

    uint32_t last = fsm_hal_clock_ms();
    const uint32_t PERIOD_MS = 10u;

    for (;;) {
        osDelay(PERIOD_MS);

        uint32_t now = fsm_hal_clock_ms();
        Motor_tick(&motor, now - last);
        last = now;

        Motor_Event_t ev;
        while (Motor_dequeue(&motor, &ev)) {
            Motor_dispatch(&motor, &ev);
        }
    }
}
```

---

# 8. Memory Requirements

| Resource | Typical Range | Depends On |
|---|---|---|
| ROM (`.text` + `.rodata`) | 200 – 4000 bytes | Number of states, transitions, strategy |
| RAM (context struct) | 8 – 512 bytes | Context fields, timers, queue capacity |
| Stack (dispatch call depth) | 64 – 512 bytes | Maximum state nesting × frame size |

The ROM figure for TABLE strategy is higher than SWITCH due to the stored transition
table, but the dispatch function is smaller and more cache-friendly.

**Heap: 0 bytes** — guaranteed by the heap-free design (FSM-SPEC-GEN-C §19).

---

# 9. HAL Compliance Checklist

Before deploying on a target, verify:

- [ ] `fsm_hal_clock_ms()` is monotonically non-decreasing
- [ ] `fsm_hal_clock_ms()` is callable from ISR context if ISR posting is used
- [ ] `fsm_hal_assert_fail()` NEVER returns
- [ ] If `MOTOR_QUEUE_ISR_SAFE` defined: `FSM_ENTER_CRITICAL()` disables the interrupt
      source that calls `Motor_post()`
- [ ] `Motor_tick()` is called at a rate ≥ 2× the smallest `after X ms` timer
      (Nyquist-style — the minimum tick period should be no more than half the
      shortest timer duration to guarantee timely firing)
- [ ] Stack depth verified with static analysis tool or RTOS high-watermark check
- [ ] No external calls to `malloc`/`free` have been accidentally linked in
      (verify with `nm Motor.o | grep -E 'malloc|free'`)

---

*End of FSM-SPEC-HAL v1.0.0*
