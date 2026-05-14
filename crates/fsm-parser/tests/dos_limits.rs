//! DoS-limit tests for the parser entry points.
//!
//! Per Doc 00 §7.12 G-02 / audit §P1-5 — the parser must short-circuit
//! oversize input, excessive token counts, and runaway recursion depth
//! to a single diagnostic + empty CST, never to a panic / stack overflow
//! / OOM.

use fsm_parser::{parse, parse_with_limits, ParseLimits};

#[test]
fn rejects_oversize_input() {
    // Build a source that beats the default 1 MiB cap. Use a single
    // comment so the lexer happily slurps it; the size check should
    // fire before tokenization even starts.
    let body = "a".repeat(2 * 1024 * 1024);
    let src = format!("// {body}");
    let pr = parse(&src);
    assert!(
        !pr.errors.is_empty(),
        "oversize input must produce a diagnostic"
    );
    assert!(
        pr.errors[0].message.contains("exceeds maximum size")
            || pr.errors[0].message.contains("input exceeds"),
        "diagnostic message must signal the limit; got {:?}",
        pr.errors[0].message
    );
}

#[test]
fn rejects_deep_recursion() {
    // 300 nested parens is well over the default depth cap of 256.
    // The parser must reject without recursing all the way down.
    let depth = 300usize;
    // Wrap in a minimal program so the source is well-formed enough to
    // reach the expression parser. We exploit the action-block context
    // by emitting `entry: foo = (((...)))`. Use a machine wrapper.
    let inner = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    let src = format!(
        "language fsm 2.0\nmachine M {{\n  initial -> S\n  state S {{\n    entry: x = {inner}\n  }}\n}}\n"
    );
    let pr = parse(&src);
    // Expect at least one diagnostic mentioning recursion depth.
    let saw_depth_msg = pr
        .errors
        .iter()
        .any(|d| d.message.to_lowercase().contains("recursion depth"));
    assert!(
        saw_depth_msg,
        "deep nesting must trigger a recursion-depth diagnostic; saw {:?}",
        pr.errors.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn accepts_realistic_program() {
    // examples/motor/motor.fsm is the canonical small DSL sample shipped
    // with the toolchain. Loading it confirms the limits do NOT regress
    // ordinary parsing for any real-world program.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("examples/motor/motor.fsm");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let pr = parse(&src);
    // Hard errors must be empty for a known-good program. Warnings are
    // tolerated.
    let hard_errors: Vec<_> = pr
        .errors
        .iter()
        .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
        .collect();
    assert!(
        hard_errors.is_empty(),
        "realistic program produced errors: {:?}",
        hard_errors
            .iter()
            .map(|d| (d.code.to_string(), d.message.clone()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn custom_limits_work() {
    // A 128-byte source easily exceeds a 16-byte custom cap.
    let src = "language fsm 2.0\nmachine M { initial -> S\n  state S {}\n}\n";
    let limits = ParseLimits {
        max_input_bytes: 16,
        ..ParseLimits::DEFAULT
    };
    let pr = parse_with_limits(src, &limits);
    assert!(
        !pr.errors.is_empty(),
        "tight cap must trigger; saw {:?}",
        pr.errors
    );
    assert!(pr.errors[0].message.contains("exceeds maximum size"));
}

#[test]
fn custom_depth_cap_triggers_on_modest_input() {
    // With a depth cap of 8, even 20 nested parens trips the limiter.
    // This verifies the depth path is actually wired through `parse_expr`
    // independently of the byte cap.
    let inner = format!("{}1{}", "(".repeat(20), ")".repeat(20));
    let src = format!(
        "language fsm 2.0\nmachine M {{\n  initial -> S\n  state S {{\n    entry: x = {inner}\n  }}\n}}\n"
    );
    let limits = ParseLimits {
        max_recursion_depth: 8,
        ..ParseLimits::DEFAULT
    };
    let pr = parse_with_limits(&src, &limits);
    let saw = pr
        .errors
        .iter()
        .any(|d| d.message.to_lowercase().contains("recursion depth"));
    assert!(
        saw,
        "small depth cap must fire on modest nesting; saw {:?}",
        pr.errors.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn empty_input_does_not_trigger_limits() {
    // Regression: a zero-byte source must NOT trip the byte cap.
    let pr = parse("");
    // Empty file is its own kind of malformed, but the diagnostic should
    // be about the missing language header, not the size cap.
    let saw_size_msg = pr
        .errors
        .iter()
        .any(|d| d.message.contains("exceeds maximum size"));
    assert!(
        !saw_size_msg,
        "empty input must not trip the size cap; saw {:?}",
        pr.errors
    );
}
