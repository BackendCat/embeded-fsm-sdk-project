# Pre-Tag Architecture Audit — FSM Studio v1.1

| Field | Value |
|-------|-------|
| **Document ID** | AUDIT_PRE_TAG_v1_1_ARCH_2026-05-15 |
| **Date** | 2026-05-15 |
| **Lens** | Architecture, crate boundaries, complexity, quality-debt |
| **Scope** | v1.1 delta since `b480003` → main `b6f4db8` (35 commits, 15 waves) |
| **Mode** | Read-only. No code modified. No build run (warm quad at `b6f4db8` trusted: 656 pass / 0 fail, clippy `-D warnings` clean, fmt clean). |
| **Method** | Source-level verification of every claim — grep + read, not doc trust. Baselines from `AUDIT_B_ARCHITECTURE_2026_05_14.md`, `AUDIT_C_QUALITY_2026_05_14.md`. |

---

## VERDICT

**TAG-READY: YES** — **Architecture P0 count: 0**

The three architecture-debt paydown claims (AD-1/AD-2/AD-3) are **verified true in source**, not merely asserted. The dependency graph is acyclic and foundation-correct, `#![forbid(unsafe_code)]` held through all 15 v1.1 waves in all 9 crates, and determinism is structurally enforced at the IR type level (no hash-ordered collection serialized anywhere). No new god-object was introduced. Two carried-over P1 debt items (CST coupling, `pub` over-exposure) **regressed numerically** but are **non-behavioural, non-CI-blocking convention debt** — acceptable to ship in a v1.1 tag *as explicitly-tracked debt*, with a no-ship caveat on neither but a firm recommendation to gate them before v1.2.

| Source | P0 | P1 | P2 | P3 |
|--------|---:|---:|---:|---:|
| This pass (fresh) | 0 | 0 | 2 | 3 |

---

## AD-1 / AD-2 / AD-3 — Claim → Verified State

| ID | Claim (Audit B / retrospective) | Verified state | Evidence |
|----|--------------------------------|----------------|----------|
| **AD-1** | 3 duplicate LCA impls (analyzer/sim/codegen) → ONE generic `ParentResolver` in fsm-ir | ✅ **TRUE.** Exactly one walk: `fsm_ir::{ancestors,lca_inclusive,effective_lca}` generic over `ParentResolver`. All 3 carriers implement the trait and delegate; none re-walks. Codegen keeps a *thin* kind-rule wrapper (Local/Internal collapse) over the shared base walk — documented, behaviour-pinned. | Algorithm: `crates/fsm-ir/src/lca.rs:76-148`. Analyzer carrier: `crates/fsm-analyzer/src/lca.rs:99-122` (delegates at :91, :111, :121). Sim carrier: `crates/fsm-simulator/src/runtime/lca.rs:20-35` (delegates at :21, :34); `ParentResolver` impl `runtime/machine_index.rs:518`. Codegen carrier: `crates/fsm-codegen-c/src/emit/entry_exit.rs:35-77` (delegates at :76). Three oracle-equivalence test suites pin byte-identical behaviour (`fsm-ir/src/lca.rs:150-297`, `fsm-analyzer/src/lca.rs:277`, `fsm-simulator/src/runtime/lca.rs:264`). |
| **AD-2** | fsm-simulator declared fsm-analyzer as dead Cargo dep + duplicate `MachineIndex` | ✅ **TRUE.** `fsm-analyzer` moved to `[dev-dependencies]` (with a precise rationale comment); zero `fsm_analyzer::` refs in `fsm-simulator/src/` (only `examples/capture_trace.rs:14`, a scratch helper — the legitimate reason it stays a dev-dep). Exactly **one** `struct MachineIndex` workspace-wide. The analyzer's LCA carrier is the unrelated `LcaIndex`, not a `MachineIndex` dup. | `crates/fsm-simulator/Cargo.toml:36-49` (dev-dep + rationale). `grep fsm_analyzer crates/fsm-simulator/src` → none. Sole `MachineIndex`: `crates/fsm-simulator/src/runtime/machine_index.rs:104`. |
| **AD-3** | `LoweringCtx` god-object (32 methods, 27 `&mut self`) → eliminated/split | ✅ **TRUE.** No `struct/impl LoweringCtx` exists anywhere (only historical doc-comment refs). `lower/` is now 7 focused modules (2617 LOC vs the old ~2200-line single file). The sole mutable participant is `IdMinter` (5 methods, 3 `&mut self`, 3 counters); `LocCtx` is immutable; lowering is free functions threading `&mut IdMinter` + `&LocCtx`. Workspace `&mut self` in `lower/` dropped 27 → **6**. | `grep '^impl LoweringCtx\|^struct LoweringCtx'` → none. `crates/fsm-analyzer/src/lower/mod.rs:23-37` (structure doc). `IdMinter`: `crates/fsm-analyzer/src/lower/ids.rs:16-86`. `LocCtx`: `crates/fsm-analyzer/src/lower/loc.rs:19`. One tracked residue: dead field `IdMinter.m_idx` (`ids.rs:20-25`, `#[allow(dead_code)]`, Audit C P0-2, explicitly deferred to the W2b submachine decision). |

All three are genuine structural paydowns, each guarded by behaviour-equivalence tests against a hand-rolled transcription of the deleted code. This is exemplary refactor discipline.

---

## Findings (triaged)

### P2-1 — `pub` over-exposure regressed; still unguarded by any lint
- **Evidence:** Workspace bare-`pub` items **765** (Audit B P2-A1 baseline ~455); `pub(crate)` **18** (baseline 3). Per-crate: `fsm-parser` 241 bare-pub (Audit B: 88), `fsm-simulator` 109 (83), `fsm-codegen-c` 88 (80). `fsm-analyzer/src/lib.rs:29-34` still exposes 6 `pub mod` (Audit B P2-A2 untouched) though only `analyze`/`analyze_with_source`/`AnalysisResult`/`LcaIndex`/`SymbolTable` are consumed externally. No `[workspace.lints]`, `unreachable_pub`, or `missing_docs` anywhere (`grep Cargo.toml crates/*/Cargo.toml` → none).
- **Impact:** Each `pub` is a semver hazard the moment a tag is cut. v1.1's 15 waves added ~310 more pub items without the P2-A1 downgrade, so the stability surface a v1.1.0 tag freezes is ~13× larger than what's actually consumed. Non-behavioural.
- **Action:** Ship-acceptable as tracked debt for the v1.1 tag (it does not miscompile or break anything). **Strong recommendation:** make it a hard pre-v1.2 gate — add `unreachable_pub` + `missing_docs` workspace lints and do the `pub`→`pub(crate)` sweep (Audit B's estimated ~390 items, tests stay adjacent so no rewrites). **Not a v1.1 tag-blocker.**

### P2-2 — analyzer→fsm-parser CST coupling regressed materially
- **Evidence:** Audit B P1-A1 baseline = 16 `fsm_parser::cst::*` reach sites (8 typed-AST). Now: non-comment CST-reach (`SyntaxNode`/`SyntaxKind`/`cst::`/`.syntax()`/`SyntaxToken`) totals ~430 lines across **15** analyzer files — heaviest `lower/expr.rs` (101), `lower/state.rs` (75), `lower/machine.rs` (62), `checks/name_resolution.rs` (44), `checks/determinism.rs` (43). Typed-`ast::` usage also grew (212 refs), so it is *mixed*, not a wholesale regression, but the raw-CST footprint is now pervasive in lowering + checks (e.g. `lower/machine.rs:17`, `symbol_table.rs` :444/:479/:641 all `use fsm_parser::cst::SyntaxKind`).
- **Impact:** Any CST/`SyntaxKind` shape change in `fsm-parser` is now a many-site change in `fsm-analyzer` — the exact fragility Audit B P1-A1 flagged, amplified by the W2 submachine + W7-FU lowering rewrites. Architectural-cleanliness debt; **no behavioural defect** and the boundary is still a real Cargo dependency edge in the correct direction (analyzer→parser).
- **Action:** Acceptable to ship in the v1.1 tag as tracked debt — it is internal coupling, not a public-API or correctness issue. Should be a deliberate v1.2 epic (push analyzer fully above the typed AST, or formally bless raw-CST access as the analyzer's contract and stop calling it debt). **Not a v1.1 tag-blocker.**

### P3-1 — Large linear state-walker functions (no new god-object, but hotspots)
- **Evidence:** `crates/fsm-codegen-c/src/state_index.rs:` `visit_state` ≈242 lines; `crates/fsm-codegen-c/src/emit/dispatch_table.rs:` `emit_outer_dispatch` ≈180 (W7-FU-1/W2d); `crates/fsm-simulator/src/interpreter.rs:` `sync_submachines` 159 (W2 epic), `select_transitions` 149, `rtc_step` 149; `crates/fsm-analyzer/src/lower/state.rs:` `lower_state` 155.
- **Impact:** All are single-responsibility linear walkers/dispatchers with strong section comments, not branching-complexity god-functions; readable but long. Maintenance friction only.
- **Action:** Acceptable. Optional v1.2 readability pass (extract sub-steps). **Not a blocker.**

### P3-2 — `Parser`/`Lexer` impls exceed 25 methods
- **Evidence:** `crates/fsm-parser/src/parser.rs` `impl Parser` 33 fns; `crates/fsm-lexer/src/lexer.rs` `impl Lexer` 28 fns.
- **Impact:** Inherent to recursive-descent + tokenizer designs (one small method per grammar rule / token class). Not LCOM-style god-objects (cohesive, each method tiny). No action; recorded for completeness.

### P3-3 — Tracked dead field `IdMinter.m_idx`
- **Evidence:** `crates/fsm-analyzer/src/lower/ids.rs:20-25`, `#[allow(dead_code)]`.
- **Impact:** None (lint-clean). Already tracked as Audit C P0-2, explicitly deferred to the W2b submachine cross-machine-wiring decision.
- **Action:** None for v1.1. Resolve when W2b lands.

---

## Verified Clean / Structurally Sound (positive gate evidence)

1. **Single LCA algorithm (AD-1).** One generic in `fsm-ir`; three carriers delegate; three independent oracle-equivalence suites prove byte-identical pre/post behaviour. The divergence class that caused the Wave-1.9 region.initial bug is structurally closed.
2. **Dependency graph acyclic + foundation-correct.** `fsm-diagnostics` has **zero** workspace deps (only optional external serde/miette/thiserror) — `crates/fsm-diagnostics/Cargo.toml:11-18`; imports no workspace crate. `fsm-ir` depends only on `fsm-diagnostics` (no parser/lexer/analyzer import) — the cycle risk Audit B noted is held closed. Direction is sane end-to-end (diagnostics ← everything; ir ← codegen/sim/analyzer; analyzer → parser → lexer).
3. **`#![forbid(unsafe_code)]` in all 9 crates.** Verified present: diagnostics/lexer/parser/analyzer/ir/codegen-c/simulator/formatter (`src/lib.rs`) + cli (`src/main.rs`). The v1.0 P1 held through all 15 v1.1 waves.
4. **Determinism enforced at the type level.** `fsm-ir` serialized types contain **zero** `HashMap`/`HashSet`/`BTreeMap`/`BTreeSet` — ordered maps are `Vec<(K,V)>` (`crates/fsm-ir/src/model.rs:459-460`, `:989`), everything else `Vec<T>`. The only `HashMap` in the crate is the `#[cfg(test)]` LCA resolver (`lca.rs:163`). Byte-identical-output-for-identical-input is therefore a *structural property*, not merely a passing test; the W7-FU-2 fingerprint change was a deliberate spec-correctness reconciliation (default priority `0`→`100`, CHANGELOG L30) applied identically by sim + both codegen strategies, preserving the sim≡codegen determinism contract.
5. **AD-2 dead-dep removal is exact.** `fsm-analyzer` is `[dev-dependencies]` in the simulator with a rationale comment; the shipped library dependency surface no longer pulls the analyzer+serde tree.
6. **AD-3 mutation collapse is real.** `&mut self` in `lower/` 27→6; sole mutable type `IdMinter` (3 counters); behaviour pinned by an example-IR snapshot guard + the full analyzer suite.
7. **W7-FU-1 dispatch rewrite is well-factored.** Switch strategy: small focused fns (`trigger_event_c`, `trigger_id_of`, `emit_event_case`, `dispatch_switch.rs:217-297`); idiomatic `do{}while(0)` candidate scoping with thorough spec-cited rationale. Table strategy `_row_guard_enabled` (`dispatch_table.rs:279-411`) is a clean selection-time guard helper. No duplicated dispatch logic; no new god-object.
8. **No new god-object introduced by v1.1.** Only `Parser`(33)/`Lexer`(28) impls exceed 25 methods — both inherent, cohesive recursive-descent/tokenizer designs predating v1.1.

---

## Summary (≤150 words)

**TAG-READY: YES — Architecture P0 count: 0.** All three architecture-debt paydown claims verified true in source, not just asserted: AD-1 (one `fsm_ir::ParentResolver` LCA generic, three delegating carriers, three oracle suites), AD-2 (`fsm-analyzer` demoted to dev-dep, single `MachineIndex`), AD-3 (`LoweringCtx` god-object gone; `&mut self` 27→6; sole mutable `IdMinter`). Dependency graph acyclic and foundation-correct; `#![forbid(unsafe_code)]` held in all 9 crates through 15 waves; determinism is a structural property (zero hash-ordered collections serialized in `fsm-ir`). No new god-object. Two carried-over P1 items **regressed** — `pub` over-exposure (765 vs ~455, unguarded by any lint) and analyzer→CST coupling (~15 files vs 16 sites) — but both are non-behavioural, non-CI-blocking convention debt: **ship-acceptable as explicitly-tracked debt for the v1.1 tag**, with a firm recommendation to gate both before v1.2.
