//! Top-level declarations: imports, features, consts, enums, externs,
//! machines. Per Doc 04 §2 and §13.
//!
//! Many of these productions are prefixed by an optional doc-comment trivia
//! and an optional `@id("…")` stable-ID annotation. The lexer treats doc
//! comments as trivia (preserved by `Parser::bump`), but stable IDs are
//! non-trivia tokens that we explicitly consume before the keyword.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::expr::{parse_expr, parse_type_ref};
use crate::import_resolver::validate_import_path;
use crate::parser::{ExprContext, Parser};
use crate::token_set::TokenSet;

use super::machine::parse_machine_body;

/// A stable-ID annotation is the lexer's `StableId` token (single token for
/// `@ident`-style) **or** the longer `@ "id" ( "…" )` form which decomposes
/// into `@ Ident ( StringLit )`. The longer form is the canonical syntax
/// from Doc 04 §10.
///
/// Returns whether an annotation was consumed.
pub(crate) fn try_parse_stable_id(p: &mut Parser) -> bool {
    if p.at(TokenKind::StableId) {
        // `@ident` was lexed as one token. The long form `@id("payload")`
        // continues with `(StringLit)` right after — pick it up so we
        // capture the actual stable-ID payload in the CST.
        p.start_node(SyntaxKind::STABLE_ID_ANNOT);
        p.bump(); // @ident
        if p.at(TokenKind::LParen) {
            p.bump(); // (
            if p.at(TokenKind::StringLiteral) {
                p.bump();
            }
            p.expect(TokenKind::RParen, DiagnosticCode::E0010);
        }
        p.finish_node();
        return true;
    }
    if p.at(TokenKind::At) {
        // Bare `@` (no identifier immediately after) — the lexer didn't
        // promote to StableId. Accept it as a stable-ID annotation even
        // though the payload is missing; the analyzer will reject.
        p.start_node(SyntaxKind::STABLE_ID_ANNOT);
        p.bump();
        p.finish_node();
        return true;
    }
    false
}

/// Dispatch a declaration that begins with a doc-comment or stable-id
/// prefix. The doc-comments are trivia and automatically attached to the
/// next bump; the stable-id is consumed inside the dispatch.
pub(crate) fn parse_decl_with_doc_or_id(p: &mut Parser) {
    // Consume zero or more stable-IDs (multiple shouldn't happen, but be
    // defensive about user input).
    while p.at(TokenKind::StableId) || p.at(TokenKind::At) {
        try_parse_stable_id(p);
    }

    // Now we expect a real keyword. If not, recover.
    match p.current() {
        TokenKind::KwEnum => parse_enum_decl(p),
        TokenKind::KwExtern | TokenKind::KwPure => parse_extern_decl(p),
        TokenKind::KwMachine | TokenKind::KwExport => parse_machine_decl(p),
        TokenKind::KwSubmachine => parse_submachine_decl(p),
        TokenKind::KwConst => parse_const_decl(p),
        TokenKind::KwFeature => parse_feature_decl(p),
        TokenKind::KwImport => parse_import_decl(p),
        _ => {
            p.error(
                DiagnosticCode::E0010,
                format!(
                    "expected declaration after stable-ID, found '{}'",
                    p.current()
                ),
            );
            // Skip the bad token so the outer loop progresses.
            if !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token after stable-ID");
            }
        }
    }
}

/// `import_decl = "import" , string , [ "as" , identifier ] ,
///                [ "{" , identifier , { "," , identifier } , "}" ] ;`
pub(crate) fn parse_import_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::IMPORT_DECL);
    p.bump(); // import

    if p.at(TokenKind::StringLiteral) {
        // Path validation (G-02). Use the raw text minus enclosing quotes.
        let raw = p.current_text();
        let span = p.current_span();
        let unquoted = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(raw);
        if let Err(diag) = validate_import_path(unquoted, span) {
            p.push_diagnostic(diag);
        }
        p.bump();
    } else {
        p.expect(TokenKind::StringLiteral, DiagnosticCode::E0010);
    }

    // Optional `as IDENT`.
    if p.at(TokenKind::KwAs) {
        p.start_node(SyntaxKind::IMPORT_ALIAS);
        p.bump(); // as
        p.expect(TokenKind::Ident, DiagnosticCode::E0010);
        p.finish_node();
    }

    // Optional `{ ID, ID, ... }`.
    if p.at(TokenKind::LBrace) {
        p.start_node(SyntaxKind::IMPORT_LIST);
        p.bump(); // {
        if !p.at(TokenKind::RBrace) {
            parse_import_item(p);
            while p.eat(TokenKind::Comma) {
                if p.at(TokenKind::RBrace) {
                    break;
                }
                parse_import_item(p);
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
        p.finish_node();
    }

    p.finish_node();
}

fn parse_import_item(p: &mut Parser) {
    p.start_node(SyntaxKind::IMPORT_ITEM);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

/// `feature_decl = "feature" , identifier ;`
///
/// Grammar ambiguity (judgment call, documented):
///
/// Several of the Doc 04 §2.2 feature-flag names (`parallel`, `history`)
/// collide with reserved keywords in §1.5. The lexer always promotes them
/// to `Kw*` tokens, so a strict `Ident`-only match would reject the
/// canonical examples in the spec. Accept any keyword OR identifier as a
/// feature name; the analyzer maps the surface text to a known flag.
pub(crate) fn parse_feature_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::FEATURE_DECL);
    p.bump(); // feature
    if p.at(TokenKind::Ident) || p.current().is_keyword() {
        p.bump();
    } else {
        p.error(
            DiagnosticCode::E0010,
            format!("expected feature name, found '{}'", p.current()),
        );
    }
    p.finish_node();
}

/// `const_decl = "const" , identifier , "=" , const_expr ;`
///
/// `const_expr` is left to the Pratt parser. The analyzer evaluates it
/// later — the parser only ensures it has *some* parseable shape.
pub(crate) fn parse_const_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::CONST_DECL);
    p.bump(); // const
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.expect(TokenKind::Eq, DiagnosticCode::E0010);
    p.start_node(SyntaxKind::CONST_EXPR);
    parse_expr(p, 0, ExprContext::Action);
    p.finish_node();
    p.finish_node();
}

/// `enum_decl = "enum" , identifier ,
///              "{" , enum_variant , { "," , enum_variant } , [ "," ] , "}" ;`
pub(crate) fn parse_enum_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::ENUM_DECL);
    p.bump(); // enum
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            let progressed_at = p.current_span().start;
            // Optional doc-comment (trivia) + variant.
            parse_enum_variant(p);
            if !p.eat(TokenKind::Comma) {
                // No trailing comma allowed before `}`.
                break;
            }
            if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token in enum body");
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

fn parse_enum_variant(p: &mut Parser) {
    p.start_node(SyntaxKind::ENUM_VARIANT);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.eat(TokenKind::Eq) {
        p.expect(TokenKind::IntLiteral, DiagnosticCode::E0010);
    }
    p.finish_node();
}

/// `extern_decl = [ "pure" ] , "extern" , identifier ,
///                "(" , [ param_list ] , ")" , [ ":" , type ] ;`
pub(crate) fn parse_extern_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::EXTERN_DECL);
    let _ = p.eat(TokenKind::KwPure);
    p.expect(TokenKind::KwExtern, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LParen, DiagnosticCode::E0010) {
        if !p.at(TokenKind::RParen) {
            parse_param_list(p);
        }
        p.expect(TokenKind::RParen, DiagnosticCode::E0010);
    }
    if p.eat(TokenKind::Colon) {
        parse_type_ref(p);
    }
    p.finish_node();
}

fn parse_param_list(p: &mut Parser) {
    p.start_node(SyntaxKind::PARAM_LIST);
    parse_param(p);
    while p.eat(TokenKind::Comma) {
        if p.at(TokenKind::RParen) {
            break;
        }
        parse_param(p);
    }
    p.finish_node();
}

fn parse_param(p: &mut Parser) {
    p.start_node(SyntaxKind::PARAM);
    // Doc 04 §2.5: `param = type identifier`. We try to parse a type first;
    // if no type keyword/ident lands, recover with an error.
    if can_start_type(p.current()) {
        parse_type_ref(p);
        p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    } else {
        // Some shorthand examples in §2.5 show `pure extern can_unlock(ctx)`
        // — a single `ctx` keyword/ident without a type. Accept it as a
        // bare ident-param for compatibility with the Doc 04 example.
        p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    }
    p.finish_node();
}

/// Helper: does this token kind begin a type reference?
fn can_start_type(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwBool
            | TokenKind::KwU8
            | TokenKind::KwU16
            | TokenKind::KwU32
            | TokenKind::KwU64
            | TokenKind::KwI8
            | TokenKind::KwI16
            | TokenKind::KwI32
            | TokenKind::KwI64
            | TokenKind::KwF32
            | TokenKind::KwF64
            | TokenKind::KwOpaque
    )
}

/// `machine_decl = [ "export" ] , "machine" , identifier , "{" , … , "}" ;`
pub(crate) fn parse_machine_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::MACHINE_DECL);
    let _ = p.eat(TokenKind::KwExport);
    p.expect(TokenKind::KwMachine, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        parse_machine_body(p);
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    } else {
        // Recover up to the closing brace or the next top-level start.
        let sync = MACHINE_RECOVERY;
        p.error_until(
            sync,
            DiagnosticCode::E0010,
            "expected '{' after machine name",
        );
    }
    p.finish_node();
}

/// `submachine_decl = "submachine" , identifier , "{" , machine_body , "}" ;`
/// per Doc 04 §15. Structurally identical to `machine_decl` minus the
/// `export` modifier (a submachine is a template referenced by `is`, never
/// a top-level export target). The body reuses `parse_machine_body`, so the
/// AST view reuses the machine-item accessors.
pub(crate) fn parse_submachine_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::SUBMACHINE_DECL);
    p.expect(TokenKind::KwSubmachine, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        parse_machine_body(p);
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    } else {
        // Recover up to the closing brace or the next top-level start.
        p.error_until(
            MACHINE_RECOVERY,
            DiagnosticCode::E0010,
            "expected '{' after submachine name",
        );
    }
    p.finish_node();
}

/// Recovery sync set used when machine-body parsing has gone off the rails.
const MACHINE_RECOVERY: TokenSet = TokenSet::new(&[
    TokenKind::RBrace,
    TokenKind::KwMachine,
    TokenKind::KwSubmachine,
    TokenKind::KwExport,
    TokenKind::KwExtern,
    TokenKind::KwPure,
    TokenKind::KwImport,
    TokenKind::KwEnum,
    TokenKind::KwConst,
    TokenKind::KwFeature,
    TokenKind::Eof,
]);
