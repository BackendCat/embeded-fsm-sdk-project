//! Drive the FSM-Lang-generated Motor state machine from Rust over a tiny
//! `extern "C"` FFI. `build.rs` ran `fsm generate`, compiled the
//! generated C + HAL + extern impls (+ a thin pass-through shim) into a
//! static lib; here we link it and exercise the FSM, asserting real
//! transitions and context mutations — the §5.4 behavioural contract,
//! crossing the language boundary.
//!
//! State / event ids come from the generated `Motor.h` enums (stable ABI,
//! documented in docs/25):
//!   states: IDLE=2, RUNNING=3, FAULTED=4
//!   events: START=0, STOP=1, FAULT=2, RESET=3

use std::os::raw::{c_int, c_uchar, c_uint};

#[repr(C)]
struct Motor {
    _opaque: [u8; 0],
}

// Stable wrapper API compiled from `ffi_helpers.c` (never the private
// Motor_t layout — that is an implementation detail, not the ABI).
extern "C" {
    fn motor_new() -> *mut Motor;
    fn motor_free(m: *mut Motor);
    fn motor_send(m: *mut Motor, event_id: c_int);
    fn motor_send_fault(m: *mut Motor, code: c_uchar);
    fn motor_tick(m: *mut Motor, elapsed_ms: c_uint);
    fn motor_state(m: *const Motor) -> c_int;
    fn motor_count(m: *const Motor) -> c_uint;
    fn motor_last_fault(m: *const Motor) -> c_uchar;
    fn motor_probe_set_speed_calls() -> c_uint;
    fn motor_probe_last_rpm() -> u16;
    fn motor_probe_reset_link_calls() -> c_uint;
    fn motor_probe_set_allow_start(v: c_int);
}

const STATE_IDLE: c_int = 2;
const STATE_RUNNING: c_int = 3;
const STATE_FAULTED: c_int = 4;

const EVENT_START: c_int = 0;
const EVENT_RESET: c_int = 3;

fn main() {
    // SAFETY: every call below is a 1:1 pass-through to the generated
    // public API (docs/25 Sec 1). `motor_new` heap-allocates + inits;
    // we own the pointer and free it before returning. No aliasing: the
    // pointer is single-threaded and not shared.
    unsafe {
        motor_probe_set_allow_start(1); // can_start() guard -> true

        let m = motor_new();
        assert!(!m.is_null(), "motor_new returned null");

        assert_eq!(motor_state(m), STATE_IDLE, "init must enter Idle");

        // START [can_start] -> Running; count=1, set_speed(100).
        motor_send(m, EVENT_START);
        assert_eq!(motor_state(m), STATE_RUNNING, "START must reach Running");
        assert_eq!(motor_count(m), 1, "START must increment count to 1");
        assert_eq!(
            motor_probe_set_speed_calls(),
            1,
            "START must call set_speed once"
        );
        assert_eq!(motor_probe_last_rpm(), 100, "START must call set_speed(100)");

        // after 5000 ms watchdog -> Faulted.
        motor_tick(m, 5001);
        assert_eq!(
            motor_state(m),
            STATE_FAULTED,
            "5001ms tick must fire the after-timer to Faulted"
        );

        // RESET -> Idle.
        motor_send(m, EVENT_RESET);
        assert_eq!(motor_state(m), STATE_IDLE, "RESET must return to Idle");

        // START again -> Running; count must now be 2.
        motor_send(m, EVENT_START);
        assert_eq!(motor_state(m), STATE_RUNNING);
        assert_eq!(motor_count(m), 2, "second START must make count=2");

        // FAULT(7) -> Faulted; last_fault_code=7, reset_link() fired.
        motor_send_fault(m, 7);
        assert_eq!(motor_state(m), STATE_FAULTED, "FAULT must reach Faulted");
        assert_eq!(
            motor_last_fault(m),
            7,
            "FAULT(7) must latch last_fault_code=7"
        );
        assert_eq!(
            motor_probe_reset_link_calls(),
            1,
            "FAULT must call reset_link once"
        );

        // RESET -> Idle, closing the loop.
        motor_send(m, EVENT_RESET);
        assert_eq!(motor_state(m), STATE_IDLE, "final RESET must reach Idle");

        motor_free(m);
    }

    println!(
        "OK Rust<->C FSM: lifecycle verified over FFI (set_speed_calls={}, reset_link_calls={})",
        // SAFETY: pure reads of the static probe counters.
        unsafe { motor_probe_set_speed_calls() },
        unsafe { motor_probe_reset_link_calls() }
    );
}
