//! Snapshot tests for the FSM-Lang lexer.
//!
//! The fixture is a small but realistic Motor machine: timers, guards,
//! actions, doc comments, stable IDs, multiple states. It covers enough of
//! Doc 04 §1 that a regression in the lexer here is impossible to miss.
//!
//! Snapshot files live under `tests/snapshots/` and are managed by `insta`.
//! When updating, run `cargo insta review`.

use fsm_lexer::{tokenize, Token, TokenKind};

/// Source of truth for the realistic-sample snapshot.
///
/// Derived from `docs/04-DSL-Specification.md` §12 patterns but trimmed to
/// stay in the ~50-line range the task brief asks for.
const FIXTURE: &str = r#"language fsm 2.0

/// Motor controller — three-state example.
/// Demonstrates timer + guard + action.

feature timers

const SPIN_UP_MS = 250

pure extern is_safe(ctx) : bool
     extern engage_brake()

@id("m-motor")
export machine Motor {
    context {
        rpm : u32 = 0
    }

    events {
        START
        STOP
    }

    @id("init-motor")
    initial Idle

    @id("s-idle")
    state Idle {
        on START [is_safe] -> Spinning
    }

    state Spinning {
        entry : engage_brake
        after SPIN_UP_MS ms -> Running
        on STOP -> Idle
    }

    state Running {
        on STOP -> Idle
    }
}
"#;

/// Render a Token as a single line: `kind @ start..end  =  "raw"`.
/// Keeps the snapshot diff readable while still asserting span fidelity.
fn render(tokens: &[Token], src: &str) -> String {
    let mut out = String::new();
    for t in tokens {
        let raw = &src[t.span.start..t.span.end];
        // Visualise trivia chars so the snapshot stays one-token-per-line.
        let display = raw
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        out.push_str(&format!(
            "{:<24} @ {:>3}..{:<3} = {:?}\n",
            format!("{:?}", t.kind),
            t.span.start,
            t.span.end,
            display
        ));
    }
    out
}

#[test]
fn motor_machine_snapshot() {
    let toks = tokenize(FIXTURE);
    let rendered = render(&toks, FIXTURE);
    insta::assert_snapshot!("motor_machine", rendered);
}

#[test]
fn snapshot_token_set_covers_expected_kinds() {
    // Cheap structural sanity check that goes red if the fixture drifts in a
    // way that drops one of the constructs we want represented.
    let toks = tokenize(FIXTURE);
    let kinds: Vec<TokenKind> = toks.iter().map(|t| t.kind).collect();
    for required in [
        TokenKind::KwLanguage,
        TokenKind::KwFeature,
        TokenKind::KwConst,
        TokenKind::KwPure,
        TokenKind::KwExtern,
        TokenKind::KwExport,
        TokenKind::KwMachine,
        TokenKind::KwContext,
        TokenKind::KwInitial,
        TokenKind::KwState,
        // Note: `entry` is NOT a keyword per Doc 04 §1.5 — it's an Ident used
        // contextually in state bodies. Same for `exit`.
        TokenKind::KwAfter,
        TokenKind::KwOn,
        TokenKind::KwMs,
        TokenKind::Arrow,
        TokenKind::StableId,
        TokenKind::DocComment,
        TokenKind::IntLiteral,
        TokenKind::Ident,
        TokenKind::LBrace,
        TokenKind::RBrace,
        TokenKind::LParen,
        TokenKind::RParen,
        TokenKind::LBracket,
        TokenKind::RBracket,
        TokenKind::Eof,
    ] {
        assert!(
            kinds.contains(&required),
            "fixture missing required kind: {required:?}"
        );
    }
}
