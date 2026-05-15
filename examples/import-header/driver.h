/*
 * driver.h — a small, realistic embedded motor-driver HAL.
 *
 * This is the kind of header an embedded team already owns. With
 * `fsm generate --import-header driver.h motor_uses_driver.fsm` its
 * function declarations are imported as FSM `extern`s automatically — the
 * `.fsm` does NOT need to re-declare any of them by hand.
 *
 * It deliberately contains constructs the importer must SKIP cleanly
 * (a typedef, a function-like macro, a struct definition, an attribute)
 * alongside the functions that DO import, to exercise the resilience path.
 */
#ifndef DRIVER_H
#define DRIVER_H

#include <stdint.h>
#include <stdbool.h>

/* --- Skipped by the importer (not linkable symbols) ----------------- */

#define DRIVER_MAX_RPM 12000u            /* object-like macro: ignored   */
#define DRIVER_CLAMP(x) ((x) > DRIVER_MAX_RPM ? DRIVER_MAX_RPM : (x))

typedef uint16_t rpm_t;                  /* typedef: skipped with a note */

struct driver_stats {                    /* struct body: skipped         */
    uint32_t starts;
    uint32_t faults;
};

/* --- Imported as FSM externs ---------------------------------------- */

/* Bring the H-bridge up. Returns false if the supply rail is low. */
bool driver_init(void);

/* Command a target speed in RPM. */
void driver_set_speed(uint16_t rpm);

/* Read the last fault code latched by the gate driver. */
uint8_t driver_read_fault(void);

/* Clear a latched fault. `force` ignores the cool-down timer. */
extern void driver_clear_fault(bool force);

/* Snapshot counters into a caller-owned struct (pointer param ⇒ opaque). */
void driver_get_stats(struct driver_stats *out);

/* Has an attribute the importer strips; still imports cleanly. */
void __attribute__((noreturn)) driver_panic(uint8_t code);

#endif /* DRIVER_H */
