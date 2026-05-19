// The CSP-locked debug Webview script (Doc 33 §W2; DBGUX §2 layout).
//
// THE KEYSTONE (Doc 33 §2): this script computes NO FSM semantics. It
// REUSES the v1.3 `renderModel` ELK/SVG renderer VERBATIM (imported from
// `../../diagram/webview/diagramWebview` — esbuild bundles this entrypoint
// SEPARATELY, so v1.3's own `dist/webview/diagramWebview.js` is byte-
// untouched; this debug bundle inlines its OWN copy of the SAME source —
// no fork, no reimplementation; the Doc 33 §W2 reuse ledger). It then:
//   - highlights the ACTIVE configuration the W1 `fsm/simulate` response
//     reported (`resp.configuration.activeStates` — read verbatim; the
//     panel does NOT compute the active config);
//   - draws breakpoint-glyph PLACEHOLDERS on nodes (W3 owns behaviour);
//   - renders the Context Δ-inspector + the StepRecord timeline DISPLAY
//     straight off `resp.context` / `resp.steps` (verbatim — zero recompute);
//   - posts user gestures (Init / Inject(+payload) / advance-clock) to the
//     extension, which marshals each to exactly one `fsm/simulate` op.
// "Which transition fired", "is this discarded", "what is the active
// config", "which timers fired" are ALWAYS the W1 oracle's answers carried
// in `resp` — this script renders them and decides NONE of them. The W4
// keystone audit negative-greps this surface for such logic and expects ∅.
//
// SCOPE (Doc 33 §W2, Item-3 DEFERRED): NO breakpoint predicate, NO
// time-travel rewind, NO queue/data-plane surface. Breakpoint glyphs are
// inert placeholders; Run/Pause/Step are disabled (W3). The timeline is a
// DISPLAY of `resp.steps`; rewind is W3.
//
// UX (DBGUX §2.2/§6): the payload form is revealed INLINE under the event
// picker the moment a payload-bearing event is parseable (proximity); the
// status line is a DURABLE indicator, not a toast; `var(--vscode-*)` theme
// tokens only; honest verdicts — a `discarded` step is shown as discarded,
// a StepError is shown VERBATIM, never a fabricated clean end-of-run.

// REUSE the v1.3 ELK/SVG renderer VERBATIM (the Doc 33 §W2 reuse ledger).
// We import ONLY `renderModel` — NOT `showBanner`: `showBanner` is
// hard-coded to the v1.3 `#banner` id, and the debug HTML deliberately
// does NOT use `#banner` (it uses `#debugBanner`) so that the v1.3
// standalone-bootstrap guard in `diagramWebview.ts`
// (`#svg && #banner` → suppress when imported) stays correct: the debug
// bundle's inlined v1.3 copy must NOT auto-bind a SECOND message listener.
// The banner is rendered locally below against `#debugBanner` (the SAME
// VERBATIM Doc 05 §1.5.9 text — `debugPanel.ts`'s `STALE_BANNER`).
import { renderModel, getVsCodeApi } from "../../diagram/webview/diagramWebview";

// ---- Wire shapes (the postMessage contract — kept in lock-step with
// `debugPanel.ts`'s `ExtToDebugWebview`; a deliberate ~field-shape mirror,
// NOT logic — the v1.3 webview's documented duplication discipline). The
// `SimResponse`/`StepRecord` shapes are the W1 `fsm/simulate` serde forms
// re-derived from `crates/fsm-simulator/src/trace.rs`. -----------------

interface SimValue {
  type: string;
  value: unknown;
}
interface TransitionTakenRecord {
  stableId: string;
  source: string;
  target: string;
}
interface EventReceivedRecord {
  name: string;
  stableId?: string;
  payload?: Record<string, SimValue>;
}
interface StepRecord {
  traceId: number;
  kind: string;
  virtualClockMs: number;
  eventReceived?: EventReceivedRecord;
  transitionTaken?: TransitionTakenRecord;
  exitedStates?: string[];
  enteredStates?: string[];
  actionsExecuted?: string[];
  configBefore: string[];
  configAfter: string[];
}
interface SimResponse {
  configuration?: { activeStates: string[] };
  context?: Record<string, SimValue>;
  currentMs?: number;
  steps?: StepRecord[];
  error?: string;
  errorKind?: string;
}
// Mirror the v1.3 DiagramModel shape (the renderModel input contract).
interface IrSourceLocation {
  file: string;
  line: number;
  column: number;
}
interface GraphNode {
  id: string;
  label: string;
  kind: string;
  parentRegion: string | null;
  loc: IrSourceLocation;
}
interface GraphEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
  label: string;
  loc: IrSourceLocation;
}
interface GraphRegion {
  id: string;
  name: string;
  parentState: string | null;
}
interface DiagramModel {
  machineName: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  regions: GraphRegion[];
}

type ExtToDebugWebview =
  | { type: "render"; model: DiagramModel }
  | { type: "staleBanner"; text: string }
  | { type: "transport"; enabled: boolean; reason: string; events: string[] }
  | { type: "simState"; resp: SimResponse; stamp: number }
  | { type: "simError"; message: string };

// Use the SAME window-cached singleton accessor the v1.3 module exports
// (NOT a second `acquireVsCodeApi()` — this debug bundle inlines its own
// copy of the v1.3 module, which acquires the API at its module top; a
// second acquire in ONE webview throws. The shared singleton makes the
// inlined v1.3 `renderModel`'s click→`revealSource` post and this script's
// posts share the ONE handle — the canonical multi-script-webview idiom).
const vscode = getVsCodeApi();

function byId(id: string): HTMLElement {
  const e = document.getElementById(id);
  if (!e) {
    throw new Error(`missing #${id}`);
  }
  return e;
}

const SVG_NS = "http://www.w3.org/2000/svg";

/** The last rendered model + the last applied context (for the Δ column).
 * `lastContext` is only ever the verbatim `resp.context` from a prior W1
 * response — never a computed value (the keystone). */
let lastModel: DiagramModel | undefined;
let lastContext: Record<string, SimValue> = {};

/** Render the diagram by REUSING the v1.3 `renderModel` VERBATIM, then
 * apply the additive overlay (bp-glyph placeholders). Active-config
 * highlight is applied separately by `applyActiveConfig` whenever a W1
 * response arrives. This function decides NO semantics — it lays out the
 * IR-projection exactly as the v1.3 read-only diagram does. */
async function renderDiagram(model: DiagramModel): Promise<void> {
  lastModel = model;
  // ── REUSE VERBATIM: the exact v1.3 ELK/SVG renderer (same #svg target,
  // same deterministic layout, same click→`revealSource` post). No second
  // diagram (DBGUX §4.1).
  const counts = await renderModel(model);

  // ── ADDITIVE overlay (DBGUX §2.2): an inert breakpoint-glyph placeholder
  // on every state node. Behaviour (arming/hit/predicate) is W3 — these are
  // visual placeholders ONLY, no click handler, no semantics.
  const svg = byId("svg") as unknown as SVGSVGElement;
  for (const g of Array.from(svg.querySelectorAll("g.node, g.pseudo"))) {
    const glyph = document.createElementNS(SVG_NS, "text");
    glyph.setAttribute("class", "bp-glyph");
    glyph.setAttribute("x", "-10");
    glyph.setAttribute("y", "8");
    glyph.textContent = "◌"; // armed=◍ / hit=▣ are W3; placeholder=◌
    g.appendChild(glyph);
  }

  byId("status").textContent = "Diagram ready — press Init to instantiate the machine.";
  vscode.postMessage({
    type: "renderedDiagram",
    nodes: counts.nodes,
    edges: counts.edges,
  });
}

/** Highlight the ACTIVE configuration the W1 response reported. The active
 * set is `resp.configuration.activeStates` read VERBATIM — this function
 * does NOT compute which states are active; it paints the oracle's answer
 * (the keystone). State ids in the IR model match the config state ids. */
function applyActiveConfig(active: string[]): void {
  if (!lastModel) {
    return;
  }
  // `data-node-id` was set by `tagNodeIds` (positional pairing against the
  // v1.3 renderer's `model.nodes`-order groups). Toggle the `active` class
  // by membership in the W1 oracle's reported set — read VERBATIM, never
  // computed (the keystone).
  const activeSet = new Set(active);
  const svg = byId("svg") as unknown as SVGSVGElement;
  const groups = Array.from(svg.querySelectorAll("g.node, g.pseudo"));
  for (const g of groups) {
    const id = g.getAttribute("data-node-id");
    if (id && activeSet.has(id)) {
      g.classList.add("active");
    } else {
      g.classList.remove("active");
    }
  }
}

/** Tag each rendered node group with its IR id so `applyActiveConfig` can
 * match the W1 oracle's `activeStates` ids to the rendered nodes.
 *
 * The v1.3 `renderModel` (REUSED verbatim) draws exactly one
 * `g.node`/`g.pseudo` per `model.nodes` entry that received an ELK
 * position, iterating `model.nodes` IN ORDER (it loops `for (const n of
 * model.nodes)` and `continue`s only when `pos.get(n.id)` is absent — which
 * does not happen for a connected statechart). So the rendered group order
 * is `model.nodes` order. We pair them positionally and also cross-check
 * the v1.3-set `data-line`/`data-column` matches the model node's loc — if
 * the cross-check ever fails the pairing is abandoned for that group (it
 * stays untagged → simply not highlighted, never MIS-highlighted; an honest
 * degrade, never a fabricated active state). Pure presentation glue; this
 * decides NO semantics. */
function tagNodeIds(model: DiagramModel): void {
  const svg = byId("svg") as unknown as SVGSVGElement;
  const groups = Array.from(svg.querySelectorAll("g.node, g.pseudo"));
  if (groups.length !== model.nodes.length) {
    // Defensive: if the renderer skipped a node, fall back to a
    // loc-keyed index (still honest — an unmatched node is just not
    // highlighted, never wrongly highlighted).
    const index = new Map<string, string>();
    for (const n of model.nodes) {
      index.set(`${n.loc.line}:${n.loc.column}`, n.id);
    }
    for (const g of groups) {
      const id = index.get(`${g.getAttribute("data-line")}:${g.getAttribute("data-column")}`);
      if (id) {
        g.setAttribute("data-node-id", id);
      }
    }
    return;
  }
  for (let i = 0; i < groups.length; i++) {
    const g = groups[i];
    const n = model.nodes[i];
    if (
      g.getAttribute("data-line") === String(n.loc.line) &&
      g.getAttribute("data-column") === String(n.loc.column)
    ) {
      g.setAttribute("data-node-id", n.id);
    }
  }
}

/** Render the Context Δ-inspector straight off the W1 response's
 * `resp.context` (verbatim). The Δ column compares to the PRIOR W1
 * `resp.context` — a pure display diff of two oracle-produced maps, NOT a
 * computed transition effect (the keystone: the panel shows what changed,
 * it does not decide what changed). */
function renderContext(ctx: Record<string, SimValue>): void {
  const body = byId("ctxBody");
  const keys = Object.keys(ctx).sort();
  if (keys.length === 0) {
    body.className = "empty";
    body.textContent = "(no context fields)";
    lastContext = {};
    return;
  }
  body.className = "";
  const rows = keys
    .map((k) => {
      const v = ctx[k];
      const prev = lastContext[k];
      const changed = prev !== undefined && JSON.stringify(prev) !== JSON.stringify(v);
      const delta =
        changed && prev
          ? `<span class="delta">▲ was ${escapeHtml(String(prev.value))}</span>`
          : "";
      return (
        `<tr><td>${escapeHtml(k)}</td>` +
        `<td>${escapeHtml(v.type)}</td>` +
        `<td>${escapeHtml(String(v.value))}</td>` +
        `<td>${delta}</td></tr>`
      );
    })
    .join("");
  body.innerHTML =
    `<table><thead><tr><th>field</th><th>type</th><th>value</th>` +
    `<th>Δ</th></tr></thead><tbody>${rows}</tbody></table>`;
  lastContext = { ...ctx };
}

/** Append the W1 response's `resp.steps` to the timeline DISPLAY
 * (verbatim — each row IS a `StepRecord` the oracle produced; the panel
 * does not synthesize steps). Honest verdicts: a `discarded` step is
 * shown AS discarded (DBGUX §6), never silently dropped. Rewind is W3. */
const timelineRows: string[] = [];
let stepCounter = 0;
function appendSteps(steps: StepRecord[]): void {
  const tl = byId("timeline");
  if (timelineRows.length === 0) {
    tl.innerHTML =
      `<table><thead><tr><th>#</th><th>t(ms)</th><th>kind</th>` +
      `<th>detail</th></tr></thead><tbody id="tlBody"></tbody></table>`;
  }
  for (const s of steps) {
    const n = stepCounter++;
    let detail = "";
    if (s.kind === "discarded") {
      detail = `${s.eventReceived?.name ?? ""} — event had no enabled transition`;
    } else if (s.transitionTaken) {
      detail = `${s.transitionTaken.source}→${s.transitionTaken.target}`;
    } else if (s.eventReceived) {
      detail = s.eventReceived.name;
    } else if ((s.enteredStates ?? []).length > 0) {
      detail = `→${(s.enteredStates ?? []).join(",")}`;
    } else if (s.kind === "init") {
      detail = `→${(s.configAfter ?? []).join(",")}`;
    }
    timelineRows.push(
      `<tr><td>${n}</td><td>${s.virtualClockMs}</td>` +
        `<td>${escapeHtml(s.kind)}</td>` +
        `<td>${escapeHtml(detail)}</td></tr>`,
    );
  }
  const tbody = document.getElementById("tlBody");
  if (tbody) {
    tbody.innerHTML = timelineRows.join("");
  }
}

/** Recompute the pending-timer hint from the W1 response. NOTE: the W1
 * `fsm/simulate` schema (re-derived from the merged `state_block` in
 * `simulate.rs`) returns { configuration, context, currentMs, steps } —
 * it does NOT carry a pending-timer list. So the panel HONESTLY shows the
 * clock + a placeholder rather than FABRICATING timer countdowns (the
 * cardinal-sin bar: never invent data the oracle did not return; a real
 * pending-timer surface would require a W1 schema addition — out of W2
 * scope, flagged in the report as a DBGUX-vs-merged-schema drift). */
function renderTimers(): void {
  const t = byId("timers");
  t.className = "empty";
  t.textContent = "pending timers: advance the clock to fire due timers";
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Show the VERBATIM Doc 05 §1.5.9 stale banner (the v1.3 contract,
 * extended to the debug surface) against the debug-unique `#debugBanner`
 * id — NOT `#banner` (see the `renderModel`-only import note above: a
 * `#banner` here would re-trigger the inlined v1.3 standalone bootstrap).
 * Mirrors the v1.3 `showBanner` behaviour exactly. */
function showBanner(text: string): void {
  const b = byId("debugBanner");
  b.textContent = text;
  b.classList.add("show");
}

/** Clear the stale banner on a fresh valid render (the v1.3 §1.5.9
 * "the banner is the last-valid indicator" semantics, reused). */
function clearBanner(): void {
  byId("debugBanner").classList.remove("show");
}

// ---- Transport / inject wiring (user gestures → the extension, which
// marshals each to ONE `fsm/simulate` op; this script computes nothing) --

function setStatus(text: string, isError = false): void {
  const s = byId("status");
  s.textContent = text;
  s.className = isError ? "err" : "";
}

let transportEnabled = false;
let initialized = false;

function refreshButtons(): void {
  (byId("btnInit") as HTMLButtonElement).disabled = !transportEnabled;
  (byId("btnDispatch") as HTMLButtonElement).disabled = !transportEnabled || !initialized;
  (byId("btnAdvance") as HTMLButtonElement).disabled = !transportEnabled || !initialized;
  // Run/Pause/Step are W3 (single-step reveal) — stay disabled in W2.
}

/** Build the inline payload form when a payload-bearing event is chosen.
 * The schema for payloads is not in the IR event list the picker reads, so
 * W2 offers a single free-form JSON payload field revealed INLINE under the
 * picker (DBGUX §2.2 proximity / reveal-adjacent-to-trigger). The W1 oracle
 * validates the payload shape (`dispatch`'s `payload` serde) and surfaces
 * any shape error VERBATIM — the panel does not pre-validate (no semantics). */
function buildPayloadForm(): void {
  const pf = byId("payloadForm");
  pf.innerHTML =
    `<label for="pl">payload (optional JSON — sent to the oracle ` +
    `verbatim)</label>` +
    `<input id="pl" type="text" placeholder='{ "target_speed": ` +
    `{ "type": "u16", "value": 1500 } }' />`;
  pf.classList.add("show");
}

function wireControls(): void {
  byId("btnInit").addEventListener("click", () => {
    setStatus("Init…");
    vscode.postMessage({ type: "init" });
  });
  byId("btnDispatch").addEventListener("click", () => {
    const evt = (byId("evtPick") as HTMLSelectElement).value;
    if (!evt) {
      return;
    }
    const plEl = document.getElementById("pl") as HTMLInputElement | null;
    let payload: Record<string, unknown> | undefined;
    const raw = plEl?.value.trim();
    if (raw) {
      try {
        payload = JSON.parse(raw) as Record<string, unknown>;
      } catch (e) {
        // Honest: a malformed payload is reported in place, NOT sent as a
        // fake (the cardinal-sin bar; the oracle would also reject it).
        setStatus(`payload is not valid JSON: ${String(e)}`, true);
        return;
      }
    }
    setStatus(`dispatch ${evt}…`);
    vscode.postMessage({ type: "dispatch", event: evt, payload });
  });
  byId("btnAdvance").addEventListener("click", () => {
    const ms = Number((byId("clockDelta") as HTMLInputElement).value);
    if (!Number.isFinite(ms) || ms < 0) {
      setStatus("advance-clock delta must be a non-negative number", true);
      return;
    }
    setStatus(`advance clock +${ms} ms…`);
    vscode.postMessage({ type: "advanceClock", deltaMs: ms });
  });
  // The payload form is revealed inline the moment an event is picked
  // (proximity / reveal-adjacent-to-trigger — DBGUX §2.2).
  byId("evtPick").addEventListener("change", () => {
    if ((byId("evtPick") as HTMLSelectElement).value) {
      buildPayloadForm();
    } else {
      byId("payloadForm").classList.remove("show");
    }
  });
}

// ---- The extension → webview message loop. The webview RENDERS the
// VERBATIM W1 response; it recomputes nothing (the keystone — the §W2
// gate asserts the rendered config/context/timeline byte-equals `resp`). -

window.addEventListener("message", (ev: MessageEvent<ExtToDebugWebview>) => {
  const msg = ev.data;
  if (msg.type === "render") {
    clearBanner();
    renderDiagram(msg.model)
      .then(() => {
        tagNodeIds(msg.model);
      })
      .catch((e: unknown) => {
        setStatus(`Diagram layout error: ${String(e)}`, true);
        vscode.postMessage({
          type: "renderedDiagram",
          nodes: 0,
          edges: 0,
          error: String(e),
        });
      });
  } else if (msg.type === "staleBanner") {
    // The codegen-gated boundary: keep the last valid render, show the
    // VERBATIM Doc 05 §1.5.9 banner, and the transport message that
    // follows disables the rail with the reason inline (DBGUX §2.3 —
    // never blank, never fake a session).
    showBanner(msg.text);
    vscode.postMessage({
      type: "staleShown",
      hasLastValidRender: lastModel !== undefined,
    });
  } else if (msg.type === "transport") {
    transportEnabled = msg.enabled;
    if (!msg.enabled) {
      initialized = false;
    }
    refreshButtons();
    // Event picker (a PURE structural read of the IR — the user picks;
    // the W1 oracle decides the effect).
    const pick = byId("evtPick") as HTMLSelectElement;
    pick.innerHTML =
      `<option value="">— pick an event —</option>` +
      msg.events.map((e) => `<option value="${escapeHtml(e)}">${escapeHtml(e)}</option>`).join("");
    if (msg.reason) {
      setStatus(msg.reason, !msg.enabled);
    }
    // Test-observability ack (NOT acceptance on its own): lets the ExtHost
    // E2E await the deterministic point where the W1 `load` completed and
    // the transport is enabled, before firing Init (avoids racing the
    // panel's refresh()→load round-trip). The v1.3 ack-pattern.
    vscode.postMessage({ type: "transportApplied", enabled: msg.enabled });
  } else if (msg.type === "simState") {
    // ── THE KEYSTONE RENDER PATH: paint the W1 response VERBATIM. The
    // active config, the context, the StepRecord timeline, the clock are
    // ALL read straight off `resp` — this script computes NONE of them.
    initialized = true;
    refreshButtons();
    const resp = msg.resp;
    const active = resp.configuration?.activeStates ?? [];
    applyActiveConfig(active);
    renderContext(resp.context ?? {});
    appendSteps(resp.steps ?? []);
    renderTimers();
    byId("clock").textContent = `⏱ virtual clock: ${resp.currentMs ?? "—"} ms`;
    setStatus(`active: ${active.join(", ") || "(none)"}`);
    vscode.postMessage({ type: "stateApplied", stamp: msg.stamp });
  } else if (msg.type === "simError") {
    // Honest: the StepError / invalid reason VERBATIM, a DURABLE status
    // indicator (not a toast) — never a fabricated clean end-of-run
    // (DBGUX §6 "inconclusive ≠ done"; the cardinal-sin bar).
    setStatus(msg.message, true);
  }
});

wireControls();
refreshButtons();

// Signal readiness so the extension's first render is not raced (the v1.3
// race-free first-render contract, reused).
vscode.postMessage({ type: "ready" });
