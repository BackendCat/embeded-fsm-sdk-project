//! `textDocument/publishDiagnostics` projection — Doc 26 §4.2 (L1 MVP).
//!
//! Pure, total projection of an [`fsm_diagnostics::Diagnostic`] onto an
//! [`tower_lsp::lsp_types::Diagnostic`]. **No analysis happens here** — the
//! diagnostics are produced by the exact `fsm check` pipeline in
//! [`crate::analysis`]; this module only maps byte-`Span`s to LSP `Range`s
//! (via [`LineIndex`], the negotiated encoding) and the severity/code/
//! related-info fields one-to-one.
//!
//! Severity mapping is deliberately faithful to LSP (Error=1, Warning=2,
//! Info=3, Hint=4) and does **not** collapse Info/Hint the way the
//! `miette` integration does (`fsm-diagnostics` `lib.rs` maps both to
//! `Advice`) — Doc 05 §1.4.3 wants Hint as a distinct grey squiggle, so
//! Doc 26 §4.2 explicitly forbids collapsing them in the LSP layer.

use tower_lsp::lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location,
    NumberOrString, Range, Url,
};

use fsm_diagnostics::{Diagnostic, Severity};

use crate::position::{LineIndex, OffsetEncoding};

/// Map `fsm-diagnostics` severity to the LSP severity. One-to-one; Hint is
/// preserved as a distinct bucket (Doc 26 §4.2 — do NOT fold into Info).
fn severity(sev: Severity) -> DiagnosticSeverity {
    match sev {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
        Severity::Hint => DiagnosticSeverity::HINT,
    }
}

/// Project one pipeline diagnostic onto an LSP diagnostic.
///
/// `uri` + `text` + `line_index` belong to the document the diagnostic was
/// produced for; `encoding` is the negotiated `positionEncoding`. The
/// `code` is the stable `FSM-XNNNN` wire string (so an editor can group /
/// link by it, matching `fsm check --json`'s `code` field exactly).
pub fn to_lsp_diagnostic(
    d: &Diagnostic,
    uri: &Url,
    text: &str,
    line_index: &LineIndex,
    encoding: OffsetEncoding,
) -> LspDiagnostic {
    let range: Range = line_index.range(text, d.span, encoding);
    let related: Vec<DiagnosticRelatedInformation> = d
        .related
        .iter()
        .map(|r| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: line_index.range(text, r.span, encoding),
            },
            message: r.message.clone(),
        })
        .collect();
    LspDiagnostic {
        range,
        severity: Some(severity(d.severity)),
        // `format!("{}", code)` is the `FSM-XNNNN` wire form — identical to
        // the `--json` formatter's `code` field (`check.rs:106`).
        code: Some(NumberOrString::String(format!("{}", d.code))),
        code_description: None,
        source: Some("fsm".to_owned()),
        message: d.message.clone(),
        related_information: if related.is_empty() {
            None
        } else {
            Some(related)
        },
        tags: None,
        data: None,
    }
}

/// Project a whole diagnostic batch (one document's worth).
pub fn to_lsp_diagnostics(
    diags: &[Diagnostic],
    uri: &Url,
    text: &str,
    line_index: &LineIndex,
    encoding: OffsetEncoding,
) -> Vec<LspDiagnostic> {
    diags
        .iter()
        .map(|d| to_lsp_diagnostic(d, uri, text, line_index, encoding))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{DiagnosticCode, RelatedInfo, Span};

    #[test]
    fn severity_does_not_collapse_hint_into_info() {
        // Doc 26 §4.2: the miette impl folds Hint->Advice; the LSP MUST
        // keep them distinct (Hint=4, Info=3).
        assert_eq!(severity(Severity::Info), DiagnosticSeverity::INFORMATION);
        assert_eq!(severity(Severity::Hint), DiagnosticSeverity::HINT);
        assert_ne!(severity(Severity::Info), severity(Severity::Hint));
    }

    #[test]
    fn code_is_the_fsm_wire_string() {
        let src = "language fsm 2.0\nmachine M {}";
        let idx = LineIndex::new(src);
        let uri = Url::parse("file:///t.fsm").unwrap();
        let d = Diagnostic::new(DiagnosticCode::E0107, Span::new(17, 24));
        let lsp = to_lsp_diagnostic(&d, &uri, src, &idx, OffsetEncoding::Utf8);
        assert_eq!(
            lsp.code,
            Some(NumberOrString::String("FSM-E0107".to_owned()))
        );
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(lsp.source.as_deref(), Some("fsm"));
        assert_eq!(
            lsp.range.start,
            tower_lsp::lsp_types::Position {
                line: 1,
                character: 0
            }
        );
    }

    #[test]
    fn related_info_is_projected_with_its_own_range() {
        let src = "machine A {}\nmachine A {}";
        let idx = LineIndex::new(src);
        let uri = Url::parse("file:///t.fsm").unwrap();
        let d =
            Diagnostic::new(DiagnosticCode::E0020, Span::new(21, 22)).with_related(RelatedInfo {
                message: "previous declaration".into(),
                span: Span::new(8, 9),
            });
        let lsp = to_lsp_diagnostic(&d, &uri, src, &idx, OffsetEncoding::Utf8);
        let rel = lsp.related_information.expect("related present");
        assert_eq!(rel.len(), 1);
        assert_eq!(rel[0].location.range.start.line, 0);
        assert_eq!(rel[0].message, "previous declaration");
    }
}
