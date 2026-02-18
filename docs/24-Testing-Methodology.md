# FSM Studio — Testing Methodology and Executable Test Plan

**Document ID:** FSM-DEV-TEST-METH
**Version:** 2.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-TEST (15), FSM-SPEC-DSL (04), FSM-SPEC-SEM (08), FSM-SPEC-DIAG (10)

This document is both a methodology description and an **executable test plan**. Each test
case includes: unique ID, pre-conditions, exact input data, exact command to run, exact
expected output, and pass/fail criterion. An AI agent or CI runner can execute every test
mechanically without human interpretation.

---

# 1. Testing Tiers and Release Gates

```
Tier 1: Unit/Conformance (parser, analyzer, formatter, codegen, simulator protocol)
    ← 100% automated; runs on every commit; release blocked if any fail

Tier 2: Integration (full pipeline, LSP client-server, VS Code extension)
    ← ~85% automated; runs on every commit; release blocked if any fail

Tier 3: Acceptance (visual rendering, human UX judgment, hardware smoke test)
    ← ~60% automated; required before major release (x.0.0)
```

**Pre-release gate:** `cargo test --workspace` AND `bash tests/conformance/run-all.sh` must
both exit 0 before any release tag.

---

# 2. Test Environment Setup

```bash
# Verify Rust version
rustc --version   # must match rust-toolchain.toml

# Verify GCC
gcc --version     # must be ≥ 9.0

# Verify G++
g++ --version     # must be ≥ 9.0

# Verify Node.js
node --version    # must be ≥ 18.0

# Verify wasm-pack (needed for Tier 2 WASM tests only)
wasm-pack --version
```

All tests assume working directory is repo root (`sm-sdk/`).

---

# 3. Tier 1 — Parser Tests

Test ID format: `PARSE-POS-NNN` (positive) or `PARSE-NEG-NNN` (negative).

## 3.1 Parser Positive Tests

### PARSE-POS-001 — Minimal valid machine

**Pre-condition:** None
**Command:** `fsm check --no-color tests/conformance/parser/pos-001-minimal.fsm`
**Input file content:**
```fsm
machine Minimal {
    initial S
    state S { }
}
```
**Expected exit code:** `0`
**Expected stdout:** *(empty)*
**Expected stderr:** *(empty)*
**Pass:** exit code = 0 AND no output on stdout/stderr

---

### PARSE-POS-002 — Machine with context block

**Command:** `fsm check --no-color tests/conformance/parser/pos-002-context.fsm`
**Input file:**
```fsm
machine WithContext {
    context {
        speed: u16 = 0
        running: bool = false
        count: u32 = 100
    }
    initial Idle
    state Idle { }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-003 — Events with and without payload

**Command:** `fsm check --no-color tests/conformance/parser/pos-003-events.fsm`
**Input file:**
```fsm
machine WithEvents {
    event START(speed: u16, direction: u8)
    event STOP
    event FAULT(code: u8)
    initial Idle
    state Idle {
        on START -> Running
        on FAULT -> Error
    }
    state Running {
        on STOP -> Idle
    }
    state Error { }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-004 — Extern declarations (pure and impure)

**Command:** `fsm check --no-color tests/conformance/parser/pos-004-extern.fsm`
**Input file:**
```fsm
machine WithExtern {
    pure extern isValid(threshold: u16) : bool
    pure extern checkRange(low: u8, high: u8) : bool
    extern logEvent(code: u8)
    extern notify
    initial Idle
    state Idle {
        on START [isValid(100)] -> Running
    }
    state Running {
        on STOP -> Idle
    }
    event START
    event STOP
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-005 — Entry and exit blocks

**Command:** `fsm check --no-color tests/conformance/parser/pos-005-entry-exit.fsm`
**Input file:**
```fsm
machine WithEntryExit {
    extern startTimer
    extern stopTimer
    extern logStart
    event GO
    event HALT
    initial Idle
    state Idle {
        entry: { startTimer(); }
        exit:  { stopTimer(); }
        on GO -> Running
    }
    state Running {
        entry: {
            startTimer();
            logStart();
        }
        on HALT -> Idle
    }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-006 — Composite state with history

**Command:** `fsm check --no-color tests/conformance/parser/pos-006-composite.fsm`
**Input file:**
```fsm
machine WithComposite {
    event SUSPEND
    event RESUME
    event RESET
    initial Operational
    composite Operational {
        history shallow
        initial Active
        state Active { }
        state Paused { }
    }
    state Suspended {
        on RESUME -> Operational
    }
    state Operational {
        on SUSPEND -> Suspended
        on RESET -> Operational
    }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-007 — Parallel state with regions

**Command:** `fsm check --no-color tests/conformance/parser/pos-007-parallel.fsm`
**Input file:**
```fsm
machine WithParallel {
    event START
    event STOP
    initial Monitoring
    parallel Monitoring {
        region Sensors {
            initial Idle
            state Idle {
                on START -> Active
            }
            state Active {
                on STOP -> Idle
            }
        }
        region Output {
            initial Off
            state Off {
                on START -> On
            }
            state On {
                on STOP -> Off
            }
        }
    }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-008 — Timer declarations

**Command:** `fsm check --no-color tests/conformance/parser/pos-008-timers.fsm`
**Input file:**
```fsm
machine WithTimers {
    extern poll
    event DONE
    initial Waiting
    state Waiting {
        after 5000ms -> Timeout
    }
    state Polling {
        every 100ms: { poll(); }
        on DONE -> Done
    }
    state Timeout { }
    state Done { }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-009 — Choice pseudo-state

**Command:** `fsm check --no-color tests/conformance/parser/pos-009-choice.fsm`
**Input file:**
```fsm
machine WithChoice {
    context { speed: u16 = 0; }
    event CHECK
    initial Idle
    state Idle {
        on CHECK -> SpeedSelect
    }
    choice SpeedSelect {
        [ctx.speed > 100] -> Fast
        [ctx.speed > 0]   -> Normal
        [else]            -> Stopped
    }
    state Fast { }
    state Normal { }
    state Stopped { }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-010 — Inline action blocks with control flow

**Command:** `fsm check --no-color tests/conformance/parser/pos-010-actions.fsm`
**Input file:**
```fsm
machine WithActions {
    context {
        speed: u16 = 0
        retries: u8 = 3
        i: u8 = 0
    }
    extern clamp(v: u16) : u16
    extern log
    event START(target: u16)
    event STOP
    initial Idle
    state Idle {
        on START -> Running: {
            ctx.speed = clamp(event.target);
            if (ctx.retries > 0) {
                ctx.retries = ctx.retries - 1;
                log();
            } else {
                ctx.speed = 0;
            }
        }
    }
    state Running {
        on STOP -> Idle: {
            while (ctx.speed > 0) {
                ctx.speed = ctx.speed - 1;
            }
        }
    }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-011 — Doc comments and stable ID annotations

**Command:** `fsm check --no-color tests/conformance/parser/pos-011-annotations.fsm`
**Input file:**
```fsm
/// The main motor controller machine.
@id("motor-v1")
machine AnnotatedMotor {
    event START
    event STOP
    initial Idle

    /// The idle waiting state. Entered on startup and after STOP.
    @id("motor-idle")
    state Idle {
        @id("t-idle-running")
        on START -> Running
    }

    state Running {
        on STOP -> Idle
    }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

### PARSE-POS-012 — Defer declarations

**Command:** `fsm check --no-color tests/conformance/parser/pos-012-defer.fsm`
**Input file:**
```fsm
machine WithDefer {
    event PING
    event PONG
    event BUSY
    initial Idle
    state Idle {
        on PING -> Processing
    }
    state Processing {
        defer PONG
        defer BUSY
        on PING -> Done
    }
    state Done { }
}
```
**Expected exit code:** `0`
**Pass:** exit 0, no output

---

## 3.2 Parser Negative Tests

### PARSE-NEG-001 — Empty machine body (no initial transition)

**Command:** `fsm check --no-color tests/conformance/parser/neg-001-no-initial.fsm`
**Input file:**
```fsm
machine NoInitial {
    state S { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0020`
**Pass:** exit 1 AND stderr contains `FSM-E0020`

---

### PARSE-NEG-002 — Duplicate state name

**Command:** `fsm check --no-color tests/conformance/parser/neg-002-duplicate-state.fsm`
**Input file:**
```fsm
machine DupState {
    initial S
    state S { }
    state S { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0021`
**Pass:** exit 1 AND stderr contains `FSM-E0021`

---

### PARSE-NEG-003 — Duplicate event name

**Command:** `fsm check --no-color tests/conformance/parser/neg-003-duplicate-event.fsm`
**Input file:**
```fsm
machine DupEvent {
    event GO
    event GO
    initial Idle
    state Idle { on GO -> Done; }
    state Done { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0022`
**Pass:** exit 1 AND stderr contains `FSM-E0022`

---

### PARSE-NEG-004 — Duplicate context field

**Command:** `fsm check --no-color tests/conformance/parser/neg-004-duplicate-field.fsm`
**Input file:**
```fsm
machine DupField {
    context {
        speed: u16 = 0
        speed: u8 = 0
    }
    initial Idle
    state Idle { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0023`
**Pass:** exit 1 AND stderr contains `FSM-E0023`

---

### PARSE-NEG-005 — Multiple initial transitions

**Command:** `fsm check --no-color tests/conformance/parser/neg-005-multi-initial.fsm`
**Input file:**
```fsm
machine MultiInitial {
    initial S1
    initial S2
    state S1 { }
    state S2 { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0024`
**Pass:** exit 1 AND stderr contains `FSM-E0024`

---

### PARSE-NEG-006 — Transition to undefined state

**Command:** `fsm check --no-color tests/conformance/parser/neg-006-undefined-state.fsm`
**Input file:**
```fsm
machine UndefinedTarget {
    event GO
    initial Idle
    state Idle {
        on GO -> Nowhere
    }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0100`
**Expected stderr must contain:** `Nowhere`
**Pass:** exit 1 AND stderr contains both `FSM-E0100` and `Nowhere`

---

### PARSE-NEG-007 — Transition on undeclared event

**Command:** `fsm check --no-color tests/conformance/parser/neg-007-undeclared-event.fsm`
**Input file:**
```fsm
machine UndeclaredEvent {
    initial Idle
    state Idle {
        on GHOST -> Done
    }
    state Done { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0101`
**Pass:** exit 1 AND stderr contains `FSM-E0101`

---

### PARSE-NEG-008 — Guard references undeclared context field

**Command:** `fsm check --no-color tests/conformance/parser/neg-008-undefined-field.fsm`
**Input file:**
```fsm
machine UndefinedField {
    event GO
    initial Idle
    state Idle {
        on GO [ctx.phantom > 0] -> Done
    }
    state Done { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0102`
**Pass:** exit 1 AND stderr contains `FSM-E0102`

---

### PARSE-NEG-009 — Non-deterministic transitions (same event, no guards)

**Command:** `fsm check --no-color tests/conformance/parser/neg-009-nondeterministic.fsm`
**Input file:**
```fsm
machine Nondeterministic {
    event GO
    initial Idle
    state Idle {
        on GO -> A
        on GO -> B
    }
    state A { }
    state B { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0300`
**Pass:** exit 1 AND stderr contains `FSM-E0300`

---

### PARSE-NEG-010 — Type mismatch in assignment

**Command:** `fsm check --no-color tests/conformance/parser/neg-010-type-mismatch.fsm`
**Input file:**
```fsm
machine TypeMismatch {
    context { running: bool = false; }
    event GO
    initial Idle
    state Idle {
        on GO -> Done: {
            ctx.running = 42;
        }
    }
    state Done { }
}
```
**Expected exit code:** `1`
**Expected stderr must contain:** `FSM-E0200`
**Pass:** exit 1 AND stderr contains `FSM-E0200`

---

# 4. Tier 1 — Diagnostic Format Tests

These tests verify the exact human-readable output format (FSM-SPEC-CLI §4).

### DIAG-FORMAT-001 — Error message structure

**Command:** `fsm check --no-color tests/conformance/parser/neg-006-undefined-state.fsm 2>&1`
**Expected output must match pattern:**
```
error[FSM-E0100]: state `Nowhere` is not defined
  --> tests/conformance/parser/neg-006-undefined-state.fsm:5:14
   |
 5 |         on GO -> Nowhere
   |                  ^^^^^^^ undefined state
```
**Pass:** output contains `error[FSM-E0100]`, a `-->` location line, and a `^` underline

---

### DIAG-FORMAT-002 — JSON output mode

**Command:** `fsm check --json tests/conformance/parser/neg-006-undefined-state.fsm`
**Expected exit code:** `1`
**Expected stdout:** valid JSON array with at least one object containing `"code": "FSM-E0100"`
**Pass:** `fsm check --json ... | python3 -c "import sys,json; d=json.load(sys.stdin); assert any(x['code']=='FSM-E0100' for x in d)"` exits 0

---

# 5. Tier 1 — Formatter Tests

Test ID format: `FMT-NNN`.

### FMT-001 — Canonical formatting of minimal machine

**Command:**
```bash
cp tests/conformance/formatter/fmt-001-input.fsm /tmp/fmt-test.fsm
fsm fmt /tmp/fmt-test.fsm
diff /tmp/fmt-test.fsm tests/conformance/formatter/fmt-001-expected.fsm
```

**Input file** (`fmt-001-input.fsm`):
```fsm
machine Motor{context{speed:u16=0;running:bool=false;}event START(target_speed:u16);event STOP;initial->Idle;state Idle{entry:{startTimer();}on START->Running:{ctx.speed=0;logStart();}after 5000ms->Error;}state Running{exit:{stopMotor();}on STOP->Idle;}}
```

**Expected file** (`fmt-001-expected.fsm`):
```fsm
machine Motor {
    context {
        speed:   u16  = 0
        running: bool = false
    }

    event START(target_speed: u16)
    event STOP

    initial Idle

    state Idle {
        entry: { startTimer(); }

        on START -> Running: {
            ctx.speed = 0;
            logStart();
        }

        after 5000ms -> Error
    }

    state Running {
        exit: { stopMotor(); }

        on STOP -> Idle
    }
}
```

**Pass:** `diff` exits 0 (files identical)

---

### FMT-002 — Idempotency check

**Command:**
```bash
cp tests/conformance/formatter/fmt-001-expected.fsm /tmp/fmt-idempotent.fsm
fsm fmt /tmp/fmt-idempotent.fsm
diff /tmp/fmt-idempotent.fsm tests/conformance/formatter/fmt-001-expected.fsm
```
**Pass:** `diff` exits 0 — formatting an already-formatted file changes nothing

---

### FMT-003 — Context block alignment

**Input file** (`fmt-003-input.fsm`):
```fsm
machine Align {
    context {
        x: u8 = 0
        longFieldName: u32 = 100
        y: bool = false
    }
    initial S
    state S { }
}
```

**Expected output** (`fmt-003-expected.fsm`):
```fsm
machine Align {
    context {
        x:             u8   = 0
        longFieldName: u32  = 100
        y:             bool = false
    }

    initial S

    state S { }
}
```

**Pass:** formatted output matches expected exactly

---

### FMT-004 — Transition arrow alignment

**Input file** (`fmt-004-input.fsm`):
```fsm
machine Arrows {
    context { speed: u16 = 0; }
    pure extern isSpeedValid(t: u16) : bool
    event START
    event STOP
    event FAULT
    initial Idle
    state Idle {
        on START [ctx.speed > 0] -> Running: { logStart(); }
        on START [isSpeedValid(ctx.speed)] -> Fast
        on STOP -> Idle
        on FAULT -> Error
    }
    state Running { on STOP -> Idle; }
    state Fast { on STOP -> Idle; }
    state Error { }
}
```

**Expected output** — all `->` aligned to the widest prefix:
```fsm
machine Arrows {
    context {
        speed: u16 = 0
    }

    pure extern isSpeedValid(t: u16) : bool

    event START
    event STOP
    event FAULT

    initial Idle

    state Idle {
        on START [ctx.speed > 0]          -> Running: { logStart(); }
        on START [isSpeedValid(ctx.speed)] -> Fast
        on STOP                            -> Idle
        on FAULT                           -> Error
    }

    state Running {
        on STOP -> Idle
    }

    state Fast {
        on STOP -> Idle
    }

    state Error { }
}
```

**Pass:** formatted output matches expected exactly

---

### FMT-005 — `--check` flag exits 1 on unformatted input

**Command:** `fsm fmt --check tests/conformance/formatter/fmt-001-input.fsm`
**Expected exit code:** `1`
**Pass:** exit code = 1

---

### FMT-006 — `--check` flag exits 0 on already-canonical input

**Command:** `fsm fmt --check tests/conformance/formatter/fmt-001-expected.fsm`
**Expected exit code:** `0`
**Pass:** exit code = 0

---

# 6. Tier 1 — C99 Code Generator Tests

Test ID format: `CGEN-C-NNN`.

### CGEN-C-001 — Generated file set

**Input** (`tests/conformance/codegen-c/motor.fsm`):
```fsm
machine Motor {
    context {
        speed:   u16  = 0
        running: bool = false
    }

    event START(target_speed: u16)
    event STOP

    pure extern isReady() : bool
    extern startHardware(speed: u16)
    extern stopHardware

    initial Idle

    state Idle {
        on START [isReady()] -> Running: {
            ctx.speed = event.target_speed;
            ctx.running = true;
            startHardware(ctx.speed);
        }
    }

    state Running {
        on STOP -> Idle: {
            stopHardware();
            ctx.running = false;
            ctx.speed = 0;
        }
    }
}
```

**Command:**
```bash
fsm generate --target c99 --out /tmp/cgen-test/ tests/conformance/codegen-c/motor.fsm
ls /tmp/cgen-test/
```

**Expected files created:**
- `Motor.h`
- `Motor.c`
- `Motor_impl.h`
- `Motor_conf.h`

**Pass:** all 4 files exist and are non-empty

---

### CGEN-C-002 — Generated header compiles

**Command:**
```bash
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -I/tmp/cgen-test/ \
    -c /tmp/cgen-test/Motor.c \
    -o /tmp/cgen-test/Motor.o \
    2>&1
echo "EXIT: $?"
```
**Expected exit code:** `0`
**Expected output:** `EXIT: 0` (no compiler warnings or errors)
**Pass:** gcc exits 0

---

### CGEN-C-003 — Generated Motor.h contains required declarations

**Command:**
```bash
grep -c "typedef struct Motor_t" /tmp/cgen-test/Motor.h
grep -c "Motor_dispatch" /tmp/cgen-test/Motor.h
grep -c "Motor_init" /tmp/cgen-test/Motor.h
grep -c "Motor_tick" /tmp/cgen-test/Motor.h
```
**Expected:** each grep prints `1` (exactly one match)
**Pass:** all 4 greps exit 0

---

### CGEN-C-004 — Motor_impl.h contains all extern declarations

**Command:**
```bash
grep -c "isReady" /tmp/cgen-test/Motor_impl.h
grep -c "startHardware" /tmp/cgen-test/Motor_impl.h
grep -c "stopHardware" /tmp/cgen-test/Motor_impl.h
```
**Expected:** each grep prints `1`
**Pass:** all 3 greps exit 0

---

### CGEN-C-005 — Behavioral correctness: Idle → Running on START

**Test harness** (`tests/conformance/codegen-c/motor_test.c`):
```c
#include "Motor.h"
#include "Motor_impl.h"
#include <stdio.h>
#include <assert.h>

/* Stub implementations */
static int hardware_started = 0;
static int hardware_stopped = 0;

bool isReady(const Motor_t *ctx) { return true; }
void startHardware(Motor_t *ctx, uint16_t speed) { hardware_started = 1; }
void stopHardware(Motor_t *ctx) { hardware_stopped = 1; }

int main(void) {
    Motor_t m;
    Motor_init(&m);

    /* Test 1: initial state is Idle */
    assert(m._state == Motor_State_Idle);
    printf("PASS: initial state is Idle\n");

    /* Test 2: dispatch START with speed=100 → Running */
    Motor_Event_t e;
    e.kind = Motor_Event_START;
    e.START.target_speed = 100;
    Motor_dispatch(&m, &e);

    assert(m._state == Motor_State_Running);
    assert(m.speed == 100);
    assert(m.running == true);
    assert(hardware_started == 1);
    printf("PASS: START transitions to Running, speed=100\n");

    /* Test 3: dispatch STOP → Idle */
    e.kind = Motor_Event_STOP;
    Motor_dispatch(&m, &e);

    assert(m._state == Motor_State_Idle);
    assert(m.speed == 0);
    assert(m.running == false);
    assert(hardware_stopped == 1);
    printf("PASS: STOP transitions to Idle, speed=0\n");

    printf("ALL TESTS PASSED\n");
    return 0;
}
```

**Command:**
```bash
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -I/tmp/cgen-test/ \
    /tmp/cgen-test/Motor.c \
    tests/conformance/codegen-c/motor_test.c \
    -o /tmp/motor_test && \
/tmp/motor_test
```

**Expected exit code:** `0`
**Expected stdout contains:** `ALL TESTS PASSED`
**Pass:** binary compiles, runs, and prints `ALL TESTS PASSED`

---

### CGEN-C-006 — No heap allocation in generated code

**Command:**
```bash
grep -rn "malloc\|calloc\|realloc\|free\|new\|delete" /tmp/cgen-test/Motor.c /tmp/cgen-test/Motor.h
echo "EXIT: $?"
```
**Expected:** grep prints nothing (no matches)
**Expected exit code of grep:** `1` (no matches found)
**Pass:** grep exits 1 (zero matches = zero heap calls)

---

### CGEN-C-007 — Timer: `after` timer creates Motor_tick

**Input** (`tests/conformance/codegen-c/timer-machine.fsm`):
```fsm
machine Watchdog {
    event RESET
    initial Active
    state Active {
        after 5000ms -> Timeout
        on RESET -> Active
    }
    state Timeout { }
}
```

**Command:**
```bash
fsm generate --target c99 --out /tmp/timer-test/ tests/conformance/codegen-c/timer-machine.fsm
grep -c "Watchdog_tick" /tmp/timer-test/Watchdog.h
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -I/tmp/timer-test/ \
    -c /tmp/timer-test/Watchdog.c \
    -o /tmp/timer-test/Watchdog.o
echo "COMPILE_EXIT: $?"
```
**Expected:** `grep` prints `1`; `COMPILE_EXIT: 0`
**Pass:** both conditions met

---

# 7. Tier 1 — Simulator Protocol Tests

Test ID format: `SIM-NNN`.

These tests start a simulator process and communicate via WebSocket JSON-RPC 2.0.
Use the test client at `tests/conformance/simulator/ws-client.py` or the Rust integration tests.

### SIM-001 — Load machine and initialize

**JSON-RPC sequence:**
```jsonc
// Request
{"jsonrpc":"2.0","id":1,"method":"sim/load","params":{"ir": <motor-ir-json>}}

// Expected response
{"jsonrpc":"2.0","id":1,"result":{"instanceId":"motor-1"}}

// Request
{"jsonrpc":"2.0","id":2,"method":"sim/init","params":{"instanceId":"motor-1"}}

// Expected response
{"jsonrpc":"2.0","id":2,"result":{"configuration":{"active":["Motor:state:Idle"]}}}
```
**Pass:** both responses have no `error` field and match the expected structure

---

### SIM-002 — Dispatch START event

**Continuation of SIM-001 session:**
```jsonc
// Request
{"jsonrpc":"2.0","id":3,"method":"sim/dispatch","params":{
    "instanceId":"motor-1",
    "event":{"name":"START","payload":{"target_speed":100}}
}}

// Expected response
{"jsonrpc":"2.0","id":3,"result":{"configuration":{"active":["Motor:state:Running"]}}}

// Also expect notification (may arrive before or after response):
{"jsonrpc":"2.0","method":"sim/stateChanged","params":{
    "instanceId":"motor-1",
    "from":["Motor:state:Idle"],
    "to":["Motor:state:Running"],
    "event":"START"
}}
```
**Pass:** dispatch response shows `Running`; `sim/stateChanged` notification received

---

### SIM-003 — Virtual clock fires timer

**Continuation of SIM-001 session (reset first):**
```jsonc
{"jsonrpc":"2.0","id":10,"method":"sim/reset","params":{"instanceId":"motor-1"}}
{"jsonrpc":"2.0","id":11,"method":"sim/setClockMode","params":{"instanceId":"motor-1","mode":"virtual"}}
{"jsonrpc":"2.0","id":12,"method":"sim/advanceClock","params":{"instanceId":"motor-1","deltaMs":5001}}
```

**Expected notification:**
```jsonc
{"jsonrpc":"2.0","method":"sim/timerFired","params":{
    "instanceId":"motor-1",
    "state":"Motor:state:Idle",
    "timerType":"after",
    "durationMs":5000
}}
```
**Pass:** `sim/timerFired` notification received after advanceClock(5001)

---

### SIM-004 — Get context after dispatch

```jsonc
// After dispatching START with target_speed=100:
{"jsonrpc":"2.0","id":20,"method":"sim/getContext","params":{"instanceId":"motor-1"}}

// Expected:
{"jsonrpc":"2.0","id":20,"result":{"context":{"speed":100,"running":true}}}
```
**Pass:** context shows `speed: 100` and `running: true`

---

### SIM-005 — Load invalid IR returns error

```jsonc
{"jsonrpc":"2.0","id":30,"method":"sim/load","params":{"ir":{"irVersion":"1.0.0","machines":[]}}}

// Expected error response:
{"jsonrpc":"2.0","id":30,"error":{"code":-32001,"message":"...", "data":{"fsm_code":"FSM-SIM-E001"}}}
```
**Pass:** response has `error` field with `fsm_code` = `FSM-SIM-E001`

---

### SIM-006 — Trace records captured

```jsonc
// After init + dispatch START:
{"jsonrpc":"2.0","id":40,"method":"sim/getTrace","params":{"instanceId":"motor-1"}}

// Expected result contains at least 2 records:
{"jsonrpc":"2.0","id":40,"result":{"records":[
    {"type":"init","configuration":{"active":["Motor:state:Idle"]}},
    {"type":"dispatch","event":"START","from":["Motor:state:Idle"],"to":["Motor:state:Running"]}
]}}
```
**Pass:** trace contains `init` and `dispatch` records with correct fields

---

# 8. Tier 1 — CLI Exit Code Tests

### CLI-EXIT-001 — Exit 0 on clean file

**Command:** `fsm check tests/conformance/parser/pos-001-minimal.fsm; echo $?`
**Expected output contains:** `0`
**Pass:** exit code = 0

---

### CLI-EXIT-002 — Exit 1 on user error (bad input)

**Command:** `fsm check tests/conformance/parser/neg-001-no-initial.fsm; echo $?`
**Expected output contains:** `1`
**Pass:** exit code = 1

---

### CLI-EXIT-003 — Exit 3 on missing file

**Command:** `fsm check /nonexistent/path/file.fsm; echo $?`
**Expected output contains:** `3`
**Pass:** exit code = 3

---

### CLI-EXIT-004 — Exit 4 on invalid config

**Command:** `fsm check --config /nonexistent/fsm.toml tests/conformance/parser/pos-001-minimal.fsm; echo $?`
**Expected output contains:** `4`
**Pass:** exit code = 4

---

# 9. Tier 2 — Full Pipeline Integration Tests

### INT-PIPE-001 — Motor example end-to-end

**Command:**
```bash
fsm check examples/motor/motor.fsm && \
fsm generate --target c99 --out /tmp/motor-full/ examples/motor/motor.fsm && \
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -I/tmp/motor-full/ \
    -c /tmp/motor-full/Motor.c \
    -o /tmp/motor-full/Motor.o && \
echo "PIPELINE_OK"
```
**Expected output:** `PIPELINE_OK`
**Pass:** all stages exit 0

---

### INT-PIPE-002 — IR output is valid JSON

**Command:**
```bash
fsm ir examples/motor/motor.fsm | python3 -c "
import sys, json
ir = json.load(sys.stdin)
assert ir['irVersion'] == '1.0.0'
assert len(ir['machines']) >= 1
assert ir['machines'][0]['name'] == 'Motor'
print('IR_VALID')
"
```
**Expected output:** `IR_VALID`
**Pass:** Python script exits 0 and prints `IR_VALID`

---

### INT-PIPE-003 — Formatter pipeline: format then check succeeds

**Command:**
```bash
cp examples/motor/motor.fsm /tmp/motor-fmt-test.fsm
fsm fmt /tmp/motor-fmt-test.fsm && \
fsm fmt --check /tmp/motor-fmt-test.fsm && \
echo "FMT_IDEMPOTENT"
```
**Expected output:** `FMT_IDEMPOTENT`
**Pass:** both fmt invocations exit 0

---

# 10. Tier 2 — LSP Integration Tests

These tests run using `@vscode/test-electron` (Node.js). Test file: `editors/vscode/src/test/lsp.test.ts`.

### LSP-INT-001 — Server starts and reports capabilities

**Test code:**
```typescript
test('LSP server starts and responds to initialize', async () => {
    const client = await startLSPClient();
    const caps = await client.sendRequest('initialize', {
        rootUri: testWorkspaceUri,
        capabilities: {}
    });
    assert.ok(caps.capabilities.textDocumentSync);
    assert.ok(caps.capabilities.completionProvider);
    assert.ok(caps.capabilities.hoverProvider);
    assert.ok(caps.capabilities.definitionProvider);
    assert.strictEqual(caps.serverInfo.name, 'fsm-lang-server');
});
```
**Pass:** all assertions pass, no exception thrown

---

### LSP-INT-002 — Diagnostics published for invalid file

**Test code:**
```typescript
test('Diagnostics appear for undefined state reference', async () => {
    const uri = openTestFile('neg-006-undefined-state.fsm');
    const diagnostics = await waitForDiagnostics(uri, 2000);
    assert.ok(diagnostics.length >= 1);
    assert.strictEqual(diagnostics[0].code, 'FSM-E0100');
    assert.ok(diagnostics[0].message.includes('Nowhere'));
});
```
**Pass:** diagnostic with `FSM-E0100` appears within 2000ms

---

### LSP-INT-003 — Completion provides event names

**Test code:**
```typescript
test('Completion after "on " provides event names', async () => {
    const uri = openTestFile('motor.fsm');
    // Position cursor after "on " inside state Idle
    const items = await requestCompletion(uri, {line: 12, character: 11});
    const labels = items.map(i => i.label);
    assert.ok(labels.includes('START'));
    assert.ok(labels.includes('STOP'));
});
```
**Pass:** completion list includes declared event names

---

### LSP-INT-004 — Hover returns state info

**Test code:**
```typescript
test('Hover over state name returns hover card', async () => {
    const uri = openTestFile('motor.fsm');
    // Hover over "Idle" in "state Idle {"
    const hover = await requestHover(uri, {line: 10, character: 10});
    assert.ok(hover.contents);
    const text = typeof hover.contents === 'string'
        ? hover.contents
        : hover.contents.value;
    assert.ok(text.includes('Idle'));
    assert.ok(text.includes('state'));
});
```
**Pass:** hover card contains state name and kind

---

# 11. Tier 3 — VS Code Acceptance Tests (Manual + Playwright)

## 11.1 Automated Playwright Tests

### E2E-001 — Extension activates on .fsm file

**Playwright test:**
```typescript
test('Extension activates when .fsm file opened', async ({ page }) => {
    await vscode.openFile('examples/motor/motor.fsm');
    await page.waitForSelector('.statusbar-item:has-text("FSM")');
    const langMode = await page.locator('.language-mode').textContent();
    assert.strictEqual(langMode, 'FSM-Lang');
});
```
**Pass:** status bar shows `FSM` indicator and language mode is `FSM-Lang`

---

### E2E-002 — Diagram panel opens

**Playwright test:**
```typescript
test('Diagram panel opens via Ctrl+Shift+D', async ({ page }) => {
    await vscode.openFile('examples/motor/motor.fsm');
    await page.keyboard.press('Control+Shift+D');
    await page.waitForSelector('[aria-label="FSM Diagram"]', { timeout: 3000 });
    // Verify SVG is rendered with at least one state node
    const stateNodes = await page.locator('.fsm-state-node').count();
    assert.ok(stateNodes >= 2);
});
```
**Pass:** diagram panel opens and renders state nodes

---

## 11.2 Manual Acceptance Checklist

To be performed by a human tester before each major release.

### A. Syntax Highlighting

| # | Action | Expected | Tester Judgment |
|---|---|---|---|
| A-1 | Open `examples/motor/motor.fsm` in VS Code | `machine`, `state`, `on` keywords are a distinct color | Visual |
| A-2 | Observe `context` block | Field names, `:`, type names, `=`, default values each have distinct token colors | Visual |
| A-3 | Type `// comment` on a line | Comment turns gray/muted | Visual |
| A-4 | Type `/* block */` | Block comment turns gray/muted | Visual |
| A-5 | Type `/// doc comment` before a state | Doc comment has a slightly different shade than regular comments | Visual |
| A-6 | Type a guard `[ctx.speed > 100]` | Guard brackets and content are colored distinctly from transition body | Visual |

### B. Live Diagram

| # | Action | Expected | Auto? |
|---|---|---|---|
| B-1 | Open diagram with `Ctrl+Shift+D` | SVG diagram appears with state boxes and arrows | Playwright |
| B-2 | Rename state `Idle` → `Waiting`, save | Diagram updates within 1 second; `Waiting` appears instead of `Idle` | Playwright |
| B-3 | Add `state NewState { }` and a transition to it, save | `NewState` appears in diagram | Playwright |
| B-4 | Delete a state and all its transitions, save | State box disappears from diagram | Playwright |
| B-5 | Open `examples/traffic-light/` (composite states) | Nested states shown inside parent composite box | Visual |
| B-6 | Click on a state box in diagram | Editor scrolls to state declaration | Manual |

### C. Simulator

| # | Action | Expected | Auto? |
|---|---|---|---|
| C-1 | Open simulator with `Ctrl+Alt+S` | Simulator panel appears | Playwright |
| C-2 | Click "Initialize" | Machine initializes; `Idle` state highlighted in diagram | Manual |
| C-3 | Dispatch `START` with `target_speed = 100` | State changes to `Running`; context panel shows `speed: 100, running: true` | Manual |
| C-4 | Advance virtual clock 5001ms | Timer fires if `after` timer is present; trace log shows entry | Manual |
| C-5 | Export trace to `.json` | File downloaded; contains init + dispatch records | Manual |
| C-6 | Import trace and replay | Simulator ends in same final state | Manual |

---

# 12. Automation Coverage Summary

| Category | Tests | Auto | Manual |
|---|---|---|---|
| Parser positive | 12 | 12 | 0 |
| Parser negative | 10 | 10 | 0 |
| Diagnostic format | 2 | 2 | 0 |
| Formatter | 6 | 6 | 0 |
| C99 codegen | 7 | 7 | 0 |
| Simulator protocol | 6 | 6 | 0 |
| CLI exit codes | 4 | 4 | 0 |
| Full pipeline | 3 | 3 | 0 |
| LSP integration | 4 | 4 | 0 |
| VS Code E2E | 2 | 2 | 0 |
| VS Code manual | 20 | 10 | 10 |
| **TOTAL** | **76** | **66 (87%)** | **10 (13%)** |

---

# 13. CI Configuration

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Rust tests
        run: cargo test --workspace
      - name: Formatter check (dogfood)
        run: cargo run -p fsm-cli -- fmt --check examples/**/*.fsm
      - name: Conformance test suite
        run: bash tests/conformance/run-all.sh
      - name: VS Code extension build
        run: cd editors/vscode && npm ci && npm run compile && npm test
      - name: C99 codegen compile check
        run: bash tests/conformance/codegen-c/run.sh
```

**Pre-release additional step:**
```yaml
      - name: Full acceptance tests (major release only)
        if: startsWith(github.ref, 'refs/tags/v') && !contains(github.ref, '-')
        run: npx playwright test editors/vscode/src/test/e2e/
```

---

# 14. Test Data File Organization

All test input and golden output files live in `tests/conformance/`:

```
tests/conformance/
├── run-all.sh                          # Master runner; exits non-zero on any failure
├── MANIFEST.json                       # Machine-readable test index
├── parser/
│   ├── pos-001-minimal.fsm
│   ├── pos-002-context.fsm
│   ├── ... (pos-001 through pos-012)
│   ├── neg-001-no-initial.fsm
│   └── ... (neg-001 through neg-010)
├── formatter/
│   ├── fmt-001-input.fsm
│   ├── fmt-001-expected.fsm
│   ├── fmt-003-input.fsm
│   ├── fmt-003-expected.fsm
│   ├── fmt-004-input.fsm
│   └── fmt-004-expected.fsm
├── codegen-c/
│   ├── motor.fsm
│   ├── motor_test.c                    # Behavioral test harness
│   ├── timer-machine.fsm
│   └── run.sh                          # Compiles and runs all codegen tests
└── simulator/
    ├── motor-ir.json                   # Pre-compiled IR for simulator tests
    └── ws-client.py                    # Python WebSocket test client
```

---

# 15. Golden Test File Format (`.fsm.test`)

Golden test files combine input, expected outputs, and expected diagnostics in a single
self-contained file. This format allows an AI agent or CI runner to execute every test
mechanically without requiring multiple files per test case.

### 15.1 File Structure

A `.fsm.test` file uses section delimiters to separate the input source, expected IR,
expected diagnostics, and expected formatted output:

```
=== INPUT ===
<FSM-Lang source code>

=== EXPECTED_IR ===
<Canonical IR JSON output (compact or pretty)>

=== EXPECTED_DIAGNOSTICS ===
<One diagnostic per line, format: CODE:LINE:COL:MESSAGE_SUBSTRING>

=== EXPECTED_FORMATTED ===
<Expected output of `fsm fmt` on the INPUT section>

=== EXPECTED_CODEGEN_C99 ===
<Expected generated C99 code (or key fragments)>

=== EXPECTED_CODEGEN_CPP17 ===
<Expected generated C++17 code (or key fragments)>

=== META ===
<Key-value metadata: id, tags, description, expected_exit_code>
```

### 15.2 Section Rules

1. Each section starts with `=== SECTION_NAME ===` on its own line.
2. Sections are optional — a test file need only include the sections relevant to its
   test category.
3. The `INPUT` section is REQUIRED — all other sections are optional.
4. The `META` section, if present, MUST be last.
5. Trailing whitespace in section content is significant for formatter tests.
6. Empty sections (delimiter present but no content) are valid and mean "expect empty
   output."

### 15.3 META Section Keys

```
id: PARSE-POS-001
tags: parser, positive, minimal
description: Minimal valid machine with one state
expected_exit_code: 0
category: parser
skip: false
skip_reason: Requires feature X (not yet implemented)
```

| Key | Required | Description |
|---|---|---|
| `id` | Yes | Unique test identifier (matches MANIFEST.json) |
| `tags` | No | Comma-separated tags for filtering |
| `description` | No | Human-readable description |
| `expected_exit_code` | Yes | Expected CLI exit code (0, 1, 2, 3, or 4) |
| `category` | Yes | `parser` \| `validator` \| `semantic` \| `codegen-c` \| `codegen-cpp` \| `formatter` \| `simulator` |
| `skip` | No | `true` to skip this test (default: `false`) |
| `skip_reason` | No | Explanation for why the test is skipped |

### 15.4 EXPECTED_DIAGNOSTICS Format

Each line specifies one expected diagnostic:

```
FSM-E0100:5:14:not found
FSM-W0200:12:5:Loop in action block
```

Format: `CODE:LINE:COL:MESSAGE_SUBSTRING`

- `CODE` — the FSM diagnostic code (e.g., `FSM-E0100`)
- `LINE` — 1-based line number in the INPUT section
- `COL` — 1-based column number
- `MESSAGE_SUBSTRING` — a substring that MUST appear in the diagnostic message

A test passes if and only if:
1. Every line in `EXPECTED_DIAGNOSTICS` matches a diagnostic emitted by the compiler.
2. No unexpected ERROR-level diagnostics are emitted (unexpected warnings are tolerated).

### 15.5 Complete Example

```
=== INPUT ===
machine Motor {
    context {
        speed: u16 = 0
    }
    event START
    initial Idle
    state Idle {
        on START -> Running
    }
}

=== EXPECTED_DIAGNOSTICS ===
FSM-E0100:8:22:Running

=== EXPECTED_IR ===

=== META ===
id: PARSE-NEG-020
tags: parser, negative, undefined-state
description: Transition to undefined state 'Running'
expected_exit_code: 1
category: parser
```

### 15.6 Test Runner Integration

The `fsm test` command discovers `.fsm.test` files in the test suite directory:

```bash
fsm test --suite tests/conformance/

# Discovers:
#   tests/conformance/parser/pos-001-minimal.fsm.test
#   tests/conformance/parser/neg-020-undefined.fsm.test
#   tests/conformance/formatter/fmt-001-canonical.fsm.test
#   ...
```

For each `.fsm.test` file, the runner:
1. Extracts the `INPUT` section and writes it to a temporary `.fsm` file.
2. Runs `fsm check` on the temporary file and compares exit code to `expected_exit_code`.
3. If `EXPECTED_DIAGNOSTICS` is present, verifies all expected diagnostics appear.
4. If `EXPECTED_FORMATTED` is present, runs `fsm fmt` and compares output exactly.
5. If `EXPECTED_IR` is present, runs `fsm ir` and compares JSON structurally
   (order-insensitive for object keys, order-sensitive for arrays).
6. If `EXPECTED_CODEGEN_C99` is present, runs `fsm generate --target c99` and verifies
   the generated code contains all expected fragments.
7. Reports PASS/FAIL per section, with unified diff on failure.

### 15.7 Updating Golden Files

```bash
# Regenerate all golden outputs (IR, formatted, codegen) from current compiler:
fsm test --suite tests/conformance/ --update-golden

# Regenerate only formatter goldens:
fsm test --suite tests/conformance/ --update-golden --category formatter
```

The `--update-golden` flag overwrites the `EXPECTED_FORMATTED`, `EXPECTED_IR`, and
`EXPECTED_CODEGEN_*` sections in-place. `EXPECTED_DIAGNOSTICS` is NEVER auto-updated
(it must be maintained manually).

---

*End of FSM-DEV-TEST-METH v2.0.0*
