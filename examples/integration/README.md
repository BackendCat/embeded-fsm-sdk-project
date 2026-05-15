# Worked integration examples

Four self-contained, **genuinely-building** end-to-end integrations of the
same Motor FSM into real build ecosystems. Copy any subdirectory and you
have a working build — not documentation to debug.

| Dir | Ecosystem | One command | What it proves |
|---|---|---|---|
| [`make/`](make/) | GNU Make | `make run` | `fsm generate` → strict `gcc` → linked binary → runs, asserting the full lifecycle. |
| [`cmake/`](cmake/) | CMake ≥ 3.13 | `cmake -S . -B build && cmake --build build && ./build/motor_app` | `add_custom_command` codegen + executable target; same strict flags + runnable. |
| [`cargo-rust/`](cargo-rust/) | Rust + Cargo | `cargo run` | `build.rs` runs `fsm generate`, `cc` compiles the C, Rust drives it over `extern "C"` FFI. Standalone (own workspace). |
| [`platformio/`](platformio/) | PlatformIO | `pio run` | Pre-build hook codegen; `native` + Arduino `uno` envs off one tree. |

Every example uses the strict contract `-std=c99 -Wall -Wextra
-Wpedantic -Werror` and a runnable driver whose **exit code 0 means every
transition / context mutation / extern call was asserted correct** — the
behavioural-acceptance bar from `docs/processes/SUBAGENT_CONVENTIONS.md`
§5.4, not symbol-presence.

Acceptance tests in `crates/fsm-cli/tests/integration_examples.rs`
actually build + run make / cmake / cargo-rust (tools required, present)
and build + run the PlatformIO project via `pio` when installed, else fall
back to compiling its generated C with host `gcc -Werror` (a loud,
documented skip — never silent).

The full ABI contract, the C↔C++/Rust recipes, build-system fragments and
a troubleshooting matrix are in
[`docs/25-Integration-Guide.md`](../../docs/25-Integration-Guide.md).

`fsm` is taken from PATH; each example also accepts an explicit binary
path (`make FSM=…`, `cmake -DFSM=…`, `FSM=… cargo …`, `FSM=… pio …`).
