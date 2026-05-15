/* User-provided extern + entry/exit implementations for the Motor FSM.
 *
 * The generated Motor_impl.h declares (but does not define) every symbol
 * the DSL referenced:
 *   - can_start()              the `[can_start]` pure guard
 *   - set_speed(uint16_t)      the START / STOP transition action
 *   - reset_link()             the FAULT transition action
 *   - Motor_entry_X/Motor_exit_X  per-state entry/exit hooks (no DSL
 *                                  entry/exit actions here, so no-ops)
 *
 * On real hardware these would talk to a motor driver / comm link. Here
 * they record observable side effects so main.c can ASSERT they actually
 * ran (behavioural acceptance, not symbol-presence).
 */
#include <stdint.h>
#include <stdbool.h>

#include "Motor.h"
#include "motor_externs.h"

motor_probe_t g_motor_probe = { 0, 0, 0, false };

/* Pure guard: gate START. Driven by the probe so the test can flip it. */
bool can_start(void)
{
    return g_motor_probe.allow_start;
}

void set_speed(uint16_t rpm)
{
    g_motor_probe.set_speed_calls += 1u;
    g_motor_probe.last_rpm = rpm;
}

void reset_link(void)
{
    g_motor_probe.reset_link_calls += 1u;
}

/* No entry/exit actions in motor.fsm, but codegen still emits prototypes
 * (analyzer-derived from the state list) -- provide empty bodies. */
void Motor_entry_IDLE(Motor_t *m)    { (void)m; }
void Motor_entry_RUNNING(Motor_t *m) { (void)m; }
void Motor_entry_FAULTED(Motor_t *m) { (void)m; }
void Motor_exit_IDLE(Motor_t *m)     { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)  { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)  { (void)m; }
