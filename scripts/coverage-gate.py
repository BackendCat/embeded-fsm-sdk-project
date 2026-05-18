#!/usr/bin/env python3
# ============================================================================
# Phase-6.0-W4 — the Rust coverage RATCHET GATE engine (Doc 32 §W4).
# ============================================================================
#
# This is the gate `docs/ci/coverage-rust-lane.yml` calls (the
# `on-target-lane.yml` "+ any script it calls" precedent). It is ALSO the
# locally-runnable gate (W5's `make ci-local` invokes the same script — one
# gate, two callers).
#
# The single source of truth for the floors is `coverage-floors.toml` (the
# `tests/conformance/COVERAGE_MAP.md` recorded-artifact analogue, GT-9
# discipline). This script does NOT decide floors — it ENFORCES the
# committed ones and proves it is non-vacuous.
#
# ─────────────────────────────────────────────────────────────────────────
# SUBCOMMANDS
# ─────────────────────────────────────────────────────────────────────────
#   --measure                Run `cargo +nightly llvm-cov --branch` per
#                            package, write per-package JSON to
#                            <covdir>/cov-<pkg>.json. (The MEASURE step the
#                            lane runs; also the local "re-measure to raise
#                            a floor" step.) Doc 32 §W4: floor is measured,
#                            never guessed.
#   --gate                   Read the per-package JSONs + coverage-floors.toml
#                            and FAIL (exit 1) if ANY package's freshly
#                            measured line/branch drops below its floor_*.
#                            (The actual CI gate. Doc 32 §W4 §5.4.)
#   --check-monotonic BASE   Compare the working-tree coverage-floors.toml
#                            against BASE (a git ref's copy) and FAIL if ANY
#                            floor_* was LOWERED (the ratchet only goes up —
#                            Doc 32 §W4 (3) monotonic non-decreasing).
#   --red-proof              NON-VACUITY SELF-TEST (the EXPECTED:73 / W3
#                            deliberate-drop method, Doc 32 §W4 §5.4): prove
#                            the gate logic FAILS on a deliberate
#                            floor-bump-without-new-tests AND on a
#                            coverage-drop, using the IDENTICAL gate
#                            function — then prove it PASSES on the real
#                            inputs (the "restore" half). A gate that
#                            cannot fail is worthless.
#
# Exit code is the gate verdict (0 = pass, 1 = fail) — the lane keys CI on it.

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

# ── Repo layout ────────────────────────────────────────────────────────────
SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
FLOORS_TOML = REPO_ROOT / "coverage-floors.toml"

# The 11 first-party workspace crates (Doc 00 §11.70 / Doc 32 §5 NOTE-1
# list). fsm-lsp is measured via --lib (see coverage-floors.toml
# [exclusions.fsm-lsp-integration-test]).
PACKAGES = [
    "fsm-diagnostics",
    "fsm-lexer",
    "fsm-parser",
    "fsm-ir",
    "fsm-analyzer",
    "fsm-codegen-c",
    "fsm-simulator",
    "fsm-verify",
    "fsm-formatter",
    "fsm-cli",
    "fsm-lsp",
]
# fsm-lsp: measure via --lib (subprocess-stdio integration test is
# instrumentation-flaky — a recorded, justified exclusion, NOT a code defect).
LIB_ONLY = {"fsm-lsp"}
# fsm-lsp branch: a reproduced LLVM-22 getInstantiationGroups SIGSEGV under
# --branch. Line coverage IS measured + gated; only branch is excluded.
BRANCH_EXCLUDED = {"fsm-lsp"}


# ── coverage-floors.toml loader ────────────────────────────────────────────
@dataclass
class Floor:
    package: str
    floor_line: float
    floor_branch: float | None  # None == branch excluded for this package


def load_floors(path: Path = FLOORS_TOML) -> dict[str, Floor]:
    with path.open("rb") as fh:
        doc = tomllib.load(fh)
    out: dict[str, Floor] = {}
    for pkg, tbl in doc.get("package", {}).items():
        fl = tbl["floor_line"]
        fb = tbl.get("floor_branch")
        # A string floor_branch ("EXCLUDED") means the metric is an
        # enumerated, justified exclusion (see [exclusions.*]) — not gated.
        fb_val: float | None
        if isinstance(fb, str):
            fb_val = None
        else:
            fb_val = float(fb)
        out[pkg] = Floor(package=pkg, floor_line=float(fl), floor_branch=fb_val)
    return out


# ── cargo-llvm-cov per-package measurement ─────────────────────────────────
def measure(covdir: Path) -> int:
    """Run `cargo +nightly llvm-cov --branch` per package; write
    <covdir>/cov-<pkg>.json. Doc 32 §W4: the floor is MEASURED, never
    guessed; this is that measurement. Returns 0 on success, 1 if a
    package that is NOT a recorded exclusion fails to measure."""
    covdir.mkdir(parents=True, exist_ok=True)
    failures: list[str] = []
    for pkg in PACKAGES:
        out_json = covdir / f"cov-{pkg}.json"
        base = [
            "cargo",
            "+nightly",
            "llvm-cov",
            "--no-cfg-coverage",
            "-p",
            pkg,
            "--json",
            "--summary-only",
            "--output-path",
            str(out_json),
        ]
        if pkg in LIB_ONLY:
            base.insert(base.index("-p"), "--lib")
        # `cargo llvm-cov clean` between packages keeps the object set small
        # (the documented per-crate workflow) AND disk-protective on a
        # tight box (GT-10): each package run is isolated, no accumulating
        # instrumented test binaries.
        subprocess.run(
            ["cargo", "+nightly", "llvm-cov", "clean", "--workspace"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
        )

        branch_ok = False
        if pkg not in BRANCH_EXCLUDED:
            # Try WITH --branch first (the Doc 32 §W4 branch-coverage
            # requirement). cargo-llvm-cov --branch is nightly-only.
            r = subprocess.run(
                base[:3] + ["--branch"] + base[3:],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            branch_ok = r.returncode == 0 and out_json.is_file()
            if not branch_ok:
                # SELF-HEALING DISCLOSURE (Doc 32 §W4): a BRANCH_EXCLUDED
                # package is excluded for a reproduced LLVM-22 bug. If
                # --branch SUCCEEDS for a package we did NOT expect to
                # fail, that is a real gate gap — fail loudly.
                print(
                    f"::error::cov-measure: `{pkg}` --branch FAILED but is "
                    f"NOT in BRANCH_EXCLUDED — investigate (rc={r.returncode})",
                    file=sys.stderr,
                )
        if pkg in BRANCH_EXCLUDED or not branch_ok:
            # Line/region-only measurement (no --branch). For
            # BRANCH_EXCLUDED packages this is the intended path; the
            # exclusion is recorded + justified in coverage-floors.toml.
            r = subprocess.run(
                base,
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            if pkg in BRANCH_EXCLUDED:
                # LOUD self-heal probe: re-attempt --branch so that the
                # day the upstream LLVM-22 bug is fixed, the lane SHOUTS
                # "fsm-lsp --branch now works — remove the exclusion".
                probe = subprocess.run(
                    base[:3] + ["--branch"] + base[3:] + ["--output-path",
                    str(covdir / f"_probe-{pkg}.json")],
                    cwd=REPO_ROOT,
                    check=False,
                    capture_output=True,
                    text=True,
                )
                if probe.returncode == 0 and (covdir / f"_probe-{pkg}.json").is_file():
                    print(
                        f"::warning::cov-measure: `{pkg}` --branch NOW "
                        f"SUCCEEDS — the LLVM-22 getInstantiationGroups bug "
                        f"appears fixed. REMOVE the "
                        f"[exclusions.fsm-lsp-branch] entry and add a "
                        f"measured floor_branch (Doc 32 §W4 self-healing "
                        f"disclosure).",
                        file=sys.stderr,
                    )
            if r.returncode != 0 or not out_json.is_file():
                failures.append(
                    f"{pkg}: cargo-llvm-cov failed to produce {out_json.name} "
                    f"(rc={r.returncode}); NOT a recorded exclusion"
                )
                continue
        pct = _read_json_pct(out_json)
        bstr = (
            "EXCLUDED(LLVM-22)" if pkg in BRANCH_EXCLUDED else f"{pct[1]:.2f}"
        )
        print(f"  measured {pkg}: line {pct[0]:.2f}% branch {bstr}")
    if failures:
        for f in failures:
            print(f"::error::cov-measure: {f}", file=sys.stderr)
        return 1
    return 0


def _read_json_pct(p: Path) -> tuple[float, float]:
    d = json.loads(p.read_text())
    t = d["data"][0]["totals"]
    return float(t["lines"]["percent"]), float(t["branches"]["percent"])


# ── THE GATE (Doc 32 §W4 §5.4) ─────────────────────────────────────────────
@dataclass
class Measured:
    package: str
    line: float
    branch: float | None  # None == branch metric not measured (excluded pkg)


def read_measured(covdir: Path) -> dict[str, Measured]:
    out: dict[str, Measured] = {}
    for pkg in PACKAGES:
        p = covdir / f"cov-{pkg}.json"
        if not p.is_file():
            raise FileNotFoundError(
                f"missing {p} — run `--measure` first (the gate enforces a "
                f"MEASURED number, never a guessed one — Doc 32 §W4)"
            )
        line, branch = _read_json_pct(p)
        b: float | None = None if pkg in BRANCH_EXCLUDED else branch
        out[pkg] = Measured(package=pkg, line=line, branch=b)
    return out


def evaluate_gate(
    measured: dict[str, Measured], floors: dict[str, Floor]
) -> list[str]:
    """THE pure gate predicate — factored so --red-proof exercises the
    IDENTICAL logic the real gate uses (the W3 `uncovered_codes` factoring;
    a RED-proof that re-implements the logic proves nothing). Returns the
    list of floor VIOLATIONS; empty == gate passes."""
    violations: list[str] = []
    for pkg in PACKAGES:
        m = measured.get(pkg)
        f = floors.get(pkg)
        if m is None:
            violations.append(f"{pkg}: no measured data")
            continue
        if f is None:
            violations.append(
                f"{pkg}: no floor in coverage-floors.toml (every package "
                f"MUST have a recorded floor)"
            )
            continue
        if m.line < f.floor_line:
            violations.append(
                f"{pkg}: LINE {m.line:.2f}% < floor {f.floor_line:.0f}% "
                f"(dropped below the ratchet floor)"
            )
        if f.floor_branch is not None:
            if m.branch is None:
                violations.append(
                    f"{pkg}: branch floor {f.floor_branch:.0f}% is set but "
                    f"branch was not measured"
                )
            elif m.branch < f.floor_branch:
                violations.append(
                    f"{pkg}: BRANCH {m.branch:.2f}% < floor "
                    f"{f.floor_branch:.0f}% (dropped below the ratchet floor)"
                )
    return violations


def cmd_gate(covdir: Path) -> int:
    floors = load_floors()
    measured = read_measured(covdir)
    violations = evaluate_gate(measured, floors)
    print("── Coverage ratchet gate (Doc 32 §W4) ──")
    for pkg in PACKAGES:
        m, f = measured[pkg], floors.get(pkg)
        fb = (
            "EXCLUDED"
            if (f is None or f.floor_branch is None)
            else f"{f.floor_branch:.0f}"
        )
        mb = "EXCLUDED" if m.branch is None else f"{m.branch:.2f}"
        fl = "?" if f is None else f"{f.floor_line:.0f}"
        print(
            f"  {pkg:<16} line {m.line:6.2f}% (floor {fl:>3}) "
            f"branch {mb:>9} (floor {fb:>8})"
        )
    if violations:
        print("\n::error::COVERAGE RATCHET GATE FAILED — below floor:")
        for v in violations:
            print(f"::error::  {v}", file=sys.stderr)
        print(
            "\nA package dropped below its monotonic ratchet floor. Either a "
            "regression removed coverage (fix it) or — if intentional and "
            "behaviourally justified — this needs review; the floor NEVER "
            "auto-lowers (Doc 32 §W4 (3))."
        )
        return 1
    print("\nGate OK: every package at/above its ratcheted floor.")
    return 0


# ── MONOTONIC RATCHET CHECK (Doc 32 §W4 (3)) ───────────────────────────────
def cmd_check_monotonic(base_ref: str) -> int:
    """FAIL if any floor_* in the working tree is BELOW the BASE ref's
    committed value. The ratchet only ascends (Doc 32 §W4 (3)). New
    packages (absent in base) are allowed; a removed package is flagged."""
    try:
        base_blob = subprocess.run(
            ["git", "show", f"{base_ref}:coverage-floors.toml"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except subprocess.CalledProcessError:
        # No baseline yet (W4 introduces the file): nothing to ratchet
        # against — the FIRST commit IS the baseline. Honest, not a pass-by-
        # accident: print it.
        print(
            f"note: coverage-floors.toml absent at {base_ref} (W4 introduces "
            f"it) — this commit establishes the ratchet baseline; no "
            f"monotonic check possible yet (correct, not a skipped gate)."
        )
        return 0
    base = tomllib.loads(base_blob).get("package", {})
    cur = tomllib.loads(FLOORS_TOML.read_text()).get("package", {})
    regressions: list[str] = []
    for pkg, btbl in base.items():
        if pkg not in cur:
            regressions.append(f"{pkg}: REMOVED from coverage-floors.toml")
            continue
        for metric in ("floor_line", "floor_branch"):
            bv, cv = btbl.get(metric), cur[pkg].get(metric)
            # String floor ("EXCLUDED") -> not a numeric ratchet point.
            if isinstance(bv, str) or isinstance(cv, str):
                if isinstance(bv, (int, float)) and isinstance(cv, str):
                    regressions.append(
                        f"{pkg}.{metric}: was numeric {bv}, now EXCLUDED "
                        f"(de-gating a metric is a ratchet regression — "
                        f"needs an enumerated justification, not a silent "
                        f"downgrade)"
                    )
                continue
            if cv is None or bv is None:
                continue
            if float(cv) < float(bv):
                regressions.append(
                    f"{pkg}.{metric}: LOWERED {bv} -> {cv} (the ratchet is "
                    f"monotonic non-decreasing — a floor may only rise; "
                    f"Doc 32 §W4 (3))"
                )
    if regressions:
        print("::error::MONOTONIC RATCHET VIOLATION:")
        for r in regressions:
            print(f"::error::  {r}", file=sys.stderr)
        return 1
    print(f"Monotonic ratchet OK vs {base_ref}: no floor was lowered.")
    return 0


# ── NON-VACUITY RED-PROOF (Doc 32 §W4 §5.4; the W3 / EXPECTED:73 method) ────
def cmd_red_proof(covdir: Path) -> int:
    """Prove the gate is NON-VACUOUS using the IDENTICAL `evaluate_gate`
    the real gate uses (the W3 `red_proof` deliberate-drop method — a
    RED-proof that re-implements the predicate proves nothing). Two RED
    halves + one GREEN ("restore") half:

      RED-1  a deliberate floor-BUMP-without-new-tests (raise a real
             package's floor ABOVE its measured value) MUST make the gate
             FAIL — proves a "ratchet up without earning it" is rejected.
      RED-2  a deliberate coverage-DROP (synthetically lower a package's
             measured number below its floor) MUST make the gate FAIL —
             proves a real regression is caught.
      GREEN  the UN-tampered real inputs MUST pass — proves the gate is
             not stuck-red (the "restore" half).
    """
    floors = load_floors()
    measured = read_measured(covdir)

    # GREEN half — the real inputs pass (precondition; "restore").
    real = evaluate_gate(measured, floors)
    if real:
        print(
            "::error::RED-PROOF precondition FAILED: the real inputs already "
            "violate the gate — cannot run the non-vacuity proof on a "
            "broken baseline:",
            file=sys.stderr,
        )
        for v in real:
            print(f"::error::  {v}", file=sys.stderr)
        return 1

    # Pick a real package that has a numeric branch floor (so both halves
    # are exercisable on a genuine entry).
    victim = next(
        p
        for p in PACKAGES
        if floors[p].floor_branch is not None and measured[p].branch is not None
    )

    # RED-1: floor-BUMP-without-new-tests. Raise victim's line floor to
    # ceil(measured)+1 — i.e. demand MORE than the suite produces, with NO
    # new tests. The gate MUST fail (you cannot ratchet up unearned).
    bumped = dict(floors)
    m = measured[victim]
    bumped[victim] = Floor(
        package=victim,
        floor_line=math.floor(m.line) + 1.0,  # strictly above measured
        floor_branch=floors[victim].floor_branch,
    )
    red1 = evaluate_gate(measured, bumped)
    if not any(victim in v and "LINE" in v for v in red1):
        print(
            f"::error::RED-PROOF FAILED (gate VACUOUS): bumping {victim}'s "
            f"line floor to {math.floor(m.line)+1} (above its measured "
            f"{m.line:.2f}%, with NO new tests) did NOT fail the gate. A "
            f"gate that cannot reject an unearned ratchet-up is worthless.",
            file=sys.stderr,
        )
        return 1

    # RED-2: coverage-DROP. Synthetically drop victim's measured branch to
    # floor_branch - 5 (a regression). The gate MUST fail.
    dropped = dict(measured)
    fb = floors[victim].floor_branch
    assert fb is not None
    dropped[victim] = Measured(
        package=victim, line=m.line, branch=max(0.0, fb - 5.0)
    )
    red2 = evaluate_gate(dropped, floors)
    if not any(victim in v and "BRANCH" in v for v in red2):
        print(
            f"::error::RED-PROOF FAILED (gate VACUOUS): synthetically "
            f"dropping {victim}'s branch coverage to {max(0.0, fb-5.0):.2f}% "
            f"(below its floor {fb:.0f}%) did NOT fail the gate. A gate that "
            f"cannot catch a coverage regression is worthless.",
            file=sys.stderr,
        )
        return 1

    print(
        "RED-PROOF OK (gate is NON-VACUOUS, proven via the IDENTICAL "
        "evaluate_gate the real gate uses):\n"
        f"  RED-1  floor-bump-without-tests on `{victim}` "
        f"(line floor -> {math.floor(m.line)+1} > measured {m.line:.2f}%) "
        f"=> gate FAILS  ✓\n"
        f"  RED-2  coverage-drop on `{victim}` "
        f"(branch -> {max(0.0, fb-5.0):.2f}% < floor {fb:.0f}%) "
        f"=> gate FAILS  ✓\n"
        f"  GREEN  un-tampered real inputs => gate PASSES  ✓\n"
        "A deliberate unearned ratchet-up AND a deliberate coverage-drop "
        "are BOTH caught; the real tree is green (Doc 32 §W4 §5.4)."
    )
    return 0


# ── CLI ────────────────────────────────────────────────────────────────────
def main() -> int:
    ap = argparse.ArgumentParser(
        description="Phase-6.0-W4 Rust coverage ratchet gate (Doc 32 §W4)."
    )
    ap.add_argument(
        "--covdir",
        default=str(REPO_ROOT / "target" / "w4-coverage"),
        help="dir for per-package cov-<pkg>.json (default target/w4-coverage)",
    )
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--measure", action="store_true")
    g.add_argument("--gate", action="store_true")
    g.add_argument("--red-proof", action="store_true")
    g.add_argument(
        "--check-monotonic",
        metavar="BASE_REF",
        help="git ref to ratchet-compare coverage-floors.toml against",
    )
    a = ap.parse_args()
    covdir = Path(a.covdir)

    if a.measure:
        return measure(covdir)
    if a.gate:
        return cmd_gate(covdir)
    if a.red_proof:
        return cmd_red_proof(covdir)
    if a.check_monotonic:
        return cmd_check_monotonic(a.check_monotonic)
    return 2


if __name__ == "__main__":
    sys.exit(main())
