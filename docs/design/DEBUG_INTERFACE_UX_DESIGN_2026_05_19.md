# FSM Studio — Interactive `.fsm` Debug / Develop Interface — UX Design

**Document ID:** FSM-DESIGN-DBGUX
**Status:** _Design proposal — owner-collaborative. The 5 forks in §8 are
RECOMMENDED, not locked._
**Date:** 2026-05-19
**Branch:** `phase8.0/debug-interface-ux` (base main `3e4ec53`)
**Author role:** senior UX + senior frontend/tooling (implementer of this design)
**Depends on:** FSM-SPEC-SIM (Doc 13), Doc 30 §4.1 (the keystone), the SoT
tee-up memory `project_embeded_fsm_debug_interface_design_teeup`.
**Scope of this doc:** DESIGN + DOC ONLY. No `crates/` / `Cargo*` / `.github/`
change. No build. This is the design the owner returns to decide; the
implementation is a later, gated minor.

---

## 0. The one-paragraph thesis

A `.fsm` author today writes a state machine *blind*: they edit text, run
`fsm test` / `fsm verify` from a terminal, read a JSON trace, and re-form a
mental model of "what would the machine actually do on event X at time T".
That round-trip is slow and lossy — the diagram (v1.3) is read-only, the
simulator is reachable only through a YAML trace file and the CLI, and there
is no way to *poke* a running machine. This design adds the missing third
verb to the loop — **author → _simulate/step/inspect_ → iterate** — as a
**VS Code debug surface that drives the already-shipped
`fsm_simulator::Interpreter` as the single semantic oracle**. It reuses the
v1.3 diagram, the v1.5 Extension-Host command seam, and the Doc 13 protocol
shape. It introduces **no second FSM semantics anywhere** — that is the
binding keystone and the spine of every decision below.

---

## 1. The developer workflow — the DSL design/debug loop, end-to-end

### 1.1 The loop today (the CLI baseline — what we are improving)

```
   ┌─────────────┐   save    ┌──────────────────────┐
   │ edit foo.fsm │ ───────▶  │ terminal: fsm check  │  (squiggles already
   └─────────────┘           │           fsm verify │   live via LSP — good)
          ▲                  └──────────────────────┘
          │                            │ to actually SEE behaviour:
          │                            ▼
          │                  hand-write foo.trace.yaml  ← author the events,
          │                  (init ctx, the event list,   the clock advances,
          │                   the expected StepRecords)    AND the expected
          │                            │                   output, by hand
          │                            ▼
          │                  terminal: fsm test foo.trace.yaml
          │                            │
          │                            ▼
          └──────────  read raw JSON StepRecord stream in the terminal,
                       re-build the active-config / context mentally,
                       guess which transition the guard picked, edit, repeat
```

Pain points (each is a measured cost this design removes):

| # | Friction today | Cost |
|---|---|---|
| F1 | No way to *interactively* send an event; you must pre-commit the whole event list to a YAML file before seeing step 1. | Every "what if I send STOP here?" is a file edit + full re-run. |
| F2 | The active configuration / context is a JSON blob in a scrollback buffer, not on the diagram. | The author re-derives the statechart position in their head every step. |
| F3 | Virtual-clock / timer behaviour (`after N ms`) is invisible until you encode the exact `advanceClock` deltas and read which timer fired from JSON. | Timer bugs (the F-1 / FW110 class) are found late, by reading traces. |
| F4 | No breakpoints. To inspect "the moment we enter `Running`" you bisect the trace by eye. | Linear scan of every StepRecord. |
| F5 | No rewind. A wrong event means re-run from `init` and re-drive every prior event. | The whole prefix is replayed manually. |
| F6 | The expected-output column of the trace file must be authored *by hand before* you know the answer. | Authoring tests is backwards: you guess outputs, then check. |

### 1.2 The loop with the debug surface (the target)

```
   ┌──────────────────────────────── VS Code ───────────────────────────────┐
   │  ┌───────────────┐   live (on save / on demand)   ┌──────────────────┐ │
   │  │ foo.fsm editor │ ─────────────────────────────▶ │  ⬡ Debug panel   │ │
   │  │  (squiggles +  │ ◀───────────────────────────── │  (this design)   │ │
   │  │   click-to-src)│   click a state → reveal line   └──────────────────┘ │
   │  └───────────────┘                                          │           │
   └────────────────────────────────────────────────────────────┼───────────┘
                                                                 │ every
                              the SHIPPED, never-forked           │ verb is
                              fsm_simulator::Interpreter           ▼ one
                              ── the single oracle ──      Interpreter call
```

The author now does, **without leaving the editor and without writing a
trace file first**:

1. **Open the debug panel** on the active `.fsm` (one command, beside the
   editor — the v1.3 diagram-panel placement convention).
2. **Init** with optional context overrides → the diagram lights up the
   initial active configuration.
3. **Inject an event** (pick from the machine's declared events, fill its
   payload from the typed schema) → the diagram animates the transition,
   the trace timeline gets a StepRecord, the context inspector diffs.
4. **Advance the virtual clock by N ms** → fired timers are listed, their
   resulting steps animate, pending timers show their countdown.
5. **Set a breakpoint** on a state-enter / state-exit / transition by
   clicking its diagram glyph; the next run pauses *exactly there*.
6. **Single-step** the internal RTC queue when paused, watching one micro-step
   at a time (completion events, deferred releases, sub-machine delegation).
7. **Rewind** to any prior StepRecord on the timeline (time-travel) and
   branch a different event from that point.
8. **Promote the session to a `.trace.yaml`** — the events you actually
   injected + the StepRecords the oracle actually produced become a
   regression fixture (this *inverts* F6: the test is recorded, not guessed,
   and is byte-identical to what `fsm test` will assert because it came from
   the same oracle).

Where this saves time vs the CLI: F1–F5 collapse from a file-edit+full-rerun
cycle to a single click each; F6 inverts from "guess then verify" to "explore
then capture". The author iterates the *DSL design itself* by simulation —
the explicit owner ask ("the most convenient way to work with the DSL
designement process").

---

## 2. The surface — panel / webview layout

**Primitive:** a single `WebviewPanel` titled `⬡ <Machine> — Debug`, opened
`ViewColumn.Beside` the editor — the **exact identity + placement contract
the v1.3 `DiagramController` already uses** (`editors/vscode/src/diagram/
diagramPanel.ts`): keyed by **resolved file path** (not machine name, so a
transient parse error does not orphan the panel), CSP-locked, `postMessage`-
only, `retainContextWhenHidden`. The debug panel is the diagram panel **plus
a live overlay and a control rail** — not a new rendering stack.

> **Reuse, not reinvent (binding):** the statechart itself is the v1.3
> `irGraph.ts` pure `IR-JSON → {nodes,edges}` model + the bundled
> `diagramWebview.js` ELK/SVG renderer. The debug overlay is an *additive
> render layer* over that exact model. No second diagram.

### 2.1 Full-panel ASCII mockup (running, paused at a breakpoint)

```
┌─ ⬡ Motor — Debug ───────────────────────────────────────────────[ ⤢ ][ × ]┐
│ ┌─ Transport ────────────────────────────────────────────────────────────┐ │
│ │  [⟲ Init]   [▶ Run]  [⏸ Pause]  [⏭ Step]   ⏱ virtual clock: 5 000 ms   │ │
│ │             status: ● PAUSED — breakpoint  bp-1  (enter Motor.Running)  │ │
│ └────────────────────────────────────────────────────────────────────────┘ │
│ ┌─ Statechart (live) ─────────────────────────┐ ┌─ Inject ───────────────┐ │
│ │                                             │ │ event ▾  START         │ │
│ │   ╭───────╮   START [speed==0]   ╔════════╗ │ │  payload (typed):      │ │
│ │   │ Idle  │ ───────────────────▶ ║Running ║ │ │   target_speed : u16   │ │
│ │   ╰───●───╯  ◀── bp-1 here ──────╢ ┌────┐ ║ │ │   [ 1500          ]    │ │
│ │      ▲ active                    ║ │Norm│◀╫─┐│ │           [ Dispatch ] │ │
│ │      │                           ║ └────┘ ║ ││ ├────────────────────────┤ │
│ │   ╭──┴───╮  STOP    ╭────────╮   ║ ┌────┐ ║ ││ │ ⏱ Advance clock        │ │
│ │   │Fault │◀─────────│ (hist) │   ║ │Fast│ ║ ││ │  by [  500 ] ms  [ ⏩ ] │ │
│ │   ╰──────╯          ╰────────╯   ╚════════╝ ││ │ pending timers:        │ │
│ │   ◍ = breakpoint   ▣ = bp(hit)   ║ active   ││ │  Running.every500ms    │ │
│ │   bold = active config           ╚══════════╝│ │   fires in 500 ms (▮▮░) │ │
│ └─────────────────────────────────────────────┘ └────────────────────────┘ │
│ ┌─ Context ───────────────────────────┐ ┌─ Trace / Timeline (time-travel)─┐ │
│ │ field     type   value      Δ       │ │ # t(ms) kind        detail      │ │
│ │ speed     u16    1500   ▲ was 0     │ │ 0   0   init        →Idle       │ │
│ │ running   bool   true   ▲ was false │ │ 1   0   dispatch    START       │ │
│ │ faults    u8     0                  │ │ 2   0   transition  Idle→Running│ │
│ │                                     │ │ 3 5000  timerFired  every500ms  │ │
│ │ [ ✎ set field… ]  (force; no step)  │ │►4 5000  ‹BREAK bp-1 enter Run.›  │ │
│ │ history: Main.history → Idle        │ │                                 │ │
│ └─────────────────────────────────────┘ │  ◀ rewind to #2   capture ▷ .yaml│ │
│                                          └─────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Why this layout (senior-UX rationale — proximity, no floating cringe)

- **Transport rail spans the top, full width**, holding *only* whole-session
  verbs (Init / Run / Pause / Step) + the clock readout + the status line.
  These act on the *whole machine*, so they sit above the whole surface —
  the affordance is adjacent to its scope. The PAUSED-because-breakpoint
  reason is **in the status line, not a toast** (a transient toast for a
  persistent state is the exact "floating cringe" anti-pattern; the state is
  durable, so the indicator is durable and in place).
- **The Inject panel sits immediately right of the diagram**, because an
  injected event's *effect* is the diagram animating. Trigger and result are
  side-by-side (proximity). The payload form is **revealed inline under the
  event picker the moment an event with a payload is chosen** — the
  revealed-UI-adjacent-to-trigger principle; it does not pop a modal, it
  does not live in a far settings pane.
- **Advance-clock is in the same right rail, directly above the pending-timer
  list it affects.** You set "+500 ms" right next to "this timer fires in
  500 ms" — the control is glued to the data it changes, and the countdown
  bar (`▮▮░`) is a calm, non-animated progress glyph (no spinner theatre).
- **Context inspector is bottom-left, the trace timeline bottom-right.** The
  context **Δ column** shows what the *last step* changed (`▲ was 0`) right
  in the row — you do not diff two JSON blobs by eye (kills F2). `✎ set
  field` is a small inline affordance *in the inspector it mutates*, not a
  floating FAB; its label states the consequence ("force; no step") so the
  user is not surprised that `setContext` fires no transition.
- **The timeline is the time-travel surface.** Each row is a StepRecord; the
  caret `►` marks the current position; `◀ rewind to #N` is a row action
  *on the row*, not a global toolbar button divorced from its target
  (proximity again). `capture ▷ .yaml` is at the timeline's end because the
  timeline *is* the thing being captured.
- **Breakpoint glyphs live on the diagram nodes/edges themselves** (`◍`
  armed, `▣` hit) — you set a breakpoint *where the thing is*, by clicking
  it; you do not manage breakpoints in a detached list (a detached
  breakpoint list is the classic proximity violation; an *optional*
  collapsed "all breakpoints" disclosure is offered for bulk clear, but the
  primary affordance is on the glyph).
- **Theming:** every colour is a `var(--vscode-*)` token (the v1.3 webview
  already does exactly this) — the panel is dark/light/high-contrast correct
  for free, no bespoke palette, no cringe gradients.
- **One panel, four regions, zero chrome that floats.** No speed-dial, no
  hovering action bar, no controls detached from their referent. Every
  affordance is within visual reach of what it changes — the senior-UX 2026
  bar.

### 2.3 The empty / honest states (the v1.3 cardinal-sin bar, inherited)

- **No valid IR yet** (the `.fsm` does not parse / codegen): the panel shows
  the *last valid* statechart + the verbatim v1.3 stale banner `⚠ Diagram
  shows last valid state. Fix parse errors to update.` and the transport is
  **disabled with the reason inline** ("Cannot simulate: fix parse errors").
  It never blanks, never fakes a session — the exact `diagramPanel.ts`
  contract, extended to the transport.
- **Loaded but not init'd:** diagram shown dimmed, only `[⟲ Init]` enabled,
  one-line hint "Press Init to instantiate the machine."
- **Terminated** (final state, no completion transition): status line `●
  TERMINATED`, transport offers only `⟲ Init`; the timeline is fully
  navigable for post-mortem (rewind still works — it is pure snapshot
  restore).

---

## 3. The interaction model — every gesture mapped to the concrete oracle call

This is the heart of the keystone: **every UI verb is exactly one shipped
`Interpreter` method.** No UI verb computes FSM semantics; it only calls the
oracle and renders the returned `StepRecord`(s) / config / context.

| UI gesture | Doc 13 method (the wire name) | The SHIPPED oracle call it maps to (verbatim, `crates/fsm-simulator/src/`) | What the UI renders |
|---|---|---|---|
| Open panel on `foo.fsm` | (client-side) | `emitIr` (reuse v1.3 `diagram/emitIr.ts`) → `Interpreter::new(&ir)` `interpreter.rs:102` | the v1.3 statechart, transport disabled until Init |
| **Init** (opt. context) | `sim/init` / `sim/load` | `Interpreter::init(InitOptions{ machine_name, initial_context, virtual_clock_start_ms })` `interpreter.rs:129` → `Vec<StepRecord>` | initial active-config highlight; StepRecord #0 (`StepKind::Init`) on timeline; context seeded |
| **Inject event** (no payload) | `sim/dispatch` | `Interpreter::dispatch(event_name)` `interpreter.rs:228` → `Vec<StepRecord>` | each StepRecord animates exit/enter on the diagram; timeline appends; context Δ |
| **Inject event + payload** | `sim/dispatch` (`event.payload`) | `Interpreter::dispatch_with_payload(name, Some(payload))` `interpreter.rs:232` | as above; `eventReceived.payload` shown on the timeline row |
| **Advance clock by N ms** | `sim/advanceClock` | `Interpreter::advance_clock(delta_ms)` `interpreter.rs:285` → `Vec<StepRecord>` | fired timers listed; each resulting StepRecord animates; `currentMs` readout updates; pending-timer countdowns recompute |
| **Single-step** (paused) | `sim/step` | drive one queue item: `dispatch`/`advance_clock` already drain the RTC queue to quiescence and return the *ordered* `Vec<StepRecord>`; "step" = **advance the timeline cursor one StepRecord within the already-computed vector** (see §3.1) | one StepRecord revealed; diagram shows that single micro-step |
| **Set breakpoint** on state-enter / state-exit / transition | `sim/setBreakpoint` | **client-side predicate over `StepRecord`** — see §3.2 (NO new semantics: the breakpoint is a *filter on the oracle's own output*, not a re-evaluation of guards) | `◍` glyph on the node/edge; on hit, `▣` + status line + auto-pause |
| **Inspect context** | `sim/getContext` | `Interpreter::context()` `interpreter.rs:397` (+ `current_states_named()` `:357`, `virtual_clock_ms()` `:404`) | the Context region; pure read, never a step |
| **Force-set a field** | `sim/setContext` | (see §3.3 — the one method needing a tiny additive helper; explicitly *not* a transition) | field updates; **no** StepRecord (label says so) |
| **Rewind to StepRecord #N** | (client time-travel) | `Interpreter::snapshot()` `interpreter.rs:415` taken *after every step*; rewind = `Interpreter::restore(snapshots[N])` `interpreter.rs:444` | diagram + context + clock + timeline cursor jump to state #N; new injects branch from there |
| **Capture session → `.trace.yaml`** | (client) | the injected `TraceCommand`s + the oracle's own `Vec<StepRecord>` → `write_trace_yaml` (`trace.rs:274`); `fsm test` later re-runs it via `execute_trace` `trace.rs:280` (the SAME oracle) ⇒ byte-identical by construction | a saved fixture; a "captured ✓" inline confirmation (no premature success — see §6) |
| **Reset** | `sim/reset` | re-`init` (same as Init) | same as Init |

### 3.1 "Single-step" — honest mapping (a design judgment, disclosed)

Doc 13 `sim/step` says "execute one internal step … only when paused". The
shipped `Interpreter` exposes RTC progress at the granularity of
`dispatch` / `advance_clock`, **each of which already returns the *ordered
vector* of every internal StepRecord** that the one external stimulus
produced (completion events, deferred releases, sub-machine delegation are
each their own StepRecord in the vector — see `StepKind` in `trace.rs:113`).

Therefore **single-step is a UI-side cursor over that already-computed,
oracle-produced vector** — *not* a re-implementation of the RTC algorithm.
Concretely: when paused, the very next `dispatch`/`advance_clock` is executed
*eagerly to quiescence by the oracle* (one call), the resulting
`Vec<StepRecord>` is buffered, and `[⏭ Step]` reveals them **one row at a
time** on the timeline + diagram, taking a `snapshot()` per revealed record
so rewind is per-micro-step. This is keystone-clean: the *semantics* (which
records, in what order, with what config deltas) are 100 % the oracle's; the
client only paces the *reveal*. (If the owner later wants true
queue-suspension granularity, that is an additive `Interpreter` capability —
flagged in §8 fork 4 as an explicit MVP scope line, not invented here.)

### 3.2 Breakpoints — snapshot/restore, no second semantics (the keystone in action)

A breakpoint is a **predicate over the StepRecords the oracle emits**, never
a re-evaluation of guards/transitions:

- **state-enter** `S` → `step.entered_states.contains(S)` (`StepRecord.
  entered_states`, `trace.rs:64`).
- **state-exit** `S` → `step.exited_states.contains(S)` (`trace.rs:62`).
- **transition** `T` → `step.transition_taken.stable_id == T` (`trace.rs:
  172` / Doc 13 §11 `transitionTaken`).

Mechanism (pure oracle + snapshot): a run is `dispatch`/`advance_clock`
producing `Vec<StepRecord>`. The client walks the vector; **before revealing
the record that satisfies a breakpoint predicate**, it stops, having already
called `snapshot()` *after the prior record*. "Pause at the breakpoint" =
*restore the snapshot taken just before the breaking step and hold there*
(the machine state is exactly pre-step). Resume = continue revealing the
buffered vector (the oracle already computed it — deterministically). This is
the precise reuse the SoT names: **snapshot/restore = breakpoint +
time-travel; dispatch/advance_clock/step = stepping; StepRecord = the wire
record.** No guard is ever evaluated by the debug layer.

> Doc 13's `sim/breakpointHit` notification (§10) is the wire shape this
> emits to the client; `sim/setBreakpoint`'s `targetId` is the IR stable id
> (the same id `StepRecord.transition_taken.stable_id` / the IR node carries
> — already the click→source correlation key in `irGraph.ts`). Zero new
> identity scheme.

### 3.3 The single honest gap: `sim/setContext`

Every other verb maps to an *already-public* `Interpreter` method. Force-set
(`sim/setContext`) needs a write into the live context map *without* running
a step. The interpreter exposes `context()` read-only and no public mutator.

**Recommendation (NOT a unilateral lock — it touches `crates/`, out of this
doc's scope):** the implementing minor adds **one** minimal, additive,
explicitly-non-stepping helper on `Interpreter` (e.g. `set_context_field`)
that writes the field and **emits no StepRecord** — the keystone is "no
second *transition/guard/step semantics*"; a guarded raw field poke that runs
*nothing* does not implement semantics, it sets state, exactly as
`InitOptions.initial_context` already does at init. If even that is
unwanted, the **fallback with zero `crates/` change** is to scope
`sim/setContext` to *re-init with overridden context* (lossy: it resets the
run). The decision rides §8 fork 4 (MVP scope) — surfaced, not pre-decided.

### 3.4 Worked micro-scenario (the loop, concretely)

> Author writes `state Idle { on START [speed == 0] -> Running }`, suspects
> the guard is wrong.
>
> 1. `[⟲ Init]` → `Interpreter::init` → diagram lights `Idle`; timeline `#0
>    init →Idle`; context `speed=0`.
> 2. Click the `Idle→Running` edge glyph → `◍` (transition breakpoint, bp-1).
> 3. Inject `START` (no payload) → `dispatch("START")`. The oracle returns
>    `[#1 dispatch START, #2 transition Idle→Running …]`. The client had
>    `snapshot()`-ed after `#1`; `#2` matches bp-1 → **restore the pre-`#2`
>    snapshot, status ● PAUSED bp-1**, diagram shows `Idle` still active with
>    the edge highlighted "about to fire".
> 4. Author inspects context: `speed=0` → guard `speed==0` *did* hold; the
>    bug is elsewhere. They `[⏭ Step]` to reveal `#2`, watch `Idle` exit /
>    `Running`+`Running.Normal` enter on the diagram.
> 5. They `◀ rewind to #1`, edit the guard in the editor to `speed > 0`,
>    save (panel auto-re-acquires IR like v1.3), re-inject — now the oracle
>    discards `START` (`StepKind::Discarded`), the timeline says so
>    explicitly. The author *saw* the design flaw by simulation in ~10 s,
>    not by reading a JSON trace.
> 6. `capture ▷ .yaml` → the `START` they injected + the oracle's exact
>    StepRecords become `idle_guard.trace.yaml`; `fsm test` will assert it
>    via the same `execute_trace` oracle — green by construction.

---

## 4. The keystone-honoring architecture

### 4.1 The architecture diagram (there is exactly one semantics)

```
┌──────────────────────── VS Code Extension Host (Node/TS) ───────────────────┐
│                                                                             │
│  Debug WebviewPanel (TS, CSP-locked, postMessage-only)                      │
│  ├─ statechart  = v1.3 irGraph.ts model + diagramWebview.js  (REUSED)       │
│  ├─ overlay     = active-config highlight + breakpoint glyphs (additive)    │
│  ├─ injector / inspector / timeline   (new render, NO semantics)            │
│  └─ breakpoint  = a PREDICATE OVER StepRecord  (NOT guard eval — §3.2)      │
│            │  postMessage JSON only (the v1.3 boundary)                     │
│            ▼                                                                │
│  Debug client glue (TS)  — resolves the .fsm, marshals verbs, paces reveal  │
│            │                                                                │
└────────────┼────────────────────────────────────────────────────────────────┘
             │  exactly one of these two transports (§8 fork 1):
             │
   ┌─────────┴───────────┐                 ┌──────────────────────────────┐
   │ (A) in fsm-lsp:      │   OR            │ (B) standalone fsm simulate  │
   │  `fsm/simulate*`     │                 │  WS daemon, localhost:7842   │
   │  LSP requests        │                 │  JSON-RPC 2.0 (Doc 13 wire)  │
   │  (reuses the v1.5    │                 │  (Doc 13's literal model)    │
   │   A2 `fsm/verify`    │                 │                              │
   │   embed seam)        │                 │                              │
   └─────────┬───────────┘                 └───────────────┬──────────────┘
             │                                              │
             └──────────────────────┬───────────────────────┘
                                     ▼
        ┌──────────────────────────────────────────────────────────┐
        │   T H E   S I N G L E   O R A C L E                       │
        │   crates/fsm-simulator  ::  Interpreter                   │
        │   { new, init, dispatch, dispatch_with_payload,           │
        │     advance_clock, snapshot, restore, context,            │
        │     current_states_named, virtual_clock_ms }              │
        │   + StepRecord (trace.rs)  + execute_trace (replay)       │
        │                                                           │
        │   ── THE SAME library `cmd/test.rs` (execute_trace) and   │
        │      the v1.4 differential and `fsm-verify` already drive  │
        │   ── NO transition-selection / guard-eval / completion /   │
        │      step semantics exist in the WS server OR the client   │
        └──────────────────────────────────────────────────────────┘
```

### 4.2 The no-fork proof obligations (the binding tag-gate, stated up front)

This minor inherits the W1/W2/v1.4/v1.5/factory **keystone phase-audit**
precedent as its **binding tag-gate** (the SoT names this explicitly: "a
keystone phase-audit … will be the binding tag-gate for this minor"). The
audit must independently re-derive **from source**:

1. **Negative-grep** the WS server / LSP-embed AND `editors/vscode/**` debug
   surface for any transition-selection, guard evaluation, completion-event
   synthesis, RTC-step, or active-config-computation logic — every hit read &
   adjudicated; the expected result is **zero** (the breakpoint predicate of
   §3.2 reads `StepRecord` fields only — that is the adjudicated-clean shape).
2. **Byte-identity:** the debug transport's `StepRecord` stream for a fixture
   `== fsm test`'s `execute_trace` output for the same events (the same
   differential discipline as v1.4/factory — the debug surface is just
   another oracle frontend).
3. **The `fsm_simulator` crate + the `cmd/test.rs` seam are byte-untouched**
   by the transport layer (EMPTY structural diff on the oracle, exactly the
   v1.5/factory keystone-audit shape).

Stating these here makes the design *falsifiable against the keystone before
a line ships* — the project's verify-the-record discipline applied forward.

---

## 5. Reuse ledger — what already exists that this rides (no reinvention)

| Need | Existing asset (reused verbatim / extended additively) | Source |
|---|---|---|
| The statechart | v1.3 pure `IR-JSON → {nodes,edges}` model | `editors/vscode/src/diagram/irGraph.ts` |
| Diagram render (ELK/SVG, CSP, nonce, theme tokens) | v1.3 bundled webview | `editors/vscode/src/diagram/webview/diagramWebview.ts` |
| Panel identity / placement / stale-banner / click→source | v1.3 `DiagramController` / `DiagramView` | `editors/vscode/src/diagram/diagramPanel.ts` |
| Get IR for the active `.fsm` (the codegen-gated boundary) | v1.3 `emitIr` | `editors/vscode/src/diagram/emitIr.ts` |
| Extension-Host command + CLI-spawn seam + honest-failure bar | v1.5 A1 `fsm.verify` / `cliRunner` / `cliBinary` | `editors/vscode/src/commands/{verify,cliRunner,cliBinary}.ts` |
| LSP-embed request seam (if fork-1 = ride the LSP) | v1.5 A2 `fsm/verify` round-trip pattern (`client.sendRequest`) | `editors/vscode/src/commands/verify.ts:374-409` |
| The semantics (ALL of it) | the shipped `Interpreter` + `StepRecord` + `execute_trace` | `crates/fsm-simulator/src/{interpreter.rs,trace.rs}` |
| The wire protocol shape | Doc 13 (spec normative; server is what this builds) | `docs/13-Simulator-Protocol.md` |

Net new code is: the overlay render layer, the injector/inspector/timeline
TS, the breakpoint predicate, and **one** transport (fork 1) — every piece
of *semantics* is pre-existing and untouched.

---

## 6. UX guardrails carried from the project's hard-won bars

- **No premature "success."** "Captured ✓" appears only after
  `write_trace_yaml` returns and the file exists (the `copyIr.ts`
  refuse-to-fake-the-clipboard cardinal-sin precedent; the
  no-"Сохранено"-before-server-confirms doctrine, applied here).
- **Honest verdicts.** A `StepKind::Discarded` step is rendered as
  *discarded* ("event had no enabled transition"), never silently dropped —
  the F1/F2 visibility win *and* the verification-UI honesty bar (never a
  false-positive "it worked").
- **Inconclusive ≠ done.** If the run hits the `CompletionLoop` guard
  (`StepError`, `interpreter.rs:84`) or a queue overflow, the status line
  says exactly that with the error verbatim — never a fabricated clean
  end-of-run.
- **Reveal adjacent to trigger** (payload form under the event picker;
  rewind action on the timeline row; breakpoint glyph on the node) — applied
  consistently, no detached chrome.
- **The panel is read-*driving*, the source is the truth.** Edits happen in
  the `.fsm` editor; the panel re-acquires IR on save (v1.3 contract). The
  debug panel never lets you edit the machine *in the panel* — that is the
  future visual-constructor capstone, explicitly out of scope here (no scope
  bleed).

---

## 7. MVP vs full Doc 13 (the scope spine — alternatives are the menu, not drift)

| Tier | Capability | Oracle backing |
|---|---|---|
| **MVP (core)** | `init` · `dispatch`(+payload) · `advanceClock` · context inspect · breakpoints (enter/exit/transition) via snapshot · time-travel rewind · capture→`.trace.yaml` · the v1.3 overlay | all already-public `Interpreter` methods (only `setContext` needs the §3.3 micro-helper or the re-init fallback) |
| **Tier 2** | true queue-suspend single-step granularity · `setContext` mutator (if not in MVP) · watchpoints (context-predicate breakpoints) | small additive `Interpreter` capability — *named*, not invented here |
| **Tier 3** | `multiMachine` (cross-machine `send`) · `replay` at 0/1×/2× speed · multi-instance list | Doc 13 §13 / §8 — the simulator already routes cross-machine internally; UI is the tail |

Discipline: Tier 2/3 are the **menu**, captured so they are *not* mid-epic
drift (the §2.1-style scope-spine the project enforces). The owner cuts the
line in §8 fork 4.

---

## 8. The 5 open owner-forks — decision table (RECOMMENDED, not locked)

> Each row: the fork, the options, the **recommended default + rationale**,
> and the override knob. The owner accepts or overrides any row; nothing
> here is unilaterally locked (the product-owner-autonomy /
> surface-don't-auto-scope discipline).

| # | Fork | Options | **Recommended default** | Rationale (why) | Owner override |
|---|---|---|---|---|---|
| **1** | **Process model** | (A) ride the long-lived `fsm-lsp` via new `fsm/simulate*` LSP requests · (B) standalone `fsm simulate` WS daemon on :7842 (Doc 13 literal) | **(A) ride `fsm-lsp`** — interactive step-debugging IS an editor-coupled, long-lived-session activity; `fsm-lsp` is *already* that process and v1.5 A2 *already* proved the embed seam (`fsm/verify`). Zero new daemon, zero new port, **no new network attack surface** (kills fork 2 almost entirely), keystone-cleanest (same lib, no second process to drift). Doc 30's "the LSP already IS the long-lived process … no new server is ever needed." | A daemon is a *second* long-lived process speaking a *second* protocol to the *same* oracle — pure surface + security cost for an editor-bound use case. Doc 13's wire *shape* is still honored as the message contract **inside** the LSP requests (the protocol is reused as the schema, not as a separate server). (B) stays the documented alternative if a non-VS-Code / web client ever needs it — then it is its own SEC-gated minor. | pick (B) if a headless/3rd-party WS client is a near-term requirement |
| **2** | **Security posture** | localhost-only · + handshake token · full auth | **If (1A): inherits the LSP's existing stdio/trust boundary — no new network surface, no new auth needed (the strongest posture: the attack surface is *removed by construction*, the v1.4/Doc-00-§G-02 "removed by deletion not hardening" pattern).** If (1B) is ever chosen: **localhost-bind ONLY + a per-session handshake token + 4 MB frame cap + reject binary frames** (Doc 13 §1 already specifies localhost+4 MB; add the token) and it gets a **SEC-P0-1-grade pre-tag security lens** (the v1.x four-lens precedent). | The cleanest security review is the one with nothing to review; (1A) achieves that. The (1B) hardening is the *minimum* responsible bar for *any* localhost RPC daemon (loopback is not a trust boundary on a shared dev box — the project already learned the "0.0.0.0 no-auth" lesson, Doc 00 §G-02). | tighten further (mTLS / unix-socket-only) if (1B) and the threat model warrants |
| **3** | **VS Code surface** | Debug Adapter Protocol (DAP) bridge · custom debug webview · extend the v1.5 Extension-Host command seam | **Custom debug webview that *extends the v1.5 Extension-Host command seam* + *reuses the v1.3 diagram panel*** — the statechart-overlay + timeline + injector is inherently a *visual* surface; DAP's variables/call-stack/breakpoints model does not fit a *statechart* (no call stack; "stack frame" ≠ active configuration; the diagram IS the debug view). The webview reuses the entire v1.3 render + v1.5 command/CSP/nonce baseline (C1/C2 frontend-practice baseline). | DAP would force a square peg (linear call-stack mental model) into a round hole (hierarchical active-configuration); we'd fight the protocol and *still* need a custom webview for the diagram. The custom panel is *less* net-new because it stands on v1.3+v1.5. A thin DAP bridge can be added *later* purely for the F5/breakpoint *keybindings* if users ask — additive, not foundational. | add a DAP veneer later if users want F5-style ergonomics; it would still drive the same oracle |
| **4** | **MVP scope cut** | MVP-core only · MVP + Tier 2 · full Doc 13 | **MVP-core (the §7 "MVP" row) for the first minor**, Tier 2/3 named as the explicit menu. Include the §3.3 **micro-helper** for `setContext` (one tiny additive non-stepping mutator) so force-set is real, not the lossy re-init fallback. | The MVP closes F1–F6 *entirely* with **near-zero `crates/` change** (only the one §3.3 helper) — maximal author value per unit of new attack/complexity surface. Tier 2/3 are real but additive and must not balloon the first cut (the scope-spine discipline; the v1.4 "named deferral, not silent drop" precedent). | pull `replay` or `multiMachine` into the first cut if a target use case needs it; or drop the §3.3 helper and accept the re-init fallback |
| **5** | **Epic numbering vs the owner-reserved v1.6** | this minor = v1.6 · = a `checkpoint/` quality milestone · = a v1.6.x / post-v1.6 minor | **Do NOT consume v1.6.** v1.6 is **owner-reserved for the landing-page + doc-honesty pass** (ROADMAP, repeatedly affirmed). Recommend this lands as its **own minor *after* v1.6** (e.g. the next `vX.Y.0` post-v1.6), gated by the §4.2 keystone phase-audit + the §11.30 cold-quad — the proven release-gate cadence. Until tagged it carries the `checkpoint/<name>` immutable-anchor pattern, never a `vX.Y.0` claim. | Unilaterally numbering this v1.6 would consume the owner's reserved minor — the exact unilateral product-numbering the project's verify-the-record / owner-autonomy discipline forbids (the factory-epic set this precedent: a quality milestone that *precedes* v1.6 is a `checkpoint/`, not `v1.6.0`). Sequencing (before/after v1.6, or parallel) is an **owner/orchestrator call** — surfaced here, not resolved. | sequence it before v1.6, or as a v1.6.x point minor, at the owner's discretion |

---

## 9. Assumptions & judgment calls (every one disclosed)

1. **"Single-step" is a UI-paced reveal over the oracle's already-computed
   `Vec<StepRecord>`, not RTC-suspension** (§3.1). Judgment call: this keeps
   the keystone absolute (zero re-implemented stepping). True
   queue-suspension is named as Tier-2 additive, not invented. *If the owner
   wants true suspend-the-queue stepping in MVP, that is a small additive
   `Interpreter` capability — flagged, deliberately not designed into
   `crates/` here (out of this doc's mandate).*
2. **`sim/setContext` needs one additive non-stepping `Interpreter` helper**
   (§3.3); recommended for MVP, with a zero-`crates/`-change re-init
   fallback. Disclosed as the *single* place the MVP is not 100 % covered by
   already-public methods. **Not unilaterally added** — it touches `crates/`,
   explicitly out of this doc's scope; it rides fork 4.
3. **Recommended transport = ride `fsm-lsp` (fork 1A).** This is a
   *recommendation with rationale*, not a lock — the SoT names this the
   genuinely owner-owned central fork. The whole design is transport-agnostic
   below the client glue (the architecture diagram shows both); choosing 1B
   changes only the transport box, not the oracle or the UX.
4. **The breakpoint model is a predicate over `StepRecord` fields**
   (`entered_states`/`exited_states`/`transition_taken.stable_id`), proven
   present in `trace.rs`. Judgment: this is the *only* keystone-clean way to
   do breakpoints (any guard re-evaluation would be a second semantics). It
   does mean a breakpoint resolves *at StepRecord granularity* (you pause
   *before the breaking step*, machine state restored to pre-step via
   snapshot) — which is exactly the debugger-correct "stopped at the
   breakpoint, not past it" behaviour.
5. **DAP rejected as the foundation** (fork 3) — a deliberate design
   judgment that a statechart's hierarchical active-configuration does not
   map to DAP's linear call-stack model; offered as a *later additive
   veneer* for keybindings only. Recommended, overridable.
6. **No `crates/` / `Cargo*` / `.github/` change is proposed as an action**
   in this doc — every such item (the §3.3 helper, any Tier-2 capability) is
   surfaced as an owner/scope decision, honoring the doc's DESIGN-ONLY
   mandate and the project's "surface, don't auto-scope" discipline.
7. **The v1.5 C1/C2 frontend-practice baseline is taken as the bar** (CSP +
   nonce + theme tokens + Extension-Host test discipline + honest-failure) —
   assumed, because the SoT directs reuse of exactly that baseline; the
   deferred-tracked v1.6 FE-hygiene batch (flat-ESLint-9 etc.) is *not*
   pulled into this design (leave-and-explain).
8. **`fsm simulate` does not exist today** (confirmed: `crates/fsm-cli/src/
   cmd/` has no `simulate.rs`; only `test.rs` drives the simulator via
   `execute_trace`). The design therefore specifies the *new* surface; it
   does not assume any unbuilt capability of the oracle beyond the public
   methods read in `interpreter.rs` / `trace.rs`.

---

*End of FSM-DESIGN-DBGUX — design proposal, owner-collaborative; the §8
forks are recommended defaults the owner accepts or overrides.*
