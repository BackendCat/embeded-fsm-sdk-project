// The CSP-locked diagram Webview script (Doc 27 §6.2/§8-V4; Doc 05 §1.5).
//
// Runs INSIDE the VS Code Webview (a browser context), loaded as the single
// nonce'd `<script>` the CSP allows. Responsibilities (and ONLY these —
// read-only, Doc 27 §9 OUT-scopes graphical editing + simulator overlay):
//   1. receive the structural `DiagramModel` (the pure fsm-ir projection)
//      from the extension via `postMessage`;
//   2. run ELK Layered (`elkjs`, the Doc 05 §1.5.9 "ELK Layered" / ROADMAP
//      §105 "@elklayout/core" the architecture intends) over the
//      node/edge/region graph — layout is a PURE client-side concern, the
//      server/CLI is never involved (Doc 27 §6.2);
//   3. render the laid-out graph as inline SVG (Doc 05 §1.5.5/§1.5.6
//      states/pseudo-states/transitions);
//   4. on a node click, post `revealSource` back so the extension reveals
//      the declaration line (Doc 05 §1.5.4 click → editor line, via the IR
//      `SourceLocation`);
//   5. on the `staleBanner` message (the codegen-gated boundary), SHOW the
//      Doc 05 §1.5.9 banner WITHOUT touching the existing rendered SVG —
//      the "keep the last valid state" contract; on first-ever-open with
//      no valid render it shows the banner over an honest "no diagram yet"
//      placeholder (never a blank panel pretending success).
//
// TESTABILITY: after a successful layout the script posts a `rendered` ack
// carrying the exact node/edge counts it laid out + whether the banner is
// visible. The V4 Extension-Host test asserts that ack equals the
// structural set of the SAME `--emit-ir` artifact the oracle parsed (a real
// IR→graph fidelity assertion — NOT "a webview opened", the P0-1 bar).

import ELK, { ElkNode } from "elkjs/lib/elk.bundled.js";

// ---- Types mirrored from ../irGraph.ts (the Webview cannot import the
// extension module; this is the postMessage wire shape, kept in lock-step
// by the shared protocol — a deliberate, commented duplication of the
// ~5-field model shape, NOT logic). ---------------------------------------

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

// Re-export the wire-shape interfaces + the pure `renderModel` renderer so
// the debug-W2 webview can REUSE this exact v1.3 ELK/SVG renderer VERBATIM
// rather than reimplement it (Doc 33 §W2 reuse ledger; the keystone:
// "additive overlay over that exact model — no second diagram"). These are
// ADDITIVE named exports only (erased at IIFE-bundle runtime — zero v1.3
// behaviour effect); the v1.3 standalone path below is byte-unchanged.
// esbuild bundles each entrypoint SEPARATELY, so v1.3's own
// `dist/webview/diagramWebview.js` is untouched — the debug bundle inlines
// its own copy of THIS source. (`showBanner` is intentionally NOT consumed
// by the debug webview — it is hard-coded to `#banner`; the debug surface
// uses `#debugBanner`. It is exported for symmetry / future reuse only.)
export type { DiagramModel, GraphNode, GraphEdge, GraphRegion, IrSourceLocation };
export { renderModel, showBanner };

type ExtToWebview = { type: "render"; model: DiagramModel } | { type: "staleBanner"; text: string };

interface VsCodeApi {
  postMessage(msg: unknown): void;
}
declare function acquireVsCodeApi(): VsCodeApi;

/**
 * Acquire the Webview→extension API as a window-cached SINGLETON.
 *
 * `acquireVsCodeApi()` may be called **at most once per webview** (a hard
 * VS Code constraint — a second call throws). On the v1.3 standalone path
 * this is the FIRST and ONLY acquirer, so the cache is populated on the
 * first call and behaviour is byte-identical to the previous bare
 * `acquireVsCodeApi()` (the SUBAGENT pre/post-identity bar — the v1.3
 * diagram.test.ts stays green). The reason it must be a singleton: the
 * debug-W2 webview (Doc 33 §W2) bundles its OWN inlined copy of THIS
 * module to reuse `renderModel` VERBATIM **and** itself needs the API —
 * two `acquireVsCodeApi()` calls in one webview would throw. Caching on
 * `window` makes both the inlined v1.3 copy and the debug script share the
 * ONE handle (the canonical multi-script-webview idiom). Exported so the
 * debug webview uses the SAME accessor. */
export function getVsCodeApi(): VsCodeApi {
  const w = window as unknown as { __fsmVsCodeApi?: VsCodeApi };
  if (!w.__fsmVsCodeApi) {
    w.__fsmVsCodeApi = acquireVsCodeApi();
  }
  return w.__fsmVsCodeApi;
}

const vscode = getVsCodeApi();
const elk = new ELK();

const SVG_NS = "http://www.w3.org/2000/svg";

function el<K extends keyof SVGElementTagNameMap>(name: K): SVGElementTagNameMap[K] {
  return document.createElementNS(SVG_NS, name);
}

function byId(id: string): HTMLElement {
  const e = document.getElementById(id);
  if (!e) {
    throw new Error(`missing #${id}`);
  }
  return e;
}

/** Pseudo-state kinds rendered as a small circle/diamond, not a box. */
const PSEUDO_KINDS = new Set([
  "initial",
  "final",
  "choice",
  "junction",
  "history",
  "fork",
  "join",
  "entry_point",
  "exit_point",
]);

/**
 * Lay the model out with ELK Layered and render the result as SVG. Returns
 * the laid-out counts for the test ack. Layout is deterministic for a given
 * input (Doc 05 §1.5.9 "same input → same layout") — `elk.layered` with no
 * randomised seed.
 */
async function renderModel(model: DiagramModel): Promise<{
  nodes: number;
  edges: number;
}> {
  const elkGraph: ElkNode = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "DOWN",
      "elk.spacing.nodeNode": "40",
      "elk.layered.spacing.nodeNodeBetweenLayers": "60",
    },
    children: model.nodes.map((n) => ({
      id: n.id,
      width: PSEUDO_KINDS.has(n.kind) ? 24 : Math.max(140, n.label.length * 9 + 24),
      height: PSEUDO_KINDS.has(n.kind) ? 24 : 44,
    })),
    edges: model.edges.map((e) => ({
      id: e.id,
      sources: [e.source],
      targets: [e.target],
    })),
  };

  const laid = await elk.layout(elkGraph);

  const svg = byId("svg") as unknown as SVGSVGElement;
  while (svg.firstChild) {
    svg.removeChild(svg.firstChild);
  }
  const w = Math.ceil(laid.width ?? 800);
  const h = Math.ceil(laid.height ?? 600);
  svg.setAttribute("viewBox", `0 0 ${w + 40} ${h + 40}`);

  const pos = new Map<string, { x: number; y: number; w: number; h: number }>();
  for (const c of laid.children ?? []) {
    pos.set(c.id, {
      x: c.x ?? 0,
      y: c.y ?? 0,
      w: c.width ?? 0,
      h: c.height ?? 0,
    });
  }

  // Edges first (drawn beneath nodes).
  for (const e of model.edges) {
    const g = el("g");
    g.setAttribute("class", "edge");
    const s = pos.get(e.source);
    const t = pos.get(e.target);
    if (s && t) {
      const x1 = s.x + s.w / 2;
      const y1 = s.y + s.h;
      const x2 = t.x + t.w / 2;
      const y2 = t.y;
      const p = el("path");
      p.setAttribute(
        "d",
        `M ${x1} ${y1} C ${x1} ${(y1 + y2) / 2}, ${x2} ${(y1 + y2) / 2}, ${x2} ${y2}`,
      );
      g.appendChild(p);
      if (e.label) {
        const lbl = el("text");
        lbl.setAttribute("x", String((x1 + x2) / 2 + 4));
        lbl.setAttribute("y", String((y1 + y2) / 2 - 2));
        lbl.textContent = e.label;
        g.appendChild(lbl);
      }
    }
    svg.appendChild(g);
  }

  // Nodes.
  for (const n of model.nodes) {
    const p = pos.get(n.id);
    if (!p) {
      continue;
    }
    const g = el("g");
    const isPseudo = PSEUDO_KINDS.has(n.kind);
    g.setAttribute("class", isPseudo ? "pseudo" : "node");
    g.setAttribute("transform", `translate(${p.x},${p.y})`);
    g.setAttribute("data-line", String(n.loc.line));
    g.setAttribute("data-column", String(n.loc.column));

    if (isPseudo) {
      const c = el("circle");
      c.setAttribute("cx", String(p.w / 2));
      c.setAttribute("cy", String(p.h / 2));
      c.setAttribute("r", String(Math.min(p.w, p.h) / 2 - 2));
      g.appendChild(c);
    } else {
      const r = el("rect");
      r.setAttribute("width", String(p.w));
      r.setAttribute("height", String(p.h));
      r.setAttribute("rx", "6");
      g.appendChild(r);
    }
    const text = el("text");
    text.setAttribute("x", String(p.w / 2));
    text.setAttribute("y", String(p.h / 2 + 4));
    text.setAttribute("text-anchor", "middle");
    text.textContent = n.label;
    g.appendChild(text);

    // Doc 05 §1.5.4 — click → reveal the declaration line in the editor
    // (via the IR SourceLocation; the extension does the actual reveal).
    g.addEventListener("click", () => {
      vscode.postMessage({
        type: "revealSource",
        line: n.loc.line,
        column: n.loc.column,
      });
    });
    svg.appendChild(g);
  }

  return { nodes: model.nodes.length, edges: model.edges.length };
}

function showBanner(text: string): void {
  const b = byId("banner");
  b.textContent = text;
  b.classList.add("show");
}

let everRendered = false;

// STANDALONE-DIAGRAM GUARD (behaviour-preserving): the v1.3 read-only
// diagram HTML (`diagramPanel.ts`) ships BOTH `#svg` AND `#banner`, so this
// guard is ALWAYS true on the v1.3 path — its message-listener / `ready`
// post is byte-identical to before (the v1.3 diagram.test.ts proves it
// stays green — the SUBAGENT refactor pre/post-identity bar). The debug-W2
// webview imports this module to reuse `renderModel` VERBATIM but drives
// the diagram via its OWN message listener; its HTML deliberately uses
// `#debugBanner` (NOT `#banner`) so this guard is FALSE there, suppressing
// the inlined v1.3 copy's auto-bootstrap (a second listener / a second
// `ready` would race the debug panel). `#svg` is shared (renderModel needs
// it); `#banner` is the discriminator. Gating the side-effects on the
// exact v1.3 DOM contract keeps v1.3 byte-identical while making the
// renderer cleanly reusable with NO fork.
if (document.getElementById("svg") && document.getElementById("banner")) {
  window.addEventListener("message", (ev: MessageEvent<ExtToWebview>) => {
  const msg = ev.data;
  if (msg.type === "render") {
    // A fresh valid render — clear any stale banner (Doc 05 §1.5.9: the
    // banner is the "last valid state" indicator; a successful re-render
    // means we are no longer stale).
    byId("banner").classList.remove("show");
    byId("status").style.display = "none";
    renderModel(msg.model)
      .then((counts) => {
        everRendered = true;
        vscode.postMessage({
          type: "rendered",
          machineName: msg.model.machineName,
          nodes: counts.nodes,
          edges: counts.edges,
          bannerVisible: false,
        });
      })
      .catch((e: unknown) => {
        // A layout failure is honest, not silent: surface it and ack so
        // the test never hangs (the cardinal-sin bar in the Webview too).
        byId("status").style.display = "block";
        byId("status").textContent = `Diagram layout error: ${String(e)}`;
        vscode.postMessage({
          type: "rendered",
          error: String(e),
          nodes: 0,
          edges: 0,
          bannerVisible: false,
        });
      });
  } else if (msg.type === "staleBanner") {
    // The codegen-gated boundary: keep whatever is already rendered
    // (the "last valid state") and show the Doc 05 §1.5.9 banner. If
    // nothing has ever rendered, replace the loading text with an honest
    // placeholder (never a blank panel presented as a working diagram).
    if (!everRendered) {
      byId("status").style.display = "block";
      byId("status").textContent =
        "No diagram yet — the machine did not produce IR (codegen-gated).";
    }
    showBanner(msg.text);
    vscode.postMessage({
      type: "staleShown",
      bannerVisible: true,
      hasLastValidRender: everRendered,
    });
  }
  });

  // Signal readiness so the extension's first render is not raced.
  vscode.postMessage({ type: "ready" });
}
