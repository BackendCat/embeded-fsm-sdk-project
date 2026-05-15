# Import-Header — `extern`s straight from a C header

Teams with an existing embedded C codebase shouldn't have to hand-transcribe
every HAL/driver function into a DSL `extern` declaration. `fsm generate
--import-header` reads a real `.h` file and derives the `extern`s for you.

## Run it

```sh
fsm generate --import-header driver.h motor_uses_driver.fsm --out generated/
```

`motor_uses_driver.fsm` contains **zero `extern` declarations** — yet its
transition actions call `driver_set_speed(...)` and `driver_clear_fault(...)`.
Those symbols are imported from `driver.h`. The generated
`generated/Motor_impl.h` declares them exactly as if you'd written
`extern driver_set_speed(u16 rpm)` in the `.fsm` yourself:

```c
void driver_set_speed(uint16_t rpm);
void driver_clear_fault(bool force);
bool driver_init(void);
uint8_t driver_read_fault(void);
```

Imported externs are **indistinguishable downstream** from DSL `extern`s:
they flow through the same parser → analyzer → codegen pipeline (the flag
splices synthesized DSL `extern` lines into the source before compilation),
so name-resolution, codegen prototypes and the impl-header contract all
treat them identically.

Repeat `--import-header` for several headers, or set it project-wide in
`fsm.toml`:

```toml
[generate]
import_headers = ["hal/gpio.h", "hal/adc.h"]
```

## Resilience over completeness

The extractor is a focused, self-contained C-declaration parser (no
libclang, no compiler shell-out — deterministic and dependency-free). A
construct it cannot confidently model is **skipped with a note**, never
misparsed. Running the command above prints, for this header:

```
note: [import-header] skipped `#define DRIVER_CLAMP(x) …` — function-like macro …
note: [import-header] skipped `typedef uint16_t rpm_t` — typedef …
note: [import-header] skipped `struct driver_stats { … }` — struct/enum/union body …
note: [import-header] skipped `void driver_get_stats(struct driver_stats *out)` —
      parameter `out: struct driver_stats *` is a pointer/struct/opaque type …
```

`driver.h` deliberately contains those skip-cases (a macro, a typedef, a
struct body, a pointer-param function) next to the importable scalar
functions to demonstrate the resilience path. See **Doc 18 §5** (`fsm
generate`) for the full supported-C-subset and the C→IR type table.

## Files

| File | Role |
|---|---|
| `driver.h` | The pre-existing HAL header (input to `--import-header`). |
| `motor_uses_driver.fsm` | A motor FSM that calls the HAL — no `extern`s. |
| `driver.c` | Host implementations of the importable functions. |
| `driver_probe.h` | Side-effect counters so a test can prove the externs ran. |
