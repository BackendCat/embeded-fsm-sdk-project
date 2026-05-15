/*
 * driver.c — host implementation of the importable driver.h functions.
 *
 * The e2e test links this with the generated Motor.c + a tiny HAL stub and
 * RUNS the binary, asserting that the FSM actually called the imported
 * externs (observable through `g_driver`'s side-effect counters).
 *
 * On a real target these bodies would poke H-bridge / ADC registers; here
 * they record what the state machine asked for so a test can prove the
 * imported externs were genuinely invoked.
 */
#include <stdint.h>
#include <stdbool.h>

#include "driver_probe.h"

struct driver_probe g_driver = {0};

bool driver_init(void) {
    g_driver.init_calls++;
    return true; /* supply rail OK */
}

void driver_set_speed(uint16_t rpm) {
    g_driver.set_speed_calls++;
    g_driver.last_rpm = rpm;
}

uint8_t driver_read_fault(void) {
    g_driver.read_fault_calls++;
    return 0;
}

void driver_clear_fault(bool force) {
    g_driver.clear_fault_calls++;
    g_driver.last_clear_force = force;
}

/* `driver_panic` / `driver_get_stats` are intentionally not exercised by
 * the example FSM; `driver_panic` is still imported (scalar signature) and
 * linked to keep the contract honest, `driver_get_stats` is skipped by the
 * importer (pointer param — see README). */
void driver_panic(uint8_t code) {
    g_driver.last_panic_code = code;
}
