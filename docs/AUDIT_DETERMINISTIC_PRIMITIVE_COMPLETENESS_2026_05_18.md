# AUDIT — Deterministic-Primitive Completeness (Factory-Control Timing/Reactive Matrix)

**Document ID:** FSM-AUDIT-DETPRIM-2026-05-18
**Status:** Frozen audit record (independent auditor/architect)
**Gate role:** The owner's 2026-05-18 authoritative-execution-sequence **item 4** —
the deterministic-primitive completeness audit that MUST complete **before** the
interactive debug-interface endpoint (item 5) is designed.
**Base:** worktree `embeded-fsm-sdk-wt-detprim-audit`, branch
`phase7.0/det-primitive-completeness-audit`, off `fbae38b` (post-factory-tag HEAD).
**Toolchain:** `rustup show active-toolchain` from the worktree → **1.75.0**
(repo-pinned via `rust-toolchain.toml`) — asserted, not assumed.
**Discipline:** READ-ONLY except this doc. Every matrix cell is **behaviourally
proven** (toolchain run on a real `.fsm`), never keyword-presence. No
fabrication of any owner-OWED context.

---

## 0. Scope, sources, and method

This audit answers, for the factory-control timing/reactive primitive set named
in the owner's 2026-05-18 language-roadmap directive
(`project_user_messages_embeded_fsm_2026_05_18_language_roadmap_directive`) and
the authoritative execution sequence
(`project_user_messages_embeded_fsm_2026_05_18_authoritative_execution_sequence`,
item 4): is each primitive **SUPPORTED** / **DEFERRED** / **GAP**, and which GAPs
are core-adjacent deterministic and should be pulled forward as scoped waves.

**Honest-gap caveat carried (the project policy applied to ourselves).** The
underlying language-primitives discussion + the probabilistic-deferral *rationale*
are explicitly **OWED-from-owner** (both directives say so verbatim). They are
**not in this session and are NOT fabricated here.** Where the directive's 5-item
distillation is explicit, it is acted on; where a rationale is owed, it is flagged
`OWED/TBD-from-owner`.

**Classification rule (strict).**
- **SUPPORTED** = spec'd in Doc 04 + grammar-tokenized + lowered + codegen'd +
  sim'd + **behaviourally demonstrated** (the `.fsm` + the observed behaviour).
- **DEFERRED** = explicitly spec-deferred / roadmap-tracked (cite Doc 02/04/07/08).
- **GAP** = factory-control-needed per the directive, **neither** supported **nor**
  explicitly deferred — the actionable finding. A *silent miscompile* (accepted,
  no diagnostic, wrong/empty output) is the **most severe** GAP sub-class and is
  called out as such.

**Evidence harness.** `fsm` CLI built `--release` from this worktree
(`/root/dev/embeded-fsm-sdk-target/release/fsm`, the documented shared
`CARGO_TARGET_DIR`); probes in `/tmp/dp-probe` (NOT in the repo). Every command +
its observed output is reproduced inline below.

**Code surface inspected (the "actually implemented" ground truth):**
`crates/fsm-lexer/src/{token.rs,lexer.rs}` (keyword set),
`crates/fsm-parser/src/grammar/{state.rs,stmt.rs,transition.rs}` (grammar),
`crates/fsm-ir/src/model.rs` (`enum Trigger`, primitive types),
`crates/fsm-analyzer/src/{checks/timer.rs,lower/state.rs}` (timer const-fold +
lowering), `crates/fsm-codegen-c` (emit), `crates/fsm-simulator/src/trace.rs`
(`StepRecord` — the signal record/export seam),
`crates/fsm-verify/src/digest.rs` (the v1.4 W2 clock-origin keystone).

---

## 1. The factory-control timing/reactive primitive matrix

| # | Primitive | Verdict | One-line basis |
|---|---|---|---|
| P1 | Runtime-variable timer **duration** (`after`/`every` with a runtime/context-computed deadline) | **DEFERRED** (explicit, multi-doc) — **with a silent-miscompile latent defect on the *spec-mandated `const`-reference* form, see §1.1 / Finding F-1** | `Trigger::After{duration_ms:u32}` / `Every{period_ms:u32}` — IR carries a *folded `u32`*, no expr; Doc 08 §13.5 pt 5 + Doc 15.1 Note + Doc 02 §6 explicitly post-v1.0 |
| P2 | **Watchdog** (deadline-since-last-kick supervisory timer) | **GAP** (not spec'd, not deferred, not in grammar) | `watchdog` is **not a lexer keyword**; parse error `FSM-E0010 unexpected 'watchdog' in state body` |
| P3 | **Output-signal modeling** (a first-class `signal`/`output`/`emit` primitive) | **GAP** for the *language primitive*; the **effect is SUPPORTED via the extern-action / `raise`-event idiom** (the spec's deliberate design) | `signal`/`output`/`emit` are **not keywords** (parse `FSM-E0010`); `extern set_led(...)` + `ctx.x=` + `raise EVT` all lower & codegen correctly |
| P3b | **Signal import / record / export** (directive item-2) | **REQUIREMENT — LOGGED, not built** (per directive item-2; not a "GAP" because explicitly logged as a roadmap requirement) — real reuse seam = `StepRecord` / `fsm baseline`, but the typed signal value-channel is **absent** today | `StepRecord` (`trace.rs`) records events/transitions/states/`actions_executed`/config — **no `signals`/`outputs` value field**; no `import {signal}` form |
| P4 | **Debounce** (time-filtered trigger) | **GAP** (not spec'd, not deferred, not in grammar) | `debounce` is **not a keyword**; parse error `FSM-E0010 expected '->','~>' or ':' after transition trigger` |
| P5 | **Edge-vs-level triggering** (`rising`/`falling`/`edge`/`level`) | **GAP** (not spec'd, not deferred, not in grammar) | `rising`/`falling`/`edge`/`level` are **not keywords**; parse error `FSM-E0010 ... after transition trigger` |
| P6 | **Bounded repetition** (`repeat N times`-class periodic-with-count) | **GAP** (not spec'd, not deferred, not in grammar) | `repeat`/`times` are **not keywords**; parse errors `FSM-E0010 expected '->'` / `unexpected 'times' in state body` |

> **§5.4 discipline applied.** Not one cell above is asserted from a keyword's
> presence. Each is decided by **running the toolchain** on a real `.fsm` (the
> exact command + output is in §1.1–§1.6). The lexer keyword scan
> (`crates/fsm-lexer/src/token.rs` L43–95; `lexer.rs` L661–705) independently
> corroborates: the timing/reactive vocabulary actually tokenized is exactly
> `after`, `every`, `on`, `done`, `defer`, `schedule`, `cancel`, `raise`,
> `send`, `priority`, `ms`, `initial`, `final` — and **none** of
> `watchdog`/`signal`/`output`/`emit`/`debounce`/`rising`/`falling`/`edge`/
> `level`/`repeat`/`times`.

### 1.1 P1 — Runtime-variable timer duration (the directive's first concern) — DEFERRED, **+ Finding F-1 (silent miscompile of the spec-mandated `const`-reference form)**

**The directive's precise question:** is the timer DURATION runtime/context-variable
or compile-time-constant only?

**Structural answer (IR — definitive).** `crates/fsm-ir/src/model.rs`
`pub enum Trigger`:
```
After  { duration_ms: u32, timer_id: String }
Every  { period_ms:   u32, timer_id: String }
```
The duration is a **folded `u32` scalar, not an expression node.** There is no
`Trigger::AfterExpr`, no `duration_field`, no runtime-duration form anywhere in
the IR. A factory deadline that must be runtime-computed (e.g. `after
ctx.deadline_ms ms`) **cannot be represented**.

**Spec answer (explicit, triple-anchored — this is DEFERRED, not an undocumented GAP).**
- **Doc 02 §6 (Timer Model):** "`N` MUST be a compile-time constant (integer or
  `const` reference)."
- **Doc 08 §13.5 proof-basis point 5 (normative):** "Timer durations are
  compile-time constants … there is no `at <absolute>` / deadline /
  runtime-variable-duration form. §15.1's Note forecloses the one construct that
  could make a duration time-dependent (**runtime-variable timer durations are
  explicitly post-v1.0**)."
- **Doc 08 §15.1 Note:** "Runtime-variable timer durations (`after ctx.green_ms
  ms`) are a post-v1.0 feature; v1.0 timers use compile-time constant values
  only."

⇒ **Runtime-variable timer duration = DEFERRED** (spec-explicit, post-v1.0,
load-bearing for the v1.4 verification soundness — see §3). This is the correct
classification: it is *explicitly* deferred in the normative spec, not an
unspoken hole.

**Behavioural proof (the SUPPORTED const-literal baseline):**
```
$ cat t_lit.fsm
language fsm 2.0
feature timers
machine M { events { E } initial A
  state A { after 5000 ms -> B }
  state B { on E -> A } }
$ fsm generate -t c99 -o gen_lit --emit-ir t_lit.fsm   # exit 0
# IR state A:
  trigger {kind:"after", duration_ms:5000, timer_id:"ps-timer-M-1"},
  timers  [{kind:"after", durationMs:5000, ownerStateId:"s-M-A", target:"s-M-B"}]
```
⇒ `after <integer-literal> ms` is fully **SUPPORTED** (lowered + codegen'd; this
is the matrix's positive control for the timer family).

**Behavioural proof (the runtime-variable form — DEFERRED-by-spec, and *silently
dropped*, no diagnostic):**
```
$ cat t_rtv_timer.fsm
language fsm 2.0
feature timers
machine M { context { deadline_ms : u32 = 5000 } events { E } initial A
  state A { after ctx.deadline_ms ms -> B }
  state B { on E -> A } }
$ fsm check t_rtv_timer.fsm           # exit 0
$ fsm check --json t_rtv_timer.fsm    # -> []   (ZERO diagnostics)
$ fsm generate -t c99 -o gen_rtv --emit-ir t_rtv_timer.fsm   # exit 0
# IR state A:  transitions = []   timers = []     <-- the after-edge VANISHED
```
The runtime-variable timer is **silently dropped** — `check` is clean, `generate`
succeeds, and State A is left with **no outgoing edge at all** (a dead state). No
`FSM-E06xx`/`FSM-E0410`/any diagnostic. **This is the most dangerous failure
mode** (a never-firing factory deadline that the toolchain reports as fine). It
is *acceptable* only because the construct is genuinely DEFERRED — but the
**absence of a rejecting diagnostic is itself a defect** (it should be a hard
`FSM-Exxxx` "runtime-variable timer durations are post-v1.0", mirroring the
defer→`FSM-E0903` precedent in Doc 02 §5.3 / §9.4).

> **FINDING F-1 — SILENT MISCOMPILE of the `const`-REFERENCE timer duration that
> Doc 02 §6 and Doc 04 §9 EXPLICITLY MANDATE AS SUPPORTED.** This is a *separate*
> latent codegen/analyzer defect (a #110-class reliability bug, NOT a
> primitive-completeness GAP) discovered behaviourally during this audit, and it
> must be surfaced (stage-gate / brutal-honesty):
>
> ```
> $ cat t_constref.fsm
> language fsm 2.0
> feature timers
> const T_MS = 5000
> machine M { events { E } initial A
>   state A { after T_MS ms -> B }    # const ref — Doc 02 §6 says MUST work
>   state B { on E -> A } }
> $ fsm check --json t_constref.fsm   # -> []   (ZERO diagnostics)
> $ fsm generate -t c99 -o gen_cref --emit-ir t_constref.fsm   # exit 0
> # IR state A:  transitions = []   timers = []   <-- DROPPED
> $ fsm verify t_constref.fsm
>   ⚠ FSM-W0602 unreachable state 'B' — no incoming transitions   # the symptom
> ```
> **Root cause (code-located).** `crates/fsm-analyzer/src/lower/state.rs:943`
> `eval_i64()` — the lowerer's timer-duration const-fold — handles **only**
> `EXPR_LITERAL` / `EXPR_UNARY` / `EXPR_PAREN`. It does **not** handle
> `EXPR_NAME_REF` (a `const` reference). When the fold returns `None`, the
> `After`/`Every` lowering is skipped (`a.duration().and_then(|ce|
> duration_ms(&ce))` at L723/L763/L803). **Asymmetry:** the *check*
> `crates/fsm-analyzer/src/checks/timer.rs:124` `resolve_expr_value()`
> **does** resolve `EXPR_NAME_REF` against file consts — so the duration-bounds
> check sees the const, but the **lowerer silently drops the whole timer**. The
> verifier then operates on a corrupted IR and (correctly, given what it sees)
> reports B unreachable. **Blast radius:** any `.fsm` using a named-constant
> timer duration — the *canonical, documented, Doc-12-example idiom* (Doc 04 §12
> uses `after CONNECT_TIMEOUT_MS ms`, `every HEARTBEAT_INTERVAL_MS ms`
> throughout). **Corpus blind spot:** every example/corpus FSM
> (`codegen_equivalence_smoke.rs` `CORPUS`, broadened 5→12 in FW110, which found
> 11 real bugs) uses **integer-literal** durations only — `grep` over `examples/`
> finds **no** `after <CONST> ms` / `after ctx.X ms` actual usage. So the
> existing differential corpus structurally **cannot** catch F-1. This is
> exactly the latent-defect class #110 ("hunt the rest of the latent-defect
> class") is chartered to find — **F-1 is a #110 input, recommended for the
> #110/Factory-W-class fix queue, not this audit's primitive GAP set.**

### 1.2 P2 — Watchdog — GAP

```
$ cat t_wd.fsm
language fsm 2.0
machine M { events { KICK } initial Run
  state Run { watchdog 1000 ms -> Fault on KICK }
  state Fault { } }
$ fsm check t_wd.fsm
  × FSM-E0010 unexpected 'watchdog' in state body: found 'identifier'
```
`watchdog` is not a keyword (lexer scan: absent), no grammar production, no IR
`Trigger`. **GAP** — and a high-value one for factory control (a watchdog =
"deadline since last KICK; on expiry → safe state" is a canonical PLC/embedded
supervisory pattern). **Not** spec-deferred anywhere (Doc 02 §6 / Doc 07 do not
mention it). See §2 — recommended pull-forward (it is core-adjacent: it is a
timer-family construct).

### 1.3 P3 / P3b — Output-signal modeling, incl. import/record/export

**P3 (the language primitive):** `signal` / `output` / `emit` are **not
keywords**:
```
$ fsm check t_sig.fsm   # 'signal led : bool' + 'emit led(true)'
  × FSM-E0010 unexpected 'signal' in machine body: found 'identifier'
$ fsm check t_out.fsm   # 'output relay : bool'
  × FSM-E0010 unexpected 'output' in machine body: found 'identifier'
```
**GAP** as a first-class primitive. **However**, the spec's *deliberate design*
(Doc 02 §0/§G3 "the only inline statements are pure FSM-semantic ops; all
computation lives in C") models an output **effect** via the extern-action or the
`raise`/`send` event idiom, and that path is **SUPPORTED** (positive controls):
```
$ cat t_sig_ok.fsm   # the SUPPORTED idiom
... extern set_led(bool v)
  state A { on E -> B : set_led(true); ctx.led_on = true }
  state B { entry : set_led(false) } }
$ fsm generate -t c99 -o gen_sigok t_sig_ok.fsm   # exit 0
# gen_sigok/M.c:226   set_led(true);
# gen_sigok/M_impl.h  void set_led(bool v);          <-- emitted correctly

$ cat t_evt_ok.fsm   # event-as-output
  state A { on E -> B : raise LED_ON }
  state B { on LED_ON -> A } }
$ fsm generate -t c99 -o gen_evt t_evt_ok.fsm   # exit 0 (3× LED_ON/raise in M.c)
```
⇒ **The output *effect* is SUPPORTED via the documented extern/`raise` idiom; a
*declarative `signal` primitive* is a GAP.** Whether to add a declarative
`signal` (vs. keep the "outputs are externs" contract) is a **product/language
decision the owner owns** (it touches Doc 02 §G3 "Declarative Purity") — flagged,
not decided here.

**P3b (signal import / record / export — directive item-2):** This is explicitly
a **LOGGED REQUIREMENT** in the directive (item-2: "Logged as a language-roadmap
requirement (scope/design TBD)"), so it is **not** a "GAP" in the actionable
sense — it is a recognised, recorded, unscheduled requirement. The directive's
own note ("record/export rhymes with the W1 trace-differential + `fsm baseline`
corpus machinery — a likely reuse seam") is **confirmed by inspection**:
- `crates/fsm-simulator/src/trace.rs` `StepRecord` already records, per step:
  `event_received`, `transition_taken`, `exited_states`, `entered_states`,
  **`actions_executed` (extern callee names)**, `config_before/after`,
  `submachine` detail — emitted as the frozen `fsm-trace/v1` baseline corpus by
  `fsm baseline --record`.
- **What is absent:** a typed **signal/output value channel** — there is no
  `signals` / `outputs` / `ctx_snapshot` field on `StepRecord`. So
  record/export of *signal values* would be an **append-only `StepRecord`
  extension** (the same additive discipline the `submachine` field used —
  `trace.rs` L74–82) consumed by `fsm baseline`. And there is no `import "x.fsm"
  { someSignal }` form (Doc 04 §2.1 import resolves events/enums/externs/machine
  refs only). **Conclusion: the reuse seam is real and the requirement is
  correctly logged; nothing is built; this is design-time work folding into the
  post-quality language roadmap, NOT a pull-forward (see §2).**

### 1.4 P4 — Debounce — GAP

```
$ cat t_db.fsm
... state A { on BTN debounce 50 ms -> B }
$ fsm check t_db.fsm
  × FSM-E0010 expected '->', '~>' or ':' after transition trigger, found 'identifier'
```
`debounce` not a keyword, no grammar, no IR. **GAP.** Not spec-deferred. (A
debounce is *time-filtered triggering* — semantically a timer+guard composite;
core-adjacent but with a real semantic-design question — see §2.)

### 1.5 P5 — Edge-vs-level triggering — GAP

```
$ cat t_edge.fsm
... state A { on rising SIG -> B }
      state B { on falling SIG -> A } }
$ fsm check t_edge.fsm
  × FSM-E0010 expected '->', '~>' or ':' after transition trigger, found 'identifier'
```
`rising`/`falling`/`edge`/`level` not keywords, no grammar, no IR. **GAP.** Not
spec-deferred. (FSM-Lang's event model is already inherently *edge-like* —
discrete enqueued events; *level*-sensitivity would be a new semantic mode. This
is more a modeling-guidance / design question than a pure deterministic-core
extension — see §2, classified larger/later.)

### 1.6 P6 — Bounded repetition — GAP

```
$ cat t_rep.fsm
... state A { every 100 ms repeat 5 times -> B }
$ fsm check t_rep.fsm
  × FSM-E0010 expected '->', found 'identifier'
$ cat t_rep2.fsm   # 'on E -> B times 3'
$ fsm check t_rep2.fsm
  × FSM-E0010 unexpected 'times' in state body: found 'identifier'
```
`repeat`/`times` not keywords, no grammar, no IR. **GAP.** Not spec-deferred.
(Bounded `every`-with-count is **highly core-adjacent**: it is `Trigger::Every` +
a counter + an auto-`done`; modelable today only with an explicit `ctx` counter +
a `[ctx.n >= 5]` guard. See §2 — recommended pull-forward candidate.)

---

## 2. Core-adjacent deterministic-gap pull-forward (RECOMMEND + SCOPE only — NOT implemented)

Of the GAPs, which are core-adjacent + deterministic + factory-critical (close to
the existing timer/`Trigger`/codegen core) → recommend as scoped future waves;
which are larger/later. **None implemented; recommendation + scope only.**

### 2.1 PULL FORWARD — **Watchdog** (P2) — core-adjacent, deterministic, factory-critical

**Scope.** A supervisory one-shot timer that is *reset by a nominated event*
rather than only by state exit: e.g. `watchdog WD = 1000 ms reset_on KICK -> Safe`.
Semantically = a `Trigger::After` whose arming is *also re-triggered* on each
occurrence of `KICK` while the owning state is active. Fully deterministic
(virtual-clock-driven, identical to existing `after`/`every` timer semantics —
no clock *read*, so it does **not** disturb §3).
**Touch-set.** Doc 04 (a `watchdog_decl` EBNF in §9 + the `watchdog`,
`reset_on` keywords appended to §1.5 — append-only minor-version event) ·
`fsm-lexer` (2 keywords) · `fsm-parser/grammar/state.rs` (a `WATCHDOG_DECL`,
modelled on `parse_after_decl_at`) · `fsm-ir` (a `Trigger::Watchdog { duration_ms:
u32, reset_event_id, timer_id }` — strictly additive to the enum) ·
`fsm-analyzer/lower/state.rs` (reuse the *fixed* `duration_ms` fold) ·
`fsm-codegen-c` (one armed-on-entry/disarmed-on-exit pair **plus** a re-arm on
the reset event — a small extension of the existing per-timer emit) ·
`fsm-simulator` (re-arm on the reset event in the timer model) ·
`fsm-verify` (digest already keys timers by *remaining duration* — a watchdog is
just another armed timer; **no soundness change** — confirm by a digest
golden-test).
**§5.4 acceptance shape.** A `watchdog`+`reset_on` fixture added to the
`codegen_equivalence_smoke.rs` `CORPUS`: compile generated C with `gcc -Werror`,
drive a trace where KICK arrives before expiry (stays in Run) and one where it
does not (→ Safe), assert `sim ≡ codegen` byte-equal trace; a `fsm verify`
golden asserting the watchdog-deadlock case is detected (not `Inconclusive`).

### 2.2 PULL FORWARD — **Bounded repetition** (P6) — core-adjacent, deterministic

**Scope.** `every N ms times K -> T` (and the internal `: a` form): fire the
periodic timer at most `K` times, then auto-`done`/transition. Deterministic;
expressible today only with a hand-rolled `ctx` counter + guard (error-prone,
verbose) — a count is exactly the kind of *structure* the DSL should own.
**Touch-set.** Doc 04 §9 (`[ "times" , const_expr ]` suffix on
`every_decl`/`every_internal_decl` + `times`/`repeat` keyword(s) appended to
§1.5) · `fsm-lexer` (1 keyword) · `fsm-parser/grammar/state.rs` (extend
`parse_every_decl_at` with an optional `times` clause) · `fsm-ir`
(`Trigger::Every` gains an additive `Option<u32> max_count`, default `None` =
unbounded — byte-compatible with existing IR) · `fsm-analyzer`
(count = the *same fixed* const-fold path) · `fsm-codegen-c` (a per-timer
counter + stop-at-K — small, local) · `fsm-simulator` (count the fires) ·
`fsm-verify` (a *bounded* periodic timer **improves** termination — it removes an
infinite-`every` cycle; digest unaffected).
**§5.4 acceptance shape.** A `stress-bounded-every` fixture in `CORPUS`: assert
the trace fires exactly K times then transitions, `gcc -Werror`, `sim ≡ codegen`;
a `fsm verify` golden that the bounded form *terminates* (vs. the unbounded
`every` which the §3 clock-merge keeps from non-terminating).

### 2.3 PULL FORWARD — **The rejecting diagnostic for runtime-variable / non-const-foldable timer durations** (the §1.1 silent-drop) — *deterministic correctness, not a new primitive*

**Scope.** This is **not a new primitive** — it is closing the *silent* failure
mode of an already-DEFERRED construct (and is the natural sibling of F-1's fix).
When a timer `const_expr` does not fold to a positive integer constant (a
`ctx`/`payload` ref, a non-const-foldable expr), the analyzer MUST emit a hard
diagnostic (proposed `FSM-E0411` "timer duration must be a compile-time
constant; runtime-variable durations are post-v1.0", mirroring the
defer→`FSM-E0903` precedent) **instead of** silently dropping the timer.
**Touch-set.** `fsm-analyzer/checks/timer.rs` (emit on the `resolve_const_expr →
None` branch that today `return`s silently) · Doc 10 (allocate the code) ·
Doc 04 §9 / Doc 02 §6 (state the diagnostic). **No** grammar/IR/codegen/verify
change.
**§5.4 acceptance shape.** `fsm check --json` on `after ctx.x ms` /
`after non_const_expr ms` asserts exactly the new code at the right span; the
const-literal and (post-F-1-fix) const-ref forms stay clean.
**Why pull forward:** a silent miscompile (§1.1) is the cardinal toolchain sin;
this is tiny, local, and removes a sharp edge directly in the deterministic
timer core — and it is *load-bearing context for the debug interface* (a debug
session must not silently lose a timer; see §6).

### 2.4 LARGER / LATER (do **not** pull forward)

- **Output-signal `signal` primitive + signal import/record/export (P3/P3b).**
  Touches Doc 02 §G3 "Declarative Purity" (is an output a *declared signal* or an
  *extern effect*? — a **language-philosophy / product decision the owner owns**),
  plus a typed `StepRecord` value-channel extension + an `import { signal }`
  form. It is a *cohesive language-roadmap feature* with a real design phase, and
  the directive **already logs it as item-2 (requirement, scope TBD)** and item-5
  adjacency. Correctly belongs in the **post-quality language roadmap**, not a
  near-term deterministic-core pull-forward. (The *effect* is already supported
  via externs/`raise` — there is no factory-control capability *gap*, only an
  ergonomics/first-classness gap.)
- **Debounce (P4)** and **edge/level triggering (P5).** These introduce a *new
  triggering semantics* (time-filtered; level-sensitive) rather than extending
  the existing timer/event core. FSM-Lang's model is deliberately
  discrete-event/edge-like (Doc 02 §5); level-sensitivity in particular is a
  semantic-model change with verification implications. Real features, but
  **larger/later**, design-led, post the deterministic-core + the §2.1–2.3 pull-
  forwards. Debounce *can* be modeled today (a `state` + an `after` + a re-arming
  guard); the ergonomic gap does not block factory control.

---

## 3. Clock-read-in-guards verification-tension scoping (binding, architecture-grounded — SCOPE ONLY, do NOT implement)

**Establishing the tension from Doc 08 §13.5 + `fsm-verify/src/digest.rs`
(verified in this audit).**

The v1.4-W2 keystone is `crates/fsm-verify/src/digest.rs`
`normalize_clock_origin_sub` (L112) / `canonical_bytes` (L147): both set
`virtual_clock_ms = 0` and rewrite every armed timer's absolute `expiry_ms` to
its **remaining duration** (`expiry_ms.saturating_sub(origin)`), recursively per
submachine sub-instance. Its soundness rests **entirely** on Doc 08 §13.5's
*Absolute-Virtual-Clock Non-Observability* lemma: because **no FSM-Lang
construct** (guard, action arg, `pure extern` param, timer duration) can read the
absolute virtual clock, two configs related by a uniform clock-origin shift are
**strongly bisimilar**, so merging them into one visited-set key can **never hide
a reachable deadlock** (sound in the cardinal direction: never a false
`ProvenNoDeadlock`), while the merge is *required* for termination on
cyclic-timer / `every`-heartbeat machines (without it the absolute clock advances
forever, the digest never repeats, and a genuine timer-deadlock is wrongly
`Inconclusive`).

**Behaviourally corroborated in this audit:** there is **no clock builtin
today** — `[clk > 1000]` parses `clk` as a (here undeclared) identifier in
`field_ref` position, not a clock primary; `ctx.now_ms` is an inert user field
the runtime **never** populates with `virtual_clock_ms`. This empirically
confirms §13.5 points 1–6: no surface syntax reads the absolute clock. A future
`[clk > X]` clock-read would therefore be a **brand-new construct**.

**The tension (rigorous).** A `[clk > X]`-style clock-read-in-a-guard makes the
absolute virtual clock **observable**. Then:
1. Two clock-origin-shifted configs are **no longer bisimilar** (one satisfies
   `clk > X`, the other does not, at the "same" relative phase) — §13.5's
   lemma's hypothesis is broken at its root (point 6: the RTC step would now read
   the absolute clock in guard evaluation, not only at the §13.4 relative
   `M_tick` delta).
2. The digest's clock-origin merge would then collapse configs whose futures
   **differ**, so it could return `ProvenNoDeadlock` while a deadlock is in fact
   reachable — **the cardinal verification sin (a false proof).**
3. **Conversely, removing the merge to restore soundness re-introduces the
   v1.4-W2 non-termination**: cyclic-timer / `every`-heartbeat machines advance
   the absolute clock forever, the digest never repeats, and a genuine
   timer-deadlock is wrongly reported `Inconclusive`. So the merge cannot simply
   be dropped.

⇒ A clock-read-in-guards feature **cannot be bolted onto the shipped verifier**;
it MUST be **co-designed with the verification core** (this is the directive's
binding, architecture-grounded item-4).

**Viable co-design directions (trade-offs) — for a future verification-extension
wave; NOT decided here:**

- **(a) Honest-bound: verifier returns `out-of-scope/INCONCLUSIVE` for any
  clock-guard machine.** Detect any clock-read construct in the model;
  `fsm verify` reports `Inconclusive (clock-guard: out of verification scope)`
  and **never** a `ProvenNoDeadlock` for it. *Pro:* trivially sound (the
  honest-bound discipline already used elsewhere); zero risk to the v1.4
  keystone for non-clock-guard machines (the overwhelming majority). *Con:*
  zero verification value *for* clock-guard machines (they get no deadlock
  proof). *Lowest risk, lowest reward; the safe default.*
- **(b) Restrict clock-guards to a bisimilarity-preserving form.** Permit only a
  shape that is invariant under clock-origin shift (e.g. *relative*
  "time-in-state ≥ K" sugar that lowers to an internal `after K ms`-armed
  boolean — semantically a timer, not an absolute-clock read). *Pro:* keeps the
  v1.4 merge sound *as-is* (the construct never observes the *absolute* clock —
  it is a relative timer in disguise); high practical value (covers most
  "timeout-ish guard" needs). *Con:* it is **not** general clock-reads — it is a
  carefully-restricted timer sugar; needs a soundness proof that the restricted
  form preserves the §13.5 bisimulation. *Best value/risk if the use cases are
  actually "relative timeout", which most factory ones are.*
- **(c) Sound region/zone abstraction (timed-automata class) replacing the
  origin-merge for clock-guard machines.** A proper timed-automata zone/DBM
  abstraction for the clock-guard sub-class (the standard decidable theory).
  *Pro:* fully general — real verification of genuine clock-guard machines.
  *Con:* a *major* verification-core extension (a second abstraction domain
  alongside the explicit-state engine), large design+proof+impl cost, and it
  must be reconciled with the existing digest so non-clock-guard machines keep
  the cheap proven path. *Highest reward, by far the highest cost; a research-
  grade wave.*

**Binding constraint to record (RECOMMEND the edit — this audit does NOT edit
Doc 08 / the backlog):**
> Any future clock-read-in-guards (or absolute-time/`deadline`/runtime-variable-
> timer-duration that exposes absolute time) feature is **gated** on being
> co-designed with the verification core via one of directions (a)/(b)/(c); it
> may **never** be shipped in a form that lets the v1.4
> `digest.rs` clock-origin merge return a false `ProvenNoDeadlock`. Recommended
> homes for this binding record: **Doc 08 §13.5's neighbourhood** (a "Scoping
> constraint" subsection adjacent to the lemma — the durable spec-layer home),
> **Doc 00 §11** (next free entry, currently §11.87+), and the **project
> backlog / `docs/ROADMAP.md`** under the post-quality verification-extension
> menu, so it can never be silently violated. *This recommendation is recorded
> here only; the actual Doc 08 / Doc 00 / ROADMAP edits are deferred to the
> owner / a future closeout, per the read-only guardrail.*

> **Note — this also binds P1 (runtime-variable timer duration).** §13.5
> proof-basis point 5 explicitly lists "timer durations are compile-time
> constants" as one of the six pillars of the non-observability lemma. A
> *runtime-variable* duration that is itself a function of elapsed/absolute time
> would equally threaten the merge. So the §2.1/§2.2 pull-forwards (watchdog,
> bounded `every`) are deliberately scoped to keep durations **compile-time
> constant** (they reuse the *fixed* const-fold) — they are safe by
> construction; a *runtime-variable duration* remains in this §3 co-design
> envelope, not a near-term pull-forward.

---

## 4. Probabilistic layer — DEFERRED-empirical (observe/record/export, NOT pass/fail gating)

**Disposition (recorded, per directive item-3 + execution-sequence item 4):** the
probabilistic layer is **DEFERRED** — *not* a formal probabilistic-verification
model; deferred to an **empirical observe → record → export** model, and it
**stays deferred** (it does **not** become a pass/fail verification gate). This
is spec-consistent: **Doc 02 §18 "Explicitly Out of Scope"** lists
"Probabilistic / stochastic state machines" and "Formal model checking
integration (future plugin)" / "Non-deterministic automata" — so the deferral is
anchored in the normative spec, not invented here.

**The "why" is OWED-from-owner — explicitly NOT fabricated.** Both 2026-05-18
directive memories state, verbatim, that the probabilistic-deferral **rationale
was NOT supplied** and is **TBD-from-owner** ("the owner explicitly wants it
recorded with the rationale; the rationale was not supplied; NOT fabricated").
Per the honest-gap discipline applied to ourselves:

> **`OWED / TBD-from-owner`** — the owner's stated *reason* for deferring the
> probabilistic layer to empirical observe/record/export. The directive memory
> offers a *plausible-but-unconfirmed* parenthetical ("formal probabilistic
> verification tensions with the deterministic-core soundness") and **explicitly
> instructs that this parenthetical is NOT to be treated as the owner's stated
> reason**. It is therefore recorded here **only** as a flagged-open item, **not
> fabricated** into a rationale. Capture verbatim when the owner provides it
> (alongside the owed verbatim language-primitives discussion); it does not
> block this audit or the sequence.

---

## 5. Auto-synthesis-from-stimulus — LOGGED deferred product-vision note

Per directive item-5: auto-synthesising an FSM from observed stimulus/traces is a
**logged, deferred product-vision item** — **not a gap, not scheduled, not a
wave**. Recorded here as a vision note with its architectural adjacency
(confirmed in this audit): the **W1/W3 trace-differential + `fsm baseline`
corpus machinery** (`fsm-simulator/src/trace.rs` `StepRecord` → frozen
`fsm-trace/v1` corpus, driven by the shipped interpreter as the single oracle) is
the natural substrate a future auto-synthesis capability would *consume in
reverse* (traces → inferred model), and it shares the **same seam as the item-2
signal record/export** (§1.3 P3b). A vision item to revisit at the post-quality
language-roadmap horizon; logged, not actioned.

---

## 6. VERDICT (the gate)

### **PULL-FORWARD-FIRST** — for a *narrow, surgically-scoped* set; **then DEBUG-LAYER-MAY-PROCEED**

The deterministic primitive set is **substantially complete** for the interactive
debug interface to be designed on the **current frozen primitive set**, *with one
binding precondition*. Rigorous justification (no rubber-stamp):

**Why most GAPs are NOT load-bearing for the debug interface (DEBUG-LAYER-MAY-
PROCEED for these).** The debug interface (execution-sequence item 5) is a
*WebSocket-simulator-class layer reusing the existing simulator engine as the
single oracle*. Its job is to step/inspect/replay **whatever the frozen primitive
set already expresses**. Watchdog (P2), bounded repetition (P6), debounce (P4),
edge/level (P5), and a declarative `signal` primitive (P3) are **additive future
language features** — their absence does **not** make the debug interface
unsound, incorrect, or undesignable: the debug layer steps the simulator, and the
simulator faithfully executes the (rich, shipped) event/timer/HSM/parallel/
history/submachine core. Output *effects* are already observable in traces
(`actions_executed` — §1.3). Signal record/export (P3b) and auto-synthesis (§5)
are *logged roadmap/vision* items, explicitly not in scope now. The
clock-read-in-guards tension (§3) concerns a **non-existent future construct**;
it does **not** affect a debug layer over the *current* clock-non-observable
primitive set (the v1.4 keystone is sound for everything the debugger will step).
**No core-adjacent deterministic GAP is load-bearing for the debug interface.**

**The one binding PULL-FORWARD-FIRST precondition (the §1.1 / §2.3 silent
miscompile).** A debug/simulator layer whose entire value is *trust* ("step the
machine, see the truth") **cannot** be built on a toolchain that **silently drops
a timer** with zero diagnostics — `after ctx.x ms` (DEFERRED) **and**, far worse,
the *spec-mandated* `after CONST ms` (Finding F-1) both vanish from the IR with a
clean `fsm check`. A debugger would faithfully show a *silently corrupted model*
(the timer the user wrote simply absent — exactly the §1.1 `fsm verify`
"unreachable state B" symptom, now hidden behind a step UI). This is the **only**
deterministic-core defect that is genuinely *load-bearing for the debug
interface's correctness*, and it MUST be landed first:

1. **Fix F-1** (the highest-severity, since it breaks the *documented canonical
   idiom*): `fsm-analyzer/src/lower/state.rs:943` `eval_i64` must resolve
   `EXPR_NAME_REF` against file consts (the *check* at `checks/timer.rs:124`
   already does — close the lowerer/check asymmetry). Scope: a #110 /
   Factory-W-class reliability fix (it is a latent codegen defect, the exact
   class #110 is chartered to hunt; the FW110 5→12 corpus structurally cannot
   catch it — §1.1).
2. **Land §2.3** (the rejecting diagnostic): non-const-foldable timer durations
   MUST hard-error, never silently drop. Tiny, local, removes the cardinal
   silent-miscompile edge.

These two are small, surgical, and squarely in the deterministic timer core —
not new primitives. **The watchdog (§2.1) and bounded-repetition (§2.2) pull-
forwards are RECOMMENDED for the post-debug language roadmap** (high factory
value, core-adjacent, cleanly scoped) but are **NOT** debug-layer blockers (the
debugger does not need them to faithfully step the current core).

**Net gate decision:**
- **BLOCKING the debug layer:** F-1 fix + §2.3 rejecting diagnostic (the silent-
  timer-drop class). Land + verify (§5.4 acceptance) **before** the debug
  interface is designed.
- **NOT blocking (DEBUG-LAYER-MAY-PROCEED once the above lands):** every other
  GAP/requirement/vision item in this audit — watchdog, bounded repetition,
  debounce, edge/level, declarative `signal`, signal import/record/export,
  probabilistic, auto-synthesis. All are additive future roadmap/vision work;
  none is load-bearing for a faithful debug-over-the-frozen-set layer.
- **§3 clock-read-in-guards:** record the binding constraint (recommended Doc 08
  §13.5 + Doc 00 §11 + backlog edits) so it is never silently violated; it is
  **not** a debug-layer blocker (no such construct exists in the frozen set).

This is a deliberate, justified `PULL-FORWARD-FIRST` on a **two-item silent-
miscompile class only**, *not* a blanket block and *not* a rubber-stamp
DEBUG-MAY-PROCEED — the gate is set exactly at "the debugger must not be built
over a toolchain that silently loses model elements," which is the minimum bar
for an inspect/replay layer to be trustworthy.

---

## 7. Recommended records (RECOMMEND only — this audit edits no other doc)

- **Doc 08 §13.5 neighbourhood:** add a "Scoping constraint — clock-read-in-
  guards / absolute-time exposure" subsection adjacent to the lemma (the §3
  binding constraint + the (a)/(b)/(c) directions). *Recommended; not done here.*
- **Doc 00 §11 (next free entry, §11.87+):** record the §3 binding constraint,
  the P1 runtime-variable-timer DEFERRED status, and this audit's gate verdict.
- **`docs/ROADMAP.md`** (post-quality language-roadmap menu): the §2.1/§2.2
  pull-forward candidates (watchdog, bounded repetition); the §2.4
  larger/later (signal primitive + import/record/export, debounce, edge/level);
  the §3 verification-extension envelope; the §4 probabilistic-deferred (with the
  flagged `OWED` rationale); the §5 auto-synthesis vision note.
- **`docs/processes/TEST_DEBT.md` and the #110 / Factory-W fix queue:**
  **Finding F-1** (silent miscompile of `const`-referenced timer durations —
  `lower/state.rs:943` vs `checks/timer.rs:124` asymmetry) + the §2.3 rejecting
  diagnostic, with the corpus-blind-spot note (FW110's 5→12 literal-only corpus
  cannot catch it). *F-1 is a #110-class input, surfaced here per the stage-gate
  / brutal-honesty mandate; not fixed in this read-only audit.*
- **Carried OWED (owner's, non-blocking):** the verbatim language-primitives
  discussion + the probabilistic-deferral rationale (§4) — capture verbatim when
  the owner provides; recorded here as `OWED/TBD-from-owner`, **not fabricated**.

---

## 8. Confirmations (auditor attestations)

- **Read-only except this audit doc.** No primitive, no clock-scoping, no Doc 08 /
  Doc 04 / backlog edit was implemented. The only file written/committed on
  `phase7.0/det-primitive-completeness-audit` is this document.
- **No `git stash`** was used at any point (the shared-stack contamination
  hazard). Base↔HEAD inspection used `git diff`/`git show`/`grep` only.
- **Did NOT fabricate the OWED context** — neither the verbatim
  language-primitives discussion nor the probabilistic-deferral "why." Both are
  explicitly flagged `OWED/TBD-from-owner` (§4); the directive's
  plausible-but-unconfirmed parenthetical is explicitly **not** treated as the
  owner's reason.
- **Every matrix cell is behaviourally proven** by running the `fsm` toolchain on
  a real `.fsm` (commands + outputs reproduced in §1.1–§1.6), never from a
  keyword's mere presence; the lexer-keyword scan is used only as *corroboration*
  of the behavioural result.
- **Toolchain asserted from the worktree:** `rustup show active-toolchain` →
  `1.75.0-x86_64-unknown-linux-gnu (overridden by …/rust-toolchain.toml)`.
- **Judgment calls disclosed:** (i) P1 = DEFERRED (spec-explicit in Doc 02 §6 +
  Doc 08 §13.5 pt5 + §15.1) rather than GAP — because it is *explicitly* deferred
  normatively; the *absence of a rejecting diagnostic* is the defect, not the
  deferral. (ii) P3 = GAP for the *primitive* but the output *effect* is
  SUPPORTED via the spec's deliberate extern/`raise` design — so there is no
  factory-control *capability* gap, only first-classness; the "add a declarative
  `signal`?" call is flagged as **owner-owned** (Doc 02 §G3 Declarative Purity).
  (iii) P3b = LOGGED-REQUIREMENT (per directive item-2) not "GAP." (iv) F-1
  classified as a **#110-class reliability defect, not a primitive-completeness
  GAP** (it is a latent codegen bug in an already-spec-SUPPORTED feature) — but
  surfaced as load-bearing for the verdict because a debugger over a
  silently-dropping toolchain is untrustworthy. (v) Verdict = a *narrow*
  PULL-FORWARD-FIRST (the two-item silent-miscompile class only) **then**
  DEBUG-MAY-PROCEED — rigorously justified in §6, deliberately not rubber-
  stamped to reach the endpoint faster, and deliberately not a blanket block
  (no core-adjacent deterministic GAP is load-bearing for a faithful
  debug-over-the-frozen-set layer; the *silent-drop* class is the single real
  trust-blocker).

---

*End of FSM-AUDIT-DETPRIM-2026-05-18.*
