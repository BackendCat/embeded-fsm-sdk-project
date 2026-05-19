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

/** W3 breakpoint kinds — a PREDICATE OVER ONE StepRecord field, never a
 * guard re-evaluation (DBGUX §3.2; kept in lock-step with
 * `debugPanel.ts`'s `BpKind`). */
type BpKind = "enter" | "exit" | "transition";
interface Breakpoint {
  kind: BpKind;
  targetId: string;
}

type ExtToDebugWebview =
  | { type: "render"; model: DiagramModel }
  | { type: "staleBanner"; text: string }
  | { type: "transport"; enabled: boolean; reason: string; events: string[] }
  // W3: the edge-id → transition-stableId map (the predicate target for a
  // transition breakpoint is the IR `stableId`, NOT the diagram edge id).
  | { type: "transitionIds"; map: Record<string, string> }
  | {
      type: "simState";
      resp: SimResponse;
      stamp: number;
      revealCount?: number;
      newCall?: boolean;
    }
  // W3 PAUSED-at-breakpoint (a DURABLE status indicator, NOT a toast).
  | {
      type: "paused";
      resp: SimResponse;
      steps?: StepRecord[];
      bpKind: BpKind;
      targetId: string;
      reasonText: string;
      revealCount: number;
      stamp: number;
    }
  // W3 rewind: jump diagram+context+clock+timeline-cursor to row #index.
  | { type: "rewound"; resp: SimResponse; index: number; stamp: number }
  // W3: the armed-breakpoint set (paint glyph states ◌/◍/▣).
  | { type: "breakpoints"; armed: Breakpoint[] }
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

// ── W3 client-side breakpoint + time-travel UI state (NO FSM semantics —
// the predicate runs in the EXTENSION over the W1 StepRecord fields; this
// is pure presentation + gesture forwarding) ──────────────────────────────

/** edge `GraphEdge.id` → IR transition `stableId` (from the extension's
 * pure IR read). The transition-breakpoint predicate target is the
 * `stableId` (matched against `StepRecord.transitionTaken.stableId`), NOT
 * the diagram edge id — a DIFFERENT IR field. */
let edgeStableId: Record<string, string> = {};

/** The armed-breakpoint set the extension owns (mirrored here ONLY to
 * paint glyph states ◌/◍/▣). */
let armedBps: Breakpoint[] = [];

/** The breakpoint currently HIT (paused) — its glyph shows ▣. Cleared on
 * resume/rewind/re-init. Pure presentation. */
let hitBp: Breakpoint | undefined;

/** `true` while PAUSED at a breakpoint (drives Run/Pause/Step enablement
 * + the durable status line). */
let pausedAtBp = false;

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

  const svg = byId("svg") as unknown as SVGSVGElement;

  // ── ADDITIVE overlay (DBGUX §2.2 — the breakpoint glyph lives ON the
  // node/edge, the proximity principle): W3 makes the glyph FUNCTIONAL.
  // Clicking a STATE node toggles a state-ENTER breakpoint; clicking a
  // TRANSITION edge toggles a transition breakpoint. The glyph carries
  // NO semantics — it only tells the EXTENSION which StepRecord-field
  // value to compare (the predicate runs there, over the W1 oracle's own
  // steps; the W4 audit expects ∅ semantics on this surface).
  //
  // Node glyph → state-ENTER breakpoint (the common "stop when we reach
  // S"). State-EXIT is also a supported predicate kind in the extension;
  // surfacing a second per-node affordance is a deliberate W3-MVP scope
  // line (disclosed) — the primary glyph is enter, matching the §3.4 /
  // debugger-correct mental model.
  for (const g of Array.from(svg.querySelectorAll("g.node, g.pseudo"))) {
    const glyph = document.createElementNS(SVG_NS, "text");
    glyph.setAttribute("class", "bp-glyph bp-armable");
    glyph.setAttribute("x", "-10");
    glyph.setAttribute("y", "8");
    glyph.textContent = GLYPH_NONE;
    glyph.addEventListener("click", (ev) => {
      ev.stopPropagation(); // do NOT also trigger the v1.3 click→source
      const stateId = g.getAttribute("data-node-id");
      if (stateId) {
        // The predicate target for enter/exit IS the raw IR state id
        // (matched against StepRecord.enteredStates — re-derived from
        // source: that vector carries raw IR state ids). No semantics.
        vscode.postMessage({
          type: "toggleBreakpoint",
          bpKind: "enter",
          targetId: stateId,
        });
      }
    });
    g.appendChild(glyph);
  }

  // EDGE glyphs (W3-new — W2 had none): a clickable breakpoint marker per
  // transition edge. The v1.3 `renderModel` draws edges as `g.edge` in
  // `model.edges` order with NO data attrs; we pair them positionally to
  // `model.edges` (the renderer iterates `model.edges` in order — the
  // documented stable contract) and set `data-edge-id` so the click can
  // resolve the IR transition `stableId` (the predicate target).
  const edgeGroups = Array.from(svg.querySelectorAll("g.edge"));
  for (let i = 0; i < edgeGroups.length && i < model.edges.length; i++) {
    const g = edgeGroups[i];
    const e = model.edges[i];
    g.setAttribute("data-edge-id", e.id);
    // Place the glyph near the edge's mid-label (the path midpoint the
    // v1.3 renderer used for the event label — proximity to the edge).
    const pathEl = g.querySelector("path");
    let gx = 0;
    let gy = 0;
    if (pathEl) {
      try {
        const len = (pathEl as SVGPathElement).getTotalLength();
        const mid = (pathEl as SVGPathElement).getPointAtLength(len / 2);
        gx = mid.x - 12;
        gy = mid.y - 6;
      } catch {
        // jsdom/headless may not implement getPointAtLength — fall back
        // to the path's bbox centre (still ON the edge, never floating).
        const bb = (g as SVGGraphicsElement).getBBox?.();
        if (bb) {
          gx = bb.x + bb.width / 2 - 12;
          gy = bb.y + bb.height / 2 - 6;
        }
      }
    }
    const glyph = document.createElementNS(SVG_NS, "text");
    glyph.setAttribute("class", "bp-glyph bp-armable bp-edge");
    glyph.setAttribute("x", String(gx));
    glyph.setAttribute("y", String(gy));
    glyph.textContent = GLYPH_NONE;
    glyph.addEventListener("click", (ev) => {
      ev.stopPropagation();
      const edgeId = g.getAttribute("data-edge-id");
      // The transition-breakpoint predicate matches the IR `stableId`
      // (StepRecord.transitionTaken.stableId), NOT the diagram edge id.
      const stableId = edgeId ? edgeStableId[edgeId] : undefined;
      if (stableId) {
        vscode.postMessage({
          type: "toggleBreakpoint",
          bpKind: "transition",
          targetId: stableId,
        });
      } else {
        setStatus(
          "transition breakpoint unavailable: the IR did not expose a " +
            "stable id for this edge (cannot arm without it).",
          true,
        );
      }
    });
    g.appendChild(glyph);
  }
  paintGlyphs();

  byId("status").textContent = "Diagram ready — press Init to instantiate the machine.";
  vscode.postMessage({
    type: "renderedDiagram",
    nodes: counts.nodes,
    edges: counts.edges,
  });
}

// Glyph states (DBGUX §2.2): none / armed / hit.
const GLYPH_NONE = "◌";
const GLYPH_ARMED = "◍";
const GLYPH_HIT = "▣";

/** Repaint every breakpoint glyph from the armed set + the hit bp. PURE
 * presentation — the predicate lives in the extension over the W1
 * oracle's StepRecord fields; this only colours the marker. */
function paintGlyphs(): void {
  const svg = byId("svg") as unknown as SVGSVGElement;
  // Nodes: a state-enter (or exit) breakpoint targets the node's IR id.
  for (const g of Array.from(svg.querySelectorAll("g.node, g.pseudo"))) {
    const id = g.getAttribute("data-node-id");
    const glyph = g.querySelector("text.bp-glyph");
    if (!glyph) {
      continue;
    }
    const armed = armedBps.some(
      (b) => (b.kind === "enter" || b.kind === "exit") && b.targetId === id,
    );
    const hit =
      !!hitBp &&
      (hitBp.kind === "enter" || hitBp.kind === "exit") &&
      hitBp.targetId === id;
    glyph.textContent = hit ? GLYPH_HIT : armed ? GLYPH_ARMED : GLYPH_NONE;
    (glyph as SVGElement).classList.toggle("armed", armed && !hit);
    (glyph as SVGElement).classList.toggle("hit", hit);
  }
  // Edges: a transition breakpoint targets the IR `stableId`.
  for (const g of Array.from(svg.querySelectorAll("g.edge"))) {
    const edgeId = g.getAttribute("data-edge-id");
    const stableId = edgeId ? edgeStableId[edgeId] : undefined;
    const glyph = g.querySelector("text.bp-glyph");
    if (!glyph) {
      continue;
    }
    const armed = !!stableId && armedBps.some((b) => b.kind === "transition" && b.targetId === stableId);
    const hit = !!hitBp && hitBp.kind === "transition" && !!stableId && hitBp.targetId === stableId;
    glyph.textContent = hit ? GLYPH_HIT : armed ? GLYPH_ARMED : GLYPH_NONE;
    (glyph as SVGElement).classList.toggle("armed", armed && !hit);
    (glyph as SVGElement).classList.toggle("hit", hit);
  }
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

/** The timeline is the TIME-TRAVEL surface (DBGUX §2.2). Each row is a
 * `StepRecord` the W1 oracle produced (verbatim — the panel synthesizes
 * none). `revealedRows` is the UI reveal cursor (§3.1 — a cursor over the
 * oracle's already-computed vector; NOT a re-implemented RTC step): rows
 * past it are buffered-but-not-yet-revealed. Each revealed row carries a
 * `◀ rewind to #N` action ON THE ROW (proximity — not a detached toolbar
 * button); the caret `►` marks the current position. Honest verdicts: a
 * `discarded` step is shown AS discarded (DBGUX §6), never dropped. */
interface TimelineEntry {
  readonly t: number;
  readonly kind: string;
  readonly detail: string;
  /** `false` while this row is buffered-but-not-yet-revealed (paused
   * mid-vector — §3.1). Shown dimmed/honest, never hidden. */
  revealed: boolean;
}
/** Entries from FULLY-COMPLETED stepping-calls (immutable once the call
 * finishes; truncated only by a rewind). The single source of truth for
 * the timeline — the webview re-derives the table from this + the active
 * call below. */
const committed: TimelineEntry[] = [];
/** The active stepping-call's StepRecords + how many are revealed (the
 * §3.1 cursor). Folded into `committed` when the call completes. */
let activeSteps: StepRecord[] = [];
let activeRevealed = 0;
/** The caret `►` row (the current position; -1 = none). */
let caretRow = -1;

function stepDetail(s: StepRecord): string {
  if (s.kind === "discarded") {
    return `${s.eventReceived?.name ?? ""} — event had no enabled transition`;
  }
  if (s.transitionTaken) {
    return `${s.transitionTaken.source}→${s.transitionTaken.target}`;
  }
  if (s.eventReceived) {
    return s.eventReceived.name;
  }
  if ((s.enteredStates ?? []).length > 0) {
    return `→${(s.enteredStates ?? []).join(",")}`;
  }
  if (s.kind === "init") {
    return `→${(s.configAfter ?? []).join(",")}`;
  }
  return "";
}

function entryFor(s: StepRecord, revealed: boolean): TimelineEntry {
  return { t: s.virtualClockMs, kind: s.kind, detail: stepDetail(s), revealed };
}

/** Begin/replace the ACTIVE stepping-call's steps + reveal cursor. The
 * entries are the oracle's StepRecords verbatim (zero synthesis). */
function setActiveCall(steps: StepRecord[], revealed: number): void {
  activeSteps = steps;
  activeRevealed = Math.max(0, Math.min(revealed, steps.length));
}

/** Fold the active call into `committed` (call complete) and clear it. */
function commitActiveCall(): void {
  for (let i = 0; i < activeSteps.length; i++) {
    committed.push(entryFor(activeSteps[i], true));
  }
  activeSteps = [];
  activeRevealed = 0;
}

/** The full row list = committed + the active call's steps (revealed
 * ones live, the rest shown honestly as "⋯ buffered"). */
function allRows(): TimelineEntry[] {
  const rows = committed.slice();
  for (let i = 0; i < activeSteps.length; i++) {
    rows.push(entryFor(activeSteps[i], i < activeRevealed));
  }
  return rows;
}

function renderTimeline(): void {
  const tl = byId("timeline");
  const rows = allRows();
  if (rows.length === 0) {
    tl.innerHTML = `<div class="empty">No steps yet.</div>`;
    return;
  }
  const body = rows
    .map((e, idx) => {
      const caret = idx === caretRow ? "►" : "";
      if (!e.revealed) {
        // Buffered-but-not-revealed (paused mid-vector): shown dimmed,
        // honestly, as "pending reveal" — never hidden, never faked.
        return (
          `<tr class="tl-pending"><td>${caret}${idx}</td>` +
          `<td>${e.t}</td><td>${escapeHtml(e.kind)}</td>` +
          `<td>${escapeHtml(e.detail)} <span class="tl-pendmark">⋯ buffered</span></td></tr>`
        );
      }
      return (
        `<tr${idx === caretRow ? ' class="tl-cur"' : ""}>` +
        `<td>${caret}${idx}</td><td>${e.t}</td>` +
        `<td>${escapeHtml(e.kind)}</td>` +
        `<td>${escapeHtml(e.detail)} ` +
        `<button class="tl-rewind" data-row="${idx}" ` +
        `title="time-travel: restore the machine to this point (W1 snapshot/restore)">` +
        `◀ rewind to #${idx}</button></td></tr>`
      );
    })
    .join("");
  tl.innerHTML =
    `<table><thead><tr><th>#</th><th>t(ms)</th><th>kind</th>` +
    `<th>detail</th></tr></thead><tbody>${body}</tbody></table>`;
  // Wire the per-row rewind action (proximity — the action is ON the row
  // it targets, not a detached global button).
  for (const b of Array.from(tl.querySelectorAll("button.tl-rewind"))) {
    b.addEventListener("click", () => {
      const row = Number(b.getAttribute("data-row"));
      if (Number.isFinite(row)) {
        setStatus(`rewind to #${row}…`);
        vscode.postMessage({ type: "rewind", index: row });
      }
    });
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
  // While paused at a breakpoint, inject/advance are GATED (the author
  // must Step/Resume the buffered vector first — an honest precondition,
  // matching the extension's guard; never silently drop the buffer).
  const liveOk = transportEnabled && initialized && !pausedAtBp;
  (byId("btnDispatch") as HTMLButtonElement).disabled = !liveOk;
  (byId("btnAdvance") as HTMLButtonElement).disabled = !liveOk;
  // W3: Run/Step are enabled ONLY while paused (they resume/advance the
  // buffered reveal — §3.1); Pause is informational (the run is already
  // paused — there is no free-running mode in the MVP UI-paced model, so
  // Pause stays disabled, honestly labelled, never a dead fake button).
  (byId("btnRun") as HTMLButtonElement).disabled = !pausedAtBp;
  (byId("btnStep") as HTMLButtonElement).disabled = !pausedAtBp;
  (byId("btnPause") as HTMLButtonElement).disabled = true;
}

/** Highlight the active configuration. Prefers the LAST REVEALED
 * `StepRecord.configAfter` (raw IR state ids — these match the diagram's
 * `data-node-id`, set from `GraphNode.id`) so a partial reveal shows the
 * state AT THE CURSOR; falls back to the W1 `configuration.activeStates`
 * (the qualified-name read) when no step is revealed (e.g. just after a
 * rewind/restore). EITHER way the values are the W1 ORACLE'S — read
 * verbatim off its StepRecord / state_block, never computed (keystone). */
function highlightFromOracle(resp: SimResponse, revealed: number): void {
  const steps = resp.steps ?? [];
  if (revealed > 0 && steps.length >= revealed && steps[revealed - 1]) {
    // The oracle's OWN post-step config for the last revealed step.
    applyActiveConfig(steps[revealed - 1].configAfter ?? []);
    return;
  }
  if (steps.length > 0 && revealed === 0) {
    // Paused PRE-first-step: the oracle's pre-step config (configBefore
    // of step 0 — its own answer for "before this step").
    applyActiveConfig(steps[0].configBefore ?? []);
    return;
  }
  // No steps in this response (a bare restore/getContext) — the W1
  // state_block's active set (qualified names; matched best-effort).
  applyActiveConfig(resp.configuration?.activeStates ?? []);
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
  // ── W3: Run/Step resume/advance a breakpoint-paused reveal (§3.1 — a
  // cursor over the oracle's already-computed vector; zero recompute).
  byId("btnStep").addEventListener("click", () => {
    setStatus("⏭ step…");
    vscode.postMessage({ type: "step" });
  });
  byId("btnRun").addEventListener("click", () => {
    setStatus("▶ resume…");
    vscode.postMessage({ type: "run" });
  });
  // W3: the OPTIONAL bulk-clear (the PRIMARY affordance is the glyph ON
  // the node/edge — DBGUX §6 allows a collapsed bulk-clear, not a
  // detached breakpoint manager).
  byId("btnClearBps").addEventListener("click", () => {
    vscode.postMessage({ type: "clearBreakpoints" });
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
  } else if (msg.type === "transitionIds") {
    // W3: the edge-id → IR transition-stableId map (a PURE structural IR
    // read from the extension — the predicate target for a transition
    // breakpoint). Repaint glyphs (an edge may now resolve a stableId).
    edgeStableId = msg.map;
    paintGlyphs();
  } else if (msg.type === "breakpoints") {
    // W3: the armed-breakpoint set the EXTENSION owns (the predicate
    // lives there, over the W1 oracle's StepRecord fields). We only paint
    // the glyph states — zero semantics here.
    armedBps = msg.armed;
    if (!pausedAtBp) {
      hitBp = undefined;
    }
    paintGlyphs();
  } else if (msg.type === "simState") {
    // ── THE KEYSTONE RENDER PATH: paint the W1 response VERBATIM. The
    // active config, context, StepRecord timeline, clock are ALL read
    // straight off `resp` — this script computes NONE of them. W3 adds a
    // REVEAL CURSOR (§3.1): show `revealCount` of the oracle's already-
    // computed vector (omitted ⇒ all). `newCall` ⇒ commit the prior
    // active call's rows first; otherwise this is a ⏭ Step cursor advance
    // on the SAME active call (no recomputation — the oracle computed the
    // whole vector once).
    initialized = true;
    pausedAtBp = false;
    hitBp = undefined;
    byId("status").classList.remove("paused");
    const resp = msg.resp;
    const steps = resp.steps ?? [];
    const reveal = msg.revealCount ?? steps.length;
    if (msg.newCall ?? true) {
      // A new stepping-call: fold the prior active call (if any) into the
      // committed timeline, then make THIS the active call.
      commitActiveCall();
      setActiveCall(steps, reveal);
    } else {
      // ⏭ Step cursor advance on the SAME call — just move the cursor.
      activeSteps = steps;
      activeRevealed = Math.max(activeRevealed, Math.min(reveal, steps.length));
    }
    if (reveal >= steps.length) {
      // The call is fully revealed → commit it (immutable history).
      commitActiveCall();
    }
    caretRow = committed.length + activeRevealed - 1;
    highlightFromOracle(resp, reveal);
    renderContext(resp.context ?? {});
    renderTimeline();
    renderTimers();
    byId("clock").textContent = `⏱ virtual clock: ${resp.currentMs ?? "—"} ms`;
    const liveActive = resp.configuration?.activeStates ?? [];
    setStatus(
      reveal < steps.length
        ? `revealed ${reveal}/${steps.length} step(s) — ⏭ Step / ▶ Run to continue`
        : `active: ${liveActive.join(", ") || "(none)"}`,
    );
    refreshButtons();
    paintGlyphs();
    vscode.postMessage({ type: "stateApplied", stamp: msg.stamp });
  } else if (msg.type === "paused") {
    // ── W3 PAUSED-at-breakpoint. `resp` is the W1 `restore` response for
    // the snapshot taken just BEFORE the breaking step — the machine is
    // byte-exactly pre-step (the debugger-correct "stopped at the
    // breakpoint, not past it"). A DURABLE status line (NOT a toast —
    // DBGUX §2.2) + the glyph hit-state. The breaking step + any after
    // are BUFFERED (shown as "⋯ buffered" rows — honest, never hidden).
    // `msg.steps` is the oracle's already-computed full vector for the
    // paused call (verbatim — for the timeline rows ONLY; diagram/context
    // use the restored pre-step state).
    initialized = true;
    pausedAtBp = true;
    hitBp = { kind: msg.bpKind, targetId: msg.targetId };
    const resp = msg.resp;
    // The paused call becomes the active call; `revealCount` rows are
    // revealed, the rest (incl. the breaking step) shown as "⋯ buffered".
    commitActiveCall();
    setActiveCall(msg.steps ?? [], msg.revealCount);
    caretRow = committed.length + Math.max(0, msg.revealCount - 1);
    // Diagram/context/clock = the RESTORED pre-step state (the W1 restore
    // answer — verbatim; we compute nothing).
    applyActiveConfig(resp.configuration?.activeStates ?? []);
    renderContext(resp.context ?? {});
    renderTimeline();
    renderTimers();
    byId("clock").textContent = `⏱ virtual clock: ${resp.currentMs ?? "—"} ms`;
    setStatus(`● PAUSED — breakpoint (${msg.reasonText})`, false);
    byId("status").classList.add("paused");
    refreshButtons();
    paintGlyphs();
    vscode.postMessage({
      type: "pausedApplied",
      bpKind: msg.bpKind,
      targetId: msg.targetId,
      stamp: msg.stamp,
    });
  } else if (msg.type === "rewound") {
    // ── W3 TIME-TRAVEL. `resp` is the W1 `restore` response for snapshot
    // #index — diagram+context+clock jump there (verbatim; the oracle
    // restored its runtime, we render its answer). The timeline cursor
    // moves to #index; future rows are discarded (a new inject branches).
    initialized = true;
    pausedAtBp = false;
    hitBp = undefined;
    const resp = msg.resp;
    // Truncate history to the rewind point (a new inject branches here).
    activeSteps = [];
    activeRevealed = 0;
    committed.length = Math.min(committed.length, msg.index + 1);
    caretRow = Math.min(msg.index, committed.length - 1);
    applyActiveConfig(resp.configuration?.activeStates ?? []);
    renderContext(resp.context ?? {});
    renderTimeline();
    renderTimers();
    byId("clock").textContent = `⏱ virtual clock: ${resp.currentMs ?? "—"} ms`;
    byId("status").classList.remove("paused");
    setStatus(
      `↺ rewound to #${msg.index} — a new inject branches from here`,
      false,
    );
    refreshButtons();
    paintGlyphs();
    vscode.postMessage({ type: "rewoundApplied", index: msg.index, stamp: msg.stamp });
  } else if (msg.type === "simError") {
    // Honest: the StepError / invalid reason VERBATIM, a DURABLE status
    // indicator (not a toast) — never a fabricated clean end-of-run
    // (DBGUX §6 "inconclusive ≠ done"; the cardinal-sin bar).
    byId("status").classList.remove("paused");
    setStatus(msg.message, true);
  }
});

wireControls();
refreshButtons();

// Signal readiness so the extension's first render is not raced (the v1.3
// race-free first-render contract, reused).
vscode.postMessage({ type: "ready" });
