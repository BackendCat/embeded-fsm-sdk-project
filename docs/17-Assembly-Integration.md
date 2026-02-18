# FSM Studio — Assembly Integration Strategy

**Document ID:** FSM-SPEC-ASSY
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-GEN-C

Defines how FSM-generated C99 runtime code integrates with assembly modules, and the
strategy for rare pure-assembly targets where no C compiler is available.

---

# 1. Scope and Recommended Path

The vast majority of embedded targets support C99:

| Target | Recommended toolchain |
|---|---|
| ARM Cortex-M0/M0+/M3/M4/M7/M33 | `arm-none-eabi-gcc -std=c99` |
| ARM Cortex-A (Linux) | `aarch64-linux-gnu-gcc -std=c99` |
| RISC-V (RV32I, RV64I) | `riscv32-unknown-elf-gcc -std=c99` |
| AVR (ATmega, ATtiny) | `avr-gcc -std=c99` |
| MSP430 | `msp430-elf-gcc -std=c99` |
| ESP32 (Xtensa) | `xtensa-esp32-elf-gcc -std=c99` |
| RP2040 | `arm-none-eabi-gcc -std=c99` |
| POSIX (Linux, macOS) | `gcc -std=c99` |

**Recommended path for all targets above:** generate C99 with `fsm generate --target c99`,
cross-compile with the target toolchain, and link the resulting `.o` files with any
assembly modules normally. No special work required.

Pure-assembly integration (this document's focus) is only needed when:
1. Calling the generated dispatch function from hand-written assembly ISRs or boot code.
2. Accessing context struct fields from assembly without going through C wrapper functions.
3. The (rare) case of a target with no C compiler at all.

---

# 2. Calling Generated C Code from Assembly

## 2.1 ARM AAPCS (Cortex-M, Cortex-A, ARM64)

ARM Architecture Procedure Call Standard (AAPCS/AAPCS64):

| Register class | Registers |
|---|---|
| Arguments (1st–4th word args) | `r0`–`r3` |
| Return value | `r0` (or `r0:r1` for 64-bit) |
| Caller-saved (may be clobbered) | `r0`–`r3`, `r12`, `lr` |
| Callee-saved (preserved by callee) | `r4`–`r11`, `sp` |

`Motor_dispatch(Motor_t *m, const Motor_Event_t *ev)`:
- `r0` = pointer to `Motor_t`
- `r1` = pointer to `const Motor_Event_t`

```asm
    @ ARM Thumb (Cortex-M) — call Motor_dispatch from assembly
    .syntax unified
    .thumb

    LDR   r0, =motor_instance     @ Motor_t* (address of the context struct)
    LDR   r1, =event_buffer       @ const Motor_Event_t* (address of event)
    BL    Motor_dispatch           @ call; r0-r3 may be clobbered after return
```

`Motor_post(Motor_t *m, const Motor_Event_t *ev)` — same calling convention:

```asm
    @ Posting an event from a SysTick ISR (Cortex-M)
    .thumb_func
SysTick_Handler:
    PUSH  {r0-r3, lr}
    LDR   r0, =motor_instance
    LDR   r1, =systick_timer_event
    BL    Motor_post
    POP   {r0-r3, pc}
```

`Motor_tick(Motor_t *m, uint32_t elapsed_ms)`:
- `r0` = pointer to `Motor_t`
- `r1` = elapsed milliseconds (uint32_t)

```asm
    LDR   r0, =motor_instance
    MOV   r1, #10              @ 10ms elapsed
    BL    Motor_tick
```

---

## 2.2 AVR (avr-gcc Convention)

AVR is an 8-bit architecture with 16-bit pointers. The avr-gcc calling convention:

| Purpose | Registers |
|---|---|
| Arguments (1st 16-bit arg) | `r24:r25` (lo:hi) |
| Arguments (2nd 16-bit arg) | `r22:r23` |
| Arguments (3rd 16-bit arg) | `r20:r21` |
| Return value (16-bit) | `r24:r25` |
| Caller-saved | `r18`–`r27`, `r30`, `r31` |
| Callee-saved | `r2`–`r17`, `r28`, `r29` |

Pointers are 16-bit on classic AVR (SRAM ≤ 64KB):

```asm
    ; AVR — call Motor_dispatch
    LDI   r24, lo8(motor_instance)
    LDI   r25, hi8(motor_instance)    ; r24:r25 = Motor_t*
    LDI   r22, lo8(event_buffer)
    LDI   r23, hi8(event_buffer)      ; r22:r23 = Motor_Event_t*
    CALL  Motor_dispatch
```

---

## 2.3 RISC-V (RV32I and RV64I)

RISC-V calling convention (RISC-V ELF psABI):

| Register | ABI name | Purpose |
|---|---|---|
| `x10`–`x17` | `a0`–`a7` | Arguments and return values |
| `x1` | `ra` | Return address (caller-saved) |
| `x5`–`x7`, `x28`–`x31` | `t0`–`t6` | Temporaries (caller-saved) |
| `x8`–`x9`, `x18`–`x27` | `s0`–`s11` | Saved registers (callee-saved) |

```asm
    # RISC-V (RV32I) — call Motor_dispatch
    la    a0, motor_instance   # Motor_t* in a0
    la    a1, event_buffer     # Motor_Event_t* in a1
    call  Motor_dispatch       # pseudo-instruction: auipc+jalr
```

---

## 2.4 MSP430

MSP430 16-bit architecture (TI ultra-low-power MCUs):

| Register | Purpose |
|---|---|
| `r15` | 1st argument / return value |
| `r14` | 2nd argument |
| `r13` | 3rd argument |
| `r12` | 4th argument |
| `r4`–`r10` | Callee-saved |
| `r11`–`r15` | Caller-saved |

```asm
    ; MSP430 — call Motor_dispatch
    MOV.W #motor_instance, r15   ; Motor_t*
    MOV.W #event_buffer,   r14   ; Motor_Event_t*
    CALL  #Motor_dispatch
```

---

# 3. Context Struct Layout from Assembly

To access context fields from assembly without going through C functions, you need the
byte offsets of each field. These are platform-dependent due to alignment padding.

## 3.1 Offset Header Generation

When `FSM_MOTOR_GENERATE_ASM_OFFSETS` is defined in `Motor_conf.h`, the compiler
generates an additional file `Motor_asm_offsets.h` containing the exact offsets for
the current platform:

```c
/* Motor_asm_offsets.h — generated by running Motor_offsets_gen.c */
/* DO NOT EDIT — regenerate after changing target platform or compiler flags */

#define MOTOR_CTX_OFFSET_SPEED             0
#define MOTOR_CTX_OFFSET_RUNNING           2
#define MOTOR_CTX_OFFSET__STATE            4
#define MOTOR_CTX_OFFSET__HISTORY_MAIN     5
#define MOTOR_CTX_OFFSET__JOIN_BITS        6
#define MOTOR_CTX_OFFSET__TIMER_AFTERIDLE  8
#define MOTOR_CTX_SIZEOF                   16
```

The offset generator source (`Motor_offsets_gen.c`) is also generated:

```c
/* Motor_offsets_gen.c — compile and run on the target platform to get offsets */
#include <stdio.h>
#include <stddef.h>
#include "Motor.h"

int main(void) {
    printf("#define MOTOR_CTX_OFFSET_SPEED            %zu\n",
           offsetof(Motor_t, speed));
    printf("#define MOTOR_CTX_OFFSET_RUNNING          %zu\n",
           offsetof(Motor_t, running));
    printf("#define MOTOR_CTX_OFFSET__STATE           %zu\n",
           offsetof(Motor_t, _state));
    printf("#define MOTOR_CTX_SIZEOF                  %zu\n",
           sizeof(Motor_t));
    return 0;
}
```

## 3.2 Reading a Field from Assembly (ARM Cortex-M)

```asm
    .include "Motor_asm_offsets.h"   @ include generated offset header

    LDR   r0, =motor_instance
    LDRH  r1, [r0, #MOTOR_CTX_OFFSET_SPEED]   @ read ctx.speed (uint16_t)
    LDRB  r2, [r0, #MOTOR_CTX_OFFSET__STATE]  @ read ctx._state (uint8_t enum)
```

---

# 4. Jump Table Strategy (Pure Assembly — No C Compiler)

This section covers the rare case of a target with no C compiler. The recommended
path remains: use C99 codegen + cross-compile. Only proceed with pure assembly if
you have verified that no C toolchain exists for your target.

## 4.1 State Dispatch as Jump Table (ARM Thumb)

```asm
    .syntax unified
    .thumb

Motor_dispatch_asm:
    @ r0 = Motor_t* (context pointer)
    @ r1 = event ID (Motor_EventId_t)

    @ Load current state from context struct
    LDRB  r2, [r0, #MOTOR_CTX_OFFSET__STATE]

    @ Branch via table indexed by state ID
    TBB   [pc, r2]

state_dispatch_table:
    .byte ((dispatch_Idle    - state_dispatch_table) / 2)
    .byte ((dispatch_Running - state_dispatch_table) / 2)
    .byte ((dispatch_Error   - state_dispatch_table) / 2)

dispatch_Idle:
    CMP   r1, #0          @ MOTOR_EVENT_START = 0
    BEQ   transition_Idle_to_Running
    BX    lr              @ event not handled in Idle

dispatch_Running:
    CMP   r1, #1          @ MOTOR_EVENT_STOP = 1
    BEQ   transition_Running_to_Idle
    CMP   r1, #2          @ MOTOR_EVENT_FAULT = 2
    BEQ   transition_Running_to_Error
    BX    lr

dispatch_Error:
    BX    lr              @ no transitions from Error

@ ── Transitions ─────────────────────────────────────────────────────────────

transition_Idle_to_Running:
    @ Exit Idle
    PUSH  {r0-r3, lr}
    BL    Motor_exit_Idle_asm
    POP   {r0-r3, lr}

    @ Update state
    MOV   r2, #1          @ MOTOR_STATE_RUNNING = 1
    STRB  r2, [r0, #MOTOR_CTX_OFFSET__STATE]

    @ Enter Running
    PUSH  {r0-r3, lr}
    BL    Motor_entry_Running_asm
    POP   {r0-r3, lr}
    BX    lr
```

## 4.2 Entry/Exit as Assembly Labels

```asm
    .global Motor_entry_Idle_asm
Motor_entry_Idle_asm:
    @ User-written entry code for Idle state
    @ r0 = Motor_t* (context)
    @ No return value
    PUSH  {lr}
    @ ... user code ...
    POP   {pc}

    .global Motor_exit_Idle_asm
Motor_exit_Idle_asm:
    PUSH  {lr}
    @ ... user code ...
    POP   {pc}
```

In pure assembly, the developer is responsible for calling entry/exit in the correct
LCA order. The compiler cannot help. Thoroughly review FSM-SPEC-SEM §6–§7 before
implementing.

---

# 5. Linker Script Considerations

Generated C tables (transition table in TABLE strategy) are placed in `.rodata`.
When linking C objects with assembly in a custom memory map:

```ld
/* Example linker script excerpt for STM32 Cortex-M4 */
SECTIONS {
    .text : {
        *(.text*)
        *Motor.o(.text)         /* FSM dispatch function */
    } > FLASH

    .rodata : {
        *(.rodata*)
        *Motor.o(.rodata)       /* FSM transition table */
        . = ALIGN(4);
    } > FLASH

    .data : {
        motor_instance = .;
        . += MOTOR_CTX_SIZEOF;  /* Reserve space for context struct in RAM */
        *(.data*)
    } > RAM AT > FLASH
}
```

If the context struct is in BSS (zero-initialized), `Motor_init()` will overwrite the
zero-initialization with correct defaults anyway. Either approach is valid.

---

# 6. No-C Future Profile

A future `target ASM` profile in FSM-Lang (out of scope for v1.0) would emit:
- GAS-compatible `.s` files (GNU Assembler syntax)
- One file per machine: `Motor_asm.s`
- Switch-based strategy only (no C struct for table-driven)
- Symbolic `EQU` definitions for state IDs and event IDs
- Entry/exit label stubs for user implementation

This profile is low priority. Estimated effort: 4× the C99 generator. File a GitHub
issue to discuss requirements before implementation begins.

---

# 7. Practical Decision Guide

```
Does your target have a C99 compiler?
├── YES → Use fsm generate --target c99 and cross-compile. Done.
│         This covers: Cortex-M, RISC-V, AVR, MSP430, ESP32, RP2040, POSIX.
│
└── NO  → You have a pure-assembly-only target (very rare).
          │
          ├── Can you call C functions from assembly? (answer: almost always yes)
          │   └── YES → Use C99 target, write a minimal assembly stub that sets up
          │             r0/r1 and calls Motor_dispatch. Link C and assembly together.
          │             This is the simplest path.
          │
          └── No C calling convention at all (exotic DSP, custom ISA)
              └── Implement your own state machine manually using §4 as a guide.
                  The FSM-Lang compiler cannot help here in v1.0.
                  File a GitHub issue with your target details.
```

---

*End of FSM-SPEC-ASSY v1.0.0*
