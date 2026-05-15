# FSM Studio SDK — Decisions and Reconciliation (v1.0)

**Document ID:** FSM-SPEC-DEC
**Version:** 1.0.0
**Status:** Normative — supersedes conflicting text in Docs 01–24
**Date:** 2026-05-11
**Authors:** TL/PM working with senior-dev framing
**Baseline:** `VALIDATION_REPORT.md` dated 2026-02-18 (14 BLOCKERS, 42 GAPs, 28 INCONSISTENCIES)

---

## 0. One-Page Summary

This document is the result of one round of spec reconciliation against the gate-review
report. It is the **first** document an implementer, reviewer, or downstream agent should
read; everything else in `docs/` is to be interpreted through these decisions.

**Scope of v1.0 (locked):**

- **Statechart power:** full UML 2.5.1 behavioral statechart — composite states, parallel
  (orthogonal) regions, shallow + deep history, fork/join, submachines (referenced via `is`),
  deferred events, entry/exit points, choice/junction pseudo-states.
- **Codegen targets:** **C99 only**. C++17 (Doc 12) and Rust runtime are explicitly
  deferred to v1.1.
- **Frontends:** **CLI only** (`fsm` binary per Doc 18). LSP (Doc 14), VS Code extension
  (Doc 22), the Web IDE (Doc 03 §X / Doc 05), and TextMate grammar publishing (Doc 21)
  are deferred to v1.1+. The simulator interpreter is retained for **internal testing
  only** — no WebSocket server is exposed in v1.0.
- **Kept in v1.0:** lexer, parser, analyzer, IR, codegen-c, simulator-as-library,
  formatter, CLI, conformance tests, HAL spec, assembly integration (the optional sections
  in Doc 17).

**What was reconciled in this pass:**

1. All **14 BLOCKERS** from VALIDATION_REPORT are resolved with explicit, normative
   decisions (Section 2). Each carries a problem statement, a chosen resolution, a
   rationale citing UML 2.5.1 / HSM literature / pragmatic embedded constraints, the
   affected docs, and a doc-only-vs-implementation tag.
2. All **28 INCONSISTENCIES** are resolved in a Doc-A-vs-Doc-B table (Section 3).
3. A single-source-of-truth registry (Section 4) names exactly one authoritative document
   per concept; every other doc cross-references it.
4. The **42 GAPs** are triaged into "must-fix for v1.0" vs "deferred to v1.1+" with the
   reasoning (Section 5).
5. A consolidated list of explicitly **deferred items** (Section 6) so future agents can
   tell at a glance what is intentionally not in scope.
6. **Implementation guidance** (Section 7) tells the Phase-1 implementer waves how each
   blocker resolution maps to a crate, a file, and a struct/function.
7. A **doc-update plan** (Section 8) — a bullet list of specific patches to existing docs
   so a follow-on rippling agent can execute mechanically. **This document does not edit
   any existing doc;** that is intentional and that is the next agent's job.

**Standing principle:** when a v1.0 doc disagrees with this document, this document
wins. When two v1.0 docs disagree and this document is silent, fall back to the single-
source-of-truth registry in Section 4.

---

## 1. How to Read This Document

- **Normative** statements use MUST / MUST NOT / SHOULD / SHOULD NOT / MAY per RFC 2119.
- Each blocker resolution is tagged either **doc-only** (no implementation change required
  — the code already does the right thing, the docs describe the wrong thing) or
  **doc + implementation** (a normative semantic decision an implementer must encode).
- Finding IDs (e.g., `1.8`, `3.2`, `6.1`) reference VALIDATION_REPORT.md sections.
- Doc references use the form `Doc NN §X.Y` matching the docs/ directory.

---

## 2. Blocker Resolutions (14 items)

### B-01 (1.8) — Diagnostic Code Authority Conflict

**Problem.** Doc 02 §13 and Doc 10 both define diagnostic codes and disagree across the
entire `E01xx` range and on `E0001–E0005`. Codes are also re-used inconsistently inside
Doc 04 (§3 vs §10 vs §14.2 vs §15). See report findings 1.1–1.8 and 7.1.

**Resolution.**

1. **Doc 10 is the single normative source for diagnostic codes.** Every other document
   that names a code MUST link to Doc 10 and MUST NOT redefine the code's meaning.
2. **Doc 02 §13** (the four sub-tables 13.1–13.6) is **retired**. The whole section is
   replaced by a one-line cross-reference: *"See Doc 10 — Diagnostic Code Catalog."*
3. **Phantom codes referenced but not yet defined** (1.1, 1.2, 1.3) are added to Doc 10:
   - `FSM-W0604` — Possible precision loss in `as` cast between float widths.
   - `FSM-E0110` — Local transition (`~>`) target is not a descendant of the source.
   - `FSM-H0006` — Stable ID `@id(...)` placed before doc comment (suggest swap).
4. **E0600 collision** (Doc 04 = "feature not enabled", Doc 10 = "parallel region has no
   initial"). Doc 10's existing meaning wins for `FSM-E0600`. A **new code `FSM-E0610`**
   is allocated for "construct used without required feature flag." Doc 04 §15 and Doc 04
   §2.2 references update to E0610.
5. **E0304 vs E0600 duplication** (1.7). Both describe "parallel region has no initial."
   `FSM-E0304` is **retired** (status: Deprecated — no longer emitted; suppressed forms
   still parsed per Doc 10 §14). `FSM-E0600` is the canonical code.
6. **Doc 04 internal contradictions** (1.4 / 1.5 / 1.6) — codes get realigned to Doc 10:
   - §10 "Duplicate `@id`: FSM-E0750" → **FSM-E0025** (matches §14.2 and Doc 10).
   - §3 "Opaque field guard: FSM-E0303" → keep E0303 if and only if Doc 10's E0303 is
     reworded; otherwise **allocate FSM-E0210** ("opaque field used in field-comparison
     guard"). I choose: **rename Doc 10's E0303 to "Join source not in parallel region"
     stays, and allocate a new E0210 for opaque-field-in-guard.** That preserves the
     E03xx determinism block.
   - §15 "Feature not enabled: FSM-E0600" → **FSM-E0610** (per item 4 above).

**Rationale.** A single normative table is the only structurally stable answer; the rest
is the recurring "drift" problem identified in the report's closing note. Choosing Doc 10
matches the report's recommendation, matches the LSP/CLI machine-readable surface, and is
the most-cross-referenced of the three. Reserving a new code (E0610, E0210) for an
overlapping concept is cheaper than re-numbering, and Doc 10 §14 already mandates
"diagnostic codes are never reused" — so retiring E0304 / leaving E0600 means the
deprecation policy applies cleanly.

**Affected docs.** Doc 02 §13 (replace with cross-ref), Doc 04 §3, §10, §15, Doc 10 (add
W0604, E0110, H0006, E0610, E0210; mark E0304 deprecated), Doc 11 §26, Doc 15, Doc 24.

**Type.** Doc-only (no runtime behaviour changes; only the code numbers reported).
Implementation impact is limited to a string-table in `fsm-diagnostics`.

---

### B-02 (1.9 / 1.10 / 1.11 / 1.22) — Simulator Protocol Has Two Incompatible Definitions

**Problem.** Doc 03 §X (Infrastructure) and Doc 13 each define what is supposed to be the
same simulator protocol; they use different method names (`sim.init` vs `sim/init`),
different method sets, different response shapes, different notifications, and three
incompatible trace-record schemas (Doc 03, Doc 13, Doc 24).

**Resolution.**

1. **For v1.0 there is no WebSocket simulator.** The simulator is an in-process Rust
   library used by the conformance test runner and by `fsm test`. No JSON-RPC, no port
   7842, no auth.
2. **Doc 13** is **frozen as the v1.1 reference** for when the WebSocket simulator ships.
   It is **deferred** (see Section 6). The whole document remains in the tree as
   *"Status: Reserved — v1.1 surface"* but no implementation is required.
3. **Doc 03's simulator section** is **deleted** (it is the older draft and conflicts
   with the more complete Doc 13). The CLI's `fsm simulate` subcommand (Doc 18 §5.7)
   for v1.0 launches the in-process interpreter, prints active states, and exits — no
   network.
4. **Trace record format** (1.22): the **`StepRecord` schema from Doc 13 §11** is
   the single normative trace shape, regardless of consumer. Conformance `.trace`
   files (Doc 15 / Doc 24) MUST emit/expect this exact JSON shape. Doc 03's and Doc 24's
   older variants are retired.
5. When v1.1 reopens the simulator, the **slash form** (`sim/init`, `sim/step`, …) is
   normative. JSON-RPC matches strings literally; dot-form is forbidden.

**Rationale.** Cutting the WebSocket scope for v1.0 removes the third copy of an already-
duplicated spec, eliminates the security gap (4.5: simulator on `0.0.0.0` with no auth),
removes the reconnection-protocol gap (2.23), and prevents drift while the test runner is
the only consumer. Doc 13 is more recent and complete than Doc 03's section, so it
becomes the future authority. The slash form matches rust-analyzer / clangd LSP
conventions and is unambiguous for JSON-RPC method dispatch.

**Affected docs.** Doc 03 (delete simulator section), Doc 13 (mark Reserved/v1.1), Doc 15
(adopt Doc 13 StepRecord), Doc 18 §5.7 (specify no-network behaviour for v1.0), Doc 24
(use Doc 13 StepRecord).

**Type.** Doc-only (the v1.0 implementation never had a WebSocket server) + a small
implementation note: the in-process interpreter MUST emit `StepRecord` JSON when run
under `fsm test`.

---

### B-03 (1.12 / 1.13 / 1.14) — Keyword List Drift

**Problem.** Doc 04 §1.5 is the master keyword list. Doc 21 (TextMate) drops 11 and adds
10. Doc 14 (LSP) drops 28 and adds 10. Doc 19 (Formatter) uses syntax that doesn't even
exist in the grammar.

**Resolution.**

1. **Doc 04 §1.5 is the single normative keyword list.** Every other doc MUST
   cross-reference it and MUST NOT redefine it inline.
2. Docs 14, 21, and 22 are **deferred to v1.1** (see Section 6) — but the underlying
   list still must reconcile **now**, because Doc 19 (Formatter) is **in scope** for
   v1.0 and ships first, and the LSP/TextMate work in v1.1 will start from the v1.0 list.
3. Doc 04 §1.5 is treated as **append-only** going forward (new keywords are minor-version
   adds, never removals — per Doc 10 §14 deprecation policy applied analogously).
4. Doc 19 (Formatter) is reconciled in B-13 below; both the keyword set and the syntax
   shapes are corrected there.

**Rationale.** Single-source-of-truth. The list lives in the grammar specification where
the parser implementer reads it.

**Affected docs.** Doc 04 §1.5 (canonical), Doc 14 (defer; when revived, reference Doc 04),
Doc 19 (rewrite — see B-13), Doc 21 (defer; when revived, reference Doc 04), Doc 22 (defer).

**Type.** Doc-only.

---

### B-04 (1.19 / 1.20) — VS Code Three-Way Conflict (Deferred)

**Problem.** Doc 03, Doc 05, and Doc 22 each define VS Code commands and configuration
keys. They use different command IDs, different prefix conventions (`FSM Studio:` vs
`FSM:`), and different setting names (`fsmLang.lspPath` vs `fsmLang.compilerPath`).

**Resolution.** **Deferred entirely to v1.1.** No VS Code extension ships in v1.0. For
the v1.1 revival, **Doc 22** (the actual manifest specification) is designated
authoritative; Doc 03's and Doc 05's command/settings tables are deleted or rewritten as
cross-references to Doc 22.

**Rationale.** v1.0 scope is CLI-only; resolving the three-way drift now serves no v1.0
implementer. Recording the resolution preserves it.

**Affected docs.** None for v1.0. For v1.1: Doc 03 (delete), Doc 05 (rewrite/defer),
Doc 22 (canonical).

**Type.** Deferred.

---

### B-05 (2.4 / 2.5) — Expression Grammar Is Left-Recursive

**Problem.** Doc 04 §8.7 defines `expr = expr , bin_op , expr` which is directly
left-recursive. An engineer implementing the EBNF as written produces an infinite-loop
parser. The same problem exists in the guard grammar (§8.2 by inheritance).

**Resolution.**

1. **The EBNF in Doc 04 §8.7 is informational.** A one-line normative banner is added
   at the top of §8.7: *"The EBNF in this section is presentation-only. The
   normative grammar for expression parsing is the Pratt operator-precedence table in
   §8.7.1; implementations MUST use that table or an equivalent precedence-climbing
   algorithm."*
2. **§8.7.1 (Pratt precedence table) is normative.** It is the source of truth for binding
   powers, associativity, and operator semantics.
3. Doc 04 §8.7 is rewritten in non-left-recursive form **as well** (see doc-update plan,
   Section 8) so a casual reader does not produce a broken parser. Concretely:

   ```ebnf
   expr        = expr_or ;
   expr_or     = expr_and , { "||" , expr_and } ;
   expr_and    = expr_cmp , { "&&" , expr_cmp } ;
   expr_cmp    = expr_bit ,  [ cmp_op , expr_bit ] ;
   expr_bit    = expr_shift, { bitwise_op , expr_shift } ;
   expr_shift  = expr_add  , { shift_op   , expr_add } ;
   expr_add    = expr_mul  , { add_op     , expr_mul } ;
   expr_mul    = expr_cast , { mul_op     , expr_cast } ;
   expr_cast   = unary_expr, { "as" , type_ref } ;
   unary_expr  = { "!" | "-" | "~" } , postfix_expr ;
   postfix_expr= primary , { "." , identifier | "(" , [ arg_list ] , ")" } ;
   primary     = literal | field_ref | identifier | "(" , expr , ")" ;
   ```

   This rewrite preserves the precedence table exactly (each rule = one level).

**Rationale.** This is the rust-analyzer / Rust reference / Roslyn precedent: declarative
EBNF reads naturally but is unimplementable; the precedence-climbing table is the
real spec. Marking the EBNF informational with a Pratt-table normative anchor is a single
sentence that costs zero engineering effort. The non-left-recursive rewrite removes the
trap for engineers who skim only the EBNF.

**Affected docs.** Doc 04 §8.2, §8.7, §8.7.1.

**Type.** Doc-only.

---

### B-06 (2.7–2.11) — IR Schema Has Missing Construct Categories

**Problem.** The IR (Doc 09) cannot represent five constructs the DSL supports:
`const` declarations, `feature`/`import`/`queue`/`target` top-level blocks, `cast`
expressions and `float` literals, `enum_variant` literals, and the difference between
local (`~>`) and external (`->`) transitions. Result: `fsm decompile` is not
round-trippable, and codegen consumers cannot read queue capacity from the IR alone.

**Resolution.** The IR schema for v1.0 MUST add the following constructs. Each is a
**normative** addition.

1. **`MachineObject.consts: ConstDecl[]`** (new, REQUIRED, may be empty).
   ```jsonc
   ConstDecl = {
     "id":      "c-<slug>",
     "stableId": "Machine:const:NAME",
     "name":    "MAX_RETRIES",
     "type":    TypeRef,        // existing TypeRef union
     "value":   Literal,        // existing Literal union
     "loc":     SourceLocation
   }
   ```

2. **Top-level (file-scope) document blocks** on `IrDocument`:
   - `IrDocument.features: string[]` (new, REQUIRED, may be empty) — list of declared
     feature flag names.
   - `IrDocument.imports: ImportDecl[]` (new, REQUIRED, may be empty).
     ```jsonc
     ImportDecl = {
       "path":   "common/events.fsm",
       "alias":  "Common" | null,
       "items":  ["PacketType", "ErrorCode"] | null,
       "loc":    SourceLocation
     }
     ```

3. **`MachineObject.queue: QueueConfig`** (new, REQUIRED).
   ```jsonc
   QueueConfig = {
     "capacity": 32,                                    // integer, power-of-2 recommended
     "overflow": "assert" | "drop_oldest" | "drop_newest",
     "loc":      SourceLocation
   }
   ```

4. **`MachineObject.target: TargetConfig`** (new, REQUIRED).
   ```jsonc
   TargetConfig = {
     "profile":   "C99" | "C99-RTOS" | "Cpp17" | "Simulation",
     "strategy":  "switch_based" | "table_driven",
     "allowFloat": false,
     "maxNesting": 8,
     "loc":       SourceLocation
   }
   ```

5. **`Expr.kind = "cast"`** (new variant of the Expression discriminated union).
   ```jsonc
   { "kind": "cast", "operand": Expr, "targetType": TypeRef, "loc": SourceLocation }
   ```

6. **`Literal.literalKind = "float"`** (new variant).
   ```jsonc
   { "kind": "literal", "literalKind": "float", "value": 1.5, "loc": SourceLocation }
   ```

7. **`Literal.literalKind = "enum_variant"`** (new variant).
   ```jsonc
   { "kind": "literal", "literalKind": "enum_variant",
     "enumName": "PacketType", "variant": "HEARTBEAT", "loc": SourceLocation }
   ```

8. **`TransitionObject.kind`** (new REQUIRED field) replacing the inadequate
   `internal: boolean`. Values: `"external"` | `"local"` | `"internal"` | `"completion"`.
   The existing `internal` boolean is **deprecated** but preserved for one minor cycle
   (consumers MUST treat `kind === "internal"` as authoritative; `internal: true` is
   computed from `kind === "internal"`). After v1.0 → v1.1 it is removed.

**Rationale.** Each construct exists in the DSL grammar (Doc 04) and in the formal
semantics (Doc 08), so the round-trip property the spec promises in Doc 02 G7 is broken
without them. Distinguishing local from external transitions is essential because they
produce different LCA-derived exit sets (per Doc 08 §5 and per UML 2.5.1 §14.2.3.9).
Choosing a `kind` discriminator is more extensible than `internal: bool` and matches the
TransitionKind enum in UML/MDA tooling (Eclipse UML2, MagicDraw).

**Affected docs.** Doc 09 (schema additions), Doc 11 (codegen must consume new fields),
Doc 15 (golden IR comparisons regenerated), Doc 24 (test fixtures regenerated). The
`schema/ir/1.0.0/model.json` file MUST be updated in lockstep.

**Type.** Doc + implementation — `fsm-ir` crate gains five new struct/enum definitions;
`fsm-parser` populates them; every IR consumer adds match arms.

---

### B-07 (3.1) — Guarded Completion Transitions Are Forbidden In One Doc, Allowed In Another

**Problem.** Doc 08 §4.4 forbids guards on completion transitions (raises FSM-E0301).
Doc 02 G4 and Doc 04 §8.4 explicitly include `guard_clause` on the completion grammar.

**Resolution.** **Guards on completion transitions are ALLOWED.** Doc 08 §4.4's
prohibition is removed. The `FSM-E0301` code is **repurposed and renamed** to
"Completion transition has both a guard and an `else` branch where guard always
evaluates true" (a less aggressive lint), or — simpler — **retired** (Doc 10 §14
deprecation policy) and replaced with **FSM-W0301** ("Completion transition has a guard
that may never fire") as a warning, not an error. I choose **retire E0301 entirely** and
introduce no replacement; the existing nondeterminism (E0300) and dead-transition
(W0101) codes already cover the failure modes that motivated E0301.

**Rationale.** UML 2.5.1 §14.2.3.9.5 explicitly permits guards on completion transitions
and uses them as the standard way to model "when this state finishes AND condition X,
go to T1, else go to T2." Statecharts: Visual Formalism (Harel 1987) treats completion
as a degenerate event so guards on it are no different from guards on any other
transition. Forbidding them eliminates a standard modeling pattern with no safety
benefit.

**Affected docs.** Doc 08 §4.4 (remove prohibition), Doc 10 (mark E0301 Deprecated),
Doc 02 (no change — already correct), Doc 04 (no change — already correct).

**Type.** Doc + minor implementation — the analyzer must stop emitting E0301; remove or
replace the corresponding test case (B-09 below).

---

### B-08 (3.2) — Parallel State Completion Fires Prematurely

**Problem.** Doc 08 §9.1 implies each region independently generates a completion event
when reaching its final state, which (read literally) fires the parent's `done`
transition as soon as the first region finishes. UML semantics require the completion
transition of a parallel state to fire only after **all** regions have reached an
active final state. Doc 04 §11 has the same ambiguity.

**Resolution.** **A parallel composite state's completion event is generated if and only
if every region's currently active leaf is a `final` state.** Until that holds, no
completion event is enqueued for the parallel state. Per-region final-entry no longer
triggers a parent completion event individually.

Normative algorithm (replaces Doc 08 §9.1 and harmonizes with Doc 04 §11 DISPATCH's
completion pass):

```
function maybe_enqueue_completion(state S):
    if S is a simple/composite final state:
        enqueue_completion_event(S.parent)
    elif S is composite and S.region.active_leaf is final:
        enqueue_completion_event(S)             # forward outward
    elif S is parallel:
        if for every region R of S: R.active_leaf is final:
            enqueue_completion_event(S)
    # else: nothing to do
```

The runtime invokes `maybe_enqueue_completion(active_leaf_of_each_region_just_entered)`
after every entry sequence.

**Rationale.** UML 2.5.1 §14.2.3.4.5: "A composite state is completed when all its
regions have reached a FinalState." Direct citation. Harel's original statecharts paper
does not address parallel completion explicitly, but every UML-conforming tool (Yakindu,
ENTERPRISE Architect, IBM Rhapsody, Telelogic Statemate) implements all-regions-done.
The premature-fire reading would also break the test in Doc 24 PARSE-POS-008 (fork+join
example) if examined carefully.

**Affected docs.** Doc 08 §9.1, Doc 04 §11 (clarify DISPATCH completion pass).

**Type.** Doc + implementation — `fsm-simulator` and the C99 codegen completion handler
both must check all regions of a parallel state before enqueueing a completion event.

---

### B-09 (3.3) — LCA for Self-Transitions Is Wrong For External Variant

**Problem.** Doc 08 §5.1 defines LCA inclusively ("a state is its own ancestor"), so
`LCA(S, S) = S`. For an **external** self-transition (`S -> S`), the spec then says no
exit and no entry happen — but UML 2.5.1 requires external self-transitions to exit
and re-enter S (firing exit and entry actions). For a **local** self-transition
(`S ~> S`), no exit/entry is correct.

**Resolution.** **The LCA function is no longer overloaded for self-transitions.**
Instead, the exit-set / entry-set computation in Doc 08 §6 and §7 consults the
transition's **kind** (B-06):

| Transition kind | LCA(S, S) | Exit set | Entry set |
|---|---|---|---|
| External (`->`) | `S.parent` | `{S}` | `{S}` |
| Local (`~>`)    | `S`        | `∅`   | `∅`   |
| Internal (`internal on E`) | `S` | `∅` | `∅` |
| Completion (no trigger)    | as per source/target | as per source/target | as per source/target |

When `source == target` for an external transition, the algorithm treats LCA as
`S.parent` so the exit set contains `S` and the entry set contains `S`. The text
"a state is its own ancestor" in §5.1 is reworded: *"For LCA computation, a state is
considered an ancestor of itself except when the transition kind is `external` and
source equals target, in which case the effective LCA is `source.parent`."*

**Rationale.** UML 2.5.1 §14.2.3.9.6 ("Effect of a Transition") spells this out: an
external transition always crosses the source state's boundary, so a self-transition
exits and re-enters. A local transition is defined precisely as the variant that does
not cross the source boundary. The distinction is the whole point of having two notations.

**Affected docs.** Doc 08 §5.1, §6, §7, Doc 11 §8 (switch codegen exit/entry sequencing
for self-transitions), Doc 04 §8.3 (already correct; just reinforce in note form).

**Type.** Doc + implementation — `fsm-analyzer` LCA function and `fsm-codegen-c`
exit/entry emitter both consume the transition `kind`.

---

### B-10 (3.8) — Switch-Based Codegen Doesn't Walk Up From The Leaf

**Problem.** Doc 11 §8's switch-based dispatch matches `m->_state` (always a leaf) and
never walks up to ancestors. Transitions declared on a composite parent silently fail to
fire when the leaf is a nested descendant. This is a classic HSM codegen bug.

**Resolution.** The switch-based dispatch MUST be reorganized so that, for each event
delivered, the runtime tries transitions in **innermost-first ancestor-walk** order. A
concrete normative pseudocode template (added to Doc 11 §8):

```c
void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    Motor_StateId_t s = m->_state;       /* always a leaf */
    while (s != MOTOR_STATE__ROOT_SENTINEL) {
        if (Motor_try_transitions_in_state(m, s, ev)) {
            return;                      /* a transition fired */
        }
        s = Motor_parent_table[s];        /* static const Motor_StateId_t[] */
    }
    /* No ancestor of m->_state defined a transition for ev; event is discarded. */
}

/* Per-state generated helper. Returns true if a transition fired. */
static bool Motor_try_transitions_in_state(
    Motor_t *m, Motor_StateId_t s, const Motor_Event_t *ev)
{
    switch (s) {
    case MOTOR_STATE_OPERATIONAL:
        switch (ev->id) {
        case MOTOR_EVENT_FAULT:
            /* exit set / entry set are baked in at codegen time */
            Motor_exit_path_OPERATIONAL_to_ERROR(m);
            m->_state = MOTOR_STATE_ERROR;
            Motor_entry_path_ROOT_to_ERROR(m);
            return true;
        default: break;
        }
        break;
    case MOTOR_STATE_OPERATIONAL_RUNNING:
        switch (ev->id) {
        /* ... per-state generated cases ... */
        default: break;
        }
        break;
    /* ... one case per state ... */
    default: break;
    }
    return false;
}
```

The `Motor_parent_table[]` is a static const array indexed by `Motor_StateId_t`,
populated from the IR's state hierarchy at codegen time. Cost: 1 byte per state on
≤256-state machines (i.e. ≤256 bytes ROM for the table; trivial).

Priority within a single state is handled inside `try_transitions_in_state` (sort cases
by priority then document order at codegen time). The ancestor-walk handles the
"inner-beats-outer" rule for **free** because the leaf is tried first.

**Rationale.** This is the canonical pattern for HSM dispatch in switch-based codegen
(see: Samek, "Practical UML Statecharts in C/C++," 2nd ed., Chapter 4 — the hierarchical
event processor; QP/C and QP/Cpp use this exact pattern with a slightly different
`top()` sentinel). The parent table costs at most O(N) ROM, and the ancestor walk costs
at most O(depth) per event — usually 1–4 hops. The alternative (replicating every
parent transition into every leaf's switch case) explodes binary size on deep
hierarchies and is harder to debug.

**Affected docs.** Doc 11 §8 (rewrite with the above template), Doc 11 §9 (table-driven
strategy needs the equivalent — see B-11), Doc 20 (architecture diagram callout), Doc 15
(add hierarchical-dispatch conformance test).

**Type.** Doc + implementation — `fsm-codegen-c` switch-strategy emitter is rewritten.
This is the largest implementation change in this reconciliation.

---

### B-11 (3.9) — Table-Driven Dispatch Returns Too Early In Parallel Regions

**Problem.** Doc 11 §9's table-driven dispatch does a linear scan with early `return`
after the first match. In a parallel state with two active regions, the second region
never gets a chance to react.

**Resolution.** The table-driven dispatch is split into two phases:

1. **Collection.** Walk the table once, collecting at most **one** matching transition
   per region (innermost active state in each region, ancestor walk per B-10). Resolve
   priority/document order at this point. The result is a set `selected` of size ≤
   number-of-active-regions.
2. **Execution.** Execute every transition in `selected` in region-declaration order
   (deterministic).

Normative C99 template (replaces the early-return version in Doc 11 §9):

```c
void Motor_dispatch(Motor_t *m, const Motor_Event_t *ev) {
    const Motor_TransRow_t *selected[MOTOR_MAX_PARALLEL_REGIONS];
    uint8_t selected_count = 0;

    for (uint8_t region_idx = 0; region_idx < Motor_active_region_count(m); region_idx++) {
        Motor_StateId_t s = Motor_active_leaf_of_region(m, region_idx);
        const Motor_TransRow_t *best = NULL;
        while (s != MOTOR_STATE__ROOT_SENTINEL) {
            for (uint16_t i = 0; i < MOTOR_TRANS_TABLE_SIZE; i++) {
                const Motor_TransRow_t *row = &Motor_trans_table[i];
                if (row->source != s) continue;
                if (row->trigger != ev->id) continue;
                if (row->guard && !row->guard(m, ev)) continue;
                if (!best || row->priority < best->priority) best = row;
            }
            if (best) break;
            s = Motor_parent_table[s];
        }
        if (best) selected[selected_count++] = best;
    }

    for (uint8_t i = 0; i < selected_count; i++) {
        Motor_execute_transition(m, selected[i], ev);
    }
}
```

Sort order for `Motor_trans_table[]`: by `source` (numerically), then by `priority`
(ascending), then by `document_order` (ascending). This is the order an `fsm-codegen-c`
emitter MUST use; B-12 below makes it normative.

**Rationale.** Same ancestor-walk reasoning as B-10. Splitting collection from execution
is also the only correct way to handle the case where one transition's exit set would
disrupt another transition's source — by the time you execute, you have the full
intended set in hand. (This mirrors the formal `select_transitions` algorithm in Doc 08
§4.1.)

**Affected docs.** Doc 11 §9, §13, Doc 08 §4.1 (cross-reference the same algorithm).

**Type.** Doc + implementation.

---

### B-12 (3.10) — C++17 STL Profile Heap-Allocates (DEFERRED)

**Problem.** Doc 12 specifies a C++17 STL profile using `std::queue<Event>` which is
backed by `std::deque`, which heap-allocates. This violates Doc 02 G2 (no dynamic
allocation in generated runtime).

**Resolution.** **Doc 12 is deferred entirely to v1.1** (see Section 6). The decision
for the future is **recorded here**: when Doc 12 is revived, the STL profile MUST use a
fixed-capacity circular buffer (e.g. `std::array<Event, N>` indexed manually, or
`etl::queue` from the embedded-template-library). The "STL" profile is then named "STL
containers minus dynamic memory." The no-STL profile uses the same C99 circular buffer
emitted by `fsm-codegen-c`.

**Rationale.** v1.0 ships C99 only; deferring removes the conflict. Recording the v1.1
resolution prevents the same drift from re-emerging.

**Affected docs.** Doc 12 (mark deferred), Doc 02 G2 (already correct).

**Type.** Deferred.

---

### B-13 (3.17) — `after 0 ms` Never Fires

**Problem.** The Doc 11 §11 timer template checks `_timer_remaining_ms > 0`. For
`after 0 ms`, this is false on the very first `Motor_tick` call, so the timer is
silently a no-op. The Doc 10 `FSM-W0400` already warns about zero-duration timers but
does not prohibit them, so the spec leaves a documented behaviour that the implementation
contradicts.

**Resolution.** **Zero-duration `after` and `every` timers are PROHIBITED in v1.0.**
A new diagnostic, **`FSM-E0410` "Timer duration must be greater than zero,"** is added
to Doc 10. The compiler MUST reject `after 0 ms` / `every 0 ms` / `after CONST ms` where
`CONST == 0` at compile time. `FSM-W0400` is retired (renumbered as Deprecated; the
condition is now an error).

**Rationale.** The semantics of "fire immediately on entry" already has a documented
spelling: an unconditional entry-action that `raise`s the target event, or simply an
immediate `done` completion. Allowing `after 0 ms` would require either (a) a
runtime-detected fire-now branch in `Motor_tick`, (b) changing `>` to `>=` and accepting
that the timer fires before any external event can be processed (which mixes the
internal/external queue invariants), or (c) leaving the documented bug. Prohibition is
the simplest correct answer and matches Doc 02 G1 ("No undefined behaviour"). The
"prohibit + diagnostic" path was the report's recommended option.

**Affected docs.** Doc 10 (add E0410, deprecate W0400), Doc 11 §11 (keep `> 0` check;
add note that compiler rejects 0-duration timers up-front), Doc 04 §9.1/9.2 (note that
literal `0` and `const NAME = 0` are rejected), Doc 15 (add E0410 conformance test).

**Type.** Doc + implementation — analyzer rejects the construct; codegen and runtime
do not change.

---

### B-14 (3.18) — History Without Default Is Undefined Behaviour

**Problem.** Doc 11 §14 (shallow) and §21 (deep) describe restoring `m->_history_X`,
but if the composite has never been exited and no `default ->` is declared, the field
holds `0` / `ROOT` and the behaviour is "implementation-defined." This violates
Doc 02 G1.

**Resolution.** **`default ->` is MANDATORY on every history pseudo-state** in v1.0.
The analyzer MUST emit **`FSM-E0109`** ("History default target missing") — currently
defined in Doc 10 as a related-but-different code ("History default references
non-existent state"). I keep E0109's existing meaning **and** allocate a new
**`FSM-E0111` "History pseudo-state has no `default ->` declaration."** Both are
fatal compile-time errors.

A history pseudo-state declaration without a default is a hard compile error, full stop.
This makes the `restore == MOTOR_STATE_ROOT ? default : restore` branch in Doc 11 §14
and §21 reachable only on first entry, never on undefined first-entry-without-default.

**Rationale.** UML 2.5.1 §14.2.3.4 says history default is optional in the metamodel,
but the metamodel says the runtime behaviour is undefined without it. Embedded targets
cannot accept undefined behaviour (Doc 02 G1), so we tighten the language. This is the
same trade-off TLA+ and SCXML implementations make. The cost is minimal: every existing
example in Doc 04 / Doc 08 / Doc 11 already provides a `default ->`, and adding one is
a one-line user fix.

**Affected docs.** Doc 04 §7.1, §7.2 (grammar — `default ->` becomes required, not
optional), Doc 08 §8.1, §8.2 (semantics simplified — no UB branch), Doc 09 (HistoryObject
schema: `defaultTarget` becomes REQUIRED, not nullable), Doc 10 (add E0111), Doc 11 §14,
§21 (annotate "default is always set"), Doc 15 / Doc 24 (E0111 conformance test).

**Type.** Doc + implementation — parser still accepts the grammar (so it can produce a
helpful error), analyzer emits E0111, codegen is unchanged.

---

## 3. Inconsistency Resolutions (28 items, table form)

For each pair "Doc A says X, Doc B says Y," the table names the authoritative side and
the side to update.

| # | Topic | Doc A | Doc B | Authoritative | Action |
|---|---|---|---|---|---|
| I-01 | E0001-E0005 codes (lexer vs analyzer) | Doc 02 §13.1 | Doc 10 §3 | **Doc 10** | Doc 02 §13: delete table; cross-ref Doc 10 (per B-01) |
| I-02 | E0100-E0102 codes (semantic vs symbol resolution) | Doc 02 §13.2 | Doc 10 §5 | **Doc 10** | Same as I-01 |
| I-03 | Duplicate `@id` code | Doc 04 §10 (E0750) | Doc 04 §14.2 (E0025) + Doc 10 (E0025) | **Doc 10 / §14.2** | Doc 04 §10: change to E0025 |
| I-04 | Opaque-field-in-guard code | Doc 04 §3 (E0303) | Doc 10 (E0303 = join source) | **Doc 10** | Doc 04 §3: change to new E0210 (per B-01) |
| I-05 | Feature-not-enabled code | Doc 04 §15 (E0600) | Doc 10 §7 (E0600 = parallel no-initial) | **Doc 10** | Doc 04: change to new E0610 (per B-01) |
| I-06 | E0304 vs E0600 duplicate meaning | Doc 10 §7 (both define "parallel region has no initial") | — | **E0600** | Doc 10: retire E0304 (per B-01) |
| I-07 | Sim method naming (dot vs slash) | Doc 03 §X (`sim.init`) | Doc 13 (`sim/init`) | **Doc 13 (slash)** | Doc 03: delete simulator section (per B-02) |
| I-08 | Sim method set | Doc 03 (8 RPCs) | Doc 13 (~25 RPCs) | **Doc 13** | Doc 03 deletion (per B-02); Doc 13 itself deferred to v1.1 |
| I-09 | Sim response schemas | Doc 03 (`{active}`) | Doc 13 (`{activeStates, traceId}`) | **Doc 13** | Same |
| I-10 | Sim notifications (8 vs 4) | Doc 03 | Doc 13 | **Doc 13** | Same |
| I-11 | Trace record schema (3 versions) | Doc 03, Doc 13, Doc 24 | — | **Doc 13 StepRecord §11** | Doc 03 + Doc 24: replace with Doc 13 shape (per B-02) |
| I-12 | Keyword list (Doc 04 vs Doc 21) | Doc 04 §1.5 | Doc 21 §X | **Doc 04 §1.5** | Doc 21 deferred; on revival, regenerate keyword scopes from Doc 04 (per B-03) |
| I-13 | Keyword list (Doc 04 vs Doc 14) | Doc 04 §1.5 | Doc 14 | **Doc 04 §1.5** | Doc 14 deferred; on revival, generate completions from Doc 04 |
| I-14 | Formatter keywords vs grammar | Doc 04 §1.5 | Doc 19 (uses `composite`, `parallel`, `internal on`, `history shallow` etc.) | **Doc 04 §1.5** | Doc 19: rewrite with `state`, `state ... parallel`, `internal on EVENT:`, `shallow_history` (per Section 8) |
| I-15 | IrVisitor `m.states` vs IR `m.root.regions[0].states` | Doc 20 §12.8 | Doc 09 §3 | **Doc 09** | Doc 20: rewrite the visitor walk to descend `machine.root` (per Section 8); add `visit_extern` |
| I-16 | Doc 02 G6 inline-computation "out of scope" vs Doc 02 §9 defining inline language | Doc 02 §18 | Doc 02 §9 | **Doc 02 §9 (inline IS in scope)** | Doc 02 §18: rewrite "no general-purpose expression language" to clarify "the constrained action expression language of §9 IS supported; arbitrary user-defined computation outside it is not" |
| I-17 | Exit codes (Doc 03 has 0–2, Doc 18 has 0–4) | Doc 03 | Doc 18 §3 | **Doc 18 §3** | Doc 03 exit-codes bullet deleted (and the whole simulator section per B-02); Doc 03 retained sections that mention exit codes must cross-ref Doc 18 |
| I-18 | `fsm compile` semantics (Doc 03: produces IR; Doc 18: alias for `fsm generate`) | Doc 03 | Doc 18 §5.4 | **Doc 18 §5.4** | Doc 03: delete or rewrite; Doc 18's "alias for `fsm generate`" wins. To produce IR, the canonical command is `fsm ir`. |
| I-19 | Transition selection algorithm (Doc 04 §11 vs Doc 08 §4.1) | Doc 04 §11 | Doc 08 §4.1 | **Doc 08 §4.1 (normative)** | Doc 04 §11: re-tag as informational with explicit pointer to Doc 08 |
| I-20 | Per-region defer (Doc 08 §10.5 specifies it; Doc 11 §22 implements flat per-machine) | Doc 08 §10.5 | Doc 11 §22 | **Doc 08 §10.5** | Doc 11 §22: rewrite with per-region defer bitmask array; codegen emits one bitmask per region (per Section 8) |
| I-21 | LCA(S,S) semantics for self-transitions | Doc 08 §5.1 | UML 2.5.1 | **B-09 in this doc** | Doc 08 §5.1: reword; consume `transition.kind` (per B-09) |
| I-22 | Completion-guard prohibition | Doc 08 §4.4 (forbid) | Doc 02 G4 / Doc 04 §8.4 (allow) | **Allow (B-07)** | Doc 08 §4.4: delete prohibition |
| I-23 | Switch dispatch leaf-only vs ancestor walk | Doc 11 §8 | Doc 08 §4.1 (ancestor-aware) | **Doc 08 §4.1** | Doc 11 §8: rewrite (per B-10) |
| I-24 | Table dispatch early-return | Doc 11 §9 | Doc 08 §4.1 | **Doc 08 §4.1** | Doc 11 §9: rewrite (per B-11) |
| I-25 | Shallow history stores leaf vs direct child | Doc 11 §14 (`m->_state` = leaf) | Doc 08 §8.1 ("direct child") | **Doc 08 §8.1** | Doc 11 §14: rewrite (per B-09 / and Section 8) |
| I-26 | Deep history single-StateId for parallel sub-states | Doc 11 §21 | UML / Doc 08 §8.2 (one per region) | **Per-region storage** | Doc 11 §21: rewrite with per-region history slots when parent is parallel (per Section 8) |
| I-27 | VS Code command IDs (3 docs disagree) | Doc 03, Doc 05, Doc 22 | — | **Doc 22 (deferred)** | All three deferred (per B-04) |
| I-28 | `fsmLang.lspPath` vs `fsmLang.compilerPath` | Doc 03 | Doc 22 | **Doc 22 (deferred)** | Deferred (per B-04) |

---

## 4. Single-Source-of-Truth Registry

| Concept | Authoritative document | Cross-references |
|---|---|---|
| **Diagnostic codes** (all FSM-E / FSM-W / FSM-I / FSM-H) | **Doc 10** | Docs 02, 04, 11, 15, 18, 24 |
| **Keyword list** (token set) | **Doc 04 §1.5** | Docs 06, 19 (Docs 14, 21 → v1.1) |
| **Lexical grammar** (literals, identifiers, comments, operators) | **Doc 04 §1.1–§1.6** | Doc 19 (formatter), Doc 06 (style) |
| **Surface grammar** (EBNF) | **Doc 04 §§2–10, §12** | Doc 19 |
| **Operator precedence** | **Doc 04 §8.7.1 (Pratt table)** | Doc 09 (expression shape), Doc 11 (codegen) |
| **Formal execution semantics** | **Doc 08** | Doc 04 §11 (informational), Doc 11 (codegen must match), Doc 13 (sim must match) |
| **IR schema** | **Doc 09 + `schema/ir/1.0.0/model.json`** | All IR consumers — codegen-c, analyzer, formatter, simulator |
| **CST/AST JSON schema** (parse-only output) | **Doc 09 Appendix (new — see Section 5)** | Doc 18 `fsm parse --emit-cst` |
| **C99 codegen contract** | **Doc 11** | Doc 02 §15.1, Doc 15 (conformance), Doc 16 (HAL), Doc 17 (assembly) |
| **CLI surface (subcommands, flags, stdout/stderr)** | **Doc 18** | Doc 03 (deleted/rewritten), Doc 23 (onboarding) |
| **Exit codes** | **Doc 18 §3** | Doc 03 (cross-ref only) |
| **Configuration schema (`fsm.toml`)** | **Doc 18 §6** | (Doc 22 deferred) |
| **Simulator protocol (RPC, transport, methods)** | **Doc 13** (deferred) | — |
| **Step / trace record format** | **Doc 13 §11 `StepRecord`** | Doc 15 `.trace` files, Doc 24 trace tests |
| **HAL** | **Doc 16** | Doc 11 §X (timer integration), Doc 17 |
| **Assembly integration** | **Doc 17** | Doc 11 §17 (heap-free) |
| **Style** | **Doc 06** | Doc 19 |
| **Formatter** | **Doc 19** | Doc 04 §1.5 (keywords), Doc 06 (style) |
| **Conformance test categories** | **Doc 15** | Doc 24 (concrete tests), Doc 10 (one test per code) |
| **VS Code commands / settings** | (Doc 22 — deferred) | — |
| **LSP capabilities / methods** | (Doc 14 — deferred) | — |
| **TextMate grammar** | (Doc 21 — deferred) | — |

Rule of thumb: when implementing a feature, read the authoritative doc; when an
ambiguity arises, this Section 4 says where the answer lives.

---

## 5. Gap Triage — Must-Fix For v1.0 vs Deferred

### 5.1 GAPs that MUST be fixed for v1.0

| # | Gap | Why must-fix | Owner |
|---|---|---|---|
| **G-01** (4.4) | Memory budget formula | Doc 02 G2 promises "computable at compile time"; without a formula it's a marketing claim. | Add **Doc 11 §28 Memory Budget** specifying `sizeof(M_t) = sizeof(StateId) + sizeof(EventId) + sizeof(Queue<N>) + Σ history_slots + Σ timer_slots + Σ context_fields + per-region defer bitmask`. The CLI MUST grow `fsm generate --report-memory` that prints the computed total. |
| **G-02** (4.5) | Security section | The DSL has `import "path"` (path traversal) and `opaque "C_type"` (code injection). v1.0 ships compile-time tooling; if it's installed in CI on shared infrastructure it MUST be safe. | Add **Doc 18 §10 Security** with: (a) import path sanitization — paths MUST resolve under the workspace root, `..` segments are rejected, symlinks resolved before the check; (b) opaque type validation — content matched against `^[A-Za-z_][A-Za-z0-9_ *]*$`, semicolons / parens / brackets / strings rejected; (c) parser input size limit — default 8 MB, configurable via `fsmLang.parser.maxFileBytes`; (d) feature-flagged identifier limits — max 1024 chars; max nesting depth 32. Simulator WebSocket security is moot for v1.0 (no server). |
| **G-03** (4.1) | Build system | A separate foundation agent is producing `Cargo.toml` + `rust-toolchain.toml` in parallel. | Foundation agent (out of scope for this doc, but recorded as a dependency). |
| **G-04** (2.12) | Periodic timer codegen | `every N ms` is in v1.0 scope; no codegen template exists. | Add **Doc 11 §11a Periodic Timer Codegen** — same pattern as `after`, except the `Motor_tick` block re-arms `_timer_remaining_ms = N` immediately after firing instead of leaving it 0. |
| **G-05** (2.13) | Local transition codegen | The `~>` operator's codegen is undefined. | Add **Doc 11 §23a Local Transition Codegen** consuming `transition.kind = "local"` from the IR (B-06): no exit / no entry of source, only inline action execution. |
| **G-06** (2.14) | `send EVENT to M` codegen for C99 | The DSL supports it; codegen has no spec. | Add **Doc 11 §29 Send-to-Other-Machine** — codegen emits `OtherMachine_dispatch(other_machine_ptr, &ev)`; v1.0 requires the user to wire `other_machine_ptr` in user code. Multi-machine routing tables are v1.1. |
| **G-07** (2.15) | Table sort order unspecified | The dispatch in B-11 requires a specific order; current spec is silent. | Doc 11 §9: "The `Motor_trans_table[]` MUST be sorted by (`source`, `priority`, `document_order`), ascending. The codegen emitter is responsible for this sort." |
| **G-08** (2.16) | Defer bitmask caps at 32 events | `uint32_t` overflows at 33 events. | Doc 11 §22: codegen MUST pick `uint32_t` / `uint64_t` / `uint8_t[ceil(N/8)]` based on declared event count; if N > 256, raise **FSM-E0903** ("too many event types for defer bitmask — reduce or split machine"). |
| **G-09** (2.24) | No CST/AST JSON schema for `fsm parse --emit-cst` | Doc 18 ships the command; users can't consume the output. | Add **Doc 09 Appendix A — CST/AST Schema** — a stripped-down JSON shape with only source spans + node kinds + raw token text (no semantic info). The Rust struct lives in `fsm-parser::cst`. |
| **G-10** (6.1) | Only 11% of diagnostic codes are tested | Doc 15 mandates one test per code. ~48 codes are untested. | Doc 24: add one positive (compiles cleanly) + one negative (emits the code with the right span) test per code in Doc 10. This is mechanical work and scopes to a single implementer wave. |
| **G-11** (6.2–6.5) | Four broken test cases | Five-minute fix; currently the tests pass against the wrong codes. | Doc 24: edit `PARSE-NEG-001` (E0020 → E0107), `PARSE-NEG-005` (E0024 → E0108), `PARSE-NEG-008` (E0102 → E0104), `PARSE-NEG-010` (E0200 → E0201). |
| **G-12** (2.21 / 2.22) | Formatter spec uses non-grammar syntax | Formatter is in v1.0. Currently the spec describes a different language. | See B-13 (this doc) and Section 8 doc-update plan. The formatter implementation works off Doc 04's grammar; Doc 19 is rewritten to match. |

### 5.2 GAPs deferred to v1.1+

| # | Gap | Why defer |
|---|---|---|
| **G-D1** (2.17 / 2.18) | C++17 codegen gaps | Doc 12 deferred entirely (per B-12). |
| **G-D2** (2.19 / 2.20) | LSP undocumented capabilities | LSP deferred (per B-04). |
| **G-D3** (2.23) | Simulator reconnect | No WebSocket server in v1.0 (per B-02). |
| **G-D4** (2.26) | VS Code snippets | VS Code deferred (per B-04). |
| **G-D5** (2.27) | WebView message protocol | Web UI deferred. |
| **G-D6** (4.2) | CI/CD multi-platform | Build agent owns Phase-1; deep cross-compile / Marketplace publishing is Phase-2. |
| **G-D7** (4.3) | DSL `fsm upgrade` migration tool | v1.0 is "v1"; no migration needed yet. Policy text (G-D8) is enough. |
| **G-D8** (4.6) | Plugin API | The IR is already stable enough for out-of-tree consumers to read; a documented plugin/loader is a polish item. |
| **G-D9** (4.7, 4.8, 4.9, 4.10, 4.11) | IR migration tool, performance limits, i18n, `fsm doc` output, generated-code license | All quality-of-life items; documented as Weaknesses, not Blockers. |

---

## 6. Decisions Deferred to v1.1+

This list is the authoritative scope-cut. Future agents reading the spec corpus
should treat any text in these areas as **non-binding draft**.

| Item | What is deferred | Reason |
|---|---|---|
| **D-01** | Doc 12 C++17 codegen | C99-only v1.0 scope. STL profile heap issue (B-12) recorded for v1.1. |
| **D-02** | Doc 13 simulator WebSocket protocol | No server in v1.0. Doc 13 marked Reserved; resurrects in v1.1 as authoritative (slash-form methods). |
| **D-03** | Doc 14 LSP capability spec | CLI-only v1.0. When revived, keyword list comes from Doc 04 §1.5. |
| **D-04** | Doc 21 TextMate grammar | Editor support deferred; when published, generated from Doc 04 §1.5. |
| **D-05** | Doc 22 VS Code extension manifest | No extension in v1.0. Doc 22 designated future authority over Docs 03 / 05 conflicts. |
| **D-06** | Doc 05 UI specification (the UI-shaped parts) | The non-UI parts of Doc 05 (information architecture, command surface) may still inform CLI; the rendered-UI parts are deferred. |
| **D-07** | Doc 03 Web IDE / share URL | XSS risk; no Web IDE in v1.0. |
| **D-08** | Multi-machine `send` routing tables | v1.0 requires the user to pass the target machine pointer explicitly. |
| **D-09** | DSL version migration tool (`fsm upgrade`) | No older versions exist yet. |
| **D-10** | Out-of-tree generator plugin API | The `IrVisitor` trait is usable today as the de facto API. |
| **D-11** | Cross-file rename (LSP-driven) | LSP deferred. |
| **D-12** | Generated-code license declaration | Polish — to be addressed before public release announcement. |
| **D-13** | i18n / `docs/RU/` translation strategy | Engineering-internal docs are English-only for v1.0. |
| **D-14** | Coverage metrics (line / branch / MC/DC) | Add `tarpaulin` in v1.1; v1.0 ships with the test count Doc 15 mandates. |

---

## 7. Implementation Guidance (For Phase-1 Waves)

This section tells implementer waves **which crate / file / struct** carries each
resolved blocker. Pseudocode is provided where the answer is non-obvious.

### 7.1 B-01 Diagnostic codes
- **Crate.** `fsm-diagnostics`.
- **File.** `crates/fsm-diagnostics/src/codes.rs`.
- **Encode.** A single Rust enum `DiagnosticCode { E0001, …, E0610, E0210, W0604, E0110, H0006, … }` keyed off Doc 10. Generate the string form (`"FSM-E0610"`) via `Display`. Keep a static `CODE_TABLE: &[(DiagnosticCode, Severity, &'static str)]`. **Retire** E0301, E0304 from the live enum (move them to a `Deprecated` submodule that the suppression-parser still recognizes per Doc 10 §14).
- **Test.** One unit test per variant asserting code → severity + display.

### 7.2 B-02 Simulator
- **Crate.** `fsm-simulator` (interpreter only).
- **File.** `crates/fsm-simulator/src/interpreter.rs` (the core `Interpreter` API).
- **Encode.** No WebSocket layer; no `fsm-rpc-server` crate. The `fsm simulate` CLI in `crates/fsm-cli/src/cmd/simulate.rs` instantiates `Interpreter`, prints active states and traces in `StepRecord` JSON, exits. The `fsm test` runner uses the same interpreter to execute `.trace` files.

### 7.3 B-05 Pratt parser
- **Crate.** `fsm-parser`.
- **File.** `crates/fsm-parser/src/expr.rs`.
- **Encode.** A `parse_expr(min_bp: u8)` function with the binding-power table from
  Doc 04 §8.7.1. **Do not** implement EBNF recursive descent for expressions. Guard
  expressions reuse the same function with a `GuardContext` flag that rejects non-pure
  externs.

### 7.4 B-06 IR additions
- **Crate.** `fsm-ir`.
- **Files.** `crates/fsm-ir/src/model.rs` (add `ConstDecl`, `ImportDecl`, `QueueConfig`,
  `TargetConfig`, `TransitionKind`, `CastExpr`, `FloatLit`, `EnumVariantLit`).
  `schema/ir/1.0.0/model.json` (update JSON Schema accordingly).
- **Encode.**
  ```rust
  pub enum TransitionKind { External, Local, Internal, Completion }

  pub struct TransitionObject {
      pub id: String,
      pub stable_id: String,
      pub source: StateId,
      pub target: StateId,
      pub trigger: Option<Trigger>,
      pub guard: Option<GuardExpr>,
      pub actions: Vec<Statement>,
      pub priority: u16,
      pub kind: TransitionKind,           // NEW
      #[deprecated(note = "derive from `kind`")]
      pub internal: bool,                  // KEEP for one cycle
      pub loc: SourceLocation,
  }
  ```
- **Test.** Round-trip property test: parse → IR → JSON → parse-IR → compare.

### 7.5 B-07 Allow completion guards
- **Crate.** `fsm-analyzer`.
- **File.** `crates/fsm-analyzer/src/checks/completion.rs`.
- **Encode.** Delete the check that emits E0301. Add the W0301 lint instead (or leave
  unguarded since W0101 dead-transition already covers the failure modes).

### 7.6 B-08 Parallel-state completion
- **Crate.** `fsm-codegen-c`, `fsm-simulator`.
- **Files.** `crates/fsm-codegen-c/src/emit/completion.rs`,
  `crates/fsm-simulator/src/runtime/completion.rs`.
- **Encode.** When emitting the `Motor_handle_completion` C99 helper or running the
  interpreter step, check the parent state's `kind`. For `Parallel`, walk every region
  and verify each region's active leaf is `Final`. For `Composite`, the single-region
  case is already correct. Pseudocode:
  ```rust
  fn maybe_enqueue_completion(m: &Machine, just_entered: StateId) {
      let s = m.parent_of(just_entered);
      match s.kind() {
          Composite if s.single_region.active_leaf().is_final() =>
              m.queue.push_front(Completion(s.id)),
          Parallel if s.regions().all(|r| r.active_leaf().is_final()) =>
              m.queue.push_front(Completion(s.id)),
          _ => (),
      }
  }
  ```

### 7.7 B-09 Self-transition LCA
- **Crate.** `fsm-analyzer`.
- **File.** `crates/fsm-analyzer/src/lca.rs`.
- **Encode.**
  ```rust
  fn effective_lca(t: &TransitionObject, ir: &Ir) -> StateId {
      let base = lca_inclusive(t.source, t.target, ir);
      if t.source == t.target && matches!(t.kind, TransitionKind::External) {
          ir.parent_of(t.source)
      } else {
          base
      }
  }
  ```
  Codegen-c consumes `effective_lca` when emitting exit/entry sequences.

### 7.8 B-10 / B-11 Hierarchical dispatch
- **Crate.** `fsm-codegen-c`.
- **Files.** `crates/fsm-codegen-c/src/emit/dispatch_switch.rs` (B-10),
  `crates/fsm-codegen-c/src/emit/dispatch_table.rs` (B-11).
- **Encode.** Emit a static `Motor_parent_table[]: const StateId[]` populated by
  walking the IR's `MachineObject.root` tree. For switch strategy, emit a
  `Motor_try_transitions_in_state` per-state function (one switch over events) and a
  `Motor_dispatch` outer ancestor-walk loop. For table strategy, emit the
  collect-then-execute two-phase function shown in B-11.
- **Test.** Conformance suite gains `CODEGEN-POS-HIER-001`: composite parent declares
  `FAULT -> Error`; nested leaf machines must transition correctly.

### 7.9 B-13 Reject `after 0 ms`
- **Crate.** `fsm-analyzer`.
- **File.** `crates/fsm-analyzer/src/checks/timer.rs`.
- **Encode.** When walking timers, resolve any `const` expression in the duration; if
  the resolved value is `0`, emit `FSM-E0410` with the span of the literal/constant
  reference.

### 7.10 B-14 Mandatory history default
- **Crate.** `fsm-analyzer`.
- **File.** `crates/fsm-analyzer/src/checks/history.rs`.
- **Encode.** For every `History` pseudo-state in the IR, require `default_target`
  to be `Some(StateId)`. Otherwise emit `FSM-E0111`. The parser may still accept the
  defaultless form so the analyzer can produce a span-accurate error.

### 7.11 G-01 Memory budget
- **Crate.** `fsm-codegen-c` (computes), `fsm-cli` (prints).
- **File.** `crates/fsm-codegen-c/src/budget.rs`, surfaced through
  `fsm generate --report-memory`.

### 7.12 G-02 Security
- **Crates.** `fsm-parser` (path sanitizer for `import`, opaque-type validator).
- **Files.** `crates/fsm-parser/src/import_resolver.rs`,
  `crates/fsm-parser/src/opaque_type_validator.rs`.
- **Encode.** Resolve import paths via `Path::canonicalize` and assert the result starts
  with the workspace root. Validate opaque type strings against the regex shown in G-02
  before emitting them into C code.

---

## 8. Doc-Update Plan (Patches To Existing 24 Docs)

A future agent executes this list mechanically. Patches are listed in (Doc, section)
order. Each item names the doc, the section, the action, and (where helpful) a one-line
snippet of the replacement text.

### Doc 02 — Language Requirements
- **§9.** Add a closing note: *"This section defines the action expression language. Its
  formal grammar lives in Doc 04 §8.7; its semantics in Doc 08 §6.2."*
- **§13** (entire section, 13.1–13.6). **DELETE** the tables. Replace with:
  *"All diagnostic codes are defined in Doc 10 — Diagnostic Code Catalog. Implementations
  MUST emit codes per Doc 10. This document does not redefine codes."*
- **§18.** Rewrite the "Inline computation: out of scope" bullet to: *"A general-purpose
  user-defined expression language is out of scope. The constrained action expression
  language and guard expression language defined in Doc 04 §8.2 and §8.7 ARE in scope."*

### Doc 03 — Infrastructure Requirements
- Entire **simulator protocol section** (currently a long subsection with `sim.*`
  methods). **DELETE.** Replace with a one-paragraph cross-reference: *"The simulator
  protocol is specified in Doc 13 (Reserved — v1.1). In v1.0 the simulator is an
  in-process library invoked by `fsm simulate` and the test runner; no network protocol
  is exposed."*
- **Exit codes line** (3 codes). **DELETE.** Replace with cross-reference to Doc 18 §3.
- **`fsm compile`** description. **DELETE** any wording that says `fsm compile` produces
  IR. Cross-reference Doc 18 §5.4 ("alias for `fsm generate`"); for IR use `fsm ir`.

### Doc 04 — DSL Specification
- **§1.5.** Add a one-line top note: *"This is the master keyword list. Other documents
  (Doc 19 formatter, future Docs 14/21) MUST cross-reference this section and MUST NOT
  redefine the list."*
- **§3 (Type Casting).** Change the FSM-E0303 reference for "opaque field in guard" to
  **FSM-E0210** (new code; see Doc 10 update).
- **§8.7.** Add normative banner at the top: *"The EBNF in this section is informational.
  The normative source of expression grammar is the operator precedence table in §8.7.1.
  Implementations MUST use precedence climbing / Pratt parsing."*
- **§8.7 EBNF.** Replace the left-recursive `expr = expr , bin_op , expr` block with the
  layered non-left-recursive form shown in B-05.
- **§7.1, §7.2 (Shallow / Deep History).** Make `default ->` REQUIRED in the EBNF;
  remove the `[ ... ]` optionality markers around the default clause. Note: *"FSM-E0111
  is emitted if absent."*
- **§9.1, §9.2 (Timer declarations).** Note: *"`const_expr` MUST evaluate to a positive
  integer at compile time. Zero is rejected with FSM-E0410."*
- **§10.** Change `Duplicate IDs: FSM-E0750` → `Duplicate IDs: FSM-E0025`.
- **§11.** Add an opening banner: *"This algorithm is informational. The normative
  transition selection semantics live in Doc 08 §4. Where this section and Doc 08
  disagree, Doc 08 wins."*
- **§14.2.** No change (already correct).
- **§15.** Change `FSM-E0600` → `FSM-E0610` in the "feature not enabled" sentence.
  Remove the "[Future — Post-v1.0]" header — submachines ARE in v1.0 scope per locked
  decisions. Re-tag as "§15 — Submachine Syntax."

### Doc 05 — UI Specification
- Mark the entire document **"Status: Reserved — v1.1 surface."** Implementer waves for
  v1.0 do not consume this doc.

### Doc 08 — Formal Execution Semantics
- **§4.4.** Delete the sentence "Completion transitions MUST NOT have a guard (FSM-E0301
  if present)." Replace with: *"Completion transitions MAY carry a guard; standard
  transition-selection rules apply."*
- **§5.1.** Reword the inclusive-ancestor sentence: *"For LCA computation, a state is
  considered an ancestor of itself **except** when computing the exit/entry set of an
  EXTERNAL self-transition (`source == target` and `kind == external`), in which case
  the effective LCA is `source.parent`. See Doc 04 §8.1 / §8.3 for the syntactic
  distinction between external (`->`) and local (`~>`) transitions."*
- **§9.1.** Rewrite to the all-regions-final rule shown in B-08.

### Doc 09 — Canonical IR Schema
- **§3 (MachineObject).** Add fields `consts: ConstDecl[]`, `queue: QueueConfig`,
  `target: TargetConfig`. Document-level fields (file scope) on `IrDocument`:
  `features: string[]`, `imports: ImportDecl[]`.
- **§4.8 (History).** Make `defaultTarget` REQUIRED, not nullable.
- **§6 (Transition).** Add `kind` field with enum `{external, local, internal, completion}`;
  mark `internal: boolean` as deprecated alias.
- **§8 (Statement Schema).** Already covers `raise` / `send` / `defer`.
- **§9 (Expression).** Add `cast` discriminator variant, `float` literal kind,
  `enum_variant` literal kind.
- **NEW §X (Appendix A — CST/AST Schema).** Document the JSON shape emitted by
  `fsm parse --emit-cst`.

### Doc 10 — Diagnostic Code Catalog
- Add **FSM-W0604** — Possible precision loss in float cast.
- Add **FSM-E0110** — Local transition target is not a proper descendant of source.
- Add **FSM-H0006** — Stable `@id` placed before doc comment (suggest swap).
- Add **FSM-E0610** — Construct used without required `feature` flag.
- Add **FSM-E0210** — Opaque field used in field-comparison guard.
- Add **FSM-E0410** — Timer duration must be greater than zero.
- Add **FSM-E0111** — History pseudo-state has no `default ->` declaration.
- Add **FSM-E0903** — Too many event types for defer bitmask (>256 events).
- Mark **FSM-E0301** (Guard on completion transition) **Deprecated — no longer emitted.**
- Mark **FSM-E0304** (Parallel region has no `initial` — duplicate of E0600) **Deprecated.**
- Mark **FSM-W0400** (Timer duration is zero) **Deprecated** (now E0410, fatal).

### Doc 11 — Codegen C99
- **§8 (switch strategy).** Rewrite per B-10. Include the parent-table pattern and the
  `try_transitions_in_state` helper.
- **§9 (table strategy).** Rewrite per B-11. Include the two-phase collect-execute
  pattern.
- **§11 (timer integration).** Keep `> 0` check; add a sentence: *"Zero-duration timers
  are rejected at compile time (FSM-E0410)."*
- **§13 (parallel region dispatch).** Delete the early-fall-through example; cross-ref
  §9 two-phase dispatch.
- **§14 (shallow history).** Storage stores **the direct child of the composite**, not
  `m->_state` directly. Generate `Motor_get_direct_child_of_X(m)` helper at codegen time.
- **§15 (completion).** Change the `_completion_depth` from `static uint8_t` (file scope)
  to a per-instance field `_completion_depth: uint8_t` in `Motor_t`.
- **§21 (deep history).** For history pseudo-states whose parent is a Parallel state,
  emit one storage slot **per region**, not a single `StateId`.
- **§22 (deferred events).** Rewrite with per-region defer bitmask arrays for machines
  containing parallel states (per Doc 08 §10.5).
- **§26 (codegen error references).** Re-verify every diagnostic code against Doc 10
  after B-01 reconciliation.
- **NEW §28 — Memory budget.** Per G-01.
- **NEW §29 — Send-event-to-machine.** Per G-06.

### Doc 12 — Codegen C++17
- Mark the whole document **"Status: Deferred to v1.1."**

### Doc 13 — Simulator WebSocket Protocol
- Mark the whole document **"Status: Reserved — v1.1 surface."** The text remains as
  the authoritative future spec; no v1.0 implementation. Add a top-banner explicit
  cross-reference: *"In v1.0 the simulator runs in-process; this protocol activates in
  v1.1."*

### Doc 14 — LSP Capability Specification
- Mark **"Status: Deferred to v1.1."**

### Doc 15 — Conformance Test Suite
- **§7 (Codegen tests).** Add the six missing items listed in 6.6: table-strategy,
  composite-state LCA exit/entry order, history store/restore (shallow + deep), parallel
  both-region dispatch, queue overflow assert, ARM cross-compilation. The C++17
  test category is **removed** for v1.0.
- **§11.** Re-affirm "one test per diagnostic code." Update the count expectation.

### Doc 18 — CLI Specification
- **NEW §10 — Security.** Per G-02.
- **§3 (Exit codes).** No change — already canonical per I-17.
- **§5.4 (`fsm compile`).** Already correct (alias for `fsm generate`).
- **§5.7 (`fsm simulate`).** Add: *"v1.0 launches the in-process interpreter and prints
  active states; no network protocol is exposed. The v1.1 simulator WebSocket per
  Doc 13 is reserved."*
- **§5.8 (`fsm test`).** Already correct.
- **NEW §11 — `--report-memory`** flag on `fsm generate`. Per G-01.

### Doc 19 — Formatter Specification
- **§8 (Transition formatting).** Replace `internal on EVENT:` with `internal on EVENT :`
  matching Doc 04 §8.2 — actually verify against the grammar; rewrite all examples.
- **§12 (Composite / Parallel).** Replace `composite Operational {` and
  `parallel Monitor {` with the grammar-correct form: every state uses the `state`
  keyword; composite states have braces with nested `state` declarations, parallel
  states use `parallel Monitor { region NAME { ... } }`. Per Doc 04 §3.3 / §3.4.
- **§12.** Replace `history shallow` (two words) with `shallow_history` (one keyword).
  Same for `deep_history`.
- **§13 (Pseudo-states).** Add formatting rules for `choice`, `junction`, `fork`, `join`,
  `final`, `initial`.
- **Coverage gap.** Add sections for: `import` declarations, `language fsm 2.0` header,
  `queue` block, `target` block, `enum` declarations, `as` casts, `send EVENT to M`,
  `raise EVENT`, `feature NAME` declarations.

### Doc 20 — Architecture Overview
- **§12.8 (IrVisitor).** Rewrite the `walk_machine` body to descend into `machine.root`
  (a `RegionObject`) instead of `m.states`/`m.transitions`. Add a `visit_extern`
  method. Add `visit_const`, `visit_import`, `visit_queue`, `visit_target` methods to
  match the IR additions (B-06).
- **§X (Crate diagram).** Confirm no `fsm-rpc-server` / no `fsm-lsp` crates for v1.0
  (remove if present).

### Doc 21 — TextMate Grammar
- Mark **"Status: Deferred to v1.1."**

### Doc 22 — VS Code Extension Manifest
- Mark **"Status: Deferred to v1.1."**

### Doc 23 — Developer Onboarding
- Remove any references to the LSP server / VS Code extension / WebSocket simulator
  from "What you need to know on day 1." Add a pointer to this document.

### Doc 24 — Testing Methodology
- **PARSE-NEG-001** — change `FSM-E0020` → `FSM-E0107` (per G-11).
- **PARSE-NEG-005** — change `FSM-E0024` → `FSM-E0108`.
- **PARSE-NEG-008** — change `FSM-E0102` → `FSM-E0104`.
- **PARSE-NEG-010** — change `FSM-E0200` → `FSM-E0201`.
- **Add 48 negative tests** — one per untested diagnostic code in Doc 10 (per G-10).
- **Add 6 new positive tests** — hierarchical dispatch, deep+parallel history, etc.
  (per Doc 15 update).
- Update the trace record schema in every `.trace` fixture to match Doc 13 §11
  `StepRecord` (per B-02).

---

## 9. Outstanding Open Questions (Returned To Product Strategy)

Per the task brief, this section flags anything I encountered that is a real
product/strategic question rather than a technical one — i.e. items I would NOT decide
unilaterally even at TL level.

1. **Submachines in v1.0 vs Post-v1.0 (consistency check).** The locked scope says
   submachines ARE in v1.0. But Doc 04 §15 currently says "[Future — Post-v1.0]" and
   the report's Section 6 backs that off too. I have written the doc-update plan to
   bring submachines into v1.0 per the locked decision, but this is a fairly large
   surface (cross-machine `send`, instance lifecycle, IR submachine schema). If the
   user actually meant "feature-flag submachines and ship them as preview" rather than
   "submachines are fully production v1.0," the answer would differ. **Default I
   took: submachines IN v1.0 per the locked-scope statement; conformance tests
   required.**
2. **Doc 17 (Assembly integration) scope.** Listed as "optional sections kept" in v1.0
   scope. I have not had to touch any text in Doc 17 for the reconciliation; assumed
   the existing optional content is sufficient. Confirm whether assembly integration
   needs its own conformance tests in v1.0.
3. **Doc 16 (HAL) versus Doc 11 timer integration.** Doc 11 §11 currently assumes the
   user provides a `tick()` driver. Doc 16 specifies a HAL with its own timer abstraction.
   Both are in v1.0. The question is whether the codegen MUST consume the HAL (forcing
   every embedded user to adopt Doc 16) or MAY do so. I've not made this normative
   either way — defer to product on whether HAL is mandatory.
4. **The "STL" name for the future C++17 profile.** Recorded as deferred (D-01). Naming
   for v1.1 — keep "STL" or rename to "embedded-template-library" or "C++17-embedded"
   — is a brand-surface question.
5. **License of generated code** (4.11, deferred). v1.0 ships compile-time tooling; the
   first time it actually matters is when a paying customer ships firmware. Decide
   before public 1.0 launch; not a v1.0-spec blocker.
6. **i18n strategy for diagnostic messages** (4.9). The `docs/RU/` directory exists.
   Whether v1.0 diagnostics ship localized message strings is a product call.

None of these are spec inconsistencies; all are scope/branding choices that should be
made by product before they crystallize.

---

## 10. TL Decisions on §9 Escalations

Per the autonomy mandate granted 2026-05-11 (TL/PM/Architect role, full product
authority delegated until test-confirmed MVP), the items in §9 are decided here.

### 10.1 — Submachines in v1.0 (§9.1)

**Decision: IN v1.0 as production, not preview.**

The locked scope explicitly named "submachines" as part of the full UML statechart
target. Treating them as feature-flagged preview would silently shrink v1.0 below
what was agreed. The doc-update plan in §8 ALREADY brings Doc 04 §15 in line with
this. Implementer waves treat submachines as a v1.0 first-class feature: IR schema,
codegen, conformance tests, simulator support all included. The cross-machine `send
EVENT to M` codegen (B-08, now resolved as a non-blocker requirement) is mandatory.

### 10.2 — Doc 17 Assembly Integration (§9.2)

**Decision: KEEP THE DOC, NO CONFORMANCE TEST OBLIGATION FOR v1.0.**

Doc 17 stays as informative/optional reference content. v1.0 ships C99 output; any
conforming C compiler handles assembly-level integration concerns downstream. No
cross-compile matrix (AVR/RISC-V/MSP430) or hand-written assembly conformance tests
are required for v1.0 gate. ARM cross-compile is on Doc 15 wishlist but deferred to
v1.1 along with the C++17 target. **MVP gate item G4 (`gcc -std=c99 ... -Werror` on
emitted code, host compile) is sufficient.**

### 10.3 — HAL Mandatory vs Optional (§9.3)

**Decision: HAL IS MANDATORY for v1.0 codegen output.**

Rationale:
- The "deterministic embedded" value proposition requires a single clock-abstraction
  contract. Optional HAL forks the codegen into two code paths (with-HAL,
  without-HAL) which doubles maintenance and conformance surface.
- Users who want trivial integration write ≤10 lines of HAL glue
  (`uint32_t fsm_hal_clock_now_ms(void) { return millis(); }` and a
  no-op `fsm_hal_assert(...)`). This is not a meaningful adoption barrier.
- Mandatory HAL makes the simulator trace-match deterministic across host and
  target — critical for the codegen-vs-simulator equivalence gate (G6).

Implementation guidance: `fsm-codegen-c` MUST emit `#include "fsm_hal.h"` in
`Motor.c` and the generated `Motor_dispatch` / `Motor_advance_clock` MUST call
`fsm_hal_clock_now_ms()` for timer logic. Doc 11 §11 update is in §8 patch list;
codegen brief (1.6) inherits this.

### 10.4 — License of Generated Firmware Code (§9.5)

**Decision: MIT BY DEFAULT, USER-OVERRIDABLE VIA `--license=<SPDX>` FLAG.**

Generated `.c` and `.h` files include a header block:
```
/* SPDX-License-Identifier: MIT
 * Generated by FSM Studio v1.0.0
 * Source: motor.fsm
 * Do not edit by hand. */
```

`fsm generate --license=Apache-2.0` overrides; emitted SPDX line uses the provided
identifier verbatim (validated against SPDX license list at compile time, FSM-W code
for unknown identifier). `fsm.toml` `[generate] license = "..."` provides project-wide
default.

### 10.5 — i18n for Diagnostic Messages (§9.6)

**Decision: ENGLISH-ONLY MESSAGE STRINGS FOR v1.0; CODES REMAIN LANGUAGE-NEUTRAL.**

Rationale:
- Diagnostic codes (FSM-E0100, FSM-W0203, etc.) are stable identifiers consumable
  by tools without translation.
- Localized message strings add a translation pipeline (resource bundles, fallback
  rules, plural forms) that is out of scope for an MVP.
- `docs/RU/` Russian translations of normative specs stay; they're documentation,
  not runtime strings.

A future message-table refactor (post-v1.0) can introduce i18n without
breaking the wire-format of diagnostics.

### 10.6 — C++17 Profile Naming (§9.4)

**Decision: ENTIRELY DEFERRED TO v1.1; NO NAMING DECISION RECORDED IN v1.0.**

Doc 12 stays as authored but is flagged "v1.1 target" at top. Whether the
no-STL/STL split is renamed (e.g., to `embedded-bare` / `embedded-stl`) is a v1.1
decision that benefits from real customer signal. No v1.0 spec impact.

---

## 11. Phase 1 Implementation-Time Decisions

> _Added 2026-05-14 in v1.0 doc reconciliation; see CHANGELOG._

Decisions made during implementation that go beyond Doc 00's original
reconciliation scope. Each row links the deciding wave and commit. These
build on §2 (BLOCKER fixes) and §10 (TL escalation decisions); for any
future change of similar magnitude, append §11.N here with the same column
shape.

| ID | Decision | Reasoning | Wave / Commit |
|---|---|---|---|
| §11.1 | Add `fsm-diagnostics` foundation crate (zero workspace deps) holding `Span`, `SourceLocation`, `Severity`, `DiagnosticCode` (75 variants), `Diagnostic` | Breaks cyclic dep risk; rust-analyzer / Roslyn precedent for shared diagnostic types | Wave 1.0 / `db5ef83` |
| §11.2 | `RegionObject.initial: StateId` always points at an Initial pseudo-state in `region.states` (not a bare state name string) | Doc 09 §4.4 + §18 example mandate it; bare-name caused simulator init failure | Wave 1.9 / `c4f372d` |
| §11.3 | Multi-active-leaf representation: `m->_active[MAX_PARALLEL_REGIONS]` array, NOT `m->_state + _state_region_N` slots | Uniform handling; non-parallel = N=1 slot; matches simulator's `RuntimeState.active_states` Vec | P0-2/P0-3 / `e4884bc` |
| §11.4 | B-11 collect-then-execute table dispatch: per-region ancestor walk → `selected[]` buffer → execute pass | Doc 00 §7.8 normative; eliminates first-match-return bug for parallel | P0-2/P0-3 / `e4884bc` |
| §11.5 | Per-timer distinct event IDs (`MOTOR_EVENT_TIMER_<TIMER_ID>_FIRED`); `timer_id` field on `Trigger::After`/`Every` | Prevents collision with `done` completion's shared EVENT__COMPLETION | P0-4 / `cde56e2` |
| §11.6 | Timer arm-on-entry to timer-owning state; disarm-on-exit | Doc 08 §13 semantics; previous arm-at-init broke for any non-initial timer-owning state | P0-4 / `cde56e2` |
| §11.7 | `defer EVENT` rejected at analyzer with `FSM-E0903` in v1.0 (option-b downgrade) | Honesty-over-broken: silent-drop violated G1; full defer queue deferred to v1.1 | P0-5 / `6ec5268` |
| §11.8 | `done` auto-fire scoped to Simple states only; Composite/Parallel `done` still gates on region-Final (B-08) | UML completion semantics; preserves parallel-completion contract | R1 / `a94ff91` |
| §11.9 | Codegen panics → `Result<_, EmitError>` propagated to CLI exit code 2 | P1-8; pre-flight validation makes `must_lookup` unreachable in production | R1 / `a94ff91` |
| §11.10 | HashMap → BTreeMap for all serialized public types (`StepRecord.payload`/`context`, `InterpreterSnapshot.history`) | Doc 13 §11 wire-format byte-exact determinism; 100-iter regression test | Sim polish / `e6dba2c` |
| §11.11 | Guard-eval errors propagate via `StepError::GuardEval(EvalError)` instead of silent `unwrap_or(false)` | Audit P1-7; runtime guard errors surface rather than disabling transitions | Sim polish / `e6dba2c` |
| §11.12 | Import path security: `resolve_import` with `Path::canonicalize` + workspace-root prefix check | Doc 00 §7.12 G-02; symlink escape detection beyond shape check | P1 security / `bccc46f` |
| §11.13 | Parser DoS limits: `max_input_bytes`=1MiB, `max_recursion_depth`=256, `max_token_count`≈262k; RAII depth guard | Doc 00 §7.12; stack/RAM exhaustion blocked at `parse()` entry | P1 security / `bccc46f` |
| §11.14 | Multi-strategy codegen exposed: `--strategy {switch,table,auto}` CLI flag; Auto picks switch if state count < 64, else table | Doc 11 §8 + Doc 00 §5.1; both strategies share `emit/transition.rs` for behaviour equivalence | P0-2/P0-3 / `e4884bc` |
| §11.15 | Empty `expected` in `.trace` files is now a hard fail (was silent pass); `--allow-empty-expected` opt-in | Doc 23 §9 G6 gate was unverifiable | P1 wave / `1f5d7de` |
| §11.16 | gcc tests for ALL 3 shipped examples (motor, traffic-light, vending-machine); skip is opt-in via `FSM_SKIP_GCC_TESTS` env var | P1-3; previously silent skip on missing gcc | P1 wave / `1f5d7de` |
| §11.17 | CGEN-002 conformance fixture asserts on structural strings (`parent_table`, ancestor-walk loop pattern), not literal comment text | Comments are reword-fragile; structural code proves B-10 | Trace refresh / `e400bc1` |
| §11.18 | Context field defaults emitted in `Motor_init` AND applied in `Interpreter::init` | Closed silent data-loss path; users' declared defaults now take effect | R1 / `a94ff91` |
| §11.19 | Submachine epic reconstructed as 4 waves after false "already lowered" claim; nested-in-composite/parallel ref REJECTED at analysis (FSM-E0502), not silently emitted as broken C | P0-1 aspirational-prose pattern recurred (incl. orchestrator-level, caught by §11.3 phase audit); clean reject > broken output (G1, defer/E0903 precedent) | W2a-d `7a69612`/`2384608`/`8b66dc2`/`af8c300` + P1-2 `02d4ded` |
| §11.20 | Leading comment/whitespace before `language` no longer panics rowan builder (FILE node opened before leading-trivia flush) | License/banner headers atop files are ubiquitous; a parser panic violates G1 | PB1 `3017c1c` |
| §11.21 | `likely`/`rare` are **contextual** (not reserved) keywords; hint wraps the guard condition; portable `__builtin_expect` macro + non-GNU fallback + opt-out; sim accepts-ignores (layout-only) | Back-compat (existing `.fsm` may use them as idents); embedded portability; zero semantic effect keeps sim≡codegen | W4 `300d1e4` |
| §11.22 | Checkpoint/release tags require a COLD from-source green quad; warm shared-`CARGO_TARGET_DIR` can serve stale cross-worktree test binaries (baked `-wt-` abs paths) | Phase-audit P1-1: a warm post-merge quad is necessary-not-sufficient; "known-good" must mean built-from-source | §11.1 hardened / `e2e9294` |
| §11.23 | OPAQUE-BUG-1 **modelled end-to-end** (not loud-rejected): `lower_type_ref` re-parented to `lower_type_ref_node(parent)` resolving `TYPE_REF` *or* `OPAQUE_TYPE_REF`; `OpaqueTypeRef::c_type()` accessor added; all 4 call sites (context field, extern param, extern return, `as`-cast) fixed. IR `Type::Opaque` + codegen `c_type_str` already existed → no IR/codegen variant added. Symptom was scenario-C silent-data-loss (`fsm check` exit 0, then broken + wrong-arity C) on a documented Doc 04 construct | Bounded fix — only the AST→IR layer was broken; IR + codegen already supported opaque — made full modelling cheaper than the defer/E0903-style loud-reject; restores core embedded HAL handle-passing. W5 importer opaque-skip *may* now relax (noted, not changed — separate product-surface decision) | OPAQUE-BUG-1 / `phase2.14/ob1-opaque-extern` |
| §11.24 | W6 integration examples are **real builds, not docs**: cargo-rust isolated via an empty `[workspace]` table (cargo walks up the tree; parent uses explicit `members` so wouldn't absorb, but the empty table makes it its own root — `cargo metadata` verified); Rust↔C bridged by a thin `ffi_helpers.c` shim over the **stable public API**, not by mirroring `Motor_t` (156-byte private layout is an impl detail, not the ABI); all C/C++ sources forced **ASCII-only** (em-dash/§ bytes broke a -Werror build — embedded toolchain portability); platformio = `native`+`uno` envs with a real Arduino sketch; `cmake` installed via apt (was absent; required deliverable; standard non-destructive dev tool; engineering-hygiene autonomy) | A copy-pasted example that doesn't actually compile is worse than none (the P0-1 aspirational-prose lesson applied to integration surface); the shim/ASCII/workspace nuances are exactly the gotchas a real integrator hits — documented in `docs/25` §3.3/§5/troubleshooting so adopters aren't surprised. **No codegen/CLI defect surfaced** — generated C passed `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` across make/cmake/cargo-rust/host-gcc (independent corroboration that P1-2/PB1/OB1 + the pipeline hold under real ecosystems) | W6 `e74888a` |
| §11.25 | **Per-machine dispatch override** (`fsm.toml [machine.<Name>] strategy`). (a) Precedence resolved per machine = `[machine.M].strategy` > CLI `--strategy` > default `auto`; the single resolution point is `CodegenConfig::strategy_for(name)` (codegen-c/config.rs) consulted in `emit_machine_recursive` (codegen-c/emit/mod.rs) — the override map holds ONLY the explicit `[machine.M]` tier, and `strategy_for` falls back to `cfg.strategy` (which already encodes the flag>generate-section>auto lower tiers) for un-overridden machines, so one function expresses the whole chain. (b) `Auto` is resolved per emitted unit against THAT unit's own state count (not the project's) so the small-machine-readability heuristic is preserved. (c) **Absent-machine = WARN, not error** — a project-wide fsm.toml legitimately generates a subset of its `.fsm` files; erroring would break that valid workflow, and a WARN still catches the typo case. (d) **Submachine templates inherit the parent top-level machine's effective strategy** (threaded `effective` param), NOT their own name's map entry — a submachine has no user-facing `[machine.X]` surface and mixing dispatch *within* one logical machine would surprise; the feature is per-top-level-machine. (e) Invalid value = exit-4 (Doc 18 §3) naming the offending machine, never silent fallback (zero-legacy). Empty map ⇒ byte-identical output to pre-W7. | Mixed-dispatch in one project is a real embedded need (size-critical machine → table, debuggable machine → switch) without splitting projects or losing the `auto` default. **Surfaced (NOT introduced) W7-FU-1**, a pre-existing core dispatch defect of the P0-1 silent-data-loss class: a state with two `on EVENT` transitions distinguished only by guards is mis-lowered by BOTH strategies independently of W7 — switch emits duplicate C `case` labels (gcc hard error, even without `-Werror`), table's `select_for_region` returns the first `(source,trigger)` row ignoring the guard then drops the event if that guard fails. Fixing it is in `dispatch_switch.rs`/`dispatch_table.rs` and explicitly OUT of W7 scope (brief DO-NOT). Honestly flagged + pinned by the `w7_fu1_*` tripwire test (fails loudly when fixed, forcing conversion to a positive test) — same SUB-FU-2/OPAQUE-BUG-1 honest-surface standard. The W7 fixture deliberately uses unambiguous one-event-per-transition so it proves mixed dispatch cleanly without entangling W7-FU-1. | W7 `phase2.16/w7-per-machine-strategy` |
| §11.26 | **W7-FU-1 fixed — guard-disambiguated same-event dispatch (BOTH strategies), codegen-only.** The defect was `dispatch_switch.rs` emitting one C `case <EVENT>:` per transition (duplicate-`case` gcc hard error; 2nd+ guarded transition unreachable) and `dispatch_table.rs::select_for_region` returning the first `(source,trigger)` row ignoring its guard, the executor re-checking and silently no-op'ing (event dropped — P0-1 silent-data-loss class). (a) **Spec rule applied = Doc 08 §4.1/§4.2** (mirrored Doc 04 informational SELECT): among same-(source,event) transitions the candidate set is the guard-enabled ones; winner = `min(candidates, key=(priority, document_order))` — first-enabled in (priority asc, then declared source order); an unguarded/`[else]` transition is an always-enabled catch-all; if none enabled the event bubbles up (no consume). (b) **Sim-vs-SPEC cross-check: the simulator was independently verified CORRECT against the spec** (`interpreter.rs::select_transitions` filters the guard into `candidates` BEFORE a STABLE priority sort that preserves the IR Vec's document order, then takes `candidates[0]`). Single-bug case (codegen-only); the simulator is the oracle and the fix makes both strategies match it. (c) **switch:** `SwitchEmitter::visit_state` now groups the priority/doc-order-sorted candidates by resolved C event enum and emits ONE `case` per event whose body chains each candidate as `do { if(!guard) break; <body>; return true; } while(0)` — guard-fail falls to the next candidate, an unguarded candidate (no `if`) is the terminating catch-all (`emit_event_case`/`trigger_event_c` replace `emit_one_case`). **table:** new `<M>_row_guard_enabled(row_idx,m,ev)` evaluates each row's guard (keyed by deterministic `row_idx` because the guard addresses the per-event payload union member); `select_for_region` now `continue`s past a guard-false row instead of returning it — the table is pre-sorted `(source,priority,row_idx)` so the first guard-enabled match is the spec winner. (d) Behaviourally accepted per §5.4 on BOTH strategies: `fsm generate` → `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (0 warnings) → **RUN-asserted** the priority-tiebreak (>1 guard simultaneously true → declared/priority-first wins), the unguarded fallback, and the no-match-no-fallback (discard) cases, plus sim==codegen. The `w7_fu1_*` tripwire was CONVERTED to a positive correctness test (both strategies); conformance CGEN-004 added. (e) **Surfaced (NOT fixed — out of scope, flagged):** a `priority`-clause-less transition lowers to IR priority **0**, not the Doc 04 §8.6-stated **100** (`lower/state.rs` `.unwrap_or(0)`). Applied IDENTICALLY by sim and codegen so sim≡codegen and the spec rule hold regardless of the absolute default; but it means an unguarded transition with no explicit priority would out-prioritise a defaulted guarded one — the acceptance fixture uses explicit `priority 1/2/3` (the FSM-E0300 fix-option-3 W0300 path) which is also the only spec-legal construct where >1 guard can be simultaneously true so order genuinely decides. Recommend a separate doc-vs-impl reconciliation wave (pick 100-as-default or amend Doc 04 §8.6). | A core legal UML construct silently miscompiling while the project advertises "full UML" is the exact P0-1/submachine overstatement the project keeps re-learning; the §5.4 behavioural-acceptance mandate caught it at the W7 boundary (§11.3 working as designed). Restricting the fix to dispatch *selection* (not redesigning dispatch) and matching the spec-verified simulator keeps sim≡codegen — the project's hard invariant — provably intact. The priority-default discrepancy is real but orthogonal, consistently applied, and a product-surface decision; flagged not silently "fixed" (§11.19/§11.23 honest-surface standard). | W7-FU-1 `phase2.17/w7fu1-guard-disambiguated-dispatch` |

---

*End of FSM-SPEC-DEC v1.0.0 (TL-amended 2026-05-11; §11 appended 2026-05-14; v1.1 rows §11.19-26 appended 2026-05-15)*
