//! Snapshot tests pinning the AST shape for the Doc 04 §12 industrial
//! example and a smaller Motor controller. Updates via `cargo insta review`.

use fsm_parser::{ast::ast_dump, parse};

/// A medium-sized representative Motor controller spanning composite
/// states, regions, timers, guards, completion transitions, and the
/// action sublanguage.
const MOTOR_SRC: &str = r#"language fsm 2.0

feature hsm
feature timers

const STARTUP_MS = 250
const MAX_RPM    = 5000

enum Mode {
    OFF = 0,
    LOW = 1,
    HIGH = 2
}

pure extern at_target_rpm(ctx) : bool
     extern set_pwm(u16 pwm)
     extern emergency_stop()

@id("m-motor")
export machine Motor {

    context {
        rpm   : u16  = 0
        mode  : Mode
        retry : u8   = 0
    }

    events {
        START
        STOP
        FAULT(code : u8)
    }

    queue {
        capacity = 16
        overflow = drop_oldest
    }

    initial Idle

    @id("s-idle")
    state Idle {
        entry : set_pwm(0)
        on START -> Spinning
    }

    @id("s-spinning")
    state Spinning {
        initial Ramping

        @id("s-ramping")
        state Ramping {
            entry : set_pwm(100)
            after STARTUP_MS ms -> Running
            done [at_target_rpm()] -> Running
        }

        @id("s-running")
        state Running {
            on FAULT [ctx.retry < 3] ~> Ramping : ctx.retry = ctx.retry + 1
            on FAULT [ctx.retry >= 3] -> Idle : emergency_stop()
            on STOP -> Idle
        }
    }

    target C99 {
        strategy    = switch_based
        allow_float = false
    }
}
"#;

#[test]
fn motor_controller_snapshot() {
    let pr = parse(MOTOR_SRC);
    assert!(
        pr.errors.is_empty(),
        "motor source should parse cleanly:\n{:#?}",
        pr.errors
    );
    let dump = ast_dump(&pr.syntax());
    insta::assert_snapshot!(dump);
}

#[test]
fn motor_round_trips_source() {
    let pr = parse(MOTOR_SRC);
    assert_eq!(pr.reconstructed_text(), MOTOR_SRC);
}

#[test]
fn motor_traversal_sanity() {
    let pr = parse(MOTOR_SRC);
    let file = pr.ast();
    assert_eq!(file.consts().count(), 2);
    assert_eq!(file.enums().count(), 1);
    assert_eq!(file.externs().count(), 3);
    let m = file.machines().next().unwrap();
    assert_eq!(m.name().as_deref(), Some("Motor"));
    assert!(m.is_export());
    assert_eq!(m.states().count(), 2); // Idle, Spinning
    let spinning = m
        .states()
        .find(|s| s.name().as_deref() == Some("Spinning"))
        .unwrap();
    assert_eq!(spinning.nested_states().count(), 2);
}
