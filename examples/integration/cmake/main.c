/* Runnable driver for the Motor FSM (Make integration example).
 *
 * Walks the canonical lifecycle and ASSERTS observable behaviour at every
 * step -- state transitions taken, context fields mutated, externs called.
 * Exit code 0 == every assertion held; a non-zero code pinpoints the
 * exact failing step (see the `return N` sites). This is the Sec 5.4
 * behavioural-acceptance contract, expressed as a real program.
 *
 * Lifecycle (mirrors examples/motor/motor.trace):
 *   init                       -> Idle
 *   START [can_start]          -> Running   (count=1, set_speed(100))
 *   advance_clock(5001 ms)     -> Faulted   (after 5000 ms timer)
 *   RESET                      -> Idle
 *   START [can_start]          -> Running   (count=2, set_speed(100))
 *   FAULT(7)                   -> Faulted   (last_fault_code=7, reset_link())
 *   RESET                      -> Idle
 */
#include <stdio.h>

#include "Motor.h"
#include "motor_externs.h"

int main(void)
{
    Motor_t mtr;

    g_motor_probe.allow_start = true;

    Motor_init(&mtr);
    if (Motor_current_state(&mtr) != MOTOR_STATE_IDLE) {
        fprintf(stderr, "init: expected IDLE, got %d\n", Motor_current_state(&mtr));
        return 1;
    }

    /* START -> Running: count incremented, set_speed(100) fired. */
    Motor_Event_t start = { .id = MOTOR_EVENT_START };
    Motor_dispatch(&mtr, &start);
    if (Motor_current_state(&mtr) != MOTOR_STATE_RUNNING) {
        fprintf(stderr, "START: expected RUNNING, got %d\n", Motor_current_state(&mtr));
        return 2;
    }
    if (mtr.context.count != 1u) {
        fprintf(stderr, "START: expected count=1, got %u\n", mtr.context.count);
        return 3;
    }
    if (g_motor_probe.set_speed_calls != 1u || g_motor_probe.last_rpm != 100u) {
        fprintf(stderr, "START: expected set_speed(100) (calls=%u rpm=%u)\n",
                g_motor_probe.set_speed_calls, g_motor_probe.last_rpm);
        return 4;
    }

    /* after 5000 ms watchdog: advance the clock past the deadline. */
    Motor_advance_clock(&mtr, 5001u);
    if (Motor_current_state(&mtr) != MOTOR_STATE_FAULTED) {
        fprintf(stderr, "timer: expected FAULTED after 5001ms, got %d\n",
                Motor_current_state(&mtr));
        return 5;
    }

    /* RESET -> Idle. */
    Motor_Event_t reset = { .id = MOTOR_EVENT_RESET };
    Motor_dispatch(&mtr, &reset);
    if (Motor_current_state(&mtr) != MOTOR_STATE_IDLE) {
        fprintf(stderr, "RESET: expected IDLE, got %d\n", Motor_current_state(&mtr));
        return 6;
    }

    /* START again -> Running: count must now be 2. */
    Motor_dispatch(&mtr, &start);
    if (Motor_current_state(&mtr) != MOTOR_STATE_RUNNING || mtr.context.count != 2u) {
        fprintf(stderr, "START#2: expected RUNNING count=2, got state=%d count=%u\n",
                Motor_current_state(&mtr), mtr.context.count);
        return 7;
    }

    /* FAULT(7) -> Faulted: last_fault_code latched, reset_link() fired. */
    Motor_Event_t fault = { .id = MOTOR_EVENT_FAULT };
    fault.__payload.FAULT.code = 7u;
    Motor_dispatch(&mtr, &fault);
    if (Motor_current_state(&mtr) != MOTOR_STATE_FAULTED) {
        fprintf(stderr, "FAULT: expected FAULTED, got %d\n", Motor_current_state(&mtr));
        return 8;
    }
    if (mtr.context.last_fault_code != 7u) {
        fprintf(stderr, "FAULT: expected last_fault_code=7, got %u\n",
                mtr.context.last_fault_code);
        return 9;
    }
    if (g_motor_probe.reset_link_calls != 1u) {
        fprintf(stderr, "FAULT: expected reset_link() called once, got %u\n",
                g_motor_probe.reset_link_calls);
        return 10;
    }

    /* RESET -> Idle, closing the loop. */
    Motor_dispatch(&mtr, &reset);
    if (Motor_current_state(&mtr) != MOTOR_STATE_IDLE) {
        fprintf(stderr, "final RESET: expected IDLE, got %d\n",
                Motor_current_state(&mtr));
        return 11;
    }

    printf("OK Motor FSM: lifecycle verified "
           "(count=%u set_speed=%u reset_link=%u last_fault=%u)\n",
           mtr.context.count, g_motor_probe.set_speed_calls,
           g_motor_probe.reset_link_calls, mtr.context.last_fault_code);
    return 0;
}
