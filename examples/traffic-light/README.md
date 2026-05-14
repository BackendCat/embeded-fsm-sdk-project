# Traffic Light — Composite + Shallow History

A traffic light with a manual-override mode. Demonstrates:

- **Composite state** (`Auto`) containing a cycle of leaf states.
- **Hierarchical dispatch** — `OVERRIDE` is declared on the composite and
  fires from any descendant (per Doc 08 §3 B-10 leaf-to-root walk).
- **Shallow history** (`HAuto`) — when the operator returns to `Auto` via
  `RESUME`, the light resumes at the same colour, not from `Red`.
- **One-shot timers** drive the colour cycle.

## State chart

```
TrafficLight
├── Auto (composite)
│   ├── initial Red
│   ├── shallow_history HAuto { initial Red }
│   ├── Red                — after 2000 ms -> GreenAccelerating
│   ├── GreenAccelerating  — after 2000 ms -> Green
│   ├── Green              — after 30000 ms -> Yellow
│   └── Yellow             — after 5000 ms -> Red
└── Manual
    └── on RESUME -> HAuto      (resumes Auto at last-active colour)

Auto has: on OVERRIDE -> Manual  (composite-parent transition)
```

## Why this example

Auto's `on OVERRIDE -> Manual` proves the leaf-to-root candidate walk: a
`Red`-state leaf must inherit Auto's `OVERRIDE` transition. The
shallow-history transition `Manual -> HAuto` proves the history pseudo-
state is wired correctly to its default target on first entry and to the
recorded child afterwards.

## Build

```sh
fsm check examples/traffic-light/traffic-light.fsm
fsm generate --target c99 examples/traffic-light/traffic-light.fsm \
    --out examples/traffic-light/generated/
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -c examples/traffic-light/generated/*.c
```

## Replay

```sh
fsm test examples/traffic-light/
```

The trace walks one full `Auto` cycle then exercises OVERRIDE / RESUME.

## v1.0 codegen note

The C99 emitter currently produces history-record / history-restore
helpers as `static inline` stubs that are not yet wired into the dispatch
path. Hierarchical transitions and timers compile cleanly; history-pseudo
wiring is tracked as a Phase 2.3 follow-up. The simulator (`fsm test`)
honours history per the formal semantics, so the trace flows correctly.
