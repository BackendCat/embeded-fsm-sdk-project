//! Formatter configuration.
//!
//! Doc 19 §2 only exposes `indent-size` and `bracket-style` as configurable.
//! The implementation surface adds two non-controversial knobs that the spec
//! left as fixed-by-implementation:
//!
//! - `trailing_comma` — Doc 04 §2.4 already accepts a trailing comma in
//!   `enum_decl`; the formatter defaults to emitting one in multi-line
//!   lists for cleaner version-control diffs. Set `false` to skip.
//! - `align_arrows` — Doc 19 §10 mandates vertical `->` alignment across
//!   consecutive transitions. Set `false` to fall back to single-space
//!   separation (useful when running the formatter under tools that diff
//!   poorly on column shifts).
//!
//! `blank_line_between_decls` toggles Doc 19 §3.6's
//! "one blank line between top-level declarations" — defaults `true`.
//! `max_line_width` is advisory today (the formatter doesn't break call-
//! argument lists yet) but is reserved for the multi-line fork/join wrap
//! at Doc 19 §13.3 / §13.4.

/// Tunables for [`crate::format`] and [`crate::format_string`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    /// Spaces (or tab repetitions, see `use_tabs`) per indent level.
    /// Default per Doc 19 §2 = `4`.
    pub indent_width: u8,
    /// Indent with hard tabs (`\t`) instead of spaces. Default `false`.
    pub use_tabs: bool,
    /// Soft target for line width. Reserved — Doc 19 §3.5 says "no hard
    /// limit"; the formatter uses this only for multi-line fork/join
    /// wrapping (Doc 19 §13.3 / §13.4). Default `100`.
    pub max_line_width: u16,
    /// Emit a trailing comma in multi-line bracketed lists (enums, fork
    /// targets, join sources, etc.). Default `true`.
    pub trailing_comma: bool,
    /// Insert one blank line between top-level declarations and between
    /// machine sections. Default `true`.
    pub blank_line_between_decls: bool,
    /// Align the `->` columns within a sibling group of `on …` / `done`
    /// / `after` / `every` transitions. Default `true`.
    pub align_arrows: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 4,
            use_tabs: false,
            max_line_width: 100,
            trailing_comma: true,
            blank_line_between_decls: true,
            align_arrows: true,
        }
    }
}

impl FormatOptions {
    /// One-line literal of the indentation unit. `String` rather than
    /// `&'static str` because the count varies; the result is short and
    /// reuse across the formatter is cheap.
    pub fn indent_unit(&self) -> String {
        if self.use_tabs {
            "\t".to_string()
        } else {
            " ".repeat(self.indent_width as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_doc_19() {
        let o = FormatOptions::default();
        assert_eq!(o.indent_width, 4);
        assert!(!o.use_tabs);
        assert_eq!(o.indent_unit(), "    ");
    }

    #[test]
    fn tabs_swap_unit() {
        let o = FormatOptions {
            use_tabs: true,
            ..Default::default()
        };
        assert_eq!(o.indent_unit(), "\t");
    }
}
