//! Byte-offset `Span` <-> LSP `Position`/`Range` — the encoding boundary.
//!
//! Doc 26 §4.1 (risk-1, HIGH) is explicit: `fsm_diagnostics::Span` is a
//! **byte-offset** half-open range, and there are already **two** divergent,
//! non-LSP-correct byte->line/col converters in-tree —
//! `fsm_analyzer::util::compute_line_col` (counts *bytes*, 1-based) and
//! `fsm_cli::cmd::check::line_col` (counts *Unicode scalars*, 1-based).
//! LSP 3.17 positions are **0-based** line + **0-based** `character`, where
//! `character` is measured in the *negotiated* `positionEncoding` unit.
//! Conflating either existing impl would produce off-by-one (0 vs 1 base)
//! and silently-wrong columns on any non-ASCII / non-BMP line.
//!
//! This module owns the **one** correct converter for the LSP. It does NOT
//! reuse — and is deliberately not shared with — the two existing impls:
//! converging those would change analyzer/CLI behaviour (their 1-based,
//! byte-or-scalar contract is baked into IR `SourceLocation` and the
//! `--json`/human renderer, and is asserted by their own tests). That
//! convergence is tracked separately as DRIFT-2 (Doc 00 §11.32); widening
//! L1's scope to touch it would risk a non-behaviour-neutral change to two
//! shipped subsystems, which the project bar forbids. `position.rs` is
//! therefore *authoritative for the LSP only*.
//!
//! ## `LineIndex`
//!
//! Built once per analysis: a `Vec<u32>` of line-start byte offsets.
//! `byte -> line` is a binary search; the intra-line column is then encoded
//! in the negotiated unit (UTF-8 byte count, or UTF-16 code-unit count).
//! This is the rust-analyzer model: O(n) build, O(log n) lookup.
//!
//! ## Encoding negotiation (Doc 26 §4.1 decision)
//!
//! The server advertises `["utf-8", "utf-16"]` and **defaults to UTF-8**.
//! Under UTF-8 an LSP `character` is a *byte* column, so it maps directly
//! onto `Span` byte offsets with only a line-start lookup — the entire
//! UTF-16 transcoding class is eliminated. UTF-16 is the mandated fallback
//! for clients that do not support the 3.17 `positionEncoding` capability.

use tower_lsp::lsp_types::{Position, PositionEncodingKind, Range};

use fsm_diagnostics::Span;

/// The position encoding negotiated with the client in `initialize`.
///
/// LSP 3.17 lets the client and server agree on how the `character` field
/// of a `Position` is counted. We support the two the spec defines that a
/// byte-offset substrate can serve losslessly; UTF-32 is intentionally not
/// advertised (no in-tree consumer needs it and it is the rarest client
/// capability).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OffsetEncoding {
    /// `character` counts UTF-8 bytes. The server's preferred default —
    /// makes the byte-offset `Span` map onto LSP positions with no
    /// transcoding (Doc 26 §4.1).
    Utf8,
    /// `character` counts UTF-16 code units. The LSP 3.16-and-earlier
    /// default; the mandated fallback for clients without the 3.17
    /// `general.positionEncodings` capability.
    Utf16,
}

impl OffsetEncoding {
    /// Map an LSP `PositionEncodingKind` string to our enum, falling back
    /// to UTF-16 for anything we did not advertise (defensive: the client
    /// must echo back one of the values we offered, but an out-of-spec
    /// client gets the universally-correct UTF-16 path rather than a
    /// silently-wrong UTF-8 assumption).
    pub fn from_lsp(kind: &PositionEncodingKind) -> Self {
        if *kind == PositionEncodingKind::UTF8 {
            OffsetEncoding::Utf8
        } else {
            OffsetEncoding::Utf16
        }
    }

    /// The wire value to put in `ServerCapabilities.position_encoding`.
    pub fn to_lsp(self) -> PositionEncodingKind {
        match self {
            OffsetEncoding::Utf8 => PositionEncodingKind::UTF8,
            OffsetEncoding::Utf16 => PositionEncodingKind::UTF16,
        }
    }
}

/// Precomputed line-start table for one source buffer.
///
/// `line_starts[i]` is the byte offset of the first character of line `i`
/// (0-based line numbering, as LSP requires). `line_starts[0]` is always
/// `0`. A trailing `\n` produces a final (possibly empty) line entry so a
/// position at end-of-file is representable.
#[derive(Clone, Debug)]
pub struct LineIndex {
    /// Byte offset of the start of each line. Strictly increasing.
    line_starts: Vec<u32>,
    /// Total length of the indexed source in bytes — clamps out-of-range
    /// offsets (a `Span` past EOF must still yield a valid position rather
    /// than panic; resilient-parser inputs can produce such spans).
    len: u32,
}

impl LineIndex {
    /// Build the index for `text`. O(n) single pass.
    pub fn new(text: &str) -> Self {
        let mut line_starts = Vec::with_capacity(text.len() / 24 + 1);
        line_starts.push(0u32);
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                // Next line starts on the byte *after* the '\n'. `i` is a
                // byte index into a `&str`, so it fits a u32 for any file
                // the DoS-bounded parser accepts (Doc 26 §4.4).
                line_starts.push(i as u32 + 1);
            }
        }
        LineIndex {
            line_starts,
            len: text.len() as u32,
        }
    }

    /// Convert a byte offset into an LSP `Position` in `encoding`.
    ///
    /// `text` MUST be the exact buffer this index was built from — the
    /// intra-line slice is re-scanned from it to count code units. Offsets
    /// past EOF clamp to the end of the last line (never panics — the
    /// resilient parser can hand us a span past the buffer on broken
    /// input, Doc 26 §4.4 / risk-3).
    pub fn position(&self, text: &str, byte: u32, encoding: OffsetEncoding) -> Position {
        let byte = byte.min(self.len);
        // `partition_point` gives the count of line-starts <= byte; minus
        // one is the 0-based line. There is always at least one entry (0).
        let line = self.line_starts.partition_point(|&start| start <= byte) - 1;
        let line_start = self.line_starts[line];
        // The intra-line byte slice [line_start, byte). `byte` is clamped
        // to `len`; `line_start <= byte` holds by construction, and both
        // are valid char boundaries (line_start follows a '\n'; `byte`
        // comes from a `Span` produced over the same `&str`).
        let segment = &text[line_start as usize..byte as usize];
        let character = match encoding {
            OffsetEncoding::Utf8 => segment.len() as u32,
            OffsetEncoding::Utf16 => segment.chars().map(|c| c.len_utf16() as u32).sum(),
        };
        Position {
            line: line as u32,
            character,
        }
    }

    /// Project a byte-offset [`Span`] onto an LSP [`Range`]. Total and
    /// mechanical (Doc 26 §4.2): no analysis, pure projection. An inverted
    /// span (`end < start`, which `Span` permits) is normalised so the LSP
    /// `Range` is always well-formed.
    pub fn range(&self, text: &str, span: Span, encoding: OffsetEncoding) -> Range {
        let (lo, hi) = if span.end >= span.start {
            (span.start, span.end)
        } else {
            (span.end, span.start)
        };
        Range {
            start: self.position(text, lo as u32, encoding),
            end: self.position(text, hi as u32, encoding),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_single_line_utf8_is_zero_based() {
        let src = "machine M {}";
        let idx = LineIndex::new(src);
        // byte 0 -> line 0, char 0 (LSP is 0-based, unlike the two
        // existing 1-based impls — the §4.1 off-by-one guard).
        let p = idx.position(src, 0, OffsetEncoding::Utf8);
        assert_eq!(
            p,
            Position {
                line: 0,
                character: 0
            }
        );
        let p = idx.position(src, 8, OffsetEncoding::Utf8);
        assert_eq!(
            p,
            Position {
                line: 0,
                character: 8
            }
        );
    }

    #[test]
    fn multi_line_byte_to_line_col() {
        let src = "language fsm 2.0\nmachine M {\n}\n";
        let idx = LineIndex::new(src);
        // First byte of line 1 ('m' of machine) is offset 17.
        let p = idx.position(src, 17, OffsetEncoding::Utf8);
        assert_eq!(
            p,
            Position {
                line: 1,
                character: 0
            }
        );
        // The '{' on line 1 is offset 27 -> column 10.
        let p = idx.position(src, 27, OffsetEncoding::Utf8);
        assert_eq!(
            p,
            Position {
                line: 1,
                character: 10
            }
        );
    }

    #[test]
    fn non_ascii_line_utf8_counts_bytes_utf16_counts_code_units() {
        // A 4-byte non-BMP scalar (🚀, U+1F680) inside a doc comment, then
        // a newline, then the token whose column must be correct on line 1.
        // Under UTF-8 the LSP `character` is the *byte* count; under UTF-16
        // it is the code-unit count (🚀 = 2 UTF-16 units). A naive
        // byte-as-character OR scalar-as-character shim gets exactly one of
        // these wrong — this asserts both, which is the whole risk-1 proof.
        let src = "/// 🚀\nmachine M {}";
        let idx = LineIndex::new(src);
        // Byte layout: '/'(1)+'/'(1)+'/'(1)+' '(1)+🚀(4) = 8 bytes for
        // line 0's content; the '\n' is byte 8; line 1 ("machine…")
        // starts at byte 9. `str::find` returns a *byte* offset, so this
        // is 9 — the line-1 start.
        let line1_machine = src.find("machine").unwrap() as u32;
        assert_eq!(line1_machine, 9);
        let p8 = idx.position(src, line1_machine, OffsetEncoding::Utf8);
        assert_eq!(
            p8,
            Position {
                line: 1,
                character: 0
            }
        );
        let p16 = idx.position(src, line1_machine, OffsetEncoding::Utf16);
        assert_eq!(
            p16,
            Position {
                line: 1,
                character: 0
            }
        );

        // A position WITHIN the non-ASCII line: byte 8 = end of line 0
        // (just past 🚀, where the '\n' sits). UTF-8: the segment is 8
        // bytes -> character 8. UTF-16: '/'×3 + ' ' + 🚀(2 units) = 6.
        // A scalar count would say 5. All three differ — this is exactly
        // the divergence the two in-tree (byte / scalar, 1-based) impls
        // get wrong and `position.rs` must get right.
        let p8 = idx.position(src, 8, OffsetEncoding::Utf8);
        assert_eq!(
            p8,
            Position {
                line: 0,
                character: 8
            }
        );
        let p16 = idx.position(src, 8, OffsetEncoding::Utf16);
        assert_eq!(
            p16,
            Position {
                line: 0,
                character: 6
            }
        );
    }

    #[test]
    fn cyrillic_line_both_encodings() {
        // Cyrillic 'ы' is 2 bytes UTF-8, 1 UTF-16 unit. A string literal
        // with Cyrillic before an error column is the realistic case from
        // Doc 26 §4.1.
        let src = "// мышь\nmachine M {}";
        let idx = LineIndex::new(src);
        // "// мышь": '/'+'/'+' ' = 3 bytes, then м(2)ы(2)ш(2)ь(2) = 8
        // bytes -> end of line 0 at byte 11.
        let nl = src.find('\n').unwrap();
        assert_eq!(nl, 11);
        let p8 = idx.position(src, 11, OffsetEncoding::Utf8);
        assert_eq!(
            p8,
            Position {
                line: 0,
                character: 11
            }
        );
        // UTF-16: 3 ASCII + 4 Cyrillic (1 unit each) = 7.
        let p16 = idx.position(src, 11, OffsetEncoding::Utf16);
        assert_eq!(
            p16,
            Position {
                line: 0,
                character: 7
            }
        );
    }

    #[test]
    fn offset_past_eof_clamps_does_not_panic() {
        let src = "machine M {}";
        let idx = LineIndex::new(src);
        // A resilient-parser span can point past the buffer on broken
        // input (Doc 26 §4.4). Must clamp, not panic.
        let p = idx.position(src, 9999, OffsetEncoding::Utf8);
        assert_eq!(
            p,
            Position {
                line: 0,
                character: 12
            }
        );
    }

    #[test]
    fn span_to_range_projection_and_inverted_span_normalised() {
        let src = "language fsm 2.0\nmachine M {}";
        let idx = LineIndex::new(src);
        let r = idx.range(src, Span::new(17, 24), OffsetEncoding::Utf8);
        assert_eq!(
            r.start,
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(
            r.end,
            Position {
                line: 1,
                character: 7
            }
        );
        // Inverted span must still yield a well-formed (lo<=hi) Range.
        let r2 = idx.range(src, Span::new(24, 17), OffsetEncoding::Utf8);
        assert_eq!(
            r2.start,
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(
            r2.end,
            Position {
                line: 1,
                character: 7
            }
        );
    }

    #[test]
    fn trailing_newline_yields_final_line() {
        let src = "machine M {}\n";
        let idx = LineIndex::new(src);
        // Offset at EOF (after the '\n') is line 1, char 0.
        let p = idx.position(src, 13, OffsetEncoding::Utf8);
        assert_eq!(
            p,
            Position {
                line: 1,
                character: 0
            }
        );
    }
}
