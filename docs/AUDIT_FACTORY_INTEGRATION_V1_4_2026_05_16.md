# §11.3 v1.4-W4a — Factory-Integration Completeness Audit (the headless verify→generate→check→baseline pipeline)

**Document ID:** FSM-AUDIT-FACTORY-INTEGRATION-V14-W4a
**Version:** 1.0.0 (FROZEN evidence doc — **never overwrite**; a new audit is a new versioned file)
**Audit type:** §11.3 / Doc 30 §4.2-W4 + §5.2 factory-pipeline gate of record. Real-run evidence audit (binary built + driven; every claim is a captured command + its actual exit code + its `jq`-parsed JSON — the anti-symbol-presence bar, Doc 30 §186 / SUBAGENT §5).
**Audited commit:** `fc88ec1` (`fc88ec1d95688d09379db4c8c09f7a9703bcbfe8` — `v1.4-W3: trace differential replay (fsm baseline drift harness)`). v1.4's verification core is complete + independently verified sound on `main` at this commit (the W2 §11.3 audit `AUDIT_PHASE_V1_4_W2_2026_05_16.md` = `PROCEED-WITH-NOTES`, keystone intact).
**Worktree / branch:** `/root/dev/embeded-fsm-sdk-wt-v14w4a` on `phase4.4/v14-w4a-factory-audit` (cut from `fc88ec1`, clean). Built binary: `/root/dev/embeded-fsm-sdk-target/debug/fsm` (`fsm 0.1.0`, `cargo build -p fsm-cli --bin fsm`, toolchain `1.75.0` overridden by `rust-toolchain.toml` — asserted from inside the worktree).
**Auditor:** W4a implementer/auditor (own work; not delegated).
**Governing record:** Doc 30 §4.2-W4 (sharpened) / §5.2 (the pipeline-before-UI tag-gate of record) / §186, Doc 18 §3, `crates/fsm-cli/src/cmd/{verify,baseline,generate,check}.rs`, `[[feedback_embeded_fsm_pipeline_before_ui]]`, `docs/AUDIT_PHASE_V1_4_W2_2026_05_16.md` (the frozen-evidence-doc quality bar), `docs/processes/SUBAGENT_CONVENTIONS.md` §5/§11.

---

## 0. Executive summary (read this first)

**Verdict: `FACTORY-COMPLETE`.**

The full functional pipeline is **usable headless at the CLI layer with zero workflow gaps**. Every workflow the owner's binding bar requires was demonstrated by a **real captured command + its actual exit code + its parsed JSON** (not "the flag exists"):

- `fsm verify --json` correctly returns the **three distinct verdicts** with the documented exit-code family: `verified` (exit 0), `property-violated` (exit 1, with a non-empty counterexample `witness`), and the **honest-bound** `inconclusive` (exit 2, `bound.hit == true`) — it **never** reports a false `verified` when a bound is hit (confirmed at the JSON-field level, not just the exit code: under `--max-states 1` the unexplored states are reported with `reachability.result == "inconclusive"`, never `"holds"`).
- Error buckets stay **distinct from a verdict**: a missing path → exit 3, a model that won't compile → exit 4.
- `fsm generate --target c99 --emit-ir` produces a complete C unit set + HAL + IR (exit 0); the emitted C compiles clean under `-std=c99 -Wall -Wextra -Wpedantic -Werror`.
- `fsm check --json` emits a JSON diagnostics array (exit 0; empty for the clean model).
- `fsm baseline --record` then `--check --json` closes the regression-oracle loop (both exit 0; second JSON `verdict == "no-drift"`, `schema == "fsm-trace-diff/v1"`).
- The **whole loop as ONE CI-shaped script** (the shipped `examples/verify/README.md` script) runs end-to-end headless and is **deterministic**: the schema-versioned JSON is byte-identical across two runs (the only varying field is a faithful provenance echo of a caller-chosen path — a correctness property, not a defect).

There is **no capability or workflow gap** that blocks headless factory integration. The single finding (§3.G) is a **documentation-discoverability** one — the `fsm verify`/`fsm baseline` exit-2 INCONCLUSIVE verdict is a sanctioned superset of Doc 18 §3's general process-exit table, and Doc 18 §3 (which declares itself the single authoritative source) does not yet surface it. Per the W4 plan this is **DEFER** red into the W4b doc-batch — it is **NOT a code change** (the in-source `0/1/2/3/4` verdict family is the owner-mandated correct design; Doc 30 §4.2-W4/§5.2 explicitly reject a generic `0/1`) and **NOT a tag blocker**. The interim is already closed by this wave's `examples/verify/README.md` + the appended Doc 25 §9, which document the superset for integrators.

### Findings table

| ID | Sev | One-line | Evidence | Disposition |
|---|---|---|---|---|
| **A–H** | — | The full headless verify→generate→check→baseline loop is demonstrably driveable by a CI script, machine-readable, deterministic, zero UI/interactive step — every workflow proven by a real captured command + exit code + parsed JSON | §2 evidence table; §2.H determinism transcript | **PASS** |
| **§3.G** | P3 (doc) | `fsm verify`/`fsm baseline` exit `2` = first-class INCONCLUSIVE verdict; Doc 18 §3 (self-declared single authoritative exit-code source) maps exit 2 to "tool error" and contains zero mention of the verify/baseline verdict family ⇒ a docs-*only* factory integrator would misread exit 2 as a tool crash. The two contracts are internally coherent + symmetric (baseline mirrors verify); the in-source contract is self-documented + owner-mandated-correct. | `crates/fsm-cli/src/cmd/verify.rs:11-33`; `crates/fsm-cli/src/cmd/baseline.rs:39-54`; `docs/18-CLI-Specification.md:49-62`; `grep` of Doc 18 for `verify`/`baseline`/`inconclusive` ⇒ ∅ | **DEFER → W4b doc-batch** (Doc 18 §3 sub-section authoritatively documenting the sanctioned superset). NOT a code change. NOT a tag blocker. Item I final tick gated on W4b carry-item-5. Interim discoverability closed by this wave's `examples/verify/README.md` + Doc 25 §9. |

**Verdict line: `FACTORY-COMPLETE` — the pipeline capability is solid + factory-integratable headless with zero workflow gaps; the only gap is the §3.G doc-discoverability one, deferred to W4b (not a tag blocker, not a code change).**

---

## 1. Method & toolchain assertion

```
$ cd /root/dev/embeded-fsm-sdk-wt-v14w4a && rustup show active-toolchain
1.75.0-x86_64-unknown-linux-gnu (overridden by '/root/dev/embeded-fsm-sdk-wt-v14w4a/rust-toolchain.toml')

$ git rev-parse HEAD
fc88ec1d95688d09379db4c8c09f7a9703bcbfe8        # branch phase4.4/v14-w4a-factory-audit, clean

$ cargo build -p fsm-cli --bin fsm
    Finished dev [unoptimized + debuginfo] target(s) in 10.37s          # incremental; shared target dir
$ /root/dev/embeded-fsm-sdk-target/debug/fsm --version
fsm 0.1.0
```

Disk held within margin throughout (`/` at 92% / ≈6.7G free before and after the build — incremental build against the warm shared `target/`, no `cargo clean` needed; NOTE-1 stale-target artifact did not arise because W4a *runs the binary* rather than `cargo test`-ing untouched crates). Every workflow below is a real invocation; exit codes captured via `$?`, JSON parsed via `jq` (anti-symbol-presence bar — a flag's mere existence is never accepted as evidence).

---

## 2. The factory-completeness acceptance checklist — real-run evidence

The two worked machines are the shipped `examples/verify/clean.fsm` (a timer-driven thermal controller, no guarded transitions ⇒ sound) and `examples/verify/deadlocks.fsm` (a safety-latch with a reachable guard-trap sink — a guard statically unsatisfiable on the reachable context; the canonical reachable-deadlock shape the W2 fixture `crates/fsm-verify/tests/fixtures/deadlock_guard_trap.fsm` cross-checks). Each `.fsm` carries a header hand-verifying its property against Doc 08 §3.1/§9.1.

| Item | Command | `$?` | Parsed JSON (the load-bearing fields) | Verdict |
|---|---|---|---|---|
| **A** | `fsm verify --json examples/verify/clean.fsm` | **0** | `.schema=="fsm-verify/v1"`, `.verdict=="verified"`, `.properties.deadlockFree.result=="holds"`, `.properties.reachability.result=="holds"`, `.bound.hit==false`, `.bound.stopReason=="exhausted"`, `.bound.configsVisited==4` (all 4 states reachable, search exhaustive) | **PASS** |
| **B** | `fsm verify --json examples/verify/deadlocks.fsm` | **1** | `.verdict=="property-violated"`, `.properties.deadlockFree.result=="violated"`, `.properties.deadlockFree.counterexample.config==["s-SafetyLatch-Latched"]`, `.properties.deadlockFree.counterexample.witness==["ARM"]` (non-empty) | **PASS** |
| **C** | `fsm verify --json --max-states 1 examples/verify/clean.fsm` | **2** | `.verdict=="inconclusive"`, `.bound.hit==true`, `.bound.stopReason=="max-states-hit"`, `.properties.deadlockFree.result=="inconclusive"`, **`.properties.reachability.result=="inconclusive"`** (the not-yet-explored states are NOT falsely reported `holds`) — honest bound, never a false "verified" | **PASS** |
| **D1** | `fsm verify --json examples/verify/does-not-exist.fsm` | **3** | (stderr: `error: cannot read … No such file or directory`; no stdout) — IO bucket, distinct from a verdict | **PASS** |
| **D2** | `fsm verify --json /tmp/broken.fsm` (undeclared event + missing target state) | **4** | (stderr: `error: … has analysis errors; cannot verify a model that does not compile`; `fsm check --json` on the same file ⇒ `["FSM-E0101","FSM-E0100"]`) — won't-compile bucket, distinct from a verdict | **PASS** |
| **E** | `fsm generate --target c99 --emit-ir examples/verify/clean.fsm --out <out>` | **0** | Emitted: `ThermalController.c`, `ThermalController.h`, `ThermalController_conf.h`, `ThermalController_impl.h` (C unit set), `fsm_hal.h` (HAL), `ThermalController.ir.json` (IR). `jq .machines[0].name` ⇒ `"ThermalController"` (IR valid). `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror -c ThermalController.c` ⇒ exit 0 (emitted C compiles clean) | **PASS** |
| **F** | `fsm check --json examples/verify/clean.fsm` | **0** | `jq type` ⇒ `"array"`, `jq length` ⇒ `0` (well-formed JSON diagnostics array; empty = clean model) | **PASS** |
| **G** | `fsm baseline --record --corpus <c> crates/fsm-cli/tests/fixtures/w3_baseline_suite` then `fsm baseline --check --json --corpus <c> <suite>` | **0** / **0** | record: 6 corpus files written (`composite/deferred/motor/parallel/submachine/traffic-light`). check JSON: `.schema=="fsm-trace-diff/v1"`, `.verdict=="no-drift"`, `.exitCode==0`, `.fsms|length==6`, `[.fsms[].result]|unique==["match"]` | **PASS** |
| **H** | The whole loop as ONE CI-shaped script (`examples/verify/README.md` §3 `ci-verify.sh`), run twice, JSON byte-compared | **0** / **0** | Both runs print `GATE PASSED`. `cmp` of the schema-versioned JSON across runs: `verify-clean.json`, `verify-bad.json`, `verify-bound.json` byte-identical (SHA256 stable). `bl-check.json` byte-identical once the **`.corpus` provenance echo** (a caller-chosen path, deliberately varied in the harness) is held fixed (`del(.corpus)` ⇒ identical; same fixed `--corpus` ⇒ SHA256-identical). All 6 corpus files byte-identical across runs. | **PASS** (deterministic) |

### 2.A–C — verbatim JSON (the three verdicts)

```jsonc
// A — fsm verify --json examples/verify/clean.fsm   ($? == 0)
{ "schema":"fsm-verify/v1","verdict":"verified","exitCode":0,"machine":"ThermalController",
  "properties":{ "deadlockFree":{"result":"holds"},
    "reachability":{"result":"holds","reachableStates":["s-ThermalController-Cooling",
      "s-ThermalController-Heating","s-ThermalController-Idle","s-ThermalController-Running"],
      "unreachableStates":[],"diagnostics":[]} },
  "bound":{"maxStates":100000,"maxSteps":2000000,"configsVisited":4,"edgesExplored":14,
    "hit":false,"stopReason":"exhausted"} }

// B — fsm verify --json examples/verify/deadlocks.fsm   ($? == 1)
{ "schema":"fsm-verify/v1","verdict":"property-violated","exitCode":1,"machine":"SafetyLatch",
  "properties":{ "deadlockFree":{"result":"violated",
      "counterexample":{"config":["s-SafetyLatch-Latched"],"witness":["ARM"]}},
    "reachability":{"result":"holds","reachableStates":["s-SafetyLatch-Idle",
      "s-SafetyLatch-Latched"],"unreachableStates":[],"diagnostics":[]} },
  "bound":{"maxStates":100000,"maxSteps":2000000,"configsVisited":2,"edgesExplored":4,
    "hit":false,"stopReason":"exhausted"} }

// C — fsm verify --json --max-states 1 examples/verify/clean.fsm   ($? == 2)
{ "schema":"fsm-verify/v1","verdict":"inconclusive","exitCode":2,"machine":"ThermalController",
  "properties":{ "deadlockFree":{"result":"inconclusive"},
    "reachability":{"result":"inconclusive","reachableStates":["s-ThermalController-Heating",
      "s-ThermalController-Idle"],"unreachableStates":["s-ThermalController-Cooling",
      "s-ThermalController-Running"],"diagnostics":[]} },
  "bound":{"maxStates":1,"maxSteps":2000000,"configsVisited":2,"edgesExplored":1,
    "hit":true,"stopReason":"max-states-hit"} }
```

**The honest-bound is the single most safety-critical property and it holds at the JSON-field level.** In C the search hit the `--max-states 1` ceiling after exploring only `Idle`/`Heating`. The verdict is `inconclusive` (exit 2), `bound.hit==true`, and — critically — `properties.reachability.result` is `"inconclusive"`, NOT `"holds"`. `Cooling`/`Running` appear under `unreachableStates` **but a docs-only consumer is not fooled**: the contract is "branch on `.verdict` / per-property `.result`", and both say inconclusive. A factory script keying on `.verdict=="verified"` (or `.properties.deadlockFree.result=="holds"`) correctly rejects this run. There is no path by which a bound-hit run is reported as a proof.

### 2.H — determinism transcript

```
$ WORK=/tmp/.ci-run-1 sh ci-verify.sh   →  GATE PASSED   ($? == 0)
$ WORK=/tmp/.ci-run-2 sh ci-verify.sh   →  GATE PASSED   ($? == 0)

$ cmp .ci-run-1/verify-clean.json .ci-run-2/verify-clean.json   # IDENTICAL
$ cmp .ci-run-1/verify-bad.json   .ci-run-2/verify-bad.json     # IDENTICAL
$ cmp .ci-run-1/verify-bound.json .ci-run-2/verify-bound.json   # IDENTICAL
$ diff <(jq -S 'del(.corpus)' .ci-run-1/bl-check.json) \
       <(jq -S 'del(.corpus)' .ci-run-2/bl-check.json)          # IDENTICAL (only .corpus, the
                                                                #   caller-chosen path, differs)
# same command, SAME fixed --corpus, twice:
$ sha256sum blc1.json blc2.json
d51ac295…  blc1.json
d51ac295…  blc2.json                                            # byte-identical
$ # all 6 corpus files byte-identical across the two runs
```

The verify/baseline JSON is sorted-by-construction (`BTreeMap`; arrays in stable order — `crates/fsm-cli/src/cmd/verify.rs:35-39,284-318`, `crates/fsm-cli/src/cmd/baseline.rs:105-110` module docs; corpus `StepRecord`s are `BTreeMap`/`Vec` in interpreter-emission order, `baseline.rs:56-77`). The **only** field that varies between runs is `baseline`'s `.corpus`, which echoes the `--corpus` path verbatim as provenance — varying it is *correct* (it faithfully reflects a caller-chosen input), not a non-determinism defect. With a fixed corpus path (the realistic CI shape) the output is byte-identical. **Determinism: confirmed.**

---

## 3. Findings of record

### 3.A–H — the headless pipeline is factory-complete (PASS)

Every item A–H passed as a real run (§2). The pipeline is:

- **Non-interactive** — every command takes only file paths + flags; no prompt anywhere in the loop.
- **Zero UI / LSP / network / daemon dependency** — `fsm verify` behaves exactly like `fsm check`/`fsm generate` (confirmed by running the whole loop with nothing but the binary + `jq`; Doc 30: the CLI is the canonical verification surface, no server/plugin by design).
- **Machine-readable** — every CI decision is a JSON field (`.verdict`, `.exitCode`, `.properties.*`, `.bound.hit`, `.fsms[].result`) plus the process exit code.
- **Deterministic** — proven by the two-run byte-compare (§2.H).
- **Gap-free** — verify gates → generate emits buildable firmware (the emitted C compiles `-Werror`) → check backstops → baseline catches semantic regression; the negative case (`deadlocks.fsm`, exit 1) proves the gate *fails when it must*; the bound-hit case (exit 2) proves the gate is honest.

The worked recipe + the runnable CI-shaped script + the determinism harness ship at `examples/verify/README.md`; the contract reference (exit-code family + the `fsm-verify/v1` / `fsm-trace/v1` / `fsm-trace-diff/v1` schema-versioning policy) ships there and in the appended `docs/25-Integration-Guide.md` §9.

### 3.G — finding-of-record: the Doc 18 §3 exit-2 divergence (P3, doc-discoverability) — DEFER

**Confirmed from source (not re-discovered blind — the W4 plan flagged this; this audit *confirms* it):**

1. **The verify/baseline contracts are internally coherent + symmetric with each other.** `crates/fsm-cli/src/cmd/verify.rs:11-33` defines a `0/1/2/3/4` verdict family: 0 `verified`, 1 `property-violated` (with witness / proven `FSM-E0400`), 2 `inconclusive` (bound hit — "Explicitly NOT verified"), 3 IO/not-found, 4 won't-compile (distinct from a verdict). `crates/fsm-cli/src/cmd/baseline.rs:39-54` defines the **same family**, explicitly stating it *"Mirrors `fsm verify`'s family"*: 0 `no-drift`, 1 `drift` (with first-mismatch), 2 `inconclusive` (corpus absent — "Explicitly NOT no-drift"), 3 IO, 4 out-of-scope. The symmetry is exact: bucket-for-bucket the same shape, with the honest-INCONCLUSIVE-at-2 discipline preserved in both ("never a false pass" in each). The two contracts are sound and a coherent sibling pair — a factory integrator who learns one knows the other.

2. **The divergence from Doc 18 §3 is real and would mislead a docs-only factory integrator on exit 2.** `docs/18-CLI-Specification.md:53-54` declares: *"This table is the single authoritative source for exit codes. Other docs … MUST NOT redefine."* Its table (`:56-62`) maps exit `2` → *"Tool error — internal compiler bug, I/O error, invalid flags"*. A `grep` of the entire Doc 18 for `verify` / `baseline` / `inconclusive` / `property-violated` / `no-drift` / the verdict superset returns **∅** — Doc 18 does not surface the per-subcommand verdict family at all. A factory integrator reading **only Doc 18** would therefore interpret a `fsm verify` exit `2` as a *tool crash to retry*, when it actually means **the safety property is NOT proven** (the bound was hit). This is a genuine, specific misread hazard on the single most safety-critical exit code. (The in-source contract is self-documented and correct — `verify.rs:26-33` contains the explicit reconciliation note explaining why exit 2 is a verdict, not a tool error; the gap is purely that the *authoritative doc* does not yet carry it.)

**Disposition — DEFER (fixed by the W4 plan; this audit records, does not re-decide):** folded into the **W4b doc-batch** as a Doc 18 §3 sub-section authoritatively documenting the *sanctioned per-subcommand verdict superset* (Doc 18 remains the single authoritative source, now correctly including the superset). This is:

- **NOT a code change.** The in-source `0/1/2/3/4` verdict family is the **owner-mandated correct design** — Doc 30 §4.2-W4 (sharpened) + §5.2 explicitly **reject a generic `0/1`** ("the three exit codes are the **distinct** verified / property-violated / inconclusive contract (not a generic 0/1)"). Changing the code to collapse onto Doc 18's generic table would *break the owner mandate*. The code is right; the authoritative doc is incomplete.
- **NOT a tag blocker.** It is a doc-reconciliation item, the project's correct batched-doc discipline (same class as the W2-audit closeout-batch findings F5–F10).
- **Item I's *final* tick is therefore gated on W4b**, recorded precisely as: *capability present + self-documented in-source; docs-alone discoverability closed by W4b carry-item-5 (the Doc 18 §3 surfacing)*. The **interim discoverability is already closed by this wave**: `examples/verify/README.md` §1.1 and the appended `docs/25-Integration-Guide.md` §9.1 both document the exit-2-is-INCONCLUSIVE superset for a factory integrator with an explicit "treat exit 2 as failure-to-prove, never a pass, never a blind retry" instruction — so no integrator who reads the integration surfaces this wave ships is misled in the window before W4b lands.

This finding does **not** lower the verdict: it is a documentation-surfacing gap with a fixed disposition and an interim mitigation already shipped, not a capability/workflow gap in the headless pipeline.

---

## 4. Scope-boundary attestation

W4a touched **only** the exclusively-owned files. `git diff --name-only fc88ec1..HEAD` (post-commit) is exactly:

- `examples/verify/clean.fsm` (new)
- `examples/verify/deadlocks.fsm` (new)
- `examples/verify/README.md` (new)
- `docs/AUDIT_FACTORY_INTEGRATION_V1_4_2026_05_16.md` (new — this doc)
- `docs/25-Integration-Guide.md` (APPEND-ONLY: new "## 9. Verification in CI / factory pipelines"; no existing text edited)

No crate source, no Doc 00/30/08/18, no other example, no backlog, no other doc. All generated artifacts and the materialized CI script used for evidence were written under `/tmp` (never the repo). The commit on `phase4.4/v14-w4a-factory-audit` is **not merged / not pushed / not tagged** — the orchestrator gates.

---

## 5. Verdict

> **`FACTORY-COMPLETE`.** The full `fsm verify → fsm generate → fsm check → fsm baseline` workflow is usable headless at the CLI layer with **zero workflow gaps** — CI/script/factory-integratable, machine-readable, deterministic, zero interactive/UI step — proven item-by-item by real captured commands + actual exit codes + `jq`-parsed JSON (§2). The honest-bound discipline (no false "verified" on a bound hit) holds at the JSON-field level, not merely the exit code. The only finding (§3.G) is the **doc-discoverability** divergence of the exit-2 INCONCLUSIVE verdict from Doc 18 §3's general table — **DEFER**red into the W4b doc-batch per the W4 plan; it is **not a code change** (the in-source verdict family is the owner-mandated correct design) and **not a tag blocker**, with interim discoverability already closed by this wave's `examples/verify/README.md` + Doc 25 §9. Item I's final tick is gated on W4b carry-item-5 (the Doc 18 §3 surfacing).
>
> A false `FACTORY-COMPLETE` would be the worst outcome; this verdict is rendered after driving the real binary end-to-end, twice, and confirming the §3.G finding from source — it is the honest result of the owner's binding gate.
