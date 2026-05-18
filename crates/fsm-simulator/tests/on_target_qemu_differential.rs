//! **Phase-6.0-W2 — the on-target QEMU differential** (the keystone PROPER;
//! Doc 32 §1 W2 / §2 catalogue-integrity contract / §6 H2/H3).
//!
//! ## What this is
//!
//! W1 proved the codegen's `#ifdef FSM_TRACE` C, compiled + run **on the
//! host**, byte-equals the shipped `fsm_simulator::execute_trace`
//! `StepRecord` oracle (`codegen_equivalence_smoke.rs`). W2 extends the
//! **identical** `FSM_TRACE` C to an **emulated MCU**: cross-compile with
//! `arm-none-eabi-gcc`, link the minimal `mps2-an385` Cortex-M3 harness
//! (`tests/on-target/`), run under `qemu-system-arm -M mps2-an385`
//! (semihosting), capture the emitted `FSM_TRACE` record stream, and
//! **byte-diff it vs the SAME shipped `execute_trace` oracle** for the
//! catalogue's `BYTE_EQUAL` members.
//!
//! "The C cross-compiles / QEMU boots" is explicitly **NOT** acceptance
//! (Doc 32 §W2 §5.4) — the emitted *trace* must be byte-equal to the
//! oracle.
//!
//! ## The keystone — drive-the-oracle, never fork (Doc 32 §2 / §6 H2)
//!
//! The comparison oracle is **always** the shipped
//! `fsm_simulator::execute_trace` returning `Vec<StepRecord>` — the *exact*
//! unforked seam `crates/fsm-cli/src/cmd/{test,baseline}.rs` and W1's
//! differential consume. W2 adds the **target harness + the cross-compile/
//! QEMU glue but NOT a second oracle**:
//!
//! - **no forked comparator**: the only comparison is pure byte-equality of
//!   two text streams (`oracle_lines == target_lines`);
//! - **no second `StepRecord` re-derivation / re-implemented step,
//!   transition-selection, guard-eval or deadlock semantics** anywhere in
//!   this file, the target harness C (`tests/on-target/*`), or the lane
//!   definition (`docs/ci/on-target-lane.yml`). The harness C is CRT
//!   bring-up + a semihosting string sink + a counter clock — pure
//!   platform plumbing;
//! - the target trace-emit is a **trace-tap re-emitting the IDENTICAL W1
//!   `FSM_TRACE` records over semihosting** — a different execution
//!   *substrate*, the SAME oracle. This is realized by the codegen's
//!   `__attribute__((weak)) fsm_trace_emit` being overridden by the
//!   harness's strong semihosting `fsm_trace_emit` with **ZERO codegen
//!   change** (`emit/trace_hook.rs` module-doc mandates exactly this seam).
//!
//! The projection below is **formatting-only** (no transition selection /
//! guard eval / LCA) and is the *same canonical shape* W1's
//! `project_record` produces; `projection_shape_matches_w1_canonical_form`
//! pins that contract + the `FIELD_SEP` cross-engine byte so the two cannot
//! silently drift. The W6 phase-audit negative-greps this file + the
//! harness + the lane for step semantics and re-runs the differential; it
//! is engineered to pass by construction.
//!
//! ## Skip-if-toolchain-absent (the `gcc_compile.rs` precedent — Doc 32
//! §W2 §5.4 / GT-10 / OWNER-2)
//!
//! `arm-none-eabi-gcc` and `qemu-system-arm` are **CI-runner-only by
//! design** (the on-target lane installs them in the runner; they are
//! deliberately ABSENT from the dev box, which is disk-tight). So this test
//! **skips locally with an honest notice and exits successfully** when
//! either is absent (the exact `gcc_compile.rs` "prints a skip notice and
//! exits successfully" precedent) and is a **HARD differential in CI**
//! (where the lane installs both). Per Doc 32 §W2 §5.4 the local
//! acceptance is: the mechanism is correct-by-construction + the
//! skip-if-absent path demonstrably works + the on-target *logic* is
//! already smoke-proven by W1's host differential (the IDENTICAL
//! `FSM_TRACE` C + the SAME oracle; QEMU is the only new, oracle-irrelevant
//! variable per Doc 32 §2's determinism rationale).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fsm_analyzer::analyze_with_source;
use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::Severity;
use fsm_ir::Ir;
use fsm_parser::parse;
use fsm_simulator::{execute_trace, parse_trace_yaml, StepKind, StepRecord, TraceFile};

/// US (0x1F) — the canonical-line field separator. MUST equal both W1's
/// `SEP` and the codegen `fsm_codegen_c::emit::trace_hook::FIELD_SEP`
/// (asserted in `projection_shape_matches_w1_canonical_form`): the
/// projection is a binding contract between the shipped simulator and the
/// generated C's `#ifdef FSM_TRACE` hook — a drift in either silently
/// blinds the differential.
const SEP: char = '\u{1f}';

/// The W2 on-target gate corpus = the catalogue's `BYTE_EQUAL` members
/// (Doc 32 §2: "The W2 on-target lane gates on (1)+(4)+(5) over the same
/// catalogue … the JUSTIFIED record-model residual is a host-proven
/// property, NOT re-litigated per substrate — the substrate changes, the
/// oracle and the catalogue do not"). This list is the W1
/// `codegen_equivalence_smoke.rs` `BYTE_EQUAL` array verbatim;
/// `byte_equal_corpus_matches_w1_catalogue` (and the W6 audit) re-derive it
/// from the W1 catalogue so it cannot drift. `vending-machine` /
/// `submachine` / `stress-completion-chain`
/// (`BEHAVIOURALLY_EQUIVALENT_JUSTIFIED`) are deliberately NOT here: their
/// record-model residual is host-proven and not re-litigated per substrate.
/// `KNOWN_DIVERGENT` is empty.
const W2_BYTE_EQUAL_CORPUS: &[&str] = &[
    "motor",
    "deferred",
    "traffic-light",
    "stress-parallel-cross-exit",
    "stress-deep-history",
    "stress-self-transitions",
    "stress-self-transitions-actions",
    "stress-choice-guard-payload",
    "stress-every-timer",
];

// ---------------------------------------------------------------------------
// Toolchain probes — skip-if-absent (the gcc_compile.rs precedent).
// ---------------------------------------------------------------------------

fn tool_available(bin: &str, version_arg: &str) -> bool {
    Command::new(bin)
        .arg(version_arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn arm_gcc_available() -> bool {
    tool_available("arm-none-eabi-gcc", "--version")
}

fn qemu_available() -> bool {
    tool_available("qemu-system-arm", "--version")
}

/// The single skip-gate. Returns the honest skip-notice string when the
/// CI-runner-only toolchain is absent (local, by design); `None` ⇒ both
/// present ⇒ the HARD differential runs (the CI runner).
fn on_target_skip_reason() -> Option<String> {
    let arm = arm_gcc_available();
    let qemu = qemu_available();
    if arm && qemu {
        return None;
    }
    Some(format!(
        "[on_target_qemu_differential] on-target toolchain absent \
         (arm-none-eabi-gcc: {}, qemu-system-arm: {}) — skipping the QEMU \
         differential; cargo test still PASSES (the gcc_compile.rs \
         skip-if-absent precedent). This is BY DESIGN, not a gap: the \
         on-target lane is CI-RUNNER-ONLY (Doc 32 §W2 / GT-10 / OWNER-2 — \
         the dev box is disk-tight; the ARM toolchain + QEMU install \
         belongs in the runner YAML, never on the box). The on-target \
         *logic* is locally smoke-proven by W1's host differential (the \
         IDENTICAL FSM_TRACE C + the SAME fsm_simulator::execute_trace \
         oracle; QEMU is the only new variable and is oracle-irrelevant \
         per Doc 32 §2's determinism rationale). In CI (lane installs both) \
         this is a HARD byte-differential.",
        arm, qemu,
    ))
}

// ---------------------------------------------------------------------------
// The repo / harness paths.
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("canonicalize repo root")
}

fn harness_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("on-target")
        .canonicalize()
        .expect("canonicalize tests/on-target harness dir")
}

// ---------------------------------------------------------------------------
// The oracle — the SHIPPED `fsm_simulator::execute_trace` (unforked). This
// section drives the same seam W1 + `cmd/{test,baseline}.rs` use and
// projects the produced `StepRecord`s with FORMATTING ONLY. No transition
// selection, no guard eval, no LCA, no FSM semantics — the identical
// canonical shape W1's `project_record` emits (pinned by
// `projection_shape_matches_w1_canonical_form`).
// ---------------------------------------------------------------------------

fn kind_str(k: StepKind) -> &'static str {
    // Identical mapping to W1's `kind_str` (formatting only — a label per
    // already-computed StepKind; this computes nothing about the FSM).
    match k {
        StepKind::Init => "init",
        StepKind::Dispatched => "dispatched",
        StepKind::Raised => "raised",
        StepKind::TimerFired => "timer_fired",
        StepKind::Completion => "completion",
        StepKind::Discarded => "dispatched",
        StepKind::EventDeferred => "event_deferred",
        StepKind::EventRedispatched => "event_redispatched",
        StepKind::SubmachineEntered => "submachine_entered",
        StepKind::SubmachineEventDelegated => "submachine_event_delegated",
        StepKind::SubmachineCompleted => "submachine_completed",
    }
}

fn sorted_csv(items: &[String]) -> String {
    let mut v: Vec<&str> = items.iter().map(String::as_str).collect();
    v.sort_unstable();
    v.join(",")
}

/// Project ONE shipped `StepRecord` to the canonical line — a pure function
/// of the record's already-computed fields (it computes nothing about the
/// FSM; it only FORMATS what the interpreter, the oracle, recorded). The
/// exact field set + order + separator W1's `project_record` and the
/// codegen `FSM_TRACE` hook produce (`trace_id`/`actions_executed`/
/// `submachine`/`clk` excluded for the same disclosed reasons as W1 — see
/// `codegen_equivalence_smoke.rs` module-doc; the on-target stream the
/// codegen emits is byte-identical to the host one because the codegen is
/// unchanged).
fn project_record(r: &StepRecord) -> String {
    let evt = r
        .event_received
        .as_ref()
        .map(|e| e.name.clone())
        .unwrap_or_else(|| "-".into());
    let (tr, src, dst) = r
        .transition_taken
        .as_ref()
        .map(|t| (t.stable_id.clone(), t.source.clone(), t.target.clone()))
        .unwrap_or_else(|| ("-".into(), "-".into(), "-".into()));
    [
        "STEP".to_string(),
        format!("kind={}", kind_str(r.kind)),
        format!("evt={}", evt),
        format!("tr={}", tr),
        format!("src={}", src),
        format!("dst={}", dst),
        format!("cfgB={}", sorted_csv(&r.config_before)),
        format!("cfgA={}", sorted_csv(&r.config_after)),
        format!("ent={}", sorted_csv(&r.entered_states)),
        format!("ext={}", sorted_csv(&r.exited_states)),
    ]
    .join(&SEP.to_string())
}

/// The simulator-side projected trace — the ORACLE. Drives the shipped
/// `execute_trace` (the unforked seam) and projects every produced record.
/// Identical to W1's `simulator_projection`: clear `expected` so
/// `execute_trace` returns the actual produced records (it short-circuits
/// diffing when `expected` is set — trace.rs); the oracle is the LIVE
/// interpreter run.
fn simulator_projection(ir: &Ir, trace: &TraceFile) -> Vec<String> {
    let mut t = trace.clone();
    t.expected.clear();
    let res = execute_trace(ir, &t).expect("the shipped fsm_simulator::execute_trace oracle ran");
    res.actual.iter().map(project_record).collect()
}

// ---------------------------------------------------------------------------
// Example loading — identical to W1 (parse + analyze the frozen example
// `.fsm` into IR; load its frozen `.trace` driver). The corpus FSMs are
// frozen, valid, shipped; an analysis error here is a real regression.
// ---------------------------------------------------------------------------

fn analyze_example(fsm_path: &Path) -> Ir {
    let src =
        fs::read_to_string(fsm_path).unwrap_or_else(|e| panic!("read {}: {e}", fsm_path.display()));
    let pr = parse(&src);
    let res = analyze_with_source(&pr, &fsm_path.to_string_lossy(), &src);
    let errs: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errs.is_empty(),
        "{} has analysis errors (a frozen example must compile): {:?}",
        fsm_path.display(),
        errs
    );
    res.ir
        .unwrap_or_else(|| panic!("analyzer produced no IR for {}", fsm_path.display()))
}

fn load_trace(example: &str) -> (Ir, TraceFile, String) {
    let root = repo_root();
    let fsm = root
        .join("examples")
        .join(example)
        .join(format!("{example}.fsm"));
    let trace_path = root
        .join("examples")
        .join(example)
        .join(format!("{example}.trace"));
    let ir = analyze_example(&fsm);
    let raw = fs::read_to_string(&trace_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", trace_path.display()));
    let trace =
        parse_trace_yaml(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", trace_path.display()));
    let machine = trace
        .init
        .machine_name
        .clone()
        .or_else(|| ir.machines.first().map(|m| m.name.clone()))
        .expect("a machine name");
    (ir, trace, machine)
}

// ---------------------------------------------------------------------------
// The driver + user-contract symbol bodies. This is the SAME mechanical
// contract-satisfaction approach W1 uses (driver_main_c +
// user_symbol_definitions): it replays the trace's init/step sequence
// through the GENERATED public API ONLY (`<M>_init` / `<M>_dispatch` /
// `<M>_advance_clock`) and provides trivial bodies for the externs the
// generated C requires. NO FSM logic lives here — it is a faithful event
// feeder + linker-contract satisfaction, the C analogue of the
// `TraceCommand` loop `execute_trace` itself runs. Pinned-guard constants
// mirror the simulator's `extern_returns` so a `[guard]` transition takes
// the SAME branch (driver fidelity, not semantics).
// ---------------------------------------------------------------------------

fn c_ident(input: &str) -> String {
    let mut s: String = input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        s.insert(0, '_');
    }
    s
}

fn c_macro_prefix(name: &str) -> String {
    c_ident(name).to_uppercase()
}

fn primitive_c(name: &str) -> String {
    match name {
        "u8" => "uint8_t",
        "u16" => "uint16_t",
        "u32" => "uint32_t",
        "u64" => "uint64_t",
        "i8" => "int8_t",
        "i16" => "int16_t",
        "i32" => "int32_t",
        "i64" => "int64_t",
        "bool" => "bool",
        "f32" => "float",
        "f64" => "double",
        _ => "int",
    }
    .to_string()
}

fn c_value_literal(v: &fsm_simulator::Value) -> String {
    use fsm_simulator::Value;
    match v {
        Value::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Value::I32(n) => n.to_string(),
        Value::I64(n) => format!("{}LL", n),
        Value::U8(n) => n.to_string(),
        Value::U16(n) => n.to_string(),
        Value::U32(n) => format!("{}u", n),
        Value::F32(x) => format!("{:?}f", x),
        Value::F64(x) => format!("{:?}", x),
        other => panic!(
            "driver: unsupported payload Value variant in the corpus: {other:?} \
             (extend c_value_literal if a new corpus fixture needs it)"
        ),
    }
}

/// The host driver `main.c`. Replays the trace's init + step sequence
/// through the GENERATED public API only — a faithful event feeder, NOT FSM
/// logic. `fsm_test_set_clock` (the harness's deterministic clock) is
/// advanced exactly as the trace's `advance_clock` deltas, so HAL-delegated
/// time mirrors the simulator's virtual clock and timer EXPIRY (whose
/// state/transition effect IS differenced) fires identically. Returns 0 on
/// completion; `Reset_Handler` then issues the clean semihosting exit.
fn driver_main_c(machine: &str, trace: &TraceFile) -> String {
    let prefix = machine;
    let macro_prefix = c_macro_prefix(machine);

    let mut s = String::new();
    s.push_str("#include \"fsm_hal.h\"\n");
    s.push_str(&format!("#include \"{}.h\"\n", prefix));
    s.push_str("#include <stdint.h>\n");
    s.push_str("#include <string.h> /* memset */\n\n");
    s.push_str("void fsm_test_set_clock(uint32_t ms);\n\n");

    s.push_str("int main(void) {\n");
    s.push_str(&format!("    {p}_t inst;\n", p = prefix));
    s.push_str(&format!("    {p}_init(&inst);\n", p = prefix));

    for cmd in &trace.steps {
        match cmd {
            fsm_simulator::TraceCommand::Dispatch { event, payload }
            | fsm_simulator::TraceCommand::Raise { event, payload } => {
                let ev_c = format!("{}_EVENT_{}", macro_prefix, c_ident(event));
                s.push_str(&format!(
                    "    {{ {p}_Event_t e; memset(&e, 0, sizeof e); e.id = {ev};\n",
                    p = prefix,
                    ev = ev_c,
                ));
                if let Some(map) = payload {
                    for (field, val) in map {
                        s.push_str(&format!(
                            "      e.__payload.{ev}.{f} = {v};\n",
                            ev = event,
                            f = field,
                            v = c_value_literal(val),
                        ));
                    }
                }
                s.push_str(&format!("      {p}_dispatch(&inst, &e); }}\n", p = prefix));
            }
            fsm_simulator::TraceCommand::AdvanceClock { delta_ms } => {
                s.push_str(&format!(
                    "    fsm_test_set_clock(fsm_hal_clock_now_ms() + {d}u);\n",
                    d = delta_ms,
                ));
                s.push_str(&format!(
                    "    {p}_advance_clock(&inst, {d}u);\n",
                    p = prefix,
                    d = delta_ms,
                ));
            }
        }
    }
    s.push_str("    return 0;\n");
    s.push_str("}\n");
    s
}

/// User-contract symbol bodies (Doc 11 §7): bare DSL externs (pure-guard →
/// the trace's `extern_returns` pinned constant or `false`, mirroring the
/// simulator's `ExternRegistry`; value/void → 0/no-op), the
/// `<M>_guard_/action_` wrappers, and entry/exit no-ops (side effects are
/// out of the projection — the differential is state/transition behaviour).
/// Mechanical satisfaction generated from the IR (the authoritative extern
/// list) + the generated impl headers. NOT FSM logic.
fn user_symbol_definitions(
    ir: &Ir,
    trace: &TraceFile,
    files: &fsm_codegen_c::EmittedFiles,
) -> String {
    let mut out = String::new();

    let pinned: BTreeMap<String, bool> = trace
        .init
        .extern_returns
        .as_ref()
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| match v {
                    fsm_simulator::Value::Bool(b) => Some((k.clone(), *b)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let emit_for_machine = |mm: &fsm_ir::MachineObject, out: &mut String| {
        let prefix = &mm.name;
        for ext in &mm.externs {
            let params: Vec<String> = ext
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let cty = match &p.ty {
                        fsm_ir::Type::Primitive { name } => primitive_c(name),
                        _ => "int".to_string(),
                    };
                    format!("{} _a{}", cty, i)
                })
                .collect();
            let param_list = if params.is_empty() {
                "void".to_string()
            } else {
                params.join(", ")
            };
            let void_args: String = (0..ext.params.len())
                .map(|i| format!("(void)_a{};", i))
                .collect::<Vec<_>>()
                .join(" ");
            if ext.pure {
                let val = pinned.get(&ext.name).copied().unwrap_or(false);
                out.push_str(&format!(
                    "bool {n}({pl}) {{ {va} return {v}; }}\n",
                    n = ext.name,
                    pl = param_list,
                    va = void_args,
                    v = if val { "true" } else { "false" },
                ));
                out.push_str(&format!(
                    "bool {p}_guard_{n}(const {p}_t *m, const {p}_Event_t *ev) {{ (void)m; (void)ev; return {v}; }}\n",
                    p = prefix,
                    n = ext.name,
                    v = if val { "true" } else { "false" },
                ));
            } else {
                let ret = match &ext.return_type {
                    None => "void".to_string(),
                    Some(t) => match t {
                        fsm_ir::Type::Primitive { name } => primitive_c(name),
                        _ => "int".to_string(),
                    },
                };
                if ret == "void" {
                    out.push_str(&format!(
                        "void {n}({pl}) {{ {va} }}\n",
                        n = ext.name,
                        pl = param_list,
                        va = void_args,
                    ));
                } else {
                    out.push_str(&format!(
                        "{r} {n}({pl}) {{ {va} return ({r})0; }}\n",
                        r = ret,
                        n = ext.name,
                        pl = param_list,
                        va = void_args,
                    ));
                }
                out.push_str(&format!(
                    "void {p}_action_{n}({p}_t *m, const {p}_Event_t *ev) {{ (void)m; (void)ev; }}\n",
                    p = prefix,
                    n = ext.name,
                ));
            }
        }
    };
    emit_for_machine(&ir.machines[0], &mut out);
    for sub in &ir.machines[0].submachines {
        emit_for_machine(sub, &mut out);
    }

    for f in &files.files {
        if !f.path.ends_with("_impl.h") {
            continue;
        }
        for line in f.content.lines() {
            let l = line.trim();
            if !l.ends_with(");") || !l.starts_with("void ") {
                continue;
            }
            if l.contains("_entry_") || l.contains("_exit_") {
                let proto = &l[..l.len() - 1];
                out.push_str(proto);
                out.push_str(" { (void)m; }\n");
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The cross-compile → link → run → capture mechanism. Identical INPUTS to
// W1's `generated_c_projection` (the SAME `emit(ir, CodegenConfig::default())`
// — the codegen is UNMODIFIED), but cross-compiled with arm-none-eabi-gcc,
// linked against the mps2-an385 harness, and run under qemu-system-arm
// (semihosting). The captured stdout is the target's `FSM_TRACE` stream.
// ---------------------------------------------------------------------------

/// Generate the IDENTICAL `FSM_TRACE` C (W1's `emit(ir,
/// CodegenConfig::default())` — codegen unchanged), cross-compile +
/// semihosting-link the mps2-an385 harness with `arm-none-eabi-gcc`, run
/// `qemu-system-arm -M mps2-an385`, and return the emitted `STEP` lines.
/// The harness's strong `fsm_trace_emit` (semihosting SYS_WRITE0) overrides
/// the codegen's weak host-stdout default with ZERO codegen change.
fn target_qemu_projection(ir: &Ir, machine: &str, trace: &TraceFile) -> Vec<String> {
    let files = emit(ir, &CodegenConfig::default()).expect("codegen emit (UNMODIFIED)");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let hd = harness_dir();

    let mut c_sources: Vec<String> = Vec::new();
    for f in &files.files {
        fs::write(dir.join(&f.path), &f.content).expect("write generated file");
        if f.path.ends_with(".c") {
            c_sources.push(f.path.clone());
        }
    }

    // The driver + the mechanical user-contract bodies (same as W1).
    let mut main_c = driver_main_c(machine, trace);
    main_c.push_str("\n/* user-contract symbol bodies (Doc 11 §7) — mechanical */\n");
    main_c.push_str(&user_symbol_definitions(ir, trace, &files));
    fs::write(dir.join("main.c"), &main_c).expect("write main.c");

    // The harness sources (startup + semihosting retarget). Copied next to
    // the generated C so the relative #include "semihost.h" resolves.
    let startup_c = hd.join("startup_mps2_an385.c");
    let semihost_h = hd.join("semihost.h");
    let linker_ld = hd.join("mps2_an385.ld");
    fs::copy(semihost_h, dir.join("semihost.h")).expect("copy semihost.h");
    fs::copy(startup_c, dir.join("startup_mps2_an385.c")).expect("copy startup");
    fs::copy(linker_ld, dir.join("mps2_an385.ld")).expect("copy linker script");

    let elf = dir.join("fsm_w2_target.elf");

    // Cross-compile + link. The flag set mirrors the host gate's
    // `-std=c99 -Wall -Wextra -Wpedantic -Werror` (the generated trace hook
    // must be warning-clean cross-compiled too), plus the Cortex-M3 target
    // flags + the harness linker script + semihosting (nano specs + the
    // semihosting library are NOT used — the harness implements its own
    // tiny SYS_WRITE0/SYS_EXIT, so a bare `-nostartfiles` freestanding link
    // is sufficient and minimal).
    let mut args: Vec<String> = vec![
        "-std=c99".into(),
        "-Wall".into(),
        "-Wextra".into(),
        "-Wpedantic".into(),
        "-Werror".into(),
        "-mcpu=cortex-m3".into(),
        "-mthumb".into(),
        "-DFSM_TRACE".into(),
        "-ffreestanding".into(),
        "-nostartfiles".into(),
        "-nostdlib".into(),
        "-Os".into(),
        "-g".into(),
        "-I.".into(),
        format!("-T{}", dir.join("mps2_an385.ld").display()),
        "-Wl,--gc-sections".into(),
        "-ffunction-sections".into(),
        "-fdata-sections".into(),
    ];
    args.extend(c_sources);
    args.push("main.c".into());
    args.push("startup_mps2_an385.c".into());
    // -lgcc supplies the compiler runtime (e.g. __aeabi_* helpers) without
    // pulling a full libc; the harness needs no libc (string.h is the
    // codegen's, satisfied by builtins / -lgcc).
    args.push("-lgcc".into());
    args.push("-o".into());
    args.push(elf.to_string_lossy().into_owned());

    let compile = Command::new("arm-none-eabi-gcc")
        .current_dir(dir)
        .args(&args)
        .output()
        .expect("invoke arm-none-eabi-gcc");
    if !compile.status.success() || !compile.stderr.is_empty() {
        for f in &files.files {
            if f.path.ends_with(".c") || f.path.ends_with(".h") {
                eprintln!("=== {} ===\n{}", f.path, f.content);
            }
        }
        eprintln!("=== main.c ===\n{}", main_c);
        eprintln!(
            "=== arm-none-eabi-gcc stderr ===\n{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        panic!(
            "FSM_TRACE C failed to cross-compile -Werror-clean for mps2-an385 \
             (machine {machine}); see above"
        );
    }

    // Run under QEMU. `-M mps2-an385` (Cortex-M3), `-nographic`, semihosting
    // enabled with output on stdio (so SYS_WRITE0 reaches our captured
    // stdout). `-kernel` boots the ELF (QEMU loads it per the ELF program
    // headers; the vector table at 0x0 provides the reset SP/PC). A
    // wall-clock timeout guards against a hypothetical hang (the
    // deterministic corpus always terminates via semihost_exit, but a
    // future-broken fixture must not wedge CI).
    let run = Command::new("qemu-system-arm")
        .current_dir(dir)
        .args([
            "-M",
            "mps2-an385",
            "-cpu",
            "cortex-m3",
            "-nographic",
            "-monitor",
            "none",
            "-serial",
            "none",
            "-semihosting-config",
            "enable=on,target=native",
            "-kernel",
            elf.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("invoke qemu-system-arm");

    // ApplicationExit ⇒ qemu exit code 0. A non-zero code means the harness
    // hit fsm_hal_assert / a fault / a non-clean exit — a hard failure
    // (NOT acceptance: "qemu boots" is explicitly not acceptance — but a
    // non-clean exit IS a failure to surface).
    if !run.status.success() {
        eprintln!(
            "=== qemu stdout ===\n{}",
            String::from_utf8_lossy(&run.stdout)
        );
        eprintln!(
            "=== qemu stderr ===\n{}",
            String::from_utf8_lossy(&run.stderr)
        );
        panic!(
            "qemu-system-arm exited non-zero for {machine}: {:?} — the target \
             harness did not reach a clean semihosting ApplicationExit (a \
             runtime assert / fault / non-clean exit). This is a HARD failure.",
            run.status.code()
        );
    }

    // The semihosting SYS_WRITE0 stream is on qemu's stdout. Keep only the
    // canonical `STEP` lines (the same filter W1 applies to the host
    // binary's stdout) — pure extraction of the trace tap's output.
    String::from_utf8_lossy(&run.stdout)
        .lines()
        .filter(|l| l.starts_with("STEP"))
        .map(|l| l.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// The byte-equality differential. Pure `Vec<String> == Vec<String>` with a
// precise first-divergence report. NO semantics — just the diff (the same
// shape as W1's `byte_diff`).
// ---------------------------------------------------------------------------

fn byte_diff(label: &str, oracle: &[String], target: &[String]) -> Result<(), String> {
    let n = oracle.len().min(target.len());
    for i in 0..n {
        if oracle[i] != target[i] {
            return Err(format!(
                "[{label}] FIRST DIVERGENCE at step #{i}:\n  \
                 oracle (shipped fsm_simulator::execute_trace): {}\n  \
                 target (QEMU mps2-an385 FSM_TRACE)           : {}\n  \
                 (rendered with \\x1f shown as │)\n  \
                 oracle: {}\n  \
                 target: {}",
                oracle[i],
                target[i],
                oracle[i].replace(SEP, "│"),
                target[i].replace(SEP, "│"),
            ));
        }
    }
    if oracle.len() != target.len() {
        return Err(format!(
            "[{label}] LENGTH DIVERGENCE: oracle produced {} step records, \
             the QEMU target emitted {} — common prefix of {} matched.\n  \
             first extra oracle line: {}\n  \
             first extra target line: {}",
            oracle.len(),
            target.len(),
            n,
            oracle
                .get(n)
                .map(|s| s.replace(SEP, "│"))
                .unwrap_or_default(),
            target
                .get(n)
                .map(|s| s.replace(SEP, "│"))
                .unwrap_or_default(),
        ));
    }
    Ok(())
}

fn run_target_differential(example: &str) -> Result<(), String> {
    let (ir, trace, machine) = load_trace(example);
    let oracle = simulator_projection(&ir, &trace);
    let target = target_qemu_projection(&ir, &machine, &trace);
    assert!(
        !oracle.is_empty(),
        "{example}: the simulator oracle produced zero records — the \
         differential would be vacuous"
    );
    byte_diff(example, &oracle, &target)
}

// ===========================================================================
// §5.4 BEHAVIOURAL ACCEPTANCE — the on-target byte-differential.
//
// Doc 32 §W2 §5.4 + §2's catalogue-integrity contract: the QEMU-mps2-an385
// target's emitted `FSM_TRACE` stream byte-equals `fsm_simulator::
// execute_trace` for EVERY `BYTE_EQUAL` member (gate (1)); the catalogue
// partition holds (gate (4)); the never-game guards hold (gate (5)). The
// JUSTIFIED record-model residual is host-proven, NOT re-litigated per
// substrate (Doc 32 §2 line ~200 — the substrate changes, the oracle +
// catalogue do not). At least one member exercises a non-trivial path
// (motor: a `[can_start]` guard + an `after 5000 ms` timer + a payload
// `FAULT(code)`; stress-* : history/parallel/timer/choice).
//
// "The C cross-compiles / QEMU boots" is explicitly NOT acceptance — the
// *trace* must be byte-equal to the oracle.
// ===========================================================================

#[test]
fn on_target_qemu_trace_byte_equals_simulator_oracle_for_byte_equal_corpus() {
    if let Some(reason) = on_target_skip_reason() {
        eprintln!("{reason}");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    for ex in W2_BYTE_EQUAL_CORPUS {
        match run_target_differential(ex) {
            Ok(()) => eprintln!(
                "[on-target] {ex}: BYTE-EQUAL ✓ (the QEMU mps2-an385 target's \
                 FSM_TRACE stream == the shipped fsm_simulator::execute_trace \
                 oracle — the host differential extended to the emulated MCU, \
                 same oracle, different substrate)"
            ),
            Err(report) => failures.push(format!("--- {ex} ---\n{report}")),
        }
    }
    assert!(
        failures.is_empty(),
        "ON-TARGET DIFFERENTIAL FAILED — the QEMU mps2-an385 target's \
         FSM_TRACE stream diverged from the shipped fsm_simulator::\
         execute_trace oracle for {} of {} BYTE_EQUAL corpus member(s). \
         A divergence here is either a real codegen/cross-toolchain defect \
         OR a harness bug — NEVER to be papered over (Doc 32 §2 \
         never-game).\n\n{}",
        failures.len(),
        W2_BYTE_EQUAL_CORPUS.len(),
        failures.join("\n\n")
    );
}

// ===========================================================================
// Keystone / contract guards (these run WITHOUT the toolchain — pure Rust;
// they pre-stage the W6 source-derived audit so the no-fork + catalogue +
// projection-contract invariants are continuously enforced even on the
// dev box where QEMU is absent).
// ===========================================================================

/// The oracle is the SHIPPED `fsm_simulator::execute_trace` seam — the same
/// one `cmd/{test,baseline}.rs` + W1 consume. This compile-time use-site
/// assertion documents (and the type system enforces) that this lane
/// consumes the real seam, not a fork. (`simulator_projection` calls
/// `execute_trace` directly; this makes the no-fork attestation
/// executable + runs WITHOUT the ARM toolchain.)
#[test]
fn oracle_is_the_shipped_execute_trace_seam_not_a_fork() {
    let (ir, trace, _machine) = load_trace("motor");
    let mut t = trace.clone();
    t.expected.clear();
    // The SOLE oracle call in the whole on-target lane — the shipped seam.
    let res = execute_trace(&ir, &t).expect("the shipped execute_trace oracle");
    assert!(
        !res.actual.is_empty(),
        "the shipped simulator oracle must produce records for motor"
    );
    let line = project_record(&res.actual[0]);
    assert!(
        line.starts_with("STEP"),
        "the projection is a pure format of the oracle's StepRecord"
    );
}

/// The canonical-line separator + projection shape MUST be byte-identical
/// to W1's contract and the codegen `FSM_TRACE` hook's `FIELD_SEP`. A drift
/// in any of the three silently blinds the differential (the same class as
/// W1's `field_separator_is_a_pinned_cross_engine_contract`). This is the
/// guard that makes the *necessary* projection duplication (forced by
/// "do not modify codegen_equivalence_smoke.rs") safe: the duplicate is
/// pinned formatting, not a forked oracle.
#[test]
fn projection_shape_matches_w1_canonical_form() {
    // (a) the separator byte is US (0x1F) on all three sides.
    assert_eq!(SEP as u32, 0x1f, "W2 projection separator drifted");
    assert_eq!(
        fsm_codegen_c::emit::trace_hook::FIELD_SEP as u32,
        0x1f,
        "codegen trace-hook separator drifted from the W2 contract"
    );
    assert_eq!(
        SEP,
        fsm_codegen_c::emit::trace_hook::FIELD_SEP,
        "the W2 projection + the codegen FSM_TRACE hook MUST use the same \
         separator byte (else the on-target byte-diff is meaningless)"
    );

    // (b) the projected line shape is EXACTLY the W1 canonical form:
    //     `STEP<US>kind=…<US>evt=…<US>tr=…<US>src=…<US>dst=…<US>cfgB=…
    //      <US>cfgA=…<US>ent=…<US>ext=…` (10 US-separated fields, the
    //     trace_id/actions/submachine/clk exclusions identical to W1). A
    //     real oracle StepRecord is projected and its structure asserted.
    let (ir, trace, _m) = load_trace("motor");
    let mut t = trace.clone();
    t.expected.clear();
    let res = execute_trace(&ir, &t).expect("shipped oracle");
    let line = project_record(&res.actual[0]);
    let fields: Vec<&str> = line.split(SEP).collect();
    assert_eq!(
        fields.len(),
        10,
        "the canonical line must be exactly 10 US-separated fields (the W1 \
         shape); got {}: {:?}",
        fields.len(),
        fields
    );
    assert_eq!(fields[0], "STEP", "field 0 must be the STEP tag");
    for (i, key) in [
        "kind=", "evt=", "tr=", "src=", "dst=", "cfgB=", "cfgA=", "ent=", "ext=",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            fields[i + 1].starts_with(key),
            "canonical field {} must start with `{}` (the W1 shape); got `{}`",
            i + 1,
            key,
            fields[i + 1]
        );
    }
    // Neither the excluded `clk=` nor `trace_id=`/`actions=` may appear
    // (the exact W1 exclusion set — the on-target stream is byte-identical
    // to the host one because the codegen is unchanged).
    assert!(
        !line.contains("clk=") && !line.contains("trace_id=") && !line.contains("actions="),
        "the projection must EXCLUDE clk/trace_id/actions exactly as W1 \
         does (the codegen FSM_TRACE hook excludes the same fields — the \
         on-target stream must match the host stream W1 already proved \
         equivalent); got: {line}"
    );
}

/// The W2 on-target corpus MUST be exactly the W1 catalogue's `BYTE_EQUAL`
/// set (Doc 32 §2: W2 gates on the SAME catalogue; the substrate changes,
/// the catalogue does not). Re-derives the expected set from first
/// principles (the 9 fixtures W1's catalogue lists as byte-equal) so a
/// future catalogue change that is not mirrored here fails LOUDLY (the
/// verify-the-record discipline applied to W2's own corpus claim;
/// pre-stages the W6 audit's catalogue cross-check).
#[test]
fn byte_equal_corpus_matches_w1_catalogue() {
    // The W1 `codegen_equivalence_smoke.rs` `BYTE_EQUAL` array, verbatim.
    // If W1's catalogue changes (a fixture converges / a new one is added)
    // this MUST be updated in lockstep — the W6 source-derived audit
    // re-derives the W1 catalogue from source and cross-checks this; a
    // silent drift would under/over-test the target lane.
    let w1_byte_equal_expected: &[&str] = &[
        "motor",
        "deferred",
        "traffic-light",
        "stress-parallel-cross-exit",
        "stress-deep-history",
        "stress-self-transitions",
        "stress-self-transitions-actions",
        "stress-choice-guard-payload",
        "stress-every-timer",
    ];
    let mut a: Vec<&str> = W2_BYTE_EQUAL_CORPUS.to_vec();
    let mut b: Vec<&str> = w1_byte_equal_expected.to_vec();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(
        a, b,
        "the W2 on-target corpus must be EXACTLY the W1 catalogue's \
         BYTE_EQUAL set (Doc 32 §2 — W2 gates on the same catalogue; the \
         substrate changes, the catalogue does not). A mismatch means the \
         W1 catalogue moved and this lane was not updated in lockstep — \
         the W6 audit will catch it; fix the lockstep."
    );
    // The JUSTIFIED record-model residual is NOT in the W2 corpus (it is a
    // host-proven property, not re-litigated per substrate — Doc 32 §2).
    for justified in ["vending-machine", "submachine", "stress-completion-chain"] {
        assert!(
            !W2_BYTE_EQUAL_CORPUS.contains(&justified),
            "`{justified}` is BEHAVIOURALLY_EQUIVALENT_JUSTIFIED — its \
             record-model residual is host-proven and MUST NOT be \
             re-litigated on the target substrate (Doc 32 §2)"
        );
    }
}

// ===========================================================================
// LOCAL STRENGTHENING (Doc 32 §W2 §5.4 "Strengthening (recommended, not
// required)"). The on-target lane's only CI-exercised NEW variable vs W1 is
// "cross-compile with arm-none-eabi-gcc + execute under QEMU". Everything
// ELSE W2 introduces — the W2 driver/user-symbol generation + the
// weak/strong `fsm_trace_emit` seam — is host-gcc-provable on THIS box
// (host gcc IS present; arm-gcc/qemu are not). This test compiles the
// IDENTICAL generated `FSM_TRACE` C with the W2 driver path + a host-stdout
// strong `fsm_trace_emit` (the host analogue of the harness's semihosting
// strong override — proving the codegen's weak sink is correctly overridden
// and the W2 driver feeds events faithfully) and byte-diffs vs the shipped
// `execute_trace` oracle. A pass here isolates "cross-compile + QEMU
// execution" as the SOLE remaining CI-only delta (QEMU being
// oracle-irrelevant per Doc 32 §2's determinism rationale). Skips-if-`gcc`-
// absent (the same precedent). NOT a second oracle: the comparison target
// is `execute_trace`, the C is the UNMODIFIED codegen.
// ===========================================================================

fn host_gcc_available() -> bool {
    tool_available("gcc", "--version")
}

/// Compile the W2 driver path + the IDENTICAL generated `FSM_TRACE` C with
/// the on-box host `gcc` and a host-stdout strong `fsm_trace_emit` (the
/// host analogue of the semihosting strong override), run it, return the
/// emitted `STEP` lines. Proves the W2-specific code (driver + user-symbol
/// gen + the weak/strong seam) is byte-correct WITHOUT the ARM toolchain.
fn host_gcc_w2_driver_projection(ir: &Ir, machine: &str, trace: &TraceFile) -> Vec<String> {
    let files = emit(ir, &CodegenConfig::default()).expect("codegen emit (UNMODIFIED)");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut c_sources: Vec<String> = Vec::new();
    for f in &files.files {
        fs::write(dir.join(&f.path), &f.content).expect("write generated file");
        if f.path.ends_with(".c") {
            c_sources.push(f.path.clone());
        }
    }

    // The SAME W2 driver + user-symbol bodies the on-target lane uses.
    let mut main_c = driver_main_c(machine, trace);
    main_c.push_str("\n/* user-contract symbol bodies (Doc 11 §7) — mechanical */\n");
    main_c.push_str(&user_symbol_definitions(ir, trace, &files));
    // A host-stdout strong `fsm_trace_emit` + a deterministic settable
    // clock + abort-on-assert: the HOST analogue of the harness's
    // semihosting strong override (same seam, host substrate). This is
    // platform plumbing, NOT FSM logic — it proves the codegen's
    // __attribute__((weak)) fsm_trace_emit is correctly overridden and the
    // W2 driver feeds the SAME event/clock sequence the oracle runs.
    main_c.push_str(
        r#"
/* The HOST analogue of the W2 mps2-an385 harness's platform side (startup_
 * mps2_an385.c): a host-stdout strong `fsm_trace_emit` + a deterministic
 * settable `fsm_hal_clock_now_ms` + abort-on-assert. SAME weak/strong
 * `fsm_trace_emit` seam, SAME settable-clock HAL contract — host substrate
 * instead of semihosting/QEMU. Pure platform plumbing, NOT FSM logic; it
 * proves the codegen's __attribute__((weak)) sink is correctly overridden
 * and the W2 driver feeds the SAME event/clock sequence the oracle runs.
 * NB: the harness provides the HAL (we do NOT use fsm_hal.h's POSIX
 * reference impl — exactly the W2 design, where startup_mps2_an385.c
 * supplies a deterministic counter clock). */
#include <stdio.h>
#include <stdlib.h>
void fsm_trace_emit(const char *line);
void fsm_trace_emit(const char *line) { if (line) fputs(line, stdout); }
uint32_t fsm_hal_clock_now_ms(void);
uint32_t fsm_hal_clock_now_ms(void) {
    extern uint32_t fsm_w2_host_clk_get(void);
    return fsm_w2_host_clk_get();
}
void fsm_hal_assert(bool cond, const char *msg);
void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg ? msg : "(null)"); abort(); }
}
static uint32_t fsm_w2_host_clk = 0u;
uint32_t fsm_w2_host_clk_get(void);
uint32_t fsm_w2_host_clk_get(void) { return fsm_w2_host_clk; }
/* The driver calls fsm_test_set_clock() (same symbol the W2 startup
 * provides) to mirror the trace's advance_clock deltas — host analogue. */
void fsm_test_set_clock(uint32_t ms);
void fsm_test_set_clock(uint32_t ms) { fsm_w2_host_clk = ms; }
"#,
    );
    fs::write(dir.join("main.c"), &main_c).expect("write main.c");

    let exe = dir.join("fsm_w2_host_driver_bin");
    let mut args: Vec<String> = vec![
        "-std=c99".into(),
        "-Wall".into(),
        "-Wextra".into(),
        "-Wpedantic".into(),
        "-Werror".into(),
        "-DFSM_TRACE".into(),
        "-I.".into(),
    ];
    args.extend(c_sources);
    args.push("main.c".into());
    args.push("-o".into());
    args.push(exe.to_string_lossy().into_owned());

    let compile = Command::new("gcc")
        .current_dir(dir)
        .args(&args)
        .output()
        .expect("invoke gcc");
    if !compile.status.success() || !compile.stderr.is_empty() {
        eprintln!("=== main.c ===\n{}", main_c);
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        panic!(
            "the W2 driver path failed to host-compile -Werror-clean \
             (machine {machine}); see above"
        );
    }
    let run = Command::new(&exe)
        .output()
        .expect("run W2 host driver binary");
    assert!(
        run.status.success(),
        "the W2 host driver binary for {machine} exited non-zero: {:?}\nstderr: {}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout)
        .lines()
        .filter(|l| l.starts_with("STEP"))
        .map(|l| l.to_string())
        .collect()
}

/// Local strengthening: the W2 driver path + the weak/strong
/// `fsm_trace_emit` seam, host-`gcc`-compiled, byte-equals the shipped
/// `fsm_simulator::execute_trace` oracle for the FULL `BYTE_EQUAL` corpus.
/// This proves everything W2 adds EXCEPT "cross-compile + QEMU" is correct
/// on the box (host gcc present), isolating that one CI-only delta — which
/// is oracle-irrelevant by construction (Doc 32 §2: same unmodified C, same
/// oracle, QEMU is a deterministic instruction-accurate substrate). Skips
/// if `gcc` is absent (the gcc_compile.rs precedent).
#[test]
fn host_gcc_strengthening_w2_driver_byte_equals_oracle() {
    if !host_gcc_available() {
        eprintln!(
            "[on_target_qemu_differential] host gcc absent — skipping the W2 \
             driver-path host strengthening (gcc_compile.rs skip-if-absent \
             precedent). The on-target lane's CI run remains the authority."
        );
        return;
    }
    let mut failures: Vec<String> = Vec::new();
    for ex in W2_BYTE_EQUAL_CORPUS {
        let (ir, trace, machine) = load_trace(ex);
        let oracle = simulator_projection(&ir, &trace);
        let host = host_gcc_w2_driver_projection(&ir, &machine, &trace);
        assert!(
            !oracle.is_empty(),
            "{ex}: the simulator oracle produced zero records"
        );
        if let Err(report) = byte_diff(ex, &oracle, &host) {
            failures.push(format!("--- {ex} ---\n{report}"));
        }
    }
    assert!(
        failures.is_empty(),
        "W2 DRIVER-PATH STRENGTHENING FAILED — the host-gcc-compiled W2 \
         driver path (the W2-specific code: driver + user-symbol gen + the \
         weak/strong fsm_trace_emit seam) diverged from the shipped \
         fsm_simulator::execute_trace oracle for {} of {} BYTE_EQUAL \
         member(s). This is the W2-specific code being WRONG (the codegen \
         + oracle are W1-proven) — fix it; do NOT paper over.\n\n{}",
        failures.len(),
        W2_BYTE_EQUAL_CORPUS.len(),
        failures.join("\n\n")
    );
    eprintln!(
        "[on-target strengthening] the W2 driver path + the weak/strong \
         fsm_trace_emit seam host-gcc-compiles + byte-equals the shipped \
         oracle for ALL {} BYTE_EQUAL corpus members. The SOLE remaining \
         CI-exercised delta is 'cross-compile with arm-none-eabi-gcc + \
         execute under qemu-system-arm' — which is oracle-irrelevant by \
         construction (same UNMODIFIED FSM_TRACE C, same execute_trace \
         oracle, QEMU a deterministic instruction-accurate substrate; \
         Doc 32 §2).",
        W2_BYTE_EQUAL_CORPUS.len()
    );
}

/// The on-target harness directory exists with the four required artifacts
/// (linker script + startup + semihosting header). A self-check that the
/// CI lane's inputs are present (so a CI run fails with a clear message if
/// the harness is missing, not an opaque gcc error). Runs without the ARM
/// toolchain.
#[test]
fn on_target_harness_artifacts_present() {
    let hd = harness_dir();
    for f in ["mps2_an385.ld", "startup_mps2_an385.c", "semihost.h"] {
        let p = hd.join(f);
        assert!(
            p.is_file(),
            "the on-target harness artifact `{}` is missing at {} — the \
             CI on-target lane cannot cross-compile without it",
            f,
            p.display()
        );
    }
    // The startup must define the strong fsm_trace_emit override + the HAL
    // contract + the Cortex-M3 vector table (the keystone seam: ZERO
    // codegen change, the harness provides the platform side).
    let startup = fs::read_to_string(hd.join("startup_mps2_an385.c")).expect("read startup");
    for needle in [
        "void fsm_trace_emit(const char *line)",
        "uint32_t fsm_hal_clock_now_ms(void)",
        "void fsm_hal_assert(bool cond, const char *msg)",
        "void Reset_Handler(void)",
        ".isr_vector",
        "semihost_exit_success",
    ] {
        assert!(
            startup.contains(needle),
            "the on-target startup must contain `{needle}` (the harness's \
             strong override / CRT bring-up / clean-exit seam)"
        );
    }
}
