# FSM Studio — Formal Execution Semantics

**Document ID:** FSM-SPEC-SEM
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-DSL

Defines the run-to-completion (RTC) step semantics for FSM-Lang machines. Every
conformant implementation MUST produce behavior identical to the algorithm specified
here for all inputs.

---

# 1. Definitions

| Term | Definition |
|---|---|
| **Configuration** | The set C of currently active basic states. |
| **Basic state** | A state of kind `simple`, `final`, or a parallel state's region when no substate is active. |
| **Completion event** | An internal event automatically generated when a `final` state is entered. |
| **RTC step** | One complete run-to-completion cycle: dequeue one event, select and execute all enabled transitions, stabilize. |
| **Enabled transition** | A transition T is enabled iff its source is in C, its trigger matches the current event (or is a completion), and its guard evaluates to true. |
| **LCA** | Lowest Common Ancestor — the deepest state that is an ancestor of both source and target of a transition. |
| **Exit set** | The set of states that are exited during a transition — all states from source up to (but not including) the LCA. |
| **Entry set** | The set of states that are entered during a transition — from the LCA down to (and including) the target, plus any required initial substates. |
| **Internal transition** | A transition that does not exit or re-enter the source state. No LCA computation required. |

---

# 2. Configuration

## 2.1 Formal Definition

The configuration C is a set of state node IDs. The following invariants MUST hold at
all times between RTC steps:

1. **Completeness.** For every orthogonal region R in the state tree: exactly one state
   in R is in C.
2. **Consistency.** If a composite state S is in C, then exactly one of its substates is
   also in C.
3. **Parallel completeness.** If a parallel state S is in C, then for every region R of
   S, exactly one state in R is in C.
4. **Root coverage.** The root region always has exactly one state in C.

## 2.2 Initial Configuration

At machine initialization (`M_init`):

```
C = ∅
execute_entry_sequence(root.initial_target, ctx)
```

`execute_entry_sequence` is defined in §7.

## 2.3 Active Configuration Representation

In generated C code, the active configuration MUST be representable as a set of state
IDs stored in a fixed-size array or bitfield. For non-parallel machines, exactly one
state ID is stored at any time.

---

# 3. Run-to-Completion Step

## 3.1 Event Processing

```
procedure RTC_step(M, event):
    transitions ← select_transitions(M, C, event)
    if transitions = ∅:
        return   (* event discarded — no enabled transition *)
    exit_states ← compute_exit_set(transitions)
    record_history(exit_states)
    execute_exit_actions(exit_states)
    execute_transition_actions(transitions)
    entry_states ← compute_entry_set(transitions)
    execute_entry_actions(entry_states)
    update_configuration(exit_states, entry_states)
    handle_completion_events(M)
```

## 3.2 Queue Drain Policy

One event is dispatched per call to `M_dispatch()`. Timer events are synthesized by
`M_tick()` and inserted into the event queue as ordinary events. Internal `raise`
events are processed BEFORE the next external event (they are inserted at the front of
the queue).

```
M_dispatch(M, event):
    RTC_step(M, event)
    while internal_queue is not empty:
        e ← dequeue_front(internal_queue)
        RTC_step(M, e)
```

---

# 4. Transition Selection Algorithm

## 4.1 Full Algorithm

```
function select_transitions(M, C, event) → TransitionSet:
    selected ← ∅
    done     ← ∅   (* states already covered by a selected transition *)

    for each basic_state s in C (ordered innermost first):
        if s ∈ done: continue

        (* Walk up from s to root collecting enabled transitions *)
        for each state ancestor of s (s itself, then s.parent, ... , root):
            candidates ← transitions of ancestor
                where trigger matches event
                and guard(t) = true

            if candidates ≠ ∅:
                (* Choose by priority, then document order *)
                t ← min(candidates, key = (t.priority, t.document_order))
                selected.add(t)
                done.add_all(exit_set(t))
                break   (* found a transition for this state; stop walking up *)

    (* Conflict check: error if two selected transitions have overlapping exit sets *)
    for each pair (t1, t2) in selected × selected where t1 ≠ t2:
        if exit_set(t1) ∩ exit_set(t2) ≠ ∅:
            raise FSM-E0300   (* caught at compile time; runtime should not reach here *)

    return selected
```

## 4.2 Priority Rule

When multiple transitions from the same source match the same event:
1. **Lower priority number wins** (priority 1 beats priority 100).
2. On equal priority, **document order** determines winner (first declared wins).
3. **Inner transition wins over outer** — a transition on a substate always takes
   precedence over a transition on an ancestor for that region. This is implemented by
   the innermost-first walking order in the algorithm above.

## 4.3 Guard Evaluation

Guards MUST be evaluated exactly once per candidate transition per RTC step. Guards MUST
NOT have side effects (enforced by the `pure` extern requirement for extern guards; the
compiler MUST verify that inline guard expressions contain no assignments).

## 4.4 Completion Transitions

A completion transition has `trigger = null`. It is selected if and only if the current
state is a `final` state and no event-triggered transition is enabled. Completion
transitions MUST NOT have a guard (FSM-E0301 if present).

---

# 5. LCA Algorithm

## 5.1 Definition

The Lowest Common Ancestor (LCA) of states S and T is the deepest state in the state
tree that is an ancestor of both S and T (inclusive — a state is its own ancestor).

## 5.2 Algorithm

```
function LCA(source, target) → state:
    ancestors_of_source ← path_to_root(source)   (* ordered: source first, root last *)
    for each a in path_to_root(target):
        if a ∈ ancestors_of_source:
            return a
```

```
function path_to_root(state) → list:
    path ← [state]
    while state.parent ≠ null:
        state ← state.parent
        path.append(state)
    return path
```

## 5.3 LCA for Parallel States

When source and target are in different regions of a parallel state P, the LCA is P
itself. Both regions are affected: the source region exits; the target region enters.
The parallel state P is NOT exited or re-entered.

---

# 6. Exit Sequence

## 6.1 Computation

```
function compute_exit_set(transitions) → ordered list of states:
    exit_set ← ∅
    for each transition t in transitions:
        lca ← LCA(t.source, t.target)
        (* Add all states from source up to (not including) LCA *)
        s ← t.source
        while s ≠ lca:
            exit_set.add(s)
            s ← s.parent
    (* Order: innermost first *)
    return topological_sort(exit_set, order = innermost_first)
```

## 6.2 Execution Order

Exit actions MUST be called in **innermost-first** order. For a configuration
`Root → Operational → Running`:

```
call exit_Running()
call exit_Operational()
(Root is the LCA — not exited)
```

## 6.3 Parallel Regions Exit Order

When exiting a parallel state, all regions MUST be exited before the parallel state
itself. Within a parallel state's regions, exit order is reverse of declaration order
(last declared region exits first).

```
(parallel state Monitor with regions: Sensors, Output, Log — declared in this order)
exit order: Log.current_state, Output.current_state, Sensors.current_state, Monitor
```

## 6.4 History Recording

History pseudo-states record the exiting configuration BEFORE exit actions run.
The recorded value is the active basic state (shallow) or the full active descendant
set (deep). Recording happens immediately upon determining exit_set, before any exit
action is called.

---

# 7. Entry Sequence

## 7.1 Computation

```
function compute_entry_set(transitions) → ordered list of states:
    entry_set ← ∅
    for each transition t in transitions:
        lca ← LCA(t.source, t.target)
        (* Add all states from LCA down to target (not including LCA) *)
        path ← path_from_lca_to_target(lca, t.target)
        entry_set.add_all(path)
        (* Expand composite and parallel initial substates *)
        entry_set.add_all(expand_initial(t.target))
    return topological_sort(entry_set, order = outermost_first)
```

## 7.2 Execution Order

Entry actions MUST be called in **outermost-first** order. For entering
`Operational → Running`:

```
call entry_Operational()
call entry_Running()
```

## 7.3 Initial Substate Expansion

When a composite state is entered, its initial substate MUST also be entered (unless
the transition targets the composite directly and the composite has a history, in which
case history restoration applies). This process recurses until a basic state is reached.

```
function expand_initial(state) → list of states:
    if state is simple or final: return []
    if state is composite:
        initial ← state.initial_target
        return [initial] + expand_initial(initial)
    if state is parallel:
        result ← []
        for each region r in state.regions:
            result += [r.initial_target] + expand_initial(r.initial_target)
        return result
```

---

# 8. History Semantics

## 8.1 Shallow History

Shallow history stores the **direct active child** of the parent composite state.
On the next `~>` (history transition) into that composite, that direct child is entered
(and its own initial expansion runs normally).

```
Composite: Operating
  Shallow history: ~>History
  States: Idle, Working, Finishing

If configuration was Operating.Working when Operating was exited:
  history_Operating = Working
Next ~>History entry: enters Working (then expands Working's initial substate if composite)
```

## 8.2 Deep History

Deep history stores the **full deepest active descendant** of the parent composite.
On restore, the exact leaf state is re-entered via the full path.

## 8.3 First Entry (No History Stored)

If no history is stored and a `~>` transition is taken:
- If `history` has a `defaultTarget`: enter that state.
- If no `defaultTarget`: FSM-W0100 is emitted at compile time. At runtime, the
  machine MUST emit an assertion failure (FSM_ASSERT) or enter the initial substate
  as a fallback (implementation-defined; MUST document the choice).

---

# 9. Completion Events

## 9.1 Generation

A completion event is generated when:
- A `final` state is entered as part of the entry sequence.
- The parent composite state's region has reached its terminal condition.

## 9.2 Queue Position

Completion events MUST be placed at the **front** of the internal event queue, NOT the
back. They are processed before any queued external events.

## 9.3 Propagation

A completion event propagates outward: if the outermost composite state that contains
the `final` state has a completion transition enabled, that transition fires. This
continues upward until no enabled completion transition exists.

## 9.4 Infinite Loop Protection

If more than **100 consecutive completion events** fire without an external event being
processed, the machine MUST halt and emit FSM-E0900. Conformant implementations MUST
detect this at runtime; the compiler SHOULD warn about cycles detectable statically.

---

# 10. Deferred Events

## 10.1 Deferral Mechanism

When an event E matches a `defer E` declaration in the current state, E is placed in
the **defer set** instead of being dispatched normally.

## 10.2 Release

When the machine exits to a state that does NOT have `defer E`, all events in the defer
set are released: they are prepended to the external event queue in FIFO order (the
order they were deferred), **before** the next external event.

## 10.3 Queue Position on Release

```
queue (before release): [A, B, C]
defer set:              [D, E]   (D deferred before E)
queue (after release):  [D, E, A, B, C]
```

## 10.4 Recursion Prevention

Deferred events MUST NOT be immediately re-deferred if the new state also has `defer E`.
The release only happens on exit from the last deferring state in the ancestor chain.

---

# 11. Fork/Join Semantics

## 11.1 Fork

A `fork` pseudo-state simultaneously enters all its target states, which MUST be initial
states of distinct regions within the same parallel state.

```
fork targets: [Sensors.initial, Output.initial]
entry order: Sensors.initial (and expansion), then Output.initial (and expansion)
(region declaration order)
```

## 11.2 Join Condition

A `join` pseudo-state fires its outgoing transition when ALL source states listed in
`sources` are simultaneously in the active configuration. The join MUST be reached from
different regions of the same parallel state.

```
function evaluate_join(join, C) → bool:
    return all(s ∈ C for s in join.sources)
```

## 11.3 Join Out-of-Order Exit

Regions may reach their join source states in any order. The implementation MUST use a
bitfield (or equivalent) to track which sources have been reached. The transition fires
only when all bits are set.

```
join_bits[join_id] |= bit_for(exited_region)
if join_bits[join_id] = all_bits_set:
    fire join transition
```

---

# 12. Submachine Semantics

## 12.1 Context Separation

A submachine reference creates an independent execution context. The submachine's
context fields are separate from the parent machine's context. Events are NOT
automatically forwarded between parent and submachine.

## 12.2 Entry

Entry into a submachine reference:
1. If the transition targets a named entry point EP: enter the submachine at EP's
   connected state.
2. If no entry point: enter the submachine at its initial state.

## 12.3 Exit

Exit from a submachine reference:
1. If a final state with an exit point mapping is reached: the parent machine receives
   a synthetic exit event and transitions via the corresponding exit point.
2. On parent-triggered exit (external transition leaving the submachine reference
   state): the submachine is fully exited (all exit actions called, innermost first).

---

# 13. Timer Lifecycle

## 13.1 Start Condition

A timer declared with `after X ms` or `every X ms` MUST start exactly when the
**owning state** is entered. "Entered" means the entry action for that state has been
called (or would be called — the timer starts even if the state has no entry action).

For a timer owned by a composite state: the timer starts on entry to the composite
itself, not on entry to any substate.

## 13.2 Cancel Condition

A timer MUST be cancelled exactly when the **owning state** is exited. If the owning
state is re-entered via a self-transition (external), the timer MUST be cancelled and
restarted.

Internal self-transitions do NOT cancel timers.

## 13.3 Every-Timer Restart

`every X ms` timers re-arm automatically after each firing. The timer is not restarted
from the firing moment — it is restarted from the scheduled moment to prevent drift:

```
next_fire = last_scheduled_fire + X
```

## 13.4 Timer Interaction with Virtual Clock

In the simulator's virtual clock mode, `M_tick(elapsed)` MUST process all timers that
would have fired in the elapsed interval, in chronological order.

---

# 14. Queue Drain Policy

| Policy Rule | Normative Requirement |
|---|---|
| Event dispatch | One external event per `M_dispatch()` call. |
| Internal events | `raise` inserts at front of internal queue; drained before next external. |
| Timer events | Inserted by `M_tick()`; treated as external events; processed in order of firing time. |
| Deferred events | Released on exit from deferring state; prepended to external queue. |
| Completion events | Inserted at front of internal queue; processed immediately. |

---

# 15. Worked Example — TrafficLight

## 15.1 Machine Definition

```
machine TrafficLight {
    context { green_ms: u32 = 30000; }
    event TIMER;
    initial -> Red;

    state Red   { after 30000ms -> Green; }
    state Green { after ctx.green_ms ms -> Yellow; }
    state Yellow { after 5000ms -> Red; }
}
```

## 15.2 Step-by-Step Execution

**Step 0 — Init:**
- C = ∅
- execute_entry_sequence: enter Red
- C = { Red }
- Timer `AfterRed` started (30000ms)

**Step 1 — M_tick(30001):**
- Timer `AfterRed` fires → synthetic TIMER event inserted
- M_dispatch(TIMER):
  - select_transitions: Red has `after 30000ms -> Green` with trigger = timer TIMER_AfterRed
  - transition Red → Green: LCA = root
  - exit_set = { Red }
  - history: none
  - execute exit_Red() (no-op)
  - execute transition actions (none)
  - entry_set = { Green }
  - execute entry_Green() (no-op)
  - C = { Green }
  - Timer `AfterRed` cancelled; Timer `AfterGreen` started (30000ms from ctx.green_ms)
  - no completion event (Green is not final)

**Step 2 — M_tick(30001) (relative to Green entry):**
- Timer `AfterGreen` fires → TIMER dispatched
- select: Green → Yellow
- exit Green, enter Yellow
- C = { Yellow }
- Timer `AfterYellow` started (5000ms)

**Step 3 — M_tick(5001):**
- Yellow → Red
- C = { Red }
- Timer `AfterRed` restarted

The cycle repeats indefinitely. No nondeterminism, no completion events, no history.

---

*End of FSM-SPEC-SEM v1.0.0*
