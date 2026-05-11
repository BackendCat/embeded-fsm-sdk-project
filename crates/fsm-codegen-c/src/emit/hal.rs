//! `fsm_hal.h` contract header — emitted once per `fsm generate` invocation.
//!
//! Per Doc 00 §10.3 (HAL mandatory) and Doc 16 §2 (HAL header contract).
//! The user MUST provide a `fsm_hal.c` (or equivalent) that implements the
//! declared functions. The generated `Motor.c` includes this header for
//! clock + assertion access.

use crate::config::CodegenConfig;

use super::license::header_block;
use super::{EmittedFile, FileRole};

/// Emit the shared HAL header. Path is always `fsm_hal.h`.
pub fn emit_hal_header(config: &CodegenConfig) -> EmittedFile {
    let header = header_block(config, None);
    // NOTE on -Wpedantic: the POSIX reference implementation is gated on
    // FSM_HAL_PROVIDE_POSIX_REFERENCE and emitted as `static` so it can
    // coexist with the non-static `extern` declaration above. The user must
    // #define _POSIX_C_SOURCE before including this header to get
    // clock_gettime — we do that automatically inside the gate.
    let body = format!(
        r#"#ifndef FSM_HAL_H
#define FSM_HAL_H

/* The POSIX feature macro MUST be set before any system header is parsed.
 * Targets that include `<time.h>` etc. through their RTOS headers can
 * either provide their own `_POSIX_C_SOURCE` macro or define
 * `FSM_HAL_PROVIDE_POSIX_REFERENCE` to opt into the reference impl. */
#if defined(FSM_HAL_PROVIDE_POSIX_REFERENCE) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {{
#endif

/* ────────────────────────────────────────────────────────────────────────
 * Clock contract (Doc 16 §3).
 *
 * The user MUST implement this function. Requirements:
 *  - Monotonically non-decreasing.
 *  - Callable from any context (main loop, task, ISR).
 *  - 32-bit wraparound after ~49.7 days is permitted; the runtime relies
 *    on unsigned subtraction to handle wrap correctly.
 *  - Precision MUST be ≤ 1 ms for sub-100ms timers.
 * ──────────────────────────────────────────────────────────────────────── */
#ifndef FSM_HAL_PROVIDE_POSIX_REFERENCE
uint32_t fsm_hal_clock_now_ms(void);
void fsm_hal_assert(bool cond, const char *msg);
#endif

#ifndef FSM_ASSERT
#define FSM_ASSERT(cond) fsm_hal_assert((cond), #cond)
#endif

/* ────────────────────────────────────────────────────────────────────────
 * Optional: critical sections for ISR-safe posting. The default is a
 * no-op; targets that need ISR-safe behaviour redefine these macros in
 * their own platform header included before fsm_hal.h.
 * ──────────────────────────────────────────────────────────────────────── */
#ifndef FSM_ENTER_CRITICAL
#define FSM_ENTER_CRITICAL() ((void)0)
#endif
#ifndef FSM_EXIT_CRITICAL
#define FSM_EXIT_CRITICAL()  ((void)0)
#endif

/* ────────────────────────────────────────────────────────────────────────
 * Queue overflow policy tags. The generated Motor_conf.h selects one via
 * the per-machine MOTOR_QUEUE_OVERFLOW macro.
 * ──────────────────────────────────────────────────────────────────────── */
#define FSM_QUEUE_ASSERT        0
#define FSM_QUEUE_DROP_NEWEST   1

/* ────────────────────────────────────────────────────────────────────────
 * Reference host-only implementation (POSIX). Compile a single translation
 * unit with -DFSM_HAL_PROVIDE_POSIX_REFERENCE to pick up trivial
 * implementations suitable for `cargo test` host runs. Embedded targets
 * MUST provide their own.
 *
 * When the macro is defined the symbols below are emitted as `static
 * inline` so multiple TUs may include this header without violating ODR.
 * ──────────────────────────────────────────────────────────────────────── */
#ifdef FSM_HAL_PROVIDE_POSIX_REFERENCE
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static inline uint32_t fsm_hal_clock_now_ms(void) {{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)((uint64_t)ts.tv_sec * 1000u + (uint64_t)ts.tv_nsec / 1000000u);
}}

static inline void fsm_hal_assert(bool cond, const char *msg) {{
    if (!cond) {{
        fprintf(stderr, "[FSM ASSERT] %s\n", msg);
        abort();
    }}
}}
#endif /* FSM_HAL_PROVIDE_POSIX_REFERENCE */

#ifdef __cplusplus
}}
#endif

#endif /* FSM_HAL_H */
"#,
    );
    EmittedFile {
        path: "fsm_hal.h".to_owned(),
        role: FileRole::Hal,
        content: format!("{}\n{}", header, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal_header_declares_clock_and_assert() {
        let f = emit_hal_header(&CodegenConfig::default());
        assert_eq!(f.path, "fsm_hal.h");
        assert_eq!(f.role, FileRole::Hal);
        assert!(f.content.contains("uint32_t fsm_hal_clock_now_ms(void);"));
        assert!(f
            .content
            .contains("void fsm_hal_assert(bool cond, const char *msg);"));
    }

    #[test]
    fn hal_header_defines_overflow_macros() {
        let f = emit_hal_header(&CodegenConfig::default());
        assert!(f.content.contains("FSM_QUEUE_ASSERT"));
        assert!(f.content.contains("FSM_QUEUE_DROP_NEWEST"));
    }
}
