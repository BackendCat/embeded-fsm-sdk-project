# FSM Studio — Conformance Test Suite Specification

**Document ID:** FSM-SPEC-TEST
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL, FSM-SPEC-SEM, FSM-SPEC-DIAG, FSM-SPEC-GEN-C

Defines the structure, categories, naming conventions, and coverage requirements of
the FSM Studio conformance test suite. A tool claiming "FSM-Lang 2.0 conformant" MUST
pass all tests marked `normative: true` in the suite MANIFEST.

---

# 1. Purpose

The conformance test suite serves three roles:

1. **Regression prevention.** Every bug fix MUST be accompanied by a test that would
   have caught the bug.
2. **Specification validation.** Tests are the executable form of the specification.
   If a test conflicts with a spec document, the spec wins — but the conflict MUST be
   reported and resolved.
3. **Conformance certification.** Third-party implementations of an FSM-Lang toolchain
   can use this suite to verify their conformance claim.

---

# 2. Test Categories

| Category | Directory | Tests what |
|---|---|---|
| 1 — Parser | `tests/01-parser/` | Lexer + parser: valid and invalid syntax |
| 2 — Validator | `tests/02-validator/` | Analyzer: semantic errors and warnings |
| 3 — Semantic | `tests/03-semantic/` | Execution semantics via simulator traces |
| 4 — Code Generator | `tests/04-codegen-c/` | C99 generated code: compiles + behaves correctly |
| 5 — Formatter | `tests/05-formatter/` | `fsm fmt`: canonical whitespace/formatting |

---

# 3. Repository Layout

```
tests/
├── MANIFEST.json               # Machine-readable test registry
├── 01-parser/
│   ├── positive/               # .fsm files that MUST parse without error
│   │   └── simple-machine.fsm
│   └── negative/               # .fsm files that MUST produce specific parse errors
│       ├── missing-brace.fsm
│       └── missing-brace.expected.json
├── 02-validator/
│   ├── positive/               # Valid files: no errors, no warnings
│   ├── warnings/               # Valid files: specific warnings expected
│   │   ├── loop-in-action.fsm
│   │   └── loop-in-action.expected.json
│   └── negative/               # Invalid files: specific errors expected
│       ├── nondeterminism.fsm
│       └── nondeterminism.expected.json
├── 03-semantic/
│   ├── traces/                 # .trace YAML files — event sequences + expected outcomes
│   │   ├── traffic-light.trace
│   │   └── parallel-regions.trace
│   └── machines/               # .fsm machine definitions for semantic tests
├── 04-codegen-c/
│   ├── sources/                # .fsm source files
│   ├── expected/               # expected output snippets (golden files)
│   ├── harness/                # C test harness files
│   │   ├── harness.h           # test assertion macros
│   │   └── posix_hal.h         # POSIX HAL for compiled tests
│   └── results/                # populated by CI; not committed
└── 05-formatter/
    ├── input/                  # unformatted .fsm files
    └── expected/               # formatted expected output
```

---

# 4. MANIFEST.json Schema

```json
{
  "version": "1.0.0",
  "tests": [
    {
      "id": "P001",
      "category": "parser",
      "subcategory": "positive",
      "file": "01-parser/positive/simple-machine.fsm",
      "description": "Minimal single-state machine parses without error",
      "normative": true,
      "tags": ["minimal", "smoke"],
      "diagCodes": []
    },
    {
      "id": "N-E0300-001",
      "category": "validator",
      "subcategory": "negative",
      "file": "02-validator/negative/nondeterminism.fsm",
      "expectedFile": "02-validator/negative/nondeterminism.expected.json",
      "description": "Two overlapping guards on same event emit FSM-E0300",
      "normative": true,
      "tags": ["determinism"],
      "diagCodes": ["FSM-E0300"]
    }
  ]
}
```

Fields:
- `id` — unique string; prefix convention: `P` = parser positive, `N` = negative,
  `W` = warning, `S` = semantic, `G` = codegen, `F` = formatter
- `category` — `"parser"` | `"validator"` | `"semantic"` | `"codegen-c"` | `"formatter"`
- `normative` — if true, required for conformance claim
- `diagCodes` — diagnostic codes expected to be triggered (empty for positive tests)
- `tags` — free-form labels for filtering

---

# 5. Parser Test Format

### Positive test

**File:** `tests/01-parser/positive/NAME.fsm`
**Expected:** `fsm parse NAME.fsm` exits with code `0` and no diagnostic output.

No companion file required.

### Negative test

**File:** `tests/01-parser/negative/NAME.fsm`
**Companion:** `tests/01-parser/negative/NAME.expected.json`

```json
{
  "exitCode": 1,
  "expectedDiagnostics": [
    {
      "code": "FSM-E0010",
      "line": 5,
      "col": 1,
      "messageContains": "expected '->'"
    }
  ]
}
```

The test runner verifies:
1. Exit code matches `exitCode`.
2. Every entry in `expectedDiagnostics` has a matching diagnostic in the output.
   Matching is: same `code`, `line`, `col` within ±1, and `message` contains the
   `messageContains` substring (case-insensitive).
3. No unexpected error diagnostics are emitted (warnings are permitted).

---

# 6. Validator Test Format

Same as parser tests, but uses `fsm validate` (which includes semantic analysis).

### Warning test companion

```json
{
  "exitCode": 0,
  "expectedDiagnostics": [
    { "code": "FSM-W0200", "line": 8, "col": 5 }
  ]
}
```

---

# 7. Semantic Trace Test Format

Trace files are YAML documents describing an event sequence and expected outcomes.

```yaml
# tests/03-semantic/traces/traffic-light.trace
machine_file: ../machines/traffic-light.fsm
description: "Traffic light cycles Red → Green → Yellow → Red"

steps:
  - action: init
    context:
      green_duration: 30000
    expect:
      configuration: ["TrafficLight.Red"]
      context:
        green_duration: 30000

  - action: advance_clock
    delta_ms: 30001
    expect:
      configuration: ["TrafficLight.Green"]

  - action: advance_clock
    delta_ms: 30001
    expect:
      configuration: ["TrafficLight.Yellow"]

  - action: advance_clock
    delta_ms: 5001
    expect:
      configuration: ["TrafficLight.Red"]

  - action: dispatch
    event: MANUAL_OVERRIDE
    payload: {}
    expect:
      discarded: true   # no transition on MANUAL_OVERRIDE — event is dropped
      configuration: ["TrafficLight.Red"]
```

**Actions:**
- `init` — call `sim/init` with optional context
- `dispatch` — call `sim/dispatch` with event and optional payload
- `advance_clock` — call `sim/advanceClock` with `delta_ms`
- `set_context` — call `sim/setContext`

**Expect fields:**
- `configuration` — list of expected active state dot-paths (must match exactly)
- `context` — map of field name to expected value (partial match — other fields ignored)
- `discarded` — if true, expect zero steps returned from dispatch
- `steps_count` — exact number of RTC steps expected

---

# 8. Code Generator Test Format

### Source and golden files

**Source:** `tests/04-codegen-c/sources/NAME.fsm`
**Expected snippets:** `tests/04-codegen-c/expected/NAME/` directory containing one or more
`.c` or `.h` snippet files.

The test runner:
1. Runs `fsm generate --target c99 NAME.fsm --out /tmp/NAME/`
2. Compiles the output with: `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror -I tests/04-codegen-c/harness/ NAME.c`
3. Checks that each snippet file content is a substring of the corresponding generated file.
4. Compiles and runs the test harness: `gcc -std=c99 harness/NAME_test.c NAME.c -o /tmp/NAME_test && /tmp/NAME_test`
5. Verifies exit code `0`.

### Test harness structure

```c
/* tests/04-codegen-c/harness/motor_3state_test.c */
#include "Motor.h"
#include "Motor_impl.h"
#include "harness.h"
#include "posix_hal.h"

/* Stub implementations */
bool Motor_guard_isSpeedValid(const Motor_t *m, const Motor_Event_t *ev) {
    return ev->start.data.target_speed > 0 && ev->start.data.target_speed <= 3000;
}
void Motor_entry_Idle(Motor_t *m)    { (void)m; }
void Motor_exit_Idle(Motor_t *m)     { (void)m; }
void Motor_entry_Running(Motor_t *m) { (void)m; }
void Motor_exit_Running(Motor_t *m)  { (void)m; }
void Motor_entry_Error(Motor_t *m)   { (void)m; }
void Motor_exit_Error(Motor_t *m)    { (void)m; }
void Motor_action_startMotor(Motor_t *m, const Motor_Event_t *ev) {
    m->speed = ev->start.data.target_speed;
    m->running = true;
}
void Motor_action_stopMotor(Motor_t *m, const Motor_Event_t *ev) {
    (void)ev;
    m->speed = 0;
    m->running = false;
}

int main(void) {
    Motor_t m;
    Motor_init(&m);
    FSM_TEST_ASSERT(m._state == MOTOR_STATE_IDLE);

    Motor_Event_t ev = { .id = MOTOR_EVENT_START };
    ev.start.data.target_speed = 1500;
    Motor_dispatch(&m, &ev);
    FSM_TEST_ASSERT(m._state == MOTOR_STATE_RUNNING);
    FSM_TEST_ASSERT(m.speed == 1500);
    FSM_TEST_ASSERT(m.running == true);

    Motor_Event_t stop = { .id = MOTOR_EVENT_STOP };
    Motor_dispatch(&m, &stop);
    FSM_TEST_ASSERT(m._state == MOTOR_STATE_IDLE);
    FSM_TEST_ASSERT(m.running == false);

    printf("PASS\n");
    return 0;
}
```

`harness.h`:
```c
#include <stdio.h>
#include <stdlib.h>
#define FSM_TEST_ASSERT(cond) \
    do { if (!(cond)) { fprintf(stderr, "FAIL: %s:%d: %s\n", __FILE__, __LINE__, #cond); exit(1); } } while(0)
```

---

# 9. Formatter Test Format

**Input:** `tests/05-formatter/input/NAME.fsm`
**Expected:** `tests/05-formatter/expected/NAME.fsm`

The test runner runs `fsm fmt --check NAME.fsm` and verifies that the output matches
the expected file byte-for-byte. If not, `fsm fmt NAME.fsm > /tmp/actual.fsm` and
`diff -u expected/NAME.fsm /tmp/actual.fsm` is shown.

Additionally: `fsm fmt NAME.fsm | fsm fmt` MUST produce identical output (idempotency
requirement).

---

# 10. Parser Positive Coverage Requirements

Parser positive tests MUST cover every grammar production rule at least once:

- [ ] `machine` declaration with `context`, `event`, `extern`
- [ ] `composite` state with `initial`, `history` (shallow and deep)
- [ ] `parallel` state with ≥ 2 named regions
- [ ] `simple` state with `entry`, `exit`, `on`, `after`, `every`, `defer`
- [ ] `final` state
- [ ] `choice` pseudo-state with ≥ 2 branches including `else`
- [ ] `junction` pseudo-state
- [ ] `fork` → parallel regions
- [ ] `join` from parallel regions
- [ ] Submachine reference with entry and exit points
- [ ] `entry_point` and `exit_point` declarations
- [ ] Transition with full guard expression (and, or, not, field comparison, extern call)
- [ ] Action block with: assignment, if/else, while loop, for loop, extern call, raise, send, defer
- [ ] `@id("X")` stable ID annotation on state, event, and transition
- [ ] `/// doc comment` on machine, state, event
- [ ] `priority N` clause on transition
- [ ] `internal on E: { }` transition
- [ ] Multiple machines in one file
- [ ] `pure extern` and non-pure `extern` declarations
- [ ] All primitive type keywords: `bool`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `f32`, `f64`
- [ ] `opaque "C_type"` field type

---

# 11. Validator Negative Coverage Requirements

Negative tests MUST cover every diagnostic code in FSM-SPEC-DIAG:

| Must cover | Codes |
|---|---|
| All FSM-E0xxx (lexer/parser) | E0001–E0011 |
| All FSM-E0020–E0025 (duplicate names) | One test per code |
| All FSM-E01xx (symbol resolution) | E0100–E0109 |
| All FSM-E02xx (type) | E0200–E0205 |
| All FSM-E03xx (determinism) | E0300–E0304, including ≥ 3 conflict patterns for E0300 |
| All FSM-E04xx (reachability) | E0400 |
| All FSM-E05xx (submachine) | E0500–E0502 |
| FSM-E0900 (runtime protection) | E0900 |
| All FSM-Wxxx | W0100, W0101, W0200, W0201, W0300, W0400, W0401, W0500, W0501 |

---

# 12. Semantic Coverage Requirements

Semantic trace tests MUST cover:

- [ ] Simple two-state machine: `A -EVENT-> B -EVENT-> A`
- [ ] Composite state: entry/exit order verified (`entry_A` before `entry_A_Sub`)
- [ ] Parallel state: two regions both entered simultaneously, both exit on trigger
- [ ] Shallow history: exit `Composite.Sub1`, re-enter `Composite` via history → lands in `Sub1`
- [ ] Deep history: exit nested composite, re-enter via deep history → lands in deepest state
- [ ] Choice: each branch taken based on context value
- [ ] Fork → join: both regions reach final state, join fires, outer transition taken
- [ ] Completion event: `Final` state automatically transitions to outer state
- [ ] Deferred event: event deferred in state A, released on transition to state B, immediately processed
- [ ] `after` timer: fires at correct virtual clock time
- [ ] `every` timer: fires repeatedly at correct interval
- [ ] `internal on E`: state not exited; entry action NOT called again; timer NOT reset
- [ ] External self-transition: state IS exited and re-entered; timer IS reset
- [ ] Submachine: enter via entry point, exit via exit point
- [ ] `send EVENT to Machine`: event delivered to second machine instance
- [ ] Guard evaluation: transition not taken when guard returns false
- [ ] Priority: lower number wins when both guards true

---

# 13. Code Generator Coverage Requirements

Codegen tests MUST verify:

- [ ] 3-state machine with START/STOP/FAULT events — switch strategy
- [ ] Same machine — table strategy (compare generated tables against golden)
- [ ] Machine with composite state — verify LCA exit/entry order in harness
- [ ] Machine with history — verify history stored and restored correctly
- [ ] Machine with parallel state — verify both regions receive event
- [ ] Machine with timer — verify `Motor_tick()` fires transition at correct time
- [ ] Machine with queue overflow — verify assert policy triggers (POSIX abort)
- [ ] Machine with inline action (assignment + if) — verify generated helper function
- [ ] Generated code compiles with: `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`
- [ ] Generated code compiles with: `arm-none-eabi-gcc -std=c99 -mcpu=cortex-m4 -mthumb`
- [ ] `nm generated/*.o | grep malloc` produces no output

---

# 14. Conformance Claim

### "FSM-Lang 2.0 conformant — compiler"

A tool claiming this label MUST:
1. Pass **100%** of tests with `normative: true` in all categories 1, 2.
2. Report all FSM-Exxx and FSM-Wxxx codes with the exact code string (e.g., `"FSM-E0300"`).
3. Accept all positive parser tests (exit code 0, no FSM-E diagnostics).
4. Reject all negative tests (exit code ≠ 0, correct codes emitted).

### "FSM-Lang 2.0 conformant — code generator (C99)"

Additionally:
5. Pass **100%** of tests in category 4 (codegen-c).
6. Generated code MUST compile with `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`.
7. Generated code MUST pass all behavioral harness tests.
8. `nm` must confirm no `malloc`/`free` symbols in generated object files.

### "FSM-Lang 2.0 conformant — simulator"

Additionally:
9. Pass **100%** of tests in category 3 (semantic traces).
10. Simulator MUST implement the `sim/replay` method.

---

# 15. Test Runner CLI

```bash
# Run all normative tests
fsm test --suite tests/

# Run a specific category
fsm test --suite tests/ --category parser
fsm test --suite tests/ --category semantic

# Run tests matching a tag
fsm test --suite tests/ --tag smoke

# Run a single test by ID
fsm test --suite tests/ --id N-E0300-001

# Verbose output (show diff on failure)
fsm test --suite tests/ --verbose

# Output JUnit XML for CI integration
fsm test --suite tests/ --format junit > results.xml

# Show only failing tests
fsm test --suite tests/ --failing-only

# Update golden files (formatter and codegen expected output)
fsm test --suite tests/ --update-golden
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | All selected tests passed |
| 1 | One or more tests failed |
| 2 | Test runner error (suite not found, compile error in harness, etc.) |

---

*End of FSM-SPEC-TEST v1.0.0*
