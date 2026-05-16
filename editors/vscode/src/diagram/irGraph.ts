// Pure `fsm-ir` JSON → diagram graph-model mapping (Doc 27 §6.2/§6.3; Doc 05
// §1.5.3-§1.5.6; Doc 09 §4/§5/§6).
//
// WHY a pure function with NO VS Code / DOM imports: this is the
// un-drift-able core of the diagram. It consumes the CANONICAL `fsm-ir`
// document (the SAME IR the whole toolchain — codegen, simulator,
// `--emit-ir` — is built on; Doc 09), so the rendered graph can never drift
// from the language semantics (exactly as the LSP's reuse of the `fsm
// check` pipeline made the squiggle un-drift-able — Doc 27 §6.3). Keeping
// it a pure `IR-JSON → {nodes,edges}` function (no `vscode`, no `elkjs`,
// no DOM) means: (a) the Extension-Host §5.4 acceptance can assert the
// structural model DIRECTLY (the exact state/transition set of the
// fixture's `--emit-ir` JSON — a real structural assertion, not "a webview
// opened"); (b) the same model is the ELK input the Webview lays out; and
// (c) the diagram stays Web-IDE-compatible (a pure `IR-JSON → ELK → SVG`
// function, Doc 27 §6.2) without building the Web IDE.
//
// SCHEMA (verified against crates/fsm-ir/src/model.rs + a real
// `fsm generate --emit-ir` artifact for the `Gate` fixture): the IR is
// `{ irVersion, machines: [{ name, root: RegionObject, ... }] }`;
// `RegionObject = { initial, states: StateNode[], ... }`; `StateNode` is a
// `kind`-tagged union (snake_case tags: `simple|composite|parallel|initial|
// final|choice|junction|history|fork|join|submachine_ref|entry_point|
// exit_point`); transitions live on `simple|composite|parallel|
// submachine_ref` states (`.transitions[]`, each `{ source, target, kind,
// trigger?, ... loc }`); composite/parallel own nested `regions[]`. Every
// node carries `loc = { file, line, column }` (1-based — fsm-diagnostics
// SourceLocation) for click→source.

/** A `loc` field as serialised by fsm-diagnostics `SourceLocation`. */
export interface IrSourceLocation {
  readonly file: string;
  /** 1-based source line (fsm-diagnostics is 1-based; VS Code is 0-based). */
  readonly line: number;
  /** 1-based source column. */
  readonly column: number;
}

/** Minimal structural view of the fsm-ir JSON this mapper consumes. */
interface IrTransition {
  readonly id: string;
  readonly source: string;
  readonly target: string;
  readonly kind: string;
  readonly trigger?: { readonly kind: string; readonly eventId?: string } | null;
  readonly guard?: unknown;
  readonly loc: IrSourceLocation;
}

interface IrStateNode {
  readonly kind: string;
  readonly id: string;
  readonly name?: string;
  readonly loc: IrSourceLocation;
  readonly transitions?: readonly IrTransition[];
  readonly regions?: readonly IrRegion[];
  // pseudo-state target shapes (initial/history/junction-branch etc.) are
  // not needed for the v1.3 read-only structural model beyond their node;
  // transitions are the authoritative edge set (Doc 09 §6).
}

interface IrRegion {
  readonly id: string;
  readonly name: string;
  readonly initial: string;
  readonly states: readonly IrStateNode[];
}

interface IrMachine {
  readonly id: string;
  readonly name: string;
  readonly root: IrRegion;
}

export interface IrDocument {
  readonly irVersion: string;
  readonly machines: readonly IrMachine[];
}

/** A render node — one state or pseudo-state. */
export interface GraphNode {
  /** The IR node id (stable within the document; the ELK node id). */
  readonly id: string;
  /** Display label (state name; pseudo-states get a synthetic label). */
  readonly label: string;
  /** The IR `kind` tag (drives the Webview shape per Doc 05 §1.5.5). */
  readonly kind: string;
  /** Containment parent region id; `null` for top-level (root-region) nodes. */
  readonly parentRegion: string | null;
  /** Source location for click→editor navigation (1-based, as in the IR). */
  readonly loc: IrSourceLocation;
}

/** A render edge — one transition. */
export interface GraphEdge {
  readonly id: string;
  readonly source: string;
  readonly target: string;
  /** External / local / internal / completion (Doc 09 §6). */
  readonly kind: string;
  /** Human label `EVENT` (or `done` for completion) — Doc 05 §1.5.6. */
  readonly label: string;
  readonly loc: IrSourceLocation;
}

/** A region container (composite/parallel inner regions + the root). */
export interface GraphRegion {
  readonly id: string;
  readonly name: string;
  /** Owning state id; `null` for a machine root region. */
  readonly parentState: string | null;
}

/** The full structural model for one machine — the ELK input + the test's
 * structural oracle. */
export interface DiagramModel {
  readonly machineName: string;
  readonly nodes: readonly GraphNode[];
  readonly edges: readonly GraphEdge[];
  readonly regions: readonly GraphRegion[];
}

/** `kind`s that own a `transitions[]` array (Doc 09 §4/§6). */
const STATES_WITH_TRANSITIONS = new Set([
  "simple",
  "composite",
  "parallel",
  "submachine_ref",
]);
/** `kind`s that own nested `regions[]` (Doc 09 §4.2/§4.3). */
const STATES_WITH_REGIONS = new Set(["composite", "parallel"]);

/** A readable label for a pseudo-state that has no DSL `name`. */
function pseudoLabel(kind: string, fallbackId: string): string {
  switch (kind) {
    case "initial":
      return "●";
    case "final":
      return "◎";
    case "choice":
      return "◇";
    case "junction":
      return "•";
    case "history":
      return "H";
    case "fork":
      return "⊟ fork";
    case "join":
      return "⊟ join";
    case "entry_point":
      return "⊙ entry";
    case "exit_point":
      return "⊙ exit";
    default:
      return fallbackId;
  }
}

/** The transition's display label (Doc 05 §1.5.6 `EVENT [guard] : action`;
 * v1.3 read-only renders the event/`done`, leaving guard/action detail to a
 * later wave's tooltip — Doc 05 §1.5.4 hover is OUT-scoped here). */
function edgeLabel(t: IrTransition): string {
  if (t.kind === "completion") {
    return "done";
  }
  const trig = t.trigger ?? undefined;
  if (trig && trig.kind === "event" && typeof trig.eventId === "string") {
    // Event ids are `ev-<Machine>-<EVENT>`; the user-facing token is the
    // trailing EVENT segment (stable across the document).
    const seg = trig.eventId.split("-");
    return seg[seg.length - 1] || trig.eventId;
  }
  if (trig && trig.kind === "after") {
    return "⏱ after";
  }
  return t.kind === "internal" ? "(internal)" : "";
}

/**
 * Recursively walk a region's states, accumulating nodes/edges/regions.
 * Pure + total: every state node becomes a `GraphNode`; every transition on
 * a transition-bearing state becomes a `GraphEdge`; composite/parallel inner
 * regions recurse. This is the exact structural projection of the fsm-ir
 * document — the test asserts the rendered model equals it.
 */
function walkRegion(
  region: IrRegion,
  parentState: string | null,
  nodes: GraphNode[],
  edges: GraphEdge[],
  regions: GraphRegion[],
): void {
  regions.push({ id: region.id, name: region.name, parentState });
  for (const st of region.states) {
    const label =
      typeof st.name === "string" && st.name.length > 0
        ? st.name
        : pseudoLabel(st.kind, st.id);
    nodes.push({
      id: st.id,
      label,
      kind: st.kind,
      parentRegion: region.id,
      loc: st.loc,
    });

    if (STATES_WITH_TRANSITIONS.has(st.kind) && st.transitions) {
      for (const t of st.transitions) {
        edges.push({
          id: t.id,
          source: t.source,
          target: t.target,
          kind: t.kind,
          label: edgeLabel(t),
          loc: t.loc,
        });
      }
    }
    if (STATES_WITH_REGIONS.has(st.kind) && st.regions) {
      for (const inner of st.regions) {
        walkRegion(inner, st.id, nodes, edges, regions);
      }
    }
  }
}

/**
 * Project one machine of an fsm-ir document into its diagram model. The
 * machine is selected by name when given (Doc 05 §1.5.1 "one diagram panel
 * per machine"), else the first machine. Throws `IrGraphError` on a
 * structurally invalid document (NEVER returns a half/empty model silently —
 * the cardinal-sin bar at the model boundary; a malformed IR is a hard
 * failure the caller surfaces, not a blank diagram pretending success).
 */
export function buildDiagramModel(
  ir: IrDocument,
  machineName?: string,
): DiagramModel {
  if (!ir || typeof ir.irVersion !== "string") {
    throw new IrGraphError(
      "IR document is missing the required `irVersion` field — it is " +
        "not a valid fsm-ir document (Doc 09 §2).",
    );
  }
  if (!Array.isArray(ir.machines) || ir.machines.length === 0) {
    throw new IrGraphError(
      "IR document contains no machines — nothing to diagram.",
    );
  }
  const machine =
    machineName !== undefined
      ? ir.machines.find((m) => m.name === machineName)
      : ir.machines[0];
  if (!machine) {
    throw new IrGraphError(
      `IR document has no machine named "${machineName}".`,
    );
  }
  if (!machine.root || !Array.isArray(machine.root.states)) {
    throw new IrGraphError(
      `machine "${machine.name}" has no root region — malformed IR.`,
    );
  }

  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];
  const regions: GraphRegion[] = [];
  walkRegion(machine.root, null, nodes, edges, regions);
  return { machineName: machine.name, nodes, edges, regions };
}

/** Hard failure parsing/projecting an IR document (never a silent blank). */
export class IrGraphError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "IrGraphError";
  }
}

/** Parse raw `--emit-ir` JSON then project it. Surfaces JSON-parse and
 * structural failures as `IrGraphError` (the caller keeps the last-valid
 * render + shows the Doc 05 §1.5.9 banner — it does NOT blank the webview). */
export function parseAndBuild(
  irJson: string,
  machineName?: string,
): DiagramModel {
  let doc: IrDocument;
  try {
    doc = JSON.parse(irJson) as IrDocument;
  } catch (e) {
    throw new IrGraphError(
      `the --emit-ir output is not valid JSON: ${String(e)}`,
    );
  }
  return buildDiagramModel(doc, machineName);
}
