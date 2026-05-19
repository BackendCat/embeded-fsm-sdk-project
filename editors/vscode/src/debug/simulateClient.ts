// The `fsm/simulate` LSP-request marshalling boundary for the debug-W2
// WebviewPanel (Doc 33 §W1/§W2; the v1.5 W-A2 `fsm/verify`
// `client.sendRequest` seam this MIRRORS exactly — GT-11).
//
// THE KEYSTONE (Doc 33 §2 — re-derived from the MERGED W1 source
// `crates/fsm-lsp/src/capabilities/simulate.rs`, NOT from design-doc prose,
// the GT-7 verify-the-record discipline): this module is a PURE marshalling
// frontend. It sends one `fsm/simulate` LSP custom request per debug verb
// (an `op`-discriminated params object) and returns the server's response
// VERBATIM. It computes NO FSM semantics — not which transition fires, not
// the active configuration, not which timers fired, not whether an event
// was discarded. Every one of those is the W1 `Interpreter`'s answer, read
// straight off the response. A second simulator semantics — even a partial
// "fast in-editor" one — is the exact P0-1 / v1.4-keystone /
// v1.5-KEYSTONE-IN-UI regression the entire debug epic guards against.
//
// THE WIRE SCHEMA, re-derived from the merged `run_simulate` (the SoT):
//   request : { op, instanceId, ...op-specific }
//     load          { op:"load", instanceId, text, uri, machineName? }
//                     → { instanceId, machineName } | { error }
//     init          { op:"init", instanceId, context?, virtualClockStartMs?,
//                     machineName? }
//     dispatch      { op:"dispatch", instanceId, event:{ name, payload? } }
//     advanceClock  { op:"advanceClock", instanceId, deltaMs }
//     getContext    { op:"getContext", instanceId }
//     setContext    { op:"setContext", instanceId, fields }      (W3-era UI)
//     snapshot      { op:"snapshot", instanceId }                (W3)
//     restore       { op:"restore", instanceId, snapshotIndex }  (W3)
//     listInstances { op:"listInstances" } · unload { op:"unload", … }
//   response (init/dispatch/advanceClock/getContext/setContext):
//     { configuration:{ activeStates:string[] }, context:{...},
//       currentMs:number, steps:StepRecord[] }                   (state_block)
//   honest error (StepError or invalid input — surfaced VERBATIM, never a
//     fabricated clean end-of-run; the "inconclusive ≠ done" bar, DBGUX §6):
//     { error:string, errorKind?:string }   (errorKind only for StepError)
//
// `StepRecord` is the `fsm-simulator::trace::StepRecord` camelCase serde
// form (Doc 13 §11; re-derived from `crates/fsm-simulator/src/trace.rs`) —
// byte-equal to what `fsm test`'s `execute_trace` emits for the same
// `TraceCommand`s (the W1 module establishes this by construction; the W2
// gate proves the PANEL renders exactly this, adding zero semantics).

import { LanguageClient, State } from "vscode-languageclient/node";

/** A context value in the trace `Value` internally-tagged serde form
 * (`crates/fsm-simulator/src/runtime/value.rs`:
 * `#[serde(tag="type", content="value", rename_all="snake_case")]`). The
 * panel treats these OPAQUELY — it renders `type`/`value` as-is and never
 * coerces or computes on them (the keystone: zero semantics). */
export interface SimValue {
  readonly type: string;
  readonly value: unknown;
}

/** The transition the W1 oracle reported as taken (`TransitionTakenRecord`,
 * camelCase serde). The panel only DISPLAYS this — it never decides it. */
export interface TransitionTakenRecord {
  readonly stableId: string;
  readonly source: string;
  readonly target: string;
}

/** An event the W1 oracle recorded as received (`EventReceivedRecord`). */
export interface EventReceivedRecord {
  readonly name: string;
  readonly stableId?: string;
  readonly payload?: Record<string, SimValue>;
}

/**
 * One `StepRecord` exactly as the W1 `fsm/simulate` response carries it
 * (the `crates/fsm-simulator/src/trace.rs::StepRecord` camelCase serde
 * form). Re-derived from source, not transcribed: `serde` omits the
 * `skip_serializing_if` optional/empty fields, so they are optional here.
 * The panel RENDERS these fields; it computes NONE of them (the keystone).
 */
export interface StepRecord {
  readonly traceId: number;
  /** `StepKind` snake_case: init | dispatched | raised | timer_fired |
   * completion | discarded | event_deferred | event_redispatched |
   * submachine_entered | submachine_event_delegated | submachine_completed. */
  readonly kind: string;
  readonly virtualClockMs: number;
  readonly eventReceived?: EventReceivedRecord;
  readonly transitionTaken?: TransitionTakenRecord;
  readonly exitedStates?: string[];
  readonly enteredStates?: string[];
  readonly actionsExecuted?: string[];
  readonly configBefore: string[];
  readonly configAfter: string[];
  readonly submachine?: unknown;
}

/** The shared post-call state block every stepping/read op returns
 * (`state_block` in the merged `simulate.rs`). Every field is a READ off
 * the W1 `Interpreter` — the panel renders it, computes nothing. */
export interface SimStateBlock {
  readonly configuration: { readonly activeStates: string[] };
  readonly context: Record<string, SimValue>;
  readonly currentMs: number;
  readonly steps: StepRecord[];
}

/** A `load` op response. */
export interface SimLoadResult {
  readonly instanceId?: string;
  readonly machineName?: string;
  /** The honest "cannot simulate a model that does not compile" reason
   * (never a fabricated session over broken input — the cardinal-sin bar,
   * surfaced VERBATIM from W1's `ir_for`). */
  readonly error?: string;
}

/** Any `fsm/simulate` response is EITHER an honest error OR an op body.
 * `error` present ⇒ surface it verbatim; never fake a clean result. */
export type SimResponse = (Partial<SimStateBlock> & SimLoadResult) & {
  readonly error?: string;
  readonly errorKind?: string;
  readonly snapshotIndex?: number;
};

/** Why a debug verb could not reach the oracle — surfaced HONESTLY in the
 * panel status line / stale-banner, NEVER a fabricated session (the
 * v1.3/v1.5 cardinal-sin bar at the request boundary). */
export class SimTransportError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SimTransportError";
  }
}

/**
 * Marshal one `fsm/simulate` op over the running language client and return
 * the server's response VERBATIM (no reshaping — the panel renders exactly
 * what W1 returned; that byte-fidelity IS the keystone the §W2 gate proves).
 *
 * @param client the running `LanguageClient` (the v1.5 W-A2 boundary). A
 *   non-running client is an HONEST transport error, never a faked session.
 * @param params the op params object (`op` + the op's fields — the merged
 *   `run_simulate` schema, re-derived from source above).
 */
export async function simulate(
  client: LanguageClient | undefined,
  params: Record<string, unknown>,
): Promise<SimResponse> {
  if (!client || client.state !== State.Running) {
    throw new SimTransportError(
      "the language server is not running — interactive simulation is " +
        "unavailable. Restart it (`FSM Studio: Restart Language Server`).",
    );
  }
  try {
    // The request is sent VERBATIM and the response returned VERBATIM —
    // this function decides nothing about FSM behaviour (the keystone).
    return await client.sendRequest<SimResponse>("fsm/simulate", params);
  } catch (e) {
    // A JSON-RPC error (e.g. invalid-params) / transport failure — surfaced
    // verbatim, NEVER collapsed into a fake clean step (cardinal-sin bar).
    const msg = e instanceof Error ? e.message : String(e);
    throw new SimTransportError(`fsm/simulate request failed — ${msg}`);
  }
}
