//! Top-level grammar entry — `parse_file`.
//!
//! Per Doc 04 §2:
//!
//! ```ebnf
//! file = language_decl , { import_decl } , { top_level_decl } ;
//! top_level_decl =
//!       feature_decl
//!     | const_decl
//!     | enum_decl
//!     | extern_decl
//!     | machine_decl ;
//! ```
//!
//! The `language fsm X.Y` header is required by the spec but the parser
//! treats it as optional: a missing header produces a diagnostic (E0010)
//! but the parser continues so a user editing the body of their file in an
//! LSP setting still sees structure / completions.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::parser::Parser;
use crate::token_set::TokenSet;

use super::top_level;

/// Sync set for top-level recovery: anything that may begin a file-scope
/// declaration. EOF is included so unrecoverable input doesn't spin.
pub const TOP_LEVEL_STARTS: TokenSet = TokenSet::new(&[
    TokenKind::KwLanguage,
    TokenKind::KwImport,
    TokenKind::KwFeature,
    TokenKind::KwConst,
    TokenKind::KwEnum,
    TokenKind::KwExtern,
    TokenKind::KwPure,
    TokenKind::KwMachine,
    TokenKind::KwSubmachine,
    TokenKind::KwExport,
    TokenKind::DocComment,
    TokenKind::At,
    TokenKind::StableId,
    TokenKind::Eof,
]);

/// Entry point: parse a whole file. Wraps everything in a `FILE` node so the
/// root of the green tree is uniquely typed.
pub fn parse_file(p: &mut Parser) {
    p.start_node(SyntaxKind::FILE);

    // 0. File-leading trivia. The parser ctor advanced past it without
    //    emitting (no node was open then — emitting pre-root trips rowan's
    //    single-root assertion, PARSE-BUG-1). Now that FILE is open, flush
    //    those banner/license comments + whitespace inside it so the CST
    //    stays byte-exact. Trivia *between* later tokens is handled by
    //    Parser::bump's own skip_trivia.
    p.flush_leading_trivia();
    if p.at(TokenKind::KwLanguage) {
        parse_language_decl(p);
    } else if !p.at(TokenKind::Eof) {
        // No header: emit diagnostic but continue.
        p.error(
            DiagnosticCode::E0010,
            "expected 'language fsm X.Y' header at start of file",
        );
    }

    // 1. Zero-or-more imports.
    while p.at(TokenKind::KwImport) {
        top_level::parse_import_decl(p);
    }

    // 2. Zero-or-more top-level decls.
    while !p.at(TokenKind::Eof) {
        let starting = p.current();
        let progressed_at = p.current_span().start;
        match starting {
            TokenKind::KwFeature => top_level::parse_feature_decl(p),
            TokenKind::KwConst => top_level::parse_const_decl(p),
            TokenKind::DocComment => {
                // Doc comments attach to the next declaration. The lexer
                // delivers them as trivia, so they are auto-appended by
                // bump; but a *bare* doc comment with nothing after it
                // would loop forever — guard.
                // (In practice trivia is consumed inside Parser::bump's
                // skip_trivia; reaching here with a DocComment is unusual.)
                top_level::parse_decl_with_doc_or_id(p);
            }
            TokenKind::At | TokenKind::StableId => {
                top_level::parse_decl_with_doc_or_id(p);
            }
            TokenKind::KwEnum => top_level::parse_enum_decl(p),
            TokenKind::KwExtern | TokenKind::KwPure => top_level::parse_extern_decl(p),
            TokenKind::KwMachine | TokenKind::KwExport => top_level::parse_machine_decl(p),
            TokenKind::KwSubmachine => top_level::parse_submachine_decl(p),
            TokenKind::KwImport => {
                // Allowed only in the import block above; re-encountering it
                // here is a recoverable error.
                top_level::parse_import_decl(p);
            }
            _ => {
                let sync = TOP_LEVEL_STARTS;
                p.error_until(
                    sync,
                    DiagnosticCode::E0010,
                    "expected top-level declaration",
                );
            }
        }
        // Forward-progress guard: if a rule didn't consume any tokens we
        // would loop forever. Force a single bump and retry.
        if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
            p.err_and_bump(
                DiagnosticCode::E0010,
                format!("unexpected '{}', skipping", p.current()),
            );
        }
    }

    // Attach any trailing whitespace/comments to the FILE node so the
    // green tree round-trips the source byte-for-byte.
    p.drain_trailing_trivia();
    p.finish_node();
}

/// `language_decl = "language" , "fsm" , version ;`
///
/// Grammar ambiguity (judgment call, documented):
///
/// Doc 04 §2 specifies the version as `integer , "." , integer`, but
/// Doc 04 §1.3's lexer rule consumes `2.0` as a single `FloatLiteral` —
/// `float = digit, {digit}, ".", digit, {digit}`. With the lexer's
/// maximal-munch policy, `language fsm 2.0` round-trips through three
/// tokens (`language`, `fsm`, `2.0`-as-float), NOT five.
///
/// Resolution: accept *either* a single float literal (`2.0`) OR the
/// explicit `int '.' int` form. The CST records whichever shape the user
/// wrote; the analyzer normalises both to a `(major, minor)` tuple.
/// Without this accommodation the most natural file header would never
/// parse cleanly.
fn parse_language_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::LANGUAGE_DECL);
    p.bump(); // `language`
              // The next token MUST be the identifier `fsm`. The lexer doesn't
              // reserve `fsm` as a keyword (see Doc 04 §1.5 — it isn't in the
              // table), so we match by text.
    if p.at(TokenKind::Ident) && p.current_text() == "fsm" {
        p.bump();
    } else {
        p.error(DiagnosticCode::E0010, "expected 'fsm' after 'language'");
    }
    p.start_node(SyntaxKind::LANGUAGE_VERSION);
    match p.current() {
        TokenKind::FloatLiteral => {
            p.bump();
        }
        TokenKind::IntLiteral => {
            p.bump();
            p.expect(TokenKind::Dot, DiagnosticCode::E0010);
            p.expect(TokenKind::IntLiteral, DiagnosticCode::E0010);
        }
        _ => {
            p.error(
                DiagnosticCode::E0010,
                "expected version literal (e.g. '2.0')",
            );
        }
    }
    p.finish_node(); // LANGUAGE_VERSION
    p.finish_node(); // LANGUAGE_DECL
}
