# FSM Studio — v1.3-W0 §11.49 `analyzer→parser-CST coupling` Cleanup: Extraction + Seam Design

**Document ID:** FSM-PLAN-W0-CST
**Version:** 1.0.0
**Status:** Living Document — the **plan of record** for v1.3-W0 (the §11.49
CST-coupling debt-paydown wave). Read-only planning artifact: the
coupling-site catalogue, the target seam, the definitive
`is-fsm-parser-in-the-edit-set` resolution, the pre-committed
leave-and-explain residuals, the pre/post-identity acceptance manifest,
the recommended wave shape, and the self-contained W0 implementer brief.
Authored against shipped reality at HEAD `87ffebd` (every claim
`file:line`-verified in source, not from summaries).
**Depends on / governed by:** Doc 28 §4 + §6.1-K-8 + §6.2(1) (the binding
W0 sequencing + discipline contract), Doc 00 §11.49 (owner decision),
Doc 00 §11.44 (the DRIFT-2 leave-and-explain canonical model),
`docs/AUDIT_B_ARCHITECTURE_2026_05_14.md` §"P1-A1" (the catalogued debt),
`docs/processes/SUBAGENT_CONVENTIONS.md` §5.4/§6/§10/§11.1/§11.3,
`docs/GATE_VERIFICATION_v1_2.md` §6 (the accepted-tracked-debt record).

---

## 0. Why a new Doc 29 (numbering justification)

Doc 28 §0 justified itself as a **new numbered sibling** (not an edit of
Doc 27) because the kickoff-pass-as-distinct-artifact pattern is what
caught the Doc-20 §4.5 false-incremental-parse drift. The W0 plan is the
symmetric case one level down: Doc 28 §4 is the **sequencing decision**
("run §11.49 as W0 before V1, DRIFT-2-grade, never parallel"); it is
deliberately *not* the engineering plan (it does not catalogue a single
coupling site, design the seam, or resolve whether `fsm-parser` is in the
edit set — the highest-leverage W0 unknown). A dedicated Doc 29 is the
convention-consistent choice for three reasons:

1. **Precedent symmetry.** Doc 26 (FSM-ARCH-LSP) was the LSP epic's
   engineering plan built on Doc 14's scope; Doc 28 §4 is to W0 what
   Doc 14 was to the LSP epic (the *decision*), and this Doc 29 is the
   Doc-26 analogue (the *plan* — the catalogue, the seam, the brief).
2. **Editing Doc 28 §4 in place would bloat the v1.3 wave-plan** with a
   ~15-file site catalogue + seam ADR + implementer brief, losing the
   "one artifact, one job" separation that keeps the wave plan a
   navigable queue. Doc 28 §4 stays the 1-page sequencing contract; it
   gains one pointer line to Doc 29 when the orchestrator folds the
   reconciliation pass (out of scope for this read-only wave — this wave
   touches **only this new doc**, exactly as Doc 28 §0's last paragraph
   constrained itself).
3. **It is the closed-§11.49 evidence artifact.** Doc 28 §6.2(1) requires
   "the §11.49 item recorded as closed in `GATE_VERIFICATION_v1_3.md`
   (or, if a residual is left-and-explained, that residual recorded as
   the new accepted-tracked-debt with reason)". Doc 29 is where the
   residual-vs-removed disposition is pinned **before** the implementer
   runs, so the gate doc later cites a decided plan, not an open question.

Doc 28 remains the v1.3 plan of record; Doc 29 is the W0 sub-plan built
on Doc 28 §4. ROADMAP §111 / a Doc-00 §11 pointer row are folded by the
orchestrator in a separate pass.

---

## 1. Governing contract — the constraints W0 inherits (restated, binding)

Per Doc 28 §4 / §6.1-K-8 / §6.2(1), Doc 00 §11.49 / §11.44, and
SUBAGENT §5.4/§6/§10/§11.1/§11.3, W0 inherits **all** of the following.
They are not advisory; a deliverable that violates any is rejected back.

| # | Inherited constraint | Source |
|---|---|---|
| C-1 | **DRIFT-2-grade leave-and-explain.** Restructure **only** where a clean, behaviour-safe restructuring genuinely improves clarity. Where removing the coupling would worsen clarity or risk behaviour, **leave it and explain why** in a code comment + the §11 record. A correctly left-and-explained residual is a **success of the discipline, not a failure**. Do NOT contort to hit "zero `cst::*` imports". | Doc 00 §11.44; SUBAGENT §10 last row |
| C-2 | **Non-behavioural.** Zero user-visible change. Acceptance is the SUBAGENT-§5.4 **pre/post-identity proof** (not "tests pass" — *byte-unchanged* across the full corpus, §4). | Doc 28 §4; SUBAGENT §10 row "Refactor waves that change behavior" |
| C-3 | **0 new deps.** `Cargo.lock` byte-unchanged (the DRIFT-2 precedent). No new crate, no version bump. | Doc 28 §4; Doc 00 §11.44 |
| C-4 | **Run BEFORE V1; never in a v1.2.1 patch lane; never parallel with any feature wave.** W0 must land + post-merge-quad-green + phase-audit-clean *before* V1's analyzer-dependent Electron tests are written, so V1's oracle is computed against the post-cleanup `fsm-analyzer`, never a moving target. | Doc 28 §4 (the L0-before-LSP precedent) |
| C-5 | **Orchestrator independently re-runs the gate post-merge** (§11.1); a **phase-boundary audit runs after W0** before V1 dispatch (§11.3). Self-reported green is not proof; warm-stale-`-wt-` caveat applies (§11.1). | SUBAGENT §11.1/§11.3 |
| C-6 | **Scope-boundary discipline.** A "DO-NOT-touch" set is mandatory (§6). If the implementer discovers an out-of-scope need, **report it — do not silently expand** (the cardinal scope-creep bar). | SUBAGENT §6 |
| C-7 | **Recorded as the closed §11.49 item** in `GATE_VERIFICATION_v1_3.md` at tag time (or its residual carried as the new accepted-tracked-debt with reason). | Doc 28 §6.2(1) |

**The single highest-leverage unknown this doc must definitively resolve
(§5):** *is `fsm-parser` in W0's edit set?* — i.e. does W0's seam require
*extending* `fsm-parser`'s public typed-AST API, which changes W0's blast
radius and whether `fsm-parser` is in scope. A plan that does not resolve
this is incomplete (Doc 28 §1.1's "highest-leverage unknown" discipline
applied to W0). **§3 resolves it: YES — bounded, additive-only.**

---

## 2. The catalogued debt (audit citation, verified)

**Audit of record:** `docs/AUDIT_B_ARCHITECTURE_2026_05_14.md`, finding
**P1-A1** (lines 107–115), titled *"Analyzer reaches into
`fsm_parser::cst::{SyntaxKind, SyntaxNode}` instead of the typed AST"*.
Verbatim, the finding's three load-bearing claims:

- **Files (audit, l.109):** `crates/fsm-analyzer/src/{lower.rs,
  symbol_table.rs, checks/*.rs}` — "16 `use fsm_parser::cst::*` sites;
  `crates/fsm-analyzer/src/util.rs:4` (the `span_of(SyntaxNode)` helper is
  built on raw CST)". *(Note: post-AD-3 the monolithic `lower.rs` was
  split into `lower/{mod,machine,state,expr,loc,ids,hash}.rs`; the site
  count migrated with it — §2.1 re-counts against shipped HEAD.)*
- **Observation (audit, l.111):** the parser exposes **both** a typed AST
  (`fsm_parser::ast::*`) and a raw CST (`fsm_parser::cst::*`); the
  analyzer pipeline "was supposed to live above the typed AST" but "mixes
  both — e.g. `name_resolution.rs:74` walks `&SyntaxNode` and matches on
  `SyntaxKind` directly inside check functions. Result: any AST shape
  adjustment in `fsm-parser` becomes a 24-site change in `fsm-analyzer`."
- **Severity (audit, l.113):** **P1**. "Not a layering violation (parser
  exposes both as public API), but a leakage of the parser's CST shape
  into the analyzer."
- **Suggested fix (audit, l.115):** "Promote any check that currently
  matches `SyntaxKind` to take the typed AST node and use its accessors.
  **Where the typed AST lacks an accessor, add it to `fsm-parser::ast`
  rather than dropping to CST.** Document `cst` module as 'for the
  formatter and LSP-incremental-reparse only' — analyzer should not need
  it."

The metrics record `docs/metrics/2026-05-16.json`
`tracked_deferred` corroborates: *"fsm-analyzer directly imports and
walks Rowan CST node types from fsm-parser (~15 files). The correct seam
is analyzer consuming the parser's typed AST API only."* ROADMAP §72
states the method even more decisively: *"analyzer → AST coupling (16
sites reach into fsm-parser's CST instead of using typed AST). **Extend
AST accessors then strip cst::\* imports.**"* — the project's own
roadmap **pre-commits to extending `fsm-parser`'s AST accessors**. This
is the first piece of evidence that resolves §5's scoping question to
YES.

### 2.1 Shipped-HEAD re-count (`87ffebd`) vs the audit's `2026-05-14` count

The audit counted "16 `use fsm_parser::cst::*` sites" pre-AD-3. At
`87ffebd` the verified count (`grep -rn "use fsm_parser::cst\|fsm_parser::cst::"
crates/fsm-analyzer/src`) is **17 `cst`-import lines across 14 files**
(plus `util.rs:4 use fsm_parser::ast;` which is the *typed* import, not a
coupling site). The "~15 files" in Doc 00 §11.49 / the metrics record is
accurate. Per-file raw-CST-type usage density (lines mentioning
`SyntaxNode`/`SyntaxKind`/`fsm_parser::cst`/`rowan`), highest first:

| File | CST-type lines | Coupling shape |
|---|---|---|
| `crates/fsm-analyzer/src/lower/expr.rs` | 71 | **deepest** — action/guard sublanguage tree-rewrite |
| `crates/fsm-analyzer/src/lower/machine.rs` | 45 | type-ref / literal / queue / target token scans |
| `crates/fsm-analyzer/src/lower/state.rs` | 38 | state-tree dispatch + timer/priority/payload extraction |
| `crates/fsm-analyzer/src/checks/determinism.rs` | 37 | guard-shape classification (operator tokens, lit values) |
| `crates/fsm-analyzer/src/checks/name_resolution.rs` | 33 | descendants-by-kind dispatch (the audit's l.74 exemplar) |
| `crates/fsm-analyzer/src/checks/type_check.rs` | 30 | assign/guard descendant dispatch + literal eval |
| `crates/fsm-analyzer/src/checks/timer.rs` | 17 | const-expr eval over CST |
| `crates/fsm-analyzer/src/util.rs` | 9 | `span_of`/`loc_of`/`submachine_ref_is_nested` (CST-typed) |
| `crates/fsm-analyzer/src/symbol_table.rs` | 6 | `primitive_type_text` + a descendants scan |
| `crates/fsm-analyzer/src/checks/submachine.rs` | 6 | descendants-by-kind + `state_has_completion` |
| `crates/fsm-analyzer/src/checks/action_lint.rs` | 4 | `in_action_block` ancestor scan |
| `crates/fsm-analyzer/src/checks/history.rs` | 3 | descendants-by-kind dispatch |
| `crates/fsm-analyzer/src/lower/loc.rs` | 2 | `LocCtx::loc(&SyntaxNode)` (built on `util::span_of`) |
| `crates/fsm-analyzer/src/checks/parallel.rs` | 2 | `children().any(kind == INITIAL_DECL)` |
| `crates/fsm-analyzer/src/lib.rs` | 1 | doc-comment only — **not a coupling site** |
| `crates/fsm-analyzer/src/checks/completion.rs` | 1 | `span_of(t.syntax())` only — **typed-AST-clean** |

`checks/defer.rs`, `checks/mod.rs`, `checks/import.rs`, `lower/mod.rs`,
`lower/ids.rs`, `lower/hash.rs`, `lca.rs`, `scope.rs` are already
typed-AST-only (they import `fsm_parser::ast`, never `::cst::`) — they
are NOT in the catalogue except where they call the shared
`util::span_of` (addressed as the §3.2 boundary).

---

## 3. Exact coupling-site catalogue + per-site disposition

Every catalogued site is one of three **semantic-question archetypes**.
Understanding the archetype is what makes the seam design precise:

- **A — "what kind of node is this?" dispatch** (`node.kind()` match over
  `descendants()`/`children()`, then `ast::X::cast(node)`). The typed AST
  *already* answers this via typed child iterators; the CST walk is
  legacy convenience. → **Cleanly removable.**
- **B — "what structural sub-part does this node have?"** (operator token,
  Nth ident, `ARG_LIST` child, `CONST_EXPR` value, `PRIORITY_CLAUSE`
  value, payload-binding ident, timer duration, opaque-vs-primitive type).
  The typed AST currently **lacks an accessor**, so the analyzer drops to
  CST. → **Removable ONLY by adding the missing accessor to
  `fsm_parser::ast` (the §5 YES); else leave-and-explain.**
- **C — "where is this node in the tree?"** (`.parent()` / `.ancestors()`
  / `.text_range()` for spans / structural ancestor predicates). Rowan
  positional API with no typed equivalent and no clean one. → **Strong
  leave-and-explain candidate** (forcing a typed wrapper here worsens
  clarity for zero behaviour gain — the exact DRIFT-2 `LineIndex`
  precedent).

### 3.1 Archetype-A sites — cleanly removable (→ typed-AST child iterators)

> **⚠ W0 shipped-reality reconciliation note (N-1 — see Doc 00 §11.50; evidence: `docs/AUDIT_PHASE_W0_2026_05_16.md` §4.2).** §3.1's plan-of-record disposition — "the pure-A rows (`name_resolution` descendants-dispatch, `type_check`, `timer`, `submachine`, `symbol_table`) are **cleanly removable** … ~7 files lose `use fsm_parser::cst::*` entirely with zero behaviour risk" — was **over-optimistic and internally inconsistent with §3.4-R-2**. `util::walk_all_states` traverses **states-first-then-regions**, whereas `m.syntax().descendants()` is strict **rowan document pre-order**; for a check whose diagnostic vector is **unsorted** (independently re-derived true), those orders diverge the moment a machine interleaves regions with states — which would reorder the diagnostic stream and fail the §4.2 byte-identity gate. **Shipped reality:** `checks/parallel.rs` is the **sole** file to fully shed its `cst::` import; the §3.1-named "pure-A" files were correctly reclassified as **R-2/R-3 generalized leave-and-explain residuals** (13 `fsm-analyzer` files retain `cst::` total). This is the **correct DRIFT-2/§10 call, NOT under-delivery** — the implementer correctly narrowed when §3.1's optimism proved wrong (the byte-identity gate is the proof behaviour was preserved). The §3 plan below remains as authored for the archetype taxonomy; `docs/AUDIT_PHASE_W0_2026_05_16.md` §3 + Doc 00 §11.50 are authoritative for the *shipped* residual posture.

These walk `descendants()`/`children()` + `match kind()` purely to *find*
a node they then `cast` to typed AST. The typed AST already exposes the
equivalent typed iterator; the rewrite is a behaviour-preserving swap to
the typed walker (traversal **order must be preserved** — see §4).

| Site (`file:line`) | CST type used | Semantic question | Typed-AST answer that already exists |
|---|---|---|---|
| `checks/name_resolution.rs:69-71` (`m.syntax().descendants()` → `check_node`) and `:74-76` (`check_node(node: &SyntaxNode)` `match node.kind()`) — the audit's l.74 exemplar | `SyntaxNode`, `SyntaxKind` | "visit every transition/initial/history/send node in the machine" | `MachineDecl::states()`+`util::walk_all_states` + `StateDecl::{transitions,internal_transitions,local_transitions,completions,after,every}` typed iterators (already used elsewhere in this same crate, e.g. `determinism.rs:92-122`, `defer.rs:34-50`) |
| `checks/name_resolution.rs:36-55` (`m.syntax().children().filter(kind==INITIAL_DECL).count()`) | `SyntaxKind` | "how many `initial` decls directly under this machine; flag 2nd+" | needs an `MachineDecl::initials() -> AstChildren<InitialDecl>` accessor (currently only `MachineDecl::initial()` returns the *first*) → **A-with-a-tiny-B-accessor**, see §3.3 |
| `checks/type_check.rs:22-25` (`machine.syntax().descendants()` `match kind STMT_ASSIGN/GUARD_CLAUSE`) | `SyntaxNode`, `SyntaxKind` | "visit every assignment / guard in the machine" | `walk_all_states` + per-`StateDecl` typed transition/action iterators + `GuardClause`/`ActionBlock::statements()` (typed `Stmt` union already exists) |
| `checks/timer.rs:29-34` (`m.syntax().descendants()` `match AFTER/EVERY/EVERY_INTERNAL`) | `SyntaxKind` | "visit every timer decl" | `walk_all_states` + `StateDecl::{after,every,every_internal}()` typed iterators (already typed-exposed) |
| `checks/history.rs:21-24` (`m.syntax().descendants()` `match SHALLOW/DEEP_HISTORY_DECL`) | `SyntaxKind` | "visit every history pseudo-state" | a new `StateDecl::history_decls()` / `MachineDecl` history iterator → **A-with-a-tiny-B-accessor**, §3.3 |
| `checks/submachine.rs:69-72` (`file.syntax().descendants()` `match SUBMACHINE_DECL/REF`) and `:93-95` | `SyntaxKind` | "visit every submachine template + ref" | `File::submachines()` exists; `StateDecl::submachine_ref()` exists — replace the descendants scan with typed iteration over `walk_all_states` + `submachine_ref()` |
| `symbol_table.rs:483` (`sm.syntax().descendants()` scan) | `SyntaxNode`, `SyntaxKind` | symbol harvest over a submachine subtree | typed `SubmachineDecl` accessors (same surface as `MachineDecl`) |
| `lower/state.rs:43-95` (`parent.children()` `match kind` INITIAL/STATE/REGION/FINAL/HISTORY/CHOICE/JUNCTION/FORK/JOIN_DECL) | `SyntaxNode`, `SyntaxKind` | "lower each direct child state-ish node, **in source order**" | **PARTIALLY** — `StateDecl::{nested_states,regions,after,every,...}` exist but **there is no single typed "ordered heterogeneous child" iterator**. This site is the §3.2 hard case: order across *heterogeneous* kinds is load-bearing for IR id-minting determinism (`build_transition`/`next_pseudo_id` counters). → **leave-and-explain candidate** (§3.4 R-2) unless a typed ordered-child enum is added (a non-trivial parser API surface — see §5 blast-radius). |

**Disposition:** the pure-A rows (`name_resolution` descendants dispatch,
`type_check`, `timer`, `submachine`, `symbol_table`) are **cleanly
removable** by routing through the *already-existing* typed iterators +
`util::walk_all_states`. This is the bulk of the 17 import sites and the
audit's primary intent. Estimated: ~7 files lose `use fsm_parser::cst::*`
entirely with zero behaviour risk (the typed iterators are already
proven by the tests that currently pass — `determinism.rs`/`defer.rs`
use exactly this pattern).

### 3.2 Archetype-B sites — removable ONLY by extending `fsm_parser::ast`

These are the sites where the analyzer drops to CST **because the typed
AST has no accessor for the structural sub-part it needs**. This is the
crux of §5. For each, the missing accessor is named precisely.

| Site (`file:line`) | CST type used | Semantic question (the structural sub-part) | Missing `fsm_parser::ast` accessor that W0 must add |
|---|---|---|---|
| `lower/state.rs:885-892` `extract_priority` (also `determinism.rs:133`, every transition lowerer) | `SyntaxNode`, `SyntaxKind::IntLiteral` | "the integer value inside a `priority N` clause" | `impl PriorityClause { pub fn value(&self) -> Option<i64> }` — `PriorityClause` is declared (`ast/transition.rs:10` `ast_node!(PriorityClause, PRIORITY_CLAUSE)`) but **has no `impl` block / no accessor** |
| `lower/state.rs:813-834` `extract_trigger_payload_binding` | `SyntaxNode`, token sequence `KwOn IDENT LParen IDENT RParen` | "the payload-binding ident in `on E(p) -> …`" | `impl TransitionDecl/InternalDecl/LocalDecl { pub fn payload_binding(&self) -> Option<String> }` — no typed node exists for the binding (the source comment at `state.rs:809-812` says exactly this) |
| `lower/state.rs:838-842` `action_block_under` + `:844-883` `duration_ms`/`eval_i64` | `SyntaxNode`, `SyntaxKind::{CONST_EXPR,EXPR_*}` | "the action block + the `N ms` duration of an `after`/`every`/`every_internal`" | `impl AfterDecl/EveryDecl/EveryInternalDecl { pub fn action_block(&self) -> Option<ActionBlock>; pub fn duration(&self) -> Option<ConstExpr> }` — `AfterDecl`/`EveryDecl` expose only `target()`/`branch_hint()` (`ast/state.rs:211-231`); `EntryDecl/ExitDecl` already have `action_block()` (the exact precedent at `ast/state.rs:235-245`) |
| `lower/machine.rs:462-495` `lower_type_ref_node` (also `symbol_table.rs:443` `primitive_type_text`, `type_check.rs:193` `ty_primitive_name`) | `SyntaxNode`, `SyntaxKind::{TYPE_REF,OPAQUE_TYPE_REF,Kw*}` | "the type of a field/param/return, **including the `opaque \"C\"` sibling kind** the typed `.ty()` can't see" | this is the **OPAQUE-BUG-1 root cause** (documented at `lower/machine.rs:450-461`): `Param::ty()`/`FieldDecl::ty()`/`ExternDecl::return_type()` cast only to `TypeRef`, missing `OpaqueTypeRef`. Add a unifying `impl {Param,FieldDecl,PayloadField,ExternDecl} { pub fn type_ref(&self) -> Option<TypeOrOpaque> }` (an `enum TypeOrOpaque { Ty(TypeRef), Opaque(OpaqueTypeRef) }` in `fsm_parser::ast`) **OR** keep the current parent-node resolution as a documented leave-and-explain (§3.4 R-1 — this is a *correctness-load-bearing* site; see the conservative recommendation there) |
| `lower/expr.rs` (whole file, 71 lines) — `lower_*_expr`/`lower_guard_*`: operator-token extraction (`binary_op_from_kind`, `:327-331`), operand ordering (`children().filter_map(Expr::cast)`), `ARG_LIST` location (`:229-235`), `STMT_ELSE`/`STMT_IF` chaining (`:94-114`), `field_ref_from_node` head/field idents (`:546-559`) | `SyntaxNode`, `SyntaxKind::{Plus,Minus,…,ARG_LIST,STMT_ELSE,EXPR_FIELD_REF,Ident}` | the **entire action/guard sublanguage structure**: an `ExprBinary`'s operator + lhs/rhs, an `ExprCall`'s callee + args, a `StmtIf`'s cond/then/else | the **expression typed AST is deliberately shallow** (`ast/expr.rs`/`ast/stmt.rs`: `Expr`/`Stmt` are `cast`/`syntax`-only discriminated unions with **zero structural accessors**). Fully typed-AST-ifying this would require a large new accessor surface: `ExprBinary::{op,lhs,rhs}`, `ExprUnary::{op,operand}`, `ExprCall::{callee,args}`, `ExprCast::{operand,type_ref}`, `StmtAssign::{lhs,rhs}`, `StmtIf::{cond,then_block,else_branch}`, `StmtWhile/For` parts, `ArgList::exprs()`, `ExprFieldRef::{head,field}`. This is **the dominant blast-radius driver** (§5). → see §3.4 R-3: **strong leave-and-explain candidate** for the *deep expression-tree internals*; a *bounded* subset (operator accessors) is defensibly addable. |

**Disposition:** the §3.2 sites are **removable only if `fsm-parser`'s
typed-AST API is extended** — this is the definitive resolution of §5's
scoping question (see §5). The split:

- **B-clean (recommended for W0 removal):** `PriorityClause::value()`,
  `*Decl::payload_binding()`, `AfterDecl/EveryDecl/EveryInternalDecl::{action_block,duration}()`,
  and an `MachineDecl::initials()` / history iterator (§3.3). These are
  **small, mechanical, single-construct accessors** with an existing
  in-crate precedent (`EntryDecl::action_block` at `ast/state.rs:235`),
  each removing 1–6 analyzer CST lines for a net clarity gain. ~5 new
  accessor fns + 1 tiny enum-free helper in `fsm-parser/src/ast/{transition,state,top_level}.rs`.
- **B-deep (pre-committed leave-and-explain — §3.4):** the
  `lower/expr.rs` deep expression-tree internals and the OPAQUE-BUG-1
  parent-node type resolution. Forcing a full typed accessor layer over
  the Pratt-parsed expression CST would (a) duplicate the parser's own
  shape knowledge into ~12 new accessor fns whose bodies are *the same
  CST walks moved across the crate boundary* (no clarity gain — the walk
  still exists, just relocated), and (b) be a large new `fsm-parser`
  public surface in a wave whose contract is "non-behavioural, 0 new
  deps, DRIFT-2-grade". That is precisely the "refactor-to-number /
  contort to hit zero coupling" anti-pattern SUBAGENT §10 + Doc 00 §11.44
  forbid. **Leave `lower/expr.rs` as-is with an explanatory module
  comment + a §11 record entry** (the DRIFT-2 `LineIndex` precedent
  applied verbatim).

### 3.3 The "A-with-a-tiny-B-accessor" sites (clean, recommended)

Two Archetype-A sites need a *trivial* typed iterator that does not yet
exist, to avoid the `descendants()+kind()` walk:

- `MachineDecl::initials() -> AstChildren<InitialDecl>` (replaces
  `name_resolution.rs:36-55`'s `children().filter(kind==INITIAL_DECL)`).
  `MachineDecl::initial()` already exists (`ast/top_level.rs:220` — first
  only); adding the plural iterator is a 3-line `children(&self.0)` call
  mirroring the ~30 existing `AstChildren` accessors. Behaviour-identical
  (rowan child order == source order).
- A history iterator for `history.rs:21-24` and `lower/state.rs:782-807`
  `extract_history` — either `StateDecl::shallow_histories()/deep_histories()`
  or a small typed-children pair. Same 3-line pattern.

These are folded into the **B-clean** accessor set (§5): small, additive,
precedented.

### 3.4 Pre-committed leave-and-explain residuals (DRIFT-2 discipline)

Per C-1 and the explicit instruction to **pre-commit conservatively**,
the following are declared **leave-and-explain residuals BEFORE the
implementer runs**. A residual correctly left here is a *success*. Each
gets (i) an explanatory code comment at the site, (ii) a `GATE_VERIFICATION_v1_3.md`
accepted-tracked-debt line, (iii) a Doc 00 §11 record entry.

| Residual | Sites | Why leaving it is the honest move (DRIFT-2 criterion) |
|---|---|---|
| **R-1 — OPAQUE-BUG-1 parent-node type resolution** | `lower/machine.rs:462-495` `lower_type_ref_node`; `symbol_table.rs:443` `primitive_type_text`; `type_check.rs:193` `ty_primitive_name` | This code is the **fix** for a P0-1-class silent-data-loss bug (documented `lower/machine.rs:450-461`): the typed `.ty()` *cannot* see an `OPAQUE_TYPE_REF` sibling, so it returned `None` and the param/field vanished with no diagnostic. Resolving from the parent node is *more correct* than the typed accessor. Adding `TypeOrOpaque` to `fsm_parser::ast` is defensible but is a **new public type on the most behaviourally-critical seam** in a non-behavioural wave — the conservative call is **leave-and-explain**, with the *option* recorded for a later dedicated parser-API wave (it is not W0-shaped: it widens the parser surface for a correctness invariant W0 is not chartered to touch). Removing the coupling here would risk re-introducing the exact P0-1-class regression W0 is forbidden from causing (C-2). |
| **R-2 — `lower/state.rs:43-95` heterogeneous ordered child dispatch** | `lower/state.rs:31-97` `lower_state_children` | The IR id-minter (`IdMinter::{next_pseudo_id,next_transition_id,state_id}`) is **order-sensitive**: lowered IR ids/stable-ids/locs are byte-pinned by `lower_split_byte_identity.rs` and the conformance/example traces. This loop must visit INITIAL/STATE/REGION/FINAL/HISTORY/CHOICE/JUNCTION/FORK/JOIN children **in exact source order across heterogeneous kinds**. The typed AST exposes per-kind iterators (`nested_states()`, `regions()`, …) but **no single ordered heterogeneous-child iterator**, and adding a typed `enum StateChild` ordered iterator to `fsm-parser` is a substantial new API whose only consumer is this one loop — the "refactor-to-number" trap. **Leave-and-explain**: keep the `children()+kind()` dispatch (it is the *clearest* expression of "in source order, dispatch by kind"); a typed wrapper here would *obscure* the order-critical intent for zero behaviour gain — the verbatim DRIFT-2 `LineIndex` rationale (Doc 00 §11.44). |
| **R-3 — `lower/expr.rs` deep expression/statement tree internals** | `crates/fsm-analyzer/src/lower/expr.rs` (whole file); the operand/operator extraction in `checks/determinism.rs:163-275` (`classify_binary`/`lhs_field`/`literal_value`) | The action/guard sublanguage typed AST is **intentionally shallow** (`Expr`/`Stmt` = `cast`/`syntax`-only unions, by design — `ast/mod.rs:1-12` "trivia hidden; optional fields"). A full typed accessor layer (~12 fns: `ExprBinary::{op,lhs,rhs}`, `StmtIf::{cond,then,else}`, `ArgList::exprs()`, …) would relocate — not eliminate — the CST walks (the walk moves into `fsm-parser`; the analyzer still depends on the shape, just via more indirection), adding a large parser public surface in a 0-new-API-intended wave. This is the dominant §5 blast-radius driver and the canonical SUBAGENT §10 "contort to hit a dedup/zero count" case. **Leave-and-explain**: `lower/expr.rs` keeps its `fsm_parser::cst` import with an expanded module-doc explaining the deliberate shallow-AST contract and why folding it is worse-EV. (The *operator-token → IR-op* mapping is internal-to-analyzer translation, not parser-shape coupling — it stays here regardless.) |
| **R-4 — `util::span_of` / `loc_of` / `submachine_ref_is_nested` (Archetype-C)** | `util.rs:127-158`; transitively `lower/loc.rs:29-38` `LocCtx::loc` and **all 79 `span_of(x.syntax())` call sites** crate-wide | `span_of(&SyntaxNode) -> Span` is pure rowan-positional (`node.text_range()`); `submachine_ref_is_nested` is a `.parent()`-walk structural predicate (the W2a P1-2 defence-in-depth, documented `util.rs:104-126`). There is **no typed-AST equivalent** for "the byte range of any node" or "is this ref nested" and inventing one would be a positional-API reimplementation. These are `pub` but **verified to have zero external callers** (`grep` across all crates: no `fsm_analyzer::util::span_of\|loc_of\|submachine_ref_is_nested` outside `fsm-analyzer`; the only cross-crate `fsm_analyzer::util` use is `compute_line_col` in `fsm-lsp/hover.rs`, unrelated to the CST seam) — so they are de-facto crate-internal, not a cross-crate contract. **Leave-and-explain**: `util.rs` legitimately keeps a minimal `fsm_parser::cst::SyntaxNode` import for `span_of`/`loc_of`/`submachine_ref_is_nested` — this is the parser's *public* CST type used for its *intended* positional purpose (Doc 20: the CST is the lossless tree; `span_of` is the byte-range bridge to `fsm-diagnostics`). Folding 79 call sites to thread a typed wrapper would be a massive non-behavioural churn for zero clarity gain — explicitly the DRIFT-2 anti-pattern. |

**Net residual posture:** W0 strips `use fsm_parser::cst::*` from the
**Archetype-A files** (~7 files) and the **B-clean files** (after the §5
accessor additions: `lower/state.rs`'s priority/payload/timer helpers,
the `determinism.rs:133`/`type_check.rs` priority extraction). It
**deliberately retains, with explanation,** the CST coupling in
`lower/expr.rs` (R-3), the `lower/state.rs` heterogeneous-order dispatch
(R-2), the OPAQUE-BUG-1 type resolution (R-1), and `util.rs`'s
positional helpers (R-4). The `cst` module's doc-comment is updated per
the audit's suggested fix to *"primary consumers: the formatter and the
LSP incremental-reparse / positional capabilities; the analyzer consumes
the typed `ast` view and uses `cst` only for (a) byte-range spans via
`util::span_of`, (b) the documented R-2/R-3 leave-and-explain residuals"*
— verified accurate (the `cst` module IS heavily/legitimately consumed by
`fsm-formatter` (10 files) and `fsm-lsp` (16 files); the analyzer is the
outlier the audit correctly named).

This is a **conservative, honest** plan: it removes the coupling where
removal genuinely improves clarity (the A + B-clean sites — the bulk of
the audit's intent and the metric) and **leaves-and-explains the four
residual classes where removal would worsen clarity or risk the P0-1
regression class** — exactly the success criterion C-1 / Doc 00 §11.44
define.

---

## 4. Pre/post-identity acceptance manifest (the non-negotiable gate)

W0 is non-behavioural (C-2). Acceptance is **not "tests pass"** — it is
the SUBAGENT §10 / §5.4 **pre/post byte-identity proof**: the same test
corpus produces **byte-unchanged** output before and after. The
implementer proves identity as follows.

### 4.1 The exact corpus that must pass byte-unchanged

| Corpus | Command | Pinned baseline (at `87ffebd`, pre-W0) |
|---|---|---|
| Full `fsm-analyzer` integration tests | `cargo test -p fsm-analyzer --all-features` | **103 `#[test]` fns** across `crates/fsm-analyzer/tests/*.rs` (15 files incl. the byte-identity guards `lower_split_byte_identity.rs`, `branch_hint_lowering.rs`, `ir_schema_gate.rs`, `submachine_lowering.rs`) |
| `fsm-analyzer` in-crate unit tests | (same invocation) | **16 `#[test]` fns** in `crates/fsm-analyzer/src/**` |
| The 35 LSP `tower-lsp` client acceptance tests | `cargo test -p fsm-lsp --test lsp_client_acceptance` | **35 `#[tokio::test]` fns** in `crates/fsm-lsp/tests/lsp_client_acceptance.rs` (the §5.4-LSP harness; oracle recomputed from `fsm_lsp::analysis::analyze` = the analyzer pipeline — a position/lowering drift makes these fail by construction) |
| `fsm test examples/` | `cargo run -p fsm-cli -- test examples/` | **5/5** (the G6 full-chain + sim-trace-match gate; `examples/` has 8 example dirs / 11 `.fsm`; the `5/5` is the `fsm test`-runner's pinned count per `GATE_VERIFICATION_v1_2.md` §G6 / `0747321` Verified block) |
| Conformance suite | `cargo run -p fsm-cli -- test tests/conformance` | **26/26** (`GATE_VERIFICATION_v1_2.md` l.102) |
| Workspace quad | `cargo build/test/clippy/fmt --workspace --all-features` | `806/0`, clippy-clean `-D warnings`, fmt-clean, `rustc 1.75.0`, `forbid(unsafe_code)` intact (`GATE_VERIFICATION_v1_2.md` l.112) |

### 4.2 How the implementer PROVES identity (not just green)

A green quad is **necessary but not sufficient** (a refactor can change
IR bytes while keeping assertion-style tests green — the P0-1 lesson).
The proof is **three layered, mandatory** checks:

1. **The byte-identity guard tests are the primary proof and must pass
   unmodified.** `crates/fsm-analyzer/tests/lower_split_byte_identity.rs`
   exists specifically to pin the lowered IR byte-for-byte (it was the
   AD-3 split's identity proof — `lower/*.rs` doc-comments cite it). W0
   must **not edit this test**. It passing unchanged is the core
   pre/post-identity evidence for the lowering path.
2. **Explicit pre/post IR-bytes diff (the SUBAGENT §10 refactor proof).**
   Before touching code, on `87ffebd`, the implementer captures the
   lowered IR for the full `examples/` + `tests/conformance` corpus:
   `for f in <corpus>; do fsm generate "$f" --emit-ir -o /tmp/pre/...; done`
   (and the analyzer-diagnostics stream for `tests/conformance` negative
   fixtures). After the refactor, regenerate to `/tmp/post/` and assert
   `diff -r /tmp/pre /tmp/post` is **empty** (IR JSON, diagnostic
   codes+ranges, byte-for-byte). This catches an IR drift that a
   `contains()`-style test would miss. The completion report includes
   this transcript (empty-diff proof).
3. **The 35 LSP client tests + conformance count are the cross-pipeline
   witness.** Because the LSP oracle is recomputed from the analyzer
   pipeline, an analyzer behaviour change surfaces as an LSP Range/code
   mismatch. `35/35` + `26/26` + `5/5` unchanged is the independent
   confirmation that the seam swap was behaviour-neutral end-to-end.

**Acceptance is: (1) byte-identity guard tests pass UNMODIFIED + (2)
empty pre/post IR+diagnostics diff over the full corpus + (3)
103+16/35/5-of-5/26-of-26 + cold workspace quad green.** Anything less
(e.g. "tests pass" without the §4.2(2) explicit diff) is an incomplete
deliverable, rejected back (SUBAGENT §5.4/§11.1).

---

## 5. Blast-radius assessment + recommended wave shape (the §5 resolution)

### 5.1 DEFINITIVE answer: is `fsm-parser` in W0's edit set? — **YES, bounded, additive-only.**

This is the single highest-leverage W0 unknown (C-1 / Doc 28 §1.1
discipline). The evidence is unambiguous and triangulated:

- The audit's own suggested fix (`AUDIT_B` l.115): *"Where the typed AST
  lacks an accessor, **add it to `fsm-parser::ast`** rather than dropping
  to CST."*
- ROADMAP §72 verbatim: *"**Extend AST accessors** then strip cst::\*
  imports."*
- Source-verified accessor gaps (§3.2): `PriorityClause` has **no `impl`
  block at all** (`ast/transition.rs:10`); `AfterDecl`/`EveryDecl` expose
  only `target()`/`branch_hint()` (`ast/state.rs:211-231`) — no
  `duration`/`action_block`; there is no `payload_binding` typed node by
  the parser's own admission (`lower/state.rs:809-812`); `MachineDecl`
  has `initial()` but no `initials()`. The clean removal of the
  Archetype-A/B-clean sites is **impossible without adding these
  accessors to `fsm-parser`**.

**Therefore `fsm-parser` IS in W0's edit set — but strictly
additive-only.** W0 adds ~5–7 small `pub fn` accessors + (at most) one
tiny plural-iterator on **already-declared `ast_node!` types** in
`crates/fsm-parser/src/ast/{transition.rs,state.rs,top_level.rs}` (and a
1-line `pub use` if a helper enum is needed — *avoided* under the
recommended scope by leaving R-1 as residual). It does **NOT**:
- change any existing `fsm-parser` accessor signature or behaviour,
- change the grammar, the CST shape, `SyntaxKind`, or `parse()`,
- add any dependency (the accessors are pure rowan walks, the same the
  analyzer does today, just moved to their correct home),
- touch `fsm-parser/Cargo.toml`.

The added accessors are **behaviour-inert by construction**: each one's
body is the *exact CST walk the analyzer performs today*, relocated. The
pre/post-identity gate (§4) proves this — if an accessor's walk differs
by one token from the analyzer's old inline walk, the byte-identity
guard + the empty-diff check fail.

**Blast-radius implication:** W0's edit set is **two crates**:
`fsm-parser` (additive accessor surface only — `src/ast/*` + a possible
1-line `lib.rs` re-export; **NOT** `Cargo.toml`, **NOT** `grammar/`,
**NOT** `cst/`) and `fsm-analyzer` (the consumer migration + the
leave-and-explain comments). No other crate changes. `fsm-formatter` and
`fsm-lsp` (the legitimate `cst` consumers) are **untouched** —
confirming the audit's "the analyzer is the outlier" framing.

### 5.2 Recommended wave shape: **ONE wave, internally two-phase, NOT split into W0a/W0b**

Options considered:

- **Split (W0a: extend `fsm-parser` typed-AST API → merge → W0b: migrate
  `fsm-analyzer`).** Rationale *for*: isolates the parser-API change so
  its own quad is green before the consumer migrates. Rationale
  *against*: (a) the accessors are **only** consumed by the W0b analyzer
  migration — a W0a that adds `PriorityClause::value()` with no consumer
  is dead public API until W0b, which `unreachable_pub`/clippy would
  flag, forcing W0a to also add throwaway tests; (b) two
  merge+post-merge-quad+phase-audit cycles (C-5) on the *most
  behaviourally-critical crate* doubles the regression-exposure windows
  for **zero** isolation benefit (the accessors can't regress anything
  until consumed); (c) it fragments the pre/post-identity proof (§4.2's
  empty-diff is only meaningful end-to-end, post-migration). Splitting
  here optimises a proxy (smaller diffs) at the cost of the real goal
  (one clean behaviour-identity proof) — mild SUBAGENT §10 smell.

- **One wave, internally sequenced (RECOMMENDED).** A single W0 wave, one
  branch, one merge, one post-merge quad, one phase-boundary audit. The
  implementer works in a deterministic internal order: **(i)** add the
  ~5–7 additive accessors to `fsm-parser/src/ast/*` (compile-green in
  isolation); **(ii)** migrate the Archetype-A + B-clean `fsm-analyzer`
  sites to typed accessors; **(iii)** apply the R-1..R-4
  leave-and-explain comments + update the `cst` module doc; **(iv)** run
  the §4.2 pre/post-identity proof. One coherent diff, one identity
  proof, one audit. This matches how AD-3 (the `lower.rs` split — the
  directly analogous prior refactor of this exact crate) was run: a
  single behaviour-identity wave with `lower_split_byte_identity.rs` as
  the proof, not a split.

**Recommendation: ONE wave**, rationale tied to regression-EV: the
accessors are inert until consumed, so splitting adds a second
critical-crate merge/audit window with no risk-reduction; the binding
acceptance (§4.2 empty-diff) is inherently end-to-end and a split would
fragment it. Wave size is ~15 files but **low-complexity-per-file**
(mechanical accessor adds + import-swap + comment adds — the AD-3
precedent shows this crate absorbs a same-shaped ~15-file refactor in one
identity-proven wave). It fits one implementer's context.

### 5.3 Residual blast-radius after W0

Post-W0, the `cst::*` import remains *by design* in exactly the R-2/R-3
files (`lower/expr.rs`, the `lower/state.rs` ordered-dispatch region) and
the R-1/R-4 helpers — each carrying an explanatory comment and a
gate-doc entry. The audit's metric ("any AST shape adjustment becomes a
24-site change") is **materially reduced** (the A/B-clean sites — the
majority — are removed) without the contortion of forcing the deep
expression-tree / order-critical / positional sites through a synthetic
typed layer. §11.49 closes as **substantially-paid with documented,
reasoned residuals** — the precedent-consistent outcome (v1.1 pub-hygiene
and DRIFT-2 both closed exactly this way: paid where clean, left-and-
explained where forcing it was worse-EV).

---

## 6. W0 implementer brief (precise, self-contained)

> **You are the implementer for W0. You implement it yourself — do NOT
> delegate this further to another agent (the
> [[feedback_subagent_r1_misread]] rule: a wave brief is an instruction
> to *do the work*, not to re-dispatch it; sub-delegation of an
> implementer wave is the misread that rule exists to stop). You own the
> `fsm-parser` accessor additions + the `fsm-analyzer` migration
> end-to-end for W0.**

**Precondition (HARD GATE — do not start unless true):** W0 runs on a
clean `main` at/after `87ffebd`, **before any V1+ feature wave**, **not
in a v1.2.1 patch lane**, **not concurrently with any other writer wave**
(Doc 28 §4 / C-4). If a feature wave is in flight on a shared
`CARGO_TARGET_DIR`, stop and surface it (the §11.1 warm-stale-target
hazard) — do not grind.

**Scope (exactly this — the DRIFT-2-grade non-behavioural seam cleanup):**

- **`crates/fsm-parser/src/ast/{transition.rs,state.rs,top_level.rs}`** —
  ADD ONLY these accessors (each body = the *exact* CST walk the analyzer
  performs today, relocated; behaviour-inert):
  - `impl PriorityClause { pub fn value(&self) -> Option<i64> }` (port
    `lower/state.rs:885-892` `extract_priority` verbatim).
  - `impl TransitionDecl/InternalDecl/LocalDecl { pub fn payload_binding(&self) -> Option<String> }`
    (port `lower/state.rs:813-834` verbatim).
  - `impl AfterDecl/EveryDecl/EveryInternalDecl { pub fn action_block(&self) -> Option<ActionBlock>; pub fn duration(&self) -> Option<ConstExpr> }`
    (port `action_block_under`/`duration_ms` skeleton; mirror the
    existing `EntryDecl::action_block` at `ast/state.rs:235-245`).
  - `impl MachineDecl { pub fn initials(&self) -> AstChildren<InitialDecl> }`
    and the history-decl iterator(s) needed by `history.rs`/`extract_history`
    (3-line `children(&self.0)` mirrors, the ~30 existing `AstChildren`
    accessors are the precedent).
  - A `lib.rs` `pub use` line ONLY if strictly required (avoid — the
    recommended scope leaves R-1's `TypeOrOpaque` as a residual, so no
    new public *type* is needed).
- **`crates/fsm-analyzer/src/**`** — migrate the Archetype-A + B-clean
  sites (§3.1 + the B-clean rows of §3.2/§3.3) from
  `descendants()/children()+kind()`/inline CST walks to the new + existing
  typed accessors + `util::walk_all_states`. Remove
  `use fsm_parser::cst::*` from files that no longer need it (~7
  Archetype-A files + the B-clean helper regions).
- **The R-1..R-4 leave-and-explain residuals (§3.4): DO NOT remove their
  coupling.** Instead, at each residual site add a concise code comment
  in the DRIFT-2 form ("This deliberately retains `fsm_parser::cst`
  because <reason>; folding it would worsen clarity / risk the P0-1
  regression class for zero behaviour gain — Doc 00 §11.44/§11.49, the
  `LineIndex` precedent."). Update the `crates/fsm-parser/src/cst/mod.rs`
  module doc-comment to the audit-suggested wording (§3.4 last para;
  verify the `fsm-formatter`/`fsm-lsp` consumer framing stays accurate).

**Scope boundary — DO NOT (SUBAGENT §6):**
- Do **NOT** touch `crates/fsm-parser/Cargo.toml`, `Cargo.toml` (root),
  `Cargo.lock` (0 new deps — C-3; if `Cargo.lock` changes, you did
  something wrong, stop), `rust-toolchain.toml`.
- Do **NOT** change the grammar (`crates/fsm-parser/src/grammar/**`),
  `crates/fsm-parser/src/cst/kinds.rs`, `SyntaxKind`, `parse()`, or any
  **existing** `fsm-parser` accessor signature/behaviour. The parser
  change is **purely additive accessor surface**.
- Do **NOT** edit `crates/fsm-formatter/**` or `crates/fsm-lsp/**` (the
  legitimate `cst` consumers — they are out of scope and correct as-is).
- Do **NOT** edit any test that is a byte-identity guard
  (`crates/fsm-analyzer/tests/lower_split_byte_identity.rs`,
  `branch_hint_lowering.rs`, `ir_schema_gate.rs`, the
  `submachine_lowering.rs`/`lowering*.rs`/`symbol_table.rs`/`negative.rs`/
  `positive.rs`/`determinism.rs` corpus) **to make it pass**. If one
  fails, your refactor changed behaviour — fix the refactor, never the
  test (SUBAGENT §10 "commenting out / editing tests to go green").
- Do **NOT** touch `docs/*` (Doc 29 is authored in the planning wave;
  the `GATE_VERIFICATION_v1_3.md` closed-§11.49 entry + Doc 00 §11 record
  are folded by the orchestrator at tag time / a reconciliation pass —
  your completion report supplies the residual list they record).
- Do **NOT** "fix" the OPAQUE-BUG-1 type resolution (R-1) or fold
  `lower/expr.rs` (R-3) or the `lower/state.rs` ordered dispatch (R-2) or
  the `util::span_of` family (R-4). These are **pre-decided
  leave-and-explain residuals** (§3.4). Removing them is out of scope and
  is the anti-pattern this wave's discipline forbids.
- If you discover a site that seems to need an out-of-scope change
  (a grammar tweak, an existing-accessor signature change, a new public
  type), **STOP and report it in the completion report — do not silently
  expand scope** (SUBAGENT §6 cardinal bar).

**Read these (named sections only, do not dump):** this Doc 29 §3
(the catalogue + per-site disposition — your work list), §3.4 (the
residuals you must NOT remove), §4 (the acceptance you must prove); Doc
00 §11.44 (the DRIFT-2 leave-and-explain canonical model — the comment
form to mirror); SUBAGENT §10 (the refactor-to-number anti-pattern row)
+ §5.4; `AUDIT_B` P1-A1 (the originating finding); the shipped
`crates/fsm-parser/src/ast/{mod,transition,state,top_level}.rs` (the
`ast_node!` macro + `AstChildren`/`child`/`first_ident` helpers + the
`EntryDecl::action_block` precedent — your new accessors mirror these
exactly); the analyzer sites in §3's `file:line` cells.

**The acceptance you MUST prove (the W0 gate — not "tests pass",
non-negotiable, §4.2):**
1. **Byte-identity guard tests pass UNMODIFIED** — especially
   `crates/fsm-analyzer/tests/lower_split_byte_identity.rs`.
2. **Explicit empty pre/post diff:** capture lowered IR
   (`fsm generate --emit-ir`) + analyzer-diagnostics (codes+ranges) for
   the FULL `examples/` + `tests/conformance/` corpus on `87ffebd`
   *before* coding (`/tmp/pre`), regenerate *after* (`/tmp/post`), and
   show `diff -r /tmp/pre /tmp/post` is **EMPTY**, byte-for-byte. This
   transcript is mandatory in the completion report.
3. **Corpus green at the pinned baselines:** `cargo test -p fsm-analyzer
   --all-features` (103 integ + 16 unit `#[test]` — count unchanged or
   *higher only by* new `fsm-parser` accessor unit tests), `cargo test -p
   fsm-lsp --test lsp_client_acceptance` (**35/35**), `fsm test
   examples/` (**5/5**), `fsm test tests/conformance` (**26/26**), and a
   **COLD** `cargo build/test/clippy/fmt --workspace --all-features`
   (`806/0`+, clippy `-D warnings` clean, fmt clean, `forbid(unsafe_code)`
   intact). Clippy must stay clean — the new `pub fn` accessors must each
   be consumed by the analyzer migration (no `unreachable_pub` warning;
   that is also the proof the accessor replaced a real coupling site).

**0-new-deps proof (C-3):** `git diff 87ffebd -- Cargo.lock` is **empty**.
State this explicitly in the completion report.

**§11.x discipline inherited:** SUBAGENT §5.4 (pre/post behavioural
identity is the acceptance — your §4.2 empty-diff is the analogue of
gcc-compile-and-RUN for a non-behavioural refactor), §6 (the DO-NOT list
above), §10 (refactor-to-number / leave-and-explain — the R-1..R-4
residuals are *required* outputs, not failures; never contort to hit
zero `cst` imports), §11.1 (the orchestrator independently re-runs the
COLD quad post-merge — self-reported green is not proof; the
warm-stale-`-wt-` path caveat applies), §11.3 (a phase-boundary audit
runs after W0 **before** any V1 dispatch — W0's clean audit is the gate
that lets V1's oracle be computed against the post-W0 analyzer).

**Completion report (SUBAGENT §1.12 / §8):** branch; per-crate LOC delta
(`fsm-parser` additive accessor count + `fsm-analyzer` migration delta);
the exact list of analyzer files that lost `use fsm_parser::cst::*`; the
**verbatim R-1..R-4 residual list with the code-comment text applied**
(this is what the orchestrator records as the new accepted-tracked-debt
in `GATE_VERIFICATION_v1_3.md` per Doc 28 §6.2(1)); the §4.2(2) empty
pre/post-diff transcript; the §4.2(3) corpus-green transcript (counts);
the empty `Cargo.lock` diff; every judgment call (esp. any accessor whose
relocated walk needed a non-obvious decision, and confirmation you did
NOT touch R-1..R-4); explicit confirmation `fsm-formatter`/`fsm-lsp`/the
grammar/`Cargo.lock` are byte-untouched.

---

## 7. Summary for the orchestrator (review before merge + W0 dispatch)

- **Coupling-site catalogue:** 17 `cst`-import lines across 14
  `fsm-analyzer` files (Doc 00 §11.49's "~15 files" confirmed). Three
  archetypes: **A** (kind-dispatch — cleanly removable via existing typed
  iterators, ~7 files), **B-clean** (missing small accessors — removable
  via §5 additive parser API), **B-deep/C** (the R-1..R-4 residuals).
- **Seam design + the definitive scoping answer:** the analyzer should
  consume `fsm_parser::ast::*` typed accessors. **`fsm-parser` IS in
  W0's edit set — bounded, additive-only**: ~5–7 new `pub fn` accessors
  on already-declared `ast_node!` types in `fsm-parser/src/ast/*` (NOT
  `Cargo.toml`, NOT grammar, NOT `cst/`, NOT existing signatures). Triple-
  confirmed (audit l.115 "add it to fsm-parser::ast", ROADMAP §72 "extend
  AST accessors", source-verified gaps: `PriorityClause` has no impl
  block; `AfterDecl/EveryDecl` lack duration/action_block; no
  payload-binding typed node). **Blast-radius implication:** two crates
  edited (`fsm-parser` additive + `fsm-analyzer` consumer); zero risk to
  `fsm-formatter`/`fsm-lsp`; accessors behaviour-inert by construction
  (each body = the analyzer's current walk, relocated; the §4.2
  identity gate proves it).
- **Pre-committed leave-and-explain residuals (a success of the
  discipline, not a failure):** **R-1** OPAQUE-BUG-1 parent-node type
  resolution (`lower/machine.rs:462`+) — removing it risks the P0-1
  silent-data-loss class; **R-2** `lower/state.rs:43-95` heterogeneous
  ordered child dispatch — order is IR-id-byte-load-bearing, a typed
  wrapper obscures intent; **R-3** `lower/expr.rs` (whole file) deep
  expression/statement tree internals — the typed AST is deliberately
  shallow; a full accessor layer relocates, not eliminates, the walk and
  is the dominant blast-radius driver (the SUBAGENT §10 contort-to-zero
  trap); **R-4** `util::span_of`/`loc_of`/`submachine_ref_is_nested` +
  79 `span_of` call sites — pure positional, no typed equivalent, zero
  external callers. Each gets a code comment + a `GATE_VERIFICATION_v1_3.md`
  + Doc 00 §11 record entry.
- **Recommended wave shape:** **ONE wave**, internally two-phase
  (add accessors → migrate consumer → apply residual comments → prove
  identity). NOT split W0a/W0b — the accessors are inert until consumed,
  so a split doubles the critical-crate merge/audit window for zero
  risk-reduction and fragments the inherently-end-to-end §4.2 identity
  proof (the AD-3 single-identity-wave precedent for this exact crate).
- **Acceptance (non-negotiable):** byte-identity guard tests pass
  **unmodified** + **empty** pre/post IR+diagnostics `diff -r` over the
  full corpus + 103/16 analyzer / 35 LSP-client / 5-of-5 examples / 26-of-26
  conformance / **cold** workspace quad green + empty `Cargo.lock` diff.
- **Branch:** `phase3.1/v1_3-w0-plan` (this read-only planning doc only;
  the W0 implementer gets a fresh `phase3.1/v1_3-w0-impl`-class branch
  when you dispatch §6).

*End of FSM-PLAN-W0-CST v1.0.0*
