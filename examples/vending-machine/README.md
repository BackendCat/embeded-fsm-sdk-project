# Vending Machine — Parallel Regions + Completion

A vending machine with two orthogonal concerns. Demonstrates:

- **Parallel regions** — Payment and Selection regions of `Operational`
  execute independently and receive every event.
- **Completion (`done`) transitions** — each region transitions to a
  region-local `final` state once its sub-flow is complete.
- **Composite-parent transition** — `Operational on RESET -> Done`
  unconditionally exits the composite from any leaf in either region.
- **Guards on context** — `[ctx.balance >= ctx.price]` gates dispensing.

## State chart

```
VendingMachine
├── Operational (parallel)
│   ├── region Payment
│   │   ├── initial Idle
│   │   ├── Idle           on COIN -> CoinInserted (accumulates balance)
│   │   ├── CoinInserted   on COIN [self], on DISPENSE [balance>=price] -> ChangeAvailable
│   │   ├── ChangeAvailable done -> PaymentFinal
│   │   └── final PaymentFinal
│   └── region Selection
│       ├── initial Browsing
│       ├── Browsing       on SELECT -> Selected
│       ├── Selected       on DISPENSE -> Dispensing  (calls extern dispense_item())
│       ├── Dispensing     done -> SelectionFinal
│       └── final SelectionFinal
└── Done

Operational has: on RESET -> Done
```

## Why this example

- The `DISPENSE` event reaches BOTH regions in the same RTC step — the
  guard on the Payment region filters by balance, the Selection region
  uses it to transition from `Selected` to `Dispensing`. This proves
  B-08 (parallel completion semantics).
- The `done` transitions in each region depend on the per-region final
  states (`PaymentFinal`, `SelectionFinal`) and have unique generated C
  identifiers thanks to the codegen now respecting per-final-state names.
- `on RESET -> Done` declared on the composite parent exits both regions
  cleanly via the LCA path.

## Build

```sh
fsm check examples/vending-machine/vending-machine.fsm
fsm generate --target c99 examples/vending-machine/vending-machine.fsm \
    --out examples/vending-machine/generated/
gcc -std=c99 -Wall -Wextra -Wpedantic -Werror \
    -c examples/vending-machine/generated/*.c
```

## Replay

```sh
fsm test examples/vending-machine/
```

The trace inserts 2 coins (balance = 200, price = 150), selects an item,
dispenses (both regions advance), then RESET → `Done`.
