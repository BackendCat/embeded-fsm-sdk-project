# Make integration for FSM-Lang.
#
# One command:  make run
#
#   fsm generate --target c99 motor.fsm --out gen/
#     -> gen/Motor.c gen/Motor.h gen/Motor_impl.h gen/Motor_conf.h
#        gen/fsm_hal.h            (the generated-C ABI; see docs/25)
#   gcc -std=c99 -Wall -Wextra -Wpedantic -Werror
#     gen/Motor.c + hal.c + motor_externs.c + main.c  -> ./motor_app
#   ./motor_app                                        (exit 0 == verified)
#
# FSM defaults to the `fsm` on PATH. Override with an explicit path:
#   make FSM=/abs/path/to/fsm run

FSM      ?= fsm
CC       ?= cc
CFLAGS   ?= -std=c99 -Wall -Wextra -Wpedantic -Werror -Igen -I.
GEN_DIR   = gen
FSM_SRC   = motor.fsm
APP       = motor_app

# The five generated files form the ABI contract (docs/25 §1). Motor.c is
# the only generated translation unit; the rest are headers.
GEN_C     = $(GEN_DIR)/Motor.c
USER_C    = hal.c motor_externs.c main.c

.PHONY: all run clean

all: $(APP)

# Codegen step. The stamp file tracks the generate so we re-run only when
# the .fsm changes, not on every build.
$(GEN_DIR)/.stamp: $(FSM_SRC)
	$(FSM) generate --target c99 $(FSM_SRC) --out $(GEN_DIR)
	@touch $@

$(GEN_C): $(GEN_DIR)/.stamp

$(APP): $(GEN_C) $(USER_C) motor_externs.h
	$(CC) $(CFLAGS) $(GEN_C) $(USER_C) -o $(APP)

run: $(APP)
	./$(APP)

clean:
	rm -rf $(GEN_DIR) $(APP)
