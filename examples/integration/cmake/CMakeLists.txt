# CMake integration for FSM-Lang.
#
#   cmake -S . -B build
#   cmake --build build
#   ./build/motor_app          # exit 0 == lifecycle verified
#
# The `fsm` binary defaults to whatever is on PATH; override with
#   cmake -S . -B build -DFSM=/abs/path/to/fsm
#
# Pattern: add_custom_command(OUTPUT ...) runs `fsm generate` and declares
# the five generated files as build outputs; the executable target depends
# on the generated Motor.c, so CMake re-generates whenever motor.fsm
# changes and never otherwise. Requires CMake >= 3.13 for the
# OUTPUT/DEPENDS form used here.

cmake_minimum_required(VERSION 3.13)
project(fsm_cmake_integration C)

set(CMAKE_C_STANDARD 99)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF) # -std=c99, not -std=gnu99

# `fsm` on PATH unless overridden with -DFSM=/path/to/fsm.
set(FSM "fsm" CACHE STRING "Path to the fsm binary")

set(GEN_DIR "${CMAKE_CURRENT_BINARY_DIR}/gen")
set(FSM_SRC "${CMAKE_CURRENT_SOURCE_DIR}/motor.fsm")

# The generated-C ABI: Motor.c is the lone translation unit; the other
# four are headers (see docs/25 Sec 1).
set(GEN_SOURCES
    "${GEN_DIR}/Motor.c"
    "${GEN_DIR}/Motor.h"
    "${GEN_DIR}/Motor_impl.h"
    "${GEN_DIR}/Motor_conf.h"
    "${GEN_DIR}/fsm_hal.h")

add_custom_command(
    OUTPUT ${GEN_SOURCES}
    COMMAND ${CMAKE_COMMAND} -E make_directory "${GEN_DIR}"
    COMMAND ${FSM} generate --target c99 "${FSM_SRC}" --out "${GEN_DIR}"
    DEPENDS "${FSM_SRC}"
    COMMENT "fsm generate --target c99 motor.fsm -> gen/"
    VERBATIM)

add_custom_target(fsm_generate DEPENDS ${GEN_SOURCES})

add_executable(motor_app
    "${GEN_DIR}/Motor.c"
    hal.c
    motor_externs.c
    main.c)
add_dependencies(motor_app fsm_generate)

target_include_directories(motor_app PRIVATE
    "${GEN_DIR}"
    "${CMAKE_CURRENT_SOURCE_DIR}")

# Same strict contract as every other FSM-Lang integration.
target_compile_options(motor_app PRIVATE
    -Wall -Wextra -Wpedantic -Werror)

# `cmake --build build --target run` runs the verified binary.
add_custom_target(run
    COMMAND motor_app
    DEPENDS motor_app
    COMMENT "Running motor_app (exit 0 == FSM lifecycle verified)")
