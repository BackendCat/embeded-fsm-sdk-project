# `examples/verify/` — the headless verification → generate → check loop

This directory is the **worked recipe** for driving FSM Studio's
verification pipeline from a CI / Make / factory build step with **zero
interactive input and zero UI dependency**. Everything below is a real
`fsm` invocation keyed on its **exit code** and (where applicable) its
**deterministic, schema-versioned `--json`** on stdout.

It ships two machines:

| File | What it is | `fsm verify` outcome |
|---|---|---|
| [`clean.fsm`](./clean.fsm) | A sound thermal-controller (timer-driven cycle, no guarded transitions). | exit `0` — `verdict: "verified"`, deadlock-free, every state reachable, search exhaustive. |
| [`deadlocks.fsm`](./deadlocks.fsm) | A safety-latch with a reachable guard-trap sink (a guard that is statically unsatisfiable on the reachable context). | exit `1` — `verdict: "property-violated"`, with a counterexample `witness`. |

Both are self-contained (no `extern`, no `import`), so `fsm generate
--emit-ir` compiles `clean.fsm` to a C unit set + HAL + IR with no extra
flags.

---

## 1. The contract a factory integrates against

### 1.1 Exit codes — `fsm verify` / `fsm baseline`

`fsm verify` and `fsm baseline` layer a **verification-verdict** contract
on top of Doc 18 §3's process-exit buckets. The two subcommands use the
**same exit-code family** (`0/1/2/3/4`) with subcommand-specific verdict
meanings — a factory script keys on these:

| Exit | `fsm verify` | `fsm baseline` |
|---|---|---|
| **0** | `verified` — all properties hold (deadlock-free; if exhaustive, no unreachable-state error) | `no-drift` — every replay matched its frozen baseline (or `--record` wrote the corpus) |
| **1** | `property-violated` — a deadlock is reachable (with a counterexample witness) **or** a proven `FSM-E0400` unreachable state | `drift` — at least one FSM diverged; the first-mismatch is reported |
| **2** | `inconclusive` — a bound was hit; the reachable space was **not** fully explored. **Explicitly NOT "verified".** | `inconclusive` — the baseline corpus is absent / unreadable / wrong schema, so drift could be neither confirmed nor denied. **Explicitly NOT "no-drift".** |
| **3** | input file not found / unreadable | IO error (suite dir missing, corpus dir unwritable, a corpus file unreadable mid-walk) |
| **4** | the model does not parse/analyze (cannot verify what won't compile — kept distinct from a verdict) | out-of-scope — a suite `.fsm` does not parse/analyze, or its driver is malformed (kept distinct from a verdict) |

Exit **2** is the **honest-bound outcome**: when a `--max-states` /
`--max-steps` ceiling is hit, `fsm verify` returns `inconclusive` (exit 2)
— it NEVER reports a false `verified`. A factory script MUST treat exit 2
as "not proven" (fail the gate or widen the bound + re-run), never as a
pass.

> **Reconciliation note.** Doc 18 §3 — the single authoritative
> process-exit table — nominally uses exit `2` for "tool error / invalid
> args"; `clap` still emits exit 2 for its *own* usage errors *before*
> these subcommands run, so there is no collision. For `fsm verify` /
> `fsm baseline`, exit 2 is the first-class **INCONCLUSIVE** verdict. This
> sanctioned per-subcommand superset is the binding contract for these
> subcommands; it is authoritatively documented in Doc 18 §3 (the
> CLI-specification surfacing) so a docs-only factory integrator reads one
> coherent exit-code family. See `crates/fsm-cli/src/cmd/verify.rs`
> (module docs) and `crates/fsm-cli/src/cmd/baseline.rs` (module docs) for
> the in-source contract.

For every other subcommand (`fsm check`, `fsm generate`) the plain
Doc 18 §3 table applies: `0` ok · `1` user error (parse/semantic) · `2`
tool/internal error · `3` not found · `4` config error.

### 1.2 `--json` schema-versioning policy

Three machine-readable contracts carry an explicit `schema` field that is
the **stability marker a factory pins against**:

- `fsm verify --json` → `"schema": "fsm-verify/v1"`
- `fsm baseline --record/--check --json` → `"schema": "fsm-trace-diff/v1"`
- the persisted baseline corpus file → `"schema": "fsm-trace/v1"`

The versioning rule is identical for all three (one coherent family):

- **Additive changes keep the SAME major** (`…/v1`). *Additive* = a NEW
  key under an existing object, or a NEW element kind in an existing
  array. A consumer that reads only the keys it knows is **unaffected** —
  JSON object readers ignore unknown keys.
- **A breaking change bumps the major** (`…/v2`): the meaning, type, or
  shape of an EXISTING key changes, a key is removed/renamed, or an
  exit-code's meaning changes.
- **The exit-code family (`0/1/2/3/4`) is itself part of the versioned
  surface** and is held stable across additive revisions.

A factory consumer SHOULD therefore branch on the **major** (`schema`
prefix, e.g. `fsm-verify/v1`) and tolerate unknown keys, rather than
pinning an exact byte shape.

### 1.3 Determinism

`fsm verify --json` and `fsm baseline --json` emit a **single
deterministic JSON object** on stdout: keys are sorted (`BTreeMap`) and
every array is in a stable order by construction (the project's
reproducibility discipline — see the `cmd::verify` / `cmd::baseline`
module docs). Running the same command on the same build against the same
input yields **byte-identical** stdout. The baseline corpus files are
likewise byte-stable (`StepRecord`s use `BTreeMap`/`Vec` in
interpreter-emission order), so capturing the same suite twice on the same
build yields byte-identical corpus files. The CI script in §3 relies on
this: it runs the whole loop twice and `cmp`s the JSON.

---

## 2. The individual steps (copy-paste runnable)

Replace `fsm` with the built binary path if it is not on `$PATH`
(`cargo build -p fsm-cli --bin fsm` ⇒ `target/debug/fsm`).

```sh
# A — the sound machine verifies clean (exit 0).
fsm verify --json examples/verify/clean.fsm
#   → .verdict == "verified", .schema == "fsm-verify/v1",
#     .properties.deadlockFree.result == "holds", $? == 0

# B — the deadlocking machine is caught with a witness (exit 1).
fsm verify --json examples/verify/deadlocks.fsm
#   → .verdict == "property-violated",
#     .properties.deadlockFree.result == "violated",
#     .properties.deadlockFree.counterexample.witness == ["ARM"], $? == 1

# C — honest bound: a too-small ceiling is INCONCLUSIVE, never a false pass.
fsm verify --json --max-states 1 examples/verify/clean.fsm
#   → .verdict == "inconclusive", .bound.hit == true, $? == 2

# D — error buckets stay distinct from a verdict.
fsm verify --json examples/verify/no-such-file.fsm    # $? == 3 (not found)
fsm verify --json some-unparseable.fsm                # $? == 4 (won't compile)

# E — generate C + HAL + IR from the verified model (exit 0).
fsm generate --target c99 --emit-ir examples/verify/clean.fsm \
    --out generated/verify-clean
#   → ThermalController.{c,h,_conf.h,_impl.h} + fsm_hal.h
#     + ThermalController.ir.json, $? == 0

# F — static diagnostics as a JSON array (exit 0, empty array = clean).
fsm check --json examples/verify/clean.fsm
#   → stdout parses as a JSON array, $? == 0

# G — regression oracle: freeze a baseline, then assert no semantic drift.
fsm baseline --record --corpus baselines \
    crates/fsm-cli/tests/fixtures/w3_baseline_suite          # $? == 0
fsm baseline --check  --json --corpus baselines \
    crates/fsm-cli/tests/fixtures/w3_baseline_suite
#   → .schema == "fsm-trace-diff/v1", .verdict == "no-drift", $? == 0
```

---

## 3. The whole loop as ONE CI-shaped script

This is the headless closed loop with **zero gaps**: verify gates the
build, generate produces the firmware artifacts, check is a static
backstop, baseline is the regression oracle, and the negative case
(`deadlocks.fsm`) proves the gate actually fails when it must. It uses
only exit codes + `jq` over `--json` — no interactive prompt, no UI, no
network, no daemon. Run it twice and it is byte-identical.

Save as `ci-verify.sh`, `chmod +x`, run from the repository root.

```sh
#!/usr/bin/env sh
# FSM Studio — headless verify → generate → check → baseline gate.
# Exit 0 = the pipeline passed the gate. Any non-zero = a hard CI failure.
set -eu

FSM="${FSM:-fsm}"                       # override with the built binary path
SUITE="crates/fsm-cli/tests/fixtures/w3_baseline_suite"
WORK="${WORK:-.ci-verify-out}"          # all generated artifacts land here
rm -rf "$WORK"; mkdir -p "$WORK"

fail() { echo "GATE FAILED: $1" >&2; exit 1; }

# Run a command, capture stdout + the REAL exit code (set -e safe).
run() { _out="$1"; shift; set +e; "$@" >"$_out" 2>"$_out.err"; _rc=$?; set -e; }

# ── A. The sound model must verify clean (exit 0, verdict "verified").
run "$WORK/verify-clean.json" "$FSM" verify --json examples/verify/clean.fsm
[ "$_rc" -eq 0 ] || fail "clean.fsm did not exit 0 (got $_rc)"
[ "$(jq -r .verdict        "$WORK/verify-clean.json")" = verified ]      || fail "clean.fsm verdict != verified"
[ "$(jq -r .schema         "$WORK/verify-clean.json")" = fsm-verify/v1 ] || fail "clean.fsm schema != fsm-verify/v1"
[ "$(jq -r .properties.deadlockFree.result "$WORK/verify-clean.json")" = holds ] || fail "clean.fsm not deadlock-free"

# ── B. The deadlocking model must be CAUGHT (exit 1, witness non-empty).
run "$WORK/verify-bad.json" "$FSM" verify --json examples/verify/deadlocks.fsm
[ "$_rc" -eq 1 ] || fail "deadlocks.fsm did not exit 1 (got $_rc) — the gate would have passed an unsafe model!"
[ "$(jq -r .verdict "$WORK/verify-bad.json")" = property-violated ] || fail "deadlocks.fsm verdict != property-violated"
[ "$(jq '.properties.deadlockFree.counterexample.witness | length' "$WORK/verify-bad.json")" -gt 0 ] \
    || fail "deadlocks.fsm produced an empty counterexample witness"

# ── C. Honest bound: a too-small ceiling is INCONCLUSIVE, never a pass.
run "$WORK/verify-bound.json" "$FSM" verify --json --max-states 1 examples/verify/clean.fsm
[ "$_rc" -eq 2 ] || fail "bounded run did not exit 2 (got $_rc) — honest-bound contract broken!"
[ "$(jq -r .verdict  "$WORK/verify-bound.json")" = inconclusive ] || fail "bounded run verdict != inconclusive"
[ "$(jq -r .bound.hit "$WORK/verify-bound.json")" = true ]        || fail "bounded run did not report bound.hit"

# ── D. Error buckets stay distinct from a verdict.
run "$WORK/d-missing" "$FSM" verify --json examples/verify/__no_such_file__.fsm
[ "$_rc" -eq 3 ] || fail "missing input did not exit 3 (got $_rc)"
printf 'language fsm 2.0\nmachine X { initial S state S { on E -> Gone } }\n' > "$WORK/broken.fsm"
run "$WORK/d-broken" "$FSM" verify --json "$WORK/broken.fsm"
[ "$_rc" -eq 4 ] || fail "unparseable input did not exit 4 (got $_rc)"

# ── E. Generate firmware artifacts from the *verified* model.
run "$WORK/gen.log" "$FSM" generate --target c99 --emit-ir \
    examples/verify/clean.fsm --out "$WORK/generated"
[ "$_rc" -eq 0 ] || fail "fsm generate did not exit 0 (got $_rc)"
for f in ThermalController.c ThermalController.h fsm_hal.h ThermalController.ir.json; do
    [ -s "$WORK/generated/$f" ] || fail "generate did not emit $f"
done
jq -e . "$WORK/generated/ThermalController.ir.json" >/dev/null || fail "emitted IR is not valid JSON"

# ── F. Static diagnostics backstop (JSON array; empty = clean).
run "$WORK/check.json" "$FSM" check --json examples/verify/clean.fsm
[ "$_rc" -eq 0 ] || fail "fsm check did not exit 0 (got $_rc)"
[ "$(jq -r 'type' "$WORK/check.json")" = array ] || fail "fsm check --json stdout is not a JSON array"

# ── G. Regression oracle: freeze a baseline, then assert zero drift.
run "$WORK/bl-record" "$FSM" baseline --record --corpus "$WORK/baselines" "$SUITE"
[ "$_rc" -eq 0 ] || fail "baseline --record did not exit 0 (got $_rc)"
run "$WORK/bl-check.json" "$FSM" baseline --check --json --corpus "$WORK/baselines" "$SUITE"
[ "$_rc" -eq 0 ] || fail "baseline --check did not exit 0 (got $_rc)"
[ "$(jq -r .schema  "$WORK/bl-check.json")" = fsm-trace-diff/v1 ] || fail "baseline schema != fsm-trace-diff/v1"
[ "$(jq -r .verdict "$WORK/bl-check.json")" = no-drift ]         || fail "baseline verdict != no-drift"

echo "GATE PASSED — verify→generate→check→baseline closed headless."
```

### Determinism harness (run the loop twice, byte-compare)

```sh
FSM="${FSM:-fsm}"
WORK=.ci-run-1 sh ci-verify.sh
WORK=.ci-run-2 sh ci-verify.sh
# The schema-versioned JSON outputs are byte-identical across runs.
for j in verify-clean verify-bad verify-bound bl-check; do
    cmp ".ci-run-1/$j.json" ".ci-run-2/$j.json" \
        || { echo "NON-DETERMINISTIC: $j.json differs"; exit 1; }
done
echo "DETERMINISTIC — all schema-versioned JSON byte-identical across runs."
```

---

## 4. Why this is factory-complete

- **No interactive step.** Every command is non-interactive; the only
  inputs are file paths and flags.
- **No UI dependency.** No VS Code, no language server, no browser, no
  daemon, no network — `fsm verify` behaves exactly like `fsm check` /
  `fsm generate` (Doc 30: the CLI is the canonical verification surface).
- **Machine-readable.** Every decision a CI needs is a JSON field
  (`.verdict`, `.exitCode`, `.properties.*`, `.bound.hit`) plus the
  process exit code.
- **Deterministic.** Sorted keys + stable arrays ⇒ byte-identical output
  on re-run (the determinism harness proves it).
- **No workflow gaps.** Verify gates → generate produces firmware →
  check backstops → baseline catches semantic regressions; the negative
  case proves the gate fails when it must; bound-hit is honest.
- **Discoverable from docs alone.** The exit-code family and the
  schema-versioning policy are specified here and authoritatively in
  Doc 18 §3 and `docs/25-Integration-Guide.md` — a factory integrator
  needs no source dive.
