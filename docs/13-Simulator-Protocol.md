# FSM Studio — Simulator WebSocket Protocol Specification

**Document ID:** FSM-SPEC-SIM
**Version:** 1.0.0
**Status:** Normative Draft
**Depends on:** FSM-SPEC-IR, FSM-SPEC-SEM

Defines the JSON-RPC 2.0 over WebSocket protocol between the FSM simulator daemon and
its clients (VS Code extension, Web IDE, test scripts). All tooling that needs to
run or observe a state machine MUST use this protocol.

---

# 1. Transport

| Attribute | Value |
|---|---|
| Protocol | WebSocket (RFC 6455) |
| Subprotocol | `fsm-sim-v1` (sent in HTTP Upgrade request) |
| Frame type | Text frames (UTF-8 JSON) — binary frames MUST be rejected |
| Max message size | 4 MB (server MUST close connection on oversize message) |
| Listen address | `localhost` only by default |
| Default port | `7842` (configurable with `--port`) |

---

# 2. JSON-RPC 2.0 Framing

All messages follow JSON-RPC 2.0 exactly:

**Request:**
```json
{ "jsonrpc": "2.0", "id": 1, "method": "sim/load", "params": { ... } }
```

**Response (success):**
```json
{ "jsonrpc": "2.0", "id": 1, "result": { ... } }
```

**Response (error):**
```json
{ "jsonrpc": "2.0", "id": 1, "error": { "code": -32000, "message": "FSM-SIM-E001: ...", "data": { ... } } }
```

**Notification (server → client, no `id`):**
```json
{ "jsonrpc": "2.0", "method": "sim/stateChanged", "params": { ... } }
```

Batch requests are NOT supported. If a batch array is received, respond with error
`-32600` (Invalid Request) for the entire batch.

---

# 3. Handshake

Immediately after the WebSocket connection is established, the server MUST send a
`sim/ready` notification:

```json
{
  "jsonrpc": "2.0",
  "method": "sim/ready",
  "params": {
    "protocolVersion": "1.0.0",
    "serverVersion": "0.1.0",
    "capabilities": ["virtualClock", "replay", "breakpoints", "multiMachine"]
  }
}
```

The client MUST check that the `protocolVersion` major number matches. If it does not,
the client MUST disconnect.

---

# 4. Machine Management Methods

---

### `sim/load`

Load an IR document into the simulator as a named machine instance.

**Request params:**
```json
{
  "ir": { /* MachineObject from FSM-SPEC-IR */ },
  "instanceId": "motor-1"   /* optional; auto-generated UUID if absent */
}
```

**Response:**
```json
{
  "instanceId": "motor-1",
  "machineName": "Motor",
  "contextSchema": [
    { "name": "speed",   "type": "u16", "default": 0 },
    { "name": "running", "type": "bool", "default": false }
  ]
}
```

**Error:** `FSM-SIM-E001` if the IR fails to load (schema validation error, unknown
state kinds, cyclic submachines, etc.).

---

### `sim/unload`

**Request:** `{ "instanceId": "motor-1" }`

**Response:** `{ "success": true }`

---

### `sim/listInstances`

**Request:** `{}`

**Response:**
```json
{
  "instances": [
    { "instanceId": "motor-1", "machineName": "Motor", "status": "running" },
    { "instanceId": "ctrl-1",  "machineName": "Controller", "status": "paused" }
  ]
}
```

`status` values: `"loaded"` (not yet initialized), `"running"`, `"paused"`,
`"terminated"` (reached a final state with no completion transition).

---

# 5. Simulation Control Methods

---

### `sim/init`

Initialize (or reset) a machine instance to its initial configuration.

**Request:**
```json
{
  "instanceId": "motor-1",
  "context": { "speed": 0, "running": false }   /* optional; overrides defaults */
}
```

**Response:**
```json
{
  "configuration": { "activeStates": ["Motor.Idle"] },
  "context": { "speed": 0, "running": false },
  "traceId": 0
}
```

---

### `sim/dispatch`

Send an event to a machine instance. Processes the event and all resulting internal
events (completion, deferred releases) in one call.

**Request:**
```json
{
  "instanceId": "motor-1",
  "event": {
    "name": "START",
    "payload": { "target_speed": 1500 }
  }
}
```

**Response:**
```json
{
  "steps": [ /* array of StepRecord */ ],
  "configuration": { "activeStates": ["Motor.Running.Normal"] },
  "context": { "speed": 1500, "running": true }
}
```

Returns an empty `steps` array if the event was discarded (no enabled transition).
Returns multiple `steps` if completion events or deferred event releases occurred.

---

### `sim/step`

Execute one internal step (for debugging at the step level). Only available when
the instance is paused.

**Request:** `{ "instanceId": "motor-1" }`

**Response:** `StepRecord` (or `null` if no pending event)

---

### `sim/pause`

Pause event processing. Subsequent `sim/dispatch` calls will queue events but not
process them until `sim/resume`.

**Request:** `{ "instanceId": "motor-1" }`
**Response:** `{ "success": true }`

---

### `sim/resume`

Resume event processing. Drains any events queued during pause.

**Request:** `{ "instanceId": "motor-1" }`
**Response:** `{ "steps": StepRecord[], "configuration": ..., "context": ... }`

---

### `sim/reset`

Reset to initial configuration without unloading the machine IR.

**Request:** `{ "instanceId": "motor-1", "context": { ... } }`
**Response:** same as `sim/init`

---

# 6. Context Inspection Methods

---

### `sim/getContext`

**Request:** `{ "instanceId": "motor-1" }`
**Response:** `{ "context": { "speed": 1500, "running": true } }`

---

### `sim/setContext`

Force-set context fields. Useful for test setup. Does NOT trigger any transitions.

**Request:** `{ "instanceId": "motor-1", "fields": { "speed": 0 } }`
**Response:** `{ "context": { "speed": 0, "running": true } }`

---

### `sim/getConfiguration`

**Request:** `{ "instanceId": "motor-1" }`
**Response:**
```json
{
  "configuration": {
    "activeStates": ["Motor.Running.Normal"],
    "historyStates": { "Motor.Main.history": "Motor.Idle" }
  }
}
```

---

# 7. Virtual Clock Methods

---

### `sim/setClockMode`

**Request:** `{ "instanceId": "motor-1", "mode": "virtual" }`

Modes:
- `"wall"` — simulator uses real wall clock via `fsm_hal_clock_ms()` (default)
- `"virtual"` — simulator uses manual `sim/advanceClock` calls; real time is ignored

**Response:** `{ "mode": "virtual", "currentMs": 0 }`

---

### `sim/advanceClock`

Advance the virtual clock by `deltaMs` milliseconds. Fires all timers that expire
within the interval, in chronological order.

**Request:** `{ "instanceId": "motor-1", "deltaMs": 5000 }`

**Response:**
```json
{
  "firedTimers": ["Motor.Idle.after5000ms"],
  "steps": [ /* StepRecord[] resulting from timer fires */ ],
  "currentMs": 5000
}
```

---

### `sim/getClockState`

**Request:** `{ "instanceId": "motor-1" }`
**Response:** `{ "mode": "virtual", "currentMs": 5000, "pendingTimers": [{ "name": "Motor.Running.every500ms", "firesAtMs": 5500 }] }`

---

# 8. Trace Methods

---

### `sim/getTrace`

**Request:** `{ "instanceId": "motor-1", "since": 5 }` — `since` is a traceId (returns records with id ≥ since)

**Response:** `{ "records": TraceRecord[] }`

---

### `sim/clearTrace`

**Request:** `{ "instanceId": "motor-1" }`
**Response:** `{ "success": true }`

---

### `sim/replay`

Submit a previously captured trace for replay. The machine is reset and all events
in the trace are re-dispatched in order.

**Request:**
```json
{
  "instanceId": "motor-1",
  "trace": [ /* TraceRecord[] */ ],
  "speed": 0.0   /* 0.0 = instant, 1.0 = real-time speed, 2.0 = 2x speed */
}
```

**Response:** `{ "success": true }`

During replay, `sim/stateChanged` notifications are emitted as each step replays.
New `sim/dispatch` calls during replay return error `FSM-SIM-E007`.

---

# 9. Breakpoint Methods

---

### `sim/setBreakpoint`

**Request:**
```json
{
  "instanceId": "motor-1",
  "type": "state_enter",   /* "state_enter" | "state_exit" | "transition" */
  "targetId": "Motor:state:Running"   /* stable ID */
}
```

**Response:** `{ "breakpointId": "bp-001" }`

---

### `sim/clearBreakpoint`

**Request:** `{ "breakpointId": "bp-001" }`
**Response:** `{ "success": true }`

---

### `sim/listBreakpoints`

**Response:**
```json
{
  "breakpoints": [
    { "breakpointId": "bp-001", "type": "state_enter", "targetId": "Motor:state:Running", "enabled": true }
  ]
}
```

---

# 10. Notifications (Server → Client)

Notifications are sent without a corresponding request. The client MUST handle them
asynchronously.

---

### `sim/stateChanged`

Sent after every RTC step (dispatch, timer fire, completion event, replay step).

```json
{
  "jsonrpc": "2.0",
  "method": "sim/stateChanged",
  "params": {
    "instanceId": "motor-1",
    "step": { /* StepRecord */ },
    "configuration": { "activeStates": ["Motor.Running.Normal"] },
    "context": { "speed": 1500, "running": true }
  }
}
```

---

### `sim/breakpointHit`

Sent when execution reaches a breakpoint. The machine is paused automatically.

```json
{
  "jsonrpc": "2.0",
  "method": "sim/breakpointHit",
  "params": {
    "instanceId": "motor-1",
    "breakpointId": "bp-001",
    "step": { /* StepRecord */ }
  }
}
```

---

### `sim/timerFired`

```json
{
  "method": "sim/timerFired",
  "params": {
    "instanceId": "motor-1",
    "timerStableId": "Motor:timer:AfterIdle",
    "atMs": 5000
  }
}
```

---

### `sim/error`

Async error from the simulator (e.g., FSM-E0900 completion chain depth exceeded at
runtime). The machine transitions to `"terminated"` status.

```json
{
  "method": "sim/error",
  "params": {
    "instanceId": "motor-1",
    "code": "FSM-SIM-E900",
    "message": "Completion event chain depth exceeded 100"
  }
}
```

---

# 11. Data Schemas

### ActiveConfiguration

```json
{
  "activeStates": ["Motor.Running", "Motor.Running.Normal"],
  "historyStates": {
    "Motor:history:MainHistory": "Motor:state:Idle"
  }
}
```

All state references use dot-path notation (`Machine.State.Substate`) for human
readability and stable ID notation (`Machine:state:Name`) for toolchain correlation.
Both MUST be present when the stable ID is known.

### ContextSnapshot

```json
{
  "fields": {
    "speed":   { "value": 1500, "type": "u16" },
    "running": { "value": true, "type": "bool" }
  }
}
```

### StepRecord

```json
{
  "traceId": 5,
  "timestampMs": 1500,
  "eventReceived": {
    "name": "START",
    "stableId": "Motor:event:START",
    "payload": { "target_speed": 1500 }
  },
  "transitionTaken": {
    "stableId": "Motor:transition:idle-running-START",
    "source": "Motor.Idle",
    "target": "Motor.Running"
  },
  "exitedStates": ["Motor.Idle"],
  "enteredStates": ["Motor.Running", "Motor.Running.Normal"],
  "actionsExecuted": ["Motor_action_startMotor"],
  "configBefore": { "activeStates": ["Motor.Idle"] },
  "configAfter":  { "activeStates": ["Motor.Running", "Motor.Running.Normal"] }
}
```

---

# 12. Error Codes

FSM-simulator-specific error codes (used in JSON-RPC error objects):

| Code | Name | Description |
|---|---|---|
| `FSM-SIM-E001` | InvalidIR | Machine IR failed schema validation or semantic check |
| `FSM-SIM-E002` | UnknownInstance | `instanceId` does not exist |
| `FSM-SIM-E003` | NotInitialized | `sim/dispatch` called before `sim/init` |
| `FSM-SIM-E004` | UnknownEvent | Event name not declared in the machine |
| `FSM-SIM-E005` | ContextTypeMismatch | Context field value has wrong JSON type |
| `FSM-SIM-E006` | ClockModeError | `sim/advanceClock` called in `wall` clock mode |
| `FSM-SIM-E007` | ReplayInProgress | `sim/dispatch` called during active replay |
| `FSM-SIM-E008` | MaxInstancesReached | Server instance limit (default: 16) exceeded |
| `FSM-SIM-E900` | CompletionLoopDetected | FSM-E0900 detected at runtime |

JSON-RPC error code mapping: all FSM-SIM-Exxx codes map to JSON-RPC error code `-32000`
(Server error). The FSM-SIM code is in the `message` field prefix.

---

# 13. Multi-Machine Support

Multiple machine instances can coexist. Each has a unique `instanceId`.

Cross-machine `send EVENT to MachineName` is handled by the simulator:
1. The simulator maintains a registry: `machineName → [instanceId, ...]`.
2. When a `send` statement is executed in machine A, the simulator routes the event to
   all instances of the target machine.
3. The routed event is delivered via `sim/dispatch` internally — no WebSocket round-trip.
4. A `sim/stateChanged` notification is emitted for the receiving machine instance.

---

# 14. Protocol Versioning

The `protocolVersion` in `sim/ready` uses semantic versioning:
- **MAJOR:** Incompatible change (client MUST disconnect on mismatch).
- **MINOR:** Backwards-compatible addition (new methods, new optional fields).
- **PATCH:** Bug fixes only.

Clients MUST check the major version number from `sim/ready` before sending any
other message.

---

*End of FSM-SPEC-SIM v1.0.0*
