//! Parser DoS limits.
//!
//! Per Doc 00 §7.12 (G-02) the parser MUST be safe to run on attacker-supplied
//! `.fsm` source on shared CI infrastructure. The recursive-descent driver
//! is unbounded by default — a deeply nested expression file
//! (`(((((... 100 000 levels)))))`) would blow the stack, and a multi-gigabyte
//! input would exhaust RAM during lex / parse. Limits close both gaps.
//!
//! Two surfaces consume the limits:
//!
//! - [`crate::parse`] checks `max_input_bytes` at entry and `max_token_count`
//!   immediately after tokenization. Either exceed → a single
//!   `FSM-E0010` diagnostic + empty file CST. The parser does **not** panic.
//! - The grammar driver ([`crate::parser::Parser`]) carries a
//!   `current_depth` counter that every recursive rule increments via
//!   [`DepthGuard`]. On entry, if `current_depth >= max_recursion_depth`,
//!   the rule emits `FSM-E0010` and returns without recursing.
//!
//! ## Default values — rationale
//!
//! - `max_input_bytes: 1 MiB` — UML statecharts in the v1.0 conformance
//!   suite are well under 50 KiB. 1 MiB covers any realistic hand-written
//!   FSM with comfortable margin while making a 10 MB adversarial input
//!   trivially rejected. Tightening the cap costs almost no real input;
//!   loosening it risks RAM exhaustion since rowan must hold the full
//!   green tree.
//! - `max_recursion_depth: 256` — handwritten statecharts nest 5-15
//!   levels in practice. The Pratt expression parser recurses once per
//!   operator level (~13 levels in Doc 04 §8.7.1) plus one per `(...)`
//!   nesting. 256 is comfortably above any realistic source and well
//!   below the default 8 MB stack at typical frame sizes (~200 bytes per
//!   recursive frame ⇒ ~50 KB stack usage at depth 256).
//! - `max_token_count: 256 KiB tokens` — proportional to `max_input_bytes`
//!   assuming a token-to-byte ratio of ~1:4 on real DSL source. A million
//!   one-byte tokens would not be reachable from a 1 MiB input under our
//!   lexer rules, but the explicit cap protects against pathological
//!   tokenizations and pre-built token vectors fed via
//!   [`crate::parse_with_tokens`].

/// Caps for a single parse call. Cheap to construct (no allocation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Maximum source size in bytes. Inputs larger than this are rejected
    /// at the entry to [`crate::parse`] without tokenization.
    pub max_input_bytes: usize,
    /// Maximum recursion depth in the grammar driver. Each recursive
    /// grammar rule (state/region/action-block/expr) increments a counter
    /// via [`DepthGuard`]; if the counter would exceed this value the
    /// rule emits a diagnostic and returns without further descent.
    pub max_recursion_depth: u32,
    /// Maximum number of lexer tokens. Checked once, immediately after
    /// tokenization; a giant token vector is treated identically to a
    /// giant input.
    pub max_token_count: usize,
}

impl ParseLimits {
    /// Conservative defaults — see module-level docs for rationale.
    /// Anything above these limits is overwhelmingly more likely to be
    /// adversarial than a real DSL author at the keyboard.
    pub const DEFAULT: Self = Self {
        // 1 MiB — covers any realistic hand-written FSM.
        max_input_bytes: 1 << 20,
        // 256 — generous over the deepest realistic UML nesting (~15)
        // plus expression parens.
        max_recursion_depth: 256,
        // ~262k tokens — proportional to input cap.
        max_token_count: 1 << 18,
    };
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_sane() {
        let d = ParseLimits::default();
        // Spot-check: limits are non-zero and large enough for any real
        // file but tight enough to reject obvious adversarial inputs.
        assert!(d.max_input_bytes >= 64 * 1024);
        assert!(d.max_input_bytes <= 16 * 1024 * 1024);
        assert!(d.max_recursion_depth >= 32);
        assert!(d.max_recursion_depth <= 4096);
        assert!(d.max_token_count >= 16 * 1024);
    }

    #[test]
    fn default_is_copy() {
        // ParseLimits is Copy/Clone; verifying the trait obligations
        // compile here keeps the public API stable.
        let d = ParseLimits::default();
        let _e = d;
        let _f = d;
    }
}
