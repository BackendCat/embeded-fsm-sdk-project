/* Thin FFI shim so the Rust side never has to mirror the (private)
 * Motor_t / Motor_Event_t layout. Mirroring generated struct internals
 * across the FFI boundary is brittle (the layout is an implementation
 * detail, not the ABI); wrapping the public API in a few stable helpers
 * is the robust pattern. Each helper is a 1:1 pass-through to the
 * generated public API documented in docs/25 Sec 1.
 */
#include <stdlib.h>
#include <stdint.h>

#include "Motor.h"
#include "motor_externs.h"

/* Allocate + init a Motor instance; caller owns it (free via motor_free). */
Motor_t *motor_new(void)
{
    Motor_t *m = (Motor_t *)malloc(sizeof(Motor_t));
    if (m != NULL) {
        Motor_init(m);
    }
    return m;
}

void motor_free(Motor_t *m)
{
    free(m);
}

/* Dispatch a payload-less event by its integer id. */
void motor_send(Motor_t *m, int event_id)
{
    Motor_Event_t ev;
    ev.id = (Motor_EventId_t)event_id;
    Motor_dispatch(m, &ev);
}

/* Dispatch FAULT(code) — the one payload-carrying event in this FSM. */
void motor_send_fault(Motor_t *m, uint8_t code)
{
    Motor_Event_t ev;
    ev.id = MOTOR_EVENT_FAULT;
    ev.__payload.FAULT.code = code;
    Motor_dispatch(m, &ev);
}

void motor_tick(Motor_t *m, uint32_t elapsed_ms)
{
    Motor_advance_clock(m, elapsed_ms);
}

int motor_state(const Motor_t *m)
{
    return (int)Motor_current_state(m);
}

uint32_t motor_count(const Motor_t *m)
{
    return m->context.count;
}

uint8_t motor_last_fault(const Motor_t *m)
{
    return m->context.last_fault_code;
}

/* Probe accessors so Rust can assert the externs genuinely fired. */
uint32_t motor_probe_set_speed_calls(void) { return g_motor_probe.set_speed_calls; }
uint16_t motor_probe_last_rpm(void)        { return g_motor_probe.last_rpm; }
uint32_t motor_probe_reset_link_calls(void) { return g_motor_probe.reset_link_calls; }
void     motor_probe_set_allow_start(int v) { g_motor_probe.allow_start = (v != 0); }
