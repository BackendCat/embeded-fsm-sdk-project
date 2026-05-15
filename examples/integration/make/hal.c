/* Hand-written HAL implementation for the Make integration example.
 *
 * The generated runtime is platform-agnostic; this file is the single
 * point of integration with platform services. Per docs/16-HAL-
 * Specification.md the user MUST provide:
 *
 *   uint32_t fsm_hal_clock_now_ms(void);   // monotonic millisecond clock
 *   void     fsm_hal_assert(bool, char*);  // invariant-violation trap
 *
 * On a real MCU `fsm_hal_clock_now_ms` would read a hardware tick (e.g.
 * Arduino `millis()`, a SysTick counter, or an RTOS tick API) and
 * `fsm_hal_assert` would halt / reset. This host build uses a POSIX
 * monotonic clock and an abort() trap so the example is runnable on the
 * development machine. We provide these symbols directly rather than via
 * the fsm_hal.h POSIX-reference macro so the example mirrors a real port
 * (a hand-written hal.c is what an embedded user actually writes).
 */
#define _POSIX_C_SOURCE 200809L

#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include "fsm_hal.h"

uint32_t fsm_hal_clock_now_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)((uint64_t)ts.tv_sec * 1000u + (uint64_t)ts.tv_nsec / 1000000u);
}

void fsm_hal_assert(bool cond, const char *msg)
{
    if (!cond) {
        fprintf(stderr, "[FSM ASSERT] %s\n", msg);
        abort();
    }
}
