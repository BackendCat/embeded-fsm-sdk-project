/*
 * driver_probe.h — side-effect counters for the host driver.c.
 *
 * NOT imported by the FSM (it declares a struct + a global, no functions).
 * It exists only so the e2e test can observe that the generated state
 * machine actually invoked the header-imported externs.
 */
#ifndef DRIVER_PROBE_H
#define DRIVER_PROBE_H

#include <stdint.h>
#include <stdbool.h>

struct driver_probe {
    uint32_t init_calls;
    uint32_t set_speed_calls;
    uint32_t read_fault_calls;
    uint32_t clear_fault_calls;
    uint16_t last_rpm;
    bool     last_clear_force;
    uint8_t  last_panic_code;
};

extern struct driver_probe g_driver;

#endif /* DRIVER_PROBE_H */
