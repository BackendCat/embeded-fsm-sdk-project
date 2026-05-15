/* Side-effect probe shared by motor_externs.c and the main.c driver.
 *
 * Lets the driver ASSERT that the externs were genuinely invoked and with
 * the right arguments -- the difference between "the symbol linked" and
 * "the FSM behaved correctly".
 */
#ifndef MOTOR_EXTERNS_H
#define MOTOR_EXTERNS_H

#include <stdint.h>
#include <stdbool.h>

typedef struct {
    uint32_t set_speed_calls;
    uint16_t last_rpm;
    uint32_t reset_link_calls;
    bool     allow_start; /* return value of the can_start() guard */
} motor_probe_t;

extern motor_probe_t g_motor_probe;

#endif /* MOTOR_EXTERNS_H */
