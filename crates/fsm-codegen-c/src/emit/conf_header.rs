//! `Motor_conf.h` — compile-time configuration macros.
//!
//! Doc 11 §6 defines the macro vocabulary: queue capacity, overflow policy,
//! codegen strategy tag, optional `MOTOR_QUEUE_ISR_SAFE` define, and an
//! include of the HAL header. The generated source uses these macros for
//! preprocessor selection.

use crate::config::{DispatchStrategy, OverflowPolicy};

use super::license::header_block;
use super::{EmittedFile, FileRole, MachineEmitCtx};

pub fn emit(ctx: &MachineEmitCtx<'_>) -> EmittedFile {
    let prefix = ctx.macro_prefix();
    let stem = ctx.file_stem();
    let guard = format!("{}_CONF_H", prefix);
    let header = header_block(
        ctx.config,
        Some(&format!("{}.fsm (compile-time configuration)", stem)),
    );

    let overflow = OverflowPolicy::from_ir(ctx.machine.queue.overflow_policy);
    let strategy = match ctx.strategy {
        DispatchStrategy::Switch => "FSM_STRATEGY_SWITCH",
        DispatchStrategy::Table => "FSM_STRATEGY_TABLE",
        DispatchStrategy::Auto => "FSM_STRATEGY_SWITCH", // resolved already
    };

    let body = format!(
        r#"#ifndef {guard}
#define {guard}

#include "fsm_hal.h"

/* Event queue capacity. MUST be a power of two (Doc 11 §12 bitwise modulo).
 * Compiler enforces this at codegen time. */
#define {prefix}_QUEUE_CAPACITY     {qcap}u

/* Overflow policy: FSM_QUEUE_ASSERT or FSM_QUEUE_DROP_NEWEST. */
#define {prefix}_QUEUE_OVERFLOW     {overflow_macro}

/* Codegen strategy tag — informational, used by debuggers / linters. */
#define {prefix}_CODEGEN_STRATEGY   {strategy}

/* Timer counter type. Must be large enough to hold the largest declared
 * timer duration in milliseconds. uint32_t handles up to ~49.7 days. */
#define {prefix}_TIMER_TYPE         uint32_t

/* Define to compile Motor_post() with ISR-safe critical-section wrapping.
 * Requires FSM_ENTER_CRITICAL / FSM_EXIT_CRITICAL in fsm_hal.h. */
/* #define {prefix}_QUEUE_ISR_SAFE */

#endif /* {guard} */
"#,
        guard = guard,
        prefix = prefix,
        qcap = ctx.config.queue_capacity,
        overflow_macro = overflow.macro_name(),
        strategy = strategy,
    );

    EmittedFile {
        path: format!("{}_conf.h", stem),
        role: FileRole::ConfHeader,
        content: format!("{}\n{}", header, body),
    }
}
