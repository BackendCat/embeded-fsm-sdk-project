//! `fsm-diagnostics` — foundation crate owning diagnostic primitives.
//!
//! Per `docs/00-Decisions-And-Reconciliation.md` §7.1, this crate is the single
//! source of truth for `Span`, `SourceLocation`, `Severity`, `DiagnosticCode`,
//! `Diagnostic`, and `RelatedInfo`. The full code list is taken from
//! `docs/10-Diagnostic-Code-Catalog.md` plus the reconciler additions in
//! Doc 00 §B-01 (`E0410`, `E0111`, `E0610`, `E0210`, `W0604`, `E0110`,
//! `H0006`, `E0903`).
//!
//! The crate has **zero workspace dependencies** by design — every other
//! pipeline crate depends on `fsm-diagnostics`, never the other way round.
//! Optional integrations live behind the `serde` and `miette` cargo features.

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

use core::fmt;

// ---------------------------------------------------------------------------
// Span
// ---------------------------------------------------------------------------

/// Byte-offset range inside a single source file.
///
/// Half-open: `[start, end)`. Both ends are byte offsets, not char or UTF-16
/// units — the LSP layer is responsible for converting to whatever shape the
/// editor requires.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Construct a span. `end` may be less than `start`; callers that care
    /// should normalise via [`Span::merge`].
    #[inline]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Zero-width span anchored at `at`. Useful for "expected something
    /// here" diagnostics that point at a gap between tokens.
    #[inline]
    pub const fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }

    /// Smallest span covering both inputs.
    #[inline]
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Whether `pos` falls inside this span (half-open).
    #[inline]
    pub const fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Length in bytes. Returns 0 for inverted or empty spans.
    #[inline]
    pub const fn len(&self) -> usize {
        if self.end > self.start {
            self.end - self.start
        } else {
            0
        }
    }

    /// `true` iff the span has zero length.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// SourceLocation
// ---------------------------------------------------------------------------

/// A `Span` enriched with file path + pre-computed line/column.
///
/// This is the shape stored on every IR node (`loc` field per Doc 09 §2) so
/// downstream consumers — codegen, simulator traces, formatter — can emit
/// "file:line:col" references without re-walking the source.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceLocation {
    pub file: String,
    pub span: Span,
    pub line: u32,
    pub column: u32,
}

impl SourceLocation {
    /// Convenience constructor; mostly used by parser when building IR.
    pub fn new(file: impl Into<String>, span: Span, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            span,
            line,
            column,
        }
    }
}

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Diagnostic severity buckets per Doc 10 §1.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        })
    }
}

// ---------------------------------------------------------------------------
// DiagnosticCode
// ---------------------------------------------------------------------------

/// Macro that drives the live `DiagnosticCode` enum, its `Display`,
/// `severity()`, `default_message()`, `all_codes()`, and `from_str()` impls
/// from one source-of-truth table.
///
/// Each row is `Variant => (Severity, "FSM-XNNNN", "default message")`.
///
/// Codes intentionally **omitted** here are deprecated and live in
/// [`deprecated::DeprecatedCode`] so the suppression parser still recognises
/// them per Doc 10 §14.
macro_rules! for_each_code {
    ($cb:ident) => {
        $cb! {
            // §3 — Lexer / Parser Errors (FSM-E0001 – FSM-E0099)
            E0001 => (Error,   "FSM-E0001", "unexpected character"),
            E0002 => (Error,   "FSM-E0002", "unterminated string literal"),
            E0003 => (Error,   "FSM-E0003", "unterminated block comment"),
            E0004 => (Error,   "FSM-E0004", "invalid integer literal"),
            E0005 => (Error,   "FSM-E0005", "float literal in integer context"),
            E0006 => (Error,   "FSM-E0006", "write to read-only payload field"),
            E0010 => (Error,   "FSM-E0010", "expected token"),
            E0011 => (Error,   "FSM-E0011", "unexpected end of file"),
            E0012 => (Error,   "FSM-E0012", "invalid `as` cast"),

            // §4 — Duplicate Declaration Errors (FSM-E0020 – FSM-E0029)
            E0020 => (Error,   "FSM-E0020", "duplicate machine name"),
            E0021 => (Error,   "FSM-E0021", "duplicate state name"),
            E0022 => (Error,   "FSM-E0022", "duplicate event name"),
            E0023 => (Error,   "FSM-E0023", "duplicate context field name"),
            E0024 => (Error,   "FSM-E0024", "duplicate extern name"),
            E0025 => (Error,   "FSM-E0025", "duplicate stable ID"),

            // §5 — Symbol Resolution Errors (FSM-E0100 – FSM-E0109)
            E0100 => (Error,   "FSM-E0100", "unknown state reference"),
            E0101 => (Error,   "FSM-E0101", "unknown event reference"),
            E0102 => (Error,   "FSM-E0102", "unknown extern reference"),
            E0103 => (Error,   "FSM-E0103", "unknown machine reference"),
            E0104 => (Error,   "FSM-E0104", "unknown context field reference"),
            E0105 => (Error,   "FSM-E0105", "state reference crosses machine boundary"),
            E0106 => (Error,   "FSM-E0106", "non-pure extern used as guard"),
            E0107 => (Error,   "FSM-E0107", "no initial declaration"),
            E0108 => (Error,   "FSM-E0108", "multiple initial declarations"),
            E0109 => (Error,   "FSM-E0109", "history default references non-existent state"),
            // Reconciler additions (Doc 00 §B-01 / §8 Doc 10 patch):
            E0110 => (Error,   "FSM-E0110", "local transition target is not a proper descendant of source"),
            E0111 => (Error,   "FSM-E0111", "history pseudo-state has no `default ->` declaration"),

            // §6 — Type and Semantic Errors (FSM-E0200 – FSM-E0299)
            E0200 => (Error,   "FSM-E0200", "type mismatch in guard expression"),
            E0201 => (Error,   "FSM-E0201", "type mismatch in assignment"),
            E0202 => (Error,   "FSM-E0202", "type mismatch in extern call argument"),
            E0203 => (Error,   "FSM-E0203", "wrong number of arguments to extern"),
            E0204 => (Error,   "FSM-E0204", "assignment to read-only field"),
            E0205 => (Error,   "FSM-E0205", "invalid left-hand side of assignment"),
            E0206 => (Error,   "FSM-E0206", "integer overflow in constant expression"),
            E0207 => (Error,   "FSM-E0207", "division by zero in constant expression"),
            E0208 => (Error,   "FSM-E0208", "negative value in unsigned context"),
            // Reconciler addition (Doc 00 §B-01 item 6, §8 Doc 10 patch):
            E0210 => (Error,   "FSM-E0210", "opaque field used in field-comparison guard"),

            // §7 — Determinism Errors (FSM-E0300 – FSM-E0399)
            //   E0301 retired -> deprecated module
            //   E0304 retired -> deprecated module
            E0300 => (Error,   "FSM-E0300", "nondeterministic transition conflict"),
            E0302 => (Error,   "FSM-E0302", "fork target is not a region initial state"),
            E0303 => (Error,   "FSM-E0303", "join source is not in a parallel state region"),
            E0310 => (Error,   "FSM-E0310", "deferred event conflicts with explicit transition"),

            // §8 — Reachability Errors (FSM-E0400 – FSM-E0499)
            E0400 => (Error,   "FSM-E0400", "unreachable state"),
            E0401 => (Error,   "FSM-E0401", "external self-transition on composite state without exit"),
            // Reconciler addition (Doc 00 §8 Doc 10 patch — formerly W0400):
            E0410 => (Error,   "FSM-E0410", "timer duration must be greater than zero"),

            // §9 — Submachine Errors (FSM-E0500 – FSM-E0599)
            E0500 => (Error,   "FSM-E0500", "submachine entry point not declared"),
            E0501 => (Error,   "FSM-E0501", "submachine exit point not declared"),
            E0502 => (Error,   "FSM-E0502", "submachine instantiation cycle detected"),

            // FSM-E0600..E0799 — parallel / fork
            E0600 => (Error,   "FSM-E0600", "parallel region has no `initial` declaration"),
            // Reconciler addition (Doc 00 §B-01 item 4, §8 Doc 10 patch):
            E0610 => (Error,   "FSM-E0610", "construct used without required `feature` flag"),
            E0750 => (Error,   "FSM-E0750", "fork target is not a parallel region"),

            // §10 — Runtime Safety Errors (FSM-E0900 – FSM-E0999)
            E0900 => (Error,   "FSM-E0900", "completion event chain too deep"),
            // Reconciler addition (Doc 00 §8 / G-08):
            E0903 => (Error,   "FSM-E0903", "too many event types for defer bitmask"),

            // §11 — Warnings (FSM-W0xxx)
            //   W0400 retired (now E0410) -> deprecated module
            W0100 => (Warning, "FSM-W0100", "history state has no stored value and no default"),
            W0101 => (Warning, "FSM-W0101", "dead transition"),
            W0200 => (Warning, "FSM-W0200", "loop in action block"),
            W0201 => (Warning, "FSM-W0201", "action block complexity"),
            W0300 => (Warning, "FSM-W0300", "transition priority used to resolve conflict"),
            W0401 => (Warning, "FSM-W0401", "timer duration unusually large"),
            W0500 => (Warning, "FSM-W0500", "extern declared but never used"),
            W0501 => (Warning, "FSM-W0501", "event declared but never used"),
            W0600 => (Warning, "FSM-W0600", "region with single state"),
            W0601 => (Warning, "FSM-W0601", "timer duration exceeds 24 hours"),
            W0602 => (Warning, "FSM-W0602", "unreachable state (no incoming transitions, not initial)"),
            W0603 => (Warning, "FSM-W0603", "guard always evaluates to true/false (constant guard)"),
            // Reconciler addition (Doc 00 §B-01 item 3):
            W0604 => (Warning, "FSM-W0604", "possible precision loss in float cast"),

            // §12 — Info (FSM-I0xxx)
            I0001 => (Info,    "FSM-I0001", "compilation successful"),
            I0002 => (Info,    "FSM-I0002", "code generation complete"),
            I0003 => (Info,    "FSM-I0003", "simulator ready"),
            I0004 => (Info,    "FSM-I0004", "IR schema version mismatch (forward-compatible)"),

            // §13 — Hints (FSM-H0xxx) — editor only
            H0001 => (Hint,    "FSM-H0001", "state has no entry action"),
            H0002 => (Hint,    "FSM-H0002", "state has no exit action"),
            H0003 => (Hint,    "FSM-H0003", "unconditional transition (no guard)"),
            H0004 => (Hint,    "FSM-H0004", "single-region parallel state"),
            H0005 => (Hint,    "FSM-H0005", "prefer `every` over periodic self-transition"),
            // Reconciler addition (Doc 00 §B-01 item 3):
            H0006 => (Hint,    "FSM-H0006", "stable `@id` placed before doc comment (suggest swap)"),
        }
    };
}

// Build the enum from the table.
macro_rules! define_enum {
    ( $( $variant:ident => ($sev:ident, $wire:literal, $msg:literal), )+ ) => {
        /// Stable identifier of every diagnostic the FSM-Lang toolchain may
        /// emit. Wire form is `"FSM-XNNNN"` (see `Display`).
        ///
        /// Codes are append-only per Doc 10 §14. Retired codes live in
        /// [`deprecated::DeprecatedCode`] so the suppression parser still
        /// accepts them.
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[allow(non_camel_case_types)]
        pub enum DiagnosticCode {
            $( $variant, )+
        }
    };
}
for_each_code!(define_enum);

// Display => "FSM-XNNNN"
macro_rules! impl_display {
    ( $( $variant:ident => ($sev:ident, $wire:literal, $msg:literal), )+ ) => {
        impl fmt::Display for DiagnosticCode {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self {
                    $( DiagnosticCode::$variant => $wire, )+
                })
            }
        }
    };
}
for_each_code!(impl_display);

// severity() and default_message()
macro_rules! impl_metadata {
    ( $( $variant:ident => ($sev:ident, $wire:literal, $msg:literal), )+ ) => {
        impl DiagnosticCode {
            /// Severity bucket per Doc 10.
            pub const fn severity(&self) -> Severity {
                match self {
                    $( DiagnosticCode::$variant => Severity::$sev, )+
                }
            }

            /// Canonical English message text. May be overridden on a
            /// per-instance basis via [`Diagnostic::with_message`].
            pub const fn default_message(&self) -> &'static str {
                match self {
                    $( DiagnosticCode::$variant => $msg, )+
                }
            }

            /// Canonical wire form (`"FSM-E0100"`). Same as `Display` but
            /// `const` so it can appear in pattern contexts at the cost of
            /// allocation.
            pub const fn as_wire(&self) -> &'static str {
                match self {
                    $( DiagnosticCode::$variant => $wire, )+
                }
            }
        }
    };
}
for_each_code!(impl_metadata);

// CODE_TABLE: static array; from_str + all_codes
macro_rules! impl_table {
    ( $( $variant:ident => ($sev:ident, $wire:literal, $msg:literal), )+ ) => {
        const CODE_TABLE: &[(DiagnosticCode, Severity, &'static str)] = &[
            $( (DiagnosticCode::$variant, Severity::$sev, $msg), )+
        ];

        impl DiagnosticCode {
            /// Parse the canonical wire form (`"FSM-E0100"`) back into a
            /// `DiagnosticCode`. Returns `None` for unknown or retired codes.
            ///
            /// Retired codes (e.g. `FSM-E0301`) parse via
            /// [`deprecated::DeprecatedCode::from_str`] instead.
            ///
            /// Intentionally an inherent `Option`-returning method rather
            /// than `std::str::FromStr` — there is no meaningful `Err` type
            /// for "unknown code" beyond the `None` case, and suppression
            /// parsing wants the `Option` ergonomics.
            #[allow(clippy::should_implement_trait)]
            pub fn from_str(s: &str) -> Option<Self> {
                match s {
                    $( $wire => Some(DiagnosticCode::$variant), )+
                    _ => None,
                }
            }
        }
    };
}
for_each_code!(impl_table);

impl DiagnosticCode {
    /// All codes paired with their severity and default message. Provided so
    /// tooling (CLI `fsm doc`, registry generation) can iterate the full set
    /// without hard-coding the list.
    pub const fn all_codes() -> &'static [(DiagnosticCode, Severity, &'static str)] {
        CODE_TABLE
    }
}

// ---------------------------------------------------------------------------
// Deprecated codes — kept for suppression-parser compatibility per Doc 10 §14
// ---------------------------------------------------------------------------

pub mod deprecated {
    //! Codes that have been retired (no longer emitted by the toolchain) but
    //! still recognised in `// fsm-lint:disable …` annotations.
    //!
    //! Retirement reasons (Doc 00 §B-01 + Doc 10 §8 patches):
    //!   - `E0301` "Guard on completion transition" — completion guards are
    //!      now allowed (B-07); the check is gone.
    //!   - `E0304` "Parallel region has no initial" — duplicate of E0600.
    //!   - `W0400` "Timer duration is zero" — promoted to fatal `E0410`.
    use core::fmt;

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    #[allow(non_camel_case_types)]
    pub enum DeprecatedCode {
        E0301,
        E0304,
        W0400,
    }

    impl fmt::Display for DeprecatedCode {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(match self {
                DeprecatedCode::E0301 => "FSM-E0301",
                DeprecatedCode::E0304 => "FSM-E0304",
                DeprecatedCode::W0400 => "FSM-W0400",
            })
        }
    }

    impl DeprecatedCode {
        /// Parse a retired wire form. The suppression parser MUST accept the
        /// resulting code and then drop the suppression silently (Doc 10
        /// §14 rule 2). Inherent `Option` API — see the matching note on
        /// `DiagnosticCode::from_str`.
        #[allow(clippy::should_implement_trait)]
        pub fn from_str(s: &str) -> Option<Self> {
            match s {
                "FSM-E0301" => Some(DeprecatedCode::E0301),
                "FSM-E0304" => Some(DeprecatedCode::E0304),
                "FSM-W0400" => Some(DeprecatedCode::W0400),
                _ => None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A secondary span attached to a primary [`Diagnostic`] — e.g. "previous
/// declaration was here" for a duplicate-name error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelatedInfo {
    pub message: String,
    pub span: Span,
}

/// A single diagnostic message reported by the toolchain.
///
/// Built from a [`DiagnosticCode`] plus the offending source [`Span`]. Severity
/// and the default message text are looked up from the code; both may be
/// customised via the builder API.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub related: Vec<RelatedInfo>,
}

impl Diagnostic {
    /// Construct a diagnostic for `code` pointing at `span`. Severity and
    /// message default to the canonical values from Doc 10.
    pub fn new(code: DiagnosticCode, span: Span) -> Self {
        Self {
            code,
            severity: code.severity(),
            message: code.default_message().to_owned(),
            span,
            related: Vec::new(),
        }
    }

    /// Override the default message — used when a code's text needs to be
    /// parameterised (e.g. `"expected ';', found '}'"`).
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Append a [`RelatedInfo`] entry — e.g. for "previous definition here"
    /// secondary spans.
    pub fn with_related(mut self, related: RelatedInfo) -> Self {
        self.related.push(related);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: [{}] {}", self.severity, self.code, self.message)
    }
}

#[cfg(feature = "miette")]
impl std::error::Error for Diagnostic {}

#[cfg(feature = "miette")]
impl miette::Diagnostic for Diagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(match self.severity {
            Severity::Error => miette::Severity::Error,
            Severity::Warning => miette::Severity::Warning,
            Severity::Info | Severity::Hint => miette::Severity::Advice,
        })
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        let primary =
            miette::LabeledSpan::new(Some(self.message.clone()), self.span.start, self.span.len());
        let extras = self
            .related
            .iter()
            .map(|r| miette::LabeledSpan::new(Some(r.message.clone()), r.span.start, r.span.len()));
        Some(Box::new(core::iter::once(primary).chain(extras)))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Span ---

    #[test]
    fn span_new_and_len() {
        let s = Span::new(3, 10);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 10);
        assert_eq!(s.len(), 7);
        assert!(!s.is_empty());
    }

    #[test]
    fn span_empty_is_zero_width() {
        let s = Span::empty(42);
        assert_eq!(s.start, 42);
        assert_eq!(s.end, 42);
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
    }

    #[test]
    fn span_contains() {
        let s = Span::new(5, 10);
        assert!(!s.contains(4));
        assert!(s.contains(5));
        assert!(s.contains(7));
        assert!(s.contains(9));
        // half-open: end is exclusive
        assert!(!s.contains(10));
        assert!(!s.contains(11));
    }

    #[test]
    fn span_merge_overlapping() {
        let a = Span::new(2, 6);
        let b = Span::new(4, 9);
        assert_eq!(a.merge(b), Span::new(2, 9));
        assert_eq!(b.merge(a), Span::new(2, 9));
    }

    #[test]
    fn span_merge_disjoint() {
        let a = Span::new(0, 3);
        let b = Span::new(10, 15);
        assert_eq!(a.merge(b), Span::new(0, 15));
    }

    #[test]
    fn span_merge_with_empty() {
        let a = Span::new(5, 8);
        let empty = Span::empty(20);
        assert_eq!(a.merge(empty), Span::new(5, 20));
    }

    // --- Severity ---

    #[test]
    fn severity_display_lowercase() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Info.to_string(), "info");
        assert_eq!(Severity::Hint.to_string(), "hint");
    }

    // --- DiagnosticCode: smoke per severity bucket ---

    #[test]
    fn code_severity_buckets() {
        // One representative per severity, per task brief.
        assert_eq!(DiagnosticCode::E0001.severity(), Severity::Error);
        assert_eq!(DiagnosticCode::W0300.severity(), Severity::Warning);
        assert_eq!(DiagnosticCode::I0001.severity(), Severity::Info);
        assert_eq!(DiagnosticCode::H0001.severity(), Severity::Hint);
    }

    #[test]
    fn code_severity_matches_prefix_for_every_variant() {
        for &(code, sev, _) in DiagnosticCode::all_codes() {
            let wire = code.as_wire();
            let expected = match wire.as_bytes()[4] {
                b'E' => Severity::Error,
                b'W' => Severity::Warning,
                b'I' => Severity::Info,
                b'H' => Severity::Hint,
                other => panic!("unexpected severity prefix byte: {other:?} in {wire}"),
            };
            assert_eq!(
                sev, expected,
                "severity prefix vs Severity mismatch for {wire}"
            );
            assert_eq!(code.severity(), expected);
        }
    }

    // --- Display / from_str round trip ---

    #[test]
    fn code_display_format() {
        assert_eq!(DiagnosticCode::E0001.to_string(), "FSM-E0001");
        assert_eq!(DiagnosticCode::E0100.to_string(), "FSM-E0100");
        assert_eq!(DiagnosticCode::W0300.to_string(), "FSM-W0300");
        assert_eq!(DiagnosticCode::W0604.to_string(), "FSM-W0604");
        assert_eq!(DiagnosticCode::I0002.to_string(), "FSM-I0002");
        assert_eq!(DiagnosticCode::H0006.to_string(), "FSM-H0006");
    }

    #[test]
    fn code_from_str_round_trip() {
        let sample = [
            DiagnosticCode::E0001,
            DiagnosticCode::E0025,
            DiagnosticCode::E0100,
            DiagnosticCode::E0110,
            DiagnosticCode::E0210,
            DiagnosticCode::E0410,
            DiagnosticCode::E0600,
            DiagnosticCode::E0610,
            DiagnosticCode::W0300,
            DiagnosticCode::W0604,
            DiagnosticCode::I0001,
            DiagnosticCode::H0006,
        ];
        for code in sample {
            let wire = code.to_string();
            assert_eq!(
                DiagnosticCode::from_str(&wire),
                Some(code),
                "round trip for {wire}"
            );
        }
    }

    #[test]
    fn code_from_str_unknown_returns_none() {
        assert!(DiagnosticCode::from_str("FSM-E9999").is_none());
        assert!(DiagnosticCode::from_str("bogus").is_none());
        assert!(DiagnosticCode::from_str("").is_none());
        // Retired codes do NOT parse via the live enum:
        assert!(DiagnosticCode::from_str("FSM-E0301").is_none());
        assert!(DiagnosticCode::from_str("FSM-E0304").is_none());
        assert!(DiagnosticCode::from_str("FSM-W0400").is_none());
    }

    // --- all_codes() vs variant count ---

    #[test]
    fn all_codes_matches_expected_count() {
        // Live variants: 75 (52 E + 13 W + 4 I + 6 H).
        // If a new code is added, update this constant in lockstep.
        const EXPECTED: usize = 75;
        let table = DiagnosticCode::all_codes();
        assert_eq!(
            table.len(),
            EXPECTED,
            "CODE_TABLE length drifted from EXPECTED"
        );
    }

    #[test]
    fn all_codes_entries_consistent_with_methods() {
        for &(code, sev, msg) in DiagnosticCode::all_codes() {
            assert_eq!(code.severity(), sev);
            assert_eq!(code.default_message(), msg);
        }
    }

    #[test]
    fn default_message_non_empty_for_every_variant() {
        for &(code, _, msg) in DiagnosticCode::all_codes() {
            assert!(
                !msg.is_empty(),
                "default_message() empty for {}",
                code.as_wire()
            );
            assert_eq!(code.default_message(), msg);
        }
    }

    #[test]
    fn all_codes_unique_wire_forms() {
        let mut seen = std::collections::HashSet::new();
        for &(code, _, _) in DiagnosticCode::all_codes() {
            assert!(
                seen.insert(code.as_wire()),
                "duplicate wire form: {}",
                code.as_wire()
            );
        }
    }

    // --- deprecated module ---

    #[test]
    fn deprecated_codes_display_and_parse() {
        use deprecated::DeprecatedCode;
        assert_eq!(DeprecatedCode::E0301.to_string(), "FSM-E0301");
        assert_eq!(DeprecatedCode::E0304.to_string(), "FSM-E0304");
        assert_eq!(DeprecatedCode::W0400.to_string(), "FSM-W0400");
        assert_eq!(
            DeprecatedCode::from_str("FSM-E0301"),
            Some(DeprecatedCode::E0301)
        );
        assert_eq!(
            DeprecatedCode::from_str("FSM-E0304"),
            Some(DeprecatedCode::E0304)
        );
        assert_eq!(
            DeprecatedCode::from_str("FSM-W0400"),
            Some(DeprecatedCode::W0400)
        );
        assert_eq!(DeprecatedCode::from_str("FSM-E0001"), None);
    }

    // --- Diagnostic builder ---

    #[test]
    fn diagnostic_new_seeds_from_code() {
        let d = Diagnostic::new(DiagnosticCode::E0100, Span::new(0, 4));
        assert_eq!(d.code, DiagnosticCode::E0100);
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.message, "unknown state reference");
        assert_eq!(d.span, Span::new(0, 4));
        assert!(d.related.is_empty());
    }

    #[test]
    fn diagnostic_builder_overrides() {
        let d = Diagnostic::new(DiagnosticCode::E0010, Span::empty(7))
            .with_message("expected '->', found identifier")
            .with_related(RelatedInfo {
                message: "rule starts here".into(),
                span: Span::new(0, 5),
            });
        assert_eq!(d.message, "expected '->', found identifier");
        assert_eq!(d.related.len(), 1);
        assert_eq!(d.related[0].span, Span::new(0, 5));
    }

    #[test]
    fn diagnostic_display_format() {
        let d = Diagnostic::new(DiagnosticCode::W0100, Span::new(2, 8));
        let s = d.to_string();
        assert!(s.contains("warning"));
        assert!(s.contains("FSM-W0100"));
        assert!(s.contains("history"));
    }

    // --- serde gated round trip ---

    #[cfg(feature = "serde")]
    #[test]
    fn diagnostic_serde_round_trip() {
        let d = Diagnostic::new(DiagnosticCode::E0100, Span::new(5, 9))
            .with_message("custom message")
            .with_related(RelatedInfo {
                message: "previous decl".into(),
                span: Span::new(0, 3),
            });
        let json = serde_json::to_string(&d).expect("serialise");
        let back: Diagnostic = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(d, back);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn span_serde_round_trip() {
        let s = Span::new(11, 17);
        let json = serde_json::to_string(&s).unwrap();
        let back: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
