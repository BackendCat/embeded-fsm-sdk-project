// Pure `documentSymbol` → activity-bar tree-node mapping (Doc 27 §5 /
// §8-V5; Doc 22 §7; Doc 14 §13).
//
// WHY a pure function with NO VS Code chrome construction: this is the
// un-drift-able core of the FSM activity-bar tree. It consumes the
// CANONICAL `documentSymbol` projection — the SAME single-analysis
// `analyze_with_source` result the in-editor squiggle and breadcrumbs use
// (crates/fsm-lsp/src/capabilities/document_symbol.rs:1-9 "No analysis
// happens here … one analysis feeds both"; advertised free at
// crates/fsm-lsp/src/server.rs:251). Re-projecting that response — instead
// of asking the `fsm` CLI for `--emit-ir` (V4's codegen-gated path) —
// means the tree can never drift from the language semantics AND never
// vanishes for a machine that parses but would not codegen (the MV5-1
// keystone: a chrome wave must not re-import the K-3 codegen-gated DRIFT;
// the symmetric analogue of V1's omit-transport N-1 and V4's
// ask-the-LSP-for-the-IR). Keeping it a pure
// `vscode.DocumentSymbol[] → FsmTreeNode[]` function (the only `vscode`
// dependency is the *type* `DocumentSymbol`, never a `window`/`commands`
// call) means the Extension-Host §5.4 acceptance can deep-compare the
// rendered node structure against an INDEPENDENTLY-obtained
// `executeDocumentSymbolProvider` oracle — a real structural equality,
// not "a tree provider is registered" (the P0-1 bar).
//
// SHAPE (verified against crates/fsm-lsp/src/capabilities/
// document_symbol.rs:80-200 + a real `executeDocumentSymbolProvider`
// round-trip for the multi-machine fixture): each machine is a root
// `SymbolKind.Module`; under it the analyzer emits NAMESPACE *group*
// nodes mirroring Doc 14 §13 verbatim — `context` / `events` / `externs`
// / `consts` / `enums` / `states` / `pseudo-states` — and the `states`
// subtree is the reconstructed statechart hierarchy (composite states
// nest their children; regions nest under composites). Every symbol
// carries a full-declaration `range` and a name `selectionRange`
// (selectionRange ⊆ range, the LSP contract) — `selectionRange` is the
// click→declaration target (Doc 05 §1.3.6; the same "navigate via the
// authoritative source location" discipline V4 used for click→source via
// the IR `SourceLocation`, no new location math).

import type { DocumentSymbol, Range } from "vscode";

/**
 * Which explorer a projected node belongs to. The Machines tree shows the
 * full machine→states/regions hierarchy; the Events tree is the flat
 * per-machine `events` set hoisted to the top level (Doc 22 §7:
 * `fsm.machineExplorer` / `fsm.eventExplorer`).
 */
export type FsmExplorerKind = "machines" | "events";

/**
 * One node of the projected FSM tree. A faithful, chrome-free re-shape of
 * a `vscode.DocumentSymbol`: `name`/`detail` are the symbol's verbatim,
 * `range`/`selectionRange` are carried through unchanged so click→reveal
 * navigates to the authoritative declaration (Doc 05 §1.3.6 — the
 * `selectionRange` IS the name span the LSP guarantees ⊆ `range`).
 */
export interface FsmTreeNode {
  /** The symbol name verbatim (e.g. machine `Demo`, state `Idle`). */
  readonly name: string;
  /**
   * The Doc 14 §13 parenthetical the server attaches (`Idle (simple)`,
   * `Operational (composite)`, `checkReady (pure)`, …) or the typed-field
   * form (`count: u32`) — `undefined` for the synthetic group nodes /
   * plain leaves the server emits with no `detail`.
   */
  readonly detail: string | undefined;
  /** The LSP `SymbolKind` numeric value (1-based; `Module` = 2, etc.). */
  readonly kind: number;
  /** Full-declaration range — the whole `state X { … }` (Doc 14 §13). */
  readonly range: Range;
  /**
   * The name token range. The LSP guarantees `selectionRange ⊆ range`;
   * this is what click→reveal selects (Doc 05 §1.3.6). For the synthetic
   * group nodes the server sets it to the enclosing machine span (still a
   * valid, in-file range — never a navigation no-op).
   */
  readonly selectionRange: Range;
  /** Child nodes (the statechart nesting). Empty for leaves. */
  readonly children: FsmTreeNode[];
}

/**
 * Re-project one `vscode.DocumentSymbol` (and its subtree) into an
 * {@link FsmTreeNode}. A faithful 1:1 structural copy — the node set,
 * names, kinds, nesting, and BOTH ranges are carried through verbatim so
 * the Extension-Host acceptance can deep-compare the tree against the
 * `executeDocumentSymbolProvider` oracle and click→reveal lands on the
 * exact `selectionRange` the server computed.
 */
function projectSymbol(sym: DocumentSymbol): FsmTreeNode {
  return {
    name: sym.name,
    detail: sym.detail === "" ? undefined : sym.detail,
    kind: sym.kind,
    range: sym.range,
    selectionRange: sym.selectionRange,
    children: (sym.children ?? []).map(projectSymbol),
  };
}

/**
 * Project the full `documentSymbol` response (one root per declared
 * machine) into the Machines-tree forest. A pure structural re-shape: the
 * roots, the Doc 14 §13 group nodes, and the statechart nesting are
 * preserved EXACTLY (re-projected, not re-analyzed — the MV5-1 contract).
 *
 * @param symbols the `vscode.executeDocumentSymbolProvider` result for the
 *   active `.fsm` (already the canonical single-analysis projection — this
 *   function adds no analysis and reaches for no CLI/IR path).
 */
export function projectMachineTree(symbols: readonly DocumentSymbol[]): FsmTreeNode[] {
  return symbols.map(projectSymbol);
}

/**
 * Project the Events tree: every machine's `events` group hoisted so each
 * machine root carries ONLY its event leaves (Doc 22 §7 `fsm.eventExplorer`
 * is the flat event view). A machine with no declared events is omitted
 * (the server emits no `events` group for it — there is nothing to show,
 * and a phantom empty machine row would be dishonest chrome).
 *
 * Still pure + still sourced from the SAME `documentSymbol` response — the
 * Events tree is a *lens* over the canonical projection, never a second
 * analysis or a different data source.
 */
export function projectEventTree(symbols: readonly DocumentSymbol[]): FsmTreeNode[] {
  const out: FsmTreeNode[] = [];
  for (const machine of symbols) {
    const eventsGroup = (machine.children ?? []).find((c) => c.name === "events");
    const eventLeaves = (eventsGroup?.children ?? []).map(projectSymbol);
    if (eventLeaves.length === 0) {
      continue;
    }
    out.push({
      name: machine.name,
      detail: machine.detail === "" ? undefined : machine.detail,
      kind: machine.kind,
      range: machine.range,
      selectionRange: machine.selectionRange,
      children: eventLeaves,
    });
  }
  return out;
}
