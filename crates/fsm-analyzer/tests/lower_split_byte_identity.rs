//! AD-3 byte-identity guard (2026-05-15).
//!
//! The `LoweringCtx` god-object → `IdMinter` + `LocCtx` + free `lower_*`
//! split (Audit C P0-2 / P1, AUDIT_B P1-A6) MUST be behaviour-identical.
//! This test lowers each of the four shipped examples through the real
//! `analyze_with_source` pipeline, serialises the IR with the canonical
//! `fsm_ir::to_json`, and asserts a stable SHA-256 fingerprint.
//!
//! The fingerprints below were captured from `main` *before* the split
//! (the pre-refactor `lower.rs`). If a future change to the lowering
//! structure (or the W2b submachine epic that will extend it) perturbs the
//! emitted IR for these examples, this test fails loudly — that is exactly
//! the regression class this guard exists to catch. Updating a constant
//! here must be a deliberate, reviewed act accompanied by a real semantic
//! reason, never a reflexive "make it green".

use std::fs;

use fsm_analyzer::analyze_with_source;
use fsm_parser::parse;

/// Tiny dep-free SHA-256 (mirrors the one the analyzer uses for
/// `sourceHash`) so this test needs no new crate.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded: Vec<u8> = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, c) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = String::with_capacity(64);
    for word in h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}

fn ir_fingerprint(rel_fsm: &str) -> String {
    // tests/ run with CWD = crate root (crates/fsm-analyzer); the examples
    // live at the workspace root, two levels up.
    let path = format!("../../{rel_fsm}");
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let pr = parse(&src);
    let res = analyze_with_source(&pr, rel_fsm, &src);
    let ir = res.ir.expect("analyzer produced IR");
    let json = fsm_ir::to_json(&ir).expect("serialize IR");
    sha256_hex(json.as_bytes())
}

// Captured from `main` (pre-AD-3, the monolithic `lower.rs`). These pin the
// canonical `to_json` serialization of each example's lowered IR.
//
// MOTOR_IR_SHA updated 2026-05-15 for v1.1-W4 (likely/rare branch hints).
// This is a DELIBERATE, REVIEWED semantic change, not a refactor (the
// docstring's bar): `examples/motor/motor.fsm` gained `likely on START
// [can_start]` + `rare on FAULT` to exercise the new construct. The IR
// delta was proven to be *exactly and only* the new optional `hint` field
// on those two transitions — verified by diffing the canonical generated
// C of motor with vs. without the prefixes: the *sole* `Motor.c`
// difference is `if (!can_start())` → `if (!MOTOR_LIKELY(can_start()))`,
// and `Motor.h` is byte-identical. No traversal/id/loc/order perturbation
// (the AD-3 regression class this guard exists for). Pre-W4 sha was
// 0e9a7b1cfa074e1d45e4b3b09c50d32fe4518f00c3cb9ccec0a8be5cd492560e.
//
// ALL FOUR updated 2026-05-15 for v1.1-W7-FU-2 (default transition priority
// 0 → 100). DELIBERATE, REVIEWED semantic change, not a refactor: a
// transition with no `priority` clause now lowers to the Doc 04 §8.6 / Doc
// 09 §6 normative default of 100 (was the buggy `.unwrap_or(0)`; Doc 00
// §11.27). None of the four example `.fsm` files declares an explicit
// `priority` clause, so every transition's IR `priority` flips 0 → 100;
// the serialized IR (hence `sourceHash` + the fingerprint) changes for all
// four. The delta was PROVEN to be *exactly and only* that, by building a
// pre-fix binary and diffing the canonical `--emit-ir` JSON per example:
// the ONLY differing lines are transition `"priority": 0` → `"priority":
// 100` (motor 4, traffic-light 2, vending-machine 9, deferred 4) plus the
// derived `sourceHash`; `RegionObject.priority` (a distinct field — region
// dispatch order for parallel states, Doc 09 §5) correctly stays 0; ZERO
// traversal/id/loc/order/structural perturbation (the exact AD-3
// regression class this guard exists for — verified clean). Runtime
// behaviour of the examples is unchanged (none has same-source same-event
// priority competition — FSM-E0300 would reject that without explicit
// priorities — so the absolute default value changes no selection). Pre-
// W7-FU-2 shas: MOTOR fa1b7aefc7f2d5242288c446a054573808c0e5e0d8800becbf6\
// bb23a63b7322a, TRAFFIC 1dc899b4ab61150c2ce06198fb790ebf689740f75c013506\
// 6cde4d450a6e9399, VENDING 8d33d4c126d9e61114a4890e81e8e7ef810bdf40c3a8e\
// 5f1f98ea0dce57d0df1, DEFERRED 46625337de6800016d44c2543fdcf4701dd52400c\
// 8edcd9fa62d6a373d8bc1ce.
const MOTOR_IR_SHA: &str = "d8f6dbc28a7cac786379a5437c65fd4db8a143fcae1cf6535d6203b56303fd87";

#[test]
fn motor_example_ir_unchanged() {
    // The constant is filled by the first run; see assertion message.
    let got = ir_fingerprint("examples/motor/motor.fsm");
    assert_eq!(
        got, MOTOR_IR_SHA,
        "motor.fsm lowered IR fingerprint changed — AD-3 must be \
         behaviour-identical. If this is an intentional semantic change \
         (NOT a refactor), update the constant deliberately."
    );
}

#[test]
fn traffic_light_example_ir_unchanged() {
    let got = ir_fingerprint("examples/traffic-light/traffic-light.fsm");
    assert_eq!(got, TRAFFIC_IR_SHA, "traffic-light.fsm lowered IR changed");
}

#[test]
fn vending_machine_example_ir_unchanged() {
    let got = ir_fingerprint("examples/vending-machine/vending-machine.fsm");
    assert_eq!(
        got, VENDING_IR_SHA,
        "vending-machine.fsm lowered IR changed"
    );
}

#[test]
fn deferred_example_ir_unchanged() {
    let got = ir_fingerprint("examples/deferred/deferred.fsm");
    assert_eq!(got, DEFERRED_IR_SHA, "deferred.fsm lowered IR changed");
}

const TRAFFIC_IR_SHA: &str = "6919b62676a9037209d358d057ca70a9838f02c487966a83a3ca004187fa4bd4";
const VENDING_IR_SHA: &str = "b456c765efde67c442368dcb4ba4a657658638f0859a76bf84200147c98a9b7c";
const DEFERRED_IR_SHA: &str = "8889e7398a209b44ee31d4a8ffd66856e5669426fdbe509417c2f67684f9ac05";
