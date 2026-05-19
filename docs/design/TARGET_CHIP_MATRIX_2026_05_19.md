# FSM Studio — Target-MCU Matrix & Current-Support Delta

**Document ID:** FSM-DESIGN-CHIPMATRIX
**Date:** 2026-05-19
**Status:** Design research (non-normative). Feeds (1) the external-signal/ISR
design thread (Part 1 §"Interrupt model" column is the load-bearing input)
and (2) the debug-interface design's target-awareness.
**Branch:** `phase8.0/target-chip-matrix` · base main `3e4ec53`
**Author role:** senior embedded-systems engineer (research + synthesis)

> **What this document is.** The authoritative catalogue of target-MCU
> families a deterministic-heap-free-C99 FSM runtime + a mandatory HAL must
> be aware of, plus a brutally honest statement of which rows FSM Studio
> *actually* builds/tests against today versus only assumes. It does NOT
> propose codegen changes; it is the substrate two parallel design threads
> consume.
>
> **Relationship to existing docs.** `docs/17-Assembly-Integration.md`
> already owns the per-arch **calling-convention / ABI register tables**
> (AAPCS, AVR avr-gcc, RISC-V psABI, MSP430). This document deliberately
> does **not** duplicate those; it cross-references Doc 17 and fills the
> gaps Doc 17 does not cover: **memory sizes, integer/pointer model,
> endianness, the interrupt model (ISR declaration syntax + vector table +
> priority + clock seam), and the per-family hard constraints a
> deterministic heap-free C99 generator must respect.**

---

# Part 1 — The Target-MCU Matrix

## 1.1 Compact summary table

Sizes are *typical commodity-part* ranges (engineering rule-of-thumb for
the parts a deterministic-firmware shop actually ships), not the absolute
min/max of every SKU ever made. "ISR decl in C" is the column the
external-signal/ISR thread needs.

| Family | FLASH (typ.) | RAM/SRAM (typ.) | `int` / ptr / `size_t` | Endian | Interrupt controller | ISR decl in C | Clock seam | C toolchain / ABI |
|---|---|---|---|---|---|---|---|---|
| **ARM Cortex-M0 / M0+** | 16 KB – 256 KB | 2 KB – 32 KB | 32 / 32 / 32-bit | little | NVIC, relocatable vector table (M0+ has VTOR; M0 fixed @0x0), up to 32 IRQ, 4 priority levels (2 bits) | Named weak symbol in the vector table (e.g. `void SysTick_Handler(void)`); CMSIS startup provides `__attribute__((weak))` defaults | SysTick 24-bit down-counter (`SysTick_Config()`); CMSIS `SystemCoreClock` | `arm-none-eabi-gcc -std=c99 -mcpu=cortex-m0plus -mthumb`; AAPCS (Doc 17 §2.1) |
| **ARM Cortex-M3** | 32 KB – 512 KB | 8 KB – 96 KB | 32 / 32 / 32-bit | little (BE8 selectable, rare) | NVIC + VTOR, up to 240 IRQ, 8–256 priority levels (3–8 bits), tail-chaining | Named weak vector symbol; CMSIS startup | SysTick (24-bit); `DWT->CYCCNT` cycle counter available | `arm-none-eabi-gcc -mcpu=cortex-m3 -mthumb`; AAPCS |
| **ARM Cortex-M4 (/F)** | 64 KB – 2 MB | 16 KB – 256 KB | 32 / 32 / 32-bit | little | NVIC + VTOR (as M3); optional FPU (M4F, single-precision) | Named weak vector symbol; CMSIS | SysTick + DWT cycle counter | `arm-none-eabi-gcc -mcpu=cortex-m4 [-mfpu=fpv4-sp-d16 -mfloat-abi=hard]`; AAPCS |
| **ARM Cortex-M7 (/F)** | 256 KB – 2 MB+ | 128 KB – 1 MB (+ TCM, caches) | 32 / 32 / 32-bit | little | NVIC + VTOR; superscalar; optional double-precision FPU | Named weak vector symbol; CMSIS | SysTick + DWT; cache-coherency caveats for DMA, not for the FSM clock | `arm-none-eabi-gcc -mcpu=cortex-m7 [-mfpu=fpv5-d16]`; AAPCS |
| **ARM Cortex-M33** | 128 KB – 2 MB | 32 KB – 512 KB | 32 / 32 / 32-bit | little | NVIC + VTOR; **TrustZone-M** (secure/non-secure vector tables `VTOR`/`VTOR_NS`), SAU; optional FPU/DSP | Named weak vector symbol; secure & non-secure tables are separate; CMSIS startup per security state | SysTick (one per security state) | `arm-none-eabi-gcc -mcpu=cortex-m33 [-mcmse for secure]`; AAPCS |
| **AVR 8-bit (ATmega / ATtiny)** | 1 KB – 256 KB (Harvard, separate program space) | 32 B – 16 KB | 16 / 16 (≤64 KB; some ≥128 KB use 24-bit `__memx`) / 16-bit | little | Fixed vector table at flash 0x0000 (or boot section), 1 priority level, global `I` flag (`SREG`); nested IRQ only by manual `sei()` | `ISR(VECT_vect)` macro from `<avr/interrupt.h>` (compiler emits prologue/epilogue + `reti`); names are fixed per device | No SysTick. A timer/counter (e.g. Timer0 CTC) ISR increments a `volatile` ms counter; Arduino core supplies `millis()` | `avr-gcc -std=c99 -mmcu=atmega328p`; avr-gcc convention (Doc 17 §2.2) |
| **RISC-V RV32 (GD32VF103, ESP32-C3, CH32V, …)** | 16 KB – 4 MB | 8 KB – 400 KB | 32 / 32 / 32-bit | little | **CLINT** (timer+software IRQ) + **PLIC** (external, M-mode) — or vendor **CLIC**/ECLIC (GD32VF103 Bumblebee, ESP32-C3); `mtvec` base+mode (direct/vectored), `mcause`, `mie`/`mip` | `__attribute__((interrupt))` on the handler (GCC emits `mret` + CSR save/restore); vendor (Nuclei/ESP-IDF) HALs wrap registration | `mtime`/`mtimecmp` (CLINT 64-bit machine timer) is the canonical tick; vendor HAL `systimer` on ESP32-C3 | `riscv64-unknown-elf-gcc` / `riscv32-unknown-elf-gcc -march=rv32imac -mabi=ilp32`; RISC-V ELF psABI (Doc 17 §2.3) |
| **MSP430 (16-bit, TI MSP430x / FRAM MSP430FR)** | 1 KB – 256 KB (FRAM parts unify code/data) | 128 B – 66 KB | 16 / 16 (20-bit on 430X large-memory model) / 16-bit | little | ITC: fixed prioritized vector table at top of memory (0xFFFF down); priority is by vector address (highest address = highest priority); global `GIE` in `SR` | `__attribute__((interrupt(VECTOR)))` (msp430-elf-gcc) or `#pragma vector=` + `__interrupt` (TI CCS); fixed vector names | No SysTick. A Timer_A/Timer_B CCR ISR drives the ms counter; low-power modes gate the clock | `msp430-elf-gcc -std=c99 -mmcu=msp430g2553`; MSP430 EABI (Doc 17 §2.4) |
| **ESP32 Xtensa LX6 (ESP32) / LX7 (ESP32-S2/S3)** | 0 (no internal flash — boots from external SPI flash, typ. 2–16 MB) | 200 KB – 512 KB internal SRAM (+ optional PSRAM) | 32 / 32 / 32-bit | little | Xtensa interrupt model: per-core programmable interrupt matrix routes peripheral sources to ~32 CPU interrupt lines/levels; controlled via ESP-IDF (`esp_intr_alloc`) — **not** a bare vector table you hand-edit | ESP-IDF `esp_intr_alloc(source, flags, handler, arg, &h)` registers a C handler at runtime; `xtensa-esp32-elf-gcc`; FreeRTOS-first | ESP-IDF `esp_timer` / FreeRTOS `xTaskGetTickCount()`; `esp_timer_get_time()` (64-bit µs) | `xtensa-esp32-elf-gcc -std=c99` (ESP-IDF, FreeRTOS); Xtensa windowed-ABI |
| **STM8 (8-bit, STM8S/L/AF/AL)** | 2 KB – 128 KB | 1 KB – 6 KB | **16 / 16 / 16-bit** (SDCC: `int`=16-bit) | **big-endian** (note: opposite of every other row) | ITC: fixed vector table at 0x008000 (32 entries), 4 software priority levels per IRQ, global `I1:I0` in CC | SDCC: `void irq_handler(void) __interrupt(N)`; Cosmic/IAR use `@interrupt`/`#pragma`; fixed vector slot per IRQ number | No SysTick. A TIMx update ISR increments the ms counter | **`sdcc -mstm8 --std-c99`** (NOT gcc — no GCC backend for STM8); SDCC ABI |

### Notes embedded in the table

- **"typical" sizes are conservative engineering ranges.** A few outliers
  exceed them (Cortex-M7 with 3 MB flash; ATmega2560 with 256 KB flash via
  banked far calls; MSP430FR5994 with 256 KB FRAM). Treat the table as "the
  envelope codegen/HAL must not assume to be larger than", not a hard spec.
  *(Disclosed assumption — see §3.)*
- **STM8 is the endianness odd-one-out** (big-endian) and the only family
  with **no GCC backend** (SDCC only). Any byte-order-sensitive codegen or
  debug-interface wire format must treat STM8 specially. The FSM runtime
  itself is endian-agnostic *as long as* it only uses `<stdint.h>` integer
  semantics and never `memcpy`/`union`-type-puns multi-byte fields across a
  serialization boundary (it currently does not — see Part 2).
- **AVR is the only Harvard-architecture row** in scope: code and data live
  in *separate* address spaces. String/const lookup tables that the codegen
  places in `.rodata` are, on AVR, in *flash* and require `pgm_read_*` /
  `__flash` / `PROGMEM` to read — a normal `const char *` deref reads
  *RAM*. This is the single sharpest portability constraint for any
  `.rodata` transition table or string-name array (see §2.3 / Part 2 delta).

## 1.2 Per-family interrupt model (the column the ISR thread needs)

This is the precise input the external-signal/ISR design thread requested.
For each family: *how an ISR is declared/registered in C*, *how the FSM
clock seam is wired*, and *the ISR↔`Motor_post()` critical-section
mechanism* (the FSM runtime's only ISR contact point — Doc 16 §5).

### Cortex-M (M0/M0+/M3/M4/M7/M33) — NVIC

- **Controller:** NVIC. Vector table is an array of function pointers; entry
  0 = initial MSP, entry 1 = `Reset_Handler`, entries 2–15 = architectural
  core exceptions (NMI, HardFault, …, PendSV, SysTick), 16+ = device IRQs.
- **ISR declaration in C:** a function whose **name matches the vector-table
  slot**. CMSIS device startup declares each as
  `void TIMx_IRQHandler(void) __attribute__((weak, alias("Default_Handler")))`;
  the application provides a strong definition with the *same name* to
  override it. **No special return attribute is needed** — Cortex-M does
  hardware stacking/unstacking; a plain C function + `BX LR` is a correct
  exception return (the EXC_RETURN magic is in `LR`).
  This exact weak/strong-vector pattern is what the project's own QEMU
  harness uses — `crates/fsm-simulator/tests/on-target/startup_mps2_an385.c:164-181`
  builds the M3 vector table as `const vector_t g_vector_table[]` and
  `mps2_an385.ld:44-47` `KEEP`s it in `.isr_vector` at `0x00000000`.
- **Priority:** N implementation-defined bits (M0/M0+: 2 bits → 4 levels;
  M3+: typically 3–4 bits → 8–16 levels). Lower number = higher priority.
  `BASEPRI` (M3+) masks by priority; `PRIMASK` masks all.
- **Clock seam:** SysTick 24-bit down-counter. `SysTick_Config(SystemCoreClock/1000u)`
  → 1 kHz; `SysTick_Handler` increments a `volatile uint32_t` the HAL
  returns. *(Doc 16 §6.1 ships exactly this reference.)* `DWT->CYCCNT`
  (M3+) is the high-resolution alternative.
- **`Motor_post()` critical section:** `__get_PRIMASK()`/`__disable_irq()`
  … `__set_PRIMASK()` (Doc 16 §6.1). Save-and-restore (not unconditional
  enable) so it nests safely.

### AVR (ATmega/ATtiny) — fixed vector table + global I flag

- **Controller:** a fixed prioritized vector table at flash address
  `0x0000` (or the boot section if BOOTRST set). Priority is *fixed by
  vector position* — no programmable priorities. One global interrupt
  enable: the `I` bit in `SREG`, set by `sei()`, cleared by `cli()`.
  An ISR runs with `I` cleared (no nesting unless the ISR itself `sei()`s).
- **ISR declaration in C:** the **`ISR(vectname_vect)` macro** from
  `<avr/interrupt.h>`, e.g. `ISR(TIMER0_COMPA_vect) { … }`. avr-gcc emits
  the register-save prologue, the body, the register-restore epilogue, and
  a `reti` (which re-enables `I`). Vector names are *fixed per device* (in
  `<avr/iom328p.h>` etc.). `ISR(BADISR_vect)` catches unhandled vectors.
- **Clock seam:** **no SysTick.** A timer/counter compare-match ISR (e.g.
  `TIMER0_COMPA_vect` in CTC mode at 1 kHz) increments a
  `volatile uint32_t` the HAL returns; on Arduino, the core already does
  this and the HAL is one line: `return millis();` (Doc 16 §6.5).
- **`Motor_post()` critical section:** `uint8_t s = SREG; cli();` …
  `SREG = s;` (Doc 16 §6.5 Arduino-AVR branch). Saving/restoring `SREG`
  preserves the prior `I` state so it nests.
- **Hard Harvard caveat for the ISR thread:** an event object posted from
  an ISR lives in RAM (fine); but any *string/const table* the dispatch
  touches is in flash on AVR and is **not** dereferenceable as a plain
  pointer — see §2.3.

### RISC-V RV32 (CLINT/PLIC or vendor CLIC/ECLIC)

- **Controller:** base spec = **CLINT** (machine timer `mtime`/`mtimecmp`
  + software interrupt) and **PLIC** (external-interrupt arbitration,
  M-mode). `mtvec` holds the trap-vector base + mode (0 = direct: all
  traps → one handler that decodes `mcause`; 1 = vectored: `base + 4×cause`).
  Vendor cores layer a **CLIC/ECLIC** (Nuclei Bumblebee in GD32VF103;
  ESP32-C3 has its own interrupt matrix) giving NVIC-like per-source
  vectoring + priority.
- **ISR declaration in C:** `__attribute__((interrupt))` on the handler
  (GCC emits the full CSR/GPR save-restore and `mret` instead of `ret`).
  `__attribute__((interrupt("machine")))` selects the privilege mode.
  Vendor HALs (Nuclei SDK, ESP-IDF) provide registration wrappers; on
  ESP32-C3 you typically use the ESP-IDF `esp_intr_alloc` path rather than
  hand-writing `mtvec`.
- **Clock seam:** the CLINT 64-bit `mtime` machine-timer is the canonical
  monotonic source; `mtimecmp` fires the timer IRQ. Vendor HAL exposes a
  ms/µs API the FSM HAL wraps. ESP32-C3 (a RISC-V part, ESP-IDF-first):
  use `esp_timer_get_time()` like the Xtensa ESP32.
- **`Motor_post()` critical section:** clear `mie.MEIE`/global `mstatus.MIE`
  (CSR read-modify-write) or use the vendor HAL critical-section macro
  (Nuclei `__disable_irq()` / ESP-IDF `portENTER_CRITICAL`).

### MSP430 — ITC, address-priority vector table

- **Controller:** fixed vector table at the *top* of memory (0xFFFF
  downward); the vector's *address* encodes priority (highest address =
  highest priority). Global enable = `GIE` in the status register `SR`.
- **ISR declaration in C:** `__attribute__((interrupt(TIMERA0_VECTOR)))`
  with msp430-elf-gcc, or `#pragma vector=TIMERA0_VECTOR` + `__interrupt
  void …` with TI CCS. The compiler emits the `reti`. Some shared vectors
  (e.g. TAIV) require reading the interrupt-vector register to demux.
- **Clock seam:** no SysTick. A Timer_A/Timer_B CCR0 ISR increments the
  ms counter the HAL returns. Caveat: MSP430 low-power modes (`LPMx`)
  *stop* clocks; the ms tick must run off a clock that survives the LPM
  level the application uses, or the FSM's `after X ms` timers stall.
- **`Motor_post()` critical section:** save `SR`, `__disable_interrupt()`
  (`__dint()`), restore `SR` (`__bis_SR_register`) — or
  `__bic_SR_register(GIE)` / restore.

### ESP32 Xtensa LX6/LX7 — interrupt matrix (ESP-IDF runtime registration)

- **Controller:** *not* a hand-edited vector table. An interrupt matrix
  routes ~100 peripheral sources onto ~32 per-core CPU interrupt
  lines/levels. ESP-IDF owns this.
- **ISR declaration/registration in C:** runtime —
  `esp_intr_alloc(ETS_xxx_INTR_SOURCE, flags, handler, arg, &handle)`.
  The handler is a normal C function; ESP-IDF + the FreeRTOS port supply
  the low-level vectoring. ISRs that touch FreeRTOS objects must use the
  `…FromISR` APIs and live in IRAM (`IRAM_ATTR`) if used with cache
  disabled.
- **Clock seam:** FreeRTOS-first. `esp_timer_get_time()` (64-bit µs since
  boot) or `xTaskGetTickCount()`; the FSM HAL is the FreeRTOS reference
  (Doc 16 §6.2) or an `esp_timer` wrapper.
- **`Motor_post()` critical section:** ESP-IDF `portENTER_CRITICAL(&mux)` /
  `portENTER_CRITICAL_ISR(&mux)` (a spinlock — dual-core), **not** a bare
  PRIMASK-style disable. This is a real divergence from the bare-metal
  Cortex-M pattern the ISR thread must account for.

### STM8 — ITC, fixed vector table, big-endian

- **Controller:** fixed 32-entry vector table at `0x008000`. Each IRQ has
  a 2-bit software priority (4 levels) in the ITC `SPRx` registers. Global
  mask = `I1:I0` in the condition-code register.
- **ISR declaration in C:** SDCC —
  `void TIM1_UPD_IRQHandler(void) __interrupt(11)` (the number is the IRQ
  slot). Cosmic/IAR use `@far @interrupt` / `#pragma`. SDCC emits `iret`.
- **Clock seam:** no SysTick. A TIMx update ISR increments the ms counter.
- **`Motor_post()` critical section:** `sim()` (set interrupt mask) …
  `rim()` (reset) — SDCC intrinsics — or save/restore CC.
- **Byte-order caveat:** big-endian. The FSM runtime is endian-neutral
  *only because* it does not serialize multi-byte fields across a byte
  boundary internally; the *debug interface* design (which will put FSM
  state on a wire) MUST define an explicit wire byte order and not assume
  target-native — STM8 is the proof that "target-native" is not uniform.

## 1.3 Per-family hard constraints a deterministic-heap-free-C99 generator + mandatory HAL must respect

These are the constraints that bite codegen/HAL specifically (beyond the
universal "no `malloc`/`free`, bounded stack, no recursion, no VLA, no
`<stdio.h>` on freestanding" rules the project already enforces — Doc 11
§1/§19, Doc 16 §8).

| Constraint | Families affected | Why it bites a deterministic FSM generator |
|---|---|---|
| **No 64-bit hardware divide / expensive 64-bit math** | Cortex-M0/M0+ (no divide instr at all — even 32-bit `/` is a libgcc call), AVR, MSP430, STM8 | Timer math (`after X ms`, wrap-safe subtraction) MUST stay in `uint32_t`. A user `u64`/`i64` context field (`expr.rs:116-120` maps `u64`→`uint64_t`) pulls in libgcc `__udivdi3` — large code, slow, but *correct & deterministic*. A `f32`/`f64` field (`expr.rs:121-122`) pulls soft-float on every M0/AVR/MSP430/STM8. Codegen does not forbid these; the constraint is "the generator must keep its *own* runtime arithmetic ≤32-bit", which it does (Doc 11 timers are `uint32_t` — Doc 11:191 `MOTOR_TIMER_TYPE uint32_t`). |
| **Harvard: `.rodata` is in a separate (flash) space** | AVR (and AVR only, in scope) | The TABLE dispatch strategy stores a `static const Motor_trans_table[]` in `.rodata` (Doc 11 §9). On AVR a plain pointer deref of `.rodata` reads *RAM*, not flash → wrong data. Correct AVR codegen needs `__flash`/`PROGMEM` + `pgm_read_*`, **which the current family-agnostic codegen does not emit** (see Part 2). The SWITCH strategy (no stored table) sidesteps this; large machines that auto-select TABLE (>64 states, `config.rs:97-108`) are the exposed case. |
| **Tiny RAM floor** | ATtiny (32 B–~2 KB), MSP430G2xx (128 B–512 B), STM8S003 (1 KB) | The context struct + event queue must fit. Doc 16 §8 quotes 8–512 B RAM / 64–512 B stack. On a 128-byte-SRAM ATtiny, an 8-deep event queue of a multi-field event is already over budget. Constraint: `queue_capacity` (default 8, `config.rs:50`) and event size are *not* auto-scaled to target RAM — the integrator must size them; codegen cannot know the target's SRAM. |
| **No relocatable vector table (raw M0, classic 8-bit/16-bit)** | Cortex-M0 (no VTOR), AVR, MSP430, STM8 | Irrelevant to the FSM runtime itself (it never installs vectors) but load-bearing for the ISR thread: on these parts the ISR↔FSM glue must live in the *one fixed* vector slot; you cannot runtime-register like ESP-IDF/CLIC. |
| **Big-endian** | STM8 only | The runtime is endian-neutral today (no internal multi-byte serialization). Becomes load-bearing the moment the **debug interface** puts FSM state on a wire — that design must pin a wire byte order. Flag carried forward to the debug-interface thread. |
| **No GCC backend** | STM8 (SDCC only) | The project's entire build/test/CI story is GCC-family (`arm-none-eabi-gcc`, `avr-gcc`, `riscv*-gcc`, host `gcc`). STM8 needs `sdcc`. SDCC's C99 conformance and `__attribute__` support differ from GCC; the generated C must be SDCC-clean to claim STM8 — **untested today** (Part 2). |
| **`int` is 16-bit** | AVR, MSP430, STM8 | Integer-promotion math on `int` wraps at 16 bits, not 32. The generated runtime must not assume `int` ≥ 32 bits. The codegen uses **explicit `<stdint.h>` fixed-width types throughout** (`uint8_t`/`uint16_t`/`uint32_t` — Doc 11 §3, verified `crates/fsm-codegen-c/src/emit/hal.rs:33` `#include <stdint.h>`), and event/state index widths are sized by count (Doc 11:834). This constraint is *currently respected by construction* — it is the single biggest reason the family-agnostic approach works. |
| **FreeRTOS/RTOS-mediated interrupts, dual-core** | ESP32 Xtensa LX6 (dual-core), ESP32-C3 (single but ESP-IDF) | `Motor_post()` from an ISR needs a spinlock-based critical section (`portENTER_CRITICAL_ISR`), not PRIMASK. Doc 16 §6.2 ships the FreeRTOS reference; the ESP-IDF specifics (IRAM_ATTR, `…FromISR`) are an integrator responsibility the HAL spec does not spell out per-chip. |

---

# Part 2 — What FSM Studio Currently Assumes / Targets (verified, brutally honest)

## 2.1 What the HAL seam bakes in (Doc 16 + the emitted header)

**Read:** `docs/16-HAL-Specification.md` and the actual emitter
`crates/fsm-codegen-c/src/emit/hal.rs`.

- **Freestanding, not hosted (correct).** `TargetProfile::Embedded` is the
  default (`crates/fsm-codegen-c/src/config.rs:55`), documented as "assumes
  a freestanding C99 environment" (`config.rs:152-153`). The mandatory-HAL
  design means the runtime never calls libc directly — clock + assert are
  user-provided. This is sound for every Part-1 family.
- **Integer model: fixed-width `<stdint.h>` only (correct & the key win).**
  The emitted `fsm_hal.h` is `#include <stdint.h>` +
  `uint32_t fsm_hal_clock_now_ms(void)` (`hal.rs:33,51`). The codegen
  emits `uint8_t`/`uint16_t`/`uint32_t` throughout (Doc 11 §3); no naked
  `int`/`long` for sized fields. **This is precisely why the
  family-agnostic approach is defensible across 8/16/32-bit** — it never
  assumes `sizeof(int)`.
- **Clock type: `uint32_t` milliseconds, wrap-safe (correct).** Doc 16 §3
  + `hal.rs:51`. 32-bit wrap @ ~49.7 days handled by unsigned subtraction;
  timer field is `uint32_t` (Doc 11:191). No 64-bit math forced by the
  runtime → safe on M0/AVR/MSP430/STM8.
- **`bool` from `<stdbool.h>` (`hal.rs:34`)** — C99, universally fine.
- **⚠ Documentation drift (real finding, must be flagged).** Doc 16 §2/§3
  specifies the contract symbols as `uint32_t fsm_hal_clock_ms(void)` and
  `void fsm_hal_assert_fail(const char *file, int line, const char *msg)`
  (`docs/16-HAL-Specification.md:53-54, 73, 111`). The **code actually
  emits different symbols**: `uint32_t fsm_hal_clock_now_ms(void)` and
  `void fsm_hal_assert(bool cond, const char *msg)`
  (`crates/fsm-codegen-c/src/emit/hal.rs:51-52`; the unit test at
  `hal.rs:129-132` pins the *as-built* names). Every shipped integration
  example and the on-target harness use the **code** spelling
  (`examples/integration/make/hal.c:28,35`;
  `crates/fsm-simulator/tests/on-target/startup_mps2_an385.c:72-76`). So
  the *code is internally consistent*; **Doc 16 §2–§6 is stale** (its
  `file,line,msg` assert signature and `_ms`/`_fail` names predate the
  current `now_ms`/`bool cond` contract). This is a Zero-Legacy doc-vs-code
  reconciliation item — out of scope to fix here, but recorded so neither
  the ISR thread nor the debug-interface thread codes against the stale
  Doc 16 signature.

## 2.2 What the project ACTUALLY builds/tests against today

**Inspected:** `examples/integration/{make,cmake,cargo-rust,platformio}/`,
`crates/fsm-simulator/tests/on-target/`, `crates/fsm-simulator/tests/on_target_qemu_differential.rs`,
`crates/fsm-cli/tests/integration_examples.rs`, `.github/workflows/ci.yml`.

| Surface | What it builds | Target reality |
|---|---|---|
| `examples/integration/make`, `cmake`, `cargo-rust` | `fsm generate --target c99` → compile with **host `gcc`/`cc` `-std=c99`** | **Host (x86-64 Linux) only.** No cross-compilation. Proves the generated C is ISO-C99-clean & links, on the dev/CI host. (`Makefile:17`, `CMakeLists.txt:19-21`, `build.rs:58-64`.) |
| `examples/integration/platformio` (`env:uno`, `board=uno`, `platform=atmelavr`, ATmega328P) | *Would* build real AVR firmware with `avr-gcc` via `pio run` | **`pio`-gated, and `pio` is NOT installed in CI.** `crates/fsm-cli/tests/integration_examples.rs:12-22,208-242`: "Only `pio` may be absent → loud, documented **gcc-equivalent** fallback." CI installs `make/cmake/cargo/gcc/arm-none-eabi-gcc/qemu-system-arm` only (`ci.yml`), **not** `pio`. ⇒ **The AVR/ATmega328P path is in practice compiled with host gcc, not avr-gcc, in automated CI.** Real `avr-gcc` only if a developer manually has `pio` locally. |
| `crates/fsm-simulator/tests/on-target/` + `on_target_qemu_differential` | Cross-compile the codegen's `FSM_TRACE` C with **`arm-none-eabi-gcc -mcpu=cortex-m3 -mthumb`**, link the MPS2-AN385 harness, run under **`qemu-system-arm -M mps2-an385 -cpu cortex-m3`**, byte-diff the trace vs the shipped `fsm_simulator::execute_trace` oracle | **This is the ONE genuine on-target lane.** Emulated **ARM Cortex-M3** (ARM MPS2-AN385 FPGA image), 256 KB FLASH @ 0x0 / 256 KB RAM @ 0x20000000 (`mps2_an385.ld:31-32`), ARM semihosting I/O. Real cross-toolchain + real instruction-set emulation. (`on_target_qemu_differential.rs:621,678-680`; `ci.yml` `on-target` job.) **Skips locally** when `arm-none-eabi-gcc`/`qemu-system-arm` absent (by design, box is disk-tight — `on_target_qemu_differential.rs:131-144`); **HARD in CI** (`ci.yml:317-321`). |
| `ci.yml` build matrix | `cargo build/test` on ubuntu/macos/windows | Tests the **Rust toolchain** (the compiler itself), not generated firmware on MCUs. |

**Codegen is family-agnostic by construction (verified):** a negative grep
of `crates/fsm-codegen-c/src/` for `cortex|atmega|attiny|avr|riscv|rv32|msp430|stm8|xtensa|esp32|harvard|target_arch|#[cfg`
returns **∅ on non-comment lines**. There is no per-MCU code path. The only
`TargetProfile` axis is `Embedded` vs `Host` (`config.rs:149-156`) — it
toggles include set / assert macros, **not** any chip family. Portability
rests *entirely* on `<stdint.h>` + the HAL seam. This is a deliberate,
defensible design — but it means "supports family X" is an *untested
assumption* for every X except the one CI actually cross-builds.

## 2.3 The delta — first-class vs assumption-only vs unsupported

| Matrix row | Status | Evidence / why |
|---|---|---|
| **ARM Cortex-M3** | **First-class, on-target-tested** | The QEMU `mps2-an385` differential cross-compiles with `arm-none-eabi-gcc` + runs on emulated M3 hardware in CI as a HARD gate (`on_target_qemu_differential.rs`, `ci.yml on-target`). Real ISA emulation + real cross-toolchain. This is the *only* row with on-target evidence. |
| **ARM Cortex-M0/M0+/M4/M7/M33** | **Assumption-only (strong)** | Same `arm-none-eabi-gcc`/AAPCS/NVIC family as the tested M3, ABI documented (Doc 17 §2.1); the family-agnostic `<stdint.h>` codegen has no M3-specific path so it *should* port unchanged. But **no CI cross-builds these**, and M0/M0+ adds the no-hardware-divide constraint (§1.3) the M3 lane never exercises. High-confidence assumption, not tested fact. |
| **AVR (ATmega/ATtiny)** | **Assumption-only, and weaker than ARM** | Doc 17 §2.2 documents the ABI; a PlatformIO `uno` example *exists*. But CI does **not** run `pio`, so the AVR path is host-gcc-proven only (§2.2). Worse, the **Harvard `.rodata`-in-flash** constraint (§1.3) means the TABLE dispatch strategy is **likely incorrect on real AVR** (plain pointer deref of a flash table reads RAM) and **nothing tests this**. SWITCH-strategy small machines are probably fine; TABLE/large machines are an unverified risk. Honest verdict: *plausible for small SWITCH machines, unverified, with a known TABLE-strategy hazard.* |
| **RISC-V RV32, MSP430, ESP32 Xtensa, ESP32-C3** | **Assumption-only (untested)** | Doc 17 lists toolchains/ABIs (§2.3/§2.4 + the §1 table). Codegen is family-agnostic so it *should* compile. Zero CI/example coverage; the RTOS-mediated interrupt + critical-section divergences (ESP-IDF spinlock vs PRIMASK) are integrator-side and unspecified per-chip in Doc 16. Reasonable target, no evidence. |
| **STM8** | **Effectively unsupported in practice** | Needs **SDCC** (no GCC backend). The entire build/test/CI/example toolchain is GCC-family; **`sdcc` appears nowhere** in the repo. The generated C is *probably* close to SDCC-clean (it's portable C99) but SDCC's C99/`__attribute__` quirks are real and **untested**. Plus it is the lone big-endian row (debug-interface wire-format hazard). Treat as "not supported until an SDCC build is proven." |

### The honest one-liner the owner asked for

> **The only on-target-tested chip is the ARM Cortex-M3 (QEMU
> `mps2-an385`).** Every other family — including the rest of the Cortex-M
> line — is a *family-agnostic-codegen assumption* validated only by
> host-gcc ISO-C99 compilation, not by any cross-toolchain or emulator.
> AVR carries a *known, untested correctness hazard* (Harvard `.rodata`
> with the TABLE dispatch strategy). STM8 is *de facto unsupported* (no
> SDCC anywhere in the toolchain/CI).

---

# 3. Disclosed assumptions & what could not be verified

**Assumptions made in Part 1 (engineering judgement, not repo-derived):**

1. **FLASH/RAM "typical" ranges** are conservative commodity-part envelopes
   from general embedded-domain knowledge (datasheet families: STM32C0/F0/F1/F4/F7/H7,
   ATmega/ATtiny, GD32VF103, ESP32/-S3/-C3, MSP430G/FR, STM8S/L). They are
   *not* sourced from the repo (the repo has no chip database) and are not
   exhaustive — outliers exist (noted inline). Use as a "must-not-assume-larger"
   envelope, not a hard spec.
2. **Interrupt-model / ISR-declaration / clock-seam details** are standard
   architecture facts (ARMv6-M/v7-M/v8-M, AVR, RISC-V priv-spec, MSP430,
   Xtensa, STM8) cross-checked against the project's own Doc 17 ABI tables
   and the in-repo Cortex-M3 harness; they are not repo-novel.
3. **Endianness:** all listed little-endian except STM8 (big-endian) — a
   well-established fact; not repo-verifiable (no STM8 artifact exists).
4. The **AVR TABLE-strategy `.rodata`/flash hazard** is a *reasoned
   inference* from (a) Doc 11 §9 placing the table in `.rodata`, (b) AVR's
   Harvard architecture, and (c) the absence of any `PROGMEM`/`__flash`/`pgm_read`
   in `crates/fsm-codegen-c/src/`. It is **not** an observed test failure
   (no AVR build is run anywhere). Stated as a hazard to verify, not a
   confirmed defect.

**Could NOT verify (flagged explicitly):**

- **No actual AVR/RISC-V/MSP430/ESP32/STM8 build was executed** — by task
  constraint (research+doc only, no `crates/` changes) and because those
  cross-toolchains are CI-runner-only / absent from this box by design. The
  per-family "should compile" claims rest on the verified
  family-agnostic-codegen fact (∅ per-MCU branching) + Doc 17, not on a
  green build. The AVR-TABLE hazard in particular is *unconfirmed* — it
  needs a real `avr-gcc` link + execution to settle.
- **STM8/SDCC C99 conformance of the generated C** — not assessable without
  an SDCC toolchain (absent from repo & box).
- **Doc 16 vs code signature drift** is reported (§2.1) but its *history*
  (which doc reconciliation wave dropped `now_ms`/`bool cond` without
  updating Doc 16 §2–§6) was not traced — out of scope; recorded for a
  future Zero-Legacy doc-reconciliation pass.
- **No Rust build/test was run** (research-only task); no toolchain
  assertion was needed since nothing was compiled.

---

*End of FSM-DESIGN-CHIPMATRIX — 2026-05-19.*
