"""PlatformIO pre-build hook: run `fsm generate` before compilation.

Wired via `extra_scripts = pre:scripts/fsm_generate.py` in platformio.ini.
PlatformIO runs this with SCons; `env` is injected by PlatformIO. The
generated C lands in `src/gen/` (PlatformIO compiles `src/` recursively)
and the include path is added so `#include "Motor.h"` resolves.

The `fsm` binary is taken from the `FSM` environment variable if set
(CI / a dev working tree pointing at a built binary), else `fsm` on PATH
(an installed SDK). This is the same contract as the Make/CMake examples.
"""

import os
import subprocess

Import("env")  # noqa: F821  (PlatformIO/SCons injects `env`)

PROJECT_DIR = env["PROJECT_DIR"]  # noqa: F821
GEN_DIR = os.path.join(PROJECT_DIR, "src", "gen")
FSM_SRC = os.path.join(PROJECT_DIR, "src", "motor.fsm")
FSM_BIN = os.environ.get("FSM", "fsm")

os.makedirs(GEN_DIR, exist_ok=True)

print("fsm: %s generate --target c99 src/motor.fsm --out src/gen/" % FSM_BIN)
subprocess.check_call(
    [FSM_BIN, "generate", "--target", "c99", FSM_SRC, "--out", GEN_DIR]
)

# Resolve #include "Motor.h" / "fsm_hal.h" from the generated dir.
env.Append(CPPPATH=[GEN_DIR])  # noqa: F821
