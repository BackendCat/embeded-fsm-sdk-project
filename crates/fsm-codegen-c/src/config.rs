//! Codegen configuration. Mirrors the CLI-facing surface of `fsm generate`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Top-level codegen knobs. Defaults match Doc 00 §10.3 / §10.4 (HAL
/// mandatory, MIT license) and Doc 11 §6 (queue capacity power-of-2, assert
/// on overflow).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenConfig {
    /// Dispatch lowering. `Auto` picks `Switch` for ≤ 64 states else `Table`.
    pub strategy: DispatchStrategy,
    /// Event queue capacity. MUST be a power of two so bitwise-AND modulo is
    /// legal in the generated ring buffer (Doc 11 §12).
    pub queue_capacity: u8,
    /// Behaviour when the user-facing queue overflows (Doc 11 §6).
    pub queue_overflow: OverflowPolicy,
    /// SPDX-License-Identifier string emitted into every generated file
    /// (Doc 00 §10.4). The codegen does NOT validate the SPDX identifier —
    /// the CLI is responsible for that pre-flight check.
    pub license_spdx: String,
    /// When `true`, emit simulator hook hooks (Doc 13 stub) — used by the
    /// trace-match conformance test runner. v1.0 default: off.
    pub include_simulator_hooks: bool,
    /// When `true`, inline transition action bodies into auto-generated
    /// helper functions instead of forcing the user to implement every
    /// action symbol as an extern. Default off so the user contract surface
    /// stays minimal (Doc 11 §18 is informative — opt-in for v1.0).
    pub include_inline_actions: bool,
    /// Target build profile. Influences include set + assert macros.
    pub target_profile: TargetProfile,
    /// Per-machine dispatch-strategy overrides keyed by machine name
    /// (v1.1-W7, Doc 00 §11.25). When a machine's name is present here, its
    /// value wins over [`CodegenConfig::strategy`] for that machine ONLY,
    /// enabling mixed dispatch in one project (machine A `Switch`, machine B
    /// `Table`). The CLI is responsible for resolving the source precedence
    /// (fsm.toml `[machine.M]` > `--strategy` flag > default) and populating
    /// this map; codegen just consults it per machine via
    /// [`CodegenConfig::strategy_for`]. `BTreeMap` keeps it deterministic.
    /// Empty by default → byte-identical output to pre-W7 when unused.
    #[serde(default)]
    pub machine_strategy_overrides: BTreeMap<String, DispatchStrategy>,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            strategy: DispatchStrategy::Auto,
            queue_capacity: 8,
            queue_overflow: OverflowPolicy::Assert,
            license_spdx: "MIT".to_owned(),
            include_simulator_hooks: false,
            include_inline_actions: false,
            target_profile: TargetProfile::Embedded,
            machine_strategy_overrides: BTreeMap::new(),
        }
    }
}

impl CodegenConfig {
    /// The *unresolved* effective dispatch strategy for the named machine:
    /// the per-machine override if one is registered, else the global
    /// [`CodegenConfig::strategy`]. `Auto` is NOT resolved here — the caller
    /// resolves it against that machine's own state count (an `Auto`
    /// override and an `Auto` default must both pick the same per-machine
    /// threshold). This is the single point that expresses "per-machine
    /// override wins over the global strategy"; CLI precedence (fsm.toml >
    /// flag > default) is resolved upstream when the map is built.
    pub fn strategy_for(&self, machine_name: &str) -> DispatchStrategy {
        self.machine_strategy_overrides
            .get(machine_name)
            .copied()
            .unwrap_or(self.strategy)
    }
}

/// Lowering strategy choice. `Auto` defers the decision until codegen has the
/// IR in hand (see `DispatchStrategy::resolve`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DispatchStrategy {
    /// Per Doc 11 §8 — nested switch with leaf-to-root ancestor walk (B-10).
    Switch,
    /// Per Doc 11 §9 — `.rodata` transition table, two-phase
    /// collect-then-execute (B-11).
    Table,
    /// `Switch` for ≤ 64 states (fits comfortably into compiler jump-table
    /// heuristics) else `Table`. Threshold matches Doc 11 §1 design goal of
    /// readable output for small machines.
    Auto,
}

impl DispatchStrategy {
    /// Resolve `Auto` against a given state count. `Switch` is chosen when
    /// the machine has at most 64 states (Doc 11 §1).
    pub fn resolve(self, state_count: usize) -> Self {
        match self {
            DispatchStrategy::Auto => {
                if state_count <= 64 {
                    DispatchStrategy::Switch
                } else {
                    DispatchStrategy::Table
                }
            }
            other => other,
        }
    }
}

/// Codegen-facing overflow policy. Distinct from `fsm_ir::OverflowPolicy`
/// because the IR enum has four variants (Doc 00 §7.4) but the C99 runtime
/// only ships two distinct paths (assert-or-drop) per Doc 11 §6. Mapping
/// happens in `OverflowPolicy::from_ir`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverflowPolicy {
    /// `FSM_QUEUE_ASSERT` — calls `fsm_hal_assert` on overflow.
    Assert,
    /// `FSM_QUEUE_DROP_NEWEST` — silently drops the incoming event.
    Drop,
}

impl OverflowPolicy {
    /// Map the IR's four-variant overflow policy onto codegen's two paths.
    /// `DropOldest` is treated as `Drop` to keep the runtime tiny; v1.0 ships
    /// one drop path. `Error` collapses to `Assert` — the runtime cannot
    /// surface an error code from inside an ISR.
    pub fn from_ir(p: fsm_ir::OverflowPolicy) -> Self {
        match p {
            fsm_ir::OverflowPolicy::Assert => OverflowPolicy::Assert,
            fsm_ir::OverflowPolicy::DropOldest => OverflowPolicy::Drop,
            fsm_ir::OverflowPolicy::DropNewest => OverflowPolicy::Drop,
            fsm_ir::OverflowPolicy::Error => OverflowPolicy::Assert,
        }
    }

    /// C macro name as emitted into `Motor_conf.h` (Doc 11 §6).
    pub fn macro_name(self) -> &'static str {
        match self {
            OverflowPolicy::Assert => "FSM_QUEUE_ASSERT",
            OverflowPolicy::Drop => "FSM_QUEUE_DROP_NEWEST",
        }
    }
}

/// Target build profile. v1.0 only ships two profiles; the enum stays an
/// open shape so out-of-tree consumers can add their own.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetProfile {
    /// Embedded MCU target — assumes a freestanding C99 environment.
    Embedded,
    /// POSIX host target — used for conformance test compilation.
    Host,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_mit() {
        assert_eq!(CodegenConfig::default().license_spdx, "MIT");
    }

    #[test]
    fn default_strategy_is_auto() {
        assert_eq!(CodegenConfig::default().strategy, DispatchStrategy::Auto);
    }

    #[test]
    fn auto_resolves_to_switch_for_small_machines() {
        assert_eq!(DispatchStrategy::Auto.resolve(3), DispatchStrategy::Switch);
    }

    #[test]
    fn auto_resolves_to_table_for_large_machines() {
        assert_eq!(DispatchStrategy::Auto.resolve(120), DispatchStrategy::Table);
    }

    #[test]
    fn overflow_policy_maps_drop_oldest_to_drop() {
        assert_eq!(
            OverflowPolicy::from_ir(fsm_ir::OverflowPolicy::DropOldest),
            OverflowPolicy::Drop
        );
    }

    #[test]
    fn overflow_policy_maps_error_to_assert() {
        // The C runtime can't gracefully surface "queue overflow" from an
        // ISR, so the analyzer-level Error policy collapses to Assert at the
        // C boundary. This is a design judgment call documented here.
        assert_eq!(
            OverflowPolicy::from_ir(fsm_ir::OverflowPolicy::Error),
            OverflowPolicy::Assert
        );
    }
}
