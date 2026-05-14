# Reliability & AI-Friendliness Audit — 2026-05-14

**Auditor.** Senior Rust + AI-tooling reviewer (independent agent under
autonomy mandate). Read-only audit, no code modified.

**Scope.** All 9 crates under `crates/` (~26 350 LOC Rust, 153 `.rs`
files), `docs/00-Decisions-And-Reconciliation.md` and adjacent specs,
the `README.md` and the existing prior audit (`AUDIT_2026_05_14.md`,
`AUDIT_B_ARCHITECTURE_2026_05_14.md`). The parallel P0-1 worktree
`/root/dev/embeded-fsm-sdk-wt-p01/` was deliberately ignored.

**Method.** Grep cross-cuts over the panic / unwrap / expect family
filtered to non-test code; spot-reads of the 9 `src/lib.rs` and a
sampling of representative non-trivial functions; per-crate doc-coverage
scan of `pub` items; Doc 00 ↔ code traceability per blocker and per
TL-§10 decision; HashMap/output-ordering inventory; CLI argument and
public-API quality review.

---

## Verdict

**Reliability backbone is unusually strong for a Phase-1 codebase, AI-friendliness is excellent in the foundation crates and uneven in the AST/CLI layer.** Eight of nine
crates already declare `#![forbid(unsafe_code)]`, every crate uses
`thiserror` for its error type (no `Box<dyn Error>` lazy form), and the
non-test panic surface is small (one `panic!`, six `unreachable!`, ~25
`unwrap()`/`expect()` — every one falls into a "shouldn't happen by
construction" category except the two pre-known sites already flagged
by the prior audit at P1-8 and P2-2). The only acute reliability issue
*not* in the prior audit is silent guard-error downgrade `unwrap_or(false)`
inside `eval_guard` (prior audit P1-7, which I confirm and **expand to
three call sites at L586, L665, L1048**) and a determinism risk for
`HashMap<String, Value>` snapshot / trace serialisation that lands in
the public wire format (potential golden-test flake on schema replay).
AI-friendliness scores are sharply bimodal: the *foundation* crates
(`fsm-diagnostics`, `fsm-ir`, `fsm-codegen-c`, `fsm-analyzer`) score
9–10/10 on intent recoverability with rich Doc-§ cross-references in
prose; the *boilerplate-heavy* AST view (`fsm-parser/src/ast/state.rs`
with 40 undocumented `pub fn` accessors) and the CLI command modules
(8 undocumented public `run(args)` functions) drag the average down.
Doc 00 traceability is high: 12 of 14 blockers have clear code
references in either source comments or test names; the two missing
(B-02 simulator protocol, B-03 keyword drift, B-04 VS Code) are exactly
the items §6 D-01..D-04 already mark as **deferred to v1.1**, so the
absence is correct. Five TL §10 decisions of the six are honoured in
code; §10.1 submachines is partially landed (IR + analyzer, no codegen-c
emit path — confirms prior audit's silent-drop observation extending
beyond guards/actions).

---

# Reliability Findings

## Panic Audit

I grep'd for `unwrap()`, `expect(`, `panic!`, `unreachable!`, `todo!`,
`unimplemented!` over `crates/*/src/`, then filtered out
`#[cfg(test)] mod` blocks (via `{`-balanced range scan) and the parser
DSL's `p.expect(TokenKind, code)` method (which returns `bool`, not
panics). What remains:

| File:line | Path | Category | Reachable from user input? |
|---|---|---|---|
| `crates/fsm-codegen-c/src/state_index.rs:113` | `must_lookup` panic | (c) actual `fsm generate` path — analyzer-invariant violation; prior audit P1-8 | **Yes (via malformed IR)** |
| `crates/fsm-codegen-c/src/state_index.rs:176` | `u8::try_from(...).expect(...)` | (c) `fsm generate` of machine with >255 states; prior audit P2-2 | **Yes** |
| `crates/fsm-codegen-c/src/budget.rs:51` | `.expect("cannot compute budget for an empty IR document")` | (b) construction-checked — `emit` rejects empty machines at L98 via `EmitError::EmptyMachine`. Budget should never see one. | **No** (gated by `emit` precondition) |
| `crates/fsm-codegen-c/src/emit/history.rs:31` | `rec.history_pseudo.unwrap()` | (b) iterator filters on `rec.history_pseudo.is_some()`. Locally safe. | **No** |
| `crates/fsm-codegen-c/src/emit/source.rs:115` | `ctx.index.lookup(&r.ir_id).unwrap()` inside `walk_initial` | (b) `walk_initial` itself comes from the index — the id MUST exist by construction. **But this is undocumented** and an AI editor would have to reason about it. | **No** (proven safe but fragile) |
| `crates/fsm-codegen-c/src/stmt.rs:40,48,51,54,58,60,72,82,85,86,102,117,133,144` | 14 × `writeln!(out, ...).unwrap()` writing into a `String` | (b) `writeln!` to a `&mut String` is infallible — `fmt::Result` is `Err`-impossible for `String`. Rust idiom: `let _` or `unwrap`. Acceptable; could use the `write!`-to-String idiom that doesn't return `Result`. | **No** |
| `crates/fsm-ir/src/json.rs:91` | `parse_major(CURRENT_IR_VERSION).expect("CURRENT_IR_VERSION is well-formed")` | (b) `const &str = "1.0.0"` — verified at every compile. | **No** |
| `crates/fsm-lexer/src/lexer.rs:82,210,228,427` | 4 × `peek_char().expect("...")` | (b) every site checks `pos < len()` before calling | **No** |
| `crates/fsm-lexer/src/lexer.rs:177` | `unreachable!("lex_newline only called on \\r or \\n")` | (b) caller dispatches on \r/\n; prior audit P1-8 confirms | **No** |
| `crates/fsm-parser/src/parse.rs:37` | `ast::File::cast(self.syntax()).expect("root node is always FILE")` | (b) root construction guarantee | **No** |
| `crates/fsm-parser/src/opaque_type_validator.rs:45` | `chars.next().expect("non-empty checked above")` | (b) preceded by `if raw.is_empty() { return Err(...) }` | **No** |
| `crates/fsm-simulator/src/interpreter.rs:371,382,395` | 3 × `runtime.as_ref()/as_mut().expect("not initialized")` | (a/c) **Reachable.** Caller is supposed to check via `current_states_named` returning empty list, but `context()` / `virtual_clock_ms()` / `dispatch()` only check elsewhere. Public API; an embedder calling `context()` before `init()` panics. | **Yes** |
| `crates/fsm-simulator/src/eval/arith.rs:32` | `(-v).to_f64().unwrap()` on `Value::F64` | (b) operand has just been pattern-matched to `Value::F64` | **No** |
| `crates/fsm-simulator/src/eval/arith.rs:77,99,113,117,126` | 5 × `unreachable!()` in arithmetic dispatch | (b) earlier pattern arms exhaust the live cases | **No** |

**Non-test panic/unwrap/Option-expect total: 36.**

Per crate:

| Crate | Hard panic | unreachable | unwrap()/expect | Notes |
|---|---|---|---|---|
| fsm-codegen-c | 1 | 0 | 18 | 14 of 18 are `writeln!→String` (idiom); 2 (`state_index`) already P1-8/P2-2; 1 (`source.rs:115`) is fragile-but-safe |
| fsm-simulator | 0 | 5 | 4 | **3 of the 4 expects (`interpreter.rs:371,382,395`) are reachable from public API.** New finding. |
| fsm-lexer | 0 | 1 | 4 | All construction-guarded |
| fsm-parser | 0 | 0 | 2 | Both construction-guarded |
| fsm-ir | 0 | 0 | 1 | Const-string parse — safe |
| fsm-analyzer | 0 | 0 | 0 | clean |
| fsm-diagnostics | 0 | 0 | 0 | clean |
| fsm-formatter | 0 | 0 | 0 | clean (3 non-test `assert!` are positive-invariant guards) |
| fsm-cli | 0 | 0 | 0 | clean (45 `let _ =` are unused-arg-suppression, see below) |

**Recommendation.** The user-reachable panic surface is 3 sites: the
two prior-audit items in `state_index.rs` and the **new finding** in
`fsm-simulator/src/interpreter.rs:371, 382, 395`. The simulator
interpreter exposes `context()`, `virtual_clock_ms()`, `current_states()`
on a partially-constructed `Interpreter` and panics if `init()` was not
called. Either return `Result<&Context, StepError::NotInitialized>` or
return empty/zero values like `current_states()` already does (`L322`).
Pick one and apply consistently. The 14 `writeln!→String` unwraps in
`stmt.rs` are an idiom but a clippy `unused_must_use` or replacement
with `out.push_str(&format!(...))` would silence the pattern and remove
visual noise.

## Error Type Quality

Every error type that crosses a crate boundary is a `#[derive(thiserror::Error)]`
named enum with **at least one** variant per failure mode. Inventory:

| Crate | Error type | Variants | Quality |
|---|---|---|---|
| fsm-cli | `ConfigError` (in `config.rs:63`) | 2 (`Io`, `Parse`) | Carries `path: PathBuf` + `#[source] inner`. Excellent. |
| fsm-codegen-c | `EmitError` (in `emit/mod.rs:70`) | 3 (`EmptyMachine`, `QueueCapacityNotPowerOfTwo`, `TooManyStates`) | Good — but `TooManyStates` is currently `panic!` in `state_index.rs:176` instead of returned. Misalignment. |
| fsm-formatter | `FormatError` (in `format/error.rs:13`) | n.a. — non-recoverable I/O | Adequate |
| fsm-ir | `IrJsonError` (in `json.rs:17`) | 5 (parse, schema-mismatch, version) | Excellent |
| fsm-simulator | `TraceParseError` + `ExecError` (in `trace.rs:182/189`) + `QueueError` (in `runtime/queue.rs:28`) + `EvalError` (in `eval/expr.rs:17`) + `StmtError` (in `eval/stmt.rs:21`) + `StepError` (in `interpreter.rs:53`) | per-layer; **`StepError` composes via `#[from]`** from the inner errors. Exemplary. |

Anti-patterns observed:

1. **`unwrap_or(false)` on `eval_guard` swallows runtime errors** —
   prior audit P1-7 found one site at `interpreter.rs:665`; I find
   **three sites at L586, L665, L1048**, plus a fourth `unwrap_or(true)`
   at L588 on a parallel guard check. A guard that errors at runtime
   (overflow, divide-by-zero, missing extern) is silently mapped to
   `false`/`true`. Replace with explicit `Err(e) => return Err(StepError::Eval(e))`.

2. **`unwrap_or_default()` in `lower.rs` (66 sites in crates/, 30+ in
   `lower.rs` alone)** — the AST→IR lowering treats every missing name
   as empty-string. While the analyzer is *expected* to have produced a
   diagnostic for these gaps, the IR ends up containing
   `MachineObject { name: "", ... }` if lowering races a syntax-error
   recovery. The lowering does record diagnostics — but the empty-name
   IR still flows downstream. Either reject early on empty
   `machine_name` / `state_name` (return `Err(LoweringError)`), or
   continue but flag with a `partial: true` field on `Ir` so downstream
   consumers know the doc may have placeholders.

3. **CLI's `let _ = handler.render_report(...)`** in
   `cli/diagnostics.rs:37,38,40` — explicitly documented as
   "panic-safe — writer failures are silently discarded" (L24). For
   stderr write errors this is correct (program is about to exit
   anyway). Annotated. Acceptable.

4. **`let _ = unsorted_idx;`** in `dispatch_table.rs:360` and
   **`let _ = macro_prefix;`** in 6 codegen modules — these are
   unused-binding suppression for variables held in scope for later
   work or removed branches. Prior audit P3-1 flags them. They are not
   error swallowing.

Overall the error story is **strong by Rust 2024 standards**. The one
real defect is the guard-eval downgrade.

## Type Safety

- `#![forbid(unsafe_code)]` is **declared in all 8 library crates** (`fsm-cli`
  ships as a binary and inherits the lint from its deps; `grep` confirms zero
  `unsafe` blocks anywhere except a *comment* in `cst/kinds.rs:302` saying
  "Using a transmute would be tempting but `unsafe` is forbidden", which is
  itself proof of discipline).
- **No newtypes for state-ids / event-ids.** `pub id: String` everywhere
  in `fsm-ir/src/model.rs` (47 distinct `id: String` fields). A
  `StateId(Box<str>)` newtype would let the compiler reject "passed
  event_id where state_id expected" mistakes that are currently runtime
  bugs in the analyzer. Not blocking but a v1.1 polish target.
- **Numeric casts:** 38 `as`-cast sites in src code. Audited:
  - `lower.rs:1044` `priority.clamp(0, u16::MAX as i64) as u16` —
    correct (clamped before cast).
  - `simulator/runtime/value.rs:159-164` `as i32` / `as u8` etc. inside
    `Cast` expression evaluator — `Value::to_i64()` returns `Option`,
    the `as` cast is part of the documented `String as i32` user
    feature. The integer truncation is the **intended C-style semantics**
    of the `as` operator from the DSL; not a Rust safety problem.
  - `eval/arith.rs:51-56` `!(i as u8)` / `!(i as u16)` — bitwise-NOT on
    casted operand. The cast width is selected per pattern arm; safe.
  - `eval/arith.rs:111` `wrapping_shl(ri as u32)` — explicit wrapping,
    correct.

  No dangerous narrowing casts found.
- **`From`/`TryFrom` discipline:** `From<Diagnostic> for DiagnosticObject`
  in `fsm-ir/src/model.rs:69`. `#[from]` on `StepError` variants
  composes the layered errors cleanly. `TryFrom` used implicitly via
  `u8::try_from(records.len())` — appropriate, but should `?` rather
  than `.expect()` (P2-2 prior audit).

## Determinism Hazards

| Item | Site | Risk | Mitigation in place? |
|---|---|---|---|
| `HashMap<String, Value>` in `InterpreterSnapshot` | `runtime/state.rs:114, 117` | **HIGH** — `Serialize` will emit fields in HashMap iteration order, which is randomised per program run (RandomState seed). Golden trace tests on `expected: [StepRecord]` will flake if a snapshot is round-tripped. | No. Replace with `BTreeMap<String, Value>` or sort before serialize. |
| `Option<HashMap<String, Value>>` in `StepRecord.payload`, `StepRecord.context`, `TraceFile.init.context` | `simulator/trace.rs:105, 142, 157, 166` | **HIGH** — same problem at the wire format. Doc 13 §11 trace shape is the authoritative spec; an unordered context map breaks byte-exact golden-master tests. | No. |
| `HashMap<String, Arc<NodeRef>>` in `MachineIndex` | `runtime/machine_index.rs:104` | **MEDIUM** — iterated only for ancestor walks via lookup, not for output. Order of iteration is irrelevant. | OK. |
| `HashMap<String, u8> by_ir_id` in `StateIndex` | `codegen-c/state_index.rs:92` | **NONE** — lookup only; emit order is driven by `records: Vec<StateRecord>` (sorted document order). | OK. |
| `HashMap<(String, String), Vec<Trans>>` in `analyzer/checks/determinism.rs:92` | analyzer-internal | **LOW** — output is a list of diagnostic objects; diagnostics are tested by code, not by HashMap iteration. | OK. |
| `BTreeSet<String> _event_seen` reserved in `codegen-c/emit/mod.rs:94` | unused | n.a. | n.a. |
| `Instant`, `SystemTime`, `rand::*`, `Uuid` | grep returns **zero hits** in src | none | **Compile-time tool is wall-clock-free.** Pure-deterministic foundation. |

**Most consequential finding here.** The
`InterpreterSnapshot::history: HashMap<String, Vec<String>>` and the
trace-file `payload` / `context` maps are **part of the public wire
format**. As Doc 13 §11 marks the `StepRecord` shape as
"authoritative," wire-format determinism matters. Move both to
`BTreeMap` (or change the serde representation to sort keys before
emission) **before v1.0 tag**.

## Type-Safety / Invariant Density

- **assertions** outside test code: only 5 across the workspace
  (3 in `format/expr.rs` and `format/state.rs`, 2 in
  `parser/src/parser.rs` line 363-area for grammar invariants).
  No `debug_assert!` is used anywhere — even invariants that are clearly
  "should hold by construction" (e.g., the parent table walk-depth bound,
  the active-states-must-be-in-machine-index property) are silently
  trusted rather than `debug_assert!`'d. For a compile-time tool this
  is borderline acceptable, but a small `debug_assert!` budget would
  catch IR drift earlier. **Recommendation:** add
  `debug_assert!(ctx.index.by_ir_id.contains_key(&t.target))` at the
  top of each `emit_*` site that reads transition targets; helps the
  next AI agent that maintains the codegen.
- **`assert_eq!` in test modules**: 398 — healthy.

## Contract Consistency

Spot-read of every `lib.rs` (8 files): each crate's public API is
small (`fsm-ir` re-exports 24 IR types and 4 free functions;
`fsm-parser` exposes `parse()` / `parse_with_tokens()` returning
`ParseResult`; `fsm-codegen-c` exposes `emit() -> Result<EmittedFiles, EmitError>`).
Pre-conditions are stated in prose on each `pub fn`. Examples:

- `MachineIndex::ancestors(id)` documents *"Ordered list of ancestors of `id`, **including `id` itself** at index 0. Root-most ancestor is at the tail."* — sound.
- `emit(ir, config)` documents *"Pre-flight: queue capacity must be a power of two"* in code (`mod.rs:84`) — sound.

The contracts are mostly carried by **prose comments + the `Result<_, EmitError>` type**, not types alone. An AI agent doing a refactor wave will have to read the prose. That is realistic for a v1.0 — encoding all preconditions in types (newtypes, `NonEmpty<Vec>`, `PowerOfTwo<u8>`) would add 20–40% LOC for marginal benefit at this stage.

## Race conditions / deadlock risk

- `Arc<MachineIndex>` is shared across simulator runtime
  (`runtime/state.rs:23`). `MachineIndex` is `Clone + Send + Sync`
  (build-once + read-many). Read-only after construction — no Mutex/RwLock
  is needed. Correct usage.
- `Arc<dyn Fn(&[Value]) -> Value + Send + Sync>` for extern registry
  callbacks (`eval/extern_registry.rs:20`). Standard pattern.
- **No `Mutex`, `RwLock`, `static mut`, `thread::spawn`, `tokio::spawn`
  anywhere in src.** Compile-time tool with no async machinery; matches
  Doc 00 §6 D-02/D-03 deferral of WebSocket server.

---

# AI-Friendliness Findings

## Doc 00 → Code Traceability

| Decision | Status | Where in code |
|---|---|---|
| **B-01** Diagnostic codes auth (Doc 10) | ✅ | `crates/fsm-diagnostics/src/lib.rs:7` (docstring cites B-01), `lib.rs:153-…` (single enum source of truth) |
| **B-02** Simulator protocol unification | ✅ deferred | No code (Doc 00 §6 D-02 defers to v1.1). Correct absence. |
| **B-03** Keyword list drift | ✅ deferred | No code (Doc 00 §6 D-03/D-04 defer LSP/TextMate). Correct absence. |
| **B-04** VS Code conflict | ✅ deferred | No code (Doc 00 §6 D-04/D-05 defer VS Code). Correct absence. |
| **B-05** Pratt expression parser | ✅ | `crates/fsm-parser/src/expr.rs` (Pratt climber); `lib.rs:8` cross-refs B-05; `cst/kinds.rs` lists token kinds |
| **B-06** IR schema additions | ✅ | `crates/fsm-ir/src/model.rs` — `TransitionKind`, `ConstDecl`, `ImportDecl`, `FeatureDecl`, `QueueConfig`, `TargetConfig` all present per Doc 00 §7.4 |
| **B-07** Allow completion guards | ✅ | `crates/fsm-analyzer/src/checks/completion.rs`; `tests/positive.rs` has the regression case |
| **B-08** Parallel completion | ✅ in sim, ⚠️ in codegen | sim: `simulator/src/runtime/completion.rs:1` (`B-08`); codegen: `codegen-c/src/emit/completion.rs:1` emits helper but per prior audit P0-2 dead variables — partial |
| **B-09** Self-transition LCA | ✅ | `crates/fsm-analyzer/src/lca.rs::effective_lca`; cross-referenced in `simulator/src/runtime/event.rs:25` and `codegen-c/emit/entry_exit.rs` |
| **B-10** Switch ancestor walk | ✅ | `codegen-c/src/parent_table.rs:1` + `emit/dispatch_switch.rs:34` cite B-10; integration test in `tests/hierarchical_dispatch.rs` |
| **B-11** Table collect-then-execute | ⚠️ in sim, ❌ in codegen | sim: `interpreter.rs:16-18` faithfully comments + implements; codegen: `emit/dispatch_table.rs` comments B-11 but per prior audit P0-3 the emitted dispatcher does not actually do collect-then-execute |
| **B-12** C++17 (deferred) | ✅ deferred | `fsm-ir/src/model.rs:107` notes Cpp17 in profile vocab; `cli/cmd/generate.rs:26` cli-side comment "v1.0 only ships C99" |
| **B-13** Reject `after 0 ms` | ✅ | `crates/fsm-analyzer/src/checks/timer.rs` cited from `lib.rs:16`; negative test in `tests/negative.rs` |
| **B-14** History default required | ✅ | `crates/fsm-analyzer/src/checks/history.rs` + cited from `lib.rs:17`; codegen emits restore path; negative test |

**Score: 14 of 14 with clear evidence** (12 implemented, 2 correctly
absent because they're deferred). B-08 codegen-side and B-11 codegen-side
are *named* in comments but *broken* in implementation — those are not
traceability problems, they are correctness problems already
documented in `AUDIT_2026_05_14.md` P0-2 / P0-3.

### TL §10 Decisions

| Decision | Status | Where in code |
|---|---|---|
| §10.1 Submachines IN v1.0 (production) | ⚠️ | IR ✅ (`SubmachineRef` in `fsm-ir/src/model.rs:429`, analyzer check in `analyzer/checks/submachine.rs`); **codegen-c ⚠️** — only test fixtures stub `submachines: vec![]`, no `emit_submachine` exists in `emit/`. The codegen path silently no-ops on submachines. |
| §10.2 Doc 17 Assembly Integration | ✅ kept-as-informative | No code reference needed; spec is informational |
| §10.3 HAL mandatory | ✅ | `codegen-c/src/emit/hal.rs` emits HAL header on every machine; `lib.rs:15` cross-refs §10.3 |
| §10.4 MIT default license | ✅ | `codegen-c/src/emit/license.rs` + `CodegenConfig.license_spdx`; CLI flag in `cli.rs:97` `--license=<SPDX>` default `"MIT"`; tests in `codegen-c/tests/license_header.rs` |
| §10.5 English-only messages | ✅ | No i18n machinery; diagnostic strings are plain English in `fsm-diagnostics/src/lib.rs:154-…` |
| §10.6 C++17 naming deferred | ✅ deferred | Acknowledged in `model.rs:107` "Cpp17" enum variant kept for future |

**Score: 5 of 6 honoured; 1 (§10.1 submachine codegen) is the named-but-unimplemented gap.** This is the unflagged TL-decision miss — the prior audit doesn't call it out by name, but its presence parallels P0-1's silent-drop of guards/actions. Codegen-c silently drops submachine emit.

## Intent Recoverability Scores (5 sampled functions)

For each: would a fresh AI agent, with no conversation context, figure
out WHAT and WHY the function does from the code + comments alone?

| Function | Score 0–10 | Reasoning |
|---|---|---|
| `runtime/completion.rs::check_and_enqueue_completion` | **9** | Module docstring states the algorithm with two crisp bullet rules (composite vs parallel). Doc-§ refs lock it to Doc 08 §9 + B-08. Body matches docstring 1:1. |
| `analyzer/src/lca.rs::effective_lca` | **9** | Doctring of the module + Doc 00 §7.7 cross-ref. The function is 8 lines and the entire UML 2.5.1 self-transition corner case is spelled out in the module header. Exemplary. |
| `codegen-c/src/emit/source.rs::walk_initial` | **6** | Module-level docstring is one line; the function comment explains the "follow root.initial chain" intent in 1.5 lines. The `bounce < 32` magic constant is unexplained (looks like loop-bound defence-in-depth, but the value 32 is chosen "because" — no source-doc reference). The `.unwrap()` at L115 has no comment justifying why it can't fail. |
| `codegen-c/src/emit/dispatch_table.rs::emit_executor` | **3** | Comment says *"Per-row executor: runs exit-set, target action, target entry-set"* but the body **does not run exit-set / action / entry-set** — it just writes `m->_state = row->target` and a placeholder branch on `row->kind == 2 /* Internal */`. The docstring lies about the implementation. (This is the prior audit's P0-3 cited from a different angle.) |
| `simulator/interpreter.rs::select_transitions` (146-line function) | **5** | Outer comments explain the structure (collect for parallel events, separate path for non-event), but the inner B-11 collect/exit-set-de-duplication logic is encoded with comments that point at line ranges (`L611-621`) rather than at the *invariant* ("each region selects at most one transition that does not overlap a previously-selected exit set"). A fresh AI editing this would have to read all 146 lines. |

**Average: 6.4/10.** Foundation crates (`fsm-diagnostics`, `fsm-ir`,
`fsm-analyzer`) score 8–9; codegen emit modules (`source.rs`,
`dispatch_table.rs`, `dispatch_switch.rs`) score 3–6.

## Naming Clarity (20 items)

| Item | Score | Suggestion |
|---|---|---|
| `effective_lca` (`analyzer/lca.rs`) | 10 | No change |
| `check_and_enqueue_completion` | 10 | No change |
| `walk_initial` (`codegen-c/source.rs`) | 7 | `resolve_initial_leaf` would explain the **purpose** (find the entry-time leaf), not the **operation** (walk) |
| `must_lookup` (`state_index.rs`) | 6 | The name encodes Rust idiom ("`must_*` panics on miss") but doesn't say which kind of miss is "impossible." `lookup_or_panic_indexer_invariant` is long but explicit. Alternative: convert to `Result` and rename `try_lookup_required`. |
| `kind_for_event` (`interpreter.rs:514`) | 5 | Three words, all generic. `step_kind_from_event_kind` describes return-type → return-type. |
| `emit_one_case` / `emit_outer_dispatch` | 6 | "Case" of what? The dispatcher? Better: `emit_per_state_case` / `emit_dispatcher_outer_loop`. |
| `find_transition_at_pos` (`dispatch_table.rs`) | 4 | "pos" is overloaded (table position? source position? text position?). Body shows it's "find a transition row matching (source, trigger, document-order-index)." Rename `find_transition_by_table_index`. |
| `try_apply_row` | 6 | Could mean "try to apply" or "apply if applicable." `try_take_transition_row_at_state` is clearer. |
| `lower_external` / `lower_internal` / `lower_local` / `lower_completion` | 9 | Mirror the IR `TransitionKind` variants. Good. |
| `is_leaflike` | 7 | Half-defined ("leaf-or-pseudo-or-final"). One-line comment fixes it; the name alone is ambiguous. |
| `Trigger::Completion { from }` (IR) | 5 | `from` is a state id but reads like a verb argument. `source_state_id` or `completing_state` is clearer. |
| `event_received_for` | 4 | Subject + object swap. `event_received_record_from` or `step_record_for_event`. |
| `expand_exit_with_parallel` | 6 | Verb-noun-with-prepositional-phrase. `add_parallel_region_exits_to` is verbose but unambiguous. |
| `direct_child_of(idx, ancestor, descendant)` | 8 | Clear. |
| `arm_timers_on_entry` | 9 | Clear. |
| `record_history_before_exit` | 9 | Clear. |
| `RuntimeState::active_states: Vec<String>` | 7 | "Active leaves of every active region" — comment clarifies on L24-26; without the comment, "active states" could include composites. |
| `EmittedFiles { files: Vec<EmittedFile> }` | 8 | Redundant nesting but the `find()` helper makes the rationale clear. |
| `Diagnostic::with_related` | 9 | Builder-method convention obvious. |
| `MachineIndex::ancestors` returns `Vec<String>` including self | 8 | Comment on L75-76 spells this out; without it, "ancestors" colloquially excludes self. |

**Average: 7.0/10.** Higher than intent-recoverability because the
naming convention is largely consistent. The lowest-scoring items
cluster in codegen-c/emit (dispatch_table, dispatch_switch).

## Documentation Entropy

Per-crate doc-coverage scan (`pub fn` / `pub struct` / `pub enum` /
`pub trait` items with `///` doc on the line immediately preceding):

| Crate | Documented / Total | Score |
|---|---|---|
| fsm-diagnostics | 13 / 14 | **93%** — foundation, near-perfect |
| fsm-ir | 30 / 64 | **47%** — `model.rs:23/54` undocumented; most field-level items lack a `///` but get one-line struct-doc cross-refs. Acceptable for serialisation types. |
| fsm-lexer | 7 / 7 | **100%** |
| fsm-parser | 39 / 188 | **21%** — driven by `ast/state.rs` 0/40, `ast/transition.rs` 3/20, `ast/top_level.rs` 4/54. The AST is `ast_node!` macro-generated and its accessors are repetitive `pub fn name(&self) -> Option<String>`. Comment doctrine says "WHY-only" — but a fresh AI agent reading `state.rs:30` `pub fn name(&self) -> Option<String>` cannot know "is this the state's source-text name or the IR-id form?" without grep'ing the macro. Recommendation: add **one** `//! Pattern: every AST accessor returns `Option<…>` because the CST may have a parse-error placeholder` at the top of `ast/mod.rs` and call it done. |
| fsm-analyzer | 30 / 30 | **100%** |
| fsm-codegen-c | 32 / 36 | **89%** — emit modules a touch sparse but main types covered |
| fsm-formatter | 30 / 50 | **60%** — `format/machine.rs`, `format/top_level.rs` have undocumented `pub fn format_*` (the entry points). |
| fsm-simulator | 30 / 71 | **42%** — `eval/arith.rs` 0/4, `eval/extern_registry.rs` 0/4; the public `Interpreter` API has 10/18 documented. |
| fsm-cli | 6 / 28 | **21%** — every `cmd::*::run(args)` is undocumented (8 modules × 1 fn). Mitigated by `cli.rs` clap-derive doc-strings, which become `--help` output. |

**Weighted average ≈ 55%.** The drop is concentrated in:

- `fsm-parser/src/ast/*` — macro-boilerplate accessors.
- `fsm-cli/src/cmd/*` — `pub fn run(args)`.
- `fsm-simulator/src/eval/*` — arithmetic dispatch is undocumented.

None of these are user-API surfaces (clap covers CLI; ast accessors
are consumed by analyzer only). A fresh AI agent editing the
arithmetic dispatch would have to read 4 sibling `unreachable!()` arms
to deduce the type-narrowing scheme. **Recommendation:** add a 5-line
module docstring to `eval/arith.rs` explaining "we dispatch by value
type, then by operator; unreachable arms are pruned by earlier pattern
matches."

### README accuracy

Prior audit P1-9 flagged the README misrepresenting deferred features.
Re-verified:

- README L11: *"Simulator — WebSocket-based, virtual clock, deterministic replay"* — only "virtual clock, deterministic replay" is true for v1.0; "WebSocket-based" is **deferred to v1.1 per Doc 00 §6 D-02**.
- README L13: *"VS Code extension … LSP, live diagram panel, simulator panel"* — **all deferred per Doc 00 §6 D-03/D-04/D-05**.
- README L14: *"Web IDE — browser-based editor, diagram, and simulator"* — **deferred per Doc 00 §6 D-07**.
- README L48: *"13 — Simulator WebSocket Protocol"* link — Doc 13 itself is "Reserved, resurrects in v1.1" per Doc 00 §7.2.

**Recommendation: append a "v1.0 ships:" section listing only what's real (parser, analyzer, codegen-c, formatter, in-process simulator, CLI) and mark the deferred bullets with `[v1.1+]`.**

## Refactorability Risk

Files >800 LOC that an AI agent might struggle to refactor without
losing context:

| File | LOC | Concern |
|---|---|---|
| `fsm-analyzer/src/lower.rs` | 1356 | One `AstLower` struct, ~30 lowering methods. Reasonably modular per IR node type; an AI fix wave could edit per-method. Acceptable. |
| `fsm-lexer/src/lexer.rs` | 1319 | Single state machine; the function-per-token pattern is consistent. Acceptable. |
| `fsm-simulator/src/interpreter.rs` | 1272 | `Interpreter::dispatch`, `select_transitions` (146 lines), `execute_one_transition` (109 lines). **High** — these two functions are tightly coupled to RuntimeState mutation and would be hard to edit in isolation. Recommendation: extract `select_transitions` into `runtime/selection.rs`. |
| `fsm-ir/src/model.rs` | 1070 | Pure data types + serde derive. Trivially refactor-friendly. |
| `fsm-diagnostics/src/lib.rs` | 788 | Most LOC is the diagnostic code enum + match table; easy to extend. |

Functions >100 LOC outside lib.rs/test:

- `fsm-simulator/interpreter.rs::select_transitions` (146 lines), `execute_one_transition` (109 lines)
- `fsm-analyzer/src/lower.rs::lower_state` (140-ish), `lower_region`, `build_transition` (50)
- `fsm-codegen-c/src/emit/dispatch_table.rs::emit_outer_dispatch` (110+)

For an AI agent using the Edit tool, anything >100 lines requires
careful diff sizing. Add a 3-line header comment to each large function
stating "(a) what it returns, (b) which fields it reads/writes,
(c) which doc-section governs the algorithm" — that one paragraph turns
a 100-line edit from "need to read all of it" into "match doc-§ then
edit the relevant arm."

## Generated-Code Survivability

How confident is the AI agent that a `codegen-c/src/emit/source.rs` fix
is correct *without* running gcc?

Score: **5/10**. Reasons:

- The `MachineEmitCtx` carries enough context that an AI editing one
  emit function will see the type prefix and macro prefix at function
  entry. (+)
- The `Doc 11 §X` / `B-XX` annotations on file headers tell the AI
  *which spec section* to consult when in doubt. (+)
- `walk_initial.bounce < 32` magic bound is unexplained — an AI editor
  could bump it to 1024 "to be safe" and quietly increase codegen-time
  cost for legit deep machines. (−)
- The `EmitError` type has only 3 variants and `state_index::must_lookup`
  panics outside it. An AI fix that introduces a new IR construct would
  need to either know to extend `EmitError` *and* swap `must_lookup`
  for a `?`-propagating call, or just live with the panic. (−)
- The codegen does NOT have golden-master byte-exact tests
  (per `AUDIT_2026_05_14.md` P2-1); the existing tests are
  "the substring `Motor_init` appears in `Motor.c`" — an AI fix
  that changes the indent style or adds whitespace will pass these
  tests but might break downstream gcc-compile. (−)

**To raise to 8/10:** add (a) `cargo test`-gated gcc compile of the
three `examples/` machines (covers G4 + G6); (b) one byte-exact golden
master per dispatch strategy (covers cosmetics); (c) a `debug_assert!`
inside each `emit_*` confirming `ctx.index.by_ir_id.contains_key(...)`
for every id it reads from a transition.

## Toolability / Machine-Parsable Architecture

- **Cargo.toml dep graph** is parseable from `crates/*/Cargo.toml`
  alone. Run `cargo metadata --format-version 1` and `jq` the deps —
  fsm-diagnostics ← fsm-ir ← {fsm-parser, fsm-analyzer, fsm-codegen-c,
  fsm-simulator, fsm-formatter} ← fsm-cli. No cycles.
- **Architecture diagram**: lives in Doc 20. Not duplicated in a
  machine-readable form. A `docs/ARCHITECTURE.md` with a Mermaid
  graph plus a `tools/check_arch_deps.sh` walking `cargo metadata`
  would let CI flag dep-rule violations. v1.0.x polish.
- **Module paths predictable**: `mod {name}` → `src/{name}.rs` or
  `src/{name}/mod.rs` everywhere. No surprising `#[path]` overrides.
  Excellent. (Confirmed via spot-read across all 9 crates.)

## Invariant Discoverability

How easy is it for an AI agent reading the code to **discover** what
invariants the code maintains?

| Invariant | Documented where? | Asserted? |
|---|---|---|
| "Active state must always be in MachineIndex" | `runtime/state.rs:24-26` (comment on `active_states`) | No |
| "Queue capacity is a power of two" | `codegen-c/emit/mod.rs:73-77` (enum variant docstring) | Yes — `EmitError::QueueCapacityNotPowerOfTwo` check at `emit/mod.rs:85` |
| "State count ≤ 255 for codegen-c" | `state_index.rs:177` (expect message) | Yes — `u8::try_from`, but as panic (P2-2) |
| "Every transition target is in by_ir_id" | Nowhere | No |
| "`Trigger::None` means completion transition" | `simulator/interpreter.rs:646` (inline comment) + `IR` doc 09 reference | No |
| "IR major version 1.0.0 has zero-tolerance schema drift" | `model.rs:24-26` (Ir struct doc) | Yes — `IrJsonError::IncompatibleMajorVersion` |
| "Defer bitmask ≤ 32 events (capped uint32_t)" | `codegen-c/emit/defer.rs` comment + `analyzer/checks/defer.rs` E0903 | Yes — analyzer-emitted diagnostic |
| "Parallel regions all-final = parent complete" | `runtime/completion.rs` module header + B-08 | No (sim relies on iteration). C codegen has `_completion_depth <= 64` runtime assert in generated C. |
| "Self-transition external uses parent LCA" | `analyzer/lca.rs` module header + B-09 | No |

**Pattern:** invariants enforced at *crate boundaries* (Result errors)
are well-discoverable; invariants that hold *within* the runtime
(active-state-must-be-in-index, target-must-be-in-by_ir_id) are
documented in prose only. The fix is cheap: **a `debug_assert!` per
invariant at the top of the function that depends on it**, with the
invariant statement copy-pasted from the docstring. Helps both
correctness and AI-recoverability.

---

## Tracking items by severity

| P0 | P1 | P2 | P3 |
|---|---|---|---|
| 0 | 4 | 7 | 3 |

### P1 — High

- **P1-A** `Interpreter::context()` / `virtual_clock_ms()` / `snapshot()` panic if `init()` wasn't called (`interpreter.rs:371, 382, 395`). Public API. **NEW** (not in prior audit). Convert to `Option<&Context>` / `Option<u64>` or return `Result<_, StepError::NotInitialized>`.
- **P1-B** Guard-eval errors silently downgraded at `interpreter.rs:586, 665, 1048` (3 sites). Prior audit P1-7 found 1. Replace `unwrap_or(false)` with `Err` propagation.
- **P1-C** `InterpreterSnapshot.history` + `StepRecord.payload`/`context` use `HashMap<String, Value>` — wire-format non-determinism, breaks Doc 13 §11 byte-exact contract under serde. **NEW**. Convert to `BTreeMap` or sort-on-serialize.
- **P1-D** Submachine codegen-c emit path is missing (TL §10.1 partial — IR + analyzer present, codegen-c silent no-op). **NEW** (orthogonal to prior audit's P0-1 guard-drop). Either emit submachines or fail with `EmitError::SubmachinesUnsupported` so v1.0 examples can't accidentally claim to ship.

### P2 — Medium

- **P2-A** `lower.rs` `unwrap_or_default()` 30+ sites: lowering silently produces empty names on malformed AST. Add an `Ir.partial: bool` flag or fail-fast on first empty name.
- **P2-B** Add `debug_assert!` density (estimated +50 sites at invariant boundaries) so the next AI fix wave fails fast instead of producing surprising codegen.
- **P2-C** Convert `state_index::must_lookup` panic and the 14 `stmt.rs` `writeln!→String` unwraps to either `Result` or `String::push_str` idiom — eliminates the panic-on-the-bookkeeping-path noise.
- **P2-D** `walk_initial.bounce < 32` (`emit/source.rs:143`) and `interpreter.rs:705 while guard < 1024` (prior audit P2-5) magic constants need a named `const MAX_INITIAL_CHAIN_DEPTH: usize = 32;` and `const MAX_ANCESTOR_WALK: usize = 256;` plus the rationale.
- **P2-E** Documentation entropy in `fsm-parser/src/ast/*` (21% covered) and `fsm-simulator/src/eval/*` (≤ 40%) — add 1-2 line module-level docstrings explaining the macro / pattern conventions; that single change raises the doc-coverage metric by ~25 percentage points.
- **P2-F** `Trigger::After` / `Trigger::Every` variants exist in the IR but are unused (prior audit P2-12). Either wire or remove — leaving dead variants in a 1.0 schema is a future-proofing trap.
- **P2-G** No newtype for state-id / event-id (`pub id: String` throughout `fsm-ir/src/model.rs`). v1.1 polish; would catch a class of "passed event_id where state_id expected" runtime bugs at compile time.

### P3 — Low

- **P3-A** README advertises deferred features (re-confirms P1-9 from prior audit).
- **P3-B** `let _ = macro_prefix;` style suppression in 6+ codegen emit modules (prior audit P3-1).
- **P3-C** `debug_assert_eq!` is unused workspace-wide. The conditional-compile invariant-guard pattern is missing — add it where the runtime walks unsafe-by-construction assumptions.

## Highlights

- **Best file**: `crates/fsm-diagnostics/src/lib.rs` — every public type
  has a `///` block explaining the doc-§ that authorises its shape;
  zero deps; `#![forbid(unsafe_code)]`; `Span::merge` / `Span::empty`
  builder docs are textbook. An AI agent could maintain this file with
  near-zero context loss.
- **Best inter-crate boundary**: `EmitError` ← `MachineEmitCtx` ←
  `emit(ir, config) -> Result<EmittedFiles, EmitError>`. Three layers,
  one error type, every variant is `thiserror`'d with a self-explaining
  message.
- **Worst file for AI editing**: `codegen-c/src/emit/dispatch_table.rs`
  — comments promise B-11 collect-then-execute, code does early-return
  walk; an AI told "fix the dispatcher" would have to read all 362
  lines to find the divergence.
- **Most surprising finding**: `Interpreter` public API panics if
  callers read state before `init()` — not in the prior audit. CLI
  callers always init first; embedders writing a Rust-host test might
  not. **One-line `unwrap_or_default()` → `Option<…>` fix**.
- **Cheapest high-impact reliability fix**: change three `HashMap` sites
  (`runtime/state.rs:114`, `trace.rs:105, 142`) to `BTreeMap`. Single
  trait, sub-100-LOC delta, eliminates a wire-format non-determinism
  class.
- **Highest AI-friendliness ROI**: add a 3-line docstring header to
  every `pub fn run(args)` in `crates/fsm-cli/src/cmd/*.rs` and a 5-line
  module docstring to `fsm-parser/src/ast/state.rs` and `mod.rs`. Total
  effort < 50 LOC, raises doc-coverage from 21% to ~70% in the lowest-
  scoring crates.

---

*End of AUDIT_D_RELIABILITY_AI v1.0 — 2026-05-14.*
