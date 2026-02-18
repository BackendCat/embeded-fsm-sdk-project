# FSM Studio SDK — Specification Gate Review

**Date:** 2026-02-18
**Scope:** All 24 specification documents in `docs/`
**Core question:** Can an engineering team build this SDK from these docs alone, with zero ambiguity?
**Short answer:** Not yet. Close, but not yet.

---

## Executive Summary

The spec corpus is genuinely impressive in scope and depth. The formal execution semantics, the C99 codegen with ISR-safe queues, the diagnostic catalog, the conformance suite structure -- this is serious, well-thought-out work. The problems are not about missing vision. They're about the docs growing organically and falling out of sync with each other.

The single biggest issue: **the same fact gets defined in multiple documents, and the definitions disagree.** Diagnostic codes, simulator methods, keyword lists, VS Code settings, transition algorithms -- each of these is specified in 2-4 places with different values. Every review will keep finding "new" problems until this structural issue is fixed. (See the recommendation at the end about single-source-of-truth.)

**Tally:**

| Severity | Count | What it means |
|---|---|---|
| BLOCKER | 14 | Implementation would produce wrong behavior or fail to compile |
| GAP | 42 | An implementer would have to guess or stop and ask |
| INCONSISTENCY | 28 | Two documents say different things about the same concept |
| WEAKNESS | 24 | Works but fragile, surprising, or could be better |
| NIT | 11 | Minor polish, no functional impact |

**Verdict:** Needs revision. The 14 blockers are real -- things like the codegen not handling composite-state transitions, the C++ STL profile heap-allocating despite the heap-free guarantee, and the expression grammar being unparseable as written. These aren't style issues; they'd cause implementation failures.

---

## 1. Cross-Document Consistency

This is the heart of the problem. Most findings here boil down to: "Document A says X, Document B says Y, and they can't both be right."

### The diagnostic code mess (BLOCKER)

This is the worst offender. Diagnostic codes are defined in at least three places -- Doc 02 (Language Requirements), Doc 04 (DSL Spec), and Doc 10 (Diagnostic Catalog) -- and they substantially disagree.

**The big conflict** (1.8): Doc 02 assigns E0100-E0102 to semantic errors (nondeterminism, guard overlap, illegal boundary crossing). Doc 10 assigns E0100-E0102 to symbol resolution (unknown state, unknown event, unknown extern). The entire E01xx range was reassigned at some point and Doc 02 never got updated. Same story for E0001-E0005 -- Doc 02 uses them for structural analysis, Doc 10 uses them for lexer errors.

*Fix: Designate Doc 10 as the single normative source. Replace the code table in Doc 02 Section 13 with "See Doc 10."*

**Internal contradictions in Doc 04** (1.4, 1.5, 1.6): Doc 04 itself can't agree on codes. Section 10 says duplicate `@id` produces E0750; Section 14.2 says it produces E0025. (Doc 10 says E0025 -- that one's right.) Section 3 says opaque field guards produce E0303, but Doc 10 defines E0303 as "join source not in parallel region" -- totally different error. Section 15 says using a feature-flagged construct produces E0600, but Doc 10 defines E0600 as "parallel region has no initial."

**Phantom codes** (1.1, 1.2, 1.3): Doc 04 references three diagnostic codes that simply don't exist in the catalog -- FSM-W0604 (lossy float cast), FSM-E0110 (local transition target not descendant), and FSM-H0006 (@id ordering hint). They need to be added to Doc 10.

**Catalog duplication** (1.7): E0304 and E0600 describe the exact same condition ("parallel region has no initial"). One should be retired.

### The simulator protocol split (BLOCKER)

Docs 03 and 13 describe what is supposed to be the same simulator protocol, but they're so different they might as well be describing different systems.

**Method naming** (1.9): Doc 03 uses dot notation (`sim.init`, `sim.step`). Doc 13 uses slash notation (`sim/init`, `sim/step`). For JSON-RPC, this matters -- the method string has to match exactly.

**Different method sets** (1.9): Doc 13 has `sim/load`, `sim/unload`, `sim/listInstances`, `sim/dispatch`, `sim/resume` -- none of which exist in Doc 03. Doc 03 has `sim.inject`, `sim.run`, `sim.getSchema` -- none of which exist in Doc 13.

**Response schemas disagree** (1.10): Even for the methods that do overlap, the response shapes differ. Doc 03's `sim.init` returns `{ active: string[] }`. Doc 13's `sim/init` returns `{ activeStates: [...], traceId }`. Different key names, different fields.

**Notifications disagree** (1.11): Doc 03 defines 8 granular notifications (`sim.onStateEntry`, `sim.onStateExit`, etc.). Doc 13 defines 4 coarser ones (`sim/stateChanged`, `sim/breakpointHit`, etc.). An implementer building the VS Code extension against Doc 03 would write code that never receives the notifications defined in Doc 13.

**Trace record format** (1.22): The trace/step record is defined three times in three incompatible schemas across Docs 03, 13, and 24.

*Fix: Make Doc 13 authoritative. Replace the simulator sections in Doc 03 with cross-references.*

### Keyword lists are a moving target (1.12, 1.13, 1.14)

Doc 04 Section 1.5 defines the master keyword list. Every other document that references keywords has diverged from it:

- The TextMate grammar (Doc 21) is missing 11 keywords from Doc 04 and includes 10 that aren't in Doc 04.
- The LSP spec (Doc 14) claims to "match Doc 04 Section 1.5" but is missing 28 keywords and adds 10 extras.
- The formatter spec (Doc 19) uses syntax that doesn't match the grammar at all (see 2.21 below).

*Fix: One keyword list. One document. Everyone else cross-references it.*

### VS Code extension: three conflicting specs (1.19, 1.20)

Three documents define VS Code commands -- Doc 03 (8 commands), Doc 22 (9 commands), and Doc 05 (14 commands). Command IDs differ (`fsm.restartServer` vs `fsm.restartLanguageServer`). Prefix convention differs (`FSM Studio:` vs `FSM:`). Same story for configuration settings -- `fsmLang.lspPath` vs `fsmLang.compilerPath`, settings that exist in one doc but not others.

*Fix: Doc 22 (the actual manifest spec) should be authoritative. Others reference it.*

### Other consistency issues

- **Exit codes** (1.18): Doc 03 defines 3 exit codes (0-2). Doc 18 defines 5 (0-4). Neither references the other.
- **IrVisitor doesn't match IR schema** (1.15, 1.16, 1.17): The visitor in Doc 20 calls `m.states` and `m.transitions` directly on the machine object, but the IR schema (Doc 09) nests states inside `root: RegionObject`. The visitor also has no method for extern declarations.
- **Doc 02 contradicts itself** (1.21): Section 18 says inline computation is "out of scope." Section 9 defines a full inline expression language with `if/else`, `while`, `for`, and arithmetic.

---

## 2. Completeness

The question here is: if I sat down to implement each component, would I have enough information? In most cases the answer is "almost" -- but the gaps that remain are exactly the kind that cause a developer to stop and file a question.

### Parser: the expression grammar is broken (BLOCKER)

**Left recursion** (2.4): The `expr` rule in Doc 04 Section 8.7 is `expr = expr, bin_op, expr` -- directly left-recursive. This is a textbook problem: a recursive descent parser will loop forever. Section 8.7.1 provides a Pratt-parser precedence table, which is the right approach, but the grammar itself is the normative spec and it's not implementable. An engineer reading only the EBNF (which is what most people would do first) would produce a broken parser.

The guard expression grammar (2.5) has the same issue.

*Fix: Either rewrite the EBNF in non-left-recursive form or add a prominent note that the Pratt table is normative and the EBNF is informational.*

### Lexer edge cases (2.1, 2.2, 2.3)

Several underspecified areas:
- **UTF-8 handling** (2.1): Source encoding is UTF-8 but identifiers are ASCII-only. When a multi-byte character appears in identifier position, the lexer's error recovery "advances by one byte" -- which would split a multi-byte character and corrupt the stream.
- **Integer underscores** (2.2): Are `0x_FF`, `123_`, and `1__000` legal? Not stated.
- **Float literals** (2.3): No exponent notation, no NaN/Inf rules, no underscore rules.

### IR schema is missing several constructs (2.7, 2.8, 2.9, 2.10, 2.11)

The IR (Doc 09) is well-structured but has real holes:

- **No `const` declarations** (2.7): The DSL supports `const MAX_RETRIES = 3` and timers reference constants, but the IR can't represent them. This breaks the "round-trippable" design goal -- `fsm decompile` can't reconstruct constants.
- **No `feature` flags, `import` declarations, `queue` config, or `target` profiles** (2.8): A codegen module consuming only the IR can't determine queue capacity or overflow policy.
- **No `cast` expression or `float` literal** (2.9): The `as` operator and f32/f64 default values have no IR representation.
- **No `enum_variant` literal** (2.10): Guard comparisons against enum variants would lose symbolic names.
- **No local transition kind** (2.11): The transition schema has `internal: boolean` but can't distinguish local (`~>`) from external (`->`) transitions. LCA computation differs between them.

### C99 codegen gaps (2.12-2.16)

- **No periodic timer template** (2.12): `every N ms` has no codegen spec. Only one-shot `after` is shown.
- **No local transition template** (2.13): The `~>` operator's different LCA behavior needs its own codegen section.
- **No `send EVENT to M` codegen** (2.14): Cross-machine communication has no C99 implementation.
- **Table sort order unspecified** (2.15): The table-driven dispatch does linear scan with early return, but the required sort order (innermost first, by priority) is never stated.
- **Defer bitmask caps at 32 events** (2.16): Uses `uint32_t`. No fallback for machines with >32 event types.

### C++17 codegen is thin (2.17, 2.18)

Doc 12 says "C99 behavior applies where not mentioned," but that's not enough for the cases where C++ class structure changes the picture:
- No timer `tick()` method spec
- No `std::visit`-based dispatch for STL variant profile
- No completion event handling in C++ class context
- No inline action codegen
- No `send EVENT to M` in C++

### LSP has declared-but-unspecified capabilities (2.19, 2.20)

The `ServerCapabilities` response declares `documentHighlightProvider` and `workspaceSymbolProvider` as `true`, but Doc 14 has no sections specifying what they actually do. Doc 03 also lists `codeLens`, `signatureHelp`, and `rangeFormatting` as SHOULD capabilities, but they're absent from Doc 14.

### Formatter spec uses wrong syntax (2.21, 2.22)

The formatter spec (Doc 19) uses `composite Name {` and `parallel Name {` as declarations -- but the DSL grammar uses `state` for everything. It uses `internal on EVENT` -- but `internal` isn't a keyword. It uses `history shallow` as two words -- but the grammar has `shallow_history` as one token. It puts semicolons after context fields and braces around action blocks -- neither is in the grammar. And it doesn't cover `import`, `language`, `queue`, `target`, `enum`, `as`, `send...to...`, `raise`, or `feature` declarations at all (2.22).

### Other gaps

- **`fsm compile` vs `fsm generate` confusion** (2.25): Doc 18 says they're aliases (both do codegen). Doc 03 says `fsm compile` produces IR, not code. Fundamentally different pipeline semantics.
- **No CST/AST JSON schema** (2.24): `fsm parse --emit-cst` dumps JSON, but no schema exists.
- **VS Code snippets generate invalid syntax** (2.26): Snippets include semicolons and braces not in the grammar.
- **WebView message protocol unspecified** (2.27): Only two example messages shown; no complete protocol.
- **Reconnection protocol missing from Doc 13** (2.23): `sim.reconnect` exists in Doc 03 but not in the authoritative simulator protocol.

---

## 3. Semantic Correctness

These are the findings where the spec defines behavior that is actually wrong -- not just inconsistent or incomplete, but would produce incorrect state machine execution.

### Guarded completion transitions: forbidden but also allowed (BLOCKER)

**The contradiction** (3.1): Doc 08 Section 4.4 says "Completion transitions MUST NOT have a guard (FSM-E0301 if present)." But Doc 02 defines completion as `done [g] -> T : a` with an explicit guard. And Doc 04's grammar includes `guard_clause` as optional on completions. The formal semantics forbids what the grammar permits. Guarded completions are standard UML -- the prohibition should be removed.

### Parallel state completion is broken (BLOCKER)

**The problem** (3.2): When does a parallel state fire its completion transition? Doc 08 Section 9.1 suggests each region independently generates a completion event when it reaches its final state. This is wrong -- it would fire the `done` transition prematurely when the first region completes, not when all regions complete. UML semantics says: completion fires only when ALL regions have an active final state. The DISPATCH algorithm in Doc 04 has the same issue.

### Switch-based codegen doesn't handle composite states (BLOCKER)

**The problem** (3.8): The switch dispatches on `m->_state`, which is always a leaf state. If a transition is declared on a composite parent (e.g., `Operational` handles `FAULT -> Error`), and the current leaf is `Operational.Running.Normal`, the switch will never match `Operational`. The formal semantics requires walking from leaf to root looking for enabled transitions -- the codegen doesn't do this.

This is the kind of bug that would pass simple tests (flat machines) and silently fail on real-world hierarchical machines.

### C++17 STL profile heap-allocates (BLOCKER)

**The problem** (3.10): The STL profile uses `std::queue<Event>`, which is backed by `std::deque`, which heap-allocates. Doc 02 Goal G2 says "Generated runtime MUST NOT require dynamic memory allocation." This is a direct violation. Both profiles (STL and no-STL) need a fixed-capacity circular buffer.

### Other semantic issues

- **LCA for self-transitions is wrong** (3.3): The spec defines "a state is its own ancestor," so LCA(S,S) = S, which means external self-transitions wouldn't exit and re-enter. UML requires they do. Fix: LCA(S,S) = S.parent for external self-transitions.

- **`raise` event ordering is ambiguous** (3.4): Multiple `raise` calls during one RTC step get front-inserted individually, producing LIFO order. Is that intentional? It's probably not -- FIFO in declaration order would match developer expectations.

- **Deep history with parallel regions** (3.6): Deep history stores "the deepest active descendant" as a single state ID. For parallel sub-states, you need one state per region. The C99 codegen (Doc 11 Section 21) stores a single `StateId`, which is wrong.

- **Shallow history stores the wrong thing** (3.7): The codegen stores `m->_state` (the leaf), but shallow history should store the direct child of the composite -- not the leaf. Wrong for nested composites.

- **Table-driven dispatch breaks parallel regions** (3.9): Early `return` after first match prevents event delivery to other regions. Need to collect one transition per region before executing.

- **Per-region deferral not implemented** (3.5, 3.11): Doc 08 specifies per-region defer sets for parallel states. The C99 codegen has a flat per-machine defer model. When a parallel state exits, the per-region defer lifecycle is unspecified.

- **Two transition selection algorithms, different results** (3.16): Doc 04 Section 11 and Doc 08 Section 4.1 each define a normative transition selection algorithm. They can produce different results. One should be informative, one normative.

- **`after 0 ms` never fires** (3.17): The codegen checks `remaining_ms > 0`. For a 0ms timer, `0 > 0` is false. Either prohibit it or fix the codegen.

- **History without default = undefined behavior** (3.18): If no default target exists and no history is stored, behavior is "implementation-defined." This violates G1 ("No undefined behaviour"). Make `defaultTarget` mandatory.

- **Completion depth counter is global state** (3.12): `_completion_depth` is `static uint8_t` (file-scope), shared across all machine instances in a translation unit. Should be per-instance in the context struct.

- **Parallel dispatch generates compiler warnings** (3.15): The switch fallthrough pattern triggers `-Wimplicit-fallthrough` under the spec's own required `-Wall -Wextra -Werror` flags.

- **Signed/unsigned cast semantics undefined** (3.13): The `as` operator doesn't specify signed-to-unsigned conversion behavior. Mixed-sign comparisons in guards have no specified semantics.

- **Codegen error code references are wrong** (3.19): Section 26 of the C99 spec cites E0750 and E0302 for conditions that don't match their definitions in Doc 10.

---

## 4. Missing Specifications

These are whole topics that the spec corpus should address but currently doesn't.

### Security (GAP) — 4.5

Nobody wrote a security section. This matters because:
- The `import "path"` syntax allows path traversal. Nothing prevents `import "../../etc/passwd"`.
- The `opaque "C_type"` syntax emits strings verbatim into generated C. That's a code injection vector: `opaque "int; system(\"rm -rf /\"); int"`.
- The simulator opens a WebSocket on `0.0.0.0` with no authentication.
- The Web IDE share URL encodes project state as base64 -- potential XSS.
- No parser input size limits exist (denial of service via huge files).

### Memory budget computation (GAP) — 4.4

Doc 02 promises "Worst-case memory footprint MUST be computable from the model at compile time." But no document actually specifies how to compute it. There's no formula for `sizeof(M_t)`, no `--report-memory` flag, no stack depth analysis. The completion depth counter allows 100 recursive `Motor_dispatch` calls, which on a Cortex-M0 with 2KB of stack would blow up.

### Build system (GAP) — 4.1

No `Cargo.toml` workspace file. No pinned dependency versions. No `rust-toolchain.toml` content. Two engineers would make different choices.

### CI/CD (GAP) — 4.2

The CI YAML covers basic tests but nothing about: WASM builds, cross-compilation for the 5 target platforms, release workflow, VS Code Marketplace publishing, npm publishing, SARIF upload, or build caching.

### DSL version migration (GAP) — 4.3

The `language fsm 2.0` header implies future versions. But there's no `fsm upgrade` command, no multi-version support, and no deprecation policy.

### Plugin API (GAP) — 4.6

Doc 03 mentions "generator plugins" but there's no plugin interface, registration mechanism, or format specification. The `IrVisitor` trait is the closest thing but isn't documented as a plugin API.

### Other gaps

- **IR migration** (4.7, WEAKNESS): Versioning rules exist but no migration tool for breaking changes.
- **Performance limits** (4.8, WEAKNESS): No documented max state count (255 for uint8_t IDs), max event count (32 for uint32_t bitmask), or analysis time budgets.
- **i18n** (4.9, WEAKNESS): A `docs/RU/` directory exists, suggesting international intent, but no localization strategy is specified.
- **`fsm doc` output** (4.10, WEAKNESS): The command exists but its output structure is unspecified.
- **Generated code licensing** (4.11, WEAKNESS): Users shipping generated code as firmware need to know what license it's under.

---

## 5. Design Quality

Most design choices are solid. The CRTP default for C++17 embedded is correct. The power-of-2 queue capacity for bitwise-AND modulo is the right embedded optimization. The 200ms LSP debounce matches rust-analyzer and TypeScript conventions and is properly configurable.

A few areas where the design is good but the documentation could help implementers more:

- **Switch vs table-driven selection** (5.1, WEAKNESS): The "<64 states" threshold for choosing switch vs table is stated without justification. No formula for ROM/RAM tradeoff, no crossover point analysis. The LCA table alone is O(N^2) -- 10KB for 100 states -- which may dominate ROM regardless of dispatch strategy.

- **Shadow-only config resolution** (5.2, WEAKNESS): The nearest `fsm.toml` completely replaces parents instead of merging. This is defensible but surprising -- most tools (Cargo, ESLint, .gitignore) merge config chains. Adding a subdirectory `fsm.toml` to override one setting silently drops all parent settings. At minimum, add a prominent warning and example. An `extend = "../fsm.toml"` key would help monorepos.

- **Cross-file rename UX** (5.3, WEAKNESS): The LSP shows an error instead of performing an in-file rename when cross-file references exist. Better approach: do the in-file rename, show a warning listing affected files.

- **Backoff multiplier** (5.4, WEAKNESS): 3x multiplier with no cap (3s, 9s, 27s). Standard practice is 2x. If someone bumps `MAX_RESTARTS` to 5, delays reach 243 seconds. Add a cap.

- **No string type** (5.5, WEAKNESS): Reasonable for the heap-free guarantee, but the debug-label use case (human-readable state names at runtime) needs a documented workaround. Recommendation: emit a `const char* M_state_name(M_StateId_t id)` function. Zero-cost, zero-RAM.

- **LCA table ROM cost** (5.7, WEAKNESS): No alternative for constrained targets. A parent-pointer array gives O(N) ROM at the cost of O(depth) lookup time.

- **`EmittedFiles` is C/C++-specific** (5.6, NIT): Hardcoded `header`, `source`, `impl_header`, `conf_header` fields. Won't work for Rust, Python, or VHDL targets. A `Vec<EmittedFile>` with role tags would be more extensible.

- **`Motor_conf.hpp` magic constants** (5.8, NIT): `kQueueAssert`, `kCrtp`, `kNoStl` are used without enum definitions. Two implementers would define them differently.

---

## 6. Testability

The test suite structure is well-designed. The `.fsm.test` golden format, the conformance categories, the CI integration plan -- all solid foundations. The problem is coverage.

### Only 11% of diagnostic codes are tested (BLOCKER) — 6.1

Doc 10 defines roughly 54 diagnostic codes. Doc 24 has test cases for 6 of them. That's 11%. Doc 15 Section 11 normatively requires "one test per code." This is the single biggest gap in the test suite.

Untested categories include: all lexer errors (E0001-E0006), all type errors (E0200-E0208), all submachine errors (E0500-E0502), all parallel/fork errors (E0600, E0750), all 13 warning codes, all 6 hint codes, and the runtime safety limit (E0900).

### Four test cases expect the wrong diagnostic code (BLOCKER) — 6.2-6.5

These would fail immediately on a correct implementation:

| Test | Title | Expects | Should expect |
|---|---|---|---|
| PARSE-NEG-001 | "No initial transition" | E0020 (Duplicate machine name) | E0107 (No initial declaration) |
| PARSE-NEG-005 | "Multiple initial transitions" | E0024 (Duplicate extern name) | E0108 (Multiple initial declarations) |
| PARSE-NEG-008 | "Undeclared context field in guard" | E0102 (Unknown extern reference) | E0104 (Unknown context field reference) |
| PARSE-NEG-010 | "Type mismatch in assignment" | E0200 (Guard type mismatch) | E0201 (Assignment type mismatch) |

Looks like the codes were assigned from an older numbering scheme that no longer matches the catalog. Straightforward fix.

### Codegen tests cover 5 of 11 requirements (GAP) — 6.6

Doc 15 requires 11 codegen test items. Doc 24 has 7 tests but only covers 5 of them. Missing: table-driven strategy, composite state LCA exit/entry order, history store/restore, parallel both-region dispatch, queue overflow assert, ARM cross-compilation. Also: zero C++17 codegen tests exist anywhere.

### No semantic trace tests in the golden format (GAP) — 6.7

The `.fsm.test` format handles parsing, diagnostics, formatting, IR, and codegen golden comparisons. But it has no way to express behavioral traces (`init`, `dispatch`, `advance_clock` step sequences). Doc 15 defines these as YAML `.trace` files, but the golden format doesn't integrate them.

### Missing edge case tests for critical semantics (GAP) — 6.8

No tests for: deferred events in parallel regions, deep history with parallel sub-states, `after 0 ms` timer, simultaneous timer expirations, `raise` event ordering, or parallel state completion (all-regions-done semantics). These are exactly the semantic edge cases where the spec itself is ambiguous (see Dimension 3).

### Other test gaps

- **No coverage metrics** (6.9, WEAKNESS): No line coverage, branch coverage, or MC/DC requirements for the Rust crates. 76 total tests is a small denominator.
- **Formatter tests cover only 6 scenarios** (6.10, WEAKNESS): Missing deeply nested states, long identifiers, empty machines, parallel regions, pseudo-states, inline control flow, multi-machine files.
- **Simulator tests are fragile** (6.11, WEAKNESS): All 6 tests require a running WebSocket server. No unit tests exercise the `Interpreter` API directly. No test for `sim/replay`.

---

## 7. Forward Compatibility

The architecture is generally well-prepared for evolution. The IR schema versioning rules are correct semver-for-schemas. The TextMate grammar handles new keywords gracefully via scope inheritance. The codegen crates depend only on `fsm-ir`, making new targets addable without touching existing code.

A few gaps to close:

- **FSM-E0600 collision** (7.1, WEAKNESS): This code means "feature not enabled" in Doc 04 and "parallel region has no initial" in Doc 10. A v1.0 tool encountering a `submachine` keyword would show the wrong error message. Needs distinct codes.

- **No DSL deprecation strategy** (7.2, GAP): The `language fsm 2.0` header implies versions, but there's no policy for how syntax gets deprecated across versions, whether compilers must accept older versions, or what the MINOR/MAJOR version lifecycle means for backwards compatibility.

- **`fsm.toml` has no version field** (7.3, GAP): If a config key gets renamed, older tools silently ignore the new key and use defaults. An optional `version = "1"` key and a compatibility alias policy would prevent surprises.

- **LSP server doesn't advertise its version** (7.4, GAP): No `serverInfo.version` in the initialize response. Clients can't detect whether the server supports newer features. Configuration keys should be documented as append-only.

- **"Handle unknown kinds gracefully" is vague** (7.5, NIT): The IR schema says consumers must handle unknown node kinds gracefully, but doesn't say what that means. Should they skip? Emit a warning? Treat as opaque? Define it: don't crash, emit FSM-I0004, skip for processing but preserve in pass-through.

- **Suppressing unknown diagnostic codes gives no feedback** (7.6, NIT): A developer suppressing `FSM-W9999` (typo or future code) gets no indication it's unrecognized. A hint would help catch typos while staying forward-compatible.

---

## What to Do About It (Priority Order)

These are ordered by impact -- what would cause the most damage if left unfixed.

**1. Make Doc 10 the single source for diagnostic codes.**
Retire the conflicting table in Doc 02 Section 13. Go through every diagnostic code reference in Docs 04, 11, 12, 24, and 15 and verify each one against Doc 10. Add the three missing codes (W0604, E0110, H0006). Resolve the E0600 collision and the E0304/E0600 duplication. This one fix eliminates findings 1.1-1.8, 1.6, 1.7, 3.19, and 7.1.

**2. Fix the 4 broken test cases.**
NEG-001: E0020 -> E0107. NEG-005: E0024 -> E0108. NEG-008: E0102 -> E0104. NEG-010: E0200 -> E0201. Five-minute fix, prevents immediate test failures.

**3. Make Doc 13 authoritative for the simulator protocol.**
Replace the simulator sections in Doc 03 with cross-references to Doc 13. Standardize on slash notation. Define the trace record schema once.

**4. Fix switch-based codegen for hierarchical machines.**
Without parent-state transition lookup, composite states don't work. This is a design-level fix to the codegen spec, not a typo.

**5. Fix parallel state completion semantics.**
Add the "all regions must reach final" rule. Harmonize Docs 04 and 08.

**6. Allow guarded completion transitions.**
Remove the prohibition from Doc 08 Section 4.4. The grammar and requirements already allow them.

**7. Fix the C++17 STL profile.**
Replace `std::queue<Event>` with a fixed-capacity circular buffer. The heap-free guarantee is a core design goal.

**8. Fix the expression grammar.**
Either rewrite the EBNF to eliminate left recursion or mark it as informational with the Pratt table as normative.

**9. Write the missing negative tests.**
48+ diagnostic codes have no test case. Doc 15 requires one per code. This is the biggest testing gap.

**10. Complete the IR schema.**
Add `consts`, `features`, `imports`, `queue`, `targets`, `cast` expression, `float` literal, `enum_variant` literal, and a proper `kind` field on transitions.

**11. Reconcile keyword lists.**
Create one authoritative table in Doc 04. Have Docs 14, 19, and 21 reference it.

**12. Add a security section.**
Import path sanitization, opaque type validation, simulator auth, input limits.

**13. Create authoritative VS Code tables.**
One command table, one settings table. Docs 03, 05, and 22 reference them.

**14. Reconcile the formatter spec with the actual grammar.**
Doc 19 uses syntax that doesn't exist in Doc 04.

**15. Add the missing codegen and semantic tests.**
Table-driven strategy, composite states, history, parallel regions, timers, ARM cross-compilation, C++17 profiles.

---

### A note on the recurring problem

Most of these findings share a root cause: the same information lives in multiple documents and has drifted out of sync. Fixing individual inconsistencies one at a time is a game of whac-a-mole -- the next review will find new drift. The structural fix is:

1. For each concept (diagnostic codes, keywords, simulator protocol, VS Code commands, exit codes, configuration settings), pick ONE document as the source of truth.
2. In every other document that mentions that concept, replace the inline definition with a cross-reference.
3. When updating the concept, you update one place. The references stay valid automatically.

This is why I recommended the single-source-of-truth approach at the top. It's the difference between fixing symptoms and fixing the cause.
