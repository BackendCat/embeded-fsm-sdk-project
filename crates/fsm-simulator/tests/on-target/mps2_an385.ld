/* SPDX-License-Identifier: MIT
 *
 * Phase-6.0-W2 — MPS2-AN385 (Cortex-M3) linker script for the on-target
 * QEMU differential lane (Doc 32 §1 W2 / §2 keystone). NOT shipped product
 * code: this is the *test harness* that lets QEMU run the SAME unmodified
 * `FSM_TRACE` C the W1 host differential compiles, on an emulated MCU, so
 * its emitted trace can be byte-diffed against the shipped
 * `fsm_simulator::execute_trace` oracle (the substrate changes, the oracle
 * does not — Doc 32 §2).
 *
 * Memory map: the QEMU `mps2-an385` machine models the ARM MPS2+ FPGA
 * board with the AN385 (Cortex-M3) image. The standard image layout used
 * here is the documented one:
 *   - code/rodata at 0x00000000 (the AN385 boots from the ZBT SSRAM1
 *     aliased at address 0; this is where the vector table must live so the
 *     Cortex-M3 can fetch the initial SP + reset vector at reset).
 *   - data/bss/stack/heap at 0x20000000 (SRAM).
 * Sizes are deliberately small (256 KiB / 256 KiB) — the corpus FSMs +
 * trace harness are tiny; this is well within the AN385's actual 4 MiB
 * regions and keeps the image minimal.
 *
 * This is a standard, documented Cortex-M / mps2-an385 linker script
 * (correctness-by-construction; CI exercises it under real
 * arm-none-eabi-gcc + qemu-system-arm). No FSM semantics live here.
 */

ENTRY(Reset_Handler)

MEMORY
{
  FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 256K
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 256K
}

/* Top-of-stack = end of RAM (full-descending stack, ARM AAPCS). The
 * startup vector table's first word is this value; the Cortex-M3 loads it
 * into MSP at reset. */
_estack = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  /* The vector table MUST be the very first thing at 0x00000000 so the
   * core's reset sequence reads [0]=initial SP, [1]=Reset_Handler. */
  .isr_vector :
  {
    KEEP(*(.isr_vector))
  } > FLASH

  .text :
  {
    *(.text)
    *(.text*)
    *(.rodata)
    *(.rodata*)
    *(.glue_7)
    *(.glue_7t)
    . = ALIGN(4);
    _etext = .;
  } > FLASH

  /* .data load address (LMA) is in FLASH; the startup code copies it to
   * its RAM VMA before main(). Standard Cortex-M C-runtime bring-up. */
  _sidata = LOADADDR(.data);
  .data :
  {
    . = ALIGN(4);
    _sdata = .;
    *(.data)
    *(.data*)
    . = ALIGN(4);
    _edata = .;
  } > RAM AT > FLASH

  .bss :
  {
    . = ALIGN(4);
    _sbss = .;
    *(.bss)
    *(.bss*)
    *(COMMON)
    . = ALIGN(4);
    _ebss = .;
  } > RAM

  /* A small heap region is not required (the harness uses no malloc), but
   * a symbol is provided so a libc that references it links cleanly. */
  . = ALIGN(8);
  _end = .;
  PROVIDE(end = .);

  /DISCARD/ :
  {
    *(.ARM.exidx*)
    *(.ARM.extab*)
    *(.comment)
  }
}
