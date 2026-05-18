/* SPDX-License-Identifier: MIT
 *
 * Phase-6.0-W2 — MPS2-AN385 (Cortex-M3) startup + HAL/trace retarget for
 * the on-target QEMU differential harness (Doc 32 §1 W2 / §2 keystone).
 * NOT shipped product code — this is the *test harness* that lets QEMU run
 * the SAME unmodified `FSM_TRACE` C the W1 host differential compiles.
 *
 * ── What this provides ──────────────────────────────────────────────────
 *  1. The Cortex-M3 vector table (initial SP + reset + the fault vectors),
 *     placed at 0x00000000 by the linker script so the core boots.
 *  2. `Reset_Handler`: the standard minimal C-runtime bring-up — copy
 *     `.data` from its FLASH load address to RAM, zero `.bss`, call
 *     `main()`, then issue a clean semihosting exit.
 *  3. A strong `fsm_trace_emit(const char*)` that streams the codegen's
 *     `FSM_TRACE` records over ARM semihosting (SYS_WRITE0). This OVERRIDES
 *     the codegen's `__attribute__((weak))` host-stdout default with ZERO
 *     codegen change — that weak/strong seam is exactly the point
 *     (`emit/trace_hook.rs` module-doc: "W2's on-target lane provides a
 *     semihosting strong override with ZERO codegen change").
 *  4. A DETERMINISTIC `fsm_hal_clock_now_ms()` + `fsm_hal_assert()` — the
 *     established host-HAL counter-clock pattern (`gcc_compile.rs`),
 *     extended with a `fsm_test_set_clock()` the driver advances exactly as
 *     the trace's `advance_clock` deltas do. The FSM runtime uses the
 *     `fsm_hal_*` clock SEAM, not a real peripheral, so QEMU's
 *     peripheral-timing fidelity is irrelevant to the byte-differential
 *     (Doc 32 §1 W2 emulator-decision rationale (a)/(d)).
 *
 * ── The keystone (Doc 32 §2) ────────────────────────────────────────────
 * NONE of this defines or re-implements step / transition-selection /
 * guard-eval / deadlock semantics. It is pure platform plumbing (CRT
 * bring-up + a string sink + a counter clock). The comparison oracle stays
 * the shipped `fsm_simulator::execute_trace`; the C side is the unmodified
 * codegen `FSM_TRACE` hook. The W6 phase-audit negative-greps this file
 * for FSM semantics and will find ∅.
 */

#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#include "semihost.h"

/* ── Linker-script symbols (addresses, not objects) ─────────────────── */
extern uint32_t _sidata; /* .data load address in FLASH                  */
extern uint32_t _sdata;  /* .data start in RAM                           */
extern uint32_t _edata;  /* .data end in RAM                             */
extern uint32_t _sbss;   /* .bss start in RAM                            */
extern uint32_t _ebss;   /* .bss end in RAM                              */
extern uint32_t _estack; /* top of stack (provided by the linker script) */

int main(void);

/* ──────────────────────────────────────────────────────────────────────
 * Deterministic virtual clock (the gcc_compile.rs host-HAL counter-clock
 * pattern, made settable). The driver calls fsm_test_set_clock() to mirror
 * the trace's advance_clock deltas EXACTLY, so the FSM runtime's
 * HAL-delegated time matches the simulator oracle's virtual clock. `clk` is
 * deliberately OUT of the canonical projection (see the codegen
 * trace_hook.rs module-doc) so absolute time is not differenced; this
 * clock only needs to advance monotonically by the driver's deltas to make
 * timer EXPIRY (whose state/transition EFFECT *is* differenced) fire
 * identically to the oracle.
 * ────────────────────────────────────────────────────────────────────── */
static uint32_t g_virtual_clock_ms = 0u;

void fsm_test_set_clock(uint32_t ms);
void fsm_test_set_clock(uint32_t ms) { g_virtual_clock_ms = ms; }

/* The codegen HAL contract (fsm_hal.h, Doc 16 §3). Strong definitions —
 * the generated C only declares these `extern`; the harness provides the
 * bodies, exactly as an embedded target ships its own fsm_hal.c. */
uint32_t fsm_hal_clock_now_ms(void);
uint32_t fsm_hal_clock_now_ms(void) { return g_virtual_clock_ms; }

void fsm_hal_assert(bool cond, const char *msg);
void fsm_hal_assert(bool cond, const char *msg)
{
    if (!cond) {
        semihost_write0("[FSM ASSERT] ");
        semihost_write0(msg ? msg : "(null)");
        semihost_write0("\n");
        /* A failed runtime assertion is a hard error: exit non-zero so the
         * host test fails (NOT ApplicationExit). SYS_EXIT with a non-
         * ApplicationExit reason makes qemu return a non-zero code. */
        semihost_call(SEMIHOST_SYS_EXIT, (const void *)1u);
        for (;;) { /* unreachable — qemu has exited */
        }
    }
}

/* ──────────────────────────────────────────────────────────────────────
 * The STRONG fsm_trace_emit override. The codegen emits a
 * `__attribute__((weak)) void fsm_trace_emit(const char*)` whose default
 * fputs()es to stdout (host). This strong definition wins at link time and
 * streams each canonical `STEP…\n` line out over semihosting SYS_WRITE0
 * (QEMU forwards it to its stdout, captured by the host test). ZERO
 * codegen change — the weak/strong seam IS the W2 mechanism.
 *
 * SYS_WRITE0 needs a NUL-terminated string; the codegen line builder
 * already NUL-terminates `buf` (trace_hook.rs: `buf[len]='\0'`), so the
 * pointer is passed straight through. No reformatting — byte-for-byte the
 * same line the host differential's stdout-sink would print.
 * ────────────────────────────────────────────────────────────────────── */
void fsm_trace_emit(const char *line);
void fsm_trace_emit(const char *line)
{
    if (line != 0) {
        semihost_write0(line);
    }
}

/* ──────────────────────────────────────────────────────────────────────
 * Reset_Handler — minimal Cortex-M3 C-runtime bring-up.
 * ────────────────────────────────────────────────────────────────────── */
void Reset_Handler(void);
void Reset_Handler(void)
{
    /* Copy initialized data from its FLASH load address into RAM. */
    uint32_t *src = &_sidata;
    uint32_t *dst = &_sdata;
    while (dst < &_edata) {
        *dst++ = *src++;
    }

    /* Zero the .bss. */
    dst = &_sbss;
    while (dst < &_ebss) {
        *dst++ = 0u;
    }

    g_virtual_clock_ms = 0u;

    (void)main();

    /* main() returns 0 on success (the driver's faithful event replay
     * completed). Signal a clean qemu exit (ApplicationExit → exit code
     * 0). The host test asserts the qemu process exited 0 AND byte-diffs
     * the captured trace — "qemu boots" is explicitly NOT acceptance
     * (Doc 32 §W2 §5.4). */
    semihost_exit_success();

    for (;;) { /* unreachable — qemu has exited */
    }
}

/* A trap handler for any unexpected exception (hard fault, etc.). On the
 * deterministic corpus this is never taken; if it ever is, exit non-zero so
 * the host test fails loudly rather than hanging. */
static void Default_Handler(void);
static void Default_Handler(void)
{
    semihost_write0("[FSM W2] unexpected exception — failing\n");
    semihost_call(SEMIHOST_SYS_EXIT, (const void *)1u);
    for (;;) {
    }
}

/* ──────────────────────────────────────────────────────────────────────
 * The Cortex-M3 vector table. The linker script KEEPs this in .isr_vector
 * at 0x00000000. The core reads [0]=initial MSP, [1]=reset entry at reset.
 * Only the architectural core exceptions are populated (the harness uses no
 * peripheral IRQs — the FSM clock is the HAL counter, not SysTick).
 * ────────────────────────────────────────────────────────────────────── */
typedef void (*vector_t)(void);

__attribute__((section(".isr_vector"), used))
const vector_t g_vector_table[] = {
    (vector_t)(&_estack), /* 0x00 Initial Stack Pointer            */
    Reset_Handler,        /* 0x04 Reset                            */
    Default_Handler,      /* 0x08 NMI                              */
    Default_Handler,      /* 0x0C HardFault                        */
    Default_Handler,      /* 0x10 MemManage                        */
    Default_Handler,      /* 0x14 BusFault                         */
    Default_Handler,      /* 0x18 UsageFault                       */
    0, 0, 0, 0,           /* 0x1C-0x28 Reserved                    */
    Default_Handler,      /* 0x2C SVCall                           */
    Default_Handler,      /* 0x30 DebugMonitor                     */
    0,                    /* 0x34 Reserved                         */
    Default_Handler,      /* 0x38 PendSV                           */
    Default_Handler,      /* 0x3C SysTick                          */
};
