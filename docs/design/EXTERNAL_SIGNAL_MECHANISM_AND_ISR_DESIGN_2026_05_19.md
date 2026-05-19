# DESIGN — External-Signal Mechanism (code-verified) + ISR / Polling-Frequency Design Proposal

**Document ID:** FSM-DESIGN-EXTSIG-2026-05-19
**Status:** Part 1 = frozen code-verified ground truth · Part 2 = **PROPOSAL (owner owns every final call)**
**Base:** worktree `embeded-fsm-sdk-wt-extsig`, branch `phase8.0/extsig-isr-design`, off main `3e4ec53`
**Toolchain:** `rustup show active-toolchain` from the worktree → **1.75.0-x86_64-unknown-linux-gnu (overridden by …/rust-toolchain.toml)** — asserted, not assumed.
**Discipline:** READ-ONLY except this doc. No `crates/` / `Cargo*` / `.github/` edit. No `git stash`. Every Part-1 claim is cited to `file:line` or to a reproduced toolchain run; nothing is asserted from memory or from the owner's recollection.
**Owner's verbatim question (the prompt):** *"if the external signal comes from some pin — as i remember there is the possibility to check that signal not with the polling but also with the system interrupt. I think we should mark some external signals as the system interrupts + can the dsl set up the polling frequency? Does it generate the external signals check cycle? How actually does it work with the external signals"* — treated as an UNVERIFIED hypothesis throughout; the recollection ("there is the possibility to check … with the system interrupt") is **tested against code in Part 1**, not assumed.

---

## 0. Method

The frozen `docs/AUDIT_DETERMINISTIC_PRIMITIVE_COMPLETENESS_2026_05_18.md` (FSM-AUDIT-DETPRIM) is taken as the starting SoT and is **reconciled, not contradicted**. Every cell below is established by one of: (i) reading the normative spec text, (ii) reading the actual Rust emitter / IR source, (iii) running the shipped `fsm` CLI on a real `.fsm` and reading the emitted C. The harness: `fsm` `--release` at `/root/dev/embeded-fsm-sdk-target/release/fsm` (`fsm 0.1.0`, the documented shared `CARGO_TARGET_DIR`); probes in `/tmp/extsig-probe` (NOT in the repo). The real example used is `examples/motor/motor.fsm` (it contains a genuine `pure extern can_start() : bool` used as a guard `[can_start]`), plus a derived minimal `every`-internal probe.

---

# PART 1 — HOW EXTERNAL SIGNALS ACTUALLY WORK TODAY (code-verified ground truth)

## 1.1 The one-paragraph truth

**FSM-Lang has no concept of a "pin", "signal", "interrupt", or "polling frequency" anywhere — not in the keyword set, not in the grammar, not in the IR, not in the codegen, not in the HAL spec, not in the simulator.** An "external signal" enters a machine by exactly **one of two host-driven paths**, both of which the host (the user's C, an RTOS task, or an ISR the *user* wrote) decides to invoke:

1. **As an FSM event** the host hands to the runtime: the host calls `M_dispatch(m, &ev)` (synchronous, runs one RTC step now) or `M_post(m, &ev)` (enqueue for a later drain). The runtime **never reads a pin** to discover the event; the host translates "pin changed" → "post `LINK_UP`".
2. **As a `pure extern bool` read inside a guard**, evaluated **on demand at guard-evaluation time** — i.e. only while an event-triggered transition out of the active state is being considered. It is a **pull** ("is the signal true *right now, at the moment a candidate transition asks*"), never a poll loop and never an asynchronous notification.

There is **no generated "external-signals check cycle."** The only generated loop that resembles one is the `after`/`every` *timer* engine inside `M_advance_clock()`, which is a virtual-clock-driven **timer-event synthesizer** — it decrements per-state remaining-ms counters against a host-supplied `elapsed_ms` budget and never reads any extern/pin by itself. The owner's recollection that "there is the possibility to check that signal … with the system interrupt" is **not borne out by the current toolchain**: the *only* ISR-related machinery is the optional `M_post()` critical-section wrapper (`*_QUEUE_ISR_SAFE`) that makes the **user's own ISR** able to call `M_post()` safely — the toolchain neither generates an ISR, nor an interrupt vector, nor any pin-to-event binding.

## 1.2 How an external input is expressed in the DSL today

- **`pure extern f(): bool` is the only "read an external condition" surface.** Doc 04 §2.5 (`extern_decl`, lines 215–253): `[ "pure" ] , "extern" , identifier , "(" , [ param_list ] , ")" , [ ":" , type ]`. "`pure extern` — usable in guard expressions; MUST be side-effect free." (line 226). This is the canonical way a pin's level reaches the FSM: a user-written `bool read_button(void)` referenced as a guard.
- **A `pure extern` can only appear as a guard predicate, never as a trigger.** Doc 04 §8.5 guard grammar (lines 675–702): `guard_expr = identifier (* extern pure fn *) | "!" identifier | field_ref cmp_op literal_or_field | … `. The transition triggers are exhaustively: `on identifier` (§8.1 external / §8.2 internal / §8.3 local, lines 619–660), `done` (§8.4 completion, lines 662–673), and the timer declarations `after`/`every` (§9, lines 927–967). **There is no production by which an extern, a pin, or an external signal is itself a trigger / event source.**
- **Events are abstract identifiers with no hardware binding.** Doc 04 §4.2 (lines 371–398): `event_decl = … identifier , [ "(" payload_field … ")" ]`. Examples are `LINK_UP`, `DATA_RECEIVED(len: u8)` — pure symbols. Nothing in `events_block` ties an event to a pin, an IRQ number, or an acquisition mode.
- **`signal` / `edge` / `rising` / `falling` / `level` / `debounce` / `isr` / `interrupt` / `poll` / `pin` / `frequency` / `hz` are NOT keywords.** Doc 04 §1.5 authoritative keyword list (lines 79–91) — none present. **Behaviourally + code-corroborated:** the shipped lexer (`crates/fsm-lexer/src/token.rs`, `crates/fsm-lexer/src/lexer.rs`) contains **zero** of `"signal" "isr" "interrupt" "poll" "pin" "edge" "rising" "falling" "level" "debounce" "frequency" "hz"` (grep over both files: empty). FSM-AUDIT-DETPRIM §1 P3/P4/P5 already proved `signal`/`debounce`/`rising`/`falling`/`edge`/`level` are not keywords (parse-error `FSM-E0010`); this audit adds `isr`/`interrupt`/`poll`/`pin`/`frequency`/`hz` to the not-a-keyword set by direct lexer inspection. **This reconciles with — and extends — FSM-AUDIT-DETPRIM §1.**
- **IR confirms:** `crates/fsm-ir/src/model.rs:562` `pub enum Trigger { Event{…}, After{duration_ms:u32,…}, Every{period_ms:u32,…}, Completion{from} }` — four variants, no signal/pin/interrupt variant. A `pure extern` lives **only** as `crates/fsm-ir/src/model.rs:611` `GuardExpr::ExternCall { callee, args }` — a guard-tree node, never a trigger.

## 1.3 How `pure extern` guard-reads are realized in the generated C (the killer evidence)

Generated from the real `examples/motor/motor.fsm` (`pure extern can_start() : bool`, used as `likely on START [can_start] -> Running`):

- **`/tmp/extsig-probe/motor_gen/Motor.c:230`** — inside `Motor_try_transitions_in_state`, `case MOTOR_STATE_IDLE → case MOTOR_EVENT_START`:
  ```c
  do {
      if (!MOTOR_LIKELY(can_start())) break;   // line 230  — the extern is read HERE
      Motor_exit_IDLE(m);
      m->context.count = (m->context.count + 1);
      set_speed(100);
      …
  ```
  `can_start()` is called **synchronously, inline, exactly once, only when a `START` event is being dispatched and this candidate transition is evaluated** (Doc 08 §4.3: "Guards MUST be evaluated exactly once per candidate transition per RTC step"). It is a **pull at decision time**. There is no timer, no background read, no ISR, no poll.
- **`Motor.c:336` `Motor_dispatch(Motor_t *m, const Motor_Event_t *ev)`** — the entire dispatcher consumes **one event the host handed it** (`const Motor_Event_t *ev`), runs the per-region ancestor walk (`Motor.c:369–385`), then `Motor_handle_completion`. There is **no extern-poll loop** anywhere in it.
- **`Motor.c:196` `Motor_post()`** — the ISR-safe enqueue seam:
  ```c
  void Motor_post(Motor_t *m, const Motor_Event_t *ev) {
  #ifdef MOTOR_QUEUE_ISR_SAFE
      FSM_ENTER_CRITICAL();
  #endif
      Motor_queue_push(m, ev);
  #ifdef MOTOR_QUEUE_ISR_SAFE
      FSM_EXIT_CRITICAL();
  #endif
  }
  ```
  This is the **only** interrupt-aware code the toolchain emits. It does **not** generate an ISR — it makes `M_post()` *callable from a user-authored ISR* if `MOTOR_QUEUE_ISR_SAFE` is defined (`crates/fsm-codegen-c/src/emit/conf_header.rs:61–63` emits the commented-out `/* #define {prefix}_QUEUE_ISR_SAFE */` opt-in; `crates/fsm-codegen-c/src/emit/hal.rs:64–69` emits the default-no-op `FSM_ENTER_CRITICAL`/`FSM_EXIT_CRITICAL`).
- **`Motor.c:421` `Motor_advance_clock(Motor_t *m, uint32_t elapsed_ms)`** — the *only* generated loop that looks like a "check cycle." It is purely the timer engine: it computes the next timer step, subtracts the host-supplied `elapsed_ms` budget, and on a timer reaching 0 synthesizes a `MOTOR_EVENT_TIMER_…_FIRED` event and dispatches it. **It reads no extern and no pin.** It only advances when the host calls it (`(void)fsm_hal_clock_now_ms();` at `Motor.c:422` is a no-op touch — the *host* owns the time base; Doc 16 §3, lines 76–89).
- The host integration is the proof of the contract: `examples/integration/make/main.c` drives the machine by calling `Motor_dispatch(&mtr, &start)` etc. itself, and `examples/integration/make/motor_externs.c:30` implements `bool can_start(void) { return g_motor_probe.allow_start; }` — a user function read on demand. The user, not the toolchain, decides when an external thing becomes an event.

## 1.4 The HAL boundary (Doc 16) — what it specifies about acquisition + the poll/interrupt line

`docs/16-HAL-Specification.md`:

- **§1 (lines 33–39):** the generated runtime requires exactly **two** platform services — a **clock** (`fsm_hal_clock_ms`) and an **assertion handler** — and *optionally* ISR-safe critical-section macros. **There is no "signal acquisition" HAL hook.** Acquiring an external signal is **out of the HAL contract entirely**: the user reads the pin in their own extern / their own ISR and feeds the FSM via guard-return or `M_post`.
- **§3 (lines 59–89):** the clock is **host-pulled**: `M_tick(elapsed_ms)` (the spec's name; the emitter calls it `M_advance_clock`) "accepts a pre-computed elapsed interval instead of reading the clock directly. This gives the application full control over tick scheduling." The clock requirement (§3 doc-comment, lines 62–73, mirrored in `crates/fsm-codegen-c/src/emit/hal.rs:42–49`) says it MUST be "callable from any context (main loop, task, **ISR**)" — i.e. the HAL anticipates the *user* calling FSM code from an ISR; it does not itself generate one.
- **§5 ISR Safety (lines 127–163):** verbatim — "By default, `Motor_post()` is NOT ISR-safe. If events must be posted from interrupt handlers, define `MOTOR_QUEUE_ISR_SAFE` … and provide critical section macros." **This is the entire interrupt story today**: the *user writes the ISR*, the ISR calls `M_post()`, and `MOTOR_QUEUE_ISR_SAFE` wraps the queue push in a critical section. §6.1 (lines 169–207) shows a Cortex-M reference where the *user* writes `SysTick_Handler`; the toolchain emits none of it.
- **§7 Tick Call Patterns (lines 351–405)** and Doc 25 §1.1 (lines 41–65, "`Motor_post` … ISR-friendly with `MOTOR_QUEUE_ISR_SAFE`") confirm: the canonical loop is host-owned — *drain the queue, then advance the clock* — and the host is responsible for getting external events into the queue.

## 1.5 The simulator (single semantic oracle) — same model, confirming consistency

The shipped interpreter (`crates/fsm-simulator/`) — the **single semantic oracle** the codegen is byte-diffed against (Doc 30 / the v1.4 keystone) — uses the **identical host-driven, pull-on-demand model**, which is why codegen and sim agree by construction:

- Externs are a **static return registry**, not a poll: `crates/fsm-simulator/src/trace.rs:212–221` `extern_returns: Option<BTreeMap<String, Value>>` — "Static return values for extern functions, keyed by extern name." `crates/fsm-simulator/src/eval/extern_registry.rs:50` `pub fn invoke(&self, name, args, guard_context)` is called **on demand during guard evaluation** (`crates/fsm-simulator/src/interpreter.rs:1103`, `:1187`, `:1602` — all `eval_guard` sites), defaulting an unregistered guard extern to `Bool(false)` (`extern_registry.rs:54–55`). No poll loop; no async; no clock dependence in the extern read.
- Events are host-injected: `interpreter.rs:228` `pub fn dispatch(&mut self, event_name)`; the clock is host-pulled: `interpreter.rs:285` `pub fn advance_clock(&mut self, delta_ms)`. Doc 13 §`sim/dispatch` (line 186) + §`sim/advanceClock` (line 306) expose exactly these to the WebSocket protocol — there is no `sim/pinChanged`, no `sim/interrupt`, no `sim/poll`. The oracle has **no notion of a pin or an interrupt at all**.

## 1.6 SUPPORTED / NOT-PRESENT / GAP table (every row cited)

| # | Question | Verdict | Cited evidence |
|---|---|---|---|
| E1 | Is there a DSL way to read an external pin/signal? | **SUPPORTED (only via `pure extern bool` guard)** | Doc 04 §2.5 L226 (`pure extern` usable in guards); §8.5 L681 (`identifier (* extern pure fn *)`); generated `Motor.c:230` `if (!MOTOR_LIKELY(can_start())) break;`; `examples/integration/make/motor_externs.c:30` |
| E2 | Can an external signal be a transition **trigger / event source** itself? | **NOT PRESENT** | Triggers are exhaustively `on`/`done`/`after`/`every` — Doc 04 §8.1–8.4 L619–673, §9 L927–967; IR `Trigger` enum has only `Event/After/Every/Completion` — `crates/fsm-ir/src/model.rs:562`; a `pure extern` is only `GuardExpr::ExternCall` — `model.rs:611` |
| E3 | Is there a generated **external-signals check cycle / poll loop**? | **NOT PRESENT** | `Motor_dispatch` (`Motor.c:336`) consumes one host-supplied event, no extern poll; the only loop is the **timer** engine `Motor_advance_clock` (`Motor.c:421–440`) which reads no extern; no `poll` anywhere in `crates/fsm-codegen-c/src/` (grep: empty) |
| E4 | Does the runtime **poll** externs, and at what cadence? | **NOT PRESENT (pull-on-demand, not poll)** | Extern read happens only at guard-eval of a candidate transition during `M_dispatch` — `Motor.c:230` + Doc 08 §4.3 ("evaluated exactly once per candidate transition per RTC step"); never on a tick / periodic schedule |
| E5 | Is the **timer engine** a periodic check the host calls? | **SUPPORTED (timers only, not externs)** | `Motor_advance_clock` decrements per-state remaining-ms vs host `elapsed_ms`, synthesizes `…_TIMER_…_FIRED` events — `Motor.c:421–440`; `every`-internal probe `/tmp/extsig-probe/poll_gen/M.c:368–388` (re-arms `= 50u`, dispatches synthetic timer event). It is a *virtual-clock timer-event synthesizer*, NOT a pin poll. |
| E6 | Can the DSL set a **polling frequency**? | **NOT PRESENT** | No `poll`/`frequency`/`hz`/`every`-on-an-extern construct; `every N ms` periodicity applies only to *timer transitions/actions* (Doc 04 §9.2/§9.3 L948–967), not to extern/signal acquisition; FSM-AUDIT-DETPRIM §1 corroborates the reactive-vocabulary absence |
| E7 | Is there an **ISR / interrupt / vector** pathway in the toolchain? | **NOT PRESENT (toolchain emits no ISR)** | No `isr`/`interrupt`/`vector`/`nvic`/`primask` in `crates/fsm-codegen-c/src/` except (a) `M_post()` critical-section wrap `conf_header.rs:61–63` + `hal.rs:64–69` + generated `Motor.c:196–204`, (b) `trace_hook.rs` `__attribute__((weak))` (unrelated to IRQ), (c) comments. No interrupt vector, no ISR body, no pin-to-IRQ binding is generated. |
| E8 | What IS the interrupt story today? | **SUPPORTED (user-authored ISR → `M_post()` + `*_QUEUE_ISR_SAFE`)** | Doc 16 §5 L127–163 ("If events must be posted from interrupt handlers, define `MOTOR_QUEUE_ISR_SAFE`"); §6.1 L169–207 (user writes `SysTick_Handler`); Doc 25 §1.1 L58–60 ("`Motor_post` … ISR-friendly"); `Motor.c:196–204` is the only IRQ-aware emitted code |
| E9 | Does this contradict the frozen FSM-AUDIT-DETPRIM? | **CONSISTENT — extends it** | FSM-AUDIT-DETPRIM §1.3 P3: extern/`raise` effect SUPPORTED, declarative `signal` is a GAP — same finding from the input side. §1.5 P5 edge/level GAP — confirmed. This doc adds the *input-acquisition* + *ISR* + *poll-frequency* axis (E2–E8), all NOT PRESENT, none contradicting any frozen cell. |

### Part-1 net verdict

**The owner's recollection is half-true and half-not, precisely:**
- ✅ TRUE that you can read a pin without a poll loop — but the mechanism is a **`pure extern bool` guard pulled at decision time**, not an interrupt.
- ✅ TRUE that an interrupt path exists — but it is **only** "the user's own ISR may call `M_post()`, optionally critical-section-guarded by `*_QUEUE_ISR_SAFE`"; the toolchain does **not** generate an ISR, an interrupt vector, or any pin↔IRQ binding, and there is **no DSL way to mark a signal as interrupt-backed**.
- ❌ NOT TRUE that the DSL can set a polling frequency, or that the toolchain generates an external-signals check cycle — neither exists. The only generated periodic machinery is the deterministic `after`/`every` *timer* engine, which reads no externs.

---

# PART 2 — DESIGN PROPOSAL (PROPOSED — owner owns every final call)

> Everything below is a **proposal**. It is deliberately conservative against the project invariants: **deterministic, heap-free, HAL-mandatory, the single-simulator-oracle keystone (no second semantics), Doc 02 §G3 Declarative Purity, Doc 04 §1.5/§12 append-only keyword/grammar conventions, Doc 08 §13.5 absolute-clock non-observability**. Where a choice touches language philosophy (is an input a *declared signal* or an *extern effect*?) it is flagged **owner-owned**, mirroring FSM-AUDIT-DETPRIM §1.3 P3 / §2.4. Nothing here is implemented.

## 2.0 Framing: the central design tension

FSM-Lang's semantics are an **event/timer machine over an abstract clock with no absolute-time and no I/O surface** — that is exactly *why* the v1.4 verifier is sound (Doc 08 §13.5). Any "external signal" feature must preserve two things simultaneously:
1. **The oracle keystone.** The shipped simulator is the *single* semantic authority; codegen must be byte-equal to it. So whatever a pin/ISR feature lowers to, it must be **expressible as ordinary FSM events the simulator already understands** — the new surface must *desugar into events/timers*, never introduce a second execution model the simulator can't reproduce deterministically.
2. **Determinism + heap-freedom.** An ISR is inherently asynchronous and non-deterministic *in wall-clock arrival*. The only way to feed a deterministic core from an ISR without breaking it is the pattern the codebase already uses for the queue: **the ISR's sole effect is to set/enqueue a flag/event; the FSM consumes it later, at a deterministic RTC step, exactly as if the host had posted it.** The deterministic core never runs *inside* the ISR.

Both constraints point to the same architecture: **a `signal` is sugar for "an extern bool the toolchain knows how to sample, plus (optionally) an ISR-to-event bridge", and it always reduces to the existing event/guard/timer primitives the simulator already executes.** This keeps "one core, many frontends" intact.

## 2.1 (a) Marking external signals as ISR-backed vs polled

### 2.1.1 DSL syntax proposal — a first-class `signal` declaration

A new **top-level declaration** (additive to `top_level_decl`, Doc 04 §2 L126–131), gated by a new `feature signals` flag (Doc 04 §2.2). Two acquisition modes, declared explicitly:

```fsm
// polled: the toolchain samples this extern bool on a schedule (see 2.2)
signal door_closed : bool = poll read_door_closed every 20 ms

// interrupt-backed: a user-authored ISR calls the generated post-hook;
// the rising/falling edge becomes a named FSM event the machine reacts to
signal estop : bool = interrupt {
    on rising  -> raise ESTOP_PRESSED
    on falling -> raise ESTOP_RELEASED
}

// edge-debounced polled signal (debounce-at-the-edge, see 2.1.4)
signal button : bool = poll read_button every 5 ms debounce 30 ms {
    on rising -> raise BUTTON_DOWN
}
```

Proposed grammar (EBNF, Doc-04-§9-style; new reserved words **append-only** per Doc 00 §B-03, a minor-version event): add `signal`, `poll`, `interrupt`, `edge`, `rising`, `falling`, `debounce` to Doc 04 §1.5. *(Note: `edge`/`rising`/`falling`/`debounce` are the exact words FSM-AUDIT-DETPRIM §1.4/§1.5 classified as GAPs — this is the cohesive feature that retires those GAPs as **owner-owned language-roadmap** work, consistent with §2.4 LARGER/LATER.)*

```ebnf
signal_decl   = [ doc_comment ] , "signal" , identifier , ":" , "bool" , "=" , signal_source ;
signal_source = poll_source | interrupt_source ;
poll_source   = "poll" , identifier , "every" , const_expr , "ms" ,
                [ "debounce" , const_expr , "ms" ] , [ edge_block ] ;
interrupt_source = "interrupt" , [ edge_block ] ;          (* edge_block strongly recommended *)
edge_block    = "{" , { edge_rule } , "}" ;
edge_rule     = "on" , ( "rising" | "falling" ) , "->" , "raise" , identifier ;
```

- The referenced `read_*` symbol is **exactly today's `pure extern … : bool`** — *no new HAL hook, no new I/O contract*. Acquisition stays the user's job; the language only schedules the sample and/or wires the edge to an event.
- A bare `signal s : bool = poll read_s every N ms` with no `edge_block` is usable in guards as `[s]` — sugar for "the most recently sampled value of `read_s`", giving polled signals a debounced/sampled guard read without an `edge_block`.

### 2.1.2 Codegen + HAL contract for the ISR→FSM-event bridge

The ISR path lowers to **exactly the existing `M_post()` + `*_QUEUE_ISR_SAFE` machinery** (Part 1 §1.3 / Doc 16 §5) — no new runtime model:

- For each `interrupt`-mode signal `S` with `on rising -> raise E_R` / `on falling -> raise E_F`, the toolchain emits **one ISR-callable hook**:
  ```c
  /* User calls this from THEIR pin-change ISR. ISR-safe iff M_QUEUE_ISR_SAFE. */
  void Motor_signal_estop_isr(Motor_t *m, bool new_level);
  ```
  whose body is: debounce-bookkeeping (2.1.4), edge-detect against the last stored level, and on a qualifying edge `Motor_post(m, &(Motor_Event_t){ .id = MOTOR_EVENT_ESTOP_PRESSED })`. **The ISR's sole effect is enqueue** — identical to the pattern Part 1 verified at `Motor.c:196–204`. The deterministic RTC step runs later, in the host's drain loop, consuming `ESTOP_PRESSED` as an ordinary event. The generated edge events (`ESTOP_PRESSED`/`ESTOP_RELEASED`) are auto-added to the machine's event enum exactly like any `events {}` entry.
- **The toolchain still does not write the ISR vector or attach the IRQ.** The user wires their NVIC/AVR/CLIC handler to call `Motor_signal_estop_isr()`, exactly as today they wire `SysTick_Handler` to call the tick (Doc 16 §6.1). This preserves the "the toolchain emits no `__attribute__((interrupt))`/vector" invariant verified in Part 1 §1.6 E7, and keeps target-portability a HAL concern not a codegen concern (→ 2.4).

**Determinism / atomicity / re-entrancy concerns (the load-bearing part):**
- *Determinism is preserved* because the ISR contributes **only an enqueued event**; the FSM's reaction is a normal RTC step whose order is fully determined by the queue. The simulator reproduces it by the host injecting the *same* event sequence — there is no new nondeterminism the oracle can't model (it models *which events arrive in which order*, never wall-clock arrival, and never did — Doc 08 §13.5).
- *Atomicity* is the existing concern, already solved: posting from an ISR must be wrapped by `FSM_ENTER_CRITICAL/EXIT_CRITICAL` (`*_QUEUE_ISR_SAFE`). The generated `*_signal_*_isr` MUST route its post through `M_post()` (never a direct `M_dispatch`, which would run the whole RTC step *inside* the ISR — forbidden). A compile-time check should reject an `interrupt` signal unless `*_QUEUE_ISR_SAFE` is configured (proposed `FSM-Exxxx` "interrupt-backed signal requires ISR-safe queue").
- *Re-entrancy:* the last-level + debounce state for each signal must be a per-`M_t` field updated **only inside the critical section** (or be a single `volatile bool`/atomic the platform guarantees atomic). The generated hook must not call back into `M_dispatch`. The deterministic core is **never** re-entered from the ISR — it only ever runs from the host's single-threaded drain.
- *Heap-free:* signal state is fixed fields on `M_t` (one `bool` level + optional debounce counter per signal); zero allocation, consistent with Doc 02 §G2 and Doc 16 §8 "Heap: 0 bytes".

### 2.1.3 Verifier interaction

ISR-injected events are, to the verifier, just **events that can occur** — the explicit-state engine already explores "any event from the alphabet may arrive." So an `interrupt`-mode signal adds its edge events to the event alphabet and the existing reachability/deadlock analysis covers them with **no soundness change** (it does not read the clock; Doc 08 §13.5 untouched). This must be confirmed with a `fsm verify` golden, but is safe-by-construction (same class as the watchdog argument in FSM-AUDIT-DETPRIM §2.1: "no clock *read*, so it does not disturb §3").

### 2.1.4 Debounce-at-the-edge interaction

Debounce is **time-filtered edge detection** — and FSM-AUDIT-DETPRIM §1.4 P4 flagged plain `debounce` a GAP precisely because it is "semantically a timer+guard composite." Here it is well-scoped because it lives **inside the signal abstraction, lowering to the existing deterministic timer**:
- **Polled + debounce:** the sampler reads `read_s()` every `every N ms`; a candidate edge starts a `debounce K ms` countdown reusing the **exact existing `after`/timer machinery** (per-state remaining-ms decremented in `M_advance_clock`, Part 1 §1.3); the edge `raise` fires only if the level is still the new value when the debounce timer expires. Fully deterministic, virtual-clock-driven, oracle-reproducible — it is *literally* an internal `after K ms`-armed boolean, the bisimilarity-preserving "relative timer in disguise" shape FSM-AUDIT-DETPRIM §3 direction (b) explicitly blesses.
- **Interrupt + debounce:** the simplest sound model is **hardware/RC debounce is the user's responsibility, OR** the generated ISR hook only records the raw edge + a timestamp-free "pending" flag and a *polled* debounce-confirm timer (same as above) decides. Recommendation: **v1 supports `debounce` only on `poll` signals**; `interrupt + debounce` is a documented later increment (an ISR-side software debounce needs a timer read inside the ISR, which fights determinism and atomicity — defer it, owner-owned).
- **Determinism guarantee:** debounce MUST lower to the existing timer engine (no new clock surface, no absolute-time read) — this keeps Doc 08 §13.5 intact and the verifier sound by construction.

## 2.2 (b) DSL-settable polling frequency

### 2.2.1 Syntax

Already shown in 2.1.1: `poll read_s every <const_expr> ms`. The frequency is `every N ms` — **deliberately reusing the existing `every` timer keyword and the compile-time `const_expr` discipline** (Doc 04 §9; FSM-AUDIT-DETPRIM §1.1: durations are folded `u32`, must stay compile-time-constant to keep §13.5 sound). No floating Hz, no runtime-variable rate (a runtime-variable poll period would be a runtime-variable timer duration — squarely inside the FSM-AUDIT-DETPRIM §3 / §1.1 deferred envelope; **explicitly excluded** from this proposal).

### 2.2.2 What the generated check-cycle would look like

A polled signal lowers to an **internal `every N ms` timer whose action samples the extern and updates the signal's stored level (+ runs debounce/edge logic)** — i.e. it *reuses the verified `every`-internal mechanism from Part 1 §1.3* (`/tmp/extsig-probe/poll_gen/M.c:368–388`):

```c
/* inside M_advance_clock's existing timer loop — one extra per-signal timer */
if (signal_door_timer reached 0) {
    signal_door_timer = 20u;                  /* re-arm (drift-free, Doc 08 §13.3) */
    bool lvl = read_door_closed();            /* the user's pure extern, sampled */
    /* edge/debounce bookkeeping → maybe Motor_post(&DOOR_OPENED/DOOR_CLOSED) */
    m->_signal_door_closed_level = lvl;        /* fixed M_t field, heap-free */
}
```

This is **not new machinery** — it is the existing deterministic timer-event synthesizer (Part 1 §1.3/§1.6 E5) with a per-signal action that happens to call a `pure extern`. The poll "cycle" *is* the `every`-timer cycle; the host already drives it via `M_advance_clock(elapsed)`.

### 2.2.3 Interaction with the deterministic HAL clock seam + the simulator oracle

- **HAL clock seam (Doc 16):** unchanged. Polling is driven by the **same host-supplied `elapsed_ms`** the timer engine already consumes (Part 1 §1.4). No new HAL hook (acquisition is still the user's `pure extern`; only the *scheduling* is generated). The Doc 16 §9 Nyquist note ("`Motor_tick()` called at ≥ 2× the smallest timer") naturally extends to "≥ 2× the smallest poll period."
- **Simulator oracle consistency (the keystone):** this is the critical constraint and it is **satisfiable by construction** *iff* the polled `pure extern`'s value at each virtual sample tick is **deterministic in the simulator**. Today the simulator models externs as a **static return registry** (Part 1 §1.5, `trace.rs:212`, `extern_registry.rs:50`) — a constant value. For sim≡codegen byte-equality the simulator's `door_closed` sample sequence must be the *same* as the generated C's. Two oracle-consistent options (owner picks):
  - **(b-i) Static value (v1, lowest risk):** the polled extern returns its registered static value at every sample tick in both sim and codegen. Trivially oracle-consistent; covers "signal is stable then changes once" via the existing `extern_returns` + a scripted change. Recommended for v1.
  - **(b-ii) Scripted sample stream:** extend the trace/`StepRecord` + sim driver with a per-signal *scripted sample timeline* (sample i → value), and the codegen-side differential harness feeds the generated C the identical scripted reads (the W1 trace-differential seam — FSM-AUDIT-DETPRIM §1.3 P3b confirmed `StepRecord` is the additive seam). Higher fidelity, strictly an **append-only `StepRecord` extension** (same discipline as the `submachine` field), no second semantics. Recommended as the v2 increment.
- **§13.5 untouched:** the poll timer is a relative `every N ms` armed timer; it never exposes the absolute clock; it reuses the exact const-folded timer path. The v1.4 digest's clock-origin merge stays sound (FSM-AUDIT-DETPRIM §3 "Note — this also binds P1"): durations stay compile-time constant by construction.

## 2.3 (c) Current-vs-proposed generated external-signal check cycle (before/after)

| Aspect | **CURRENT (verified Part 1)** | **PROPOSED (Part 2)** |
|---|---|---|
| Read a pin level in a guard | `pure extern bool`, pulled at guard-eval of a candidate transition (`Motor.c:230`) | Unchanged for ad-hoc; `signal s … poll … ` adds a *sampled/debounced* `[s]` guard reading the last sampled level |
| Periodic acquisition | **None.** No generated extern poll; only the `after`/`every` *timer* engine (`Motor.c:421`), which reads no extern | Polled signal = an internal `every N ms` timer action that samples the `pure extern` + does edge/debounce → reuses the **exact existing timer engine** |
| Frequency control | **None** (no `poll`/`frequency`/`hz`) | `poll … every <const> ms` (compile-time const, drift-free re-arm, Doc 08 §13.3) |
| Interrupt path | **User writes the ISR**, calls `M_post()`; optional `*_QUEUE_ISR_SAFE` critical section (`Motor.c:196–204`). Toolchain emits no ISR/vector | Toolchain emits a **`*_signal_*_isr(m, level)` hook** the user calls from *their* ISR; hook does edge-detect + `M_post(edge_event)`. Still no generated vector/`__attribute__((interrupt))` |
| Edge → event | Not possible (extern can't be a trigger — Part 1 E2) | `on rising/falling -> raise EVT` desugars to a generated event the FSM reacts to as an ordinary event |
| Debounce | **GAP** (FSM-AUDIT-DETPRIM §1.4) | `debounce K ms` on `poll` signals → internal `after K ms` confirm timer (existing engine); `interrupt+debounce` deferred |
| Determinism / oracle | Deterministic; sim = static extern registry, host-injected events | Preserved: ISR→event = enqueue-only; poll = relative timer; sim consistency via static value (v1) or scripted sample stream (v2, append-only `StepRecord`) |
| HAL surface | clock + assert (+ optional critical macros) | **Unchanged** — no new HAL hook; acquisition stays the user's `pure extern` |

## 2.4 (d) Per-target interrupt-model implications — dependency on the target-chip-matrix thread

The ISR→FSM-event bridge is **deliberately target-agnostic by design**: the toolchain emits a *plain C function* (`*_signal_*_isr`) the user calls from their own handler, and the critical section is the existing HAL macro pair. This is the same portability strategy Part 1 verified for `M_post()` (Doc 16 §6.1 Cortex-M PRIMASK, §6.2 FreeRTOS, §6.5 AVR `SREG`/`cli`). **However**, the *user-facing integration guidance* (how to attach the hook to an interrupt vector, priority/masking caveats, the critical-section primitive) differs per family:

- **Cortex-M (NVIC):** vectored, `__attribute__((interrupt))` not needed (CMSIS naming), PRIMASK/BASEPRI critical section, EXTI line config.
- **AVR:** `ISR(PCINT0_vect)` macro, `cli()/SREG` critical section, pin-change interrupt groups.
- **RISC-V (CLIC/PLIC):** CLIC vectored vs PLIC claim/complete, `mstatus.MIE` / `mie` masking — materially different from NVIC.

**This is an explicit dependency on the parallel target-chip-matrix thread. This document does NOT research chips.** What is needed *from* that thread to finalize Part 2(a)/(d):
1. The per-family **critical-section primitive** and whether the existing `FSM_ENTER_CRITICAL/EXIT_CRITICAL` HAL pair is sufficient for pin-change-IRQ masking on each (it is for the queue today; confirm for signal-state updates).
2. The recommended **interrupt-attach idiom** per family for the Doc 16 §6-style reference block (user-written, not generated).
3. Any family where **enqueue-from-ISR cannot be made atomic** with the current HAL contract (would force a design exception — e.g. lock-free SPSC queue), so the proposal can pre-empt it.
4. Confirmation that **no family needs the toolchain to emit a vector/`__attribute__`** (the design assumes it never does — must hold across the matrix to keep the codegen invariant from Part 1 §1.6 E7).

Until that thread reports, Part 2(a)'s ISR codegen is **specified but not finalized**; Part 2(b) polling has **no per-target dependency** (it is pure timer-engine reuse + a `pure extern` call) and could proceed independently.

## 2.5 Invariant-alignment checklist (proposal vs project keystones)

| Invariant | How the proposal honors it |
|---|---|
| Deterministic | ISR = enqueue-only (reaction is a normal RTC step); poll = relative const-folded timer; debounce = internal `after`; no absolute-clock surface (Doc 08 §13.5 untouched) |
| Heap-free | Signal level + debounce counter are fixed `M_t` fields; zero allocation (Doc 16 §8) |
| HAL-mandatory, no new HAL surface | Acquisition stays the user's `pure extern`; only scheduling/edge-wiring is generated; critical section = existing HAL macro pair |
| Single simulator oracle (no second semantics) | Everything desugars to **events + `every` timers the simulator already executes**; sim consistency via static value (v1) or append-only `StepRecord` scripted stream (v2) — never a second interpreter |
| Doc 02 §G3 Declarative Purity | `signal` is structural wiring ("sample this extern, raise this event on this edge"); all *logic* stays in the user's `pure extern` C. Whether to add `signal` at all vs. keep "inputs are externs" is **owner-owned** (same call as FSM-AUDIT-DETPRIM §1.3 P3 / §2.4) |
| Doc 04 §1.5/§12 conventions | New keywords are **append-only minor-version** (Doc 00 §B-03); new `signal_decl` is additive to `top_level_decl`; gated by `feature signals` (Doc 04 §2.2) |
| Doc 02 §18 out-of-scope respected | No probabilistic/non-deterministic automaton introduced; ISR arrival is modeled as event-may-occur (already in scope), not stochastic |
| FSM-AUDIT-DETPRIM reconciliation | This *is* the cohesive language-roadmap feature that the frozen audit's §2.4 routed edge/level/debounce/`signal` into ("LARGER/LATER, design-led, owner-owned"); fully consistent, contradicts no frozen cell |

---

# 3. Judgment calls disclosed

1. **`signal` as sugar-over-extern (not a new I/O HAL hook).** I propose acquisition stays the user's `pure extern`; the language only schedules/edge-wires it. Rationale: preserves Doc 02 §G3, the single-oracle keystone, and the "no new HAL surface" invariant. The alternative (a real signal-acquisition HAL hook) is heavier and is **owner-owned** — flagged, not chosen.
2. **ISR path = enqueue-only via existing `M_post()`/`*_QUEUE_ISR_SAFE`, toolchain emits a hook not a vector.** This is the *only* design that keeps the deterministic core untouched and matches the verified Part 1 §1.3 pattern. I treated "generate the ISR vector" as **out of scope / against the verified invariant** (Part 1 E7) rather than a viable option.
3. **`interrupt + debounce` deferred to a later increment; v1 debounce is `poll`-only.** ISR-side software debounce needs a timer read inside the ISR (fights determinism/atomicity). Disclosed as a deliberate scope cut; owner may overrule.
4. **Polling frequency = `every <const> ms` only (no runtime-variable rate, no Hz float).** A runtime-variable poll period is a runtime-variable timer duration → squarely in the FSM-AUDIT-DETPRIM §3/§1.1 *deferred* envelope. I excluded it to keep §13.5 sound by construction; this is a constraint, not a preference.
5. **Sim-consistency: static value (v1) recommended over scripted stream (v2).** Lowest risk to the keystone; the scripted-stream path is specified as the append-only `StepRecord` v2 increment (consistent with FSM-AUDIT-DETPRIM §1.3 P3b's confirmed seam). Owner picks the phasing.
6. **The frozen FSM-AUDIT-DETPRIM is authoritative SoT and was reconciled, not contradicted.** Where Part 1 adds axes the frozen audit didn't cover (input-acquisition, ISR, poll-frequency — E2–E8), I extended rather than overrode; every such cell is independently cited.

# 4. Explicitly NOT verified / flagged-open (not fabricated)

- **Per-target interrupt-model specifics (NVIC vs AVR vector vs RISC-V CLIC/PLIC).** Deliberately **not researched** here (it is the parallel target-chip-matrix thread's job). Part 2(d) lists exactly what I need back; until then Part 2(a) ISR codegen is *specified, not finalized*.
- **Whether the owner wants a declarative `signal` primitive at all** vs. keeping "external inputs are externs/`raise`." This is a **language-philosophy product decision the owner owns** (Doc 02 §G3) — flagged identically to FSM-AUDIT-DETPRIM §1.3 P3; I did not pre-decide it.
- **Exact new diagnostic codes** (e.g. "interrupt signal requires ISR-safe queue", "poll period must be compile-time constant") — proposed by intent only; code-point allocation is a Doc 10 exercise, not done here (no `crates/`/doc edits permitted).
- **The owner's original *source* of the recollection.** The owner said "as i remember"; I found **no** prior DSL-level interrupt/poll-frequency feature in code or spec. I did not fabricate one; Part 1 reports the actual mechanism (extern-guard + user-ISR-`M_post`) which is the most likely thing being half-remembered. If the owner has a prior design note for an interrupt-backed signal, it is **not in this tree** and should be supplied.
- **Build-not-run note:** I ran `fsm generate`/`fsm check` (read-only) and read the emitted C; I did **not** compile the generated C with `gcc` (not required for a mechanism audit; the emitted text is the evidence). Flagged for completeness.

---

*End of FSM-DESIGN-EXTSIG-2026-05-19. Part 1 = code-verified ground truth (frozen). Part 2 = PROPOSAL; the owner owns every final call.*
