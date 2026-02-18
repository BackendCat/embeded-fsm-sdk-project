# FSM-Lang DSL Specification v1.0

This document formally defines the syntax and core semantics of FSM-Lang.

It includes:

- Lexical structure
- Grammar (EBNF)
- Structural elements
- Execution-related constructs
- Full example covering extended features

---

# 1. Lexical Structure

## 1.1 Character Set

FSM-Lang source files are UTF-8 encoded.

## 1.2 Identifiers

```
identifier = letter , { letter | digit | "_" } ;
letter     = "A"…"Z" | "a"…"z" ;
digit      = "0"…"9" ;
```

Identifiers are case-sensitive.

---

## 1.3 Literals

### Integer literal

```
integer = digit , { digit } ;
```

### Boolean literal

```
boolean = "true" | "false" ;
```

---

## 1.4 Comments

Single-line:

```
// comment
```

Multi-line:

```
/* comment */
```

Documentation comment:

```
/// Documentation attached to next entity
```

---

# 2. High-Level Structure

## 2.1 File Structure

```
file =
    language_decl ,
    { import_decl } ,
    { feature_decl } ,
    { const_decl } ,
    { machine_decl } ;
```

---

## 2.2 Language Declaration

```
language_decl = "language" , "fsm" , version ;
version       = integer , "." , integer ;
```

Example:

```
language fsm 1.0
```

---

## 2.3 Import Declaration

```
import_decl =
    "import" , qualified_name ,
    [ "as" , identifier ] ,
    [ "{" , identifier , { "," , identifier } , "}" ] ;
```

---

## 2.4 Feature Declaration

```
feature_decl = "feature" , identifier ;
```

Examples:

```
feature hsm
feature parallel
feature timers
```

---

## 2.5 Constant Declaration

```
const_decl = "const" , identifier , "=" , expression ;
```

---

# 3. Machine Definition

```
machine_decl =
    [ "export" ] ,
    "machine" , identifier ,
    "{" ,
        machine_body ,
    "}" ;
```

---

## 3.1 Machine Body

```
machine_body =
    { context_block
    | event_block
    | state_decl
    | initial_decl
    | target_block
    } ;
```

---

## 3.2 Context Block

```
context_block =
    "context" ,
    "{" ,
        { field_decl } ,
    "}" ;

field_decl =
    type , identifier , [ "=" , expression ] ;
```

Example:

```
context {
    int attempts = 0
    bool alarmActive = false
}
```

---

## 3.3 Event Block

```
event_block =
    "events" ,
    "{" ,
        { event_decl } ,
    "}" ;

event_decl =
    identifier ,
    [ "(" , payload_decl , ")" ] ;

payload_decl =
    type , identifier ;
```

Example:

```
events {
    BUTTON_PRESS
    TIMEOUT
}
```

---

# 4. State Definitions

```
state_decl =
    [ "export" ] ,
    "state" , identifier ,
    "{" ,
        { state_body } ,
    "}" ;
```

---

## 4.1 State Body Elements

```
state_body =
      entry_block
    | exit_block
    | transition_decl
    | internal_transition
    | after_decl
    | state_decl ;
```

Nested state_decl enables HSM.

---

## 4.2 Entry and Exit

```
entry_block =
    "entry" ,
    block ;

exit_block =
    "exit" ,
    block ;

block =
    "{" ,
        { statement } ,
    "}" ;
```

---

# 5. Transitions

## 5.1 External Transition

```
transition_decl =
    "on" ,
    identifier ,
    [ guard_clause ] ,
    [ priority_clause ] ,
    "->" ,
    identifier ,
    [ block ] ;
```

---

## 5.2 Internal Transition

```
internal_transition =
    "on" ,
    identifier ,
    [ guard_clause ] ,
    [ priority_clause ] ,
    block ;
```

---

## 5.3 Guard

```
guard_clause =
    "[" , expression , "]" ;
```

---

## 5.4 Priority

```
priority_clause =
    "priority" , integer ;
```

Lower value = higher priority.

---

# 6. Timers

```
after_decl =
    "after" ,
    integer ,
    "ms" ,
    "->" ,
    identifier ;
```

Timers generate events.

---

# 7. Parallel Regions (Extended)

```
parallel_block =
    "parallel" ,
    "{" ,
        { state_decl } ,
    "}" ;
```

Enabled only if `feature parallel` declared.

---

# 8. Expressions

## 8.1 Expression Grammar (Restricted)

```
expression =
      literal
    | identifier
    | expression , binary_op , expression
    | "(" , expression , ")" ;

binary_op =
      "+"
    | "-"
    | "*"
    | "/"
    | "=="
    | "!="
    | "<"
    | ">"
    | "<="
    | ">="
    | "&&"
    | "||" ;
```

No loops, no recursion, no side effects.

---

# 9. Initial State

```
initial_decl =
    "initial" , identifier ;
```

---

# 10. Target Block

```
target_block =
    "target" , identifier ,
    "{" ,
        { config_entry } ,
    "}" ;

config_entry =
    identifier , "=" , expression ;
```

---

# 11. Formal Determinism Rule

For a given event:

1. Collect candidate transitions.
2. Filter by guard == true.
3. If multiple:
   - select lowest priority value.
   - if equal priorities → compile-time error.
4. Execute exit → transition → entry.

---

# 12. Full Example

```
language fsm 1.0

feature hsm
feature timers

const MAX_ATTEMPTS = 3

machine DoorLock {

    context {
        int attempts = 0
        bool alarmActive = false
    }

    events {
        BUTTON
        TIMEOUT
    }

    initial Locked

    state Locked {

        entry {
            lock_motor()
        }

        on BUTTON [attempts < MAX_ATTEMPTS] -> Unlocked {
            attempts = attempts + 1
            unlock_motor()
        }

        on BUTTON [attempts >= MAX_ATTEMPTS] priority 0 -> Alarm {
            alarmActive = true
        }
    }

    state Unlocked {

        after 5000 ms -> Locked

        entry {
            led_green_on()
        }

        exit {
            led_green_off()
        }
    }

    state Alarm {
        entry {
            siren_on()
        }
    }

    target BareMetalC {
        queue_capacity = 16
    }
}
```

---

End of DSL Specification v1.0.
