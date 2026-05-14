# Architecture & Coupling Audit — 2026-05-14

**Auditor:** Senior software architect (read-only static review)
**Scope:** All 9 Rust crates under `crates/` (~30k LOC); Doc 23 §4 dep graph
+ Doc 20 §11.5; instability metrics; semantic-coupling vocab; module
locality and blast radius. Boundaries verified by `Cargo.toml` parse +
`grep -rn '^use ' crates/<x>/src` and cross-reference.
**Concurrent work:** A P0-1 fix wave is writing in
`/root/dev/embeded-fsm-sdk-wt-p01/`. This audit ignores that worktree
and only reads `/root/dev/embeded-fsm-sdk/` main.
**Companion audit:** `docs/AUDIT_2026_05_14.md` covers correctness P0s.
This report intentionally avoids restating correctness findings and
focuses on **structure**.

---

## Verdict

**Architecturally sound. The Doc 23 §4 dep graph is upheld strictly —
zero cycles, zero layering violations, foundation crates (`fsm-diagnostics`,
`fsm-lexer`) have zero workspace deps as designed.** What is shaky is
not the *graph* but the *grain* of the modules: there are three
independently-built `state_id → parent_state_id` parent maps (in the
analyzer, the simulator, and codegen-c), `effective_lca`/`lca_inclusive`
are reimplemented three times against three carrier types
(`String`/`String`/`u8`), nine ad-hoc `walk_states` recursors live one
per file in analyzer + codegen, and roughly **88 % of the `pub` surface
across the workspace is consumed only inside its own crate** (out of
~430 `pub` items, ~25 are imported elsewhere). The `fsm-simulator` crate
declares `fsm-analyzer` as a Cargo dep but does not import a single
symbol from it — `cargo build -p fsm-simulator` would succeed if the
dep were removed today. Plus one P0-grade boundary observation: the
analyzer reaches into `fsm_parser::cst` (16 sites) for raw
`SyntaxNode`/`SyntaxKind` rather than going through the typed `ast`
view, which couples the analyzer to the parser's internal CST shape.
Net effect: the dependency *graph* is clean but the dependency *surface*
is wider than the architecture intends. None of this blocks the v1.0 tag
once the correctness P0s in `AUDIT_2026_05_14.md` clear; all are P1/P2
cleanup items that should be tackled before v1.1 grows further surface
on top of these wobbly foundations.

## Severity Counts (architecture-only)

| P0 | P1 | P2 | P3 |
|---|---|---|---|
| 0  | 6  | 8  | 5  |

(P0 reserved for graph cycles, layering breakage, or missing-foundation
defects — none found.)

---

## Boundary Integrity (Doc 23 §4 Dep Graph)

Verified by reading every `crates/*/Cargo.toml` plus
`grep -rn '^use fsm_' crates/<x>/src` and comparing actual import
statements against the declared dep set.

| Crate            | Allowed deps (Doc 23 §4)                  | Cargo.toml lists                                   | `use fsm_*` actually imports                | Verdict |
|------------------|-------------------------------------------|----------------------------------------------------|----------------------------------------------|---------|
| `fsm-diagnostics`| (none)                                    | (no workspace deps)                                | (none)                                       | Conforms |
| `fsm-lexer`      | (foundation only)                         | `fsm-diagnostics`                                  | `fsm_diagnostics::{DiagnosticCode, Span}`    | Conforms |
| `fsm-parser`     | `fsm-lexer` (+rowan)                      | `fsm-diagnostics`, `fsm-lexer`, `rowan`            | `fsm_lexer::{Token, TokenKind}`, `fsm_diagnostics::*` | Conforms |
| `fsm-ir`         | `fsm-diagnostics` + serde                 | `fsm-diagnostics` (serde feat), `serde`, `serde_json`, `thiserror` | `fsm_diagnostics::{Diagnostic, SourceLocation}` only | Conforms |
| `fsm-analyzer`   | `fsm-parser` + `fsm-ir` + `fsm-diagnostics` | `fsm-parser`, `fsm-ir`, `fsm-diagnostics`         | `fsm_parser::{ast,cst,ParseResult}`, `fsm_ir::*`, `fsm_diagnostics::*` | Conforms (but see P1-A1) |
| `fsm-codegen-c`  | **ONLY** `fsm-ir` (+ diagnostics)         | `fsm-ir`, `fsm-diagnostics`                        | `fsm_ir::*` only                             | Conforms (no parser/analyzer leak) |
| `fsm-simulator`  | `fsm-ir` + `fsm-analyzer`                 | `fsm-ir`, `fsm-analyzer`, `fsm-diagnostics`        | `fsm_ir::*` only — **`fsm-analyzer` never imported** | Conforms (graph) / **P1-A2: dep is dead** |
| `fsm-formatter`  | `fsm-lexer` + `fsm-parser` (NOT analyzer) | `fsm-lexer`, `fsm-parser`, `fsm-diagnostics`, `rowan` | `fsm_parser::*`, `fsm_diagnostics::Diagnostic` | Conforms |
| `fsm-cli`        | everything                                | every pipeline crate + clap/miette/anyhow/serde/toml | `fsm_analyzer::analyze_with_source`, `fsm_codegen_c::{emit, compute_budget, CodegenConfig, DispatchStrategy}`, `fsm_simulator::{Interpreter, InitOptions, execute_trace, parse_trace_yaml}`, `fsm_formatter::{format_string, FormatOptions}`, `fsm_parser::{parse, SyntaxNode}`, `fsm_diagnostics::{Diagnostic, DiagnosticCode, Severity}` | Conforms |

**The hard rule "`fsm-codegen-c` MUST NOT import `fsm-parser` or `fsm-analyzer`" (Doc 23 §4 + Doc 20 §11.5)** holds — `grep -rn "fsm_parser\|fsm_analyzer" crates/fsm-codegen-c/src` returns zero matches.

**The hard rule "`fsm-formatter` MUST NOT depend on analyzer / IR"** holds — `grep -rn "fsm_analyzer\|fsm_ir" crates/fsm-formatter/src` returns zero matches.

---

## Fan-in / Fan-out / Instability

Fan-in (`Ca`) = count of OTHER workspace crates that depend on this one
(via Cargo.toml). Fan-out (`Ce`) = count of OTHER workspace crates this
one depends on (via Cargo.toml). `I = Ce / (Ca + Ce)`; `I=0` means
maximally stable (depended on, depends on nothing), `I=1` means maximally
unstable (depends on, depended on by nothing).

| Crate            | Ca | Ce | I = Ce/(Ca+Ce) | Expected | Notes |
|------------------|----|----|----------------|---------|-------|
| `fsm-diagnostics`| 8  | 0  | **0.00**        | low (foundation)   | Conforms perfectly — every crate depends on it, it depends on nothing. |
| `fsm-lexer`      | 3  | 1  | **0.25**        | low (foundation)   | Used by parser, formatter, indirectly via parser everywhere. |
| `fsm-parser`     | 4  | 2  | **0.33**        | low-mid | Used by analyzer, formatter, cli, (sim transitively). |
| `fsm-ir`         | 4  | 1  | **0.20**        | low (data spine)   | Central pure-data crate — exactly where it should be. |
| `fsm-analyzer`   | 2  | 3  | **0.60**        | mid     | Used by cli + (declared-but-unused) sim. |
| `fsm-codegen-c`  | 1  | 2  | **0.67**        | high    | Only `fsm-cli` consumes it. As expected — codegen is a leaf. |
| `fsm-simulator`  | 1  | 3  | **0.75**        | high    | Only `fsm-cli` consumes it. |
| `fsm-formatter`  | 1  | 3  | **0.75**        | high    | Only `fsm-cli` consumes it. |
| `fsm-cli`        | 0  | 7  | **1.00**        | top (leaf orchestrator) | Conforms — no one depends on it; it depends on everyone. |

Result: the instability gradient flows in the correct direction. The
foundation crates are stable, the leaf is maximally unstable, the middle
tier sits between. This is Bob Martin's "stable abstractions principle"
satisfied. No anti-pattern crate (e.g., a foundation crate that is also
unstable, or a leaf with afferent coupling).

---

## Architectural Entropy Findings

### P1-A1 — Analyzer reaches into `fsm_parser::cst::{SyntaxKind, SyntaxNode}` instead of the typed AST

**Files:** `crates/fsm-analyzer/src/{lower.rs, symbol_table.rs, checks/*.rs}` — 16 `use fsm_parser::cst::*` sites; `crates/fsm-analyzer/src/util.rs:4` (the `span_of(SyntaxNode)` helper is built on raw CST).

**Observation:** The parser exposes both a typed AST (`fsm_parser::ast::*`, 8 sites in analyzer) and a raw CST (`fsm_parser::cst::*`, 16 sites in analyzer). The latter is what the formatter walks; the analyzer pipeline was supposed to live above the typed AST. In practice, analyzer code mixes both — e.g. `name_resolution.rs:74` walks `&SyntaxNode` and matches on `SyntaxKind` directly inside check functions. Result: any AST shape adjustment in `fsm-parser` becomes a 24-site change in `fsm-analyzer`.

**Severity:** P1. Not a layering violation (parser exposes both as public API), but a leakage of the parser's CST shape into the analyzer.

**Suggested fix:** Promote any check that currently matches `SyntaxKind` to take the typed AST node and use its accessors. Where the typed AST lacks an accessor, add it to `fsm-parser::ast` rather than dropping to CST. Document `cst` module as "for the formatter and LSP-incremental-reparse only" — analyzer should not need it.

---

### P1-A2 — `fsm-simulator` declares `fsm-analyzer` as a Cargo dep that it never imports

**File:** `crates/fsm-simulator/Cargo.toml:17` declares `fsm-analyzer = { path = "../fsm-analyzer" }`. Source verification: `grep -rn "fsm_analyzer" crates/fsm-simulator/src` returns exactly one match — a doc-comment on `runtime/machine_index.rs:1` ("extends `fsm_analyzer`'s parent-only `MachineIndex`"). No `use fsm_analyzer::*` anywhere in `src/` or `tests/`.

**Observation:** The dep was likely added with the intent that the simulator would reuse `fsm_analyzer::lca::{MachineIndex, effective_lca, lca_inclusive}`. In practice the simulator built its own `runtime::lca::{effective_lca, lca_inclusive}` against its richer `runtime::MachineIndex` (which is a HashMap<StateId, Arc<NodeRef>> with timers, defers, transitions all flattened). The analyzer's parent-only `MachineIndex` is now strictly internal to the analyzer and `fsm-cli`'s tests, but it is `pub`-exported anyway. Build cost: the simulator compiles `fsm-analyzer` (and its serde + thiserror tree) on every clean build of `cargo test -p fsm-simulator`. Per Doc 23 §4 the simulator is allowed to depend on the analyzer, so this is not a violation — it is just **unused weight**.

**Severity:** P1.

**Suggested fix:** Either (a) delete the `fsm-analyzer` dep from `fsm-simulator/Cargo.toml`, which will fail cleanly with no source changes needed, or (b) actually share `lca::*` between sim and analyzer — see P1-A3 / P1-A5 for the shared-foundation rework.

---

### P1-A3 — Three parallel `state_id → parent_state_id` parent maps; same algorithm, three carriers

**Files:**
- `crates/fsm-analyzer/src/lca.rs:22` — `pub struct MachineIndex { parents: HashMap<String, String> }` with `MachineIndex::build(m: &MachineObject)` and `lca_inclusive(a: &str, b: &str, idx: &MachineIndex) -> String`.
- `crates/fsm-simulator/src/runtime/machine_index.rs:101` — `pub struct MachineIndex { nodes: HashMap<String, Arc<NodeRef>>, regions: HashMap<String, RegionRef>, root: String }` — richer (carries every state's kind, transitions, timers, defers, entry/exit). Its `runtime/lca.rs:11` re-implements `lca_inclusive(a: &str, b: &str, idx: &MachineIndex) -> String` against the same `String` IDs but a different index type.
- `crates/fsm-codegen-c/src/{state_index.rs, parent_table.rs}` — `StateIndex` + `ParentTable { parents: Vec<u8> }` with `lca_inclusive(a: u8, b: u8, parents: &ParentTable) -> u8` in `emit/entry_exit.rs:38`. The carrier is `u8` (state index) instead of `String` (state ID) because codegen emits a static C array — same algorithm, different element type.

**Observation:** Three impls, three carrier types, three test suites that all assert the same UML 2.5.1 walk-up-the-parent-pointers logic. Plus the unused first one (P1-A2 above). The codegen impl is forced to use `u8` (C-array index domain) so it's hard to share with the other two. The analyzer and simulator impls both use `String` IDs and could/should share.

**Severity:** P1.

**Suggested fix:** Promote the algorithm to a generic in `fsm-ir` (e.g. `fsm_ir::lca::{Walker, lca_inclusive<I: ParentResolver>}` where `ParentResolver` is a small trait with a single `fn parent(&self, id: &Self::Id) -> Option<&Self::Id>`). Both the analyzer's and simulator's `MachineIndex` types implement the trait; the codegen's `ParentTable` implements it over `u8`. One algorithm, three carriers, zero duplication.

---

### P1-A4 — Two distinct types both named `MachineIndex` in the same workspace

**Files:** `crates/fsm-analyzer/src/lca.rs:22` and `crates/fsm-simulator/src/runtime/machine_index.rs:101`.

**Observation:** Both are `pub` and both are re-exported at crate-root (`fsm_analyzer::MachineIndex`, `fsm_simulator::MachineIndex` — note the latter is the rich one). The simulator's `lib.rs:38` re-exports it without qualification, which would shadow the analyzer's name if both were imported into the same translation unit. This compounds P1-A3 — fixing the lca duplication should also resolve the name collision. The companion correctness audit logged this as P2-7.

**Severity:** P1 (architectural concept ambiguity), P2 by correctness lens.

**Suggested fix:** Rename the simulator's type to something like `RuntimeIndex` or `MachineTopology` to reflect that it carries runtime/per-step lookups, not just LCA. Reserve `MachineIndex` for the LCA-only structure.

---

### P1-A5 — Nine ad-hoc `walk_states` recursors duplicate IR / AST tree traversal

**Sites (per `grep -rn 'fn walk_state' crates/`):**
- `crates/fsm-analyzer/src/checks/completion.rs:70` — walks AST.
- `crates/fsm-analyzer/src/checks/determinism.rs:456` — walks AST.
- `crates/fsm-analyzer/src/checks/defer.rs:101` — walks AST.
- `crates/fsm-analyzer/src/checks/parallel.rs:35` — walks AST.
- `crates/fsm-codegen-c/src/state_index.rs:186` (+`192`) — walks IR.
- `crates/fsm-codegen-c/src/emit/dispatch_table.rs:119` — walks IR.
- `crates/fsm-codegen-c/src/emit/dispatch_switch.rs:70` — walks IR.
- `crates/fsm-codegen-c/src/emit/history.rs:79` — walks IR.

Plus `crates/fsm-ir/src/visitor.rs` defines `pub trait IrVisitor` with default-method-based walking — which is **never used outside its own tests** (verified by `grep -rn 'IrVisitor\|walk_ir\|walk_machine\|walk_region\|walk_state' crates/ | grep -v fsm-ir`).

**Observation:** The shared visitor is defined and exported but every consumer rolls its own recursive walk. The visitor was apparently abandoned mid-refactor (the doc-comment notes "Critical fix from VALIDATION_REPORT 1.15-1.17" — so it was meant to be canonical).

**Severity:** P1. Doesn't break anything; multiplies the surface that has to change when the IR tree shape evolves (which, per the v1.1 roadmap, includes adding doc-comment slots and assembly target fields).

**Suggested fix:** Either commit to `IrVisitor` and refactor codegen + analyzer to use it (preferred — it's already pub-exported and tested), or delete it and accept the duplication. Mixed state is the worst of both worlds.

---

### P1-A6 — `LoweringCtx` is a god object (≈30 methods, 850+ lines)

**File:** `crates/fsm-analyzer/src/lower.rs:193` defines `struct LoweringCtx<'a>` with fields `{file, src, machine_name, m_idx, transition_counter, pseudo_counter, state_counter}` and an `impl` block at L209 containing ~30 methods: `loc`, `loc_span`, `next_transition_id`, `next_pseudo_id`, `state_id`, `lower_field`, `lower_event`, `lower_extern`, `lower_consts_for_machine`, `lower_queue`, `lower_targets`, `lower_imports`, `lower_features`, `lower_type_ref`, `lower_literal`, `lower_state_children`, `lower_state`, `lower_region`, `lower_history`, `lower_choice`, `lower_junction`, `lower_fork`, `lower_join`, `lower_transitions`, `lower_external`, `lower_internal`, `lower_local`, `lower_completion`, `build_transition`, `lower_timers`, `lower_defers`. Plus 6 free functions tagged at the bottom (`duration_ms`, `eval_i64`, `extract_priority`, `state_target_id`, `extract_history`, `source_hash`, `sha256`).

**Observation:** This is the actual centre of gravity of the analyzer crate. Every `lower_X` is doing real work (the lowering is intrinsically wide because the DSL is wide), so naïve splitting would not help. But the file is 1356 LOC of one type's `impl` — the largest single-impl in the workspace.

**Severity:** P1.

**Suggested fix:** Split `lower.rs` into `lower/{mod, machine, state, transition, expr, helpers}.rs` keeping the same `LoweringCtx` but distributing the `impl` blocks across files (Rust allows multi-file `impl`). The reader can then locate "how is a state lowered" or "how is a transition lowered" without scrolling through 1300 lines. This is purely a layout refactor; no behaviour changes.

---

### P2-A1 — Massive `pub` over-exposure: ~88 % of public items unused outside their crate

**Counts:**

| Crate          | total `pub` items | `pub(crate)` | externally consumed `pub` |
|----------------|------------------:|-------------:|---------------------------:|
| `fsm-lexer`    |  7 | 1 | 5 (Token, TokenKind, Lexer, tokenize + re-exports of Span/DiagnosticCode) |
| `fsm-parser`   | 88 | 2 | ~10 (parse, ParseResult, SyntaxNode, SyntaxKind, AstNode, ast::*, cst::SyntaxToken etc.) |
| `fsm-ir`       | 72 | 0 | ~25 (most data types consumed by codegen + simulator) |
| `fsm-analyzer` | 50 | 0 | **1** (`analyze_with_source`) |
| `fsm-codegen-c`| 80 | 0 | **4** (`emit`, `compute_budget`, `CodegenConfig`, `DispatchStrategy`) |
| `fsm-simulator`| 83 | 0 | **4** (`Interpreter`, `InitOptions`, `execute_trace`, `parse_trace_yaml`) |
| `fsm-formatter`| 34 | 0 | **2** (`format_string`, `FormatOptions`) |
| `fsm-cli`      | 35 | 0 | 0 (binary crate; no lib.rs) |
| `fsm-diagnostics` | 6 | 0 | 6 (every member is re-exported by every consumer) |
| **TOTAL**      | ~455 | 3 | ~57 |

So roughly **3 × `pub(crate)` and 455 × `pub`**, but only ~57 are actually consumed outside their declaring crate. The remaining ~400 `pub` items are effectively `pub(crate)` masquerading as a public API.

**Why this matters:** Every `pub` item is a semver hazard for downstream tagging. A v1.0 release with `pub fn lower::analyze_with_source` makes sense; `pub fn lower::next_transition_id` does not.

**Observation:** This pattern is consistent across all four "thick" crates (analyzer, codegen-c, simulator, formatter). It's not a one-off; it's a workspace-wide convention of "make things `pub` so I can write a test from outside the module."

**Severity:** P2. Doesn't change behaviour; expands the v1.0 stability surface.

**Suggested fix:**
1. Add `clippy::missing_docs_in_private_items` workspace lint OFF and `missing_docs` workspace lint ON — that forces the question "should this really be public?" for every `pub`.
2. Pass through each non-`lib.rs` `pub` item and downgrade to `pub(crate)` unless it's in the externally-consumed list above. Estimate: ~390 items → `pub(crate)`.
3. Keep tests adjacent to the modules they cover (`#[cfg(test)] mod tests` inside the module) so the `pub` downgrade does not force test rewrites.

---

### P2-A2 — `fsm-analyzer` re-exports four modules (`checks`, `lca`, `lower`, `scope`, `util`, `symbol_table`) as `pub mod` but only `lower::analyze_with_source` is consumed externally

**File:** `crates/fsm-analyzer/src/lib.rs`.

**Observation:** All six submodules are `pub mod`. `lib.rs` then `pub use lca::{effective_lca, lca_inclusive, MachineIndex}` (none of which are consumed externally — see P1-A2), `pub use lower::{analyze, analyze_with_source, AnalysisResult}` (only `analyze_with_source` is consumed externally), and `pub use symbol_table::SymbolTable` (not consumed externally). Every `pub mod` is reachable from outside, but the only public entry point is `analyze_with_source`.

**Severity:** P2.

**Suggested fix:** Make every `pub mod` in `fsm-analyzer/src/lib.rs` plain `mod` (or `pub(crate) mod`). Keep only `pub use lower::{analyze, analyze_with_source, AnalysisResult};` and the re-exported foundation types from `fsm_diagnostics`/`fsm_ir`. The 50 internal `pub fn`s in checks/* and symbol_table become unreachable from outside, which is correct.

---

### P2-A3 — `fsm-codegen-c` re-exports 30+ raw IR types via `pub use fsm_ir::{ … 30 items … }`

**File:** `crates/fsm-codegen-c/src/lib.rs:42–50` does:

```rust
pub use fsm_ir::{
    BinaryOp, CastExpr, ChoiceBranch, ChoiceState, CmpOp, CompositeState, ConstDecl, ContextField,
    ContextSchema, DeferDecl, EnumVariantLit, EventObject, Expr, ExternObject, FieldRef,
    FinalState, ForkPseudo, GuardExpr, GuardOperand, HistoryKind, HistoryObject, InitialPseudo, Ir,
    JoinPseudo, JunctionState, Literal, MachineObject, OverflowPolicy as IrOverflowPolicy,
    ParallelState, Param, QueueConfig, RegionObject, SimpleState, SimpleState as SimpleStateObj,
    StateNode, Statement, SubmachineRef, TargetConfig, TimerKind, TimerObject, TransitionKind,
    TransitionObject, Trigger, Type, UnaryOp,
};
```

**Observation:** The CLI imports from `fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy, compute_budget}` only — it does not consume any of these re-exported IR types via codegen-c. The re-export gives consumers two equivalent paths to the same types (`fsm_ir::Ir` vs `fsm_codegen_c::Ir`), creating import-path ambiguity. Note `SimpleState as SimpleStateObj` aliases the same type to two names — that's a code smell on its own.

**Severity:** P2.

**Suggested fix:** Delete the bulk `pub use fsm_ir::{...}` block. If codegen-c's own modules need IR types, they import them directly from `fsm_ir` (which is already a transitive dep visible to consumers). One canonical import path.

---

### P2-A4 — `IrVisitor` trait is `pub`-exported but unused outside the defining crate

**File:** `crates/fsm-ir/src/visitor.rs:20` defines `pub trait IrVisitor` with 8 default-method `visit_*` hooks; `pub fn walk_ir`, `pub fn walk_machine`, `pub fn walk_region`, `pub fn walk_state` underneath.

**Observation:** Search across the workspace shows the only consumer is `fsm_ir::visitor::tests::CountingVisitor` (the in-file test). Every external traversal (P1-A5) rolls its own. The visitor abstraction is paid for in compile time + LOC and earns nothing.

**Severity:** P2.

**Suggested fix:** Either (a) refactor codegen + analyzer to use it (recommended — closes P1-A5 at the same time), or (b) delete `visitor.rs` entirely and rely on hand-rolled walks. Document the choice in `docs/00`.

---

### P2-A5 — `fsm-cli` has no `lib.rs` so `pub fn run` is unreachable from outside

**Files:** `crates/fsm-cli/src/{main.rs, cli.rs, config.rs, diagnostics.rs, cmd/*.rs}`. Cargo manifest declares `[[bin]] name = "fsm"` only; there is no `[lib]` target.

**Observation:** Each `cmd/*.rs` declares `pub fn run(args: …Args) -> ExitCode`. Because the crate has no library target, those `pub` are pure noise — `pub` outside a `[lib]` target is enforced only by clippy, not the compiler, and means the same thing as `pub(crate)`. This is harmless but reinforces the "everything pub by reflex" pattern (P2-A1).

**Severity:** P3 by cost, P2 by signal.

**Suggested fix:** Downgrade every `pub` in `crates/fsm-cli/src/cmd/*.rs` and the helper modules to `pub(crate)`. If a future v1.1 wants to expose `fsm` as a library (for embedding in larger toolchains), add a `[lib]` target and a deliberate `lib.rs` listing exactly what's re-exported.

---

### P2-A6 — `fsm-codegen-c` carries `pub mod budget, config, emit, expr, parent_table, state_index, stmt` (7 modules, only `emit` + `config` types consumed externally)

**File:** `crates/fsm-codegen-c/src/lib.rs`.

**Observation:** Of 7 `pub mod`, only `emit` (`emit`, `EmittedFile`, `EmittedFiles`, `FileRole`, `EmitError`) and `config` (`CodegenConfig`, `DispatchStrategy`) are consumed externally. The other five (`budget`, `expr`, `parent_table`, `state_index`, `stmt`) are internal codegen utilities — `parent_table::build_parent_table`, `state_index::build_state_index`, etc. — that don't need to be public.

**Severity:** P2.

**Suggested fix:** `pub mod emit; pub mod config;` for the public surface; the rest become plain `mod`. Internal cross-module `use` keeps working (it goes through `crate::expr`, `crate::state_index`, etc., not `crate_root::*`).

---

### P2-A7 — Files >800 LOC

| File                                                          | LOC  | Verdict |
|---------------------------------------------------------------|------|---------|
| `crates/fsm-analyzer/src/lower.rs`                            | 1356 | P1-A6 (god object split) |
| `crates/fsm-lexer/src/lexer.rs`                               | 1319 | Acceptable — every `lex_X` is one token rule and they cluster well. |
| `crates/fsm-simulator/src/interpreter.rs`                     | 1272 | Borderline — split candidate. The `impl Interpreter` block is 15 `pub` methods + 14 private helpers + 9 free functions. Splitting `interpreter/{init.rs, step.rs, dispatch.rs, transition.rs}` would help. Severity P2. |
| `crates/fsm-ir/src/model.rs`                                  | 1070 | Acceptable — model is intrinsically wide (50 type defs). |
| `crates/fsm-diagnostics/src/lib.rs`                           |  788 | Borderline. Single-file foundation is fine for now; split if it grows past 1k. |

**Severity:** P1-A6 owns lower.rs already; interpreter.rs is P2 in this row; the rest are observational.

---

### P2-A8 — Lexer keyword keyword-prefix tests are inside `crates/fsm-lexer/src/lexer.rs`, but the file already does five concerns

**File:** `crates/fsm-lexer/src/lexer.rs` mixes (a) the `Lexer` struct, (b) the byte-level `lex_*` methods (≈25 methods, one per token form), (c) helper free fns (`is_ident_start`, `is_ident_continue_byte`), (d) the public `tokenize(src) -> Vec<Token>` driver, and (e) ~700 lines of `#[cfg(test)] mod tests`.

**Observation:** Tests-in-source for a 1.3k-LOC file means roughly 50 % of the file is test code. Snapshot tests live in `tests/snapshot.rs`; the inline tests are unit-level. This is consistent with the workspace's "tests near source" convention but reads as one big file.

**Severity:** P3.

**Suggested fix:** Move the inline `tests` module into a sibling `lexer/tests.rs` (or `crates/fsm-lexer/tests/inline.rs`). Optional. Lexer is genuinely complex and a single file aids browsing.

---

## Dependency Cycles

**None.** Verified by `cargo tree -p fsm-<x>` for every crate plus
`grep -rn '^use crate::' crates/<x>/src` and visual inspection of the
intra-crate module DAGs.

Detailed per-crate intra-module DAGs:

- **fsm-lexer:** `lexer.rs → token.rs`. Trivial.
- **fsm-parser:** `parse.rs → parser.rs → grammar/* → cst::SyntaxKind`; `grammar/* → expr.rs → parser.rs`; AST module is leaf. No cycles.
- **fsm-ir:** `model.rs → json.rs → visitor.rs`. Strict topological order.
- **fsm-analyzer:** `lib.rs → lower.rs + checks/* + symbol_table.rs + lca.rs`. `checks/*` import `symbol_table`, `scope`, `util` (never each other). `lca.rs` is leaf. No cycles.
- **fsm-codegen-c:** `lib.rs → emit/* + config + budget + parent_table + state_index + stmt + expr`. Within `emit/*`, header/source/impl_header all depend on `config`, `state_index`, `parent_table` — no `emit/X → emit/Y` cycles.
- **fsm-simulator:** `lib.rs → interpreter.rs → eval/* + runtime/*`. `interpreter.rs` imports `runtime::*` and `eval::*`. No cycles.
- **fsm-formatter:** `lib.rs → format/* + options`. Each `format/*` imports `options`, `writer`, and sometimes one or two siblings. Acyclic.
- **fsm-cli:** `main.rs → cli.rs → cmd/*`. Trivial.
- **fsm-diagnostics:** single-file. Trivial.

---

## Semantic Coupling — Vocab Consistency

| Concept | Defined in | Used as | Locations | Consistent? |
|---|---|---|---|---|
| `Span` (byte range) | `fsm_diagnostics::Span` | `Span` | all 8 consumers via re-export | **Y** — single source. |
| `SourceLocation` (Span + file + line + col) | `fsm_diagnostics::SourceLocation` | `SourceLocation` | fsm-ir, fsm-lexer, fsm-analyzer, fsm-codegen-c, fsm-simulator | **Y** — single source. |
| `Severity` | `fsm_diagnostics::Severity` | `Severity` | every consumer | **Y** — single source. |
| `DiagnosticCode` | `fsm_diagnostics::DiagnosticCode` | `DiagnosticCode` | 6 crates × 36 files | **Y** — single source. |
| `Diagnostic` | `fsm_diagnostics::Diagnostic` | `Diagnostic` | every consumer | **Y** — single source. |
| `RelatedInfo` | `fsm_diagnostics::RelatedInfo` | `RelatedInfo` | analyzer, codegen | **Y** — single source. |
| StateId (the actual identity of a state node) | (no type) | bare `String` | `MachineObject.root.id`, every `StateNode::*.id`, transition `source`/`target`, `parent_state_id`, etc. — **75+ untyped `String` fields tagged `id`** in `fsm-ir/src/model.rs` alone | **N — P2-A9** |
| Stable user ID (`@stable.foo` annotation) | (no type) | `String` on most kinds, `Option<String>` on RegionObject only | `MachineObject.stable_id: String`, … `RegionObject.stable_id: Option<String>` | **Partial — P3-A1** |
| Machine vs FSM (in naming) | both used in docs | `MachineObject` in code, "FSM" in docs/comments | doc-comments use "FSM-Lang", "FSM Studio", type uses "Machine" | **Y** — convention: "FSM" in user-facing strings, "Machine" in code. Consistent. |
| LCA algorithm | three impls (analyzer, simulator, codegen-c) | `effective_lca`, `lca_inclusive` | three implementations, three carrier types | **Y in name, N in implementation — P1-A3** |
| Parent map | three impls (analyzer, simulator, codegen-c) | `HashMap<String,String>`, `HashMap<StateId, Arc<NodeRef>>`, `Vec<u8>` | three carriers | **Same — P1-A3** |
| Transition kind | `fsm_ir::TransitionKind` enum (External, Internal, Local, Completion) | `TransitionKind::*` | every consumer | **Y** — single source. |
| `Trigger` (transition-firing condition) | `fsm_ir::Trigger` enum (`Event`, `Completion`, `After`, `Every`) | `Trigger::*` | analyzer, codegen, simulator | **Y in code, but `Trigger::After` and `Trigger::Every` are dead variants** per `AUDIT_2026_05_14.md` P2-12 — see correctness audit. |

### P2-A9 — Bare `String` for state IDs everywhere; no `StateId` newtype

**File:** `crates/fsm-ir/src/model.rs` — 75+ fields named `id: String` plus `source: String`, `target: String`, `parent_state_id: String`. Every consumer (analyzer, codegen-c, simulator) holds `String` keys in its own HashMaps.

**Observation:** The lack of a newtype `StateId(String)` (and `RegionId(String)`, `EventId(String)`, etc.) means (a) the compiler can't catch `transition.source = transition.target` swaps at the IR level, (b) every API takes `&str` rather than `&StateId`, losing the chance to encode invariants like "every state ID starts with `s-{machine}-{name}`", and (c) the LCA implementations in P1-A3 all take `&str` and can't enforce "this string is actually a state ID, not a region ID or stable ID". The format invariant lives only in `fsm_analyzer::lower::LoweringCtx::state_id` (`format!("s-{}-{}")`).

**Severity:** P2.

**Suggested fix:** Introduce `pub struct StateId(String)` (and equivalents) in `fsm-ir`. Deref to `&str` for backward compat. Use as the key type in P1-A3's generalized parent map. Don't try to do this in v1.0 — it touches every IR consumer. Tracking item for v1.1.

---

## Layering Compliance

| Layer | Crates | Allowed inputs | Verdict |
|-------|--------|----------------|---------|
| Foundation | `fsm-diagnostics` | (none) | Pure. |
| Lexical | `fsm-lexer` | foundation | Pure. |
| Syntactic | `fsm-parser` | lexical | Pure (only `fsm-lexer` + `fsm-diagnostics` for Diagnostic emission). |
| Data spine | `fsm-ir` | foundation | Pure (only `fsm-diagnostics`; serde). |
| Semantic / lowering | `fsm-analyzer` | syntactic + data spine | Pure (parser + ir + diagnostics). Inner CST leakage (P1-A1) is a sub-layering concern within the parser-analyzer interface but does not cross the layer boundary. |
| Generation | `fsm-codegen-c` | data spine only | Pure. Hard rule "no parser, no analyzer" upheld. |
| Execution | `fsm-simulator` | data spine + (declared) semantic | Pure (only uses data spine in source; the analyzer dep is dead — P1-A2). |
| Rendering | `fsm-formatter` | lexical + syntactic | Pure. Does not see analyzer or IR (Doc 23 hard rule). |
| Orchestration | `fsm-cli` | everything | Pure leaf. |

**No layering violations.**

---

## Change Blast Radius (illustrative)

| Type / signature                                         | Files referencing | Crates referencing | Expected? | Risk if changed |
|----------------------------------------------------------|-------------------:|-------------------:|-----------|------------------|
| `fsm_diagnostics::DiagnosticCode`                        | 36 | 6 | YES — foundational | Adding a new variant: 36-site touch for `match` exhaustiveness. **Expected** for a foundation enum; not a smell. |
| `fsm_diagnostics::Span`                                  | every crate | 8 | YES | Changing layout: workspace-wide rebuild but trivial — only fields are two `usize`. Stable. |
| `fsm_ir::TransitionObject`                               | 15 files | 4 crates | YES | A new field with default: 15-file touch but mostly serde-derive driven. Adding a field that participates in `match` patterns (`kind: TransitionKind`) would require analyzer + codegen + simulator changes — that's the right blast radius for an IR-spine type. |
| `fsm_ir::StateNode` (13-variant enum)                    | every consumer | 4 crates | YES | Adding a new variant: every `match` on `StateNode` needs a new arm. Major IR change. Right radius. |
| `fsm_codegen_c::CodegenConfig`                           | 14 files in codegen-c + 2 in cli | 2 | YES | Adding a field is config-only; 2-crate touch. Healthy locality. |
| `fsm_analyzer::analyze_with_source` signature           | 5 files (4 in cli, 1 elsewhere) | 2 | YES | Adding an arg is 4-call-site touch. Acceptable. |
| `fsm_simulator::Interpreter::dispatch` signature        | 5+ tests, 1 trace runner | 2 | YES | Test-heavy but bounded to simulator + cli. |
| `fsm_formatter::FormatOptions`                           | 12 files in formatter, 1 in cli | 2 | YES | Bounded to formatter. |
| Internal `fsm_parser::cst::SyntaxKind` (variant added)   | parser internals + 16 sites in analyzer (P1-A1) | 2 (parser + analyzer) | NO | **Smell.** Should be parser-internal; the analyzer should not need to know SyntaxKind discriminants. See P1-A1. |
| `StateId` (= `String`)                                   | 100+ files (every `id`, `source`, `target`) | every crate | NO | **Smell.** A change like "state IDs are now `Arc<str>` not `String`" is workspace-wide and the compiler can't help. See P2-A9. |

**Reading:** Stable types (DiagnosticCode, Span, TransitionObject) earn their wide reach. Volatile types should be narrow. The two outliers — `SyntaxKind` leak and untyped `StateId` — match P1-A1 / P2-A9.

---

## Modular locality / change locality

| Feature change | Expected scope | Actual scope | Locality |
|---|---|---|---|
| Add a new diagnostic code (e.g. `FSM-W0608`) | `fsm-diagnostics` (+ emitter site in analyzer/parser/codegen) | (1) `fsm-diagnostics/src/lib.rs` add to enum + `Display`, (2) one emit site. | **Good.** |
| Add a new state kind (e.g. `Terminate` pseudo-state) | `fsm-ir/model.rs` (variant), `fsm-analyzer` (lower + checks), `fsm-codegen-c` (emit), `fsm-simulator` (interpret). | Same five. Plus `fsm-ir/visitor.rs` (unused trait — skipped). | **Good** (intrinsic — every layer needs to know). |
| Add a new transition variant | `fsm-ir/model.rs`, `fsm-analyzer/lower.rs`, `fsm-codegen-c/emit/dispatch_*.rs`, `fsm-simulator/interpreter.rs`. | Same. | **Good.** |
| Add a new syntactic form (new grammar production) | `fsm-lexer` (if new token), `fsm-parser/grammar/*`, `fsm-parser/cst/kinds.rs`, `fsm-parser/ast/*`, `fsm-analyzer` (consume new AST). | Same. Plus formatter for new rendering. **No IR/codegen/simulator touch** if the new form lowers to an existing IR shape. | **Good.** |
| Change codegen output (e.g. new dispatch strategy) | `fsm-codegen-c` only. | `fsm-codegen-c/{config.rs, emit/dispatch_*.rs}` and `fsm-cli` adds CLI flag. | **Good.** |
| Change formatter rules | `fsm-formatter` only. | `fsm-formatter/format/*`. | **Good.** |
| Add a new LCA algorithm (e.g. UML 2.5.2 self-transition tweak) | `fsm-ir` (if shared) or one consumer. | Three places: analyzer, simulator, codegen-c — see P1-A3. | **Bad.** |

Apart from the P1-A3 cross-crate algorithm duplication, the workspace
maintains good change locality. The Doc 23 §4 graph is doing its job.

---

## Abstraction inversion

Search confirms no instance of a low-level crate depending on a
high-level abstraction. `fsm-lexer` does not know about parsing,
`fsm-parser` does not know about analysis, etc. The single trait
abstraction in the workspace (`fsm_ir::IrVisitor`) goes the right way
(IR layer offers a tool that upper layers may use — see P2-A4 for why
they don't).

---

## Interface volatility

The pattern is consistent and worth naming: **every analyzer-like crate
(analyzer, codegen-c, simulator) exposes every internal module as
`pub mod` plus 50–80 `pub` items, and the CLI consumes 1–4 of them.**
The unused 95 % is interface debt — items that look like API but aren't.

P2-A1, P2-A2, P2-A3, P2-A5, P2-A6 all target this. Recommended grouped
cleanup wave for v1.1.0-beta tag.

---

## Tracking items (P3)

### P3-A1 — `RegionObject.stable_id` is `Option<String>` while every other kind has `stable_id: String` (defaulting to `""`)

**File:** `crates/fsm-ir/src/model.rs:90, 180, 194, 233, 286, 303, …`

Reason: most kinds initialize the field to `String::new()` and treat
empty-string as absent; RegionObject historically had `None` semantics
for "no stable ID". The mixed shape costs every consumer one extra
case-match. P3 because tractable in isolation; do it when StateId
newtype (P2-A9) lands.

### P3-A2 — `fsm-codegen-c/src/lib.rs:54` aliases `SimpleState as SimpleStateObj`

```
SimpleState, SimpleState as SimpleStateObj,
```

Two names for the same type; one will outlive the other. Likely a
leftover from a rename. Pick one.

### P3-A3 — `crates/fsm-cli/src/cmd/decompile.rs` is a 17-line stub that prints "not implemented in v1.0" and exits 2

Conformant with the explicit Doc 23 §9 deferral of round-trip
decompile, but the file is in the `[[bin]]` source set with a `pub fn
run` that any future test could call. P3: either gate behind
`#[cfg(feature = "decompile-v1_1")]` or keep as-is (acceptable).

### P3-A4 — `fsm-parser/src/lib.rs` declares 9 modules, of which `import_resolver`, `opaque_type_validator`, `expr`, `ast`, `cst` are `pub mod` and `grammar`, `parse`, `parser`, `token_set` are `mod`

The `pub` set is intentional (consumers need `cst::SyntaxNode` and
`ast::*`), but `import_resolver` and `opaque_type_validator` are not
referenced from outside `fsm-parser` itself per
`grep -rn 'fsm_parser::import_resolver\|fsm_parser::opaque_type_validator'`. Lower to plain `mod`.

### P3-A5 — `fsm-simulator/src/interpreter.rs:1272` defines `fn touch(_m, _n, _q) {}` — a no-op with three underscore params

Looks like a debugging or trace hook that got left in. Either wire it
up to the trace runner or delete it.

---

## Highlights — Where the Architecture Shines

1. **`fsm-diagnostics` as a true foundation.** Zero workspace deps,
   re-exported by every consumer. `Span`, `SourceLocation`, `Severity`,
   `DiagnosticCode`, `Diagnostic`, `RelatedInfo` — one definition each,
   used identically everywhere. This is the textbook stable-foundation
   pattern (Bob Martin's "Stable Abstractions Principle"). The previous
   audit comment ("Span / SourceLocation moved out of fsm-parser to
   close cycle risk") tells the story of a deliberate refactor that
   actually completed.

2. **The hard "no parser/analyzer in codegen-c" rule is enforced and
   visible.** `crates/fsm-codegen-c/Cargo.toml:11–14` has a four-line
   comment block citing Doc 23 §4 and Doc 20 §11.5 explaining why those
   deps are absent. `grep -rn "fsm_parser\|fsm_analyzer"
   crates/fsm-codegen-c/src` confirms it: zero matches. Defensive
   commenting + grep-verifiable invariant.

3. **`fsm-formatter` actually walks the CST only.** No `fsm-analyzer`
   or `fsm-ir` import anywhere — `grep -rn "fsm_analyzer\|fsm_ir"
   crates/fsm-formatter/src` returns zero. The Doc 23 §4 rule "formatter
   sees no analysis" is upheld and tested by `tests/idempotency.rs`.

4. **Instability gradient flows in the correct direction.** `I = 0.00`
   for `fsm-diagnostics`, `I = 1.00` for `fsm-cli`, with monotone
   increase along the data-flow path (lexer 0.25 → parser 0.33 → ir
   0.20 → analyzer 0.60 → codegen-c/simulator/formatter 0.67-0.75 →
   cli 1.00). No "stable leaf" or "unstable foundation" anti-patterns.

5. **CLI is 1715 LOC total across 13 files** — `main.rs` is 35 LOC, each
   subcommand lives in its own `cmd/*.rs`, and the runtime test of
   conformance fixtures (`cmd/test.rs:538`) is one file because the
   responsibility is single (drive the test runner) even though the
   code is wide. This is exactly the orchestrator shape that lets the
   underlying crates stay focused.

6. **`Cargo.toml` workspace metadata is disciplined.** All version,
   edition, MSRV pins centralized in `[workspace.package]`. Optional
   serde / miette features on `fsm-diagnostics` keep the foundation
   lean for downstream `no_std` consumers (the lexer is `no_std`
   per its doc-comment). The `tokio` / `tower-lsp` /  `wasm-bindgen`
   exclusion is commented explicitly in
   `[workspace.dependencies]` so a v1.1 contributor can't accidentally
   re-add them.

7. **No external-crate cycle, no internal-module cycle.** Verified by
   `cargo tree`, `grep -rn '^use crate::'`, and `cargo build --workspace`
   exit code 0.

---

## Recommendations Summary

**v1.0 tag does not block on any architecture item.** The correctness
P0s in `docs/AUDIT_2026_05_14.md` are the blockers.

**Pre-v1.1.0 cleanup wave (1–2 days, all P1):**

1. P1-A2: Delete `fsm-analyzer` from `fsm-simulator/Cargo.toml` (one-line change).
2. P1-A1: Move analyzer's `fsm_parser::cst::*` usage to typed AST accessors; add missing accessors to `fsm-parser/ast/`.
3. P1-A3 + P1-A4: Introduce `fsm_ir::lca::{ParentResolver, lca_inclusive, effective_lca}` generic trait; rename simulator's `MachineIndex` to `MachineTopology` (or similar); make all three crates implement the same trait. Delete the duplicate impls.
4. P1-A5: Commit to `fsm_ir::IrVisitor` (P2-A4 same item) and refactor codegen/analyzer walks to use it; OR delete the visitor trait.
5. P1-A6: Split `lower.rs` (1356 LOC) into `lower/{mod, machine, state, transition, expr, helpers}.rs`.

**v1.1.0 cleanup wave (P2):**

- P2-A1 + P2-A2 + P2-A3 + P2-A5 + P2-A6: `pub` → `pub(crate)` audit across all four thick crates. Add `missing_docs` workspace lint to catch regressions.
- P2-A7: Split `interpreter.rs` into 4 files.
- P2-A8: Move lexer inline tests out of `lexer.rs`.

**v1.x tracking (P2/P3):**

- P2-A9: Introduce `StateId` (and `RegionId`, `EventId`) newtype in `fsm-ir`.
- P3-A1: Normalize `stable_id` to always-`String` or always-`Option<String>`.
- P3-A2: Drop the `SimpleState as SimpleStateObj` alias.
- P3-A3: Decide on `decompile` future (cfg-gate or accept stub).
- P3-A4: Lower `fsm-parser` two helper modules to plain `mod`.
- P3-A5: Wire up or delete `interpreter::touch`.

---

*End of Architecture & Coupling Audit, 2026-05-14.*
