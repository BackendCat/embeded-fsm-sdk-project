/* Arduino entry point for the `uno` env.
 *
 * The Arduino framework owns `main()` and calls `setup()` once then
 * `loop()` forever -- so the firmware drives the FSM here instead of in a
 * bare `main()` (which `src/main.c`, compiled only for the `native` env,
 * provides for the host build). The generated C is `extern "C"`-clean
 * (Motor.h wraps its API in `extern "C"`), so a C++ TU links it directly
 * (docs/25 Sec 2 -- the C->C++ recipe).
 *
 * Behaviour: run the verified lifecycle once in setup(), latch the
 * outcome, and blink the on-board LED as a pass/fail indicator (fast =
 * verified OK, solid-on = an assertion failed). No serial dependency so
 * it works on a bare Uno with nothing attached.
 */
#include <Arduino.h>

extern "C" {
#include "Motor.h"
#include "motor_externs.h"
}

static bool g_verified = false;

static bool run_lifecycle(void)
{
    g_motor_probe.allow_start = true;

    Motor_t m;
    Motor_init(&m);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) return false;

    Motor_Event_t start;
    start.id = MOTOR_EVENT_START;
    Motor_dispatch(&m, &start);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) return false;
    if (m.context.count != 1u) return false;
    if (g_motor_probe.set_speed_calls != 1u || g_motor_probe.last_rpm != 100u) return false;

    Motor_advance_clock(&m, 5001u);
    if (Motor_current_state(&m) != MOTOR_STATE_FAULTED) return false;

    Motor_Event_t reset;
    reset.id = MOTOR_EVENT_RESET;
    Motor_dispatch(&m, &reset);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) return false;

    Motor_dispatch(&m, &start);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING || m.context.count != 2u) return false;

    Motor_Event_t fault;
    fault.id = MOTOR_EVENT_FAULT;
    fault.__payload.FAULT.code = 7u;
    Motor_dispatch(&m, &fault);
    if (Motor_current_state(&m) != MOTOR_STATE_FAULTED) return false;
    if (m.context.last_fault_code != 7u) return false;
    if (g_motor_probe.reset_link_calls != 1u) return false;

    Motor_dispatch(&m, &reset);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) return false;

    return true;
}

void setup()
{
    pinMode(LED_BUILTIN, OUTPUT);
    g_verified = run_lifecycle();
}

void loop()
{
    if (g_verified) {
        /* Fast blink == lifecycle verified. */
        digitalWrite(LED_BUILTIN, HIGH);
        delay(120);
        digitalWrite(LED_BUILTIN, LOW);
        delay(120);
    } else {
        /* Solid on == an assertion failed. */
        digitalWrite(LED_BUILTIN, HIGH);
    }
}
