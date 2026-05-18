#!/usr/bin/env python3
# Phase-6.0-W5 — the new-lanes' workflow-lint (Doc 32 §W5 (1e) / §5.4).
#
# WHY a bespoke linter, not `yamllint`/`actionlint`: the dev box is
# disk-tight and `act`/`yamllint`/`actionlint` are deliberately NOT
# installed on it (Doc 32 GT-10 / OWNER-2 — the no-box-installs
# constraint). PyYAML 6.x IS present (a stdlib-adjacent dep already in
# the box's Python). This lint asserts the W5-added lanes are
# (a) YAML-PARSEABLE and (b) STRUCTURALLY a valid GitHub-Actions job map
# that STRUCTURALLY REFERENCES the real W2/W3/W4 artifacts (Doc 32 §5.4:
# "the new ci.yml lanes are workflow-lint-clean and structurally
# reference the W2/W3/W4 artifacts" — not re-implementations). It is
# INVOCATION-only: it defines NO step/transition/guard/oracle semantics
# (the W6 keystone-no-fork audit negative-greps this file → ∅; see the
# KEYSTONE note at the bottom).
#
# It also re-derives + asserts the GT-1 invariant: the `build`(matrix) +
# `sca` jobs are byte-identical to the base ref (the `--prove-untouched`
# mode) — so a regression that silently edits them fails the lint.
#
# Exit non-zero on ANY structural defect (the §5.4 "actually gates"
# discipline — this lint is one of the `make ci-local` sub-gates).

import argparse
import subprocess
import sys
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent
CI_YML = REPO_ROOT / ".github" / "workflows" / "ci.yml"

# The two pre-existing jobs that MUST stay byte-untouched (Doc 32 GT-1).
FROZEN_JOBS = ("build", "sca")

# The 4 W5-added additive-isolated lanes (Doc 32 §W5 (1)).
W5_LANES = ("conformance", "extension-host", "coverage-rust", "on-target")

# Each W5 lane must STRUCTURALLY reference its real W2/W3/W4 artifact
# (Doc 32 §5.4 — a lane that does not invoke the real mechanism is a
# re-implementation / a no-op, the cardinal "symbol-presence ≠
# acceptance" failure). Matched as a substring anywhere in the job's
# rendered `run:` step bodies.
LANE_ARTIFACT_REFS = {
    "conformance": [
        # The W3 build-failing lock test target.
        "--test conformance_code_coverage_lock",
        "fsm-cli",
    ],
    "extension-host": [
        # The W4 c8 ratchet (npm scripts) + the v1.5 xvfb host harness.
        "test:coverage",
        "coverage:gate",
        "xvfb-run",
    ],
    "coverage-rust": [
        # The W4 gate engine + the additive-nightly cargo-llvm-cov.
        "scripts/coverage-gate.py",
        "cargo-llvm-cov",
    ],
    "on-target": [
        # The W2 QEMU differential test + the CI-runner-only ARM/QEMU.
        "--test on_target_qemu_differential",
        "qemu-system-arm",
        "gcc-arm-none-eabi",
    ],
}

# Artifact files the W5 lanes COMPOSE — they must exist on disk (a lane
# that references a deleted artifact is broken even if the YAML parses).
REQUIRED_ARTIFACT_FILES = (
    "crates/fsm-cli/tests/conformance_code_coverage_lock.rs",  # W3
    "crates/fsm-simulator/tests/on_target_qemu_differential.rs",  # W2
    "scripts/coverage-gate.py",  # W4
    "docs/ci/on-target-lane.yml",  # W2 source-of-truth fragment
    "docs/ci/coverage-rust-lane.yml",  # W4 source-of-truth fragment
    "editors/vscode/.c8rc.json",  # W4 c8 floors
)


def fail(msg: str) -> None:
    print(f"workflow-lint: FAIL — {msg}", file=sys.stderr)
    sys.exit(1)


def _render_run_bodies(job: dict) -> str:
    """Concatenate every step's `run:` body (the only place a lane
    INVOKES a mechanism). Pure string aggregation — no semantics."""
    parts = []
    for step in job.get("steps", []) or []:
        if isinstance(step, dict):
            run = step.get("run")
            if isinstance(run, str):
                parts.append(run)
            name = step.get("name")
            if isinstance(name, str):
                parts.append(name)
            uses = step.get("uses")
            if isinstance(uses, str):
                parts.append(uses)
    return "\n".join(parts)


def lint_structure() -> None:
    if not CI_YML.is_file():
        fail(f"{CI_YML} does not exist")

    raw = CI_YML.read_text()

    # (a) YAML-PARSEABLE. GitHub Actions parses `on:` to the YAML 1.1
    # boolean True — that is EXPECTED and correct (the real Actions
    # runner does the same); we look it up by both forms.
    try:
        doc = yaml.safe_load(raw)
    except yaml.YAMLError as exc:
        fail(f"ci.yml is not valid YAML: {exc}")

    if not isinstance(doc, dict):
        fail("ci.yml top-level is not a mapping")

    # (b) STRUCTURAL: required top-level keys.
    if "jobs" not in doc or not isinstance(doc["jobs"], dict):
        fail("ci.yml has no `jobs:` mapping")
    on_key = doc.get("on", doc.get(True))
    if on_key is None:
        fail("ci.yml has no `on:` trigger")

    jobs = doc["jobs"]

    # The 2 frozen jobs MUST still be present (a regression that
    # deletes/renames them is a GT-1 violation even before the
    # byte-diff check).
    for j in FROZEN_JOBS:
        if j not in jobs:
            fail(f"the frozen `{j}` job is missing (GT-1 violation)")

    # All 4 W5 lanes MUST be present, each a valid job map.
    for lane in W5_LANES:
        if lane not in jobs:
            fail(f"the W5 lane `{lane}` is missing")
        job = jobs[lane]
        if not isinstance(job, dict):
            fail(f"lane `{lane}` is not a job mapping")
        if "runs-on" not in job:
            fail(f"lane `{lane}` has no `runs-on`")
        if not job.get("steps"):
            fail(f"lane `{lane}` has no `steps`")
        # ADDITIVE-ISOLATED (the `sca` precedent, Doc 32 GT-1 / §6 H4):
        # a W5 lane must NOT `needs:` the build matrix (that would gate
        # the 1.75 matrix on it — the exact anti-pattern the
        # additive-isolated-job design forbids).
        needs = job.get("needs")
        if needs:
            needs_list = [needs] if isinstance(needs, str) else list(needs)
            if "build" in needs_list:
                fail(
                    f"lane `{lane}` has `needs: build` — it would gate the "
                    f"1.75 matrix; W5 lanes MUST be additive-isolated "
                    f"(the `sca` precedent, GT-1)"
                )

    # (c) STRUCTURAL-REFERENCE: every W5 lane invokes its REAL
    # W2/W3/W4 artifact (Doc 32 §5.4 — not a re-implementation/no-op).
    for lane, needles in LANE_ARTIFACT_REFS.items():
        body = _render_run_bodies(jobs[lane])
        for needle in needles:
            if needle not in body:
                fail(
                    f"lane `{lane}` does not structurally reference its "
                    f"W2/W3/W4 artifact (missing `{needle}` in its "
                    f"run/name/uses steps) — a lane that does not invoke "
                    f"the real mechanism is a re-implementation/no-op "
                    f"(Doc 32 §5.4)"
                )

    # The composed artifacts must exist on disk.
    for rel in REQUIRED_ARTIFACT_FILES:
        if not (REPO_ROOT / rel).is_file():
            fail(
                f"the W5 lanes compose `{rel}` but it does not exist "
                f"(a lane referencing a missing artifact is broken)"
            )

    print(
        "workflow-lint: OK — ci.yml parses; the 2 frozen jobs "
        f"({', '.join(FROZEN_JOBS)}) + the 4 W5 lanes "
        f"({', '.join(W5_LANES)}) are structurally valid, "
        "additive-isolated, and each W5 lane structurally references "
        "its real W2/W3/W4 artifact (no re-implementations)."
    )


def prove_untouched(base_ref: str) -> None:
    """GT-1: re-derive that the `build`+`sca` job text is byte-identical
    to the base ref. The two jobs span ci.yml lines 1..N where line N is
    the last line of the base ci.yml (the W5 banner starts at N+1). We
    diff the base file (whole) vs HEAD's first len(base) lines."""
    try:
        base_text = subprocess.run(
            ["git", "show", f"{base_ref}:.github/workflows/ci.yml"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except subprocess.CalledProcessError as exc:
        fail(f"cannot read ci.yml at {base_ref}: {exc.stderr.strip()}")

    base_lines = base_text.splitlines()
    head_lines = CI_YML.read_text().splitlines()
    head_prefix = head_lines[: len(base_lines)]

    if head_prefix != base_lines:
        # Find the first differing line for a precise message.
        for i, (b, h) in enumerate(zip(base_lines, head_prefix), start=1):
            if b != h:
                fail(
                    f"GT-1 VIOLATION: ci.yml line {i} differs from "
                    f"{base_ref} within the frozen build+sca block\n"
                    f"  base: {b!r}\n  head: {h!r}"
                )
        fail(f"GT-1 VIOLATION: the frozen build+sca block diverged from {base_ref}")

    print(
        f"workflow-lint: OK — the `build`+`sca` jobs are BYTE-IDENTICAL "
        f"to {base_ref} (GT-1: the first {len(base_lines)} lines of "
        f"ci.yml == the entire base ci.yml; W5 only APPENDS lanes)."
    )


def main() -> None:
    ap = argparse.ArgumentParser(
        description="W5 new-lanes workflow-lint (Doc 32 §W5 (1e))."
    )
    ap.add_argument(
        "--prove-untouched",
        metavar="BASE_REF",
        help="also assert build+sca are byte-identical to BASE_REF (GT-1)",
    )
    args = ap.parse_args()

    lint_structure()
    if args.prove_untouched:
        prove_untouched(args.prove_untouched)


# ───────────────────────────────────────────────────────────────────────
# KEYSTONE — no-fork attestation (Doc 32 §2; the W6 audit re-derives this
# from source). This script defines NO step / transition-selection /
# guard-eval / deadlock / trace-recording / oracle semantics. It only
# (1) parses YAML, (2) checks job structure, (3) substring-checks that
# the lanes invoke the W2/W3/W4 mechanisms, (4) git-diffs the frozen
# jobs. The differential oracle remains exclusively
# `fsm_simulator::execute_trace` (in fsm-simulator, NOT here). A
# self-negative-grep for step/transition/guard/oracle *definitions* on
# non-comment lines of this file returns ∅ by construction.
# ───────────────────────────────────────────────────────────────────────
if __name__ == "__main__":
    main()
