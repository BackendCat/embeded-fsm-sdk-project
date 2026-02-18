# FSM Studio — C++17 Code Generator Specification

**Document ID:** FSM-SPEC-GEN-CPP
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-GEN-C, FSM-SPEC-IR, FSM-SPEC-HAL

Specifies the C++17 code emitted by the FSM compiler for C++ embedded targets
(Arduino, ARM Cortex-M with C++ toolchain, RISC-V with libc++, host simulation).
This document covers only differences from the C99 generator spec (FSM-SPEC-GEN-C).
Where not mentioned, the C99 behavior applies.

---

# 1. Design Goals

1. **Zero-overhead abstractions.** No virtual dispatch unless explicitly configured.
   CRTP (Curiously Recurring Template Pattern) eliminates vtable overhead.
2. **No STL by default.** `<variant>`, `<array>`, `<optional>`, `<memory>` are opt-in.
   The default profile works with `-nostdinc++` or on Arduino.
3. **Embedded-safe flags.** Generated code MUST compile with:
   `-std=c++17 -fno-exceptions -fno-rtti -Wall -Wextra -Wpedantic -Werror`
4. **Strong typing.** Event type uses `std::variant` (STL profile) or tagged union
   (no-STL profile). State IDs use `enum class` instead of plain `enum`.
5. **Virtual-override pattern.** Guard and action functions are virtual methods for
   the non-CRTP profile (allows mocking in tests).
6. **Namespace isolation.** All generated code lives in `namespace fsm::{MachineName}`.

---

# 2. Generated File Layout

For machine `Motor`:

| File | Contents |
|---|---|
| `Motor.hpp` | All types, class declaration, static tables |
| `Motor.cpp` | Method implementations |
| `Motor_impl.hpp` | CRTP base or virtual base class — user overrides these |
| `Motor_conf.hpp` | `constexpr` configuration constants |

---

# 3. Configuration — `Motor_conf.hpp`

```cpp
// Motor_conf.hpp — generated skeleton, user may edit
#pragma once

namespace fsm::Motor::conf {

// Queue capacity (must be power of 2)
inline constexpr uint8_t  kQueueCapacity = 8;

// Overflow policy: kQueueAssert | kQueueDropOldest | kQueueDropNewest
inline constexpr auto     kQueueOverflow = kQueueAssert;

// Implementation style:
//   kCrtp            — zero-overhead CRTP base (default)
//   kVtableClassic   — classical virtual method table (easier to mock)
inline constexpr auto     kImplStyle = kCrtp;

// STL profile:
//   kNoStl           — tagged union event, raw function pointers (default)
//   kUseStl          — std::variant event, std::array queue
inline constexpr auto     kStlProfile = kNoStl;

// Timer counter type (unsigned, must hold max timer duration in ms)
using TimerType = uint32_t;

// Define to enable ISR-safe posting
// #define MOTOR_QUEUE_ISR_SAFE

} // namespace fsm::Motor::conf
```

---

# 4. Namespace and Type Definitions — `Motor.hpp`

```cpp
// Motor.hpp — generated, do not edit
#pragma once

#include <stdint.h>
#include <stdbool.h>
#include "Motor_conf.hpp"

namespace fsm {
namespace Motor {

// ── State ID ───────────────────────────────────────────────────────────────
enum class StateId : uint8_t {
    Root    = 0,
    Idle    = 1,
    Running = 2,
    Error   = 3,
    COUNT
};

// ── Event ID ───────────────────────────────────────────────────────────────
enum class EventId : uint8_t {
    Start = 0,
    Stop  = 1,
    Fault = 2,
};

// ── Payload types ──────────────────────────────────────────────────────────
struct StartPayload {
    uint16_t target_speed;
};

// ── Event type (no-STL profile: tagged union) ──────────────────────────────
struct Event {
    EventId id;
    union {
        StartPayload start;
        struct {} stop;
        struct {} fault;
    } payload;

    // Factory helpers
    static Event make_start(uint16_t speed) noexcept {
        Event e; e.id = EventId::Start; e.payload.start.target_speed = speed; return e;
    }
    static Event make_stop() noexcept {
        Event e; e.id = EventId::Stop; return e;
    }
    static Event make_fault() noexcept {
        Event e; e.id = EventId::Fault; return e;
    }
};

// ── Context ────────────────────────────────────────────────────────────────
struct Context {
    // User context fields
    uint16_t speed   = 0;
    bool     running = false;

    // Internal FSM state — do not access directly
    StateId  _state            = StateId::Idle;
    StateId  _history_Main     = StateId::Idle;
    uint8_t  _join_bits        = 0;
    uint32_t _timer_AfterIdle  = 0;

    // Queue
    Event    _queue[conf::kQueueCapacity];
    uint8_t  _queue_head  = 0;
    uint8_t  _queue_tail  = 0;
    uint8_t  _queue_count = 0;
};

} // namespace Motor
} // namespace fsm
```

---

# 5. CRTP Machine Class — `Motor_impl.hpp` (CRTP Profile)

```cpp
// Motor_impl.hpp — CRTP base; user subclasses MotorBase<Derived>
#pragma once
#include "Motor.hpp"

namespace fsm::Motor {

template <typename Derived>
class MotorBase {
public:
    // ── Public API ────────────────────────────────────────────────────────
    void init() noexcept;
    void dispatch(const Event &ev) noexcept;
    void tick(uint32_t elapsed_ms) noexcept;
    void post(const Event &ev) noexcept;

    [[nodiscard]] StateId current_state() const noexcept { return ctx_._state; }
    [[nodiscard]] const Context &context()  const noexcept { return ctx_; }

    // ── User-overridable guards (default: unconditional pass) ─────────────
    bool guard_isSpeedValid([[maybe_unused]] const Context &ctx,
                            [[maybe_unused]] const StartPayload &p) noexcept {
        return true;
    }

    // ── User-overridable entry actions (default: no-op) ───────────────────
    void entry_Idle   ([[maybe_unused]] Context &ctx) noexcept {}
    void entry_Running([[maybe_unused]] Context &ctx) noexcept {}
    void entry_Error  ([[maybe_unused]] Context &ctx) noexcept {}

    // ── User-overridable exit actions ─────────────────────────────────────
    void exit_Idle   ([[maybe_unused]] Context &ctx) noexcept {}
    void exit_Running([[maybe_unused]] Context &ctx) noexcept {}
    void exit_Error  ([[maybe_unused]] Context &ctx) noexcept {}

    // ── User-overridable transition actions ───────────────────────────────
    void action_startMotor([[maybe_unused]] Context &ctx,
                           [[maybe_unused]] const StartPayload &p) noexcept {}
    void action_stopMotor ([[maybe_unused]] Context &ctx) noexcept {}

protected:
    Context ctx_;

private:
    Derived &self() noexcept { return static_cast<Derived &>(*this); }
    const Derived &self() const noexcept { return static_cast<const Derived &>(*this); }

    void dispatch_internal(const Event &ev) noexcept;
    void enqueue_internal(const Event &ev) noexcept;
};

} // namespace fsm::Motor
```

### User Subclass (Application Code)

```cpp
#include "Motor_impl.hpp"

class MyMotor : public fsm::Motor::MotorBase<MyMotor> {
public:
    bool guard_isSpeedValid(const fsm::Motor::Context &ctx,
                            const fsm::Motor::StartPayload &p) noexcept /* CRTP: no override */ {
        return p.target_speed > 0 && p.target_speed <= 3000;
    }

    void action_startMotor(fsm::Motor::Context &ctx,
                           const fsm::Motor::StartPayload &p) noexcept {
        ctx.speed   = p.target_speed;
        ctx.running = true;
    }

    void action_stopMotor(fsm::Motor::Context &ctx) noexcept {
        ctx.speed   = 0;
        ctx.running = false;
    }
};

// Usage:
MyMotor motor;
motor.init();
motor.post(fsm::Motor::Event::make_start(1500));
motor.tick(10);  // advance 10ms
```

No vtable is generated for `MyMotor` — all virtual calls are resolved at compile time
via CRTP static dispatch. Zero overhead compared to direct function calls.

---

# 6. Virtual Method Profile — `Motor_impl.hpp` (Vtable Classic)

When `conf::kImplStyle = kVtableClassic`, the generated base uses pure virtual methods:

```cpp
namespace fsm::Motor {

class MotorBase {
public:
    void init() noexcept;
    void dispatch(const Event &ev) noexcept;
    void tick(uint32_t elapsed_ms) noexcept;
    void post(const Event &ev) noexcept;

    [[nodiscard]] StateId current_state() const noexcept { return ctx_._state; }

    // Pure virtual — user MUST override:
    virtual bool guard_isSpeedValid(const Context &, const StartPayload &) noexcept = 0;
    virtual void action_startMotor(Context &, const StartPayload &) noexcept = 0;
    virtual void action_stopMotor(Context &) noexcept = 0;

    // Virtual with no-op default:
    virtual void entry_Idle(Context &) noexcept {}
    virtual void exit_Idle(Context &) noexcept {}
    // ... etc.

    virtual ~MotorBase() = default;

protected:
    Context ctx_;
};

} // namespace fsm::Motor
```

This profile is easier to mock (use `override` in a Google Mock subclass) but adds
one vtable pointer (4–8 bytes) to the context struct and incurs indirect call overhead.

---

# 7. STL Variant Profile

When `conf::kStlProfile = kUseStl`, the Event type uses `std::variant`:

```cpp
#include <variant>

namespace fsm::Motor {

struct StopTag {};
struct FaultTag {};
using Event = std::variant<StartPayload, StopTag, FaultTag>;

// Distinguish events via std::holds_alternative<T>:
// std::holds_alternative<StartPayload>(ev) → Start
// std::holds_alternative<StopTag>(ev)      → Stop
// std::holds_alternative<FaultTag>(ev)     → Fault
}
```

Queue uses `std::array`:
```cpp
#include <array>
std::array<Event, conf::kQueueCapacity> _queue;
```

The STL profile requires `<variant>` and `<array>` from the C++17 standard library.
It is NOT compatible with `-nostdinc++` or bare-metal targets without a C++ standard
library. Only use on platforms with a full libc++ or libstdc++.

---

# 8. No-STL Profile (Default, Embedded-Safe)

The no-STL profile (default) uses:
- Tagged union for events (same structure as C99)
- Raw C-style arrays for the queue
- No `#include` of any STL header

Compatible with:
- Arduino AVR/ARM
- `-nostdinc++` toolchains
- Any target where `<variant>` is unavailable or too expensive

---

# 9. Thread Safety

| Configuration | Thread Safety |
|---|---|
| Default | Not thread-safe — no atomic operations |
| `MOTOR_QUEUE_ISR_SAFE` defined | `post()` is ISR-safe via `FSM_ENTER_CRITICAL()`/`FSM_EXIT_CRITICAL()` macros from `fsm_hal.h` |

`dispatch()` is NOT thread-safe in any configuration. The caller MUST ensure that
`dispatch()` is not called concurrently from multiple threads.

---

# 10. C++17 Specific Constructs

The generator MUST use the following C++17 features where appropriate:

| Feature | Use case |
|---|---|
| `if constexpr` | Compile-time branch on `kImplStyle`, `kStlProfile` in template methods |
| `[[nodiscard]]` | On `current_state()`, `context()` |
| `[[maybe_unused]]` | On default no-op action parameters |
| `enum class` | State IDs and Event IDs (strong typing, no implicit int conversion) |
| `inline constexpr` | All `Motor_conf.hpp` constants |
| Structured bindings | Optional, in internal table lookup helpers |
| `noexcept` | All dispatch, entry, exit, action, guard functions (exceptions disabled) |
| `= default` / `= delete` | On generated class constructors |

The generator MUST NOT use:
- `std::function` (heap allocation risk)
- `dynamic_cast` (requires RTTI)
- `typeid` (requires RTTI)
- Exceptions (throw/catch)
- Heap allocation via `new`/`delete`

---

# 11. Arduino Integration Example

```cpp
// main.ino
#include "Motor_impl.hpp"

class MotorController : public fsm::Motor::MotorBase<MotorController> {
public:
    bool guard_isSpeedValid(const fsm::Motor::Context &,
                            const fsm::Motor::StartPayload &p) noexcept {
        return p.target_speed >= 100 && p.target_speed <= 2500;
    }

    void entry_Running(fsm::Motor::Context &ctx) noexcept {
        analogWrite(MOTOR_PWM_PIN, map(ctx.speed, 0, 3000, 0, 255));
    }

    void exit_Running(fsm::Motor::Context &ctx) noexcept {
        analogWrite(MOTOR_PWM_PIN, 0);
    }

    void action_startMotor(fsm::Motor::Context &ctx,
                           const fsm::Motor::StartPayload &p) noexcept {
        ctx.speed   = p.target_speed;
        ctx.running = true;
    }

    void action_stopMotor(fsm::Motor::Context &ctx) noexcept {
        ctx.speed   = 0;
        ctx.running = false;
    }
};

MotorController motor;
uint32_t last_tick = 0;

void setup() {
    Serial.begin(115200);
    motor.init();
    last_tick = millis();
}

void loop() {
    // Tick the FSM
    uint32_t now = millis();
    motor.tick(now - last_tick);
    last_tick = now;

    // Handle serial commands
    if (Serial.available()) {
        char cmd = Serial.read();
        if (cmd == 's') {
            motor.post(fsm::Motor::Event::make_start(1500));
        } else if (cmd == 'x') {
            motor.post(fsm::Motor::Event::make_stop());
        }
    }
}
```

---

# 12. Choice Pseudo-State Codegen (C++17)

For `choice` pseudo-states, C++17 uses `if constexpr` when all guards are compile-time evaluable, otherwise a regular `if` chain.

**Runtime guards (typical):**
```cpp
template <typename Derived>
void MotorBase<Derived>::choice_SpeedSelect() noexcept {
    if (ctx_.speed > 100u) {
        // [ctx.speed > 100] -> Fast
        ctx_._state = StateId::Fast;
        self().entry_Fast(ctx_);
    } else if (ctx_.speed > 0u) {
        // [ctx.speed > 0] -> Normal
        ctx_._state = StateId::Normal;
        self().entry_Normal(ctx_);
    } else {
        // [else] -> Stopped
        ctx_._state = StateId::Stopped;
        self().entry_Stopped(ctx_);
    }
}
```

**Static guards (`if constexpr`):**
```cpp
template <typename Derived>
void MotorBase<Derived>::junction_ModeSelect() noexcept {
    if constexpr (conf::kMode == Mode::Fast) {
        ctx_._state = StateId::FastMode;
        self().entry_FastMode(ctx_);
    } else if constexpr (conf::kMode == Mode::Slow) {
        ctx_._state = StateId::SlowMode;
        self().entry_SlowMode(ctx_);
    } else {
        ctx_._state = StateId::DefaultMode;
        self().entry_DefaultMode(ctx_);
    }
}
```

The `if constexpr` form eliminates dead branches at compile time — the resulting binary contains only the matching branch.

---

# 13. Deep History Codegen (C++17)

Deep history uses `std::optional<StateId>` (STL profile) or a sentinel value (no-STL profile) to track whether history has been recorded.

**STL profile:**
```cpp
struct Context {
    // ...
    std::optional<StateId> _deep_history_Operational = std::nullopt;
};

void enter_deep_history_Operational() noexcept {
    StateId restore = ctx_._deep_history_Operational.value_or(StateId::Operational_Normal);
    // Enter all states from Operational down to the restored leaf
    enter_path_to(StateId::Operational, restore);
}
```

**No-STL profile (sentinel):**
```cpp
struct Context {
    // ...
    StateId _deep_history_Operational = StateId::COUNT;  // COUNT = "no history"
};

void enter_deep_history_Operational() noexcept {
    StateId restore = ctx_._deep_history_Operational;
    if (restore == StateId::COUNT) {
        restore = StateId::Operational_Normal;  // default
    }
    enter_path_to(StateId::Operational, restore);
}
```

The `enter_path_to` helper executes entry actions for each state from the composite root down to the leaf, using a `constexpr` array of entry paths (see §18 LCA section).

---

# 14. Deferred Event Codegen (C++17)

**STL profile — `std::queue<Event>`:**
```cpp
#include <queue>

struct Context {
    // ...
    std::queue<Event> _defer_queue;
};

// Defer mask as constexpr array
inline constexpr uint32_t defer_mask[static_cast<size_t>(StateId::COUNT)] = {
    0x00000000u,  // Idle: defers nothing
    (1u << static_cast<uint8_t>(EventId::DataReceived)),  // Connecting: defers DataReceived
    0x00000000u,  // Connected: defers nothing
};

void dispatch(const Event &ev) noexcept {
    auto state_idx = static_cast<size_t>(ctx_._state);
    uint8_t ev_bit = static_cast<uint8_t>(ev.id);
    if (defer_mask[state_idx] & (1u << ev_bit)) {
        ctx_._defer_queue.push(ev);
        return;
    }
    // ... normal dispatch ...
}
```

**No-STL profile — fixed-size circular buffer:**
```cpp
struct DeferQueue {
    Event buffer[conf::kDeferQueueCapacity];
    uint8_t head = 0, tail = 0, count = 0;

    void push(const Event &ev) noexcept {
        FSM_ASSERT(count < conf::kDeferQueueCapacity);
        buffer[tail] = ev;
        tail = (tail + 1u) & (conf::kDeferQueueCapacity - 1u);
        ++count;
    }

    bool pop(Event &out) noexcept {
        if (count == 0u) return false;
        out = buffer[head];
        head = (head + 1u) & (conf::kDeferQueueCapacity - 1u);
        --count;
        return true;
    }
};
```

---

# 15. Internal Transition Codegen (C++17)

Internal transitions use CRTP dispatch without state change — no exit/entry calls.

```cpp
template <typename Derived>
void MotorBase<Derived>::dispatch(const Event &ev) noexcept {
    switch (ctx_._state) {
    case StateId::HBActive:
        switch (ev.id) {
        case EventId::HeartbeatAck:
            // Internal transition — no state change, no exit/entry
            self().action_onHbReceived(ctx_);
            break;
        case EventId::HeartbeatTimeout:
            // External transition: HBActive -> HBFailed
            self().exit_HBActive(ctx_);
            ctx_._state = StateId::HBFailed;
            self().entry_HBFailed(ctx_);
            break;
        default: break;
        }
        break;
    // ...
    }
}
```

The CRTP `self()` call ensures the user's overridden action method is called at zero overhead (resolved at compile time, no vtable indirection).

---

# 16. Parallel Region Encoding (C++17)

C++17 uses `std::tuple` (STL profile) or individual fields (no-STL profile) for region state tracking.

**STL profile — `std::tuple`:**
```cpp
#include <tuple>

struct Context {
    // User fields
    uint16_t speed = 0;
    bool running = false;

    // Parallel region states as a tuple
    std::tuple<StateId, StateId> _region_states = {
        StateId::DataPath_Idle,
        StateId::HeartbeatMonitor_Active
    };

    // Parent state
    StateId _state_parent = StateId::Disconnected;
};

// Access:
auto &data_path_state = std::get<0>(ctx_._region_states);
auto &hb_state = std::get<1>(ctx_._region_states);
```

**No-STL profile — individual fields:**
```cpp
struct Context {
    uint16_t speed = 0;
    bool running = false;

    StateId _state_DataPath = StateId::DataPath_Idle;
    StateId _state_HeartbeatMonitor = StateId::HBMonitor_Active;
    StateId _state_parent = StateId::Disconnected;
};
```

**Dispatch — iterate all regions:**
```cpp
template <typename Derived>
void MotorBase<Derived>::dispatch(const Event &ev) noexcept {
    if (ctx_._state_parent == StateId::Connected) {
        dispatch_region_DataPath(ev);
        dispatch_region_HeartbeatMonitor(ev);
        return;
    }
    // ... non-parallel dispatch ...
}
```

---

# 17. LCA Lookup (C++17)

The LCA table is a `constexpr` array for compile-time evaluation where possible.

```cpp
inline constexpr StateId lca_table
    [static_cast<size_t>(StateId::COUNT)]
    [static_cast<size_t>(StateId::COUNT)] = {
    //          ROOT    IDLE    RUNNING  ERROR   OP_NORMAL  OP_FAST
    /* ROOT */      { StateId::Root, StateId::Root, StateId::Root, StateId::Root, StateId::Root, StateId::Root },
    /* IDLE */      { StateId::Root, StateId::Idle, StateId::Root, StateId::Root, StateId::Root, StateId::Root },
    /* RUNNING */   { StateId::Root, StateId::Root, StateId::Running, StateId::Root, StateId::Root, StateId::Root },
    /* ERROR */     { StateId::Root, StateId::Root, StateId::Root, StateId::Error, StateId::Root, StateId::Root },
    /* OP_NORMAL */ { StateId::Root, StateId::Root, StateId::Root, StateId::Root, StateId::Op_Normal, StateId::Operational },
    /* OP_FAST */   { StateId::Root, StateId::Root, StateId::Root, StateId::Root, StateId::Operational, StateId::Op_Fast },
};

[[nodiscard]] constexpr StateId lca(StateId source, StateId target) noexcept {
    return lca_table[static_cast<size_t>(source)][static_cast<size_t>(target)];
}
```

The `constexpr` qualifier allows the compiler to resolve LCA at compile time for transitions where both source and target are known statically (e.g., in `if constexpr` branches).

---

# 18. Fork Entry Sequence (C++17)

Fork enters multiple regions simultaneously. C++17 uses fold expressions (STL profile) or sequential enter calls (no-STL profile).

**No-STL profile (sequential):**
```cpp
template <typename Derived>
void MotorBase<Derived>::fork_StartAll() noexcept {
    // Enter parent parallel state
    if (ctx_._state_parent != StateId::Monitor) {
        ctx_._state_parent = StateId::Monitor;
        self().entry_Monitor(ctx_);
    }

    // Enter Region 1: Sensors -> Idle
    ctx_._state_Sensors = StateId::Sensors_Idle;
    self().entry_SensorsIdle(ctx_);

    // Enter Region 2: Output -> Off
    ctx_._state_Output = StateId::Output_Off;
    self().entry_OutputOff(ctx_);
}
```

**STL profile (fold expression for N regions):**
```cpp
template <typename... RegionEntries>
void enter_fork(RegionEntries&&... entries) noexcept {
    (entries(), ...);  // fold expression: call each entry lambda
}

// Usage:
enter_fork(
    [&]{ ctx_._state_Sensors = StateId::Sensors_Idle; self().entry_SensorsIdle(ctx_); },
    [&]{ ctx_._state_Output = StateId::Output_Off; self().entry_OutputOff(ctx_); }
);
```

The fold expression approach scales to any number of regions without manual enumeration.

---

# 19. Summary of C99 vs C++17 Pattern Differences

| Pattern | C99 | C++17 |
|---|---|---|
| Choice/Junction | `if/else if/else` chain | `if constexpr` for static guards, regular `if` chain otherwise |
| Deep history field | `Motor_StateId_t _deep_history_X` | `std::optional<StateId>` (STL) or sentinel `StateId::COUNT` (no-STL) |
| Deferred events | `uint32_t` bitmask + circular buffer | `std::queue<Event>` (STL) or fixed circular buffer (no-STL) |
| Internal transitions | Dispatch case without state change | CRTP dispatch without state change |
| Parallel regions | One `_state_regionN` field per region | `std::tuple<StateId, StateId, ...>` (STL) or individual fields (no-STL) |
| LCA table | `static const` array | `inline constexpr` array |
| Fork | Sequential enter calls | Fold expression `(entries(), ...)` (STL) or sequential (no-STL) |

---

# 20. Complete Example — Motor.hpp (3-State Machine)

For the full generated `Motor.hpp` and `Motor.cpp` for the same 3-state machine used
as the C99 example, see the conformance test suite at:

```
tests/04-codegen-cpp/sources/motor-3state.fsm
tests/04-codegen-cpp/expected/Motor.hpp
tests/04-codegen-cpp/expected/Motor.cpp
tests/04-codegen-cpp/expected/Motor_impl.hpp
tests/04-codegen-cpp/expected/Motor_conf.hpp
```

The C++ test harness at `tests/04-codegen-cpp/harness/motor_test.cpp` compiles with:
```
g++ -std=c++17 -fno-exceptions -fno-rtti -Wall -Wextra -Wpedantic -Werror motor_test.cpp Motor.cpp -o motor_test
```

and verifies identical behavioral outcomes to the C99 harness.

---

*End of FSM-SPEC-GEN-CPP v1.0.0*
