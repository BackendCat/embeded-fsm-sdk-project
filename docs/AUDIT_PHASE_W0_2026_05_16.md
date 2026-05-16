# AUDIT — §11.3 Post-W0 Phase-Boundary Audit (v1.3-W0 §11.49 CST-coupling paydown)

**Type:** SUBAGENT §11.3 phase-boundary audit — READ-ONLY judgment audit.
**Audited commit:** `main` HEAD `ceb8efd` ("Pay down §11.49 analyzer→parser-CST
coupling (W0, additive-only)"); compared against pre-W0 `7d6792a`.
**Mandate:** Doc 28 §4 / §5 (precondition 2) / §6.2(1); Doc 29 §6; Doc 00
§11.49 / §11.44; SUBAGENT §10 / §11.1 / §11.3. The gate that lets V1's
diagnostics oracle (`fsm check --json` → `fsm_analyzer`) be computed against
the **post-W0** analyzer.
**Method:** zero `cargo`/build. Derived entirely by reading source +
`git -C` diffs + `grep`. The §11.1 independent post-merge verification
already returned COLD-QUAD GREEN + IDENTITY CONFIRMED (38-file corpus,
`diff -r` byte-empty, pre-sha==post-sha, 806/0/108, 26/26, clippy -D clean,
fmt clean, forbid-unsafe 11 roots/10 crates, Cargo.lock unchanged); this
audit does **not** re-run the quad — it is the *judgment* gate (soundness,
discipline, drift, V1-readiness) layered on top of that mechanical proof.
**Date:** 2026-05-16. **Auditor:** phase-boundary audit agent (independent).
**Branch:** `phase3.1/v1_3-w0-audit` @ `ceb8efd` (manual worktree
`/root/dev/embeded-fsm-sdk-wt-w0audit`).

---

## 0. Verdict (TL;DR)

**V1-READINESS: PROCEED-WITH-NOTES.**

The post-W0 `fsm-analyzer` is **sound to build V1's behavioural oracle on**.
W0 is a correctly-disciplined DRIFT-2 non-behavioural seam cleanup:
additive-only on `fsm-parser` (zero deletions, zero grammar/CST/Cargo
change), behaviour-inert accessor relocations proven by the §4.2
byte-identity gate, and four leave-and-explain residual classes (R-1..R-4)
each carrying a textbook DRIFT-2-form code comment that cites Doc 00
§11.44/§11.49 and explains *why folding is worse-EV*. The spot-checked
migrated sites are byte-faithful relocations. The "diagnostics unsorted →
traversal-order byte-load-bearing" premise underpinning R-2 is
**independently re-derived and confirmed true** (no sort anywhere in the
analyzer→CLI diagnostics path).

The **NOTES** (none are V1 ship-blockers; all are orchestrator
record-keeping for the GATE_VERIFICATION_v1_3 / Doc 00 §11 closure pass):

- **N-1 (DRIFT — must be reconciled in the W0 closure record, not a code
  defect):** Doc 29 §3.1's plan-of-record disposition ("the pure-A rows —
  `name_resolution` descendants-dispatch, `type_check`, `timer`,
  `submachine`, `symbol_table` — are **cleanly removable** … ~7 files lose
  `use fsm_parser::cst::*` entirely with zero behaviour risk") is **stale
  and contradicted by shipped reality**. Exactly **one** file
  (`parallel.rs`) fully lost its `cst::` import; the §3.1-named "pure-A"
  files were correctly reclassified as **R-2/R-3 generalized
  leave-and-explain residuals**. This is the correct DRIFT-2/§10 call (the
  plan's §3.1 was internally inconsistent with its own §3.4-R-2 — see §4),
  **not** under-delivery — but Doc 29 §3.1 now needs a one-line
  reconciliation note, and the orchestrator must record the *shipped*
  residual posture (this doc §3), not Doc 29 §3.1's optimistic estimate, in
  GATE_VERIFICATION_v1_3.
- **N-2 (record correction — corrects the §11.1 agent's transcript):** the
  §11.1 post-merge agent's "rustc 1.95.0 / pin not honored" line was a
  **probe-context error, not a pin violation**. See §5.

This audit deliberately surfaces the migrated-site spot-checks and the
unsorted-diagnostics re-derivation (§2, §6) because "an audit that finds
nothing across a ~15-file refactor of the most behaviourally-critical crate
is itself suspect" — the work *was* checked, and the one real drift (N-1)
is named precisely rather than papered over.

---

## 1. Lens 1 — Is the post-W0 analyzer sound to build V1's oracle on?

**YES.** V1's behavioural acceptance computes its diagnostics oracle from
`fsm_analyzer` via `fsm check --json` (Doc 28 §5 / Doc 27 §3/§8, R-15).
The relevant question is whether the typed-accessor consumption is correct
and whether a high-fan-out consumer (the extension's `@vscode/test-electron`
corpus) would expose a latent foot-gun.

### 1.1 Edit-set shape (verified, additive-only)

`git diff --numstat 7d6792a ceb8efd -- crates/fsm-parser/`:

| File | +/- | Nature |
|---|---|---|
| `ast/state.rs` | `+50 / -0` | additive accessors (`RegionDecl::initials`, `{After,Every,EveryInternal}Decl::{action_block,duration}`, `action_block_child` helper) |
| `ast/top_level.rs` | `+10 / -0` | additive accessor (`MachineDecl::initials`) |
| `ast/transition.rs` | `+70 / -0` | additive accessors (`PriorityClause::value`, `{Transition,Internal,Local}Decl::payload_binding`, `payload_binding_of` helper) |
| `cst/mod.rs` | `+21 / -3` | module-doc correction ONLY (the -3 = old "for the formatter and LSP" doc lines replaced; **no code**) |

- `git diff --stat 7d6792a ceb8efd -- crates/fsm-parser/src/grammar/
  crates/fsm-parser/src/cst/kinds.rs crates/fsm-parser/Cargo.toml` →
  **empty** (grammar / `SyntaxKind` / parser-Cargo byte-untouched —
  Doc 29 §5.1 / §6 DO-NOT honoured).
- `git diff 7d6792a ceb8efd -- Cargo.lock` → **empty** (C-3 / 0 new deps
  CONFIRMED).
- `git diff --stat 7d6792a ceb8efd -- crates/fsm-formatter/ crates/fsm-lsp/`
  → **empty** (the legitimate `cst::` consumers byte-untouched — Doc 29 §5.2
  blast-radius claim CONFIRMED: exactly two crates edited).
- `git diff --stat 7d6792a ceb8efd -- crates/fsm-analyzer/tests/` →
  **empty** (NO test files touched — the byte-identity guards incl.
  `lower_split_byte_identity.rs` are **UNMODIFIED**, Doc 29 §4.2(1) /
  §6 DO-NOT honoured; this is the structural proof the implementer did not
  edit a guard to go green).
- `git diff --stat 7d6792a ceb8efd -- docs/` → **empty** (W0 *impl* did not
  touch docs; the GATE_VERIFICATION_v1_3 / Doc 00 §11 entries are
  orchestrator-folded at tag time, per Doc 29 §6 — correct).

The accessors are behaviour-inert **by construction**: each body is the
*exact* CST walk the analyzer performed today, relocated to its correct home
(verified verbatim in §2). The §11.1 independent empty `diff -r` over the
full IR + diagnostics corpus is the mechanical proof; this audit confirms
the *construction* is sound (the relocations are byte-faithful, not
re-implementations).

### 1.2 Latent foot-gun assessment for the high-fan-out V1 consumer

The one place a high-fan-out consumer could be bitten is the **diagnostics
stream ordering** (the V1 oracle is the diagnostic code+range vector).
W0 retains the order-critical traversals as R-2 residuals *precisely
because* reordering them would change the unsorted diagnostic vector
(§6 re-derivation). Therefore the post-W0 analyzer emits the **byte-identical
diagnostic stream** the pre-W0 analyzer did — the §11.1 empty-`diff -r` over
`tests/conformance` negative fixtures is the direct evidence. V1's oracle
computed against post-W0 `fsm-analyzer` == the oracle that would have been
computed against pre-W0 `fsm-analyzer`. **No latent foot-gun introduced.**

The only *new* public surface is ~7 `pub fn` accessors on
already-declared `ast_node!` types in `fsm-parser`. Clippy `-D warnings`
clean (§11.1) means every one is consumed by the analyzer migration (no
`unreachable_pub`) — which is *also* the proof each replaced a real coupling
site, not dead API. A future `fsm-parser` AST-shape change now ripples to
**these accessor bodies** (one crate, the correct home) instead of ~14
analyzer files — the audit's P1-A1 intent, materially achieved for the
A/B-clean subset.

**Lens 1 verdict: SOUND.** V1's oracle may be computed against the post-W0
analyzer.

---

## 2. Migrated-site spot-checks (the proof the work was actually checked)

Three sites spot-checked against pre-W0 (`7d6792a`) behaviour intent.

### Spot-check #1 — `checks/parallel.rs` (Archetype-A, the sole full removal)

- **Pre-W0:** `region.syntax().children().any(|c| c.kind() ==
  SyntaxKind::INITIAL_DECL)`.
- **Post-W0:** `region.initials().next().is_some()` where
  `RegionDecl::initials()` = `children(&self.0)` → `AstChildren<InitialDecl>`
  (= `children().filter_map(InitialDecl::cast)`).
- **Equivalence:** `INITIAL_DECL` is the only `SyntaxKind` that casts to
  `InitialDecl`, so `.children().any(kind == INITIAL_DECL)` and
  `children().filter_map(InitialDecl::cast).next().is_some()` are provably
  the same existence predicate. The E0600 diagnostic remains keyed by
  `span_of(region.syntax())` (unchanged) → diagnostic order preserved too.
- The file's only `cst::` use was this one `SyntaxKind`; it correctly drops
  `use fsm_parser::cst::SyntaxKind`. **PASS — behaviour-identical.**

### Spot-check #2 — `lower/state.rs` (B-clean migrations on the byte-pinned lowering path)

The most behaviourally-critical site (`lower_split_byte_identity.rs` pins
this byte-for-byte). Verified each relocation by diffing the *deleted*
analyzer helper against the *added* parser accessor:

- `extract_trigger_payload_binding(t.syntax())` → `t.payload_binding()`:
  the deleted `fn extract_trigger_payload_binding` and the added
  `transition.rs::payload_binding_of` are **byte-identical** (same
  skip-trivia / `_on` / `_trigger` / `LParen` / `Ident` / `RParen` token
  walk). Verbatim relocation.
- `extract_priority(p.syntax())` → `p.value()`: the deleted `fn
  extract_priority` and `PriorityClause::value` are **byte-identical** (find
  first `IntLiteral`, strip `_`, decimal `str::parse`). **Non-obvious
  judgment call correctly flagged:** the accessor doc explicitly preserves
  the *decimal-only* `str::parse` (NOT `parse_int_literal_i64`) — a
  `priority 0x10` parsed to `None` (default) pre-W0; changing the radix
  handling would alter lowered-IR bytes. This subtlety is documented at the
  accessor, exactly the SUBAGENT §8 disclosure expected.
- `action_block_under(a.syntax())` → `a.action_block()`: the deleted `fn
  action_block_under` and `state.rs::action_block_child` are
  **byte-identical** (first `ACTION_BLOCK` child → `cast`). Verbatim
  relocation.
- `duration_ms(a.syntax())` → `a.duration().and_then(|ce|
  duration_ms(&ce))`: subtlest change. Pre-W0 `duration_ms(node)` =
  `node.children().find(kind == CONST_EXPR)` → `.children().next()` →
  `eval_i64`. Post-W0: `a.duration()` = `super::child::<ConstExpr>` (= first
  child castable to `ConstExpr`, i.e. kind `CONST_EXPR`) → `duration_ms(&ce)`
  now does `ce.syntax().children().next()` → `eval_i64`. The
  `find(kind==CONST_EXPR)` and `child::<ConstExpr>` select the same node;
  the inner `.children().next()` + `eval_i64` are unchanged. **Equivalent.**

All four are byte-faithful relocations, not re-implementations.
**PASS — behaviour-identical.**

### Spot-check #3 — `checks/name_resolution.rs` (B-clean initial-count + R-2/R-3 residual)

- Pre-W0: two separate `m.syntax().children().filter(kind ==
  INITIAL_DECL)` walks (one `.count()`, one `.enumerate()` for E0108).
- Post-W0: collected once via `m.initials()` into `Vec<InitialDecl>`, then
  `.len()` + `.iter().enumerate()`.
- **Equivalence:** `AstChildren<InitialDecl>` yields exactly the
  `INITIAL_DECL` nodes in the same source order. The E0108 emission changed
  from `span_of(&c)` (`c: SyntaxNode`) to `span_of(c.syntax())`
  (`c: InitialDecl`); `InitialDecl::syntax()` returns the wrapped node, so
  the span is identical. The "skip first, flag 2nd+" (`if i > 0`) and the
  E0107 empty-machine branch are preserved verbatim. **PASS.**
- The surrounding `descendants()`-dispatch + expression walks correctly
  stay as the R-2/R-3 residual with a thorough module-header comment.

**Conclusion:** the migration is a faithful relocation of behaviour, with
the one non-obvious decision (decimal-only priority parse) correctly
disclosed at the site. No re-implementation drift.

---

## 3. Lens 3 — Canonical R-1..R-4 residual catalogue (authoritative gate input)

This is the precise input the orchestrator records as the new
accepted-tracked-debt in `GATE_VERIFICATION_v1_3.md` / Doc 00 §11 (Doc 28
§6.2(1)). **The shipped residual posture is broader than Doc 29 §3.4's four
named lowering sites: R-2/R-3 were correctly *generalized* to the
checks-side `descendants()`/shallow-AST passes** (see N-1 / §4). The
authoritative post-W0 residual set is **13 `fsm-analyzer` files** retaining
`use fsm_parser::cst::*`, each carrying an explicit W0/Doc-29-§3.4
leave-and-explain comment (verified present in all 13). `parallel.rs` is the
**only** file that fully shed its `cst::` import.

### R-1 — OPAQUE-BUG-1 parent-node type resolution (correctness-load-bearing)

| Site | Verified comment anchor |
|---|---|
| `crates/fsm-analyzer/src/lower/machine.rs:17-33` (head comment); applied at `lower_type_ref_node` / the `FIELD_DECL`/`PARAM` OPAQUE-BUG-1 sites `~:261,310` | "**W0 / Doc 29 §3.4 (R-1 + R-3 leave-and-explain).** Retains `fsm_parser::cst` for: (R-1) `lower_type_ref_node` — the OPAQUE-BUG-1 fix. The typed `Param::ty()`/`FieldDecl::ty()`/`ExternDecl::return_type()` cast only to `TypeRef` and *cannot see* an `OPAQUE_TYPE_REF` sibling, so they returned `None` and the param/field silently vanished with no diagnostic (a P0-1-class silent-data-loss bug …). Resolving from the *parent* node is *more correct* than the typed accessor; adding a `TypeOrOpaque` enum … is a **new public type on the most behaviourally-critical seam** in a non-behavioural wave — the conservative call is leave-and-explain (the option is recorded for a later dedicated parser-API wave; removing the coupling here would risk re-introducing the exact P0-1-class regression W0 is forbidden from causing — C-2); (R-3) `lower_literal`'s shallow-expr literal walks. … Doc 00 §11.44/§11.49, the DRIFT-2 `LineIndex` precedent. Left-and-explained." |
| `crates/fsm-analyzer/src/symbol_table.rs:18-33` (R-1 + R-2) | "(R-1) `primitive_type_text` — same OPAQUE-BUG-1-adjacent type-ref-token resolution as `lower::machine::lower_type_ref_node` (adding a `TypeOrOpaque` typed accessor is a new public type on the most behaviourally-critical seam, deferred to a dedicated parser-API wave) …" |
| `crates/fsm-analyzer/src/checks/type_check.rs:11-26` (R-1 + R-2 + R-3) | "**W0 / Doc 29 §3.4 (R-1 + R-2 + R-3 leave-and-explain).** Retains …" (R-1 instance = `ty_primitive_name` type-ref token resolution) |

### R-2 — Heterogeneous source-ordered / document-pre-order dispatch (IR-id + unsorted-diagnostic byte-load-bearing)

| Site | Verified comment anchor |
|---|---|
| `crates/fsm-analyzer/src/lower/state.rs` — `lower_state_children` (`~:30-97`) + `extract_history` (head comment `~:807-830`) | module head `:17-31`: "(R-2) `lower_state_children`'s `children()+kind()` dispatch over INITIAL/STATE/REGION/FINAL/HISTORY/CHOICE/JUNCTION/FORK/JOIN children in **exact source order across heterogeneous kinds** — the IR id-minter (`lower_split_byte_identity.rs` pins it byte-for-byte); the typed AST has per-kind iterators but no ordered heterogeneous-child iterator, and adding a typed `enum StateChild` … is the refactor-to-number trap …"; `extract_history` carries its own R-2 block ("the *first* history child in **source order** is the one bound to the composite …") |
| `crates/fsm-analyzer/src/checks/name_resolution.rs:18-46` (R-2 + R-3) | "1. **Document-order dispatch (R-2 class).** `check_machine` walks `m.syntax().descendants()` in rowan **document pre-order** … The emitted diagnostic vector is **unsorted** (`fsm_analyzer::checks::run_all` appends in pass order; `fsm-cli`'s `emit_json_aggregate` emits in collected order — neither sorts), so the *traversal order across heterogeneous node kinds is byte-load-bearing* for the diagnostics stream the W0 §4.2 gate pins. …" |
| `crates/fsm-analyzer/src/symbol_table.rs:18-33` (R-1 + R-2) | "(R-2) `build_submachine_entry`'s document-pre-order `sm.syntax().descendants()` harvest and `collect_pseudo_states`' heterogeneous-kind dispatch over FINAL/HISTORY/CHOICE/JUNCTION/FORK/JOIN/ENTRY_POINT/EXIT_POINT … the emitted duplicate-name diagnostics are unsorted so traversal order is byte-load-bearing for the W0 §4.2 gate. …" |
| `crates/fsm-analyzer/src/checks/history.rs:12+` | "**W0 / Doc 29 §3.4 (R-2 leave-and-explain, generalized to this check).** …" |
| `crates/fsm-analyzer/src/checks/timer.rs:16+` (R-2 + R-3) | "**W0 / Doc 29 §3.4 (R-2 + R-3 leave-and-explain, generalized to this …** …" |
| `crates/fsm-analyzer/src/checks/type_check.rs:11-26` (R-1 + R-2 + R-3) | R-2 instance = the `machine.syntax().descendants()` STMT_ASSIGN/GUARD_CLAUSE document-order dispatch |
| `crates/fsm-analyzer/src/checks/submachine.rs:47+` (R-2 + R-4) | "**W0 / Doc 29 §3.4 (R-2 + R-4 leave-and-explain).** Retains …" (R-2 instance = `file.syntax().descendants()` SUBMACHINE_DECL/REF document-order scan) |
| `crates/fsm-analyzer/src/checks/action_lint.rs:29+` (R-2 + R-4) | "**W0 / Doc 29 §3.4 (R-2 + R-4 leave-and-explain).** Retains …" |

### R-3 — Deliberately-shallow expression/statement sublanguage

| Site | Verified comment anchor |
|---|---|
| `crates/fsm-analyzer/src/lower/expr.rs:20-34` (whole file — the canonical R-3) | "**W0 / Doc 29 §3.4 (R-3 — the canonical leave-and-explain residual).** This whole file deliberately retains `fsm_parser::cst`. The action/guard sublanguage typed AST is **intentionally shallow** (`Expr`/`Stmt` are `cast`/`syntax`-only discriminated unions with zero structural accessors, by design …). Folding this would require ~12 new parser accessors … whose bodies are *the same CST walks moved across the crate boundary* — relocating, NOT eliminating, the shape dependency … precisely the 'contort to hit zero coupling' anti-pattern SUBAGENT §10 + Doc 00 §11.44/§11.49 forbid; it is the dominant blast-radius driver. Left-and-explained — the DRIFT-2 `LineIndex` precedent verbatim. …" |
| `crates/fsm-analyzer/src/lower/state.rs` — `duration_ms` / `eval_i64` (`~:798-825`) | "**R-3 residual (Doc 29 §3.4).** `eval_i64` walks the *deliberately shallow* expression CST (`EXPR_LITERAL`/`EXPR_UNARY`/`EXPR_PAREN`) … This is internal-to-analyzer const folding over the parser's *public* CST type used for its intended purpose; it is left-and-explained, the canonical SUBAGENT §10 / Doc 00 §11.44 case." |
| `crates/fsm-analyzer/src/checks/determinism.rs:22-33` | "**W0 / Doc 29 §3.4 (R-3 leave-and-explain).** This file deliberately retains `fsm_parser::cst` for the guard-shape classification internals (`classify_binary` / `lhs_field` / `literal_value` …). A full typed accessor layer (`ExprBinary::{op,lhs,rhs}`, …) would relocate — not eliminate — these CST walks … The `priority N` extraction WAS relocated to the typed `PriorityClause::value()` accessor (W0 §3.2 B-clean)." |
| `crates/fsm-analyzer/src/checks/name_resolution.rs:18-46` (R-2 + R-3) | "2. **Shallow-AST expression name resolution (R-3 class).** `check_expr` / `call_name` / `check_stmt_*` walk the *deliberately shallow* `Expr`/`Stmt` CST …" |
| also generalized into `checks/timer.rs`, `checks/type_check.rs`, `lower/machine.rs` (R-3 sub-clauses, see their head comments) | — |

### R-4 — `util::span_of` / `loc_of` / `submachine_ref_is_nested` positional helpers (Archetype-C)

| Site | Verified comment anchor |
|---|---|
| `crates/fsm-analyzer/src/util.rs:5-19` (canonical R-4; `span_of` at `~:163`, `loc_of` at `~:172`) | "**W0 / Doc 29 §3.4 (R-4 — the Archetype-C leave-and-explain residual).** This file legitimately retains `fsm_parser::cst` for the *positional* helpers: `span_of(&SyntaxNode) -> Span` is pure rowan-positional (`node.text_range()` … used by 79 `span_of(x.syntax())` call sites crate-wide); `loc_of` builds on it; `submachine_ref_is_nested` is a `.parent()`-walk structural predicate (the W2a P1-2 defence-in-depth). There is **no typed-AST equivalent** … These `pub` fns have **zero cross-crate callers** (verified: the only cross-crate `fsm_analyzer::util` use is `compute_line_col` in `fsm-lsp/hover.rs`, unrelated to the CST seam) — de-facto crate-internal, not a public contract. Threading a typed wrapper through 79 call sites would be massive non-behavioural churn for zero clarity gain — explicitly the DRIFT-2 anti-pattern (Doc 00 §11.44/§11.49, the `LineIndex` precedent). Left-and-explained." |
| `crates/fsm-analyzer/src/lower/loc.rs:11-18` (R-4 transitive) | "**W0 / Doc 29 §3.4 (R-4 leave-and-explain — transitive).** `LocCtx::loc` takes a `&SyntaxNode` purely to feed `util::span_of` … Folding it would mean threading a typed wrapper through every lowerer's `loc(x.syntax())` call for zero clarity gain — the DRIFT-2 anti-pattern (Doc 00 §11.44/§11.49). Left-and-explained." |
| `crates/fsm-analyzer/src/checks/submachine.rs:47+` (R-2 + R-4) | R-4 instance = `submachine_ref_is_nested` positional predicate use |
| `crates/fsm-analyzer/src/checks/action_lint.rs:29+` (R-2 + R-4) | R-4 instance = `span_of`/positional helper use |

**Catalogue summary for the orchestrator's GATE_VERIFICATION_v1_3 /
Doc 00 §11 closure entry:**

> §11.49 `analyzer→parser-CST coupling` cleanup **substantially paid as
> v1.3-W0 (`ceb8efd`), with documented reasoned residuals**. The clean
> Archetype-A removal (`parallel.rs`, full `cst::` shed) + the B-clean
> single-construct accessor relocations (`PriorityClause::value`,
> `{Transition,Internal,Local}Decl::payload_binding`,
> `{After,Every,EveryInternal}Decl::{action_block,duration}`,
> `{Machine,Region}Decl::initials`) landed; **13 `fsm-analyzer` files
> deliberately retain `use fsm_parser::cst::*` as the R-1..R-4
> leave-and-explain residual** (R-1 OPAQUE-BUG-1 parent-node type
> resolution — P0-1 silent-data-loss risk if removed; R-2 heterogeneous
> source-ordered / document-pre-order dispatch — IR-id + **unsorted
> diagnostic vector** byte-load-bearing; R-3 deliberately-shallow
> expression/statement sublanguage — folding relocates not eliminates the
> walk, dominant blast-radius; R-4 `util::span_of`-family positional
> helpers — pure rowan-positional, zero cross-crate callers). Each carries
> a DRIFT-2-form code comment citing Doc 00 §11.44/§11.49 + the `LineIndex`
> precedent. This is a **success of the C-1 / SUBAGENT §10 discipline**,
> precedent-consistent with how DRIFT-2 and v1.1 pub-hygiene closed (paid
> where clean, left-and-explained where forcing it was worse-EV). The
> shipped residual scope is **broader** than Doc 29 §3.4's four named
> lowering sites because R-2/R-3 were correctly generalized to the
> checks-side `descendants()`/shallow-AST passes (see N-1).

---

## 4. Lens 2 + Lens 4 — Is §11.49 genuinely "substantially paid with reasoned residuals"? Any W0-introduced drift?

### 4.1 Discipline assessment: substantially-paid, NOT contorted, NOT under-delivered

The residual comments are **textbook DRIFT-2 form**: every one (i) names
the residual class (R-1..R-4), (ii) cites Doc 00 §11.44/§11.49 and the
`LineIndex` precedent, (iii) explains *why folding is worse-EV* (P0-1
regression risk / order-byte-load-bearing / relocate-not-eliminate /
zero-clarity-gain churn), (iv) is conservative (R-1 explicitly defers the
`TypeOrOpaque` widening to a dedicated parser-API wave rather than
sneaking it into a non-behavioural wave). This is the C-1 success
criterion met, not the SUBAGENT §10 "contort to hit zero `cst` count"
anti-pattern. No residual rationalizes coupling that *could* have been
cleanly removed — the one genuinely clean, order-independent Archetype-A
site (`parallel.rs`) **was** removed; the rest are order-/shape-/positional-
load-bearing with sound justifications I independently checked (§6).

### 4.2 N-1 DRIFT — Doc 29 §3.1 plan-of-record is stale & internally inconsistent (must be reconciled in the W0 closure record)

**Finding.** Doc 29 §3.1 ("Archetype-A sites — cleanly removable") asserted
as plan-of-record:

> "**Disposition:** the pure-A rows (`name_resolution` descendants
> dispatch, `type_check`, `timer`, `submachine`, `symbol_table`) are
> **cleanly removable** … ~7 files lose `use fsm_parser::cst::*` entirely
> with zero behaviour risk".

**Shipped reality:** exactly **one** file (`parallel.rs`) fully shed its
`cst::` import. `name_resolution.rs`, `type_check.rs`, `timer.rs`,
`submachine.rs`, `symbol_table.rs` — the §3.1-named "pure-A cleanly
removable" set — **all retain** `use fsm_parser::cst::*` and were
reclassified as R-2/R-3 **generalized** leave-and-explain residuals.

**Root cause — Doc 29 contradicted itself.** §3.1 claimed the
`descendants()`-dispatch passes were cleanly routable through
`util::walk_all_states` + per-kind typed iterators "with zero behaviour
risk." But `util::walk_all_states` traverses **states-first-then-regions**
(`m.states()` → `collect_nested_states` → `m.regions()`), whereas
`m.syntax().descendants()` is strict **rowan document pre-order**. For a
check whose diagnostic vector is **unsorted** (independently confirmed,
§6), `walk_all_states` order ≠ `descendants()` order the moment a machine
interleaves regions with states — which would **reorder the diagnostic
stream and fail the §4.2 byte-identity gate**. Doc 29 §3.4-R-2 itself
correctly establishes that this traversal order is byte-load-bearing. So
§3.1's "cleanly removable" disposition was **internally inconsistent with
§3.4-R-2 in the plan as authored** — an over-optimistic estimate, not a
shipped failure.

**Disposition: this is the CORRECT DRIFT-2/§10 call, NOT under-delivery.**
The implementer correctly discovered §3.1's optimism was wrong and
narrowed to (a) the one genuinely-clean Archetype-A removal, (b) the
B-clean single-construct accessor relocations, (c) generalizing R-2/R-3 to
the checks-side `descendants()`/shallow-AST passes — exactly the
"leave-and-explain rather than contort to hit a number" behaviour the
discipline *mandates*. The established audit context confirms this
("the reduced cleanup scope is the correct DRIFT-2/§10 call, not
under-delivery"). The byte-identity gate (§11.1 empty `diff -r`) is the
proof the narrowing preserved behaviour.

**Required orchestrator action (record-keeping, not a code fix):**

1. **Doc 29 §3.1 needs a one-line reconciliation note** (or a Doc-28-§1-style
   stale-framing row): "§3.1's 'pure-A cleanly removable / ~7 files lose
   the import' disposition was over-optimistic and internally inconsistent
   with §3.4-R-2 (the `descendants()`-dispatch passes are document-order /
   unsorted-diagnostic byte-load-bearing — `walk_all_states` order ≠
   `descendants()` order); shipped reality: `parallel.rs` is the sole full
   removal, the rest are R-2/R-3 generalized residuals — the correct
   DRIFT-2 call." Doc 29 is the W0 plan-of-record and its §3.1 is now
   stale; leaving it unreconciled would mislead a future reader the way the
   Doc 27 stale-framing rows (R-14/R-15) would have.
2. **GATE_VERIFICATION_v1_3 / Doc 00 §11 must record the SHIPPED residual
   posture (this doc §3 — 13 files, R-2/R-3 generalized), NOT Doc 29
   §3.4's narrower four-site list nor §3.1's ~7-file removal estimate.**
   The W0 commit message itself already states the shipped reality
   accurately ("the §3.4 lowering-side residuals plus their
   analyzer-checks-side instances … are all left-and-explained"); the gate
   doc should mirror the commit + this §3 catalogue, not the pre-W0 plan
   estimate.

This is a **documentation-coherence drift**, not a behavioural or
discipline defect. It does **not** block V1 (the analyzer is
behaviour-identical regardless of how the residual is *described*).

### 4.3 Other coherence checks

- **`cst/mod.rs` module-doc correction — VERIFIED ACCURATE (Lens 4).** The
  new doc claims (a) `fsm-formatter` does **not** consume the `cst::` path,
  (b) `fsm-lsp` is the legitimate primary consumer (12 modules), (c)
  analyzer uses `cst` only for R-1..R-4. Independently checked:
  - `grep fsm_parser::cst crates/fsm-formatter/src/` → **zero hits**;
    formatter uses only top-level re-exports (`fsm_parser::{SyntaxNode,
    SyntaxToken,SyntaxKind}`). The old AUDIT_B:115 "for the formatter and
    LSP" framing **was** stale; this is a correct verify-the-record fix.
  - `grep -l fsm_parser::cst crates/fsm-lsp/src/` → **exactly 12 modules**
    (refs, semantic_tokens, resolve, hover, code_action, document_symbol,
    folding, complete, inlay_hints, references, definition, rename) —
    matches the doc's "12 modules" claim exactly.
  - analyzer residual = 13 files, all R-1..R-4 (§3). Accurate.
- **ROADMAP §72 / metrics `2026-05-16.json` / GATE_VERIFICATION_v1_2 §6**
  still carry the pre-W0 "~15 files / 16 sites / 24-site change" framing.
  These are **correctly stale-by-design**: they are *v1.2-era* documents
  describing the *deferred (not-yet-paid)* debt. They are **not** W0's
  concern to update; the forward-looking closure belongs in
  GATE_VERIFICATION_v1_3 / Doc 00 §11 (a *new* §11.N row), authored by the
  orchestrator at the v1.3 tag, per Doc 28 §6.2(1) / Doc 29 §0. No drift —
  flagged so the orchestrator does not mistake them for the W0 closure
  record.
- **Doc 28 §5 precondition 2** ("v1.3-W0 … landed, post-merge-quad-green,
  **phase-audit-clean**") — this document **is** the phase-audit-clean
  artifact. With this audit returning PROCEED-WITH-NOTES (no V1
  ship-blocker), precondition 2 is satisfiable.
- **GATE_VERIFICATION_v1_3.md does not yet exist** — expected and correct
  (authored at the v1.3 tag, per the v1.0/v1.1/v1.2 pattern). The W0 *impl*
  correctly did not author it (Doc 29 §6 DO-NOT). This audit's §3 supplies
  the residual list that document will record.

---

## 5. Toolchain-pin correction (corrects the §11.1 agent's transcript — MANDATORY record)

**The §11.1 post-merge verification agent's "rustc 1.95.0 / pin not
honored" line was a PROBE-CONTEXT ERROR, not a pin violation. The pin IS
honored.** Re-derived this audit:

- `sh -c 'cd /root/dev/embeded-fsm-sdk && rustup show active-toolchain'`
  → **`1.75.0-x86_64-unknown-linux-gnu (overridden by
  '/root/dev/embeded-fsm-sdk/rust-toolchain.toml')`**. The pin is honored
  by rustup whenever the repo is the cwd context — which is exactly how
  every `cargo` invocation in the wave/quad runs.
- A bare `rustc --version` returning `rustc 1.95.0 (59807616e 2026-04-14)`
  and `rustup show active-toolchain` (no repo cwd) returning
  `stable-x86_64-unknown-linux-gnu (default)` is **EXPECTED and BENIGN** —
  it is the *box default* (`stable` = `1.95.0`), observed only when the
  probe runs outside the repo. It is **NOT** drift and **NOT** a pin
  violation.

The §11.1 agent evidently ran its toolchain probe without the repo-cwd
context and mis-recorded the box default as a pin failure. **The record is
hereby corrected:** the project's `rust-toolchain.toml` pin to `1.75.0` is
honored for all in-repo `cargo` work; the COLD quad the §11.1 agent
otherwise verified GREEN was built on the correct pinned `1.75.0`
toolchain. W0 is independently confirmed behaviour-preserving **on the
correct toolchain**.

---

## 6. Independent re-derivation — "diagnostics unsorted → traversal order byte-load-bearing" (the R-2 premise)

The single most load-bearing residual premise (R-2, and the root cause of
N-1). Independently re-derived from source — **CONFIRMED TRUE**:

1. **`fsm_analyzer::checks::run_all`** (`crates/fsm-analyzer/src/checks/mod.rs:34-46`):
   eleven check passes (`name_resolution`, `history`, `timer`, `parallel`,
   `submachine`, `defer`, `type_check`, `completion`, `determinism`,
   `import`, `action_lint`) each `check(file, st, out)` into the **shared
   `out: &mut Vec<Diagnostic>` in pass order**. **No sort.** Consumed by
   `lower/mod.rs:86 checks::run_all(&ast, &st, &mut check_diags)`.
2. **Only `.sort` in all of `fsm-analyzer/src`** is
   `checks/determinism.rs:314 ps.sort_unstable()` — it sorts a **local
   `Vec` of priority integers** (`transitions.iter().filter_map(|t|
   t.priority)`) for a distinct-count check inside `analyze_group`. It does
   **not** touch the diagnostic vector. Irrelevant to the premise.
3. **`fsm-cli emit_json_aggregate`** (`crates/fsm-cli/src/cmd/check.rs:108-175`):
   the JSON path does `for d in diags { all_diags.push((d, src, label)) }`
   then `emit_json_aggregate(&all_diags)`, which iterates
   `for (d, src, path) in entries` building the JSON array **in collected
   order**. **No `.sort` on `entries`/`all_diags` anywhere in the
   function.** The only other CLI sorts are `cmd/test.rs:164 out.sort()`
   (test-runner *output lines*) and `generate.rs:514 names.sort()`
   (generated-file *name list*) — both unrelated to the diagnostics stream.

**Therefore:** the order in which `name_resolution::check`'s
`descendants()` document-pre-order traversal, `lower_state_children`'s
heterogeneous `children()+kind()` dispatch, and `symbol_table`'s
`build_submachine_entry` harvest emit diagnostics **IS byte-load-bearing**
for the `fsm check --json` stream that the W0 §4.2 acceptance pins and that
V1's oracle computes from. Folding any of these into per-kind typed
iterators (which cannot preserve cross-kind document order — §4.2) would
reorder diagnostics → fail the byte-identity gate. **R-2 is factually
correct, conservative, and honest — a textbook DRIFT-2 `LineIndex`-precedent
residual, not a refactor-to-number contortion.** This same fact is the
root cause of N-1 (Doc 29 §3.1's "cleanly removable" estimate
under-weighted exactly this constraint).

---

## 7. V1-readiness verdict & required orchestrator actions

**VERDICT: PROCEED-WITH-NOTES.**

The phase boundary is clean enough to dispatch V1. Per Doc 28 §5
precondition 2, V1's diagnostics oracle must be computed against the
post-W0 `fsm-analyzer` — and the post-W0 analyzer is:

- **behaviourally identical** to pre-W0 (§11.1 independent empty `diff -r`
  over the full IR + diagnostics corpus; this audit's three migrated-site
  spot-checks confirm byte-faithful relocation, not re-implementation);
- **additive-only** on `fsm-parser` (zero deletions, zero
  grammar/CST/Cargo change, 0 new deps — all independently verified);
- **disciplined**: the R-1..R-4 residuals are a *success* of C-1 /
  SUBAGENT §10, each with a textbook DRIFT-2 comment; the one genuinely
  clean removal (`parallel.rs`) was made; nothing was contorted to hit a
  number;
- **built on the correctly-pinned `1.75.0` toolchain** (§5 corrects the
  §11.1 transcript error).

No finding is a V1 ship-blocker. The NOTES are orchestrator
record-keeping, to be discharged at the v1.3 tag (or a reconciliation
pass), **before** they can mislead a future reader:

1. **[N-1, required] Add a Doc 29 §3.1 stale-framing reconciliation note**
   (§4.2): §3.1's "pure-A cleanly removable / ~7 files lose the import"
   disposition is stale and was internally inconsistent with §3.4-R-2;
   shipped reality is `parallel.rs` sole-removal + R-2/R-3 generalized
   residuals (the correct DRIFT-2 call).
2. **[N-1, required] Record the SHIPPED residual posture in
   GATE_VERIFICATION_v1_3 / a new Doc 00 §11.N row** using this doc §3's
   13-file R-1..R-4 catalogue (NOT Doc 29 §3.4's four-site list nor §3.1's
   ~7-file estimate). Doc 28 §6.2(1) requires §11.49 recorded as the closed
   item with its residual carried as the new accepted-tracked-debt.
3. **[N-2, required] Record the toolchain-pin correction** (§5) so the
   §11.1 transcript's erroneous "pin not honored" line does not propagate
   into the v1.3 gate record. The pin IS honored.
4. **[procedural] Create the `checkpoint/2026-05-16-w0-clean` annotated
   tag** at `ceb8efd`** per SUBAGENT §11.4 (this clean phase-boundary audit
   is the trigger) — a rollback anchor before V1.

With (1)–(3) folded by the orchestrator, **V1 may be dispatched** against
the post-W0 analyzer per Doc 28 §5.

---

## 8. Audit metadata

- **Lenses applied:** all 5 (analyzer soundness; §11.49 discipline; R-1..R-4
  catalogue; W0-introduced drift/coherence; V1-readiness).
- **Method integrity:** zero `cargo`/build (per mandate); all claims derived
  from `git -C` diffs + source reads + `grep`; the §11.1 mechanical COLD-quad
  + empty-`diff -r` proof is taken as given (independently produced) and
  layered with this judgment audit, not re-run.
- **Spot-checks shown:** 3 migrated sites (parallel.rs / lower-state.rs /
  name_resolution.rs) verified byte-faithful vs `7d6792a`; the
  unsorted-diagnostics R-2 premise independently re-derived from
  `checks/mod.rs` + `cmd/check.rs`.
- **Drift found:** 1 (N-1, documentation-coherence — Doc 29 §3.1 stale &
  self-inconsistent; the correct DRIFT-2 call shipped, the *plan doc* is
  stale). 1 record correction (N-2, toolchain-pin). 0 behavioural defects.
  0 discipline defects. 0 V1 ship-blockers.
- **Branch:** `phase3.1/v1_3-w0-audit` @ `ceb8efd` (manual worktree). NOT
  merged/pushed.

*End of AUDIT_PHASE_W0_2026_05_16 — §11.3 post-W0 phase-boundary audit.*
