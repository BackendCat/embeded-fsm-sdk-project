//! Byte-offset `Span` <-> LSP `Position`/`Range` — the encoding boundary.
//!
//! Doc 26 §4.1 (risk-1, HIGH) is explicit: `fsm_diagnostics::Span` is a
//! **byte-offset** half-open range, and the project has **three
//! intentionally-different** byte->line/col contracts (Doc 00 §11.32,
//! DRIFT-2):
//!
//! 1. the IR `SourceLocation` (`fsm-analyzer`) — **bytes, 1-based** (baked
//!    into the deterministic C / `lower_split_byte_identity` fingerprint);
//! 2. `fsm check`'s `--json` + human `line:col` (`fsm-cli`) — **Unicode
//!    scalars, 1-based** (asserted by the CLI tests);
//! 3. the LSP — **0-based**, `character` in the *negotiated*
//!    `positionEncoding` unit (UTF-8 byte or UTF-16 code unit).
//!
//! Contracts (1) and (2) were two hand-rolled duplicate loops; the DRIFT-2
//! convergence wave folded their *shared linear-scan mechanic* into the one
//! core `fsm_diagnostics::compute_line_col(src, pos, LineColUnit)` — each
//! caller now invokes it with its existing unit (`Byte` / `Scalar`), proven
//! byte-identical, so the duplicate loops are gone but every downstream
//! contract is unchanged.
//!
//! This module owns contract (3) and **deliberately does NOT build on that
//! converged core**. `LineIndex` is a *structurally different algorithm* —
//! a precomputed `Vec<u32>` line-start table giving O(log n)
//! `partition_point` lookup (the rust-analyzer model, mandated for the
//! editor hot path, Doc 26 §4.1), 0-based, with a negotiated UTF-8/UTF-16
//! unit, an inverse (`offset`), span->`Range` projection and EOF clamping —
//! none of which the linear core provides. Forcing the index onto a linear
//! scan would regress its complexity and risk the §5.4 byte-exact `Range`
//! contract for zero benefit; conflating it with the 1-based core would also
//! reintroduce the off-by-one + silently-wrong-column class on any
//! non-ASCII / non-BMP line. It shares only the *conceptual* line-start
//! scan with the converged core, never its loop. `position.rs` is therefore
//! *authoritative for the LSP only* (Doc 00 §11.32 boundary, upheld).
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

    /// Convert an LSP [`Position`] back to a **byte offset** into `text`.
    ///
    /// The exact inverse of [`Self::position`] — it walks the *same*
    /// `LineIndex` and decodes the intra-line `character` in the *same*
    /// negotiated unit, so a position the server emitted round-trips to the
    /// byte it came from. This is **not** a second/divergent converter (the
    /// §11.32 DRIFT-2 boundary): it is the reverse direction of the one
    /// authoritative `LineIndex`, required by L3's token-at-cursor lookup
    /// (`textDocument/hover`/`definition` receive a `Position` and must find
    /// the byte to locate the CST token there). Out-of-range input is
    /// clamped, never panics (a client may send a stale position against a
    /// newer buffer): a line past EOF clamps to the buffer end; a
    /// `character` past the line's content clamps to the line end (the
    /// line's terminating `\n`, or EOF on the last line). Returned offset is
    /// always a valid `char` boundary of `text`.
    pub fn offset(&self, text: &str, pos: Position, encoding: OffsetEncoding) -> u32 {
        let line = pos.line as usize;
        if line >= self.line_starts.len() {
            return self.len;
        }
        let line_start = self.line_starts[line];
        // Exclusive end of this line's *content* (the byte index of the
        // terminating '\n', or `len` for the final line) — the cursor can
        // legitimately sit at end-of-line and must not bleed into the next.
        let line_end = self
            .line_starts
            .get(line + 1)
            .map(|&next| next - 1) // strip the '\n' that started `next`
            .unwrap_or(self.len);
        let want = pos.character;
        let mut units = 0u32;
        let mut byte = line_start;
        // Walk the line's chars, accumulating code units in the negotiated
        // encoding, until we have consumed `want` units (or hit line end).
        for ch in text[line_start as usize..line_end as usize].chars() {
            if units >= want {
                break;
            }
            let u = match encoding {
                OffsetEncoding::Utf8 => ch.len_utf8() as u32,
                OffsetEncoding::Utf16 => ch.len_utf16() as u32,
            };
            units += u;
            byte += ch.len_utf8() as u32;
        }
        byte.min(line_end)
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
    fn offset_is_the_exact_inverse_of_position_ascii() {
        let src = "language fsm 2.0\nmachine M {\n  state S {}\n}\n";
        let idx = LineIndex::new(src);
        // Every byte that is a char boundary must round-trip
        // position->offset under BOTH encodings (ASCII: identical).
        for b in 0..=src.len() as u32 {
            if !src.is_char_boundary(b as usize) {
                continue;
            }
            for enc in [OffsetEncoding::Utf8, OffsetEncoding::Utf16] {
                let p = idx.position(src, b, enc);
                assert_eq!(
                    idx.offset(src, p, enc),
                    b,
                    "byte {b} must round-trip via position->offset under {enc:?}"
                );
            }
        }
    }

    #[test]
    fn offset_inverse_on_non_ascii_line_both_encodings() {
        // Cyrillic 'ы' (2 bytes / 1 UTF-16 unit) and 🚀 (4 bytes / 2 UTF-16
        // units) in a comment, then an ASCII token. The cursor on `machine`
        // and on `M` must map back to the right byte under EACH encoding —
        // a naive byte-as-character (or scalar) inverse fails one of these.
        let src = "// ы 🚀\nmachine M {}";
        let idx = LineIndex::new(src);
        let m_byte = src.find("machine").unwrap() as u32; // line-1 start
        let big_m = src.find(" M ").unwrap() as u32 + 1; // the `M` ident
        for enc in [OffsetEncoding::Utf8, OffsetEncoding::Utf16] {
            // Round-trip the two ASCII tokens on the line AFTER the
            // multibyte comment — the line-start lookup is encoding-stable
            // but a wrong intra-line inverse would still corrupt these.
            let pm = idx.position(src, m_byte, enc);
            assert_eq!(idx.offset(src, pm, enc), m_byte, "[{enc:?}] `machine`");
            let pbm = idx.position(src, big_m, enc);
            assert_eq!(idx.offset(src, pbm, enc), big_m, "[{enc:?}] ident `M`");
            // A position WITHIN the multibyte line round-trips to the
            // boundary byte too. Byte layout of line 0: '/'(0) '/'(1)
            // ' '(2) 'ы'(3..5, 2 bytes) ' '(5) '🚀'(6..10, 4 bytes).
            // Byte 6 (the '🚀' start, just past `// ы `) is a char
            // boundary AFTER the 2-byte Cyrillic — a naive scalar/byte
            // inverse would map the corresponding position elsewhere.
            assert!(src.is_char_boundary(6));
            let p6 = idx.position(src, 6, enc);
            assert_eq!(idx.offset(src, p6, enc), 6, "[{enc:?}] mid-comment");
        }
    }

    #[test]
    fn offset_clamps_out_of_range_position_no_panic() {
        let src = "machine M {}\n";
        let idx = LineIndex::new(src);
        // Line past EOF -> buffer end.
        let far = idx.offset(
            src,
            Position {
                line: 999,
                character: 0,
            },
            OffsetEncoding::Utf8,
        );
        assert_eq!(far, src.len() as u32);
        // Character past the line content -> clamped to line end (the '\n'
        // at byte 12), NOT bleeding into the next line.
        let past_col = idx.offset(
            src,
            Position {
                line: 0,
                character: 9999,
            },
            OffsetEncoding::Utf8,
        );
        assert_eq!(past_col, 12, "clamp to end of line-0 content (before \\n)");
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
