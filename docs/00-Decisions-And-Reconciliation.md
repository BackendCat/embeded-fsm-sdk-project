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
| §11.27 | **W7-FU-2 — default transition priority reconciled to `100` (impl was the bug, docs win).** Resolves the §11.26(e) doc-vs-impl discrepancy. **Decision = default `100`**, with cited evidence: **Doc 04 §8.6** ("Lower number = higher priority. Default: `100`."), **Doc 09 §6** ("`priority` … default 100"; canonical example `"priority": 100`; example IR lines show clause-less transitions = `100`), and the IR `model.rs` `TransitionObject.priority` doc-comment already said "Default 100". NO corpus source intends `0` (Doc 08 §4.2 only states the min-wins rule, not a default; Doc 09 §5's region-`priority` `0` is a *separate field* — region dispatch order for parallel states — not the transition default). Rationale: under min-wins on `(priority, document_order)` an unprioritized transition must be LOW priority so a small explicit number floats a specific transition above the default herd; `0` (highest) made an unprioritized transition out-prioritise any explicitly-deprioritised one and forbade ever placing a transition below default — the priority feature was half-useless. (a) **Class-of-issues fix, single source of truth:** new `fsm_ir::DEFAULT_TRANSITION_PRIORITY: u16 = 100` (+ `default_transition_priority()` for serde). Sites changed: the four `lower_*` arms in `fsm-analyzer/src/lower/state.rs` (`.unwrap_or(0)` → `.unwrap_or(i64::from(DEFAULT_TRANSITION_PRIORITY))`); `TransitionObject.priority` gained `#[serde(default = "default_transition_priority")]` + corrected doc; the **wrong** `lower/mod.rs` doc-comment ("applies Doc 09 §1 defaults: `priority: 0`") rewritten (it conflated region vs transition priority and cited the wrong section); `schema/ir/1.0.0/model.json` transition `priority` gained `"default": 100` + description. (b) **NOT touched (verified, not assumed):** the simulator (`interpreter.rs::select_transitions`) and both codegen strategies only READ the IR `priority` (no default materialized there) — already W7-FU-1-correct; `RegionObject.priority` `0` literals across analyzer/sim/codegen are the distinct region-dispatch field (Doc 09 §5) and out of scope; `extract_priority`/AST `priority()` return `Option` (no default). (c) **Regression-audited:** `fsm-analyzer/tests/lowering.rs::ir_default_priority_is_zero` was pinning the *bug* (asserted `0`) — corrected & renamed to `transition_without_priority_clause_lowers_to_spec_default_100` (the FAILS-on-old / PASSES-on-new anchor + §5.4 unit pin). No shipped example changed (no example `.fsm` uses `priority`; CGEN-004 / the W7-FU-1 fixture use explicit `priority 1/2/3`; FSM-E0300/W0300 keys off `priority.is_some()` on the AST, NOT the materialized IR default — diagnostics unaffected). Full quad + 5/5 examples + ≥25 conformance re-run green. (d) **§5.4 behavioural acceptance:** the analyzer's FSM-E0300 forbids a clause-less transition in same-source same-event runtime competition (why W7-FU-1 had to use explicit priorities), so the default's *observable* effect is only demonstrable at the codegen/sim contract layer below the compile-time gate — a **hand-built IR** (the established pattern: `codegen_equivalence_smoke.rs`, `done_autofire_c.rs`) with one `priority 50` transition vs one at `DEFAULT_TRANSITION_PRIORITY` (the value the fixed lowering yields); `emit` both strategies → `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` → RUN-asserted the spec-correct (50<100) transition fired, plus sim≡codegen, in `fsm-cli/tests/default_transition_priority_e2e.rs`. The fixture is tied to the constant so a regression to `0` flips the winner and fails loudly. | The impl silently contradicting a normative, unambiguous spec statement on a core UML construct is precisely the P0-1/submachine overstatement class; reconciling toward the spec (not bending docs to a buggy impl) plus a constant-as-single-source-of-truth + a regression-anchored §5.4 behavioural fixture closes it for good. Choosing `100` is the only coherent reading under min-wins and is what every normative source already says. | W7-FU-2 `phase2.18/w7fu2-default-transition-priority` |
| §11.28 | **SEC-P0-1 — `--import-header` / `fsm.toml import_headers` file-read surface hardened by CONVERGING on the v1.0 G-02 primitives (no third variant).** v1.1-W5 added a new file-read path that bypassed the explicitly-hardened G-02 boundary (Doc 18 §10): absolute header paths returned verbatim, relative ones joined with no shape-check / no `canonicalize` / no workspace-root containment, and `parse_header_file` read with no size cap (DoS via `/dev/zero` / multi-GB file before the parser's 1 MiB cap). **The root cause was a divergent parallel path, so the fix converges, it does not duplicate:** the attacker-controlled surface is routed through `fsm_parser::import_resolver::resolve_import` *directly* — the exact same primitive the DSL `import "..."` path uses (shape-reject `..`/NUL/absolute/UNC → `canonicalize` → workspace-root prefix); there is NO third copy of the containment logic. A small shared module `fsm-cli/src/safe_io.rs` owns the ONE bounded-read helper (`read_to_string_capped`, keyed off `ParseLimits::DEFAULT.max_input_bytes` so the header/`fsm.toml` cap cannot drift from the `.fsm` cap) and the ONE `workspace_root_for` resolver — the latter **moved out of `cmd::check`** so `fsm check` and `fsm generate` share a single workspace-root definition rather than two drifting copies (zero-legacy convergence). (a) **Deliberate absolute-path / trust decision:** `fsm.toml import_headers` is attacker-controlled (the `fsm.toml` ships *with* a possibly-hostile repo on shared CI — the exact G-02 threat) → full DSL-`import` containment by default. `--import-header` is *invocation-supplied* — the same trust level as the `.fsm` path argument and `--out`; the G-02 threat model scopes the danger to "a `.fsm` *source* / project file posted to a shared host", NOT to the CI job's own argv — and an absolute vendored-HAL path is the *documented normal* use of the flag (the W5 example/tests themselves pass an absolute path; hard-rejecting it would be both wrong for the threat model and a functional regression) → CLI flag stays absolute-capable (NUL-shape-reject + the universal DoS size cap still apply; containment intentionally NOT enforced). The genuine vendored-HAL-via-`fsm.toml` case is served by an **explicit, named, default-OFF** opt-in `[generate] allow_unscoped_import_headers = true` that downgrades `fsm.toml` entries to the trusted-invoker level — never a silent allow, and the DoS cap is non-negotiable regardless. Secure-by-default is preserved; the escape hatch is loud and opt-in. (b) **DoS cap** applied in `parse_header_file` AND (REL-P2-1, folded in per the audit — same defect class, same helper) `config::load`'s `fsm.toml` read, both via the one shared bounded read; an unsized stream (`/dev/zero`, FIFO, device reporting len 0) is `take`-bounded so it terminates+rejects instead of OOMing — proven by a real `/dev/zero` test that asserts bounded behaviour, never a hang. Oversized header → clean exit-3 (same as any unreadable header — the established mapping); oversized `fsm.toml` → clean exit-4 (the established `ConfigError` mapping); containment escape → exit-1 (a validation error in attacker-influenced input, mirroring the DSL `import` escape). (c) **§5.4 behavioural acceptance:** new `fsm-cli/tests/import_header_security.rs` (mirrors `fsm-parser/tests/import_security.rs`) proves each attack — `../../`-traversal, absolute-from-`fsm.toml`, workspace-relative **symlink-escape** (the case shape-check alone misses; only canonicalize-plus-prefix catches it), NUL byte, >1 MiB header, `/dev/zero`, oversized `fsm.toml` — is rejected with the asserted exit code AND the target is provably never read into the generated tree / never OOMs; plus positive controls that the opt-in genuinely permits an out-of-tree header and that a legitimate in-workspace relative header still imports with containment ON. The pre-existing W5 end-to-end §5.4 acceptance (`import_header_e2e.rs`: real header → `fsm generate` → `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` → RUN, both dispatch strategies, asserting imported externs genuinely invoked) **still passes unchanged** — the hardening did not break legitimate end-to-end use. Full quad green (build 0 / 673 tests 0-fail, +17 vs the 656 baseline / clippy `-D warnings` 0 / fmt clean); `fsm test examples/` 5/5 + `tests/conformance/` 25/25 no regression. | A regression in the explicitly-hardened G-02 security boundary, on a surface the project's own Doc 18 §10 says must be safe in shared CI, is the exact overstatement-vs-reality class the project keeps re-learning (P0-1 / submachine / OPAQUE-BUG-1) — and D.2 mandates a security pass before any release with new surface. Converging on the existing correct primitive (rather than authoring a third containment implementation that would itself drift) is the project's zero-legacy principle applied to security; the trust-level split is the principled reading of the threat model (source/config travels with the attacker's repo and is contained; argv is the trusted invoker's, like every other CLI path) and is the only decision that closes the exfiltration vector WITHOUT a functional regression to the documented vendored-HAL workflow, with the residual config-vendored-HAL case handled by an explicit opt-in, never a silent widening. The one remaining v1.1 tag-blocker; v1.1 is tag-ready immediately after. | SEC-P0-1 `phase2.19/secp01-import-header-hardening` |
| §11.29 | **verify-status-claims-vs-code applies to AUDIT findings too — REL-P1-1 was stale vs current code; the release record was corrected to the shipped reality, not regressed to the audit's stale claim.** The v1.1 pre-tag reliability audit (`docs/AUDIT_PRE_TAG_v1_1_RELIABILITY_2026-05-15.md` REL-P1-1) claimed a submachine ref nested in a composite/parallel state "emits non-compilable C" and recommended a *minimum gate of correcting the CHANGELOG/ROADMAP wording to say it does not compile*. Verifying against current `crates/fsm-analyzer/src/checks/submachine.rs:106-135`: that case is in fact **rejected at analysis with `FSM-E0502`** + an actionable message, `continue`-skipped so codegen never receives it, with the lowerer refusing it too (defence-in-depth) — shipped by P1-2 `02d4ded`, test-pinned by `crates/fsm-analyzer/tests/nested_submachine_rejected.rs`. The auditor had traced via the *older* `AUDIT_PHASE_SUBMACHINE_2026_05_15.md` (pre-P1-2 state) and did not re-verify against the current source. Blindly applying the audit's recommendation would have **regressed the release record into a falsehood** (stating v1.1 emits broken C when it cleanly rejects) — the cardinal overstatement sin, inverted. CHANGELOG `[1.1.0]` + ROADMAP were corrected to the shipped E0502-reject reality (completed tense, code + test cited); the stale `[Unreleased]` "fix in progress / will reject / emits broken C" wording was removed. The frozen audit doc is left intact (audit evidence is never edited); this row is the canonical correction of record. Lesson: `[[verify-status-claims-vs-code]]` is symmetric — an audit asserting a defect is itself a claim that must be code-verified before the release record is changed on its basis; conservative completed-tense wording verified against *current* source is the only safe input to a tag. | An independent audit is a strong signal but not ground truth; the ground truth is the code at HEAD. The whole project ethos is prose-vs-code reconciliation — the same rigor that catches optimistic overstatement must catch a pessimistic stale-audit claim, or the release record drifts the other way. Recording this so a future maintainer reading the frozen REL-P1-1 finding sees it was superseded by code verification, and so the meta-lesson (verify audits too) is not lost. | W8 doc-honesty pass (pre-`v1.1.0` tag) |
| §11.30 | **Release-tag commit-ordering refinement: gate-doc commit → cold quad → tag, so quad-commit ≡ tag-commit (no off-by-one).** v1.1.0's §11.22 cold-from-source quad ran at `eb35d4b`; the annotated `v1.1.0` tag is at `abc7004` = `eb35d4b` + exactly one commit, and that commit is **purely `docs/GATE_VERIFICATION_v1_1.md`** (+92, zero source/test/build surface). Verified: `git rev-list -1 v1.1.0` == `git rev-parse checkpoint/2026-05-15` == `abc7004` (the annotated-tag *object* `9634c7e` deref's to that commit — a metrics-wave "one commit after" reading was the tag-object-vs-commit confusion, not a real mismatch). The gap is provably immaterial for v1.1.0 (a Markdown-only delta cannot change `cargo build/test/clippy/fmt` / `fsm test`), and `GATE_VERIFICATION_v1_1.md` §7 reconciles the record. But it is only *provably* immaterial because the gate doc here contains no code-summarising assertion that a docs diff could falsify. **Refinement (binding for v1.2+):** the release sequence is (1) author+commit `GATE_VERIFICATION_v<x>.md`, (2) run the cold-from-source quad at THAT commit, (3) `git tag -a` THAT commit + the `checkpoint/<date>` anchor — quad-commit ≡ tag-commit exactly. The tag is **immutable and not moved** (re-cutting a release tag is a destructive op we never perform); this row + §7 are the honest resolution. | Tag immutability is sacrosanct, so an off-by-one between the verified commit and the tagged commit cannot be fixed after the fact — only prevented next time and reconciled-in-the-record this time. Codifying the ordering removes the ambiguity at its source for every future release; recording the v1.1.0 specifics so the carry-forward of evidence is auditable rather than asserted. | post-tag stewardship (`67efdc1`-adjacent) |
| §11.31 | **`pub` over-exposure (gate-before-v1.2 debt) measured + safely reduced; `unreachable_pub` lint deferred (residual ≠ 0 is test-harness-only, not `src/`).** One-off non-gated measurement (`RUSTFLAGS=-W unreachable_pub -W missing_docs cargo clippy --workspace --all-targets`): **`unreachable_pub` = 170 in `src/`** (fsm-parser 39, fsm-formatter 35, fsm-cli 96 — all in private `mod grammar`/`mod format`/the binary crate's submodules) **+ 25 in `tests/common/mod.rs`** (fsm-simulator 16, fsm-codegen-c 7, fsm-cli 2); **`missing_docs` = 1017 in `src/`** (fsm-parser 382, fsm-ir 311, fsm-simulator 157, fsm-lexer 83, fsm-codegen-c 40, fsm-diagnostics 23, fsm-analyzer 21; 0 in fsm-formatter/fsm-cli). **All 170 `src/` items downgraded `pub`→`pub(crate)`** by a targeted line-exact rewrite (only the flagged `file:line` set; rule = downgrade IFF `unreachable_pub` flagged it, since the compiler's reachability analysis *is* the proof the item is not cross-crate API and visibility-narrowing within an already-private module is behaviour-neutral) — residual `src/` `unreachable_pub` now **0**. `cargo fmt` re-wrapped a handful of now-too-long `pub(crate) fn` signatures (additive only — 170 removed lines were all the `pub` swaps, 0 logic lines). One latent pre-existing dead-code pair surfaced (the over-broad `pub` had masked it): `config.rs` `CompilerSection.{allow,deny}` are deserialized-but-never-read serde fields → `#[allow(dead_code)]` matching the established `cmd::test` precedent (NOT reverted to `pub`, which would re-trip `unreachable_pub`; NOT deleted, which would change the accepted `fsm.toml` schema). **`missing_docs` left OFF, zero doc comments written** (explicitly out of scope — a separate multi-wave effort; the 1017 count is recorded here for v1.2 planning). **`unreachable_pub` lint NOT wired:** residual is 25 (the `tests/common/mod.rs` shared-harness API consumed across dozens of integration-test files — genuine intended test API, already `#![allow(dead_code)]` for the symmetric cross-test-binary reason; conservatively LEFT `pub`). `[workspace.lints]` reaches a package's test targets, so wiring `unreachable_pub = "warn"` would fire those 25 under the quad's `cargo clippy --workspace --all-targets -- -D warnings` and break it — per the brief's residual≠0 rule the wiring is **deferred to a follow-up** (which would `#![allow(unreachable_pub)]` the 3 test-helper modules, then wire). Behaviour byte-identical: 673/0 tests unchanged, `fsm test examples/` 5/5, `tests/conformance/` 25/25, `lower_split_byte_identity` fingerprint guard 4/4 — all unchanged. | The carried debt was real but the lint-wiring was conditional on a green floor it could not reach this wave without scope-creeping into the test-harness layer; reducing the 170 provably-safe `src/` items (the actual public-surface noise) while honestly deferring the lint — rather than forcing it green by touching test infrastructure or accepting a broken quad — is the conservative measure-and-safe-reduce mandate. Downgrading only the compiler-proven-unreachable set guarantees no genuine cross-crate API moved; the one masked dead-code pair is exactly the kind of latent issue an over-`pub` surface hides, surfaced honestly (§11.19/§11.23/§11.26 honest-surface standard) not silently papered. | v1.2-gate-WA `phase2.21/v1_2gate-pub-hygiene` |
| §11.32 | **v1.2-LSP-L1 implementation decisions (the LSP spine).** (1) **`positionEncoding` default = UTF-8.** Per Doc 26 §4.1/risk-1 the server advertises preference for `utf-8` and negotiates it iff the client lists it in `general.positionEncodings`; every other case (3.17 client offering only utf-16, OR a pre-3.17 client advertising no capability) → UTF-16, the LSP ≤3.16 universal default. UTF-8 makes an LSP `character` a *byte* column so it maps directly onto the byte-offset `Span` — the entire UTF-16 transcoding defect class is deleted on the common path. Clippy correctly flagged the two non-UTF-8 outcomes as one branch, not two (they are identical) — collapsed accordingly. (2) **`position.rs` convergence: LSP-authoritative-only; the two existing impls NOT converged in L1 (tracked as DRIFT-2).** Doc 26 §4.1 names two divergent, non-LSP-correct byte→line/col converters: `fsm_analyzer::util::compute_line_col` (bytes, 1-based — baked into IR `SourceLocation` consumed by codegen/sim) and `fsm_cli::cmd::check::line_col` (Unicode-scalars, 1-based — the `--json`/human renderer contract, asserted by CLI tests). `crates/fsm-lsp/src/position.rs` adds the **one** LSP-correct converter (0-based, negotiated-encoding `LineIndex`, rust-analyzer model) and is deliberately NOT shared with the other two: converging them would change two *shipped* subsystems' observable line/col output (a non-behaviour-neutral change the project bar forbids in a non-refactor wave) for no L1 benefit. **Decision: L1's `position.rs` is authoritative for the LSP; the existing-two convergence stays tracked as DRIFT-2** (not regressed, not widened — a deliberate scope boundary, not a third silent copy). (3) **The exact reuse seam.** `crates/fsm-lsp/src/analysis.rs::analyze` mirrors `crates/fsm-cli/src/cmd/check.rs` line-for-line: `fsm_parser::parse` (check.rs:46) → workspace-root walk (check.rs:59) → `security_check_imports`/`fsm_parser::import_resolver::resolve_import` (check.rs:60,178) → `fsm_analyzer::analyze_with_source` (check.rs:61) → import-diags appended (check.rs:65). NO analysis re-implemented, NO shelling to the `fsm` binary — the editor squiggle provably cannot disagree with `fsm check --json` (the §5.4 acceptance asserts byte-exact Range + code parity against the CLI oracle). `fsm_cli::safe_io::workspace_root_for` is `pub(crate)` *inside the binary crate* and correctly NOT public API (Doc 26 §6 does not list the CLI as an intended seam); its 11-line `fsm.toml`-walk is re-implemented behaviour-identically rather than making the binary a dependency — this is NOT a #64-narrowed public-API regression (nothing cross-crate-public was narrowed; #64 narrowed only unreachable items and the public pipeline `parse`/`analyze_with_source`/`resolve_import` is fully intact — no re-widening needed). (4) **`#![forbid(unsafe_code)]` in `fsm-lsp` → workspace is now 10/10 forbid-unsafe.** `tower-lsp`/`tokio` require zero consumer-side `unsafe`; the new crate upholds the project invariant. (5) **Dependency pins (Doc 23 §2 MSRV 1.75):** `tower-lsp = "0.20"` (0.20.0 resolved; its `lsp-types` 0.94.1 re-export + `async-trait` closure is the newest line that still compiles on Rust 1.75 — 0.21+ raises the MSRV), `tokio = "1"` (1.52.x already warm in the local cargo cache). | The LSP epic's riskiest architecture bet (the position-encoding correctness class, Doc 26 risk-1) is validated end-to-end before any breadth: UTF-8-default deletes the transcoding class on the common path while UTF-16 stays correct via the same `LineIndex`; the §5.4 acceptance proves Range bytes under BOTH encodings on a multibyte line (a naive byte/scalar shim fails it — that is the point). Not converging the legacy converters is the conservative call — L1 must not change shipped analyzer/CLI behaviour, and DRIFT-2 already owns that follow-up; adding a third correct-but-separate converter is the lesser evil vs. a behaviour-changing refactor smuggled into a feature wave. The reuse seam is line-identical to `check.rs` so the LSP inherits every diagnostic-producing wave's validation transitively (Doc 26 §3). | v1.2-LSP-L1 `phase2.23/v1_2-lsp-l1` |
| §11.33 | **v1.2-LSP-L2 implementation decisions (`documentSymbol` + `foldingRange`, no new analysis).** (1) **`symbol_table` thread-through — ONE analysis feeds BOTH consumers.** L1's `crates/fsm-lsp/src/analysis.rs::analyze` ran the exact `fsm check` pipeline but discarded `result.symbol_table` (an explicit L1 scope-boundary comment). L2 adds a `symbol_table: SymbolTable` field to `Analysis` and stops dropping it — a strictly **additive, behaviour-neutral** change: `analyze()` makes the *identical* `analyze_with_source(&pr, …, src)` call in the identical order and merges import diagnostics identically; it merely *keeps* a field it previously let fall on the floor. `documentSymbol` (`server.rs::Backend::document_symbol`) consumes that threaded `symbol_table` from the SAME `analyze()` the debounced `publishDiagnostics` runs — **no second analysis pass, no parallel symbol extraction** (Doc 26 §8 L2 "one analysis feeds both"). Verified diagnostics output is byte-identical to L1: the L1 §5.4 client tests (byte-exact diagnostic-Range + `FSM-Exxxx` parity vs the `fsm check --json` oracle, incl. the non-ASCII both-encoding test) all still pass unchanged. `result.ir` is still discarded (it is the L3+ hover/inlay seam, not L2). (2) **`foldingRange` source = pure CST walk, NOT the symbol table.** Folding is purely structural, so `crates/fsm-lsp/src/capabilities/folding.rs` is a single `descendants_with_tokens()` walk of the parse tree (Doc 26 §5 `foldingRange` row: "CST node ranges … pure tree walk") — it runs **no** analysis at all (not even the threaded one) and consumes no `symbol_table`. Foldable kinds: `MACHINE_DECL`/`STATE_DECL`/`REGION_DECL`/`CONTEXT_BLOCK`/`EVENTS_BLOCK` node bodies + `BlockComment`/`DocComment` tokens, emitted only when spanning ≥2 lines (a degenerate `start==end` fold is client-discarded noise). (3) **Doc-26-seam-table vs `StateEntry`-shape drift — surfaced + handled, not papered (the prose-vs-code class the project polices).** Doc 26 §5's `documentSymbol` row says "`StateEntry.container_path`+`shape` build the Doc 14 §13 tree". The real `StateEntry.shape` (`StateShape`) only discriminates **pseudo-states** (`Final`/`Choice`/`Fork`/…) from `Plain`; it **cannot** distinguish a composite from a simple state — its own doc-comment states Composite-vs-Simple "is resolved at lowering time by inspecting nested AST children", and `visit_state` hard-codes `StateShape::Plain` for every real `state X`. So `shape` alone does **not** carry the composite/simple bit Doc 14 §13 needs (`Idle (simple)` / `Running (composite)`). Resolution: composite-vs-simple is **derived** from the `container_path` graph the same single analysis already built — a real (`Plain`) state with ≥1 child in the reconstructed hierarchy is composite, with 0 children is simple. This is still "from the symbol table" (the parent/child edges *are* `container_path`); it adds **no** analysis and re-implements **no** extraction. Doc 26 §5's phrasing is imprecise (`shape` is not sufficient on its own) but its *intent* — build the tree from `symbol_table`'s ordered tables + `container_path` + `shape` — is met; `shape` is still load-bearing for the pseudo-state kinds. Flagged here per the prose-vs-code discipline; **not** a defect requiring a code or Doc-26 change (the seam works; the doc's one-clause shorthand over-credits `shape`). (4) **`selectionRange` name-token lookup.** `SymbolTable` stores only the full-declaration `Span`, no separate name span. `selectionRange` (the name, ⊆ `range`) is produced by locating the first `Ident` token within the declaration span — the *exact* rule `fsm_parser::ast::first_ident` (and thus the table builder) used to derive the name string, so name string and `selectionRange` are byte-consistent. This is the standard LSP name-locating step (Doc 26 §5 scopes `documentSymbol` as "tree assembly + Span→Range only"; the name lookup is part of producing `selectionRange`), walked over the already-parsed CST — not a re-parse, not symbol re-extraction. Nameless decl (resilient parser on broken input) → falls back to the full `range` (always valid, never panics). All ranges go through L1's `position.rs` `LineIndex` in the negotiated encoding — **no second position converter** (the §11.32 DRIFT-2 boundary is untouched). (5) **`workspaceSymbol` (Doc 9 §9 / Doc 14 §2) — the L2 judgment call: left UN-advertised.** Doc 26 §9 made the cut "drop it from advertised capabilities in v1.2 OR back it single-file — an L2 judgment call recorded in Doc 00 §11." Decision: **not advertised** (no `workspace_symbol_provider` in `ServerCapabilities`). A single-file `workspace/symbol` is a misleading half-feature (the request is workspace-wide by contract; answering it from one file silently under-delivers — the cardinal silent-data-loss sin), and the true project index is explicitly v1.3 (Doc 26 §4.6/§9). An un-advertised capability is the honest signal; documentSymbol already gives the in-file outline. (6) **P3-1 (L1 phase-audit micro-fix).** `#![forbid(unsafe_code)]` added to `crates/fsm-lsp/src/main.rs` (the bin compilation root) — `lib.rs` already had it; a binary crate has two roots and the invariant is per-root (the `fsm-cli/src/main.rs:8` precedent). Inert (no `unsafe` in the 36-line arg parser) but makes the 10/10 forbid-unsafe claim true at *both* of the crate's roots. | L2 stays inside the extraction-first contract: it adds breadth (two read capabilities) on the *existing* forward substrate with **zero new analysis** — the one risk (silently running a 2nd analysis or copying the position math) is structurally avoided by threading the single `Analysis.symbol_table` and reusing `LineIndex`. The §5.4 client test asserts the full served tree (names/kinds/nesting + every range) byte-equal to the reused-pipeline oracle AND under both encodings on a multibyte line (a byte/scalar shim at the symbol layer fails it — the same risk-1 rigor L1 applied to diagnostics). The Doc-26-§5-vs-`StateShape` mismatch is exactly the prose-vs-code drift the project has been bitten by (P0-1 lineage); surfacing it precisely — the seam still works because `container_path` carries the hierarchy — rather than silently coding around an over-credited doc clause is the honest-surface standard (§11.19/§11.23/§11.32). Not advertising `workspaceSymbol` honours the no-silent-under-delivery bar over a tempting half-feature. | v1.2-LSP-L2 `phase2.24/v1_2-lsp-l2` |
| §11.34 | **v1.2-LSP-L3 implementation decisions (`hover` + single-file `definition`).** (1) **`ir` thread-through — ONE analysis feeds ALL consumers (the L2 pattern, extended).** L1+L2's `crates/fsm-lsp/src/analysis.rs::analyze` ran the exact `fsm check` pipeline and (L2) kept `result.symbol_table` but still discarded `result.ir` (an explicit "L3+ seam, not L2" comment). L3 adds an `ir: Option<Ir>` field to `Analysis` and stops dropping it — the **identical additive, behaviour-neutral** change L2 made for `symbol_table`: `analyze()` makes the *same* `analyze_with_source(&pr, …, src)` call in the same order and merges import diagnostics identically; it merely *keeps* a field it previously let fall on the floor. Hover (`server.rs::Backend::hover`) reads that threaded `ir` from the SAME `analyze()` the debounced `publishDiagnostics` / `documentSymbol` runs — **no second analysis pass, no parallel IR lowering** (Doc 26 §8 L2 "one analysis feeds both", widened to "all"; the invariant is preserved, not weakened). Verified behaviour-neutral: the L1 §5.4 client tests (byte-exact diagnostic-Range + `FSM-Exxxx` parity vs the `fsm check --json` oracle, incl. the non-ASCII both-encoding test) AND the L2 `documentSymbol`/`foldingRange` tests (full served tree + folds byte-equal under both encodings) all still pass **unchanged** (723 total, 701 baseline + 22 new). `fsm-ir` was added as an `fsm-lsp` path-dep (anticipated by Doc 26 §2.1's dependency table; default features, no `schema-validate`). (2) **Token-at-cursor + resolution seam — mirrors the analyzer, so a goto/hover can never disagree with a squiggle.** A new shared `crates/fsm-lsp/src/capabilities/resolve.rs` maps the cursor `Position` → byte via L1's `LineIndex` **inverse** (`position.rs::offset`, the reverse direction of the ONE authoritative converter — *not* a second/divergent one; the §11.32 DRIFT-2 boundary is untouched), then rowan's `token_at_offset` yields the `Ident` token, and its **node-ancestry `SyntaxKind` chain** classifies it **exactly as `checks::name_resolution` classifies the same token** when emitting `FSM-E0100..E0104` (the same `TRANSITION_DECL`/`LOCAL_DECL` ident-index split, `EXPR_FIELD_REF` ctx-vs-other split, `STMT_SEND` event-vs-machine split, `EXPR_CALL`-callee/bare-guard-name-ref extern, etc.). Resolution then goes through the **same** `SymbolTable::resolve_*` methods `name_resolution.rs` uses. Both `definition` and `hover` consume this one seam, so they are byte-consistent with each other and with the diagnostics (Doc 26 §3 invariant extended to L3) — there is no parallel resolver. Verified seam point (the recurring discipline): a **bare guard name-ref** like `[can_start]` is NOT E0102-checked by the analyzer (it only checks `EXPR_CALL`/`EXPR_FIELD_REF`), but `lower/expr.rs::lower_guard_expr`'s `NameRef` arm lowers it to `GuardExpr::ExternCall{callee,args:[]}` — it genuinely *is* a zero-arg pure-extern reference. Resolving a *declared* extern there can never disagree with a squiggle (the analyzer emits none for a valid one) and Doc 14 §6 explicitly wants "extern name in guard → extern declaration", so the seam resolves it; an *un*declared one still yields `None` (degradation, not a guess). (3) **Single-file graceful degradation — `null`/`None`, never a fabricated or cross-file location.** The per-file `SymbolTable` has no project index (Doc 26 §4.6/§9 — cross-file is explicitly v1.3). A symbol that does not resolve in this buffer (unknown name, `payload.`/enum operand, a would-be cross-file/import ref, a declaration site itself, whitespace, a keyword) yields `None` → the handler returns `null`. `definition`'s decl span is always a byte range of *this* parsed buffer, so the returned `Location.uri` is the request URI by construction — it can never point outside the file or at a synthesised location (the silent-data-loss cardinal sin). This is Doc 14 §8's own single-file degradation contract, shipped exactly. (4) **Hover content source — verified seam (Doc 26 §5 `hover` row).** Kind/name/decl come from the shared `resolve` seam. The **structured detail** (event payload field *types*, `pure`-extern signature with param/return types, context-field type + default *value*, a state's transitions-out count + entry/exit presence) is **not** in the `SymbolTable` — verified against `symbol_table.rs`: `MachineSymbols` carries names + `Span`s and only a *stringised primitive* `context_field_types`; it has **no** payload schema, no extern param/return types, no transition structure. The lowered **`Ir`** is the sole in-tree source (`EventObject.payload`, `ExternObject.{pure,params,return_type}`, `ContextField.{ty,default}`, the per-state `transitions`). So hover reads the threaded `Analysis.ir` for the detail — which matches Doc 26 §5's intent exactly; **here the §5 row's shorthand is accurate** ("`symbol_table` resolve_* + `Ir` (… payload types, extern signatures …)"), unlike the L2 `StateEntry.shape` over-credit (§11.33(3)). State composite-vs-simple is derived from the `container_path` graph the **same way L2's `documentSymbol` derives it** (§11.33(3)), not re-invented. `Ir == None` (catastrophic lowering failure) still yields a name+kind hover from the symbol table — degraded, never absent or wrong. (5) **Doc-14-§5-vs-code precision: the hover decl-site footer.** Doc 14 §5's state-hover *example* ends with `*Motor.fsm:12:3*`. The only in-tree `line:col` for a decl is `fsm_ir::SourceLocation.{line,column}`, produced by `fsm_analyzer::util::compute_line_col` — the **byte-counted, 1-based** converter Doc 26 §4.1 explicitly names NON-LSP-correct (the §11.32 DRIFT-2 converter). Emitting it verbatim would print a subtly wrong column on any non-ASCII declaration line and disagree with the LSP's own ranges. Disciplined resolution (the §11.33(3) derive-correct-meet-intent-flag-precisely discipline): the footer line/col is **derived from L1's authoritative `LineIndex`** over the decl span (the same converter every other LSP range uses), 1-based for the human-facing footer convention — meeting Doc 14 §5's *intent* (show where the symbol is declared) with the correct converter and keeping hover internally consistent. This is a **doc-vs-code precision** (Doc 14 §5's example implies the IR `SourceLocation` is usable as-is for an LSP-facing footer; it is not LSP-correct on multibyte lines), **NOT a defect** requiring a code or Doc-14 change — the footer is cosmetic Markdown text, not an LSP position, and the seam is satisfied by sourcing it correctly. Flagged here precisely; not silently worked around, not overclaimed. (6) **`rowan` added as an `fsm-lsp` path-dep.** Doc 26 §6 lists `cst::{SyntaxNode, SyntaxToken, …}` as the LSP's intended-public CST-traversal API; L3's token-at-cursor needs rowan's `TextSize` (byte→offset) + `TokenAtOffset` (the at-most-two tokens at a boundary) directly. Pinned via the workspace `rowan = "0.15"` (the same pin `fsm-parser` uses with `{ workspace = true }`) so the `SyntaxNode<FsmLanguage>` type is byte-identical to the trees `fsm-parser` produces (a divergent rowan would make the trees incompatible). | L3 stays inside the extraction-first contract: it adds breadth (two more capabilities) on the *existing* forward substrate with **zero new analysis** — the one risk (a 2nd analysis / 2nd lowering / 2nd position converter) is structurally avoided by threading the single `Analysis.ir` (exactly L2's `symbol_table` pattern) and inverting the one `LineIndex`. Routing `definition`+`hover` through one `resolve` seam that mirrors `name_resolution.rs`'s dispatch and reuses `SymbolTable::resolve_*` makes "a goto/hover can never disagree with a squiggle" structural, not aspirational. The §5.4 client tests assert definition `Location` ranges byte-equal to the analysis oracle, hover Markdown *structured* content (payload-field/signature/type lines, not substring flukes), the cross-file-`null` degradation, and correct position mapping under BOTH encodings on a multibyte fixture (a byte/scalar inverse fails it — the same risk-1 rigor L1/L2 applied). The bare-guard-name-ref seam point and the Doc-14-§5 footer precision are surfaced precisely per the §11.33(3) prose-vs-code discipline (P0-1 lineage) — derive-correct, meet the doc's intent, flag the imprecision, neither paper over nor overclaim a defect. | v1.2-LSP-L3 `phase2.25/v1_2-lsp-l3` |
| §11.35 | **v1.2-LSP-L4 implementation decisions (context-aware `completion`).** (1) **Context-classification reuse — L3's `resolve` substrate, NOT a parallel detector.** Doc 26 §8 L4 mandates reusing/extending L3's classifier. L3's `resolve.rs` answers "what declared entity *is* the ident under the cursor?"; completion needs the **dual** — "what may legally be typed *here*?" — often at a position that is NOT on an ident (`on |`, `-> |`, `[|`). The shared seam: three `resolve.rs` traversal helpers were promoted to `pub(crate)` and reused (NOT copied) — `enclosing` (the SAME node-ancestry walk L3 classifies with), `in_guard` (the SAME guard-context predicate), and one new sibling primitive `prev_significant_token` (nearest non-trivia token at/before the cursor — the rust-analyzer completion anchor, added alongside `resolve.rs`'s existing `ident_token_at`/`enclosing` family, same file, same traversal philosophy). `complete.rs::context_at` keys on the **preceding non-trivia token's `SyntaxKind` + the enclosing-node kind** — the SAME `TRANSITION_DECL`/`LOCAL_DECL` ident-index, `EXPR_FIELD_REF` ctx-prefix, `STMT_SEND`, `GUARD_CLAUSE` discriminators `resolve.rs::classify` uses, mirrored for the "next-token" question. The exact CST shapes at each incomplete cursor were **empirically verified** against the resilient parser (a throwaway probe, deleted pre-commit): `-> |`→prev `Arrow` under `TRANSITION_DECL`; `on |`→prev `KwOn` under `TRANSITION_DECL`; `[ |`→prev `LBracket` / inside `GUARD_CLAUSE`; `[ctx.|`→prev `Dot` whose LHS ident is `ctx` (recovers as `ERROR_NODE`); empty state body→prev `LBrace` under `STATE_DECL`; etc. The parser's day-one LSP-resilience (Doc 26 §4.4) makes every incomplete edit yield a usable tree, so this is robust on partial input — not a guess. (2) **Candidate sourcing — the ONE threaded `symbol_table`, no second analysis.** Every name candidate is read from `Analysis.symbol_table` (threaded since L2) of the SAME `analyze()` the diagnostics/`documentSymbol`/`hover` path runs (Doc 26 §3/§8 — `server.rs::completion` makes the identical reuse-seam call; no 2nd analysis pass, no 2nd position converter, no parallel context detector). Verified against `symbol_table.rs`: `MachineSymbols` exposes ordered `events`/`externs`(+`extern_pure`)/`consts`/`enums`(+`variants`)/`context_fields`(+`context_field_types`)/`states`, plus file-level `file_consts`/`file_enums`/`file_externs`(+`file_extern_pure`). The cursor's machine is resolved **by NAME via `machine_index`** — the SAME map `resolve.rs`/`resolve_machine` use, so candidate lists are correct regardless of declaration order. Guard context offers **`pure` externs only** (Doc 04 §2.5 — a guard may call only pure externs; the `extern_pure`/`file_extern_pure` parallel bit gates this); action context offers all externs + the action-statement keywords. (3) **Keyword source — Doc 04 §1.5 verbatim, pinned by a test.** `DSL_KEYWORDS` is a verbatim transcription of Doc 04 §1.5 (the single normative registry; Doc 14 §4 forbids inlining a divergent list), with `dsl_keywords_match_doc04_s1_5` asserting exact set-equality AND that every contextual keyword §1.5 deliberately keeps OUT (`entry`/`exit`/`events`/`queue`/`if`/`while`/`for`/`entry_point`/`exit_point`/`likely`/`rare`/`fsm`) is absent — so a future §1.5 addition that misses this file fails CI. The per-position keyword views (`TOP_LEVEL`/`MACHINE_BODY`/`STATE_BODY`/`ACTION`) are asserted SUBSETS of `DSL_KEYWORDS` (`keyword_subsets_are_subsets_of_doc04`), not redefinitions. (4) **Trigger characters — Doc 14 §2 verbatim, verified not guessed.** `completion_provider.trigger_characters = [".", ":", "@", "[", " "]` is the EXACT set in Doc 14 §2's `ServerCapabilities` block (cross-checked against the spec, the recurring discipline). `resolve_provider:false` — L4 returns fully-resolved items; advertising a `completionItem/resolve` it does not implement would be the stub-a-no-op sin (the `workspaceSymbol`-left-unadvertised precedent). The Doc 14 §4 keyword snippets (`machine`/`composite`/`parallel`, `insertTextFormat:2`) and the empty-state-body editor snippets (kind 15) are emitted **verbatim from Doc 14 §4** — only the ones the spec writes out; no invented snippets. (5) **Doc-26-§5-seam-vs-code precision: `payload.` is IR-only, NOT a `symbol_table` lie.** Doc 26 §5's `completion` row shorthand lists candidate sources as "events/states/externs/`ctx.`/**`payload.`**". Verified vs `symbol_table.rs`: the table genuinely provides the first four (events/states/externs/ctx-fields — **these four §5 sub-claims are ACCURATE as written**, like §11.34 found the §5 hover row accurate), but it carries **no per-event payload schema** — `payload.<field>` is event-specific and lives only in the lowered `Ir::EventObject.payload` (exactly the IR-vs-symbol_table boundary §11.34(4) established for hover). L4 therefore scopes `payload.`-member completion OUT (it is an IR-sourced concern, Doc 26 §5 itself only lists `payload.` for the future inlay/snippet seam) rather than half-wiring it from a table that does not hold it. This is a **Doc-26-§5 shorthand imprecision on the `payload.` token** — precisely analogous to L2's `StateEntry.shape` over-credit (§11.33(3)) and resolved the same disciplined way: derive-correct (the four real classes shipped + the `Ir`-only one deferred), meet the doc's intent, flag the imprecision, **NOT a code defect** and **NOT requiring a Doc-26 change** (the row is intent-shorthand; §5 already routes payload detail through the IR for hover). Flagged precisely; not silently worked around, not overclaimed; the accurate sub-claims explicitly affirmed (verify EACH, not assume-all-wrong — the §11.34/L3 precedent). (6) **Wrong-context exclusion is structural, not best-effort.** `context_at` returns exactly one `CompletionContext`; each emits ONLY its candidate-class set, so an event in a transition-target / a state after `on ` / a keyword in a guard / an impure extern in a guard is *structurally* unreachable. An unclassifiable cursor (inside a comment, mid-punctuation) → `CompletionContext::Unknown` → an **empty** list, never a dump of every symbol (offering wrong-context noise is silent-mislead-adjacent — a cardinal-sin-adjacent UX failure the §5.4 negative asserts guard against). | L4 stays inside the extraction-first contract: breadth (one more capability) on the *existing* forward substrate with **zero new analysis** — the one risk (2nd analysis / 2nd converter / **parallel context detector**) is structurally avoided by (a) reading the single threaded `symbol_table`, (b) the caller mapping the cursor via the one `LineIndex::offset` (no `TextEdit` ranges emitted at all), (c) the classifier reusing L3's `resolve` ancestry/guard helpers + one new sibling primitive in the SAME file rather than a fresh ad-hoc detector. The §5.4 client tests assert, per representative context (transition-target / `on `-trigger / guard-expr / statement-start), the served completion **set** (labels + `CompletionItemKind`) equals the analysis-oracle set AND ≥1 wrong-context candidate that exists in scope is ABSENT (both load-bearing), plus a non-ASCII (🚀+Cyrillic comment trivia — multibyte only in lexer-valid trivia, the prior-wave Cyrillic-identifier lesson) fixture exercised under BOTH `positionEncoding`s with the per-encoding-divergent cursor column hard-coded (UTF-8 46 vs UTF-16 36, Δ10) so a byte/scalar position shim fails it — the same risk-1 rigor L1/L2/L3 applied. The `payload.` Doc-26-§5 shorthand imprecision is surfaced precisely per the §11.33(3)/§11.34 prose-vs-code discipline (P0-1 lineage); the four accurate §5 sub-claims are explicitly affirmed. | v1.2-LSP-L4 `phase2.26/v1_2-lsp-l4` |
| §11.36 | **v1.2-LSP-L5 implementation decisions (`references` + `prepareRename`/`rename`; the ONE new analysis; risk-2).** (1) **`ReferenceIndex` construction — derived from the ONE analysis, semantic-only, conservative-by-construction.** `crates/fsm-lsp/src/refs.rs::ReferenceIndex::build` is a **single CST walk** over the outputs of the *one* `analyze()` the diagnostics/symbol/hover/completion path already runs (its `symbol_table` + the parsed `cst`) — it runs **no** `analyze()`, lowers **no** IR, adds **no** parallel resolver (Doc 26 §4.7, verbatim: "a single CST walk (not a re-analysis) … It reuses `SymbolTable` for the resolution decision … not a parallel analyzer"). A token is admitted to a symbol's reference set IFF it is a CST `Ident` AND either (a) it is that symbol's **declaration-name token** (the first `Ident` within the decl `Span` the `SymbolTable` recorded — the exact `ast::first_ident` rule the table builder used to derive the name, so name and token are byte-identical), or (b) the SAME L3 `resolve_at` (the §11.34 classifier mirroring `checks::name_resolution`, resolving through the SAME `SymbolTable::resolve_*` `fsm check` uses) resolves it to *exactly that declaration*. Symbol identity is a `SymbolKey` = `(kind, machine-index-or-none, declaration `Span`)` — the decl `Span` uniquely identifies a declaration in one parsed buffer; two tokens are the same symbol IFF equal `SymbolKey`. This is **never** a name-string comparison. Excluded **by construction** (no code path could include them): a token inside a string literal / comment / trivia is a `StringLiteral`/`LineComment`/`BlockComment`/`DocComment`/`Whitespace`/`Newline` token, never an `Ident` in a position `resolve_at` classifies; a same-spelled token in a different machine resolves — via `resolve_at` using the correct enclosing-machine index the SAME way `resolve_machine` does — to a different decl `Span` → a different `SymbolKey` → a different bucket; anything `resolve_at` returns `None` for (keywords, punctuation, `payload.`/enum operands, unknown/cross-file names) is not a reference to anything. A missed reference is an annoyance; a wrong edit is catastrophic (the project's silent-data-loss cardinal sin in its most acute form, Doc 26 risk-2) — they are NOT symmetric, so when semantic resolution cannot *prove* a token is the target it is excluded. (2) **rename-safety design — exhaustive `prepareRename` rejection set + `rename` exclusion-by-construction + collision + cross-file stance.** `prepareRename` (`refs.rs::prepare_rename`) resolves the cursor purely semantically via `ReferenceIndex::key_at` (a resolvable use site OR a decl-name token — nothing else), then **HARD-REJECTS up-front** (a JSON-RPC error, the LSP contract — never a silent allow that becomes a corrupting edit): a non-resolvable cursor → `NotARenameableSymbol` (whitespace/keyword/contextual-keyword/punctuation/**string interior**/**comment**/trivia/an **`@id`/state-id annotation** string are none of "a use site or decl-name token", so structurally refused — not special-cased); a **machine name** → `MachineNameOutOfScope` (codegen/ABI blast radius — generated C type/function names + cross-file `send … to M` — beyond v1.2's single-file scope, Doc 14 §8 / Doc 26 §8 L5; an *informative* refusal, not a blank "not a symbol"); a cross-file-exposed symbol → `CrossFileExposed` (Doc 14 §8's verbatim sanctioned string). `rename` (`refs.rs::plan_rename` → `capabilities/rename.rs`) emits a `WorkspaceEdit` whose edits are **EXACTLY** `ReferenceIndex::occurrences(key)` — decl + semantically-resolved uses, each the bare identifier `Span` (never the wider `state X { … }` decl span). It additionally rejects an invalid new identifier (`is_valid_identifier` — a name the lexer would not tokenise as one `Ident` must never be written, it would corrupt the parse) and an **in-scope name collision** (`collides` — renaming INTO an existing same-category same-machine name would silently merge/shadow two distinct symbols, the same corruption class) — both with a clear message and **NO edit**. (3) **Cross-file stance — single-file, conservative, the v1.3 plug-in point named.** Per Doc 26 §4.6/§9 the per-file `SymbolTable` has no project index; cross-file references/rename are explicitly v1.3. In the v1.2 single-file model the ONLY construct that exposes a *non-machine* symbol across a file boundary is — none: events/states/externs/context fields are all machine-local (`import` only affects the security pass, never the per-file `SymbolTable`; `check.rs:184`), and machine is rejected by kind before cross-file is consulted. So `cross_file_exposed()` is `false` for every *renameable* kind by construction in v1.2; it is kept as the explicit, audited statement of that boundary and the named seam where v1.3 cross-file detection plugs in — not an unstated assumption. (4) **Doc-26-§4.7/§5-seam-vs-code: VERIFIED ACCURATE, with one precision + one boundary finding (the §11.33(3)/§11.34/§11.35 discipline — verify EACH, affirm the accurate, flag precisely, don't overclaim).** Doc 26 **§4.7's core claim is ACCURATE and is the foundation of this wave**: "It reuses `SymbolTable` for the resolution decision (so 'references' means *semantically resolved* references, not text matches) … it cannot drift from `fsm check` because resolution still goes through `SymbolTable`" — implemented exactly. Doc 26 **§5's two L5 rows are ACCURATE as written**: `references | refs.rs ReferenceIndex (§4.7), resolution via symbol_table` and `rename + prepareRename | ReferenceIndex → in-file WorkspaceEdit; entity-kind gate from symbol_table` — explicitly affirmed (not assume-all-wrong). **Precision (not a defect, not a Doc-26 change):** §4.7 enumerates the reference token kinds to walk ("state ref after `->`/`~>`/`initial`/fork/join; event in `on`/`raise`/`send`/`defer`; extern call; `ctx.field`; machine in `send … to M`"). Re-deriving that list in `refs.rs` would be a *second* classifier — precisely the parallel-resolver anti-pattern §4.7 itself forbids. L5 instead routes every `Ident` through L3's existing `resolve_at` (which already mirrors `name_resolution.rs` for exactly those constructs, §11.34). The §4.7 enumeration is accurate-as-intent; the implementation deliberately *reuses the one L3 classifier* rather than re-listing the kinds — a derive-correct that **strengthens** the doc's "not a parallel analyzer" intent (one classifier, not two). **Boundary finding (conservative, flagged precisely):** `resolve_at` resolves a file-level-`extern` *use* to `Resolved::Extern{ machine: <the using machine>, decl: <file-extern decl Span> }`, so the decl `Span` is the stable identity and use sites are keyed correctly; but a file-extern's *declaration-name token* (an `extern` declared OUTSIDE any machine) has no enclosing machine, so `collect_decl_name_tokens` does NOT fabricate a machine index for it (a guessed identity would be the wrong-edit sin). Consequence: a file extern is renameable through its resolved (use-keyed) identity; a cursor *on the file-extern declaration itself with zero uses* declines rather than guess — the conservative single-file boundary, flagged here precisely, NOT a defect (no fabricated identity, no guessed range; missed-rename ≪ wrong-edit). (5) **One analysis, one converter, one resolver — reused not duplicated; L1–L4 byte-unchanged.** `server.rs` builds the `ReferenceIndex` from the SAME single `analyze()` the diagnostics path runs; ranges go through L1's authoritative `LineIndex` (no second position converter — the §11.32 DRIFT-2 boundary untouched); resolution is L3's `resolve_at` (no parallel resolver). Verified behaviour-neutral: every L1 (diagnostic byte-Range/code parity, both encodings) + L2 (documentSymbol/folding) + L3 (hover/definition) + L4 (completion) in-process client test passes **unchanged** (755 total = 739 baseline + 16 new L5: 9 unit + 7 acceptance), `fsm test examples/` 5/5, `tests/conformance/` 25/25 — the analysis stayed behaviour-neutral. `#![forbid(unsafe_code)]` intact at BOTH `fsm-lsp` roots (workspace stays 10/10). | L5 is the highest-risk wave: `rename` rewrites the user's source, so a wrong edit silently corrupts their program — Doc 26 risk-2, the cardinal sin in its most acute form. The whole design is conservative-by-construction: making "reference" mean *semantically resolved through the one L3 classifier + `SymbolTable::resolve_*`* (never a text match) makes a same-spelled string/comment/different-scope token **structurally unable** to enter the `WorkspaceEdit` — the safety is in the data model, not best-effort filtering. Deriving the index from the single analysis (no 2nd pass) and reusing L3's `resolve_at` (rather than re-listing §4.7's token kinds — which would be the very parallel resolver §4.7 forbids) keeps the extraction-first / one-analysis invariant and makes "a reference can never disagree with a goto/diagnostic" structural. The §5.4 client matrix is the most exhaustive of any wave because rename-safety demands it: the headline risk-2 test asserts the edit set is EXACTLY the semantic references AND that a same-spelled string-literal substring, comment word, AND a different-scope same-spelled symbol are NONE in the `WorkspaceEdit` (CST-derived forbidden regions → fixture-edit-proof; applying the edits leaves the other machine + comment + string byte-preserved and the buffer still parses clean — the silent-corruption guard made unmissable), plus a six-case `prepareRename` negative matrix (machine/`@id`/keyword/string/comment/whitespace each individually rejected) and all reference/rename ranges correct under BOTH `positionEncoding`s on a multibyte fixture (a byte/scalar shim fails it — the same risk-1 rigor L1–L4 applied). Doc 26 §4.7/§5's accurate rows are explicitly affirmed and the two findings (the reuse-the-one-classifier precision; the file-extern decl-cursor boundary) surfaced precisely per the §11.33(3)/§11.34/§11.35 prose-vs-code discipline (P0-1 lineage) — derive-correct, meet intent, flag imprecision, neither paper over nor overclaim a defect. | v1.2-LSP-L5 `phase2.27/v1_2-lsp-l5` |
| §11.37 | **v1.2-LSP-L6 implementation decisions (`semanticTokens/full` + `/range`; no new analysis; no parallel classifier).** (1) **Legend — derived from Doc 14 §10 + the real `fsm-lexer` `SyntaxKind`s + `symbol_table`, declared ONCE, reused for advertise+encode.** `crates/fsm-lsp/src/capabilities/semantic_tokens.rs::legend()` is the EXACT Doc 14 §2 `semanticTokensProvider.legend` array: 11 token types `[namespace,type,enum,function,variable,keyword,string,number,operator,comment,decorator]` + 4 modifiers `[declaration,readonly,deprecated,static]` (Doc 14 §10's index table). Cross-checked that every type has a real producer: `keyword`/`operator`/`number`/`string`/`comment`/`decorator` from concrete `fsm-lexer` token `SyntaxKind`s (the dense `Kw*` block; the `Arrow`/`HistoryArrow`/`Colon`/`Eq`/comparison/`LBracket`/`RBracket`/… operators; `IntLiteral`/`FloatLiteral`; `StringLiteral`; `LineComment`/`BlockComment`/`DocComment`; `StableId`/`At`), and `namespace`/`type`/`enum`/`function`/`variable` from the `symbol_table` via the reused L3/L5 classifiers. `legend()` is the SINGLE source: `server.rs`'s `semantic_tokens_provider` advertises it AND the encoder maps to its indices, so the advertised `tokenType`/`tokenModifiers` and the encoded integers cannot diverge (the §5.4 test asserts advertised==encoder==Doc-14-order). (2) **Classification — reuse L3 `resolve_at` (uses) + L5 `collect_decl_name_tokens`/`SymbolKey` (decls); ONE classifier, NO parallel one, NO 2nd analysis.** Every `Ident` is classified by: (a) decl-name-token membership via the SAME `crate::refs::collect_decl_name_tokens` L5's `ReferenceIndex` uses (promoted to `pub(crate)`, the SAME promote-and-reuse the L4 wave applied to `resolve.rs`'s helpers) → entity type + the `declaration` modifier; else (b) L3's `crate::capabilities::resolve::resolve_at` (the §11.34 classifier mirroring `checks::name_resolution`, resolving through the SAME `SymbolTable::resolve_*` `fsm check` uses), bridged by the new `refs::resolved_entity_kind` which routes through the SAME `key_of` `Resolved`→`SymbolKey` bridge L5 uses → entity type, NO `declaration`. A new `pub(crate) EntityKind` (next to `SymbolKey`, with `SymbolKey::entity_kind()`) is the ONE taxonomy both paths funnel through, so a decl and a use of one symbol get the IDENTICAL legend type and differ ONLY by Doc 14 §10 modifier 0 — exactly the Doc 26 §8 L6 decl-vs-ref requirement, decided by `symbol_table` identity, never re-classified. `server.rs` threads the `symbol_table`+`cst` of the SAME single `analyze()` the diagnostics/symbol/hover/completion/references path runs — NO second analysis pass, NO parallel classifier (Doc 26 §3/§8). (3) **Delta encoding + negotiated units via the ONE `position.rs` `LineIndex`.** Output is the LSP relative array `[deltaLine,deltaStartChar,length,tokenType,tokenModifiers]`, position-sorted, deltas vs the previous token, `deltaStartChar` RESET on a new line. `deltaStartChar`/`length` are in the NEGOTIATED `positionEncoding` unit, computed via L1's authoritative `LineIndex::position` (no second converter — the §11.32 DRIFT-2 boundary untouched). LSP `multilineTokenSupport` defaults off, so a multi-line `BlockComment`/`DocComment` is split into ONE token per line (the rust-analyzer model — a line-spanning token is rejected/mis-rendered by clients). `semanticTokens/range` is a self-contained substream: classification is filtered to byte-overlapping tokens and the relative encoding is RECOMPUTED for the subset (first token's deltas relative to the response start, per the LSP contract — not a slice of the full stream). Doc 26 §8 L6 explicitly specifies "full + range"; Doc 14 §2's block has `"full": true, "range": true` — both advertised, both genuinely implemented (NOT advertised-but-stubbed). (4) **Doc-14-§10/Doc-21-vs-code: VERIFIED, two precisions, NO defect, NO doc change (the §11.33(3)/§11.34/§11.35/§11.36 discipline — verify EACH, affirm the accurate, flag precisely, don't overclaim).** Doc 14 §10's type table is ACCURATE and implemented verbatim (machine→namespace, state decl+ref→type, event→enum, extern→function, ctx/payload→variable, keywords→keyword, strings/stable-ID→string, int/float→number, `-> ~> : = [ ] && ||`/comparison→operator, comments→comment, `@id(...)`→decorator) — explicitly affirmed. **Precision A (Doc 14 §10 imprecision, not a defect):** index 9's row reads "Line comments, block comments" and index 6's row separately lists "doc comment text" under `string`. In the REAL `fsm-lexer` a `///` line is a SINGLE `DocComment` comment-class trivia token (`SyntaxKind::is_trivia()` true) — its text is NOT separately tokenised, so there is no "doc comment text" sub-token to colour as a string. Derive-correct: the whole `DocComment` token → index 9 `comment` (its lexical kind, consistent with line/block comments). A doc comment IS a comment; fabricating a sub-token the single-token lexer never emits would be the silent-fabrication sin. Flagged precisely; NOT a code defect, NOT a Doc-14 change. **Precision B (Doc 14 §10 modifier 2 has no in-tree source):** modifier 2 `deprecated` ("reserved for future use") is advertised in the legend (the spec lists it; a client may rely on the legend shape) but NEVER emitted — no `SymbolTable`/IR field records a deprecated stable ID, and emitting a modifier with no analysis backing is a fabrication. Declared-not-emitted is the honest position (parallels L2's advertised-not-stubbed discipline). Flagged; NOT a defect. **Doc 21 (TextMate) is a DRAFT cross-checked, not obeyed:** Doc 21 §2's richer scope set (`entity.name.type.state.fsm`, `support.type.event.fsm`, `variable.other.enummember.fsm`, `keyword.operator.arrow.fsm`, …) is, by LSP design, COARSER-mapped into the 11 Doc 14 §10 legend types (semantic tokens refine, they are not 1:1 with TextMate scopes); Doc 21 §6 itself DEFERS the legend to "FSM-SPEC-LSP §10" and states the coexistence model (TextMate for un-analyzed/startup regions, semantic tokens refine analyzed ones) — so there is NO Doc-21-vs-Doc-14 legend CONFLICT to resolve, only the expected scope-superset→legend-subset mapping, which is implemented and noted (Doc 21 accurate-as-intent, affirmed). **Boundary (deliberate, flagged):** structural punctuation Doc 14 §10 gives NO legend slot (`{ } ( ) ; , .`, whitespace, `Error`/`Eof`) is NOT emitted — semantic tokens only refine legend-covered constructs; the client uses the Doc 21 TextMate scope for those (Doc 21 §6 coexistence). `payload.X`/enum `Type.Variant` (which `resolve_at` deliberately degrades to `None` — the analyzer resolves neither to one decl, §11.34/§11.35 IR-vs-symbol_table boundary) are still assigned Doc 14 §10's slots (payload field→`variable`+`readonly` mod 1; enum variant→`enum`+`static` mod 3; the lhs/rhs split MIRRORS `name_resolution.rs:388-431` exactly — same structural rule, NOT a parallel classifier), because semantic tokens are MORE precise than TextMate and Doc 14 §10 explicitly assigns them. Contextual keywords the lexer emits as `Ident` (`entry`/`exit`/`events`/`queue`/`if`/`while`/`for`/`entry_point`/`exit_point`/`fsm`/`likely`/`rare`) → index 5 `keyword` (Doc 14 §10 index 5 = "ALL FSM-Lang keywords"), decided by the enclosing-node `SyntaxKind` the parser/`resolve.rs` use (NOT an ident-text allow-list — a symbol legitimately *named* `events` is already classified by `resolve_at`/the decl-name pass first, so the node-context path is reached only for a true contextual-keyword use). (5) **One analysis, one converter, one classifier — reused not duplicated; L1–L5 byte-unchanged.** Verified behaviour-neutral: every L1 (diagnostic byte-Range/code parity, both encodings) + L2 (documentSymbol/folding) + L3 (hover/definition) + L4 (completion) + L5 (references/rename, incl. the risk-2 core + both-encoding non-ASCII) in-process client test passes **unchanged** (766 total = 755 baseline + 11 new L6: 7 unit + 4 acceptance; the L5 reuse-promotion adds NO test, only `pub(crate)` visibility + additive helpers), `fsm test examples/` 5/5, `tests/conformance/` 25/25 — `refs.rs`'s only change is making `collect_decl_name_tokens` `pub(crate)` + adding `EntityKind`/`entity_kind()`/`resolved_entity_kind` (purely additive, the L5 reference-index behaviour is byte-identical). `#![forbid(unsafe_code)]` intact at BOTH `fsm-lsp` roots (workspace stays 10/10). | L6 stays inside the extraction-first contract: breadth (semantic tokens, the most precise read capability) on the EXISTING substrate with **zero new analysis** — the one risk (a 2nd analysis / 2nd position converter / **parallel classifier**) is structurally avoided by (a) threading the single `analyze()`'s `symbol_table`+`cst`, (b) measuring delta units via the one `LineIndex`, (c) the classifier REUSING L3's `resolve_at` (uses) + L5's `collect_decl_name_tokens`/`SymbolKey` taxonomy (decls) through one shared `EntityKind` rather than a fresh ad-hoc token classifier — the SAME promote-and-reuse L4 applied. The §5.4 client tests DECODE the relative array back to absolute `(line,char,len,type,mods)` tuples and assert the FULL decoded stream equals the reused-pipeline oracle (a delta-math regression — wrong deltaLine/deltaStartChar/length, or a non-reset deltaStartChar on a new line — makes the decode wrong → fails; symbol presence is NOT acceptance), with hard-coded cross-checks (state decl carries `declaration`, a state use does not, a `ctx.field` is the field type, a comment is `comment`, a keyword is `keyword`, the `@id` decorator + its string), the advertised legend asserted == Doc 14 §2/§10 order == the encoder's, the range response asserted self-contained, AND a non-ASCII fixture (🚀+Cyrillic ONLY in lexer-valid block-comment trivia — the standing Cyrillic-identifier lesson) under BOTH `positionEncoding`s with decoded positions/lengths asserted correct in each (a byte-vs-UTF-16 mismatch makes the decoded position wrong → fails — the same risk-1 rigor L1–L5 applied, at the delta layer). The Doc-14-§10 doc-comment-text + reserved-`deprecated` precisions and the Doc-21-is-a-coarser-superset finding are surfaced precisely per the §11.33(3)/§11.34/§11.35/§11.36 prose-vs-code discipline (P0-1 lineage) — derive-correct, meet intent, flag imprecision, affirm the accurate rows, neither paper over nor overclaim a defect. | v1.2-LSP-L6 `phase2.28/v1_2-lsp-l6` |
| §11.38 | **v1.2-LSP-L7 implementation decisions (`codeAction` + `inlayHint`; the FINAL LSP wave; edit-producer = risk-2 discipline; no new analysis).** (1) **`codeAction` quick-fix verification — Doc 14 §9 vs the REAL `DiagnosticCode`s + emission sites; only the PROVABLY-MECHANICAL ship, the rest scoped-out-and-flagged (the brief's bias-hard-to-safety mandate; an edit-producer inherits L5 risk-2).** A wrong quick-fix silently rewrites the user's source = the cardinal silent-data-loss sin. Each Doc 14 §9 row was verified against `fsm-diagnostics`/`fsm-analyzer` at this HEAD: **(SHIP) FSM-E0107** "no initial" — emitted `name_resolution.rs:43` with span = *exactly* `span_of(MACHINE_DECL)`; the fix inserts `\n    initial <FirstState>` at the byte after the machine's `{` (a direct CST child token), `<FirstState>` = `MachineDecl::states().next()` (the SAME first-state the E0107 check counts at `name_resolution.rs:41`); one insertion, name from the parsed CST, position from the CST `{` → provably mechanical; re-analysis clears E0107 with **no new diagnostic** (exactly one valid `initial` naming a declared state ⇒ no E0100/E0108). **(SHIP, guarded) FSM-E0022** "duplicate event" — emitted `symbol_table.rs:300` via `push_entry` with span = *exactly* `span_of(EVENT_DECL)` of the **second** (duplicate) declaration (the first is kept); events are whitespace-separated (`grammar/machine.rs:151` `events_block = "{" , { event_decl } , "}"` — NO separator token) and the `EVENT_DECL` node range subsumes its own trailing trivia, so deleting that exact span is a provably-correct deletion (first decl + every `on EVENT` ref untouched; re-analysis clears E0022 with no new diagnostic — empirically verified). **Guard:** `try_parse_stable_id` parses an `@id(...)` as a *separate `STABLE_ID_ANNOT` sibling* BEFORE the `EVENT_DECL` (verified); if the duplicate's nearest preceding non-trivia sibling is a `STABLE_ID_ANNOT`, deleting only the `EVENT_DECL` orphans a dangling `@id` → NOT mechanical, so the action is **withheld** for that case (a missing quick-fix is a minor UX gap; a corrupting one is the cardinal sin). **(SCOPE OUT — flagged here) the other 5 Doc 14 §9 codes + BOTH `refactor.extract` actions:** `FSM-W0200` "loop in action" and `FSM-W0500` "unused extern" are **NEVER emitted anywhere in the toolchain** (catalog-reserved-only — verified zero emission sites; no diagnostic could carry them and the §5.4 apply-and-verify could not even be written — dead by construction); `FSM-E0100` "unknown state" is emitted at 14 heterogeneous sites (transition/branch/`initial`/enum-operand) so "Create state X" would need a name lifted from a non-uniform spanned expression + a placement choice — not a single unambiguous mechanical edit; `FSM-E0106` "non-pure extern in guard" spans the *call site* not the extern decl, and flipping an extern to `pure` is a **semantic assertion about the user's foreign code the tool cannot prove** (a genuinely-impure extern wrongly marked `pure` silently corrupts generated guard evaluation — worse than a syntax error); `FSM-E0300` "nondeterminism" spans only ONE transition (`determinism.rs:340,359` `b.span`) but the fix is a *multi-transition coordinated* edit whose correct set requires re-deriving the determinism grouping = a parallel re-analysis the brief forbids, and the priority-clause insertion position is non-trivial; the two `refactor.extract` actions are non-mechanical semantic refactors (extern signature inference / parameter capture / ABI) with no extraction substrate in-tree (Doc 26 §5 itself only sources them as "AST for refactor.extract"). Per the brief, **every one of these is a reported scope-out, not a silent omission** — bias hard to safety. Net: **2 provably-mechanical quick-fixes shipped, 5 §9 codes + 2 refactor.extract scoped out with a documented safety reason.** The fix is matched on the **server-authoritative native `DiagnosticCode`** (not the client-supplied `context.diagnostics`) and tied back to the LSP `Diagnostic` it resolves (`action.diagnostics`); every edit `Range` is via L1's one `LineIndex` (no string munging — the §11.32 DRIFT-2 boundary intact). (2) **`inlayHint` source — Doc-26-§5's narrowing is authoritative over Doc 14 §11's five rows (the §11.33(3)/§11.34/§11.35/§11.36/§11.37 prose-vs-code discipline — verify EACH, affirm the accurate, flag precisely, don't overclaim).** Doc 14 §11 lists 5 hint rows; **Doc 26 §5's `inlayHint` row is the authoritative scoping**: source = "`Ir` (non-default priority, timer durations, state child counts) + Doc 22 §8 toggles" — exactly THREE families, and Doc 22 §8 provides exactly three matching per-category toggles. **Doc 14 §11's row 5 (extern-param-names `// (ctx, payload)`) is SCOPED OUT + flagged:** it is NOT in Doc 26 §5's enumerated IR-sourced set AND has **no Doc 22 §8 toggle** (only the master + the three above); shipping it would be inventing an un-toggleable hint kind, contradicting Doc 26 §8 L7's "don't invent hint kinds". This is a **Doc-14-§11-vs-Doc-26-§5/Doc-22-§8 seam precision** (Doc 14 §11's 5-row list is intent-shorthand; Doc 26 §5 + Doc 22 §8 — the authoritative architecture + the actual config surface — narrow it to 3) — **NOT a code defect, NOT requiring a Doc-14 change**: derive-correct to the authoritative narrowing, flag precisely, and the three IR-sourced rows Doc 26 §5 DOES enumerate are implemented verbatim and **explicitly affirmed accurate** (non-default `TransitionObject.priority`; `TimerObject.duration_ms` human form; `Composite`/`Parallel` region-`states` count). Each is gated by its Doc 22 §8 toggle AND the master `enableInlayHints`. **Position precision (not a defect):** `TransitionObject.loc.span`/`TimerObject.loc.span` are `span_of(node)` and **subsume trailing trivia** (a transition span ends `…-> Fast\n        ` — past the meaningful text), but Doc 14 §11 wants the hint "After `… -> Fast;`" — so the priority/timer hint anchors at the **end of the last non-trivia token within the `loc.span`**, found by a bounded CST token scan over the already-parsed tree (the SAME locate-a-token-within-an-analysis-span technique L2's `selectionRange` uses, §11.33(4) — NOT a re-parse, NOT a second analysis); the substate hint anchors just after the state's first `{` child token. The timer-lowering implicit `priority 0` on `after`/`every` transitions is **excluded** from the priority hint (only `priority != 100` on `Event`/`Completion` triggers — a user-written clause, matching Doc 14 §11 row 1's `on START …` example); the hint must reflect what the user wrote, not a lowering artifact. (3) **Config plumbing — minimal behaviour-gating slice, Doc 26 §7 open-question 8.** A new `crate::config::InlayHintConfig` parses ONLY the four Doc 22 §8 inlay toggles (with the Doc 22 §8 defaults: master/priorities/timers ON, state-types OFF) from BOTH the `initialize` `initializationOptions` AND `workspace/didChangeConfiguration` `settings` (Doc 14 §3 "on startup and on `didChangeConfiguration`"); the parse is defensive (a missing/mistyped key keeps its Doc 22 §8 default — never a silent flip, never a panic) and reads both the nested-VS-Code and flat-dotted shapes. Every OTHER `fsmLang.*` key is stub-accepted (ignored — the open-question-8 decision); this is the minimal behaviour-correct slice, NOT a config subsystem. (4) **One analysis, one position converter — reused not duplicated; L1–L6 byte-unchanged.** `server.rs::code_action`/`inlay_hint` each run the SAME single `analyze()` the diagnostics path runs (the reuse seam, Doc 26 §3 — NO second analysis pass, NO second lowering); `codeAction` projects its diagnostics through the SAME `to_lsp_diagnostics`, `inlayHint` reads the SAME threaded `Analysis.ir`; all positions/ranges go through L1's ONE authoritative `LineIndex` (no second converter — the §11.32 DRIFT-2 boundary untouched). Verified behaviour-neutral: every L1 (diagnostic byte-Range/code parity, both encodings) + L2 (documentSymbol/folding) + L3 (hover/definition) + L4 (completion) + L5 (references/rename incl. risk-2 core) + L6 (semanticTokens, both encodings) in-process client test passes **unchanged** (785 total = 766 baseline + 19 new L7: 14 unit (5 config + 4 code_action + 5 inlay_hints) + 5 acceptance), `fsm test examples/` 5/5, `tests/conformance/` 25/25 (the `fsm` binary does not even depend on `fsm-lsp`; L7 is purely additive to that crate). `#![forbid(unsafe_code)]` intact at BOTH `fsm-lsp` roots (workspace stays 10/10). After this wave the v1.2 LSP capability set is **feature-complete** (L1–L7 all shipped + phase-audited at L1/L5). | L7 is the second edit-producing wave (after L5 `rename`): a wrong quick-fix silently corrupts the user's source — Doc 26 risk-2, the cardinal sin. The whole `codeAction` design is bias-hard-to-safety: a fix ships ONLY when the analysis already proved an exact span (E0107 = `span_of(MACHINE_DECL)`, E0022 = `span_of(EVENT_DECL)` of the duplicate) AND the edit is a single mechanical insert/delete that re-analysis proves clears the diagnostic with no new one — and the E0022 stable-id guard withholds the fix exactly where the edit stops being mechanical. The 5 §9 codes + 2 refactor.extract that cannot be safely mechanized are scoped OUT and flagged here (W0200/W0500 are provably dead; E0100/E0106/E0300/extract need a heuristic or a forbidden parallel re-analysis) rather than shipping a possibly-corrupting edit to hit a feature list — a missing quick-fix is a minor UX gap, a wrong edit is a catastrophe. The §5.4 client tests apply L5's apply-and-verify rigor to the edit-producer: the headline test requests `codeAction` for FSM-E0107, asserts the served `WorkspaceEdit` byte-equals the reused-pipeline oracle, **applies it**, and asserts (i) FSM-E0107 resolved, (ii) NO new diagnostic, (iii) it was a pure splice that left every other byte identical, (iv) the buffer still parses; a no-bogus-action negative proves a scoped-out diagnostic (E0100) yields NO action; the inlayHint test asserts the served set == the analysis oracle with hard-coded Doc-14-§11 family cross-checks + a `didChangeConfiguration` toggle round-trip; and a non-ASCII fixture (🚀+Cyrillic ONLY in lexer-valid comment trivia — the standing Cyrillic-identifier lesson) exercises BOTH `positionEncoding`s with the codeAction edit range AND the inlayHint positions asserted correct + round-tripped in each (a byte/scalar shim makes one wrong → fails — the same risk-1 rigor L1–L6 applied, at the L7 layer). The Doc-14-§11-row-5 scope-out is surfaced precisely per the §11.33(3)…§11.37 prose-vs-code discipline (P0-1 lineage) — derive-correct to Doc 26 §5's authoritative narrowing, affirm the three accurate rows, flag the imprecision, neither paper over nor overclaim a defect. | v1.2-LSP-L7 `phase2.29/v1_2-lsp-l7` |
| §11.39 | DRIFT-1 reconciled: Doc 20 §4.5/L787 `parse_incremental` was aspirational prose (no such symbol in fsm-parser; only full-reparse `parse`/`parse_with_limits`/`parse_with_tokens` exist — `crates/fsm-parser/src/lib.rs:52`, `parse.rs:59`/`:67`/`:94` driving `grammar::parse_file` over the whole token vec) — annotated as v1.3+-deferred, not deleted; v1.2 LSP correctly built on full re-parse (Doc 26 §4.3–§4.5). Same marker applied to the §4.1 + ADR-004 mentions and the §12.2 signature; `analyze_incremental` (Doc 20 §12.3, a separate fsm-analyzer aspirational symbol) noted out-of-scope for tracking. | <why: prose-vs-code/P0-1-class hygiene; the verify-vs-code discipline applied to a spec doc; future intent preserved-but-labelled, not silently deleted (cf. §11.19/§11.24); the v1.2 LSP epic was deliberately scoped on full re-parse so the false claim is documented-but-deferred, never built upon> | DRIFT-1 `phase2.30/drift1-doc20-reconcile` |
| §11.40 | **v1.2 re-scoping: `v1.2.0` = the LSP server only (shipped, pending tag); VS Code extension → v1.3 (Doc 27); the old v1.3 "Simulation & verification" → v1.4; C++17 codegen (Doc 12) → its own minor.** ROADMAP/CHANGELOG/§11 reflect this. The LSP epic (L1–L7, §11.32–§11.38) is large, cohesive, independently valuable and the required substrate for both the VS Code extension (an LSP client) and the future Web IDE; shipping it alone matches the validated small-tight-tagged cadence (v1.0/v1.1) and the post-v1.1 retrospective's LSP-first feed-forward. | <why: a mega-v1.2 (LSP + VS Code + C++17) contradicts the shippable-increment pattern the project validated twice; the re-version uses the ROADMAP's own "propose-with-rationale, orchestrator updates" mechanism (§"How the roadmap evolves"); delegated product-owner autonomy, **user-endorsed**, reversible> | This consolidation `phase2.39/v1_2-batched-doc-honesty` (decision recorded; LSP delivery = §11.32–§11.38) |
| §11.41 | **FU#67 resolved — `fsm.toml [compiler] allow/deny` is now wired through `fsm check`.** The `CompilerSection.{allow,deny}` fields were parsed from `fsm.toml [compiler]` but read **nowhere** (a documented option silently did nothing — the mild P0-1 silent-no-op class). `cmd::check` now finalizes the merged allow/deny set and `diagnostics::apply_allow_deny` projects it post-analysis (allow → suppressed/exit 0; deny → error/exit 1; allow+deny → allow wins; unknown code → exit-4 loud; a retired code → accepted; a control code → unchanged) — matched on the parsed `DiagnosticCode`, never `.contains()` on source. +13 tests (6 `cli_check.rs` integration + 7 `diagnostics.rs` unit — verified split; the `24f231d` commit message inverts the integration/unit breakdown, corrected here per verify-the-record, merged history not rewritten). | <why: a documented compiler-config knob that does nothing is the P0-1 "documented-but-non-functional" sin; implement-or-remove was decided implement (the config surface and Doc 18 §6/§6.1 mandate it); behavioural, not string-matched> | FU#67 `24f231d` |
| §11.42 | **SEC-FU — RUSTSEC-2026-0009 (`time` ≤0.3.36 DoS, CVSS 6.8) eliminated by bumping `jsonschema` 0.17→0.22, dropping the transitive `time` edge.** A pre-tag SCA scan (the §11.48 self-finding) found `time` pulled transitively only via `jsonschema 0.17 ← fsm-ir` for the W0 IR-schema gate. The advisory's literal remedy (`time ≥0.3.47`) was **infeasible**: every `time ≥0.3.47` needs **rustc ≥1.88 / Cargo `edition2024`**, which the deliberate load-bearing `rust-toolchain.toml` **1.75** pin cannot parse (a lockfile-only `cargo update -p time --precise` was tried first and failed the build); edition-2021 `time 0.3.44/0.3.45` are below the advisory floor anyway. `jsonschema 0.22.0` removed the `time` dependency outright (its old iso8601 path) and is the **lowest** version with no `time` edge whose MSRV (1.70) still satisfies the 1.75 pin (the caret resolves to 0.22.3 — not over-bumped). Constraint lives in `crates/fsm-ir/Cargo.toml` (a Cargo.toml edit is genuinely required — lockfile-only cannot cross the 0.17 caret); the deprecated `JSONSchema::compile` shim migrated to `jsonschema::options()/.build()/Validator` (clippy `-D warnings`); W0 schema-gate behaviour byte-identical (4 `schema_validation.rs` + the analyzer-boundary gate test pass on a clean rebuild). Re-scan: 0 vulns. The 1.75 pin is unchanged; the stable toolchain is additive (builds cargo-audit only). | <why: a known DoS CVE cannot ship under a tagged release; fixed at the **root** (drop the vulnerable lineage) rather than version-pinning a still-vulnerable one; the 1.75-pin-vs-edition2024 constraint forced the minimal-correct jsonschema bump over the advisory's infeasible literal remedy> | SEC-FU `e509734` |
| §11.43 | **FU#68 — `unreachable_pub` is now wired workspace-wide; the over-pub invariant self-enforces.** §11.31's #64 pub-hygiene sweep narrowed 170 over-`pub` `src/` items to `pub(crate)` but **deferred the lint wiring** (the residual was test-harness-only, not `src/`). FU#68 adds `[workspace.lints.rust] unreachable_pub = "warn"` with every crate opting in (`[lints] workspace = true`) and the per-test-binary integration helpers explicitly `#![allow(unreachable_pub)]`'d (the legitimate test-only idiom). The 14 residuals this surfaced (the `fsm-lsp` `semantic_tokens.rs` legend/modifier consts — 11 token-type + 3 token-modifier) were downgraded `pub → pub(crate)` (no external consumer; behaviour-inert). Combined with the quad's `clippy -D warnings`, any future accidental over-`pub` in `src/` now fails the gate rather than silently accreting. | <why: §11.31's manual sweep is one-shot — without the lint the over-pub debt re-accretes; FU#68 makes the invariant machine-enforced (the Zero-Legacy/no-recurring-debt discipline); the 14 const downgrades are the lint surfacing the same class it now guards> | FU#68 `21a2380` / `a3dba92` |
| §11.44 | **DRIFT-2 converged — ONE `fsm_diagnostics::compute_line_col(src, pos, LineColUnit::{Byte\|Scalar})` byte→line/col core; analyzer + CLI delegate; the LSP `LineIndex` is left-and-explained.** The two divergent shipped impls — `fsm_analyzer::util::compute_line_col` (bytes, 1-based) and `fsm_cli::cmd::check::line_col` (Unicode scalars, 1-based) — are replaced by ONE linear-scan core parameterised by `LineColUnit`: `analyzer::util::compute_line_col` now forwards with `Byte`, `cli::cmd::check::line_col` forwards with `Scalar` (each behaviour-identical to its old self — proven byte-for-byte on all three contracts incl. multibyte by tests that pin each unit to the legacy loop, plus a cold-verified 801/0 quad). The LSP's `position.rs::LineIndex` is a **structurally different** algorithm (precomputed line-start table, 0-based, encoding-negotiated) and is **deliberately NOT folded** into this core — recorded per the §10 leave-and-explain anti-pattern (forcing it into the linear-scan core would worsen clarity for zero behaviour gain). Cargo.lock unchanged (0 new deps). | <why: two divergent line/col impls is the class-of-issues + latent-correctness debt §11.32/§11.36 tracked open across the LSP epic; converged now (not a 3rd silent copy), with the genuinely-different LSP index left-and-explained rather than mis-merged to hit a dedup count (the §11.49 / SUBAGENT_CONVENTIONS §10 refactor-to-number discipline)> | DRIFT-2 `7cf1174` / `d399623` |
| §11.45 | **DRIFT-3 reconciled — Doc 20 parser §4.5/§12.2/§4.2/§4.3 vs shipped code.** `analyze_incremental` (§12.3) noted out-of-scope for tracking and the §4.5/§12.2 `parse_incremental` mentions + §4.1/ADR-004 annotated as the same v1.3+-deferred aspiration as DRIFT-1 (§11.39); the §4.2 module layout and §4.3 `ParseResult` shape reconciled to the shipped `crates/fsm-parser` (cited file:line, annotate-not-delete). Docs-only, code-inert. | <why: the §11.39 DRIFT-1 reconciliation explicitly flagged these adjacent §4.x/§12.x staleness sites as a tracked follow-up; same prose-vs-code / P0-1 hygiene, design intent preserved-but-labelled (cf. §11.19/§11.24/§11.39)> | DRIFT-3 `a1c726b` |
| §11.46 | **DRIFT-4 reconciled — Doc 20 analyzer §5.2 module layout + §5.3 `AnalysisResult`/`SymbolTable`/`Scope` types vs shipped code.** The §5.2 listing named a two-`phase{1,2}_*` directory tree (`phase1_structure/*`, `phase2_semantic/*`, `ir_builder.rs`, `tests.rs`, a `DiagnosticAccumulator`) that does not exist; reconciled to the shipped flat tree (`symbol_table.rs` + a `checks/` family + the AD-3 `lower/` tree; diagnostics are a plain `Vec<Diagnostic>`), the two-phase split kept as the *conceptual* model. §5.3 types reconciled (cited file:line, annotate-not-delete). Docs-only, code-inert. | <why: DRIFT-3 flagged the §5.2/§5.3 analyzer description as stale the same way; same annotate-not-delete recipe (cf. §11.19/§11.24/§11.39/§11.45)> | DRIFT-4 `d90cd2f` |
| §11.47 | **FU-DEAD-CODES decided — `FSM-W0200` IMPLEMENTED, `FSM-W0500` RETIRED.** Both were catalogued `DiagnosticCode`s emitted **nowhere** (zero emission sites — surfaced writing L7 codeAction; the G7-conformance-honesty / prose-vs-code class). Decided per-code: **W0200 ("loop in action block") IMPLEMENTED** — four normative docs (Doc 02 §9.2, Doc 04 §8.7.2, Doc 11 §18, Doc 10's W0200 catalog entry) mandate it as a real style nudge, so a single emission site was added (`checks/action_lint.rs`: one `FSM-W0200` per loop whose ancestor chain contains an `ACTION_BLOCK`); the code was conformed to the prose (the ideal honest-surface direction), conformance 25→26 (SEM-NEG-004), live count unaffected. **W0500 ("unused extern") RETIRED** — vestigial, no normative spec mandates it, never emitted; moved to `deprecated::DeprecatedCode::W0500` (still parses in suppression / `allow` per Doc 10 §14 rule 2 — the FU#67 §11.41 allow/deny contract is preserved, test-pinned), the `all_codes_matches_expected_count` CI-lock 74→**73**. | <why: a catalogued-but-never-emitted code is a G7-honesty defect (a "≥1 test per code" claim over a code nothing can produce); decide-then-do per code — implement where the corpus mandates it (W0200), retire where it is vestigial (W0500); clean honest surface over a reserved-forever placeholder> | FU-DEAD-CODES `12ba06a` (`0747321`) |
| §11.48 | **External tooling-kit evaluation verdict: ~95% rejected-proven; only 2 zero-cost take-aways — and one of them caught a real CVE pre-tag.** An external "tech-lead-agent-kit" was assessed against this mature project; the overwhelming majority of its suggestions were already in place or rejected with cause. Exactly **two** zero-cost take-aways were adopted: (1) the SUBAGENT_CONVENTIONS §10 *refactor-to-number anti-pattern* row (§11.49); (2) an SCA (software-composition-analysis) gap — which was **our own dropped recommendation**: the v1.0 `AUDIT_2026_05_14` correctness pass had recommended a periodic `cargo audit` and it was never operationalised. Running it for real (per the proven recipe — a separate recent toolchain because the 1.75-pinned `cargo-audit 0.21.1` cannot parse the CVSS-4.0 advisory DB) caught **RUSTSEC-2026-0009 before the v1.2 tag** (fixed: §11.42). | <why: meta-lesson worth keeping — a user push for external rigor did not validate the kit but surfaced a real **self-dropped audit finding** that turned into a genuine pre-tag CVE catch; "verify-the-audit-too / don't drop your own recommendations" (§11.29 lineage). The SCA step is now wired into CI (`.github/workflows/ci.yml`) and the pre-tag reliability checklist so it cannot be dropped again> | kit-evaluation (recorded this consolidation `phase2.39`; SCA catch = §11.42 `e509734`) |
| §11.49 | **`analyzer→parser-CST coupling` cleanup (arch-audit P1, ~15 files) is DEFERRED out of the v1.2 critical path — accepted-tracked-debt (v1.1 precedent).** The architecture audit rated this explicitly **ship-acceptable, non-behavioural** P1. Owner decision: a ~15-file refactor of the *most behaviourally-critical crate* (`fsm-analyzer`) **at a release boundary** is worse-EV than doing it cleanly post-tag (DRIFT-2-grade discipline, a tracked v1.2.1/early-v1.3 wave). **v1.1.0 shipped the same class of arch-debt tracked + documented in its `GATE_VERIFICATION_v1_1.md`** (the "Tracked architecture debt → gate before v1.2" / `analyzer→parser-CST coupling` entry) — deferring here is precedent-consistent, lower-risk, and still gets done properly. Must also appear in `GATE_VERIFICATION_v1_2.md` (authored at tag-time) as accepted-tracked-debt. | <why: a big risky refactor of the behaviourally-load-bearing crate at a release boundary maximises regression EV for zero user-visible gain; v1.1 set the precedent that this exact debt class ships tracked-and-documented, not rushed; the §10 leave-and-explain / worse-EV-at-a-boundary discipline (cf. §11.44)> | CST-coupling deferral (owner decision recorded this consolidation `phase2.39`; v1.1 precedent = `GATE_VERIFICATION_v1_1.md`) |
| §11.50 | **§11.49 CST-coupling debt SUBSTANTIALLY PAID as v1.3-W0 (`ceb8efd`, == `checkpoint/2026-05-16-w0-clean`), with documented reasoned R-1..R-4 residuals — the v1.2 deferral RESOLVED, not regressed.** The clean Archetype-A removal landed: `crates/fsm-analyzer/src/checks/parallel.rs` is the **sole** file to fully shed `use fsm_parser::cst::*` (its lone `INITIAL_DECL` predicate → the typed `RegionDecl::initials()` accessor). The B-clean single-construct accessor relocations landed (~7 new `pub fn` on already-declared `ast_node!` types in `fsm-parser`: `PriorityClause::value`, `{Transition,Internal,Local}Decl::payload_binding`, `{After,Every,EveryInternal}Decl::{action_block,duration}`, `{Machine,Region}Decl::initials` — each body the *exact* relocated CST walk, incl. the correctly-disclosed decimal-only `priority` parse subtlety). **13 `fsm-analyzer` files deliberately retain `use fsm_parser::cst::*` as the documented R-1..R-4 leave-and-explain residual**, each carrying a textbook DRIFT-2-form comment citing this §11.44/§11.49 + the `LineIndex` precedent: R-1 OPAQUE-BUG-1 parent-node type resolution (P0-1 silent-data-loss risk if removed; `TypeOrOpaque` widening deferred to a dedicated parser-API wave — conservative), R-2 heterogeneous source-/document-pre-order dispatch (the emitted **diagnostic vector is unsorted** → traversal order byte-load-bearing; independently re-derived true), R-3 the deliberately-shallow expr/stmt sublanguage (folding *relocates not eliminates* the walk — dominant blast-radius), R-4 `util::span_of`-family positional helpers (pure rowan-positional, zero cross-crate callers). Additive-only on `fsm-parser` (zero deletions / zero grammar/CST/Cargo change), 0 new deps (Cargo.lock byte-unchanged, verified `git diff 7d6792a ceb8efd -- Cargo.lock` empty), non-behavioural (the SUBAGENT-§5.4 pre/post-identity proof: full corpus + 35 LSP client tests + examples 5/5 + conformance 26/26 byte-unchanged; §11.1 returned COLD-QUAD GREEN + IDENTITY CONFIRMED, 38-file `diff -r` byte-empty). The post-W0 §11.3 phase-audit rated it a **success of the C-1/SUBAGENT-§10 discipline, NOT under-delivery**. | <why: this is the *resolution* of the §11.49 v1.2 deferral, paid where clean + left-and-explained where forcing it was worse-EV — precedent-consistent with how DRIFT-2 (§11.44) and v1.1 pub-hygiene closed; the genuinely-clean order-independent site WAS removed, nothing contorted to hit a zero-`cst` count> | v1.3-W0 `ceb8efd`; phase-audit `255a3e7` (`docs/AUDIT_PHASE_W0_2026_05_16.md`) |
| §11.51 | **Toolchain-probe-trap correction — the §11.1 W0 post-merge agent's "rustc 1.95.0 / pin not honored" line was a PROBE-CONTEXT ERROR, not a pin violation; the 1.75.0 pin IS honored.** The §11.1 agent ran a bare `rustc --version` (or one from a non-repo cwd), saw the box default `stable`=`1.95.0`, and falsely concluded "rust-toolchain.toml does not pin 1.75 in practice." Independent triage (post-W0 §11.3 audit §5; the `feedback_embeded_fsm_toolchain_probe_trap` memory) proved the opposite: `1.75.0` IS installed, `rust-toolchain.toml` is byte-identical to the `v1.2.0` tag, and `rustup show active-toolchain` *run from the repo dir* = `1.75.0-x86_64-unknown-linux-gnu (overridden by '…/rust-toolchain.toml')`. Cargo invoked from the repo (every wave/quad invocation) respects the override; the W0 §11.1 COLD-QUAD-GREEN result stands **on the correct pinned 1.75.0 toolchain**. **Codified probe method (binding for every future cold-quad / §11.1 / §11.3 / release-verification brief):** assert the toolchain via `rustup show active-toolchain` FROM the repo dir (expect the `1.75.0 … overridden by rust-toolchain.toml` line); a bare `rustc --version` is NOT acceptable evidence and its `1.95.0` output is expected/benign (box default), NOT a pin violation — do not flag it as drift. | <why: a false "pin broken" claim entering the release record would have triggered an unnecessary destabilizing fix and undermined every GATE_VERIFICATION attestation; verify-the-record is symmetric (§11.29 lineage) — an agent asserting an anomaly-is-a-defect is itself a claim to code-check; the 1.75 pin is load-bearing (§11.42 forced the jsonschema-0.22 floor + SCA recipe)> | toolchain-probe-trap correction (post-W0 §11.3 audit `255a3e7` §5; memory `feedback_embeded_fsm_toolchain_probe_trap`) |
| §11.52 | **V1-audit Finding 1 + Doc 27 §2.2 correction-of-record: `TransportKind.stdio` is a WRONG literal — the only round-tripping form is OMIT `transport` on the `Executable`.** Doc 27 §2.2's "the client launches the binary … with the `vscode-languageclient` stdio transport (`TransportKind.stdio`)" is wrong as a literal instruction: the shipped `fsm-lang-server` takes **no arguments** for stdio mode (`crates/fsm-lsp/src/main.rs:53-65` `None => run_stdio()`) and its strict parser **exits 2 on ANY unknown arg, including `--stdio`** (`main.rs:49-52`). Setting `transport: TransportKind.stdio` makes `vscode-languageclient@9` push `--stdio` onto argv for an `Executable` (`lib/node/main.js:410-411`) → server exits 2, client stream destroyed mid-`initialize`. Omitting `transport` is the **only** form that round-trips: for an `Executable`, v9 reads `json.transport` **without** the `|| TransportKind.stdio` coercion the `NodeModule` path applies (`main.js:409` vs `:266`), and `transport === undefined` is an **explicitly-handled first-class raw spawn path** (`main.js:425`). The shipped V1 code does exactly this and is **correct**; it is Doc 27 prose that was wrong-as-written. Independently re-derived from the locked `vscode-languageclient@9.0.1` source by the post-V1 §11.3 audit (verify-the-record, not trusted from context). **Constraint binding every `editors/vscode/` wave:** MUST NOT re-add `transport: TransportKind.stdio` to the `Executable`, MUST NOT switch to a `NodeModule` server shape — doing either silently reintroduces the exit-2 crash (the M-1 foot-gun; re-confirmed unbroken at V2/V3 `61cc3f0`, V4 `4c4c6a2`, and through V5/V6 — V5/V6 construct no `ServerOptions`). N-3 citation re-pin: Doc 27 §2.2's `main.rs:39-48` should read `:39-48` (`--port`) / `:49-52` (unknown-arg exit-2) / `:53-65` (`None→run_stdio`). | <why: the cardinal overstatement sin inverted — a doc asserting the wrong concrete mechanism; the shipped code is right, so blindly "fixing" the missing `transport` back would regress the release into a crash; verify-the-record applied symmetrically (§11.29 lineage); the frozen `AUDIT_PHASE_V1` §3.A is the durable evidence, this row is the canonical correction> | V1-audit Finding 1 (`docs/AUDIT_PHASE_V1_2026_05_16.md` §3.A, commit `4280406`); shipped V1 `6707d1d` |
| §11.53 | **V1-audit Finding 2 + Doc 27 §2.3 / §7-risk-5 / §8-V1(c) + Doc 28 §2.2 / R-4 / §3-V1(c) / §5-V1(c) correction-of-record: UTF-8 `positionEncoding` is UNREACHABLE through the official `vscode-languageclient`; the correct negotiated value is `"utf-16"`.** `lib/common/client.js:1370` **hardcodes** `general.positionEncodings = ['utf-16']` as an unconditional literal (no opt-in/override/middleware); `client.js:835-836` **throws** `Unsupported position encoding` if the server negotiates anything other than UTF-16. The official client is intrinsically UTF-16 (VS Code's text model is UTF-16). So the shipped server's documented **fallback** runs: client offers only `['utf-16']` → `server.rs:216-226` (`if offer.contains(UTF8) {Utf8} else {Utf16}`) selects **UTF-16** → client accepts (no throw). **This is CORRECT** (Doc 26 §4.1: the UTF-16 LineIndex path is still correct — it loses only the *performance* fast-path, not correctness). The substantive multibyte-column defect-class is **NOT** guarded by negotiating UTF-8 (impossible) but **end-to-end by V1 acceptance assertion (d)** (a first error *after* a non-ASCII line, Range byte-compared to the `fsm check --json` oracle, under the UTF-16 negotiation). The shipped acceptance test already encodes the corrected (c): it asserts `negotiatedPositionEncoding === "utf-16"` (`extension.test.ts`) — i.e. the implementer **correctly diverged** from Doc 28 §3/§5 clause (c)'s literal "UTF-8" instruction (N-2; recorded so a future "test ≠ brief" mechanical check does not mis-flag the *desired* state). R-4's verified server-side negotiation logic is unchanged + exact; only R-4's *client-reachability assumption* ("UTF-8 reachable iff the extension does not strip it") is false. The surviving real constraint: do NOT override `clientOptions`/`initializationOptions` to suppress the client's default capabilities (`initializationOptions` carries ONLY the four inlay keys). No code change implied — pure documentation-of-record. | <why: same inverted-overstatement class as §11.52 — the load-bearing Doc 26 §4.1/risk-1 deletion rationale rode a false client-reachability premise; the shipped code + acceptance test are already correct; the frozen `AUDIT_PHASE_V1` §3.B is the durable evidence (independently re-derived from the locked client source), this row is the canonical correction folded at closeout> | V1-audit Finding 2 (`docs/AUDIT_PHASE_V1_2026_05_16.md` §3.B, commit `4280406`); shipped V1 `6707d1d` |
| §11.54 | **v1.3-V1 implementation decision (the depth-first MVP spine).** `editors/vscode/` greenfield TS, `vscode-languageclient@9.0.1` ↔ `fsm-lang-server` over stdio (transport-omitted, §11.52); live diagnostics reusing the exact `fsm check` pipeline; the Doc 22 §2.2 binary-resolution Rule 1/2/3 (**no silent PATH/guess fallback** — the cardinal-sin bar at the client boundary, verified there is no `which`/PATH lookup) + the Doc 22 §13.2 exponential-backoff crash-recovery handler (all 8 params + the subtle "clear reset timer on death" semantics matched). Acceptance = 4 real `@vscode/test-electron` Extension-Host tests with the (a)/(d) oracle independently recomputed from `fsm check --json` + R-15 fixture isolation hard-asserted (an OS-temp-dir stage + an up-tree `fsm.toml` walk that fails loud). Post-V1 §11.3 phase-audit: **PROCEED-WITH-NOTES** — the spine is sound for V2–V6 with no rework, every load-bearing seam correct *by reading*, the omit-`transport` fix robust *by construction*. | <why: V1 is the depth-first integration keystone (the riskiest client↔server+positionEncoding bet, validated + phase-audited first, the LSP-L1 precedent); the binary-resolution no-silent-fallback + the verify-by-reading of the unexercised Rule-2/3 + backoff paths are the §5.4-analogue discipline applied to the extension> | v1.3-V1 `6707d1d`; phase-audit `4280406` |
| §11.55 | **v1.3-V2/V3 implementation decisions + N-5 CLOSED + N-6 CORRECTED-IN-SHIPPED.** **V2** (`1542bfb`, static assets): the Doc 21 TextMate grammar (every Doc 21 §2 scope name preserved *verbatim*), `language-configuration.json`, snippets; 7 grammar-test cases. **N-6 corrected-in-shipped:** Doc 21 §3's *literal JSON* is structurally defective (`machine`/`state`/`composite`/`parallel`/`region` as single-line `match` rules with **no begin/end body rule**, so a `{ … }` body had no rule to descend into and the bare-`{` `#action-block` greedily swallowed the whole machine body — the V2 behavioural gate caught it); the shipped grammar converts those five to **begin/end block rules whose body re-includes the pattern set** + a `state-declaration-bare` `match` fallback, **scope NAMES Doc-21-§2-verbatim unchanged** (J-1, §11.1-ratified; the inline `_note` `tmLanguage.json:6` is the durable correction-of-record; Doc 21 §3 prose reconciled at this closeout — Doc 21 is a Status:Deferred DRAFT). **V3** (`3c49d38`, merged `61cc3f0`, thin glue): the 7 Doc 22 §4 commands (bare title + `category:"FSM Studio"` — the JC-4 normalization, the *correct* realization since VS Code renders `<category>: <title>`; Doc 22 §4 literal `"FSM: …"` titles are the stale prose, re-pinned at closeout) + their menus/keybindings/`commandPalette` gating + the honest `cliRunner.ts`/`cliBinary.ts` seam + `copyIr.ts` codegen-gated `--emit-ir` path; 9 command-test cases. **N-5 CLOSED by V3:** `fsm.showOutputChannel` + `fsm.restartLanguageServer` (wired by V1 but correctly not contributed until V3) are now contributed+registered + `onCommand:` activation-evented. Post-V2/V3-batch §11.3 phase-audit: **PROCEED-WITH-NOTES** — disjoint+additive on V1, M-1 honoured by construction, the V4 IR-seam resolved. | <why: V2/V3 are the low-risk static-assets + thin-glue pair after V1's proven spine; N-6 is the DRIFT-2 "fix-where-demonstrably-broken + keep names + explain" discipline (Doc 21 §3 literal was defective, the grammar fixes structure not scope names); N-5 is a correctly-scoped V1→V3 forward-dependency closed on schedule> | v1.3-V2 `1542bfb` / v1.3-V3 `3c49d38` (merge `61cc3f0`); phase-audit `4aaef5d` (`docs/AUDIT_PHASE_V2V3_2026_05_16.md`) |
| §11.56 | **v1.3-V4 implementation decision + the `copyIr.ts`→`emitIr.ts` extraction (a POSITIVE closeout credit).** V4 (`4c4c6a2`): `fsm.openDiagram` → CSP-locked (`default-src 'none'`, nonce'd, `localResourceRoots`-pinned) split-right read-only `WebviewPanel`; data = the **one real** codegen-gated `fsm generate --emit-ir`→`<machine>.ir.json` path via the V3 `cliBinary.ts`+`cliRunner.ts` seam (**NO new LSP/server method, NO `fsm ir` subcommand** — the R-9/R-10 DRIFT bar held; the Doc 26 §4.5 "do not smuggle a subsystem" rule); elkjs ELK-Layered in-webview, click→source via the IR `SourceLocation`; codegen-gated boundary **sealed** = last-valid-render-retained + the Doc 05 §1.5.9 banner **VERBATIM**, asserted at the real Webview ack (the analogue of V3 `copyIr.ts:91-104` refusing to fake output). **POSITIVE:** the post-V2/V3-audit-§1.2-*recommended* `copyIr.ts`→`emitIr.ts` shared-core extraction **shipped exactly** — one honest `--emit-ir` seam consumed by both `copyIR` + the V4 panel (`grep`-verified the ONLY `--emit-ir` site, no second resolver), `copyIR` **pre/post-identical** (the V3 copyIR tests 18+19 re-passed — the SUBAGENT §10 refactor pre/post-identity bar met). 4 diagram-test cases. Post-V4 §11.3 phase-audit: **PROCEED-WITH-NOTES** — the DRIFT-class boundary sealed, no DRIFT escapes into V5; recorded the extraction as a *credit* (not a defect), V4 JC-2 (the codegen-ICE boundary tested via the V3-blessed `emitIr` non-zero-exit equivalence — a deterministic analyzer-clean-but-codegen-ICE fixture is not constructible in-tree; the chosen path is the IDENTICAL `emitIr`/`refresh()` code a real ICE takes — sound, audit-blessed), and V4 JC-4 (a separate strict `tsconfig.webview.json` for the browser-context script — additive, Zero-Legacy-correct credit). | <why: V4 is the one substantive new subsystem (the diagram, where the §6 codegen-gated-IR DRIFT lives) — gated after the client+IR-CLI seams were proven + phase-audited; the audit-recommended extraction taken with the refactor-safety bar met is exactly the SUBAGENT §10 clean-move discipline, recorded as a credit not a finding so the record is honest both ways> | v1.3-V4 `4c4c6a2`; phase-audit `203b7d5` (`docs/AUDIT_PHASE_V4_2026_05_16.md`) |
| §11.57 | **v1.3-V5/V6 implementation decisions + the V6 owner-escalations (G9-tail, VSIX-not-published, no-LICENSE).** **V5** (`1db80cd`): `fsm.machineExplorer`/`fsm.eventExplorer` `TreeDataProvider`s fed by the **free V1-client `documentSymbol`** seam (`vscode.executeDocumentSymbolProvider`, `server.rs:251` — the same single-`analyze_with_source` seam; explicitly **NOT** V4's `emitIr.ts`/`irGraph.ts` IR path: that is the wrong shape + would re-import the K-3 codegen-gated DRIFT into chrome — the MV5-1 foot-gun the post-V4 audit forbade), Doc 22 §7 view containers + Doc 22 §11 context keys (`fsm.hasOpenFsmFile`/`fsm.serverRunning`; `fsm.simulatorConnected` OUT — no simulator in v1.3); tree-fidelity asserted EXACTLY against the client result (not symbol-presence); 6 tree-test cases. **V6** (`fa3befc`, host-only): `scripts/populate-bin.mjs` builds the host-triple `fsm-lang-server`+`fsm` on the pinned 1.75 into `editors/vscode/bin/<host-triple>/` (producer/consumer re-derived from the *same* `os.platform()`-`arch()` so V1's never-exercised Rule-2 resolver fires against a real bundle); `vsce package` → a lean **4.95 MB installable `.vsix`** (excludes node_modules/src/out/test, asserted via `vsce ls`+`unzip -l`); `bin/`+`*.vsix` gitignored — committed delta is **ONLY packaging machinery** (V1 client-spawn + V2–V5 source + Rust + docs + `contributes` byte-untouched); 4 V6-test cases (Rule-2-fires + bundled-server diagnostic byte-equal to the bundled `fsm check --json` oracle + the SEC-P0-1 multibyte seam through the bundle). **V6 owner-escalations (carried, surface-don't-grind):** the 5-platform cross-compile tail is **G9-gated** (no cross toolchains — owner/infra action, NOT attempted; Doc 27 §7 risk-2 / v1.2 §5 posture); `vsce package` is **NOT publish** (publishing/signing = owner credential decision, Doc 27 §9 risk-4 — no PAT sought); the repo has **no top-level `LICENSE` file** (owner/legal — generated code carries an SPDX header per §10.4, VSIX host-only-unpublished, so not a v1.3 ship-blocker, but a published extension should carry an explicit license). | <why: V5 is chrome reusing the proven-free `documentSymbol` (the wrong-seam foot-gun forbidden, the symmetric analogue of §11.52's omit-transport); V6's host-only-MVP + the three owner-escalations are the infra-constraint-escalation discipline (surface a crisp owner decision, never grind cross toolchains / publishing / legal); no claimed-shipped item that did not ship> | v1.3-V5 `1db80cd` / v1.3-V6 `fa3befc`; Doc 28 §3-V5/§3-V6 (`87ffebd`) |
| §11.58 | **JC-3 carried to an owner/later config-owner wave: `fsmLang.codegen.outputDir`/`codegen.strategy` are extension-consumed but not Settings-UI-discoverable.** V3's `generate.ts` reads `fsmLang.codegen.outputDir`/`codegen.strategy` with the Doc-22-§8 defaults (`"generated"`/`"switch"`) but neither is in `contributes.configuration.properties` (the manifest ships only the 5 V1 keys: `compilerPath` + 4 inlay) → an IDE Settings-UI discoverability gap (the user can only set them via raw `settings.json`). Same class as the Doc 27 §10 / Doc 28 R-6..R-8 / K-6 server-dead-key config ledger, but these two are *live extension inputs*, not server-dead. **NOT a v1.3 ship-blocker** (defaults == Doc 22 §8 so not a correctness bug; V4 calls `--emit-ir` with a fixed `--target c99` to a *temp* dir, V5 reads no `codegen.*`; re-verified at V4 `4c4c6a2` + V5 that no config key was added). Carried to a later **config-owner wave** that also reconciles the R-6..R-8 server-dead-key ledger + the Doc-22-§4 bare-title re-pin (whether to also surface `codegen.defaultTarget`/`format.*` is that wave's decision). | <why: a real discoverability gap worth fixing, but defaults are correct so it is non-blocking; bundling it with the existing R-6..R-8 config ledger into one owner-decided config-owner wave avoids a half-pass; recorded so it is not silently dropped (the frozen `AUDIT_PHASE_V2V3`/`AUDIT_PHASE_V4` §2 JC-3 is the evidence)> | JC-3 (V2/V3-audit `4aaef5d` §3.B; re-verified V4-audit `203b7d5` §2) |
| §11.59 | **JC-1 environmental: the JS Extension-Host CI lane requires `xvfb` (a virtual DISPLAY) — now strictly load-bearing post the V4 Webview test.** `@vscode/test-electron` launches a real Electron Extension Host ⇒ needs a DISPLAY; the V4 `diagram.test.ts` added a real Webview, so the xvfb requirement is now *strictly* load-bearing for the §11.1 / pre-tag JS-quad (a Webview ack cannot post without a rendering context). The Doc 28 §2.3 Option-A separate additive JS CI lane MUST invoke the suite under `xvfb-run`, and **every** future §11.1 / release-verification brief MUST carry it. Folded into `GATE_VERIFICATION_v1_3` §4.2 (the pre-tag JS-quad transcript is run under `xvfb` at the release commit `X`, recorded in the annotated tag message alongside the cargo transcript) + the post-tag owner CI-lane wiring (§5.1). No source change — a CI-lane/operational detail. | <why: an environmental precondition that, if dropped, makes the JS lane silently un-runnable (a green-by-not-running trap); recording it in the gate doc + every verification brief makes it un-droppable, the same discipline as the SCA-step §11.48 carry> | JC-1 (V2/V3-audit `4aaef5d` §0; re-verified V4-audit `203b7d5` §2 — non-optional post-V4) |
| §11.60 | **The V4 `copyIr.ts`→`emitIr.ts` extraction recorded as a POSITIVE closeout credit (the audit-recommended clean move, taken with the refactor-safety bar met) — NOT a defect.** The post-V2/V3 §11.3 audit §1.2 *recommended* extracting the shared "generate-IR-to-temp → locate `*.ir.json` → read" core out of `copyIr.ts` into a reusable `emitIr` helper consumed by both `copyIR` + the V4 panel. It **shipped exactly** in V4 (`4c4c6a2`): one honest `--emit-ir` seam (`grep`-verified the ONLY such site, no second resolver, the cardinal-sin honest-`runCli` logic centralised once), `copyIR` **pre/post-identical** (the V3 copyIR tests 18+19 re-passed — the SUBAGENT §10 refactor pre/post-identity bar). Recorded as a *resolved DRIFT-2-grade consolidation credit*, not a finding, so the record is honest in both directions (we record audit-recommended improvements that shipped well, not only defects). Companion credits: V4 JC-2 (the codegen-ICE boundary proven via the `emitIr` non-zero-exit equivalence — sound, audit-blessed; a deterministic analyzer-clean-but-ICE fixture is not in-tree-constructible) and V4 JC-4 (the separate strict `tsconfig.webview.json` — additive, Zero-Legacy). | <why: SUBAGENT §10's clean-move discipline working as intended (an audit recommendation taken, with the pre/post-identity bar met); recording POSITIVE credits keeps the record honest both ways and documents the refactor-safety proof so a future reader does not mistake the `copyIr.ts` diff for a behaviour change> | V4 extraction credit (post-V4 §11.3 audit `203b7d5` §1.2/§2; recommended in V2/V3-audit `4aaef5d` §1.2) |
| §11.61 | **`makeNonce()` `Math.random`→CSPRNG hardening — a tracked v1.3.x senior-bar one-liner, NOT a v1.3 ship issue.** V4's diagram `WebviewPanel` builds its CSP nonce via `Math.random()` (`diagramPanel.ts:362-370`), not `crypto.randomBytes`. The post-V4 §11.3 audit explicitly rated this **adequate and non-contract-weakening** for a CSP nonce on a *local* `vscode-webview://` bundle with `default-src 'none'` and no remote/inline-injection vector — recorded there as a non-actionable nit, NOT catalogued as a defect. Carried here as a **tracked v1.3.x senior-bar hardening** (swap to a CSPRNG — a low-risk one-liner that strengthens defence-in-depth without changing behaviour); it does not gate v1.3 (the V4 security contract Doc 27 §8-V4 states is met). | <why: senior-bar hygiene worth doing but explicitly not a ship blocker (the audit independently rated it adequate under the local-bundle CSP); recording it as a tracked low-risk hardening rather than a defect avoids overstating it while ensuring the one-liner is not lost> | makeNonce nit (post-V4 §11.3 audit `203b7d5` §3.B — rated non-actionable) |
| §11.62 | **No top-level `LICENSE` file — surfaced as an owner/legal decision (carried, not a v1.3 ship-blocker).** `git ls-tree fa3befc` has no `LICENSE`/`LICENSE.md`/`COPYING`. Generated firmware code already carries an SPDX header (this §10.4: MIT-by-default, `--license=<SPDX>` overridable) and the v1.3 VSIX is host-only-**unpublished**, so the absence does not block the v1.3 local tag. But a *published* extension or distributed source should carry an explicit repository license; that is an **owner/legal decision** (the surface-don't-grind discipline, the §11.48/§11.57 escalation pattern). Recorded so it is not silently dropped; coupled to the §11.57 VSIX-not-published owner-escalation (publishing is the trigger that makes a repo license load-bearing). | <why: a genuine gap that is correctly non-blocking *for the local host-only tag* but becomes load-bearing the moment the owner publishes — recording it as an owner/legal escalation (not a defect) is the honest non-overstated framing; coupled to the VSIX-publish owner decision so both surface together> | no-LICENSE owner/legal escalation (recorded this v1.3 closeout; coupled to §11.57 / Doc 27 §9 risk-4) |

---

*End of FSM-SPEC-DEC v1.0.0 (TL-amended 2026-05-11; §11 appended 2026-05-14; rows §11.19-39 appended 2026-05-15; rows §11.40-49 appended 2026-05-16, the v1.2 batched doc-honesty / ledger consolidation; rows §11.50-62 appended 2026-05-16, the v1.3 batched doc/ledger consolidation — W0 §11.49-paydown + the VS Code extension V1–V6 closeout)*
