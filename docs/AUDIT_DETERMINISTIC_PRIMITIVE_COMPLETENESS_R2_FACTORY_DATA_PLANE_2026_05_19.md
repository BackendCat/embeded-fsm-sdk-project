# AUDIT R2 — Deterministic-Primitive Completeness: Factory Data-Plane

**Document ID:** FSM-AUDIT-DETPRIM-R2-2026-05-19
**Status:** Frozen audit record (independent auditor/architect; implementer of this round).
**Round:** 2. **Extends** the frozen `docs/AUDIT_DETERMINISTIC_PRIMITIVE_COMPLETENESS_2026_05_18.md` (R1). Reconciles and *extends* R1; does **not** contradict a frozen R1 cell without explicit cited counter-evidence (one R1 cell is updated — P-defer — with §A behavioural counter-evidence; disclosed in §7).
**Gate role:** The owner's Round-2 factory-data-plane completeness audit. Core-adjacent / kernel-touching. **GATES the debug-interface implementer waves** (the debug *build* is blocked on this gate; see §GATE).
**Base:** worktree `embeded-fsm-sdk-wt-detprim2`, branch `phase8.0/detprim-r2-factory-data-plane`, off main `caf49b2`.
**Toolchain:** `rustup show active-toolchain` **from the worktree** → `1.75.0-x86_64-unknown-linux-gnu (overridden by …/rust-toolchain.toml)` — asserted, not assumed (bare `rustc`→1.95 box-default is benign and irrelevant; the workspace builds under the repo-pinned 1.75).
**Discipline:** READ-ONLY on code (`crates/`/`Cargo*`/`.github/` untouched); doc deliverable only. Every verdict is **behaviourally proven** by running the `fsm` CLI (`--release`, built from this worktree at `caf49b2`, shared `CARGO_TARGET_DIR=/root/dev/embeded-fsm-sdk-target`) on scratch `.fsm` in `/tmp/dp2-probe` (NEVER in the repo) **and** traced lexer→parser→IR→analyzer→codegen→simulator→verifier with `file:line` citations. No `git stash`. No fabrication. Nothing unverifiable is asserted; the few open questions are flagged, not invented.

---

## 0. Method, classification, and what R1 already settled

This round answers the owner's three **factory-data-plane** directives — (1) state-gated request handling, (2) request/response framing of external signals, (3) a bounded data-plane primitive (typed queue/stack/FIFO) — and folds them, with the R1 candidate set (runtime-variable timers, watchdog, output-signal modeling incl. signal import/record/export, debounce/edge-level, bounded repetition), into **one unified verified-supported / expressible-today / deferred / gap matrix** with per-row determinism & verification cost.

**Three verdicts (strict):**
- **SUPPORTED** = spec'd (Doc 04) + tokenized + lowered + codegen'd + sim'd + **behaviourally demonstrated** (the exact `.fsm` + observed toolchain output cited).
- **EXPRESSIBLE-TODAY** = achievable by composing existing primitives; the exact composition is cited **and the ergonomic/semantic cost is stated honestly**. (Not a gap — but not first-class.)
- **GENUINE GAP** = factory-needed, neither supported nor expressible nor explicitly deferred. A *silent miscompile* (accepted, no diagnostic, wrong/empty output) is the most severe sub-class and is called out as such.
- (Auxiliary, R1-consistent: **DEFERRED** = explicitly spec-deferred/roadmap-tracked; **LOGGED-REQUIREMENT** = directive-logged, unscheduled.)

**R1 carried forward unchanged** (re-verified by code-trace at `caf49b2`, no contradicting evidence found): P1 runtime-variable timer duration = **DEFERRED** (Doc 02 §6 + Doc 08 §13.5 pt 5 + §15.1) + **Finding F-1** silent-miscompile of the `const`-reference timer form (`fsm-analyzer/src/lower/state.rs` `eval_i64` vs `checks/timer.rs` asymmetry — a #110-class defect, not a primitive gap); P2 watchdog = **GAP** (core-adjacent pull-forward); P3 output-signal primitive = **GAP**, effect **SUPPORTED** via extern/`raise`; P3b signal import/record/export = **LOGGED-REQUIREMENT** (`StepRecord` reuse seam, no typed value channel); P4 debounce / P5 edge-level = **GAP** (larger/later, semantic-model change); P6 bounded repetition = **GAP** (core-adjacent pull-forward); §3 clock-read-in-guards = scoped **with** the verification core (Doc 08 §13.5), **not** re-derived here. This R2 does not weaken any of those. **One R1 cell is updated with counter-evidence** (the `defer` end-to-end status — see §A and §7-(i)).

---

## A. THE EXISTING-CONSTRUCT FINDING (`queue_block` / `queue` keyword / `defer`/`deferred`) — proven, not assumed

This was flagged as the highest-value investigation. **Established end-to-end, both directions.**

### A.1 `queue` keyword / `queue_block` — what it actually IS

- **Lexer:** `queue` is **NOT a reserved keyword.** `crates/fsm-lexer/src/token.rs` enumerates exactly the 53 §1.5 keywords (`TokenKind` L42–95, `keyword_kind` map L388–445) — there is **no `KwQueue`** and **no `"queue"` arm**; `grep -i queue` over `token.rs`+`lexer.rs` returns **nothing**. This is *correct*, not a gap: Doc 04 §1.5 L96–105 explicitly lists `queue` as a **contextual keyword** ("keyword only in a specific syntactic position, ordinary identifier elsewhere; deliberately absent from the reserved list") alongside `entry`/`exit`/`events`. So it lexes as `Ident` and is recognised positionally by the parser.
- **Parser:** `queue_block` **IS implemented.** `crates/fsm-parser/src/grammar/machine.rs:89` dispatches `"queue" => parse_queue_block`; `:208` builds a `QUEUE_BLOCK` CST node = a generic `config_body` of `CONFIG_ENTRY` (`identifier "=" (int|ident|bool)`) pairs (`:224–245`). AST: `crates/fsm-parser/src/ast/top_level.rs:325` `QueueBlock::entries()`.
- **IR:** `crates/fsm-ir/src/model.rs:949` `QueueConfig { capacity: u32, overflow_policy: OverflowPolicy {Assert|DropOldest|DropNewest|Error}, loc }`, default `{16, Assert}` (`:958`).
- **Semantics — DECISIVE:** the `queue {}` block is the **bounded *event-mailbox* configuration** (the machine's external-event ring buffer), **NOT a user-visible typed data structure.** Doc 08 §3.2/§14 (queue drain policy) + Doc 04 §4.3. Codegen emits a fixed-capacity ring: `M_QUEUE_CAPACITY`, `m->_queue[]`, masked head/tail, `M_QUEUE_OVERFLOW` guard (`gen_queue/M.c:180–205`; `crates/fsm-codegen-c/src/budget.rs:69` `queue_bytes = capacity * sizeof_event`). It carries **events**, not typed user payloads pushed/drained under explicit state control. It is therefore **relevant to item 3 only as the *bounding pattern* to imitate, not as the data-plane primitive itself.**

> **NEW FINDING F-2 (silent misconfiguration — #110-class; not in R1).** The `queue {}` block is **parsed but its config entries are not honoured end-to-end.** Behavioural proof (`/tmp/dp2-probe`):
> ```
> machine M { events{GO} queue { capacity = 64  overflow = drop_newest } initial A  state A { on GO -> A } }
> $ fsm check --json  →  []                       (zero diagnostics)
> $ fsm generate -t c99 --emit-ir
>   IR  M.queue = { "capacity": 16, "overflowPolicy": "assert", loc:{47..106} }   ← DEFAULT, not 64/drop_newest
>   C   M_QUEUE_CAPACITY 8u ;  M_QUEUE_OVERFLOW FSM_QUEUE_ASSERT                   ← codegen DEFAULT, not even the IR's 16
> ```
> Two independent breaks: **(a)** the lowerer (`crates/fsm-analyzer/src/lower/machine.rs:363` `lower_queue` — logic itself is correct, reads `capacity`/`overflow` entries) receives an empty `qb.entries()` even though `machine.queue()` returns `Some` (the `loc` span 47–106 *is* captured), so the IR keeps the `QueueConfig::default()` `{16, Assert}` — the parsed `CONFIG_ENTRY` children are not surfaced by the `AstChildren<ConfigEntry>` accessor / not threaded; **(b)** even with a correct IR, codegen never consumes it: `cfg.queue_capacity` is set **only** from the `--queue-size` CLI flag or TOML `[generate].queue_size` (`crates/fsm-cli/src/cmd/generate.rs:616–619`); `crates/fsm-codegen-c/src/config.rs:50` hardcodes the `8` default and `OverflowPolicy::Assert`; the IR `QueueConfig` is **never wired into `CodegenConfig`** for the emit path. Net: a user who writes `queue { capacity = 64; overflow = drop_newest }` silently gets capacity 8 / assert with **no diagnostic** — same severity *class* as R1's F-1 (silent, documented-idiom, no rejecting diagnostic). **Disposition: a #110 / Factory-W reliability input, not a primitive-completeness gap** (the construct exists and is spec'd; it is silently mis-threaded). Surfaced here per the stage-gate / brutal-honesty mandate; **not fixed in this read-only audit.** *Judgment call disclosed (§7-(v)).*

### A.2 `defer` / `deferred` — what it actually IS (R1 cell UPDATED with counter-evidence)

- **Lexer:** `defer` **IS** `KwDefer` (`token.rs:52`, `"defer" => KwDefer` L399).
- **Parser:** `defer_decl = "defer" identifier` → `DEFER_DECL` (`crates/fsm-parser/src/grammar/state.rs:556`); statement form `STMT_DEFER` also exists (`ast/stmt.rs:15`).
- **IR:** `crates/fsm-ir/src/model.rs:905` `DeferDecl { event_id, loc }`; per-state `defers: Vec<DeferDecl>` on every state node (`:314/:331/:351`); `Statement::Defer { event_id }` (`:861`). Lowered by `crates/fsm-analyzer/src/lower/state.rs:871` `lower_defers`.
- **Analyzer — DECISIVE R1 RECONCILIATION:** `crates/fsm-analyzer/src/checks/defer.rs:1–16` (module doc, dated **2026-05-15**) states verbatim: *"`FSM-E0903` … is **retired**. Real per-state defer-buffer runtime now ships in both codegen-c and the simulator per Doc 08 §10, so analysis no longer rejects `defer`."* The only remaining defer diagnostic is **FSM-E0310** (defer-vs-explicit-transition conflict in the same state — a genuine UML 2.5.1 §14.2.3.9.1 contradiction). Corroborated by `crates/fsm-analyzer/src/lib.rs:18–21` ("G-08 / v1.1 `defer EVENT` lowered to the IR defer set; runtime support shipped in codegen-c + simulator").
- **Codegen:** fully implemented — `gen_defer/M.c` emits `M_defer_table[]` (per-state event-defer **bitmask**, `M_STATE_IDLE = 0x00000002u`), `M_active_config_defers()` (active-leaf + ancestor walk), `M_defer_push()` with **`M_DEFER_CAPACITY` and explicit bounded overflow** (`assert` or `drop-newest`, mirroring the event-queue policy — `M.c:243–254`), `M_release_deferred()` (FIFO front-insert release, Doc 08 §10.2/§10.4 recursion-prevention).
- **Simulator + Verifier:** `defer_set` is first-class runtime state — `crates/fsm-simulator/src/interpreter.rs:420` clones it into `InterpreterSnapshot`, `:448` restores it, `:585`/`:1819` push/drain. The verifier digest **keys it as part of the bounded state vector**: `crates/fsm-verify/src/digest.rs:49,77,144` — `defer_set` is in `canonical_bytes` (the observable configuration), and **Doc 08 §13.5 explicitly lists "defer set" in the bisimulation-invariant tuple** `(active states, history, defer set, context, recursive sub-instance structure)`.
- **Behavioural proof** (`/tmp/dp2-probe/t_defer.fsm`, `state Idle { defer DATA; on GO -> Active } state Active { on DATA -> Idle }`): `fsm check --json` → **`[]` (ZERO diagnostics — NO FSM-E0903)**; `fsm generate -t c99 --emit-ir` → exit 0; IR `Idle.defers = [{eventId:"ev-M-DATA", loc:{7:5}}]`; C contains the full defer machinery (27 defer/guard/reject tokens in the §B item-1 fixture).

> **R1 CELL UPDATE (cited counter-evidence; §7-(i)).** R1's matrix one-line basis for `defer` rested on Doc 04 §9.4's text *"parses but is rejected by the analyzer with `FSM-E0903`"*. That text is **STALE** (it predates the 2026-05-15 E0903 retirement). **Counter-evidence:** `checks/defer.rs:8–16` + behavioural `fsm check` → `[]`. **Updated verdict: `defer` (per-state event deferral) is SUPPORTED end-to-end** (lexer→parser→IR→analyzer→codegen→simulator→verifier), with a **bounded, explicit-overflow defer buffer** already in the verifier's bounded state vector. This is not a contradiction of an R1 *finding* (R1 did not list `defer` as its own matrix row; it appears only in R1's keyword corroboration and §1.3 prose) — it is a spec-text-vs-code drift this round resolves with proof. **Doc 04 §9.4 (L980–983) stale-text fix is RECOMMENDED (see §7); not edited here.**

> **Additional drift (NEW, minor, §7):** `feature deferred` is **not enforced** — `state A { defer D; on E -> A }` *without* `feature deferred` → `fsm check --json` = `[]`, exit 0. Doc 04 §9.4 says "Requires `feature deferred`." Code/spec drift (the feature gate is not wired); recorded, not fixed.

---

## B. The three factory-data-plane items — precise current-state verdicts

### Item 1 — State-gated request handling (accept/defer/reject by current state)

**Owner's question:** how much is expressible via deferred events + extern guards + typed payloads, vs a genuine gap?

**Verdict: EXPRESSIBLE-TODAY — fully, now that `defer` is SUPPORTED end-to-end (§A.2).** Not a gap.

**The composition (behaviourally proven — `/tmp/dp2-probe/t_item1.fsm`):**
```
pure extern can_accept (ctx) : bool
machine M {
  events { REQ ( id : u16, kind : u8 )  DONE }
  initial Ready
  state Ready {
    on REQ [can_accept] priority 0 -> Serving : start_job   // ACCEPT (state Ready + guard true)
    on REQ              priority 1 -> Ready   : reject_job   // REJECT (state Ready + guard false)
  }
  state Serving { defer REQ;  on DONE -> Ready }             // DEFER (state Serving: hold REQ, replay on exit)
}
$ fsm check --json  →  only FSM-W0300 ×2 (warnings: "transition conflict resolved by priority"); exit 0
$ fsm generate -t c99  →  exit 0;  27 defer/guard/reject/start occurrences lowered into M.c
$ fsm verify --json  →  real verdict (bound NOT hit; the defer+guard machine is fully explored)
```
The three behaviours — **process-on-state-X** (`state Ready` guarded `[can_accept]`), **defer-on-state-Y** (`state Serving { defer REQ }` holds and FIFO-replays on exit per Doc 08 §10), **reject** (the lower-priority unguarded edge as the `[!can_accept]` complement) — are all expressed with **shipped primitives** (guarded transitions + `priority` + `defer` + typed payloads). `REQ(id:u16,kind:u8)` is a fully-typed payload (Doc 04 §7); guards over it / `ctx` / `pure extern` are SUPPORTED (Doc 04 §8.5).

**Ergonomic/semantic cost (stated honestly — the only finding for item 1):** the analyzer's guard-disjointness check does **not** reason about extern-guard complementarity — `on REQ [can_accept]` paired with `on REQ [!can_accept]` (logically exhaustive) is **rejected with `FSM-E0300` "nondeterministic transition conflict — guards may overlap"** (behaviourally observed) because `can_accept` is opaque to the analyzer. The user must disambiguate with an explicit `priority` (then it is a benign `FSM-W0300` *warning*) or fold accept/reject into one transition + an extern-side branch. This is an **ergonomic friction, not a capability gap** — state-gated request handling is fully achievable today. (A future *ergonomic* sugar — e.g. an `else`-arm on a guarded transition that the analyzer treats as the proven complement — would remove the friction; logged as a minor ergonomics candidate, not a factory-capability gap.)

### Item 2 — Request/response framing of external signals (port-group ABC = request, BCD = its data)

**Owner's question:** does multi-phase signal framing need a primitive, or is it host-side extern territory — and where is the deterministic boundary?

**Verdict: EXPRESSIBLE-TODAY at the deterministic layer (the *framing state machine*); the *bit/port-pattern decode* is — correctly — host-side extern territory. Not a gap.**

**The deterministic boundary (the core answer):** FSM-Lang's deterministic core operates on **discrete, already-decoded, typed events** (Doc 02 §5 — a discrete-event model; Doc 08 §3/§4). The *interpretation of a raw port-bit pattern* (e.g. recognising the byte group `ABC` as "a request" and `BCD` as "its payload") is **signal conditioning / wire decoding** — exactly the "all computation lives in C" contract (Doc 02 §0/§G3). The deterministic boundary sits precisely here: **the host ISR/driver decodes the wire and injects typed FSM events; the FSM deterministically sequences the request→response *protocol phases*.** This is the same boundary R1 established for P3 (output effects are externs) and the extsig design (`design(extsig)`, commit `79d11b0`).

**The framing FSM is expressible today (behaviourally proven — `/tmp/dp2-probe/t_item2.fsm`):**
```
machine M {
  events { RX ( byte : u8 )  FRAME_REQ  FRAME_DATA ( payload : u32 ) }
  initial Idle
  state Idle      { on FRAME_REQ  -> AwaitData : arm_rx }       // phase 1: request recognised
  state AwaitData { on FRAME_DATA -> Idle      : consume        // phase 2: its data received
                    on RX         -> AwaitData : accumulate }    // optional byte accumulation
}
$ fsm check --json  →  []  (zero diagnostics); exit 0
```
The two-phase framing (request seen → enter `AwaitData` → its data event consumed) is a textbook state sequence; multi-phase framing scales to more states/events with **no primitive needed**. The host raises `FRAME_REQ` when it has decoded the `ABC` group and `FRAME_DATA(payload)` when it has decoded the following `BCD` group. **A dedicated framing primitive is NOT warranted** — it would pull wire-decode into the deterministic core, violating §G3 and creating verification surface (raw byte buffers) the bounded verifier should not carry. (This is consistent with the existing `design(extsig)` ISR/polling-frequency proposal — the request/response *pattern-recognition* belongs to that host-extern layer; the *protocol sequencing* is the FSM's job and is already first-class.) **Determinism preserved by construction:** events are discrete and clock-non-observable (Doc 08 §13.5); no new construct.

### Item 3 — A bounded data-plane primitive (typed queue/stack/FIFO, drained under state control) — **THE CRUX**

**Owner's HARD CONSTRAINT:** statically-sized, fixed-capacity, heap-free, deterministic, **explicitly-defined verifiable overflow**, never unbounded/dynamic; if it cannot be made bounded+verifiable, the honest verdict is **DEFERRED/OUT-OF-SCOPE — not a compromised primitive.**

**Current-state verdict: GENUINE GAP for a *user-visible typed* data-plane primitive** (no such construct exists), **with a partial expressible-today substrate (a scalar `ctx` depth-counter, payload-less) and an exact architectural precedent that proves a bounded one IS verifiable in principle (the `defer_set` + event-queue are already bounded state in the verifier).**

**What does NOT exist (behaviourally proven — `/tmp/dp2-probe`):**
- `stack` / `fifo` / `buffer` / `channel` / `ringbuf` / `deque` `Q : u8 [capacity 8]` → **all `FSM-E0010`** (not keywords, no grammar, no IR).
- Array/aggregate-typed context fields: `buf : u8[8]` → **`FSM-E0010`.** The Doc 04 §3 type grammar is **scalars + enum-ref + `opaque "C-type"` only** — no array, no aggregate, no collection type. Aggregate state can enter *only* as an `opaque "my_ring_t"` C type, which is **host-owned and verifier-invisible** (opaque fields are barred even from guards — `FSM-E0210`). So the only data the verifier can see is scalar `ctx`/`history`/`defer_set`/timers.
- **Expressible-today substrate (the honest baseline):** a scalar `ctx` **depth counter** + guards — `on PUSH [count < 8] : ctx.count = ctx.count + 1` / `on DRAIN [count > 0] : ctx.count = ctx.count - 1` (`fsm check` → `[]`, exit 0). This is a **bounded counter, NOT a typed payload-carrying queue**: it models *occupancy/back-pressure* (and is fully verifiable — a small bounded integer in the state vector) but **cannot carry per-element typed payloads or preserve element order/content**. For factory back-pressure/credit gating it suffices; for "queue these typed requests and drain them under state control" it does **not**.

**Boundedness / verifiability analysis (brutally honest — the owner's crux):**

A fixed-capacity typed FIFO/stack *can in principle* be made bounded + heap-free + verifiable **within the project invariants** — the architecture already proves it, because the `defer_set` and the event mailbox are *exactly* such structures and are already in the bounded verifier:
- **Heap-free / statically-sized: precedented.** The codegen pattern is established and proven: `M_defer_table` + `m->_deferred[M_DEFER_CAPACITY]` and `m->_queue[M_QUEUE_CAPACITY]` are fixed C arrays with masked head/tail and **explicit bounded overflow** (`assert`/`drop_oldest`/`drop_newest`/`error` — `fsm-ir` `OverflowPolicy`, `gen_*/M.c:180–254`). A user data-plane queue would reuse this exact mould — no heap, statically sized.
- **Verifiable in principle: precedented and DECISIVE.** Doc 08 §13.5 already places the **defer set** inside the bisimulation-invariant tuple, and `crates/fsm-verify/src/digest.rs:144` keys it in `canonical_bytes`. The verifier is a **bounded explicit-state BFS** (`crates/fsm-verify/src/engine.rs:109` `DEFAULT_MAX_STATES = 100_000`, `:117` `DEFAULT_MAX_STEPS = 2_000_000`) with the **honest-bound invariant** (`engine.rs:180` — bound-hit ⇒ `Inconclusive`, **never** a false `ProvenNoDeadlock`). A bounded typed queue's `(contents, depth)` would enter the same state vector: **bounded capacity ⇒ finite contents domain ⇒ finite state ⇒ verifiable in principle.** Overflow would be a **defined transition/diagnostic** (the existing `OverflowPolicy` enum is exactly the verifiable-overflow contract — never UB), and a drained-under-state-control queue is just another component of the already-explored configuration. **Clock-soundness: untouched** — a typed queue carries no time and reads no clock, so Doc 08 §13.5's lemma is unaffected (it is in the same class as `defer_set`, which §13.5 already covers).
- **The honest cost — STATE-SPACE BLOW-UP (quantified).** This is the real price and must be stated plainly. The verifier's visited-set key is the *full* configuration including queue contents. A FIFO of capacity `C` over an element domain of size `D` (e.g. an event-kind enum, or a small typed record) contributes up to **Σ_{k=0..C} Dᵏ ≈ D^C** distinct contents states, **multiplicatively** on top of the existing `(active states × history × defer_set × context × timer-phase × submachine)` product. Concretely: a capacity-8 queue over even a 4-value element domain is up to 4⁸ ≈ 6.5·10⁴ contents states *per* underlying configuration — which alone can exhaust `DEFAULT_MAX_STATES` (100 000) and force an **honest `Inconclusive`** on machines that verify cleanly today. (Empirically corroborated in this audit: an unbounded internal-event pump `state A { on TICK -> A : raise TICK }` drives the bounded verifier to its step/state ceiling and the honest-bound guard returns no false proof — the boundedness ceiling working as designed.) So a typed-queue primitive is **verifiable but verification-expensive**: it is sound (never a false proof — the honest-bound guard holds) but it materially shrinks the set of machines for which `fsm verify` can return a *conclusive* `ProvenNoDeadlock`. Mitigations exist (queue-content abstraction / a verifier "queue-bounded" honest-Inconclusive mode mirroring the §3 clock-guard honest-bound discipline) **but are a verification-core design cost, not free.**

**Item-3 verdict (one paragraph, unambiguous):** A fixed-capacity, statically-sized, heap-free typed FIFO/stack with an explicit verifiable overflow policy is **NOT inherently out-of-scope** — the project already ships exactly this shape twice (the bounded event mailbox and the per-state `defer` buffer), both heap-free and both already inside the bounded explicit-state verifier's state vector with the honest-bound guarantee and **zero clock-soundness interaction**. It therefore satisfies the owner's hard constraint *in principle* and is a **defensible DSL extension**, **not** a "compromised primitive." **However**, it is a **GENUINE GAP today** (no user-visible typed-data-structure construct exists; the type system has no aggregate/array type; the only expressible substrate is a payload-less scalar depth-counter) **and** closing it carries a **real, quantified verification cost**: queue contents enter the visited-set key and blow the state space up multiplicatively (≈ D^C per configuration), shrinking the conclusively-verifiable machine set (sound — honest `Inconclusive`, never a false proof — but materially costly). The honest recommendation (the owner's framing — primitive selection is the TL's call) is therefore **conditional**, not a flat yes/no: a bounded typed queue is *worth a DSL extension* **iff** it ships with (a) a hard `capacity` + an explicit `OverflowPolicy` (reuse the existing four-variant enum — never UB), (b) a deliberate verifier policy for queue contents (either a sound content-abstraction or an explicit "queue-bounded ⇒ honest `Inconclusive`" mode, co-designed with the verification core exactly like the §3 clock-guard discipline — **not bolted on**), and (c) a `StepRecord`/oracle extension so the debug layer can visualise queue depth+contents. Absent that co-design, the disciplined fallback is the **payload-less `ctx` depth-counter for back-pressure (expressible today)** plus **DEFERRED** for the typed-payload queue — *not* a half-bounded or verification-unsound primitive. **It is NOT "DEFERRED/OUT-OF-SCOPE because impossible" — it is "a real, scoped DSL+verification-core extension with an honestly-quantified cost; TL to select."**

---

## C. THE UNIFIED MATRIX (3 factory items + R1 candidates)

Verdict · (if gap) **DSL** / **kernel** / **both** · determinism & verification cost (bounded-verifier state-space impact, heap-freedom, Doc 08 §13.5 clock-soundness).

| # | Primitive / capability | Verdict | DSL / kernel / both | Determinism & verification cost |
|---|---|---|---|---|
| **I1** | **State-gated request handling** (accept/defer/reject by state) | **EXPRESSIBLE-TODAY** (guarded transitions + `priority` + `defer` + typed payloads — §B.1, proven) | — (ergonomics-only candidate: a proven-complement `else`-arm) | **Zero.** Reuses shipped clock-non-observable primitives; `defer_set` already in §13.5 invariant + digest. Cost: only the FSM-E0300 guard-complement friction (priority workaround). |
| **I2** | **Request/response framing of external signals** | **EXPRESSIBLE-TODAY** (states+events sequence the protocol; wire-decode is host extern — §B.2, proven) | — (no primitive warranted; host-extern boundary is correct) | **Zero.** Discrete typed events only; deterministic boundary = host decodes wire→injects events. No verifier surface added. |
| **I3** | **Bounded typed data-plane queue/stack/FIFO** (drained under state control) | **GENUINE GAP** for the typed primitive · payload-less **`ctx` depth-counter EXPRESSIBLE-TODAY** for back-pressure (§B.3, proven) | **BOTH** (DSL: a `queue`-typed declaration + push/drain + `capacity`/`overflow`; kernel: codegen reuses the bounded-ring mould, **and** a verifier queue-content policy) | **Heap-free: yes** (reuse `defer`/mailbox fixed-array mould). **Clock-soundness: zero interaction** (carries no time — same class as `defer_set`). **State-space: HIGH** — contents enter the visited-set key, ≈ D^C blow-up per config; sound (honest `Inconclusive`, never false proof) but materially shrinks the conclusively-verifiable set ⇒ needs a co-designed verifier content policy. |
| F-2 | `queue {}` event-mailbox **config threading** | **#110-class silent-misconfiguration DEFECT** (parsed; entries+overflow silently not honoured — §A.1) | (reliability fix, not a primitive) — lower-machine entries surfacing + codegen IR-wiring | Not a primitive. Severity = silent miscompile *class* (no diagnostic); **load-bearing for the debug gate** (a debugger over a silently-misconfigured queue is untrustworthy). |
| P-defer | **Per-state event deferral** (`defer E`) — *R1 cell UPDATED, §A.2* | **SUPPORTED end-to-end** (lexer→parser→IR→analyzer→codegen→sim→verifier; bounded explicit-overflow buffer) — R1's "rejected by FSM-E0903" basis is **stale** (cited counter-evidence) | — | **Bounded by construction.** `defer_set` already in Doc 08 §13.5 invariant tuple + `digest.rs` canonical bytes; codegen `M_defer_push` is fixed-capacity + explicit overflow. Doc 04 §9.4 stale-text + unenforced `feature deferred` = drift (RECOMMEND fix, §7). |
| P1 | Runtime-variable timer **duration** | **DEFERRED** (Doc 02 §6 + Doc 08 §13.5 pt5 + §15.1) — **+ Finding F-1** silent miscompile of the spec-mandated **`const`-reference** form (R1) | (DEFERRED; F-1 = #110-class reliability fix: `lower/state.rs` `eval_i64` vs `checks/timer.rs` asymmetry) | A runtime/absolute-time-dependent duration would break §13.5 pt5 ⇒ stays in the §3 verification co-design envelope. F-1 itself: silent — no state-space effect, but a trust-blocker (R1 §6). |
| P2 | **Watchdog** (deadline-since-last-kick) | **GAP** — R1 §2.1 core-adjacent **pull-forward** | **BOTH** (DSL: `watchdog`/`reset_on`; kernel: additive `Trigger::Watchdog{duration_ms,...}`, re-arm on reset event) | Compile-time-const duration ⇒ **safe by construction** (reuses fixed const-fold; digest keys timers by remaining duration; **no §13.5 change**). Low cost. |
| P3 | Output-signal **primitive** (`signal`/`emit`) | **GAP** for the primitive; **effect SUPPORTED** via extern/`raise` (R1) | DSL (+ owner language-philosophy call: Doc 02 §G3 Declarative Purity) | No capability gap (effect already observable in traces via `actions_executed`). Verification-neutral. |
| P3b | Signal **import / record / export** | **LOGGED-REQUIREMENT** (R1) — `StepRecord` reuse seam confirmed; **no typed value channel** (re-verified `trace.rs:43`) | DSL (`import { signal }`) + kernel (append-only `StepRecord` value field) | Additive trace-record extension (the `submachine`-field discipline). No verifier state-vector change. |
| P4 | **Debounce** (time-filtered trigger) | **GAP** — R1 §2.4 larger/later | **BOTH** (new triggering semantics) | New semantic mode (timer+guard composite); modelable today via state+`after`+re-arm guard. Larger/later. |
| P5 | **Edge-vs-level** triggering | **GAP** — R1 §2.4 larger/later | **BOTH** (level-sensitivity = a semantic-model change) | Level-sensitivity has verification implications (R1 §3-adjacent); design-led, later. |
| P6 | **Bounded repetition** (`every N ms times K`) | **GAP** — R1 §2.2 core-adjacent **pull-forward** | **BOTH** (DSL: `times`/`repeat` suffix; kernel: additive `Trigger::Every.max_count: Option<u32>`) | Const count; a *bounded* periodic timer **improves termination** (removes an infinite-`every` cycle); digest unaffected. **Negative** verification cost (helps). |
| — | Probabilistic layer | **DEFERRED-empirical** (Doc 02 §18; R1 §4) — NOT analyzed for gating; rationale **`OWED/TBD-from-owner`** (R1) | n/a | Out of scope by spec; not a pass/fail gate. (Recorded, not analyzed — per directive.) |
| — | Auto-synthesis-from-stimulus | **LOGGED product-vision** (R1 §5) — single line, not analyzed | n/a | Consumes the `StepRecord`/`fsm baseline` seam in reverse. Vision only. |
| — | Clock-read-in-guards | **Scoped WITH the verification core** (Doc 08 §13.5; R1 §3 (a)/(b)/(c)) — **not** re-derived here | both (verification-extension wave) | Cross-referenced only: an absolute-clock-observable guard breaks the §13.5 bisimulation ⇒ must be co-designed, never bolted on (R1 §3 binding constraint stands). |

---

## D. Scope guards honored

- **Probabilistic layer = DEFERRED-empirical** — recorded (Doc 02 §18; R1 §4), **not analyzed** for gating; the deferral *rationale* remains **`OWED/TBD-from-owner`** (R1 §4 — not fabricated here).
- **Auto-synthesis-from-stimulus = a single logged product-vision line** (R1 §5) — not analyzed, not scheduled.
- **Clock-read-in-guards tension** = **cross-referenced** to R1 §3 + Doc 08 §13.5 as *scoped-with-the-verification-work*; **not re-derived or bolted on here**. The R1 §3 binding constraint (any absolute-time-observable feature must be co-designed via (a)/(b)/(c), never shipped in a form that lets the `digest.rs` clock-merge return a false `ProvenNoDeadlock`) **stands unchanged**; item 3 is explicitly *outside* this envelope (a typed queue carries no time).

---

## GATE — TL decision required

Primitive selection is the **TL's call, not the auditor's**; this section informs the TL/orchestrator's gate decision to surface to the owner. The honest recommended split, per-item determinism/heap-free/verification trade-off stated plainly:

### Recommended **DSL-extension** set (worth it; honestly costed)
1. **Bounded typed data-plane queue (Item 3) — RECOMMEND as a DSL+kernel extension, CONDITIONAL.** It satisfies the owner's hard constraint *in principle* (heap-free + statically-sized + explicit verifiable overflow are all **already proven** by the shipped `defer`/event-mailbox buffers, both inside the bounded verifier, zero clock-soundness interaction). It is **not** a compromised primitive and **not** impossible. **But** the conditions are non-negotiable for it to stay sound and useful: **(a)** mandatory `capacity` + an explicit `OverflowPolicy` (reuse the existing four-variant `fsm_ir::OverflowPolicy` — never UB); **(b)** a *co-designed* verifier queue-content policy (a sound content-abstraction **or** an explicit "queue-bounded ⇒ honest `Inconclusive`" mode, designed *with* the verification core exactly as the §3 clock-guard discipline mandates — **not bolted on**), because contents in the visited-set key blow the state space up ≈ D^C and otherwise silently shrink the conclusively-verifiable set; **(c)** a `StepRecord`/oracle extension exposing queue depth+contents (the debug layer must visualise it). **If the TL is not willing to fund the (b) verification co-design now, the disciplined fallback is: ship the payload-less `ctx` depth-counter pattern (expressible today, fully verifiable) for back-pressure and mark the typed-payload queue DEFERRED — never a half-bounded primitive.**
2. **Watchdog (P2) and bounded repetition (P6)** — R1's core-adjacent pull-forwards still stand: compile-time-const durations ⇒ **safe by construction** (no §13.5 change; P6 *improves* termination). Low verification cost. RECOMMEND for the post-quality language roadmap (R1 §2.1/§2.2 scopes unchanged).

### Recommended **DEFERRED** / not-now set
- **Item 2 (request/response framing): no primitive** — EXPRESSIBLE-TODAY; a primitive would wrongly pull wire-decode into the deterministic core (Doc 02 §G3). Deterministic boundary = host decodes → injects events.
- **Item 1 (state-gated requests): no primitive needed** — EXPRESSIBLE-TODAY end-to-end; only a *minor ergonomic* sugar (proven-complement `else`-arm to avoid the FSM-E0300 priority workaround) is a candidate, not a capability gap.
- **P1 runtime-variable timer duration**, **debounce (P4)**, **edge/level (P5)**, **signal primitive + import/record/export (P3/P3b)**, **probabilistic**, **auto-synthesis** — all stay as R1 classified (DEFERRED / larger-later / logged). Unchanged.

### Reliability fixes the gate must NOT lose (#110 / Factory-W class — not primitives, but trust-blockers)
- **Finding F-1** (R1) — silent miscompile of the spec-mandated `const`-reference timer duration (`lower/state.rs` `eval_i64` vs `checks/timer.rs` asymmetry).
- **Finding F-2** (NEW, this round) — `queue {}` config silently not honoured end-to-end (entries unsurfaced to the lowerer **and** IR `QueueConfig` never wired into codegen; capacity 64/`drop_newest` → silently 8/`assert`, **no diagnostic**). Same severity *class* as F-1.
- **R1 §2.3** — the rejecting diagnostic for non-const-foldable timer durations (silent-drop → hard error).
- **Doc 04 §9.4 stale-text** (RECOMMEND, not edited here): L980–983 still says `defer`"is rejected by the analyzer with `FSM-E0903`" — contradicted by `checks/defer.rs:8–16` (E0903 retired 2026-05-15) and behavioural proof; **`feature deferred` is also unenforced**. Spec-vs-code drift.

### Implication for the debug-interface implementer-wave gating (the open TL-gate question)

The debug **build** is blocked on this gate. Two things the TL must decide and surface to the owner:

1. **The silent-misconfiguration class is the load-bearing debug blocker** (consistent with R1's verdict that the *silent-drop* class — not the additive primitives — is the single real trust-blocker for an inspect/replay layer). R1's blocker (F-1 + §2.3) **plus this round's F-2** (silent `queue {}` mis-threading) are the set that must land + be verified **before** the debug interface is built: a debugger whose value is *trust* cannot be built over a toolchain that silently drops a timer **or** silently mis-sizes the event queue with zero diagnostics. **The additive primitives (watchdog, bounded-rep, signal, and a future bounded typed queue) are NOT debug-layer blockers** — the debug layer faithfully steps whatever the frozen set expresses (and `defer`, now proven SUPPORTED, is already faithfully stepped — `defer_set` is in the snapshot/digest).
2. **The OPEN TL-GATE QUESTION (must be answered before the debug-interface design is finalised, because it changes the oracle/StepRecord surface the debug UI must visualise):** *Is the bounded typed data-plane queue (Item 3) accepted as a near-term DSL+kernel extension?* If **YES** → it adds queue depth+contents to the oracle/`StepRecord`, and the debug-interface design **must** budget for visualising a bounded queue (depth, contents, overflow events) and the verifier's new queue-content policy — i.e. the debug design cannot be frozen until the Item-3 verification co-design ((b)) is at least scoped. If **NO / DEFERRED** → the debug interface designs against the *current* frozen set (events/timers/HSM/parallel/history/submachine/**defer**) and no queue-visualisation surface is needed now. **This is the single product-strategy decision this audit surfaces to the owner via the TL; the auditor does not lock it.** Recommended default if the TL wants to keep the debug wave unblocked *now*: treat Item 3 as DEFERRED-pending-verification-co-design (ship the expressible-today depth-counter for back-pressure), fix F-1/F-2/§2.3, and let the debug interface design against the frozen+`defer` set — revisiting Item 3 as a scoped DSL+verification wave after the debug layer lands.

---

## E. Confirmations (auditor attestations)

- **Read-only except this doc.** No `crates/`/`Cargo*`/`.github/` edit. The only file written/committed on `phase8.0/detprim-r2-factory-data-plane` is this document. No merge, no tag, main untouched.
- **No `git stash`** at any point. Inspection used `grep`/`Read` only; the `fsm` binary was built `--release` from this worktree into the documented shared `CARGO_TARGET_DIR`.
- **Toolchain asserted from the worktree:** `rustup show active-toolchain` → `1.75.0-x86_64-unknown-linux-gnu (overridden by …/rust-toolchain.toml)`.
- **Every verdict behaviourally proven** by running the worktree-built `fsm` on real `.fsm` in `/tmp/dp2-probe` (NEVER in the repo) **and** traced lexer→parser→IR→analyzer→codegen→simulator→verifier with `file:line` citations; keyword scans used only as corroboration.
- **R1 reconciliation honest:** exactly one R1 cell updated (`defer` end-to-end status) **with explicit cited counter-evidence** (`checks/defer.rs:8–16` + behavioural `fsm check`→`[]`); R1 did not carry `defer` as its own matrix row, so no frozen R1 *finding* is contradicted — a stale Doc 04 §9.4 spec-text vs code drift is resolved with proof. No other R1 cell contradicted; R1 §3 clock envelope and probabilistic/auto-synthesis dispositions carried verbatim.
- **Did NOT fabricate OWED context** — the probabilistic-deferral rationale stays `OWED/TBD-from-owner` (R1 §4), not invented.
- **Judgment calls disclosed:** **(i)** `defer` reclassified **SUPPORTED end-to-end** (R1's FSM-E0903 basis is stale — counter-evidence cited; §A.2). **(ii)** Item 1 = **EXPRESSIBLE-TODAY** (not a gap) — the FSM-E0300 guard-complement friction is an ergonomic cost, not a capability gap; honestly stated. **(iii)** Item 2 = **EXPRESSIBLE-TODAY**, no primitive warranted — the deterministic boundary is placed at host-decode→inject-events (consistent with Doc 02 §G3 + the extsig design); a framing primitive is explicitly *not* recommended. **(iv)** Item 3 = **GENUINE GAP** for the typed primitive **but explicitly NOT "out-of-scope-because-impossible"** — it is bounded+heap-free+verifiable *in principle* (the `defer`/mailbox precedent proves it) at a **quantified** state-space cost (≈ D^C); the honest verdict is a **conditional RECOMMEND with a mandatory verifier co-design**, with a disciplined fallback (payload-less depth-counter + DEFERRED) — *never* a half-bounded primitive. The boundedness paragraph (§B.3) is deliberately unambiguous per the owner's brutal-honesty instruction. **(v)** F-2 classified a **#110-class reliability defect, not a primitive gap** (the construct is spec'd and parsed; it is silently mis-threaded) — surfaced as load-bearing for the debug gate because a debugger over a silently-misconfigured queue is untrustworthy (same logic R1 applied to F-1). **(vi)** Verdict deliberately frames the **single product-strategy question (accept Item 3 now?) as the open TL-gate item to surface to the owner**, with a recommended unblock-the-debug-wave default — not auto-decided (primitive selection is the TL's call).
- **Flagged, not invented:** the `queue {}` entries-unsurfaced root cause is pinned to *either* the `AstChildren<ConfigEntry>` accessor *or* the entry-threading (both observed symptomatically — IR keeps default despite `loc` captured); the exact line is not over-asserted beyond the cited `lower/machine.rs:363`/`generate.rs:616–619`/`config.rs:50` evidence. Nothing unverifiable is stated as fact.

---

*End of FSM-AUDIT-DETPRIM-R2-2026-05-19.*
