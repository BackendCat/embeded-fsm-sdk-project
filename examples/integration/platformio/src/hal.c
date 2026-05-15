/* HAL implementation for the PlatformIO integration example.
 *
 * PlatformIO builds this for two kinds of target:
 *   - `native`  : the host machine (desktop). Use a POSIX monotonic clock.
 *   - `uno`     : Arduino AVR (ATmega328P). Use the Arduino `millis()`
 *                 tick and a halt loop for the assert trap.
 *
 * The single `#if defined(ARDUINO)` split is the only platform-specific
 * code in the whole project -- the generated FSM runtime is identical on
 * both. This mirrors docs/16 (HAL spec): on Arduino the entire HAL is
 * ~10 lines.
 *
 * NOTE: on the host branch `_POSIX_C_SOURCE` MUST be defined before ANY
 * system header is parsed (it gates `clock_gettime` / `struct timespec`
 * visibility in glibc). It is therefore the very first thing in this
 * file, ahead of every #include.
 */
#if !defined(ARDUINO)
#define _POSIX_C_SOURCE 200809L
#endif

#include <stdint.h>
#include <stdbool.h>

#include "fsm_hal.h"

#if defined(ARDUINO)

#include <Arduino.h>

uint32_t fsm_hal_clock_now_ms(void)
{
    return (uint32_t)millis();
}

void fsm_hal_assert(bool cond, const char *msg)
{
    (void)msg;
    if (!cond) {
        /* No std error stream on bare metal: trap so a debugger / watchdog
         * catches the invariant violation rather than corrupting state. */
        for (;;) {
        }
    }
}

#else /* host / native */

#include <stdio.h>
#include <stdlib.h>
#include <time.h>

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

#endif
