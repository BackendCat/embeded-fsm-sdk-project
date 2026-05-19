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
    /// Fallback event-queue capacity used **only** when a machine has no
    /// in-source `queue {}` block AND no integrator override is set. MUST
    /// be a power of two so bitwise-AND modulo is legal in the generated
    /// ring buffer (Doc 11 §12). See [`CodegenConfig::resolve_queue`] for
    /// the full F-2 precedence (in-source `queue {}` > integrator override
    /// > this default).
    pub queue_capacity: u8,
    /// Fallback overflow policy, same precedence rules as
    /// [`CodegenConfig::queue_capacity`] (Doc 11 §6).
    pub queue_overflow: OverflowPolicy,
    /// Integrator capacity OVERRIDE — `Some` iff the build set
    /// `--queue-size` / `fsm.toml [generate] queue_size`. `None` means
    /// "no override; honor the machine's in-source `queue {}`". This is
    /// the explicit-override signal that lets codegen tell a deliberate
    /// integrator override apart from the default `8` (F-2: never silently
    /// shadow an in-source `queue {}`).
    #[serde(default)]
    pub queue_capacity_override: Option<u8>,
    /// Integrator overflow-policy OVERRIDE, same semantics as
    /// [`CodegenConfig::queue_capacity_override`]. `Some` iff the build set
    /// `--queue-overflow` / `fsm.toml [generate] queue_overflow`.
    #[serde(default)]
    pub queue_overflow_override: Option<OverflowPolicy>,
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
            queue_capacity_override: None,
            queue_overflow_override: None,
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

    /// Resolve the effective queue capacity + overflow policy for one
    /// machine (F-2 fix — close the #110/F-1 silent-misconfig of in-source
    /// `queue {}`).
    ///
    /// **Precedence (a deliberate, disclosed judgment call):**
    /// 1. an integrator override (`--queue-size` / `--queue-overflow` /
    ///    `fsm.toml [generate]`) — `*_override == Some` — wins. This is the
    ///    build-tool's explicit instruction and may legitimately retarget
    ///    a model for a constrained build.
    /// 2. else the machine's in-source `queue {}` block (`queue.is_explicit()`):
    ///    the model's own declared config (Doc 04 §9). Honored end-to-end.
    /// 3. else the codegen default (`queue_capacity` / `queue_overflow`,
    ///    `8` / `Assert`) — unchanged for models that declare neither.
    ///
    /// Returns `(capacity, overflow, note)`. `note` is `Some(_)` **iff** an
    /// integrator override shadowed an explicit in-source `queue {}` — the
    /// caller MUST surface it. This is the F-1 doctrine in force: honor the
    /// user's config, or *diagnose the conflict loudly* — never silently
    /// miscompile one into the other. Per-field: an override on capacity
    /// alone does not suppress an in-source overflow policy (and vice
    /// versa); each field resolves independently so a partial override is
    /// not a silent reset of the other field.
    pub fn resolve_queue(
        &self,
        machine_queue: &fsm_ir::QueueConfig,
    ) -> (u32, OverflowPolicy, Option<QueueOverrideNote>) {
        let explicit = machine_queue.is_explicit();

        // Capacity.
        let (capacity, cap_overridden) = match self.queue_capacity_override {
            Some(o) => (o as u32, explicit),
            None if explicit => (machine_queue.capacity, false),
            None => (self.queue_capacity as u32, false),
        };

        // Overflow.
        let ir_overflow = OverflowPolicy::from_ir(machine_queue.overflow_policy);
        let (overflow, ovf_overridden) = match self.queue_overflow_override {
            Some(o) => (o, explicit),
            None if explicit => (ir_overflow, false),
            None => (self.queue_overflow, false),
        };

        let note = if cap_overridden || ovf_overridden {
            Some(QueueOverrideNote {
                in_source_capacity: machine_queue.capacity,
                in_source_overflow: ir_overflow,
                effective_capacity: capacity,
                effective_overflow: overflow,
                capacity_overridden: cap_overridden,
                overflow_overridden: ovf_overridden,
            })
        } else {
            None
        };

        (capacity, overflow, note)
    }
}

/// Emitted by [`CodegenConfig::resolve_queue`] when an integrator override
/// (`--queue-size` / `fsm.toml`) shadowed an **explicit** in-source
/// `queue {}` block. The CLI turns this into a build note so the override
/// is never silent (F-1 doctrine: honor config OR diagnose the conflict).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueOverrideNote {
    pub in_source_capacity: u32,
    pub in_source_overflow: OverflowPolicy,
    pub effective_capacity: u32,
    pub effective_overflow: OverflowPolicy,
    pub capacity_overridden: bool,
    pub overflow_overridden: bool,
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

    // ---- F-2 `resolve_queue` precedence (in-source > override > default) --

    /// An EXPLICIT in-source `queue {}` (a real `.fsm` path in `loc`, so
    /// `is_explicit()` is true).
    fn explicit_queue(cap: u32, ovf: fsm_ir::OverflowPolicy) -> fsm_ir::QueueConfig {
        fsm_ir::QueueConfig {
            capacity: cap,
            overflow_policy: ovf,
            loc: fsm_ir::SourceLocation::new("q.fsm", fsm_ir::Span::new(0, 1), 1, 1),
        }
    }

    #[test]
    fn no_override_honors_in_source_queue() {
        // (a) absent any CLI/TOML override → the DSL `queue {}` capacity +
        // overflow MUST flow to codegen.
        let cfg = CodegenConfig::default(); // both overrides None
        let (cap, ovf, note) =
            cfg.resolve_queue(&explicit_queue(64, fsm_ir::OverflowPolicy::DropNewest));
        assert_eq!(cap, 64, "in-source capacity must win when no override");
        assert_eq!(ovf, OverflowPolicy::Drop);
        assert!(note.is_none(), "no override ⇒ no shadow note");
    }

    #[test]
    fn no_override_no_in_source_block_keeps_codegen_default() {
        // (c) absent both → the existing default (8 / Assert) stays.
        let cfg = CodegenConfig::default();
        let (cap, ovf, note) = cfg.resolve_queue(&fsm_ir::QueueConfig::default());
        assert_eq!(cap, 8, "no block + no override ⇒ codegen default 8");
        assert_eq!(ovf, OverflowPolicy::Assert);
        assert!(note.is_none());
    }

    #[test]
    fn integrator_override_shadows_explicit_block_with_a_note() {
        // (b) a CLI/TOML override of an EXPLICIT in-source `queue {}` is
        // allowed, but emits a note — NEVER a silent override (F-1 doctrine).
        let cfg = CodegenConfig {
            queue_capacity_override: Some(16),
            ..CodegenConfig::default()
        };
        let (cap, _ovf, note) =
            cfg.resolve_queue(&explicit_queue(64, fsm_ir::OverflowPolicy::DropNewest));
        assert_eq!(
            cap, 16,
            "the integrator override wins over the in-source block"
        );
        let note = note.expect("shadowing an explicit block MUST produce a note");
        assert!(note.capacity_overridden, "capacity was the shadowed field");
        assert!(
            !note.overflow_overridden,
            "no overflow override was set — that field stays in-source"
        );
        assert_eq!(note.in_source_capacity, 64);
        assert_eq!(note.effective_capacity, 16);
    }

    #[test]
    fn capacity_override_alone_does_not_reset_in_source_overflow() {
        // Per-field independence: overriding capacity must NOT silently
        // revert an in-source overflow policy to the default.
        let cfg = CodegenConfig {
            queue_capacity_override: Some(8),
            ..CodegenConfig::default()
        };
        let (cap, ovf, note) =
            cfg.resolve_queue(&explicit_queue(64, fsm_ir::OverflowPolicy::DropNewest));
        assert_eq!(cap, 8);
        assert_eq!(
            ovf,
            OverflowPolicy::Drop,
            "in-source overflow=drop_newest must survive a capacity-only override"
        );
        assert!(note.unwrap().capacity_overridden && !note_overflow(&cfg));
    }

    fn note_overflow(cfg: &CodegenConfig) -> bool {
        cfg.queue_overflow_override.is_some()
    }

    #[test]
    fn override_against_no_in_source_block_is_not_a_shadow() {
        // An override when the machine declared NO `queue {}` is the plain
        // default-replacement path — there is nothing to shadow, so no note.
        let cfg = CodegenConfig {
            queue_capacity_override: Some(32),
            ..CodegenConfig::default()
        };
        let (cap, _ovf, note) = cfg.resolve_queue(&fsm_ir::QueueConfig::default());
        assert_eq!(cap, 32);
        assert!(
            note.is_none(),
            "no explicit in-source block ⇒ an override shadows nothing ⇒ no note"
        );
    }
}
