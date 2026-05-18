/* SPDX-License-Identifier: MIT
 *
 * Phase-6.0-W2 — ARM semihosting primitives for the on-target QEMU
 * differential harness (Doc 32 §1 W2). NOT shipped product code.
 *
 * Semihosting is the standard ARM mechanism for a target running under a
 * debugger/emulator to call back into the host for I/O and process control
 * (ARM "Semihosting for AArch32 and AArch64", the canonical embedded-CI
 * pattern Doc 32 §1 W2's emulator-decision rationale (c) cites). The target
 * issues `BKPT 0xAB` (Thumb) with the operation number in r0 and an
 * argument block pointer in r1; `qemu-system-arm` with `-semihosting`
 * services it on the host.
 *
 * Only the two operations the harness needs are declared:
 *   - SYS_WRITE0 (0x04): write a NUL-terminated string to the host's
 *     debug console (QEMU routes it to its stdout/serial — captured by the
 *     host test). This is how the unmodified codegen `fsm_trace_emit` weak
 *     sink's strong override streams the `FSM_TRACE` records out.
 *   - SYS_EXIT  (0x18): terminate with a status. Used to signal a clean
 *     `qemu` exit code (ApplicationExit) so the host test can assert the
 *     run succeeded.
 *
 * No FSM semantics live here — this is pure host-callback plumbing.
 */

#ifndef FSM_W2_SEMIHOST_H
#define FSM_W2_SEMIHOST_H

#include <stdint.h>

/* ARM semihosting operation numbers (subset). */
#define SEMIHOST_SYS_WRITE0 0x04u
#define SEMIHOST_SYS_EXIT 0x18u

/* ADP_Stopped_ApplicationExit — the "application exited normally" reason
 * code for SYS_EXIT (ARM semihosting spec §5.5.2). On Cortex-M (AArch32)
 * SYS_EXIT takes the reason code directly in r1. */
#define SEMIHOST_ADP_STOPPED_APPLICATION_EXIT 0x20026u

/* Issue a semihosting call: op -> r0, arg -> r1, BKPT 0xAB, return r0.
 * `static inline` so each TU that needs it gets its own copy with no ODR
 * clash (the harness has few TUs; this keeps the call sites trivial). The
 * inline asm clobbers nothing the C ABI cares about beyond r0/r1 and is
 * memory-clobbered so the compiler does not reorder a buffer fill past the
 * host write. */
static inline uint32_t semihost_call(uint32_t op, const void *arg)
{
    register uint32_t r0 __asm__("r0") = op;
    register uint32_t r1 __asm__("r1") = (uint32_t)arg;
    __asm__ volatile("bkpt 0xAB"
                     : "+r"(r0)
                     : "r"(r1)
                     : "memory");
    return r0;
}

/* Write a NUL-terminated string to the host debug console. */
static inline void semihost_write0(const char *s)
{
    /* SYS_WRITE0 takes the string pointer directly in r1 (NOT a pointer to
     * a pointer) per the ARM semihosting spec. */
    semihost_call(SEMIHOST_SYS_WRITE0, (const void *)s);
}

/* Terminate the program with a normal-exit status (status 0 by ABI for
 * ApplicationExit). The host sees a clean qemu exit code. */
static inline void semihost_exit_success(void)
{
    /* AArch32 SYS_EXIT: r1 = reason code (ADP_Stopped_ApplicationExit). The
     * emulator returns exit code 0 for ApplicationExit. */
    semihost_call(SEMIHOST_SYS_EXIT,
                  (const void *)SEMIHOST_ADP_STOPPED_APPLICATION_EXIT);
}

#endif /* FSM_W2_SEMIHOST_H */
