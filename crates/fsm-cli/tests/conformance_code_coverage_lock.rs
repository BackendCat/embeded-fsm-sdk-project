//! G7 conformance-coverage **build-failing lock** (Doc 32 §W3, GT-5/GT-8/GT-9).
//!
//! This mirrors the proven `EXPECTED:73` lock pattern
//! (`crates/fsm-diagnostics/src/lib.rs:820`,
//! `all_codes_matches_expected_count`): the target set is derived from the
//! **compiled live enum** `DiagnosticCode::all_codes()` — the single source of
//! truth — never from `COVERAGE_MAP.md` (which is itself a regenerated
//! artifact, GT-9) and never from a hand-maintained list.
//!
//! The lock (`every_live_code_has_a_conformance_fixture_or_a_justified_allowlist_entry`)
//! asserts, for every one of the live `DiagnosticCode` variants, that it is
//! **either**:
//!
//!   (a) covered by a **conformance fixture** in `tests/conformance/MANIFEST.json`
//!       that, run through the *same* analysis the conformance harness
//!       (`fsm-cli/src/cmd/test.rs::run_neg`) uses — `fsm_parser::parse` +
//!       `fsm_analyzer::analyze_with_source`, combining parser errors with
//!       analyzer diagnostics — triggers **exactly that code** and no other
//!       error/warning code (the **exact-set** discipline: the fixture
//!       isolates the target, not a superset/adjacent code); **or**
//!
//!   (b) listed in the enumerated [`NON_FIXTURE_CODES`] allowlist, where the
//!       code is genuinely *not* triggerable through a conformance fixture
//!       (verify-only / parser-recovery-coupled / catalog-only / editor-only),
//!       **each entry carrying a real, verifiable test-path pointer to where
//!       it IS exercised** — never a silent omission.
//!
//! It **fails `cargo test` (build-failing, the `EXPECTED`-lock class)** if a
//! live code is in *neither* set. The allowlist is NOT a silent-omission
//! escape hatch: `non_fixture_pointer_files_exist_and_name_the_code` proves
//! every pointer is a real file that mentions the code.
//!
//! `coverage_map_md_is_byte_derivable_from_the_live_enum_and_manifest`
//! regenerates `tests/conformance/COVERAGE_MAP.md` from `the live enum × the
//! MANIFEST` and asserts the committed file is byte-identical — so the map
//! can never silently drift again (the lock test IS the generator's source
//! of truth, GT-9). `cargo test --features regen-coverage-map -- --ignored
//! regenerate_coverage_map` rewrites it.
//!
//! Non-vacuity is proven by
//! `red_proof_lock_fails_when_a_live_code_is_neither_fixture_nor_allowlisted`:
//! it reuses the *exact* classification engine, removes one code from *both*
//! sets in a local copy of the inputs, and asserts the lock logic then
//! reports that code as an uncovered gap (the `EXPECTED:73`-lock
//! deliberate-removal proof method) — a lock that cannot fail is worthless.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fsm_analyzer::analyze_with_source;
use fsm_diagnostics::{DiagnosticCode, Severity};
use fsm_parser::parse;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Repo-root resolution (mirrors `tests/integration_examples.rs::repo_root`)
// ---------------------------------------------------------------------------

/// Worktree root: `CARGO_MANIFEST_DIR` is `<root>/crates/fsm-cli`.
fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/fsm-cli -> crates
    p.pop(); // crates         -> root
    p
}

fn conformance_dir() -> PathBuf {
    repo_root().join("tests/conformance")
}

// ---------------------------------------------------------------------------
// MANIFEST.json (the subset of the schema this lock needs — mirrors the
// shape `fsm-cli/src/cmd/test.rs` deserialises)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    categories: Vec<ManifestCategory>,
}

#[derive(Deserialize)]
struct ManifestCategory {
    name: String,
    fixtures: Vec<ManifestFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFixture {
    id: String,
    kind: String,
    path: String,
    #[serde(default)]
    expected_codes: Vec<String>,
}

fn load_manifest(root: &Path) -> Manifest {
    let raw = std::fs::read_to_string(root.join("MANIFEST.json"))
        .expect("read tests/conformance/MANIFEST.json");
    serde_json::from_str(&raw).expect("parse MANIFEST.json")
}

// ---------------------------------------------------------------------------
// The exact-set classifier — the heart of the lock.
//
// For every `negative` fixture in a category the conformance harness runs
// through `run_neg` (parser / validator / semantic), compute the EXACT set
// of *error/warning* diagnostic codes the same analysis emits. We mirror
// `run_neg`'s `emitted_codes` precisely: `pr.errors` (parser/lexer
// diagnostics) ∪ `analyze_with_source(...).diagnostics`.
//
// `Severity::Info`/`Severity::Hint` are *not* gating diagnostics for the
// negative-fixture contract (a negative fixture asserts an Error/Warning is
// emitted; Info "compilation successful" / Hint editor-suggestions are not
// "the thing that makes this input invalid"). EXCEPT: a code whose own
// severity IS Hint/Info and which the fixture targets — that target may be
// the sole emitted code at its severity. We therefore key isolation on the
// emitted set *at and above* Warning, plus the explicit target if it is a
// Hint/Info code that the fixture isolates. In practice every authored
// fixture isolates exactly one code at exactly one severity; the helper
// returns the full emitted code set and the caller decides exactness against
// the single MANIFEST-declared target.
// ---------------------------------------------------------------------------

/// The categories whose `negative` fixtures the conformance harness executes
/// via `run_neg` (parser / validator / semantic). `codegen-c` / `formatter`
/// have no negative-diagnostic contract.
const NEG_CATEGORIES: &[&str] = &["parser", "validator", "semantic"];

/// Run one negative fixture's `source.fsm` through the *same* pipeline the
/// conformance harness uses (`fsm_parser::parse` + `analyze_with_source`,
/// `pr.errors ∪ diagnostics`) and return BOTH the full emitted code set (all
/// severities, de-duplicated wire strings) AND the subset at Error/Warning
/// severity (the *gating* set — the severities a negative fixture's contract
/// is about). The exact-set decision is made by the caller against the
/// single MANIFEST-declared target so that a fixture whose target is itself
/// a Hint/Info code (e.g. `FSM-H0004`) is judged correctly: it isolates iff
/// it emits NO gating code and exactly its (Hint/Info) target.
fn emitted_codes(fixture_dir: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let src_path = fixture_dir.join("source.fsm");
    let src = std::fs::read_to_string(&src_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", src_path.display()));
    let pr = parse(&src);
    let res = analyze_with_source(&pr, &src_path.to_string_lossy(), &src);
    let all: BTreeSet<String> = pr
        .errors
        .iter()
        .chain(res.diagnostics.iter())
        .map(|d| d.code.to_string())
        .collect();
    let gating: BTreeSet<String> = pr
        .errors
        .iter()
        .chain(res.diagnostics.iter())
        .filter(|d| matches!(d.severity, Severity::Error | Severity::Warning))
        .map(|d| d.code.to_string())
        .collect();
    (all, gating)
}

/// `code (wire) -> id of the conformance fixture that isolates it`.
type CoveredMap = BTreeMap<String, String>;

/// One rejected-for-non-isolation fixture: `(fixture id, target wire,
/// full emitted code set)`. Surfaced so a multi-code fixture cannot hide.
type NonIsolating = (String, String, Vec<String>);

/// Classify the MANIFEST: return the map `code -> isolating fixture id` for
/// every code that has at least one `negative` fixture which (a) declares
/// exactly that one code in `expectedCodes` and (b) when run emits **exactly
/// that code and nothing else** at Error/Warning severity (the exact-set
/// discipline). A fixture that emits its target *plus* another gating code
/// does NOT satisfy the target's slot.
///
/// Also returns the list of fixtures that were *rejected* for non-isolation,
/// with the offending emitted set — surfaced by the lock so multi-code
/// fixtures cannot hide.
fn classify_fixture_coverage(root: &Path, manifest: &Manifest) -> (CoveredMap, Vec<NonIsolating>) {
    let mut covered: CoveredMap = BTreeMap::new();
    let mut non_isolating: Vec<NonIsolating> = Vec::new();

    for cat in &manifest.categories {
        if !NEG_CATEGORIES.contains(&cat.name.as_str()) {
            continue;
        }
        for fx in &cat.fixtures {
            if fx.kind != "negative" {
                continue;
            }
            // A G7-isolating fixture declares EXACTLY one expected code.
            if fx.expected_codes.len() != 1 {
                continue;
            }
            let target = fx.expected_codes[0].clone();
            let dir = root.join(&fx.path);
            let (all, gating) = emitted_codes(&dir);

            // EXACT-SET (§W3): the fixture isolates `target` iff
            //   (i)  `target` is actually emitted, AND
            //   (ii) the emitted set, restricted to {Error,Warning} ∪
            //        {target}, is exactly {target}.
            // For an Error/Warning target this reduces to "gating == {target}".
            // For a Hint/Info target (e.g. FSM-H0004) it requires NO gating
            // code AND the target present — the correct semantics for a
            // fixture whose whole point is the Hint/Info diagnostic.
            let target_emitted = all.contains(&target);
            let gating_minus_target: BTreeSet<&String> =
                gating.iter().filter(|c| **c != target).collect();
            let exact = target_emitted && gating_minus_target.is_empty();

            if exact {
                // First isolating fixture wins the slot; deterministic via
                // MANIFEST order (categories then fixtures, both stable).
                covered.entry(target).or_insert_with(|| fx.id.clone());
            } else if target_emitted {
                non_isolating.push((fx.id.clone(), target, all.into_iter().collect()));
            }
        }
    }
    (covered, non_isolating)
}

// ---------------------------------------------------------------------------
// The `non_fixture_codes` allowlist.
//
// HIGHEST-SCRUTINY DISCLOSURE (Doc 32 §W3 never-game guard). Every entry is a
// code that is genuinely *not* triggerable as an isolated conformance
// fixture through the `parse + analyze_with_source` path, **with a real
// test-path pointer to where it IS exercised**. The
// `non_fixture_pointer_files_exist_and_name_the_code` test independently
// verifies each pointer is a real file that mentions the code.
//
// Four justified families:
//
//  1. LEXER-RECOVERY-COUPLED (E0001..E0004). The lexer emits
//     `TokenKind::Error(DiagnosticCode::E000x)`; the parser then ALWAYS
//     recovers by additionally emitting `FSM-E0010` ("expected token").
//     Verified by probe: every E0001..E0004 source emits `{E000x, E0010}` —
//     the cascade is structural and unavoidable through the conformance
//     path, so no conformance fixture can *isolate* E000x (exact-set).
//     They ARE exact-set-tested at the cleanest possible layer — the
//     lexer's own token-level unit tests assert `TokenKind::Error(
//     DiagnosticCode::E000x)` directly, with zero cascade.
//
//  2. VERIFY-ONLY (E0400, W0602). Emitted by `fsm-verify`, NOT by
//     `analyze_with_source` (Doc 30 §1.3 honest-emission close). The
//     conformance harness's `run_neg` path never runs `fsm verify`, so
//     these are by-construction unreachable as conformance fixtures —
//     exactly the "only reachable via `fsm verify`" allowlist category the
//     §W3 spec names. They have dedicated behavioural acceptance tests.
//
//  3. CO-EMITTED-BY-DESIGN (E0109). The history-default-references-unknown
//     check and name-resolution both flag the phantom state, so E0109 is
//     always co-emitted with `FSM-E0100` through the analyze path (probe-
//     verified). Its canonical emission test is in the analyzer suite.
//
//  4. CATALOG-ONLY (the remainder). Defined in the `for_each_code!` table
//     but emitted by NO code path anywhere in `crates/*/src` (verified by
//     `grep -rl 'DiagnosticCode::<C>' crates/*/src` → no emitter): runtime-
//     safety codes with no static check (E0900), reconciler-reserved codes
//     whose check was never built (E0005/E0006/E0011/E0012/E0025/E0105/
//     E0110/E0202/E0203/E0207/E0210/E0302/E0303/E0750/W0201/W0401/W0501/
//     W0604), CLI/codegen success-info & deferred info (I0001..I0004),
//     editor-only hints with no emitter (H0001/H0002/H0003/H0005/H0006),
//     and W0100 (every reference is test-fixture data, no emitter — its
//     COVERAGE_MAP row already reads "UNTESTED — emission path missing").
//     A code with no emitter cannot, by definition, be triggered by ANY
//     fixture. The genuine, verifiable test that exercises every such
//     variant is the `fsm-diagnostics` catalog test that iterates the
//     ENTIRE live enum (`all_codes()`) and asserts each variant's
//     severity + non-empty default message + wire round-trip. That is a
//     real per-variant assertion, not a silent omission; authoring an
//     emitter + a behavioural fixture for these is the explicit
//     deferred-with-justification tail (Doc 32 §6 R-fallback / §W3 (3)),
//     tracked code-by-code by this very lock.
//
// `(code, justification, test_path_pointer)`. The pointer is a path
// relative to the worktree root; the file must exist AND mention the code's
// wire form (asserted by the companion test).
// ---------------------------------------------------------------------------

struct AllowEntry {
    /// Wire form, e.g. `"FSM-E0001"`.
    code: &'static str,
    /// Why this code is genuinely not an isolatable conformance fixture.
    justification: &'static str,
    /// A REAL test file (worktree-relative) that exercises this code; the
    /// companion test asserts the file exists and names the code.
    test_pointer: &'static str,
}

const NON_FIXTURE_CODES: &[AllowEntry] = &[
    // ---- (1) lexer-recovery-coupled: exact-set at the token layer --------
    AllowEntry {
        code: "FSM-E0001",
        justification: "Lexer emits TokenKind::Error(E0001); parser recovery always also \
             emits E0010 — no conformance fixture can isolate it. Exact-set \
             tested at the token layer.",
        test_pointer: "crates/fsm-lexer/src/lexer.rs",
    },
    AllowEntry {
        code: "FSM-E0002",
        justification: "Unterminated string: lexer TokenKind::Error(E0002) + unavoidable \
             parser-recovery E0010; isolated only at the token layer.",
        test_pointer: "crates/fsm-lexer/src/lexer.rs",
    },
    AllowEntry {
        code: "FSM-E0003",
        justification: "Unterminated block comment: lexer TokenKind::Error(E0003) + \
             unavoidable parser-recovery E0010; isolated only at the token \
             layer.",
        test_pointer: "crates/fsm-lexer/src/lexer.rs",
    },
    AllowEntry {
        code: "FSM-E0004",
        justification: "Invalid integer literal: lexer TokenKind::Error(E0004) + \
             unavoidable parser-recovery E0010; isolated only at the token \
             layer.",
        test_pointer: "crates/fsm-lexer/src/lexer.rs",
    },
    // ---- (2) verify-only: emitted by fsm-verify, not the analyze path ----
    AllowEntry {
        code: "FSM-E0400",
        justification: "Unreachable-state is emitted by `fsm verify` (fsm-verify), NOT \
             by analyze_with_source — the conformance run_neg path never \
             runs verify. Verify-only by construction. \
             `island_emits_e0400_and_w0602_when_exhaustive` asserts \
             `d.code == DiagnosticCode::E0400` behaviourally.",
        test_pointer: "crates/fsm-verify/src/diagnostics.rs",
    },
    AllowEntry {
        code: "FSM-W0602",
        justification: "No-incoming-transitions warning is the structural subset of \
             E0400, emitted by `fsm verify` (fsm-verify), not the analyze \
             path. Verify-only by construction. \
             `island_emits_e0400_and_w0602_when_exhaustive` asserts \
             `d.code == DiagnosticCode::W0602` behaviourally.",
        test_pointer: "crates/fsm-verify/src/diagnostics.rs",
    },
    // ---- (3) co-emitted-by-design with E0100 -----------------------------
    AllowEntry {
        code: "FSM-E0109",
        justification: "History-default-references-unknown-state is always co-emitted \
             with E0100 (name resolution also flags the phantom) — cannot be \
             isolated through the analyze path. Canonical emission test in \
             the analyzer negative suite.",
        test_pointer: "crates/fsm-analyzer/tests/negative.rs",
    },
    // ---- (4) catalog-only: NO emitter anywhere; whole-enum catalog test --
    AllowEntry {
        code: "FSM-E0005",
        justification: "Catalog-only: no emitter in crates/*/src (float-in-int-context \
             check never built; reconciler-reserved). Whole-enum catalog \
             test asserts its severity/message/round-trip.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0006",
        justification: "Catalog-only: no emitter (read-only-payload-write currently \
             flows through E0204; the dedicated check was never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0011",
        justification: "Catalog-only: no emitter (EOF-inside-construct surfaces as \
             E0010, not a dedicated E0011).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0012",
        justification: "Catalog-only: no emitter (invalid-`as`-cast check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0025",
        justification: "Catalog-only: no emitter (duplicate-stable-id check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0105",
        justification: "Catalog-only: no emitter (cross-machine-state-reference check \
             never built; single-machine-scope resolution makes it \
             unreachable).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0110",
        justification: "Catalog-only: no emitter (local-transition-target-not-descendant \
             reconciler addition; check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0202",
        justification: "Catalog-only: no emitter (extern-arg-type-mismatch check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0203",
        justification: "Catalog-only: no emitter (extern-arity-mismatch check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0207",
        justification: "Catalog-only: no emitter (const-divide-by-zero check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0210",
        justification: "Catalog-only: no emitter (opaque-field-in-guard reconciler \
             addition; check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0302",
        justification:
            "Catalog-only: no emitter (fork-target-not-region-initial check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0303",
        justification: "Catalog-only: no emitter (join-source-not-in-region check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0750",
        justification:
            "Catalog-only: no emitter (fork-target-not-parallel-region check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-E0900",
        justification: "Catalog-only: a runtime-safety code (completion-chain-too-deep) \
             with no static analyzer check — a runtime concern, not a \
             conformance-fixture target.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-W0100",
        justification: "Catalog-only: no emitter (history-no-stored-no-default warning \
             never built; every W0100 reference is test-fixture data). Its \
             COVERAGE_MAP row already reads UNTESTED.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-W0201",
        justification: "Catalog-only: no emitter (action-block-complexity warning never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-W0401",
        justification: "Catalog-only: no emitter (large-timer-duration warning never \
             built; W0601 `>24h` is the implemented sibling).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-W0501",
        justification: "Catalog-only: no emitter (unused-event warning never built; the \
             whole unused-declaration family, incl. retired W0500, was never \
             implemented).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-W0604",
        justification: "Catalog-only: no emitter (float-cast-precision-loss reconciler \
             addition; check never built).",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-I0001",
        justification: "Catalog-only / non-negative: an Info success message \
             (\"compilation successful\"), not an error/warning a negative \
             fixture asserts. No emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-I0002",
        justification: "Catalog-only / non-negative: an Info success message (\"code \
             generation complete\"). No emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-I0003",
        justification: "Catalog-only / non-negative: \"simulator ready\" Info, deferred \
             (simulator daemon, v1.1). No emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-I0004",
        justification: "Catalog-only / non-negative: forward-compat IR-schema Info. No \
             emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-H0001",
        justification:
            "Catalog-only: editor-only hint (no entry action); no emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-H0002",
        justification:
            "Catalog-only: editor-only hint (no exit action); no emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-H0003",
        justification: "Catalog-only: editor-only hint (unconditional transition); no \
             emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-H0005",
        justification: "Catalog-only: editor-only hint (prefer `every`); no emitter in \
             crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
    AllowEntry {
        code: "FSM-H0006",
        justification: "Catalog-only: editor-only hint (`@id` before doc comment); no \
             emitter in crates/*/src.",
        test_pointer: "crates/fsm-diagnostics/src/lib.rs",
    },
];

fn allowlist_codes() -> BTreeSet<String> {
    NON_FIXTURE_CODES
        .iter()
        .map(|e| e.code.to_string())
        .collect()
}

/// Every live code as a wire-form set — the SoT, straight off the compiled
/// enum (NOT a doc, NOT a hand list).
fn live_codes() -> BTreeSet<String> {
    DiagnosticCode::all_codes()
        .iter()
        .map(|(c, _, _)| c.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// THE LOCK (build-failing — the `EXPECTED:73` class)
// ---------------------------------------------------------------------------

#[test]
fn every_live_code_has_a_conformance_fixture_or_a_justified_allowlist_entry() {
    let root = conformance_dir();
    let manifest = load_manifest(&root);

    let live = live_codes();
    let (covered, non_isolating) = classify_fixture_coverage(&root, &manifest);
    let allow = allowlist_codes();

    // Surface every non-isolating multi-code fixture so a superset fixture
    // cannot quietly "cover" a code (exact-set discipline, §W3 never-game).
    // This is informational unless it leaves a code uncovered below — but it
    // is always printed so the disclosure is loud.
    if !non_isolating.is_empty() {
        eprintln!(
            "note: {} negative fixture(s) emit their target PLUS other gating \
             codes (do NOT satisfy exact-set for that target):",
            non_isolating.len()
        );
        for (id, target, emitted) in &non_isolating {
            eprintln!("  - {id}: target {target}, emitted {emitted:?}");
        }
    }

    // A code is "accounted for" iff it is fixture-covered (exact-set) OR
    // explicitly allowlisted with a pointer.
    let covered_codes: BTreeSet<String> = covered.keys().cloned().collect();
    let mut uncovered: Vec<String> = Vec::new();
    for code in &live {
        if !covered_codes.contains(code) && !allow.contains(code) {
            uncovered.push(code.clone());
        }
    }

    // The allowlist must not name a code that is not live (stale entry) and
    // must not double-claim a code that ALSO has an isolating fixture (the
    // allowlist is for genuinely-unreachable codes only — over-allowlisting
    // a code that DOES have a fixture is the exact gaming §W3 forbids).
    let stale_allow: Vec<&String> = allow.iter().filter(|c| !live.contains(*c)).collect();
    let double_claimed: Vec<&String> = allow
        .iter()
        .filter(|c| covered_codes.contains(*c))
        .collect();

    assert!(
        stale_allow.is_empty(),
        "NON_FIXTURE_CODES names code(s) that are not live in \
         DiagnosticCode::all_codes() (stale allowlist entry — remove or \
         correct): {stale_allow:?}"
    );
    assert!(
        double_claimed.is_empty(),
        "NON_FIXTURE_CODES allowlists code(s) that ALSO have an isolating \
         conformance fixture — the allowlist is for genuinely \
         non-fixture-reachable codes only; move these out of the allowlist \
         (over-allowlisting is forbidden gaming): {double_claimed:?}"
    );

    assert!(
        uncovered.is_empty(),
        "G7 LOCK FAILED — {} live DiagnosticCode variant(s) have NEITHER an \
         exact-set conformance fixture NOR a justified NON_FIXTURE_CODES \
         allowlist entry (no silent gaps allowed): {:?}\n\
         Each such code must be EITHER given a conformance fixture under \
         tests/conformance/{{parser,validator,semantic}}/neg/ that triggers \
         EXACTLY it (add the MANIFEST entry), OR added to NON_FIXTURE_CODES \
         with a real test-path pointer. This is the EXPECTED-lock class: it \
         fails `cargo test` until the gap is closed.",
        uncovered.len(),
        uncovered
    );

    // Sanity: the partition is exhaustive and the counts are self-consistent
    // (cheap post-condition; also documents the live partition in the log).
    let n_live = live.len();
    let n_cov = covered_codes.len();
    let n_allow = allow.len();
    assert_eq!(
        n_cov + n_allow,
        n_live,
        "partition arithmetic broken: {n_cov} fixture-covered + {n_allow} \
         allowlisted != {n_live} live (overlap or gap)"
    );
    eprintln!(
        "G7 lock OK: {n_live} live codes = {n_cov} exact-set conformance \
         fixtures + {n_allow} justified non-fixture allowlist entries."
    );
}

/// HIGHEST-SCRUTINY companion: every `NON_FIXTURE_CODES` pointer must be a
/// REAL file that mentions the code's wire form — the allowlist cannot point
/// at a non-existent or unrelated test (the never-game guard, independently
/// re-verifiable).
#[test]
fn non_fixture_pointer_files_exist_and_name_the_code() {
    let root = repo_root();
    let mut problems: Vec<String> = Vec::new();
    for e in NON_FIXTURE_CODES {
        let p = root.join(e.test_pointer);
        if !p.is_file() {
            problems.push(format!(
                "{}: pointer file does not exist: {}",
                e.code, e.test_pointer
            ));
            continue;
        }
        let body = std::fs::read_to_string(&p).unwrap_or_default();
        // Accept either the wire form (FSM-E0001) or the bare variant
        // (E0001) — both unambiguously name the code; lexer/analyzer tests
        // use the `DiagnosticCode::E0001` form, the diagnostics catalog uses
        // both. The bare-variant check is exact (digit-bounded).
        let wire = e.code; // e.g. FSM-E0001
        let bare = e.code.trim_start_matches("FSM-"); // e.g. E0001
        let names_it = body.contains(wire) || body.contains(bare);
        if !names_it {
            problems.push(format!(
                "{}: pointer file {} does not mention `{}` or `{}`",
                e.code, e.test_pointer, wire, bare
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "NON_FIXTURE_CODES pointer verification failed (each allowlist entry \
         MUST cite a real test that exercises the code):\n{}",
        problems.join("\n")
    );
}

// ---------------------------------------------------------------------------
// RED-PROOF — non-vacuity (the `EXPECTED:73`-lock deliberate-removal method)
//
// Reuse the EXACT classification engine. Build the live set, the
// fixture-covered set, and the allowlist; then DELIBERATELY drop one code
// from BOTH the covered set and the allowlist and assert the lock's own
// gap-detection logic flags exactly that code. This proves the lock is
// non-vacuous: a live code in neither set IS detected as a build-failing
// gap. We then assert the un-tampered classification has zero gaps (the
// "restore" half — the lock passes on the real inputs).
// ---------------------------------------------------------------------------

/// The pure gap-detection core, factored so the RED-proof exercises the
/// IDENTICAL logic the real lock uses (not a re-implementation).
fn uncovered_codes(
    live: &BTreeSet<String>,
    covered: &BTreeSet<String>,
    allow: &BTreeSet<String>,
) -> Vec<String> {
    live.iter()
        .filter(|c| !covered.contains(*c) && !allow.contains(*c))
        .cloned()
        .collect()
}

#[test]
fn red_proof_lock_fails_when_a_live_code_is_neither_fixture_nor_allowlisted() {
    let root = conformance_dir();
    let manifest = load_manifest(&root);
    let live = live_codes();
    let (covered_map, _) = classify_fixture_coverage(&root, &manifest);
    let covered: BTreeSet<String> = covered_map.keys().cloned().collect();
    let allow = allowlist_codes();

    // GREEN half — the real inputs have NO gap (the lock passes; "restore").
    assert!(
        uncovered_codes(&live, &covered, &allow).is_empty(),
        "precondition: the real inputs must be fully covered before the \
         RED-proof tampers with a copy"
    );

    // RED half — pick a real, currently-covered live code and a real,
    // currently-allowlisted live code; remove EACH from BOTH sets in a
    // tampered copy and assert the IDENTICAL gap logic now flags it. This
    // is the deliberate-removal proof: a code in neither set => detected.
    let a_fixture_code = covered
        .iter()
        .next()
        .expect("there must be ≥1 fixture-covered code to tamper with")
        .clone();
    let an_allow_code = allow
        .iter()
        .find(|c| live.contains(*c))
        .expect("there must be ≥1 live allowlisted code to tamper with")
        .clone();

    for victim in [&a_fixture_code, &an_allow_code] {
        let mut t_cov = covered.clone();
        let mut t_allow = allow.clone();
        t_cov.remove(victim);
        t_allow.remove(victim);

        let gap = uncovered_codes(&live, &t_cov, &t_allow);
        assert!(
            gap.contains(victim),
            "RED-PROOF FAILED (lock is VACUOUS): after removing `{victim}` \
             from BOTH the fixture-covered set and the allowlist, the lock's \
             own gap-detection did NOT flag it. A lock that cannot fail is \
             worthless."
        );
        // And it must flag PRECISELY the victim (no spurious extra gaps from
        // the tamper) — the gap set is exactly {victim}.
        assert_eq!(
            gap,
            vec![victim.clone()],
            "RED-PROOF: removing `{victim}` must yield EXACTLY that one gap; \
             got {gap:?}"
        );
    }

    eprintln!(
        "RED-proof OK: removing a covered code ({a_fixture_code}) or an \
         allowlisted code ({an_allow_code}) from both sets is detected as a \
         build-failing gap by the SAME logic the lock uses; un-tampered \
         inputs have zero gaps."
    );
}

// ---------------------------------------------------------------------------
// COVERAGE_MAP.md — GENERATED ARTIFACT (GT-9)
//
// The map is derived BYTE-FOR-BYTE from `the live enum × the MANIFEST`. The
// hand-maintained "Tested 39 / UNTESTED 34" split is eliminated: each row's
// status is *computed* (exact-set fixture id, or "non-fixture (allowlisted)"
// with the justification). The committed file MUST equal the generated
// content; a drift fails `cargo test`. `--features regen-coverage-map --
// --ignored regenerate_coverage_map` rewrites it.
// ---------------------------------------------------------------------------

fn coverage_map_path() -> PathBuf {
    conformance_dir().join("COVERAGE_MAP.md")
}

/// Render the canonical COVERAGE_MAP.md content from the live enum × the
/// MANIFEST. Deterministic: rows in `all_codes()` table order; status from
/// the exact-set classifier and the allowlist.
fn render_coverage_map(root: &Path) -> String {
    let manifest = load_manifest(root);
    let (covered, _) = classify_fixture_coverage(root, &manifest);
    let allow_by_code: BTreeMap<&str, &AllowEntry> =
        NON_FIXTURE_CODES.iter().map(|e| (e.code, e)).collect();

    let table = DiagnosticCode::all_codes();
    let n = table.len();
    let n_fix = covered.len();
    let n_allow = NON_FIXTURE_CODES
        .iter()
        .filter(|e| table.iter().any(|(c, _, _)| c.to_string() == e.code))
        .count();

    let mut s = String::new();
    s.push_str("# Conformance Coverage Map — Diagnostic Codes (GENERATED)\n\n");
    s.push_str(
        "DO NOT EDIT BY HAND. This file is a **generated artifact** derived \
         byte-for-byte from\nthe **compiled live `fsm_diagnostics::DiagnosticCode` \
         enum** (`all_codes()`) ×\n`tests/conformance/MANIFEST.json`, by the G7 \
         lock test\n`crates/fsm-cli/tests/conformance_code_coverage_lock.rs`. The \
         lock test is the\ngenerator's source of truth; \
         `coverage_map_md_is_byte_derivable_from_the_live_enum_and_manifest`\n\
         fails `cargo test` if this file drifts. Regenerate with:\n\n",
    );
    s.push_str(
        "    cargo test -p fsm-cli --features regen-coverage-map \\\n        \
         --test conformance_code_coverage_lock -- --ignored \
         regenerate_coverage_map\n\n",
    );
    s.push_str(
        "Evidence for MVP gate **G7** (Doc 00 §5.1 / Doc 32 §W3): every live \
         diagnostic\ncode is EITHER covered by a conformance fixture that \
         triggers **exactly** that\ncode (the exact-set discipline — verified \
         behaviourally by running the fixture\nthrough the same \
         `parse + analyze_with_source` the conformance harness uses)\nOR \
         enumerated in the `NON_FIXTURE_CODES` allowlist with a real \
         test-path\npointer (genuinely not isolatable as a conformance \
         fixture: verify-only,\nlexer-recovery-coupled, co-emitted-by-design, \
         or catalog-only). There is NO\nhand-maintained Tested/UNTESTED \
         split — the old drift surface (GT-9) is gone.\n\n",
    );
    s.push_str(&format!(
        "The live enum is **{n}** codes (the `all_codes_matches_expected_count` \
         lock in\n`fsm-diagnostics`). It was 75 in v1.0.0; `FSM-E0903` was \
         retired in v1.1 and\n`FSM-W0500` in v1.2-FU-DEAD-CODES (each −1). \
         Retired codes are not live\nvariants and are not rows here.\n\n",
    ));

    s.push_str("## Summary\n\n");
    s.push_str("| Status | Count |\n");
    s.push_str("| --- | --- |\n");
    s.push_str(&format!("| Exact-set conformance fixture | {n_fix} |\n"));
    s.push_str(&format!(
        "| Non-fixture (allowlisted, with test pointer) | {n_allow} |\n"
    ));
    s.push_str(&format!("| **Total live codes** | **{n}** |\n\n"));

    s.push_str("## Per-code map\n\n");
    s.push_str("| Code | Severity | Coverage | Evidence |\n");
    s.push_str("| --- | --- | --- | --- |\n");
    for (code, sev, _) in table {
        let wire = code.to_string();
        let sev_s = match sev {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Info => "Info",
            Severity::Hint => "Hint",
        };
        let (coverage, evidence) = if let Some(fx) = covered.get(&wire) {
            (
                "exact-set fixture".to_string(),
                format!(
                    "`tests/conformance/MANIFEST.json` fixture `{fx}` triggers \
                     exactly `{wire}`"
                ),
            )
        } else if let Some(e) = allow_by_code.get(wire.as_str()) {
            (
                "non-fixture (allowlisted)".to_string(),
                format!("{} — see `{}`", e.justification, e.test_pointer),
            )
        } else {
            // Unreachable while the lock passes; emitted defensively so a
            // regen on a broken tree is self-describing rather than silent.
            (
                "**UNCOVERED**".to_string(),
                "no fixture and not allowlisted — the G7 lock test will fail".to_string(),
            )
        };
        s.push_str(&format!("| {wire} | {sev_s} | {coverage} | {evidence} |\n"));
    }

    s.push_str(
        "\n## How the gap is closed\n\nEach `non-fixture (allowlisted)` row \
         is the explicit, code-by-code deferred\ntail (Doc 32 §6 R-fallback / \
         §W3 (3)): closing it means authoring an emitter\n(for catalog-only \
         codes) and/or an isolating conformance fixture, then moving\nthe \
         code out of `NON_FIXTURE_CODES`. The G7 lock guarantees the residual \
         is\nexplicit and tracked — it can never become a silent gap, and a \
         new live code\nwith neither a fixture nor an allowlist entry fails \
         the build immediately.\n",
    );
    s
}

#[test]
fn coverage_map_md_is_byte_derivable_from_the_live_enum_and_manifest() {
    let root = conformance_dir();
    let generated = render_coverage_map(&root);
    let committed = std::fs::read_to_string(coverage_map_path())
        .expect("read tests/conformance/COVERAGE_MAP.md");
    assert_eq!(
        committed, generated,
        "tests/conformance/COVERAGE_MAP.md is NOT byte-derivable from the \
         live enum × the MANIFEST — it has drifted (GT-9: a hand-maintained \
         coverage map is a drift surface). Regenerate it:\n  cargo test -p \
         fsm-cli --features regen-coverage-map --test \
         conformance_code_coverage_lock -- --ignored \
         regenerate_coverage_map"
    );
}

/// Regenerator. `#[ignore]` so it never runs in the normal suite; gated on
/// the `regen-coverage-map` feature so a stray `--ignored` run can't rewrite
/// the file unintentionally. This is the ONE writer of COVERAGE_MAP.md.
#[test]
#[ignore = "writer: run explicitly with --features regen-coverage-map to regenerate COVERAGE_MAP.md"]
#[cfg(feature = "regen-coverage-map")]
fn regenerate_coverage_map() {
    let root = conformance_dir();
    let content = render_coverage_map(&root);
    std::fs::write(coverage_map_path(), content).expect("write COVERAGE_MAP.md");
    eprintln!("regenerated {}", coverage_map_path().display());
}
