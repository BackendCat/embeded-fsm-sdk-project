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

/* Deferred-event buffer capacity (Doc 08 §10). Sized to the queue
 * capacity: the worst case is every queued slot's worth of events being
 * held by a deferring state before release. Not required to be a power of
 * two — the defer buffer is a linearly-scanned bounded array, not a
 * bitwise-modulo ring (release filters arbitrary elements per §10.4). */
#define {prefix}_DEFER_CAPACITY     {qcap}u

/* Overflow policy: FSM_QUEUE_ASSERT or FSM_QUEUE_DROP_NEWEST. */
#define {prefix}_QUEUE_OVERFLOW     {overflow_macro}

/* Codegen strategy tag — informational, used by debuggers / linters. */
#define {prefix}_CODEGEN_STRATEGY   {strategy}

/* Maximum number of simultaneously active leaves. 1 for flat / hierarchical
 * machines; >1 when parallel regions are present. Sized at codegen time
 * from a static analysis of the IR (Doc 00 §7.8 / Doc 08 §2.3). */
#define {prefix}_MAX_PARALLEL_REGIONS  {regions}u

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
        regions = ctx.layout.max_parallel_regions,
    );

    EmittedFile {
        path: format!("{}_conf.h", stem),
        role: FileRole::ConfHeader,
        content: format!("{}\n{}", header, body),
    }
}
