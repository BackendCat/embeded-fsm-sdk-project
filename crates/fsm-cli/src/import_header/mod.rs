//! `--import-header` — derive `extern` declarations from a C header.
//!
//! ## Why this exists
//!
//! Users with an existing embedded C codebase (HAL, drivers) otherwise have
//! to hand-transcribe every function they want to call from a guard/action
//! into a DSL `extern` declaration. This module reads a `.h` file, extracts
//! the function declarations it can confidently model into
//! [`fsm_ir::ExternObject`]s, and renders them back as DSL `extern` text
//! ([`ImportedHeader::to_dsl_externs`]) which `cmd::generate` splices into
//! the `.fsm` source *before parsing* — so an imported extern flows through
//! the entire normal pipeline and is indistinguishable downstream from one
//! declared in the `.fsm` (ROADMAP v1.1 W5, Doc 18). See
//! [`ImportedHeader::to_dsl_externs`] for why source-synthesis (not
//! post-IR injection) is the correct injection point.
//!
//! ## Design constraints (Doc 23 §4 dependency rules)
//!
//! C-header parsing is a CLI-input convenience, so it lives in `fsm-cli`, NOT
//! a core pipeline crate. We deliberately do **not** shell out to a real C
//! compiler or link libclang: that would add a heavy, non-deterministic
//! system dependency to a compile-time tool. Instead this is a focused,
//! hand-rolled tokenizer + declaration matcher covering the common forms in
//! realistic embedded headers.
//!
//! ## Resilience over completeness
//!
//! A header line we cannot *confidently* turn into an extern is **skipped
//! with an `fsm-W`-level note**, never a hard error and never a misparse. A
//! gnarly real-world header full of macros, typedefs, struct bodies and
//! attributes must not crash the tool or produce a broken `extern`. The
//! supported subset is documented in Doc 18 §5 `fsm generate`.
//!
//! ## Supported C subset
//!
//! - Function declarations: `ret name(params);` and `extern ret name(params);`
//! - Return / parameter base types: `void`, `bool`/`_Bool`, `char`,
//!   `signed/unsigned char/short/int/long/long long`, `float`, `double`,
//!   and the fixed-width `<stdint.h>` set (`uint8_t` … `int64_t`,
//!   `size_t`, `uintptr_t`/`intptr_t`).
//! - One level of pointer (`T *`, `const T *`) → `opaque "T *"`.
//! - `struct X` / `struct X *` / `enum X` / `union X` → `opaque` verbatim.
//! - `const` qualifiers (ignored for IR type purposes; preserved in opaque
//!   spellings).
//! - `void` parameter list (`f(void)`) → zero params.
//! - Unnamed parameters (`int f(int);`) → synthesised `arg0`, `arg1`, ….
//! - Variadic `...` → the variadic marker is dropped; the fixed prefix
//!   params are still imported (an FSM action can only pass the fixed
//!   args anyway; see skip note).
//!
//! Everything else — `#define`/`#if` and other preprocessor lines,
//! `typedef`s, standalone `struct`/`enum`/`union` *definitions* (with a
//! body), `__attribute__((...))`, `__declspec`, K&R-style declarations,
//! function-pointer parameters, array parameters, multi-declarator lines —
//! is skipped with a note. Comments (`//` and `/* */`) and string/char
//! literals are stripped before matching so a `;` inside them never
//! terminates a phantom declaration.
//!
//! AUDIT_2026_06_06 §6 P2.1 — split from a single 1405-LOC `import_header.rs`
//! into stage-aligned submodules. Public API (`ImportedHeader`,
//! `SkipNote`, `parse_header`, `parse_header_file`) unchanged.

use std::path::Path;

use fsm_ir::ExternObject;

mod classify;
mod dsl_emit;
mod preproc;
mod types;
mod util;

use self::classify::{classify, finalize, opaque_in_signature, Decl};
use self::dsl_emit::render_extern_decl;
use self::preproc::{split_top_level_decls, strip_comments_and_preproc};
use self::util::condense_ws;

/// One skipped construct, surfaced to the user as an `fsm-W`-level note so
/// they know *why* a function they expected did not become an extern.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SkipNote {
    /// The (trimmed, comment-stripped) source fragment we declined to model.
    pub(crate) fragment: String,
    /// Human-readable reason.
    pub(crate) reason: String,
}

/// Result of importing one header: the externs we could model + the notes
/// for everything we deliberately skipped.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct ImportedHeader {
    pub(crate) externs: Vec<ExternObject>,
    pub(crate) skipped: Vec<SkipNote>,
}

/// Parse a C header file's text into importable externs.
///
/// `header_label` is used only for the synthesized [`SourceLocation::file`]
/// so diagnostics/`emit` can attribute an imported extern to its origin
/// header rather than the `.fsm`.
pub(crate) fn parse_header(src: &str, header_label: &str) -> ImportedHeader {
    let mut out = ImportedHeader::default();
    let stripped = strip_comments_and_preproc(src, &mut out.skipped);

    // Split on top-level `;` and `}` so each candidate is a single
    // declaration. Brace depth is tracked so a struct/enum body's inner
    // `;`s don't fragment it — the whole `struct {...} ;` lands as one
    // chunk and is then recognised as a definition we skip.
    for chunk in split_top_level_decls(&stripped) {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        match classify(trimmed) {
            Decl::Function(f) => {
                // Pre-existing pipeline reality (verified, NOT this
                // feature's regression): the DSL `extern` lowering path
                // (`fsm-analyzer` `lower_type_ref`) and the extern-param
                // grammar (`can_start_type`) only model the primitive type
                // keywords — an `opaque "T"` param or return on an extern
                // is silently dropped even when hand-written in a `.fsm`.
                // Emitting a synthesized extern with a pointer/struct in
                // its signature would therefore produce a *wrong* prototype
                // (params silently lost), so we SKIP such a function with a
                // precise note instead. This is correct in practice: a DSL
                // guard/action has no way to construct a `struct *`/pointer
                // argument anyway, so a pointer-param HAL function is not
                // callable from FSM logic regardless. Scalar-only functions
                // — the overwhelming majority used from guards/actions —
                // import perfectly. (Resilience > completeness; documented
                // in Doc 18 §5. Reported as a STOP-worthy pipeline gap.)
                if let Some(why) = opaque_in_signature(&f) {
                    out.skipped.push(SkipNote {
                        fragment: condense_ws(trimmed),
                        reason: why,
                    });
                } else {
                    out.externs.push(finalize(f, header_label));
                }
            }
            Decl::Skip(reason) => out.skipped.push(SkipNote {
                fragment: condense_ws(trimmed),
                reason,
            }),
        }
    }
    out
}

/// Convenience: read + parse a header file from disk.
///
/// SECURITY (SEC-P0-1): the read is **size-capped** at the same ceiling the
/// parser enforces on `.fsm` source, via the shared bounded-read primitive,
/// BEFORE the unbounded `read_to_string` + the tokenizer's `O(n)`
/// `String::with_capacity` allocations. `--import-header /dev/zero` or a
/// multi-GB header is rejected with a clean `io::Error` (mapped to a
/// non-zero exit by the caller), never an OOM/panic. Path *containment* is
/// enforced separately at the call site (`cmd::generate`) per the trust
/// model — this function only owns the DoS cap.
pub(crate) fn parse_header_file(path: &Path) -> std::io::Result<ImportedHeader> {
    let src = crate::safe_io::read_to_string_capped(path, crate::safe_io::MAX_INPUT_BYTES)?;
    let label = path.to_string_lossy().into_owned();
    Ok(parse_header(&src, &label))
}

impl ImportedHeader {
    /// Render the imported externs as DSL `extern` declaration lines.
    ///
    /// ## Why source synthesis (the injection-point decision)
    ///
    /// Extern *name resolution* (`FSM-E0102` "unknown extern") runs in the
    /// analyzer's `checks::name_resolution`, which queries the **symbol
    /// table built from the AST** — not the IR. Injecting `ExternObject`s
    /// into the post-analysis IR is therefore too late: the analyzer would
    /// already have rejected a guard/action that calls an imported function.
    ///
    /// To make an imported extern *truly* indistinguishable from a DSL one
    /// (the brief's hard requirement) it must flow through the **entire**
    /// normal pipeline — parser → symbol table → name resolution → lowering
    /// → codegen. The faithful, zero-core-change way to do that from
    /// `fsm-cli` is to emit DSL `extern` text and prepend it to the source
    /// before `parse`. The synthesized lines re-parse into exactly the same
    /// AST/IR shape a hand-written `extern` would, so every downstream stage
    /// (including E0102) treats them identically. No lexer/parser/analyzer/
    /// ir/codegen logic is touched — only `fsm-cli` string assembly.
    ///
    /// `skip_names` are extern names already declared in the `.fsm` source;
    /// they are NOT re-emitted (the DSL declaration wins, and re-emitting
    /// would trip the analyzer's own duplicate-extern check `FSM-E0024`).
    /// Returns the rendered block plus the names that were suppressed for
    /// being DSL-shadowed (so `cmd::generate` can note the conflict once).
    pub(crate) fn to_dsl_externs(&self, skip_names: &[String]) -> (String, Vec<String>) {
        let mut block = String::new();
        let mut shadowed = Vec::new();
        for ext in &self.externs {
            if skip_names.iter().any(|n| n == &ext.name) {
                shadowed.push(ext.name.clone());
                continue;
            }
            block.push_str(&render_extern_decl(ext));
            block.push('\n');
        }
        (block, shadowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_ir::Type;

    use super::dsl_emit::dsl_safe_param_name;
    use super::types::map_c_type;

    fn names(h: &ImportedHeader) -> Vec<&str> {
        h.externs.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn dsl_keyword_param_names_are_made_safe_for_synthesis() {
        // C param names `on` / `state` collide with DSL keywords; `2bad`
        // isn't a valid ident; `rpm` is fine. The synthesized DSL line must
        // be parseable, so colliding/invalid names are rewritten while a
        // clean name is kept verbatim.
        assert_eq!(dsl_safe_param_name("on", 0), "on_");
        assert_eq!(dsl_safe_param_name("state", 1), "state_");
        assert_eq!(dsl_safe_param_name("u8", 2), "u8_");
        assert_eq!(dsl_safe_param_name("", 3), "arg3");
        assert_eq!(dsl_safe_param_name("2bad", 4), "arg4");
        assert_eq!(dsl_safe_param_name("rpm", 5), "rpm");
    }

    #[test]
    fn render_extern_decl_round_trips_through_the_dsl_parser() {
        // The synthesized text MUST re-parse as a valid DSL extern (this is
        // the contract the whole feature rests on). Build one with a
        // keyword-colliding param + a void return + a scalar return and
        // feed it back through `fsm_parser::parse`.
        let h = parse_header(
            "void relay_set(bool on);\nuint8_t crc(uint8_t seed);\n",
            "t.h",
        );
        let (block, _) = h.to_dsl_externs(&[]);
        let src = format!("language fsm 2.0\n{block}\nmachine M {{ initial S state S {{}} }}");
        let pr = fsm_parser::parse(&src);
        assert!(
            pr.errors.is_empty(),
            "synthesized extern block did not re-parse cleanly:\n{block}\nerrors: {:#?}",
            pr.errors
        );
    }

    #[test]
    fn plain_void_function_imports_with_no_params() {
        let h = parse_header("void hal_reset(void);", "t.h");
        assert_eq!(names(&h), ["hal_reset"]);
        assert!(h.externs[0].params.is_empty());
        assert!(h.externs[0].return_type.is_none());
        assert!(!h.externs[0].pure);
    }

    #[test]
    fn extern_keyword_and_stdint_types_map_to_primitives() {
        let h = parse_header("extern uint16_t adc_read(uint8_t channel);", "t.h");
        assert_eq!(names(&h), ["adc_read"]);
        assert_eq!(
            h.externs[0].return_type,
            Some(Type::Primitive { name: "u16".into() })
        );
        assert_eq!(h.externs[0].params.len(), 1);
        assert_eq!(
            h.externs[0].params[0].ty,
            Type::Primitive { name: "u8".into() }
        );
        assert_eq!(h.externs[0].params[0].name, "channel");
    }

    #[test]
    fn pointer_param_function_is_skipped_with_opaque_note() {
        // The DSL extern lowering can't model opaque param types, so a
        // function with a pointer param is skipped (not misparsed, not
        // emitted with the pointer silently dropped).
        let h = parse_header("int uart_write(const uint8_t *buf, uint32_t len);", "t.h");
        assert!(h.externs.is_empty(), "must not emit a broken extern");
        assert_eq!(h.skipped.len(), 1);
        assert!(
            h.skipped[0].reason.contains("pointer/struct/opaque"),
            "reason was: {}",
            h.skipped[0].reason
        );
        assert!(h.skipped[0].reason.contains("buf"));
    }

    #[test]
    fn struct_pointer_param_function_is_skipped() {
        let h = parse_header("bool dev_open(struct device *d);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(h.skipped[0].reason.contains("struct device *"));
    }

    #[test]
    fn opaque_return_function_is_skipped() {
        let h = parse_header("struct ctx *ctx_new(uint8_t id);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(
            h.skipped[0].reason.contains("return type"),
            "reason was: {}",
            h.skipped[0].reason
        );
    }

    #[test]
    fn unnamed_params_get_synthesised_names() {
        let h = parse_header("int add(int, int);", "t.h");
        assert_eq!(h.externs[0].params[0].name, "arg0");
        assert_eq!(h.externs[0].params[1].name, "arg1");
    }

    #[test]
    fn variadic_keeps_fixed_prefix_drops_ellipsis() {
        // Fixed prefix is scalar so the function imports; the ellipsis is
        // dropped (a DSL action passes only the fixed args).
        let h = parse_header("int dbg_logf(uint8_t level, ...);", "t.h");
        assert_eq!(names(&h), ["dbg_logf"]);
        assert_eq!(h.externs[0].params.len(), 1);
        assert_eq!(h.externs[0].params[0].name, "level");
        assert_eq!(
            h.externs[0].params[0].ty,
            Type::Primitive { name: "u8".into() }
        );
    }

    #[test]
    fn variadic_with_pointer_prefix_is_skipped() {
        // `const char *fmt` is opaque ⇒ the whole function is skipped.
        let h = parse_header("int dbg_printf(const char *fmt, ...);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
    }

    #[test]
    fn typedef_is_skipped_not_misparsed() {
        let h = parse_header("typedef unsigned int handle_t;", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(h.skipped[0].reason.contains("typedef"));
    }

    #[test]
    fn struct_definition_body_is_skipped() {
        let h = parse_header("struct point { int x; int y; };", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
    }

    #[test]
    fn function_like_macro_is_noted_as_skipped() {
        let h = parse_header("#define SQUARE(x) ((x)*(x))\nvoid go(void);", "t.h");
        assert_eq!(names(&h), ["go"]);
        assert!(h.skipped.iter().any(|s| s.reason.contains("macro")));
    }

    #[test]
    fn attribute_is_stripped_function_still_imported() {
        let h = parse_header(
            "void __attribute__((noreturn)) panic(uint32_t code);",
            "t.h",
        );
        assert_eq!(names(&h), ["panic"]);
        assert_eq!(
            h.externs[0].params[0].ty,
            Type::Primitive { name: "u32".into() }
        );
    }

    #[test]
    fn multi_line_declaration_is_joined() {
        let src = "int\nfoo(\n  uint8_t a,\n  uint8_t b\n);";
        let h = parse_header(src, "t.h");
        assert_eq!(names(&h), ["foo"]);
        assert_eq!(h.externs[0].params.len(), 2);
    }

    #[test]
    fn comments_do_not_terminate_declarations() {
        let src = "/* a ; semicolon in a comment */ \
                   void clk_init(void); // trailing\n";
        let h = parse_header(src, "t.h");
        assert_eq!(names(&h), ["clk_init"]);
    }

    #[test]
    fn function_pointer_typedef_and_param_are_skipped() {
        let h = parse_header(
            "typedef void (*cb_t)(int);\nvoid reg(void (*cb)(int));",
            "t.h",
        );
        // typedef skipped; the fn-pointer *param* makes `reg` unmodellable.
        assert!(h.externs.is_empty());
        assert!(h.skipped.len() >= 2);
    }

    #[test]
    fn gnarly_header_does_not_panic_and_skips_cleanly() {
        // A deliberately hostile mix: nested macros, conditional blocks,
        // attributes, a typedef'd fn pointer, an enum body, a struct with a
        // bitfield, string/char literals containing `;` and `)`, and one
        // genuinely importable function buried in the noise.
        let src = r#"
#ifndef DRIVER_H
#define DRIVER_H
#include <stdint.h>
#include <stdbool.h>

#define DRIVER_VERSION "1.2;3)"   /* literal with ; and ) */
#define MAX(a,b) ((a)>(b)?(a):(b))
#define REG(x) (*(volatile uint32_t*)(x))

typedef enum { MODE_A = 0, MODE_B } mode_t;
typedef struct {
    uint32_t a : 4;
    uint32_t b : 28;
    char     tag[8];
} packed_t;
typedef int (*isr_t)(void *ctx, uint32_t flags);

struct opaque_handle;

extern volatile uint32_t g_ticks;

/* Opaque param ⇒ skipped (DSL can't model it). */
__attribute__((weak)) bool sensor_ready(const struct sensor *s, uint16_t timeout_ms);

/* The one scalar-only good one — survives the noise. */
uint16_t sensor_read(uint8_t channel);

int (*get_isr(void))(int);            /* fn-ptr return — skip */
void apply(int arr[static 4]);        /* array param — skip */

#if defined(__cplusplus)
extern "C" { }
#endif
#endif
"#;
        let h = parse_header(src, "driver.h");
        // Exactly one importable symbol survives the noise; the opaque-param
        // `sensor_ready` is correctly skipped, not misparsed.
        assert_eq!(names(&h), ["sensor_read"]);
        let e = &h.externs[0];
        assert_eq!(e.return_type, Some(Type::Primitive { name: "u16".into() }));
        assert_eq!(e.params[0].ty, Type::Primitive { name: "u8".into() });
        // And it skipped a healthy pile without panicking — including the
        // opaque-param `sensor_ready`.
        assert!(h.skipped.len() >= 3, "skipped={:#?}", h.skipped);
        assert!(
            h.skipped
                .iter()
                .any(|s| s.fragment.contains("sensor_ready")),
            "sensor_ready should be skip-noted: {:#?}",
            h.skipped
        );
    }

    #[test]
    fn long_long_and_signedness_map_correctly() {
        let h = parse_header(
            "unsigned long long big(long long a, unsigned char b, signed char c);",
            "t.h",
        );
        let e = &h.externs[0];
        assert_eq!(e.return_type, Some(Type::Primitive { name: "u64".into() }));
        assert_eq!(e.params[0].ty, Type::Primitive { name: "i64".into() });
        assert_eq!(e.params[1].ty, Type::Primitive { name: "u8".into() });
        assert_eq!(e.params[2].ty, Type::Primitive { name: "i8".into() });
    }

    #[test]
    fn size_t_and_pointer_function_is_skipped() {
        // `size_t` is passed through opaque (its width is platform-
        // dependent — forcing an IR primitive would be a silent truncation
        // lie); combined with the `void *` return, the whole function is
        // skipped rather than emitted with a wrong/lossy signature.
        let h = parse_header("void *alloc(size_t n);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(
            h.skipped[0].reason.contains("pointer/struct/opaque"),
            "reason was: {}",
            h.skipped[0].reason
        );
    }

    #[test]
    fn empty_header_yields_nothing() {
        let h = parse_header("\n\n  \n", "t.h");
        assert!(h.externs.is_empty());
        assert!(h.skipped.is_empty());
    }

    #[test]
    fn project_typedef_return_function_is_skipped() {
        // `my_handle_t` is an unknown typedef → mapped opaque internally;
        // since the DSL extern lowering can't carry opaque returns, the
        // function is skipped with a precise note (not emitted lossily).
        let h = parse_header("my_handle_t open_port(uint8_t id);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(h.skipped[0].reason.contains("my_handle_t"));
    }

    #[test]
    fn map_c_type_still_classifies_opaque_internally() {
        // The C→IR mapper itself continues to recognise pointers/structs as
        // Opaque (used for the precise skip-note message); only the
        // *function-level* import is gated.
        assert_eq!(
            map_c_type("const uint8_t *"),
            Some(Type::Opaque {
                c_type: "const uint8_t *".into()
            })
        );
        assert_eq!(
            map_c_type("struct device *"),
            Some(Type::Opaque {
                c_type: "struct device *".into()
            })
        );
        assert_eq!(
            map_c_type("uint16_t"),
            Some(Type::Primitive { name: "u16".into() })
        );
    }

    #[test]
    fn inline_definition_keeps_prototype_drops_body() {
        let h = parse_header(
            "static inline uint8_t clamp(uint8_t v) { return v > 7 ? 7 : v; }",
            "t.h",
        );
        assert_eq!(names(&h), ["clamp"]);
        assert_eq!(
            h.externs[0].return_type,
            Some(Type::Primitive { name: "u8".into() })
        );
    }
}
