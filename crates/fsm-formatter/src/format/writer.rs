//! Output buffer + indentation tracking.
//!
//! Methods are intentionally small and verb-shaped: every grammar rule
//! emits text by calling these. The writer enforces three invariants the
//! grammar emitters lean on:
//!
//! 1. **No trailing spaces** — `newline()` rstrips the current line before
//!    pushing `\n`. Doc 19 §3.4.
//! 2. **Exactly one terminal `\n`** — `finish()` ensures the buffer ends
//!    with a single `\n`. Doc 19 §3.3.
//! 3. **At most one consecutive blank line** outside intentional double-
//!    blank between major sections — `blank_line()` is idempotent against
//!    a buffer already ending in `\n\n`.

use crate::options::FormatOptions;

#[derive(Debug)]
pub struct FormatWriter<'opts> {
    buf: String,
    opts: &'opts FormatOptions,
    indent_level: usize,
    /// `true` when the next emission must first write the indent prefix.
    /// Set by `newline`, cleared by anything that writes non-whitespace.
    pending_indent: bool,
}

impl<'opts> FormatWriter<'opts> {
    pub fn new(opts: &'opts FormatOptions) -> Self {
        Self {
            buf: String::new(),
            opts,
            indent_level: 0,
            pending_indent: true,
        }
    }

    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    pub fn dedent(&mut self) {
        debug_assert!(self.indent_level > 0, "dedent below 0");
        self.indent_level = self.indent_level.saturating_sub(1);
    }

    /// Write raw text. If the line has not yet started, the indentation
    /// prefix is emitted first. Does NOT introduce its own newlines —
    /// callers control line breaks via `newline()`.
    pub fn write(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.maybe_indent();
        self.buf.push_str(s);
    }

    /// Single space.
    pub fn space(&mut self) {
        // Don't push a leading space at the start of a line — that would
        // bake a trailing-space risk into multi-line layouts. Indentation
        // is the column-management tool, not embedded spaces.
        if self.pending_indent {
            return;
        }
        if self.buf.ends_with(' ') {
            return;
        }
        self.buf.push(' ');
    }

    /// Pad with `n` spaces (used for column alignment of `->`). At the
    /// start of a fresh line the indent is emitted first; the padding
    /// applies after it.
    pub fn pad_spaces(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        self.maybe_indent();
        for _ in 0..n {
            self.buf.push(' ');
        }
    }

    /// End the current line. Strips trailing ASCII whitespace before
    /// pushing the `\n`; if the buffer is empty, no newline is written.
    pub fn newline(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        self.rstrip_line();
        self.buf.push('\n');
        self.pending_indent = true;
    }

    /// Ensure the buffer ends with a blank line (i.e. `\n\n`). Idempotent.
    pub fn blank_line(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        // Make sure we are at the start of a line.
        if !self.buf.ends_with('\n') {
            self.newline();
        }
        // Need exactly one extra `\n` so the buffer ends with `\n\n`.
        if !self.buf.ends_with("\n\n") {
            self.buf.push('\n');
        }
        self.pending_indent = true;
    }

    /// Current byte offset (used for arrow-alignment math). The
    /// pending-indent state means the *visible* column is the indent
    /// width — return that.
    pub fn column(&self) -> usize {
        if self.pending_indent {
            return self.indent_level * self.opts.indent_width as usize;
        }
        // Position relative to the most recent newline.
        match self.buf.rfind('\n') {
            Some(nl) => self.buf.len() - nl - 1,
            None => self.buf.len(),
        }
    }

    /// Drop trailing whitespace on the current line (no `\n` push).
    fn rstrip_line(&mut self) {
        while self.buf.ends_with(' ') || self.buf.ends_with('\t') {
            self.buf.pop();
        }
    }

    fn maybe_indent(&mut self) {
        if self.pending_indent {
            for _ in 0..self.indent_level {
                self.buf.push_str(&self.opts.indent_unit());
            }
            self.pending_indent = false;
        }
    }

    /// Consume the writer, returning the formatted text. Guarantees a
    /// single trailing `\n` per Doc 19 §3.3.
    pub fn finish(mut self) -> String {
        // Strip trailing whitespace + multiple `\n`, then re-add exactly
        // one terminator.
        while self
            .buf
            .ends_with(|c: char| c.is_ascii_whitespace() && c != '\n')
        {
            self.buf.pop();
        }
        while self.buf.ends_with('\n') {
            self.buf.pop();
        }
        // Empty output (e.g. empty input) still gets the terminator.
        self.buf.push('\n');
        self.buf
    }

    /// Hand back a read-only view of the in-progress buffer. Used by the
    /// trivia pre-pass that looks at recent output to decide whether to
    /// re-emit a comment.
    pub fn buf(&self) -> &str {
        &self.buf
    }
}
