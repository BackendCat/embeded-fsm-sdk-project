# Device — Submachine (reusable Connection protocol)

A `Device` whose `Connecting` state *is* a reusable `Connection`
submachine. Demonstrates the v1.1 submachine runtime (Doc 08 §12):

- **`feature submachines`** — `submachine Connection { … }` defines a
  reusable protocol; `state Connecting is Connection { … }` instantiates
  it as a state of `Device`.
- **Independent sub-instance** — entering `Connecting` instantiates a
  nested `Connection` execution context, initialised at the template's
  `initial` state `Idle` (§12.2; no named entry-point ⇒ implicit initial).
- **Event delegation** — `CONNECT` / `ACK` / `ESTABLISHED` are not
  consumed by the parent (`Connecting` only handles `RECONNECT` and its
  `done` completion), so each delegates into the sub-instance, which runs
  `Idle -> Handshake -> Established -> Done` (§12.1, after the established
  transition-wins parent selection).
- **Completion drives the parent** — when the sub reaches its `final`
  (`Done`), the parent receives a synthetic completion and
  `Connecting --done--> Online` fires automatically, with **no external
  event**, through the *same* completion machinery every other `done ->`
  uses (§12.3 — not duplicated).
- **Deterministic teardown** — exiting `Connecting` (here via the `done`
  completion; also via the `RECONNECT` self-transition) drops the
  sub-instance; re-entering `Connecting` re-instantiates a *fresh* one (no
  stale sub-state).

## State chart

```
Device
├── initial Connecting
├── Connecting  is Connection      — done       -> Online
│                                    on RECONNECT -> Connecting
│     └── (sub-instance) Connection
│           ├── initial Idle
│           ├── Idle         — on CONNECT     -> Handshake
│           ├── Handshake    — on ACK         -> Established
│           ├── Established  — on ESTABLISHED -> Done
│           └── final Done
└── Online       — on RECONNECT -> Connecting
```

## Run

```
fsm test examples/
```

`submachine.trace` drives `CONNECT, ACK, ESTABLISHED` and asserts the full
`StepRecord` stream: the `submachine_entered` instantiation, the
`submachine_event_delegated` markers interleaved with the sub-instance's
own (re-tagged) `dispatched` records, the `submachine_completed` detection,
and finally the parent `Connecting --done--> Online` completion. Every
`expected` record was produced by the simulator and hand-verified
record-by-record against Doc 08 §12 (no blind capture).

> Note on `events { … }` in the submachine: the `Connection` template
> declares its events explicitly. A submachine that needs event delegation
> must do so — event delegation re-resolves the parent event *by name*
> against the sub-template's own event table (Doc 08 §12.1, independent
> id namespace), and only an explicitly declared `events { … }` block
> populates that table. (Whether the analyzer should additionally
> auto-collect a template's implicitly-referenced events is a separate
> W2b-scope question and does not affect this example.)

W2d will `gcc`-compile this same example and assert the generated C's
sub-instance behaviour matches this trace byte-for-byte (sim≡codegen).
