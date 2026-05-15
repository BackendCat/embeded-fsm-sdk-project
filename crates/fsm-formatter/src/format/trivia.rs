//! Comment + blank-line preservation utilities.
//!
//! The CST stores trivia (whitespace, comments) as leaf tokens *between*
//! non-trivia tokens. To stay idempotent, the formatter must:
//!
//! - Preserve every comment, in the same vertical position relative to
//!   the declaration it annotates (Doc 19 §14, §1.3).
//! - Collapse runs of `\n` to at most a double blank line (Doc 19 §3.7).
//! - Preserve a *single* user-authored blank line between groups of items
//!   inside a state body or list (Doc 19 §1.5 "minimal diff" principle).
//! - Never emit trailing whitespace on a line (Doc 19 §3.4).
//!
//! This module is a small toolbox of pre/post-trivia helpers used by the
//! grammar-emitter modules. They never decide *whether* a declaration is
//! emitted, only how to insert any leading or trailing trivia around it.

use fsm_parser::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::NodeOrToken;

use super::writer::FormatWriter;

/// Emit a comment token verbatim, indented at the current writer level.
/// Block comments containing embedded newlines are emitted with each
/// continuation line at the same indent (raw text re-emit; we don't
/// reflow block-comment internals per Doc 19 §14.1).
pub(crate) fn emit_comment(w: &mut FormatWriter, tok: &SyntaxToken) {
    match tok.kind() {
        SyntaxKind::LineComment | SyntaxKind::DocComment => {
            w.write(tok.text().trim_end());
            w.newline();
        }
        SyntaxKind::BlockComment => {
            // Block comments may span multiple lines. Re-indent the
            // continuation lines so the comment sits flush with the
            // current indent level. We only strip *trailing* whitespace
            // on each continuation; the interior columns of the comment
            // body are user-controlled per Doc 19 §14 ("content preserved
            // verbatim").
            let text = tok.text();
            let mut lines = text.split('\n');
            if let Some(first) = lines.next() {
                w.write(first.trim_end());
            }
            for line in lines {
                w.newline();
                // The first character on the continuation line should be
                // ' ' or '*' or end-of-comment. We preserve the user's
                // leading column past our re-indent unchanged.
                w.write(line.trim_end_matches(|c: char| c == '\r'));
            }
            w.newline();
        }
        _ => {
            // Whitespace / Newline trivia handled by caller. Defensive
            // fall-through preserves whatever text the parser emitted.
            w.write(tok.text());
        }
    }
}

/// One "decision" produced by [`body_events_with_trailing`] — the
/// caller emits each node it sees while the walker hands back the trivia
/// leaves in order so the body formatter can decide how to integrate
/// them.
#[derive(Debug, Clone)]
pub(crate) enum BodyEvent {
    /// A trivia token. The walker has already classified it: `kind` is
    /// one of `Newline`, `Whitespace`, `LineComment`, `BlockComment`,
    /// `DocComment`.
    Trivia(SyntaxToken),
    /// A real node — the caller emits its content. The walker has not
    /// yet emitted anything related to this node.
    Node(SyntaxNode),
}

/// Reconstructs the trivia *gaps between sibling decls*. Trailing
/// trivia is "stuck" inside the previous
/// declaration's subtree (rowan attaches it to the last leaf's parent
/// rather than promoting it to a sibling), so we recover it by:
///
/// 1. Computing the *meaningful* range of each declaration — its first
///    non-trivia token through its last non-trivia token.
/// 2. Collecting all trivia tokens in source order whose start byte
///    falls in the gap `(prev.meaningful_end, next.meaningful_start)`.
///
/// This pulls trailing line/block/doc comments and blank-line separators
/// out of their syntactic parent and surfaces them between sibling
/// declarations, which is where they MORALLY belong (Doc 19 §14 talks
/// about "comments preceding a declaration" — the user wrote them in the
/// gap, not inside the previous node).
///
/// Properties:
///
/// - Trivia tokens are returned in source order.
/// - Trivia INSIDE a declaration's *meaningful* span (e.g. blank lines
///   inside a state body) is NOT returned — it stays the inner body
///   emitter's concern.
pub(crate) fn body_events_with_trailing(parent: &SyntaxNode) -> Vec<BodyEvent> {
    let nodes: Vec<SyntaxNode> = parent.children().collect();
    let parent_start: usize = parent.text_range().start().into();
    // The body's "interesting" end is the position of the parent's closing
    // delimiter (`}`) if any. Trailing trivia BEYOND the `}` belongs to
    // the outer body, NOT this one — counting it would conjure phantom
    // blank lines after `}`. Find the byte offset of the closing brace,
    // or fall back to the parent's range end (e.g. the `FILE` node has no
    // brace).
    let parent_end: usize = parent
        .children_with_tokens()
        .filter_map(|c| match c {
            rowan::NodeOrToken::Token(t) if t.kind() == SyntaxKind::RBrace => {
                Some(usize::from(t.text_range().start()))
            }
            _ => None,
        })
        .last()
        .unwrap_or_else(|| usize::from(parent.text_range().end()));

    // Per-node meaningful range — `[first_non_trivia.start,
    // last_non_trivia.end]`. If the node has no non-trivia leaf, fall
    // back to the node's full text range so we don't double-count.
    let meaningful: Vec<(usize, usize)> = nodes
        .iter()
        .map(|n| {
            meaningful_range(n).unwrap_or_else(|| {
                (
                    usize::from(n.text_range().start()),
                    usize::from(n.text_range().end()),
                )
            })
        })
        .collect();

    let mut all_trivia: Vec<SyntaxToken> = parent
        .descendants_with_tokens()
        .filter_map(|c| match c {
            NodeOrToken::Token(t) if t.kind().is_trivia() => Some(t),
            _ => None,
        })
        .collect();
    all_trivia.sort_by_key(|t| usize::from(t.text_range().start()));

    let mut events = Vec::new();
    let mut cursor = parent_start;
    let mut t_idx = 0usize;

    for (i, node) in nodes.iter().enumerate() {
        let (m_start, m_end) = meaningful[i];
        // Pre-node gap: trivia between cursor and the meaningful start
        // of this node. (The cursor advances to the previous node's
        // meaningful_end so trivia attached to the previous node — its
        // trailing trivia — falls into THIS pre-node gap.)
        while t_idx < all_trivia.len() {
            let t = &all_trivia[t_idx];
            let ts: usize = t.text_range().start().into();
            if ts >= m_start {
                break;
            }
            if ts >= cursor {
                events.push(BodyEvent::Trivia(t.clone()));
            }
            t_idx += 1;
        }
        events.push(BodyEvent::Node(node.clone()));
        // Skip any trivia INSIDE this node's meaningful span (the node
        // owns it).
        while t_idx < all_trivia.len() {
            let t = &all_trivia[t_idx];
            let ts: usize = t.text_range().start().into();
            if ts >= m_end {
                break;
            }
            t_idx += 1;
        }
        cursor = m_end;
    }
    // Trailing gap.
    while t_idx < all_trivia.len() {
        let t = &all_trivia[t_idx];
        let ts: usize = t.text_range().start().into();
        let te: usize = t.text_range().end().into();
        if ts >= cursor && te <= parent_end {
            events.push(BodyEvent::Trivia(t.clone()));
        }
        t_idx += 1;
    }
    events
}

/// `[first_non_trivia.start, last_non_trivia.end]` of `node`. Returns
/// `None` if the node contains only trivia (which shouldn't happen for
/// real declarations).
fn meaningful_range(node: &SyntaxNode) -> Option<(usize, usize)> {
    let first = node.descendants_with_tokens().find_map(|c| match c {
        NodeOrToken::Token(t) if !t.kind().is_trivia() => Some(t),
        _ => None,
    })?;
    let last = node
        .descendants_with_tokens()
        .filter_map(|c| match c {
            NodeOrToken::Token(t) if !t.kind().is_trivia() => Some(t),
            _ => None,
        })
        .last()?;
    Some((
        usize::from(first.text_range().start()),
        usize::from(last.text_range().end()),
    ))
}
