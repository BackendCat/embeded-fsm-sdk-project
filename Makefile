# ═══════════════════════════════════════════════════════════════════════
#  Phase-6.0-W5 — the one-command `make ci-local` quality gate
#  (Doc 32 §W5 deliverable (2) + §5 — an OWNER EXPLICIT ASK, a
#  FIRST-CLASS deliverable). `make` IS on the box.
# ═══════════════════════════════════════════════════════════════════════
#
# `make ci-local` runs EVERYTHING runnable WITHOUT a push and EXITS
# NON-ZERO on a broken tree (Doc 32 §5.4 — symbol-presence is NOT
# acceptance; the non-vacuity RED-proof is in the W5 completion report):
#   (a) the per-wave QUAD on the pinned 1.75
#   (b) CONFORMANCE — the W3 build-failing lock + corpus
#   (c) the xvfb-run Extension-Host JS lane + the W4 c8 ratchet
#   (d) an `act` dry-run of the LINUX build/sca lanes ONLY
#       (honest skip-pending-install if `act` absent — NEVER silent-pass)
#   (e) a workflow-lint of the W5 lanes (+ the GT-1 build/sca
#       byte-untouched re-derivation)
#
# It then prints the VERBATIM honest-scope (scripts/ci-local-honest-scope
# .txt — the H9 anti-mis-sell discipline): it does NOT claim to have run
# the CI-runner-only on-target/coverage matrix locally; G9 (the owner's
# push) is honestly framed as a confirmation/switch, not a redundant leap.
#
# KEYSTONE (Doc 32 §2 — the W6 audit re-derives this from source): this
# Makefile defines NO step/transition/guard/deadlock/oracle semantics. It
# only INVOKES the W2/W3/W4 mechanisms (`cargo test` of the shipped
# binaries, the npm c8 scripts, scripts/coverage-gate.py,
# scripts/workflow-lint.py). The differential oracle remains exclusively
# `fsm_simulator::execute_trace`. A self-negative-grep for
# step/transition/guard/oracle DEFINITIONS on non-comment lines of this
# file returns ∅ by construction.
#
# DESIGN NOTE — robust failure propagation: every recipe is a single
# bash invocation with `set -euo pipefail`, and `.SHELLFLAGS`/`SHELL`
# below make ALL recipes fail-fast. The ci-local target is `.PHONY` and
# its prerequisites are ordered (`ci-quad ci-conformance ci-ext-host
# ci-act-dryrun ci-workflow-lint`); make stops at the FIRST failing
# prerequisite (default serial make), so a broken tree exits non-zero
# before the honest-scope print — proven in the §5.4 RED-proof.

SHELL := /usr/bin/env bash
.SHELLFLAGS := -eu -o pipefail -c

# Base ref for the GT-1 build+sca byte-untouched re-derivation. The W5
# worktree branches off main 64a3233; override on the CLI if needed
# (`make ci-local BASE_REF=<sha>`).
BASE_REF ?= 64a3233

# The pinned-toolchain assertion the quad runs under (the
# toolchain-probe-trap discipline — assert from INSIDE the repo).
EXPECTED_CHANNEL := 1.75

.DEFAULT_GOAL := help

.PHONY: help ci-local ci-toolchain-assert ci-quad ci-conformance \
        ci-ext-host ci-act-dryrun ci-workflow-lint ci-honest-scope

help:
	@echo "Phase-6.0-W5 local quality gate:"
	@echo "  make ci-local   — run the full local gate (quad + conformance"
	@echo "                     + xvfb ext-host/c8 + act-dryrun + lint),"
	@echo "                     then print the VERBATIM honest scope."
	@echo "                     Exits non-zero on a broken tree."
	@echo "  Individual lanes: ci-quad ci-conformance ci-ext-host"
	@echo "                    ci-act-dryrun ci-workflow-lint"
	@echo "  Override the GT-1 base ref: make ci-local BASE_REF=<sha>"

# The owner's one command. Prerequisites run in order; make stops at the
# first failure (so a broken tree exits non-zero). The honest-scope is
# the LAST step and only prints after every gate passed — it can never
# accompany a green claim for a tree that failed.
ci-local: ci-toolchain-assert ci-quad ci-conformance ci-ext-host \
          ci-act-dryrun ci-workflow-lint ci-honest-scope
	@echo ""
	@echo "═══════════════════════════════════════════════════════════════"
	@echo "  make ci-local: ALL LOCAL GATES GREEN."
	@echo "  Scope is exactly as printed above — the CI-runner-only"
	@echo "  on-target/coverage matrix was NOT run locally (by design);"
	@echo "  G9 (push) confirms it. This is NOT a no-op: see the W5"
	@echo "  completion report's non-vacuity RED-proof transcript."
	@echo "═══════════════════════════════════════════════════════════════"

# Toolchain-probe-trap (Doc 32 GT-2): assert the pinned channel FROM
# INSIDE the repo. A bare `rustc`→1.95 is the BENIGN box default; the
# pin is only honoured via rust-toolchain.toml read from the repo. Fail
# loudly if the active toolchain is NOT the pinned 1.75.
ci-toolchain-assert:
	@echo "── [pre] toolchain-probe-trap assertion ──────────────────────"
	ACTIVE="$$(rustup show active-toolchain)"; \
	echo "active-toolchain (from repo): $$ACTIVE"; \
	case "$$ACTIVE" in \
	  $(EXPECTED_CHANNEL).*) \
	    echo "OK — pinned $(EXPECTED_CHANNEL) is active (probe-trap honoured)";; \
	  *) \
	    echo "FAIL — active toolchain is not the pinned $(EXPECTED_CHANNEL):" \
	         "$$ACTIVE" >&2; \
	    exit 1;; \
	esac

# (a) THE QUAD on the pinned 1.75 — the same four commands the CI
# `build` matrix runs (kept in lock-step with ci.yml's build job). The
# W1 host generated-C trace differential rides in `cargo test --workspace`
# (`codegen_equivalence_smoke` in fsm-simulator) — that is the
# locally-provable on-target *logic* smoke (Doc 32 §W5 honest-scope).
ci-quad:
	@echo "── (a) QUAD — fmt / clippy / build / test on pinned 1.75 ─────"
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo build --workspace
	cargo test --workspace

# (b) CONFORMANCE — the W3 live-enum-derived BUILD-FAILING lock + its
# in-suite RED-proof + the COVERAGE_MAP byte-derivable check + the
# corpus exact-set discipline (NOT re-implemented — composes the W3
# artifact `conformance_code_coverage_lock`).
ci-conformance:
	@echo "── (b) CONFORMANCE — W3 build-failing lock + corpus ──────────"
	cargo test -p fsm-cli --test conformance_code_coverage_lock -- --nocapture

# (c) The xvfb-run @vscode/test-electron EXTENSION-HOST suite UNDER the
# W4 `c8` instrumentation + the `c8` per-area ratchet gate (GT-11 — the
# v1.5 xvfb host already shipped; W4 added c8 + the npm scripts; this
# COMPOSES them). `npm ci` first (lockfile-exact, the box's vscode
# node_modules may be absent). The Electron download is pinned
# out-of-tree at ~/.vscode-test by runTest.ts (NEVER under /root/dev —
# the disk-tight-box discipline, GT-10).
ci-ext-host:
	@echo "── (c) EXTENSION-HOST — xvfb @vscode/test-electron + c8 ──────"
	cd editors/vscode && npm ci
	cd editors/vscode && xvfb-run -a npm run test:coverage
	cd editors/vscode && npm run coverage:gate

# (d) An `act` DRY-RUN of the LINUX `build` + `sca` lanes ONLY (NOT the
# heavy on-target/coverage matrix; Doc 32 §W5 honest-scope / GT-10).
# `act` is ABSENT on this box but installable (Docker IS present). If
# absent: report SKIPPED — pending act install and CONTINUE (an honest
# skip, the `gcc_compile.rs` skip-notice precedent) — NEVER a silent
# pass, and we do NOT install `act` on this box. `-n` = dry-run (parse +
# plan the workflow graph, run nothing); `-j` scopes to the named job.
ci-act-dryrun:
	@echo "── (d) act DRY-RUN — Linux build/sca lanes only ──────────────"
	if command -v act >/dev/null 2>&1; then \
	  echo "act present — dry-running the Linux build + sca lanes"; \
	  echo "  (NOT the on-target/coverage matrix — CI-runner-only)"; \
	  act -n -j build  -P ubuntu-latest=catthehacker/ubuntu:act-latest; \
	  act -n -j sca    -P ubuntu-latest=catthehacker/ubuntu:act-latest; \
	  echo "act dry-run OK (build + sca workflow graph parses + plans)"; \
	else \
	  echo "SKIPPED — pending act install."; \
	  echo "  act is ABSENT on this box BY DESIGN (Doc 32 GT-10 / the"; \
	  echo "  no-box-installs constraint: act pulls multi-GB runner"; \
	  echo "  images; the box is disk-tight). Docker IS present, so act"; \
	  echo "  runs once installed. This lane is HONESTLY SKIPPED, NOT"; \
	  echo "  silently passed; \`make ci-local\` did NOT install act."; \
	  echo "  The new-lanes' YAML is still validated by (e) below."; \
	fi

# (e) WORKFLOW-LINT of the W5-added lanes (Doc 32 §W5 (1e) / §5.4):
# ci.yml parses, the 4 lanes are structurally valid + additive-isolated
# + each structurally references its real W2/W3/W4 artifact, AND (the
# GT-1 re-derivation) the frozen `build`+`sca` jobs are byte-identical
# to BASE_REF. PyYAML is on the box; no `yamllint`/`actionlint` install.
ci-workflow-lint:
	@echo "── (e) WORKFLOW-LINT — W5 lanes + GT-1 build/sca untouched ───"
	python3 scripts/workflow-lint.py --prove-untouched "$(BASE_REF)"

# The VERBATIM honest scope (the H9 anti-mis-sell discipline). Printed
# only AFTER every gate above passed (it is a prerequisite of `ci-local`
# AFTER the five lanes). It states verbatim what was/was-not run locally.
ci-honest-scope:
	@echo ""
	@cat scripts/ci-local-honest-scope.txt
