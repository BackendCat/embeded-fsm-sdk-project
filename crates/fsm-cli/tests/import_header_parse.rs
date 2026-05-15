//! C-declaration extractor coverage + conflict resolution, driven through
//! the real `fsm generate --import-header … --emit-ir` surface.
//!
//! The extractor itself (`crate::import_header`) is a private module of the
//! `fsm` binary crate, so its exhaustive unit tests live INLINE there
//! (`cargo test --bin fsm import_header::`, 25+ cases incl. a deliberately
//! gnarly resilience header). THIS integration file proves the same
//! behaviour *through the CLI*: which declarations become IR externs, which
//! are safely skipped, and the DSL-wins conflict rule — i.e. the extractor
//! is wired correctly into `generate`, not just unit-correct in isolation.

#[path = "common/mod.rs"]
mod common;

use std::fs;

use assert_cmd::Command as Assert;
use serde_json::Value;

/// Write `header` + a tiny `.fsm` that declares no externs, run
/// `fsm generate --import-header header --emit-ir`, and return the IR's
/// extern objects for the single machine as `(name, json)` pairs.
fn imported_externs(header: &str, fsm_body: &str) -> Vec<(String, Value)> {
    let tmp = tempfile::tempdir().expect("tempdir");
    let hp = tmp.path().join("hdr.h");
    let fp = tmp.path().join("m.fsm");
    fs::write(&hp, header).unwrap();
    fs::write(&fp, fsm_body).unwrap();
    let out = tmp.path().join("out");

    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--emit-ir")
        .arg("--import-header")
        .arg(&hp)
        .arg(&fp)
        .arg("--out")
        .arg(&out)
        .assert()
        .success();

    let ir_path = out.join("M.ir.json");
    let raw = fs::read_to_string(&ir_path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", ir_path.display()));
    let v: Value = serde_json::from_str(&raw).expect("valid IR json");
    let machine = &v["machines"][0];
    machine["externs"]
        .as_array()
        .expect("externs array")
        .iter()
        .map(|e| (e["name"].as_str().unwrap().to_string(), e.clone()))
        .collect()
}

/// Minimal machine whose single action just calls every imported function
/// named in `calls` (so they resolve; an unreferenced extern is still
/// emitted, but referencing them also exercises FSM-E0102 resolution).
fn fsm_calling(calls: &[&str]) -> String {
    let body = calls
        .iter()
        .map(|c| format!("{c}()"))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "language fsm 2.0\n\nmachine M {{\n  events {{ GO }}\n  initial S\n  \
         state S {{ on GO -> S : {body} }}\n}}\n"
    )
}

fn names(externs: &[(String, Value)]) -> Vec<&str> {
    externs.iter().map(|(n, _)| n.as_str()).collect()
}

#[test]
fn scalar_signatures_import_with_correct_ir_types() {
    let ex = imported_externs(
        "uint16_t adc_read(uint8_t ch);\nvoid relay_set(bool on);\n\
         extern uint8_t crc8(uint8_t seed);\n",
        &fsm_calling(&["relay_set"]),
    );
    assert_eq!(names(&ex), ["adc_read", "relay_set", "crc8"]);

    let adc = &ex[0].1;
    assert_eq!(adc["returnType"]["kind"], "primitive");
    assert_eq!(adc["returnType"]["name"], "u16");
    assert_eq!(adc["params"][0]["type"]["name"], "u8");

    // `void` return ⇒ no returnType key (same as a DSL `extern foo()`).
    assert!(ex[1].1.get("returnType").is_none());
}

#[test]
fn macros_typedefs_struct_bodies_are_skipped_not_imported() {
    let ex = imported_externs(
        "#define LED_ON() gpio_set(13,1)\n\
         typedef unsigned int word_t;\n\
         struct cfg { int a; int b; };\n\
         enum mode { M_A, M_B };\n\
         void go(void);\n",
        &fsm_calling(&["go"]),
    );
    // Only the real function survives the type/macro machinery.
    assert_eq!(names(&ex), ["go"]);
}

#[test]
fn pointer_and_struct_param_functions_are_skipped() {
    // The DSL extern lowering can't model opaque param/return types, so a
    // pointer/struct-signature function is skipped rather than emitted with
    // silently-dropped params (resilience > completeness, Doc 18 §5).
    let ex = imported_externs(
        "void dma_xfer(const uint8_t *src, uint32_t n);\n\
         struct h *open_dev(uint8_t id);\n\
         void tick(uint32_t ms);\n",
        &fsm_calling(&["tick"]),
    );
    assert_eq!(
        names(&ex),
        ["tick"],
        "only the scalar-only function should import"
    );
}

#[test]
fn variadic_keeps_fixed_scalar_prefix() {
    let ex = imported_externs("void trace(uint8_t lvl, ...);\n", &fsm_calling(&["trace"]));
    assert_eq!(names(&ex), ["trace"]);
    assert_eq!(ex[0].1["params"].as_array().unwrap().len(), 1);
    assert_eq!(ex[0].1["params"][0]["type"]["name"], "u8");
}

#[test]
fn attribute_and_storage_specifiers_are_stripped() {
    let ex = imported_externs(
        "extern void __attribute__((noreturn)) halt(uint8_t code);\n\
         static inline uint8_t lo(uint8_t v) { return v & 0x0f; }\n",
        &fsm_calling(&["halt", "lo"]),
    );
    assert_eq!(names(&ex), ["halt", "lo"]);
    assert_eq!(ex[0].1["params"][0]["type"]["name"], "u8");
    assert_eq!(ex[1].1["returnType"]["name"], "u8");
}

/// DSL-wins conflict: an `extern` declared in the `.fsm` AND provided by an
/// imported header → the DSL signature is kept, the import ignored, a note
/// emitted, and `generate` still succeeds.
#[test]
fn dsl_extern_wins_over_imported_same_name() {
    let tmp = tempfile::tempdir().unwrap();
    let hp = tmp.path().join("h.h");
    let fp = tmp.path().join("m.fsm");
    // Header says `uint8_t shared(uint8_t)`; the .fsm says `shared(u16 w)`.
    fs::write(
        &hp,
        "uint8_t shared(uint8_t narrow);\nvoid header_only(void);\n",
    )
    .unwrap();
    fs::write(
        &fp,
        "language fsm 2.0\n\nextern shared(u16 w)\n\nmachine M {\n  \
         events { GO }\n  initial S\n  state S { on GO -> S : shared(1); \
         header_only() }\n}\n",
    )
    .unwrap();
    let out = tmp.path().join("out");

    let assert = Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--import-header")
        .arg(&hp)
        .arg(&fp)
        .arg("--out")
        .arg(&out)
        .assert()
        .success();

    // The conflict note is surfaced (once).
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("shared") && stderr.contains("the .fsm declaration wins"),
        "expected DSL-wins conflict note, stderr was:\n{stderr}"
    );

    // The emitted prototype is the DSL one (`uint16_t w`), NOT the header's
    // `uint8_t narrow` — proof the DSL declaration genuinely won.
    let impl_h = fs::read_to_string(out.join("M_impl.h")).unwrap();
    assert!(
        impl_h.contains("void shared(uint16_t w);"),
        "DSL signature must win; impl.h:\n{impl_h}"
    );
    assert!(
        !impl_h.contains("uint8_t narrow"),
        "header signature must NOT appear; impl.h:\n{impl_h}"
    );
    // `header_only` was not in the .fsm, so it imports normally.
    assert!(impl_h.contains("void header_only(void);"));
}

/// A gnarly header full of unparseable constructs must NOT crash `generate`
/// and must still extract the one good function (CLI-level resilience proof
/// mirroring the inline `gnarly_header_does_not_panic_and_skips_cleanly`).
#[test]
fn gnarly_header_does_not_crash_generate() {
    let header = r#"
#ifndef G_H
#define G_H
#include <stdint.h>
#define VER "1;2)"
#define CLAMP(x,h) ((x)>(h)?(h):(x))
typedef int (*cb_t)(void*, uint32_t);
typedef struct { uint32_t a:3; uint32_t b:29; } bits_t;
struct fwd;
enum e { E0, E1 };
extern volatile uint32_t g;
int (*pick(void))(int);
void arr_param(int a[static 2]);
const struct sensor * get_sensor(uint8_t i);   /* opaque ret -> skip */
uint16_t good_read(uint8_t ch);                /* the one good one  */
#endif
"#;
    let ex = imported_externs(header, &fsm_calling(&["good_read"]));
    assert_eq!(
        names(&ex),
        ["good_read"],
        "exactly the scalar function should survive the noise"
    );
    assert_eq!(ex[0].1["returnType"]["name"], "u16");
}

/// `fsm.toml [generate] import_headers = [...]` is an equivalent
/// project-wide surface to the `--import-header` CLI flag. A header listed
/// there (resolved relative to the `.fsm`, like `fsm.toml` discovery) must
/// be imported with no CLI flag at all.
#[test]
fn fsm_toml_import_headers_array_is_honoured() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("hal.h"),
        "uint16_t adc_get(uint8_t ch);\nvoid led(bool enable);\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("fsm.toml"),
        "[generate]\nimport_headers = [\"hal.h\"]\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join("m.fsm"),
        "language fsm 2.0\n\nmachine M {\n  events { GO }\n  initial S\n  \
         state S { on GO -> S : led(); adc_get(0) }\n}\n",
    )
    .unwrap();
    let out = tmp.path().join("out");

    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(tmp.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .success();

    let impl_h = fs::read_to_string(out.join("M_impl.h")).unwrap();
    assert!(
        impl_h.contains("uint16_t adc_get(uint8_t ch);")
            && impl_h.contains("void led(bool enable);"),
        "fsm.toml import_headers not honoured; impl.h:\n{impl_h}"
    );
}
