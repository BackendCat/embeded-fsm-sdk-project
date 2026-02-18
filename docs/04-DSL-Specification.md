# FSM-Lang DSL Specification

**Document ID:** FSM-SPEC-DSL
**Version:** 2.0.0
**Status:** Production Draft
**Language version:** fsm 2.0
**Targets:** C99, C++17

---

## Design Contract

FSM-Lang is a **semi-imperative structural DSL**.

It describes state machine topology and wires it to externally defined C functions.
Actions support a restricted imperative sublanguage (assignments, conditionals, loops)
for concise inline logic. Guards remain purely declarative for static analysis.
Complex algorithms should live in `extern` functions, not in inline action blocks.

---

# 1. Lexical Structure

## 1.1 Source Encoding

UTF-8. BOM silently accepted and ignored.

## 1.2 Identifiers

```ebnf
identifier     = letter , { letter | digit | "_" } ;
letter         = "A"…"Z" | "a"…"z" ;
digit          = "0"…"9" ;
qualified_name = identifier , { "." , identifier } ;
```

Case-sensitive. Keywords are reserved (see Section 1.6).

## 1.3 Literals

```ebnf
integer     = decimal | hex | binary ;
decimal     = digit , { digit | "_" } ;
hex         = "0x" , hex_digit , { hex_digit | "_" } ;
binary      = "0b" , bit , { bit | "_" } ;
hex_digit   = digit | "A"…"F" | "a"…"f" ;
bit         = "0" | "1" ;
float       = digit , { digit } , "." , digit , { digit } ;
boolean     = "true" | "false" ;
string      = '"' , { string_char } , '"' ;
string_char = ? UTF-8 except '"' and '\' ? | "\\" | '\"' | "\n" | "\r" | "\t" ;
```

Float literals (e.g., `3.14`, `0.5`) are supported for context fields of type `f32`
and `f64`. Timer durations continue to use integer literals only. Using a float literal
where an integer is required is diagnostic `FSM-E0005`.

## 1.4 Comments

```ebnf
line_comment  = "//"  , { ? not newline ? } ;
block_comment = "/*" , { ? any ? } , "*/" ;   (* does not nest *)
doc_comment   = "///" , { ? not newline ? } ;  (* attaches to next declaration *)
```

## 1.5 Keywords

```
after       as              bool        cancel      choice
composite   const           context     deep_history defer
done        else            enum        every       export
extern      f32             f64         false       feature
final       fork            i8          i16         i32
i64         import          initial     is          join
junction    language        machine     ms          on
opaque      parallel        priority    pure        raise
region      schedule        send        shallow_history state
submachine  target          to          true        u8
u16         u32             u64
```

The `as` keyword is reserved for explicit type casting (see §3.1).
The `submachine` keyword is reserved for future submachine syntax (see §15).

## 1.6 Operators and Delimiters

```
->   ~>   :   ,   ;   .   =   @
{    }    (   )   [   ]
==   !=   <   >   <=  >=  &&  ||  !
+    -    *   /   %   &   |   ^   ~   <<  >>
```

---

# 2. File Structure

```ebnf
file =
    language_decl ,
    { import_decl } ,
    { top_level_decl } ;

top_level_decl =
      feature_decl
    | const_decl
    | enum_decl
    | extern_decl
    | machine_decl ;

language_decl =
    "language" , "fsm" , integer , "." , integer ;
```

Example: `language fsm 2.0`

## 2.1 Imports

```ebnf
import_decl =
    "import" , string ,
    [ "as" , identifier ] ,
    [ "{" , identifier , { "," , identifier } , "}" ] ;
```

Examples:

```fsm
import "common/events.fsm" as Common
import "shared/types.fsm" { PacketType, ErrorCode }
```

## 2.2 Feature Flags

```ebnf
feature_decl = "feature" , identifier ;
```

| Flag | Enables |
|---|---|
| `hsm` | Hierarchical composite states. |
| `parallel` | Orthogonal regions. |
| `history` | History pseudo-states. |
| `timers` | `after` and `every` declarations. |
| `deferred` | `defer` event declarations. |
| `submachines` | Submachine state references (`is`). |
| `fork_join` | Fork and join pseudo-states. |

Using a construct without the corresponding feature flag is `FSM-E0600`.

## 2.3 Constants

```ebnf
const_decl =
    [ doc_comment ] ,
    "const" , identifier , "=" , const_expr ;

const_expr =
      integer
    | boolean
    | string
    | identifier
    | const_expr , ( "+" | "-" | "*" | "/" ) , const_expr
    | "(" , const_expr , ")" ;
```

Constants are resolved at compile time. Used in timer durations and config values.

## 2.4 Enumerations

```ebnf
enum_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "enum" , identifier ,
    "{" ,
        enum_variant , { "," , enum_variant } , [ "," ] ,
    "}" ;

enum_variant =
    [ doc_comment ] , identifier , [ "=" , integer ] ;
```

Generated as `typedef enum M_EnumName_t { ... } M_EnumName_t;` in C.

## 2.5 Extern Declarations

```ebnf
extern_decl =
    [ doc_comment ] ,
    [ "pure" ] ,
    "extern" , identifier ,
    "(" , [ param_list ] , ")" ,
    [ ":" , type ] ;

param_list  = param , { "," , param } ;
param       = type , identifier ;
```

- `pure extern` — usable in guard expressions; MUST be side-effect free.
- `extern` without `pure` — usable in action lists only.
- Return type omitted → `void`.

```fsm
pure extern can_unlock   (ctx) : bool
pure extern at_max_retries(ctx) : bool
     extern lock_motor   ()
     extern set_speed    (u16 rpm)
```

The compiler generates the corresponding C declaration in `M_impl.h`:

```c
bool   M_can_unlock(const M_ctx_t *ctx);
bool   M_at_max_retries(const M_ctx_t *ctx);
void   M_lock_motor(void);
void   M_set_speed(uint16_t rpm);
```

---

# 3. Type System

The type system serves two purposes: generating correct C struct layouts and
type-checking field-comparison guards. It is intentionally minimal.

```ebnf
type =
      "bool"
    | "u8" | "u16" | "u32" | "u64"
    | "i8" | "i16" | "i32" | "i64"
    | "f32" | "f64"
    | qualified_name      (* enum reference *)
    | "opaque" , string ; (* verbatim C type *)
```

`opaque "my_struct_t"` maps to the C type verbatim. No field-comparison guards allowed
on opaque fields (`FSM-E0303`).

## 3.1 Type Casting — `as` Operator

**No implicit type promotion.** All numeric conversions require an explicit `as` cast.

```ebnf
cast_expr = unary_expr , [ "as" , type_name ] ;
```

Examples:
```fsm
ctx.speed_u32 = ctx.speed_u16 as u32
ctx.ratio = ctx.count as f32
```

- Permitted casts: any integer type to any integer type, any integer to `f32`/`f64`,
  `f32` to `f64`, `f64` to `f32` (may lose precision — `FSM-W0604`).
- Casting between incompatible types (e.g., `bool as u32`, `opaque as u16`) is
  `FSM-E0012`.
- The `as` keyword has higher precedence than binary operators but lower than
  unary operators and postfix access (see §8.7.1 precedence table).

## 3.2 Integer Overflow Behavior

| Category | Behavior |
|---|---|
| **Unsigned overflow** | Modular arithmetic (wraps). `255_u8 + 1` = `0_u8`. |
| **Signed overflow (compile-time)** | Diagnostic `FSM-E0206` if the compiler can prove overflow in a constant expression. |
| **Signed overflow (runtime)** | Target-defined: C99 = undefined behavior, C++17 = implementation-defined. Generated code SHOULD include overflow checks in debug builds (`FSM_ASSERT`). |
| **Division by zero (compile-time)** | Diagnostic `FSM-E0207`. |
| **Negative value in unsigned context** | Diagnostic `FSM-E0208` if a negative literal or provably negative expression is assigned to an unsigned field. |

## 3.3 String Type Restrictions

String literals (`"..."`) are **NOT** a valid context field type. Strings appear only in:
1. `@id("...")` annotations.
2. `language` and `feature` header declarations.
3. `import "path"` declarations.
4. `opaque "type_name"` type specifications.

Using a string literal in a context field default value, guard expression, or action
expression is a parse error (`FSM-E0010`). There is no `string` type for context fields.

---

# 4. Machine Definition

```ebnf
machine_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    [ "export" ] ,
    "machine" , identifier ,
    "{" ,
        { machine_item } ,
    "}" ;

machine_item =
      context_block
    | events_block
    | queue_block
    | target_block
    | state_decl
    | region_decl
    | choice_decl
    | junction_decl
    | fork_decl
    | join_decl
    | initial_decl
    | extern_decl ;
```

## 4.1 Context Block

```ebnf
context_block =
    "context" , "{" , { field_decl } , "}" ;

field_decl =
    [ doc_comment ] ,
    identifier , ":" , type , [ "=" , const_expr ] ;
```

Example:

```fsm
context {
    retry_count  : u8  = 0
    alarm_active : bool = false
    mode         : DeviceMode
}
```

## 4.2 Events Block

```ebnf
events_block =
    "events" , "{" , { event_decl } , "}" ;

event_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    identifier ,
    [ "(" , payload_field , { "," , payload_field } , ")" ] ;

payload_field = identifier , ":" , type ;
```

Example:

```fsm
events {
    CMD_CONNECT   ( host : u32, port : u16 )
    CMD_DISCONNECT
    LINK_UP
    LINK_DOWN
    DATA_RECEIVED ( len : u8 )
    PACKET        ( kind : PacketType, len : u8 )
    HEARTBEAT_ACK
}
```

## 4.3 Queue Block

```ebnf
queue_block =
    "queue" , "{" , { config_entry } , "}" ;

config_entry =
    identifier , "=" , ( integer | boolean | identifier ) ;
```

Standard keys: `capacity`, `overflow` (`drop_oldest` | `drop_newest` | `assert` | `error`).

## 4.4 Target Block

```ebnf
target_block =
    "target" , identifier , "{" , { config_entry } , "}" ;
```

Built-in targets: `C99`, `C99-RTOS`, `Cpp17`, `Simulation`.

Standard keys: `strategy` (`switch_based` | `table_driven`), `allow_float`, `max_nesting`.

## 4.5 Initial Declaration

```ebnf
initial_decl = [ stable_id ] , "initial" , identifier ;
```

---

# 5. State Definitions

```ebnf
state_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    [ "export" ] ,
    "state" , identifier ,
    "{" ,
        { state_item } ,
    "}" ;

state_item =
      entry_decl
    | exit_decl
    | transition_decl
    | internal_decl
    | local_decl
    | completion_decl
    | after_decl
    | every_decl
    | every_internal_decl
    | defer_decl
    | state_decl          (* nested child state *)
    | region_decl
    | choice_decl
    | junction_decl
    | initial_decl
    | shallow_history_decl
    | deep_history_decl
    | final_decl
    | entry_point_decl
    | exit_point_decl ;
```

## 5.1 Entry and Exit

```ebnf
entry_decl = "entry" , ":" , action_list ;
exit_decl  = "exit"  , ":" , action_list ;
```

## 5.2 Final State

```ebnf
final_decl = [ doc_comment ] , [ stable_id ] , "final" , identifier ;
```

Reaching a final state generates a completion event for the enclosing composite state.

## 5.3 Entry and Exit Points

```ebnf
entry_point_decl = [ stable_id ] , "entry_point" , identifier , "->" , identifier ;
exit_point_decl  = [ stable_id ] , "exit_point"  , identifier ;
```

---

# 6. Regions

```ebnf
region_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "region" , identifier ,
    "{" ,
        { region_item } ,
    "}" ;

region_item =
      initial_decl
    | state_decl
    | choice_decl
    | junction_decl
    | shallow_history_decl
    | deep_history_decl
    | final_decl
    | fork_decl
    | join_decl ;
```

A state with one or more `region` blocks is an orthogonal composite state.
Each region executes independently. Requires `feature parallel`.

---

# 7. Pseudo-States

## 7.1 Shallow History

```ebnf
shallow_history_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "shallow_history" , identifier ,
    "{" , initial_decl , "}" ;
```

Restores the most recently active direct child. Falls back to `initial` if no history.
Requires `feature history`.

## 7.2 Deep History

```ebnf
deep_history_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "deep_history" , identifier ,
    "{" , initial_decl , "}" ;
```

Restores the most recently active leaf of the subtree. Requires `feature history`.

## 7.3 Choice Pseudo-State

Runtime guard evaluation. Guards evaluated top-to-bottom; first true branch is taken.
Use `[else]` as catch-all (required — absence of exhaustive branches is `FSM-E0100`).

```ebnf
choice_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "choice" , identifier ,
    "{" ,
        choice_branch , { choice_branch } ,
    "}" ;

choice_branch =
    "[" , guard_expr , "]" , "->" , identifier , [ ":" , action_list ] ;
```

## 7.4 Junction Pseudo-State

Compile-time guard evaluation. Branches MUST be provably disjoint and exhaustive.

```ebnf
junction_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "junction" , identifier ,
    "{" ,
        junction_branch , { junction_branch } ,
    "}" ;

junction_branch = choice_branch ;
```

## 7.5 Fork

```ebnf
fork_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "fork" , identifier ,
    "->" , "{" , identifier , { "," , identifier } , "}" ;
```

Each listed identifier MUST be in a different orthogonal region.
Requires `feature fork_join`.

## 7.6 Join

```ebnf
join_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "join" , identifier ,
    "{" , identifier , { "," , identifier } , "}" ,
    "->" , identifier ;
```

Completes when all listed source states have exited.
Requires `feature fork_join`.

---

# 8. Transitions

## 8.1 External Transition

```ebnf
transition_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "on" , identifier ,
    [ guard_clause ] ,
    [ priority_clause ] ,
    "->" , identifier ,
    [ ":" , action_list ] ;
```

## 8.2 Internal Transition

No state change. No exit or entry actions.

```ebnf
internal_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "on" , identifier ,
    [ guard_clause ] ,
    [ priority_clause ] ,
    ":" , action_list ;
    (* No -> *)
```

## 8.3 Local Transition

Target MUST be a proper descendant of the source state (`FSM-E0110` otherwise).

```ebnf
local_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "on" , identifier ,
    [ guard_clause ] ,
    [ priority_clause ] ,
    "~>" , identifier ,
    [ ":" , action_list ] ;
```

## 8.4 Completion Transition

```ebnf
completion_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "done" ,
    [ guard_clause ] ,
    [ priority_clause ] ,
    "->" , identifier ,
    [ ":" , action_list ] ;
```

## 8.5 Guard Clause

```ebnf
guard_clause = "[" , guard_expr , "]" ;

guard_expr =
      identifier                                (* extern pure fn *)
    | "!" , identifier                          (* negation of extern pure fn *)
    | field_ref , cmp_op , literal_or_field     (* field comparison *)
    | guard_expr , "&&" , guard_expr            (* logical AND *)
    | guard_expr , "||" , guard_expr            (* logical OR *)
    | "(" , guard_expr , ")" ;                  (* grouping *)
    | "else"                                    (* catch-all: always true *)

field_ref =
      "ctx"     , "." , identifier
    | "payload" , "." , identifier ;

literal_or_field =
      integer
    | boolean
    | qualified_name                            (* enum variant: PacketType.DATA *)
    | field_ref ;

cmp_op = "==" | "!=" | "<" | ">" | "<=" | ">=" ;
```

No arithmetic. No function calls except extern pure references.

## 8.6 Priority Clause

```ebnf
priority_clause = "priority" , integer ;
```

Lower number = higher priority. Default: `100`.

## 8.7 Action List

Action lists use a **restricted imperative sublanguage**. Statements execute in
declaration order. Side-effect freedom is NOT required (unlike guards).

```ebnf
action_list =
    statement , { ";" , statement } , [ ";" ] ;

statement =
      assign_stmt
    | if_stmt
    | while_stmt
    | for_stmt
    | call_action
    | raise_action
    | send_action
    | defer_stmt ;

assign_stmt =
    field_ref , "=" , expr ;

if_stmt =
    "if" , "(" , expr , ")" ,
    "{" , action_list , "}" ,
    [ "else" , ( "{" , action_list , "}" | if_stmt ) ] ;

while_stmt =
    "while" , "(" , expr , ")" ,
    "{" , action_list , "}" ;

for_stmt =
    "for" , "(" , assign_stmt , ";" , expr , ";" , assign_stmt , ")" ,
    "{" , action_list , "}" ;

call_action =
    identifier ,
    [ "(" , call_arg , { "," , call_arg } , ")" ] ;

call_arg = expr ;

raise_action =
    "raise" , identifier ,
    [ "(" , call_arg , { "," , call_arg } , ")" ] ;

send_action =
    "send" , identifier ,
    [ "(" , call_arg , { "," , call_arg } , ")" ] ,
    "to" , identifier ;

defer_stmt = "defer" , identifier ;

(* Action expression language *)
expr =
      unary_expr
    | expr , bin_op , expr
    | "(" , expr , ")"
    | identifier , "(" , [ expr , { "," , expr } ] , ")"
    | field_ref
    | literal ;

unary_expr = ( "!" | "-" | "~" ) , expr ;

bin_op =
    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^"
    | "<<" | ">>" | "&&" | "||"
    | "==" | "!=" | "<" | ">" | "<=" | ">=" ;

literal = integer | boolean | string | qualified_name ;
```

### 8.7.1 Operator Precedence (Normative, Pratt-Parsing-Friendly)

This table defines 10 precedence levels suitable for a Pratt parser implementation.
Precedence from highest (binds tightest) to lowest. Same level = left-associative
unless noted.

| Level | Category | Operators | Associativity |
|---|---|---|---|
| 10 (highest) | Primary | literals, identifiers, `(expr)`, `field_ref` | N/A |
| 9 | Postfix | `.` (field access), `(args)` (function call) | Left |
| 8 | Cast | `as` | Left |
| 7 | Unary | `!`, `-`, `~` | Right (prefix) |
| 6 | Multiplicative | `*`, `/`, `%` | Left |
| 5 | Additive | `+`, `-` | Left |
| 4 | Shift | `<<`, `>>` | Left |
| 3 | Bitwise | `&`, `^`, `\|` | Left |
| 2 | Comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` | Left |
| 1 | Logical AND | `&&` | Left |
| 0 (lowest) | Logical OR | `\|\|` | Left |

**Pratt parser binding powers:** Each level maps to a left binding power (LBP). The
parser calls `parse_expr(min_bp)` where `min_bp` starts at 0. Right-associative operators
(unary prefix) use `rbp = lbp` instead of `rbp = lbp + 1`.

**Examples:**
- `ctx.a + ctx.b * 2` → `ctx.a + (ctx.b * 2)` (level 6 before level 5)
- `ctx.x > 0 && ctx.y < 10` → `(ctx.x > 0) && (ctx.y < 10)` (level 2 before level 1)
- `!ctx.flag && ctx.speed > 0` → `(!ctx.flag) && (ctx.speed > 0)` (unary before comparison)
- `ctx.speed as u32 + 10` → `(ctx.speed as u32) + 10` (cast before additive)

The guard expression sublanguage uses the **same precedence table** for its operators
(restricted to comparison, logical, and unary operators).

### 8.7.2 Function Call vs. Identifier Disambiguation

An identifier followed by `(` is **always** parsed as a function call in both
expression/guard context and action context. There is no ambiguity: FSM-Lang has no
tuple syntax or grouping-after-identifier construct.

```
identifier "("  →  function call (always)
identifier      →  variable reference or extern name (no parens)
```

In guard context, only `pure extern` functions may be called. In action context, any
declared `extern` function may be called.

```ebnf
```

**Permitted in action blocks:**
- Context field assignments: `ctx.field = expr`
- Extern function calls with full expression arguments
- Conditional branching: `if` / `else`
- Loops: `while`, `for` (emit `FSM-W0200` — style warning only)
- FSM operations: `raise`, `send`, `defer`

**Not permitted in action blocks:**
- Heap allocation (`malloc`, `new`)
- Stack-allocated arrays
- Direct pointer arithmetic on context fields (use extern wrappers)
- **Payload field writes:** `payload.field = expr` is forbidden. Payload fields are
  **read-only** in all action blocks. Writing to a payload field is `FSM-E0006`.
  Payload fields may only be read (e.g., `ctx.speed = payload.target_speed`).

---

# 9. Timers

## 9.1 One-Shot Timer

```ebnf
after_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "after" , const_expr , "ms" ,
    "->" , identifier ,
    [ ":" , action_list ] ;
```

Timer starts on state entry. Cancelled on state exit.

## 9.2 Periodic Timer (with transition)

```ebnf
every_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "every" , const_expr , "ms" ,
    "->" , identifier ,
    [ ":" , action_list ] ;
```

## 9.3 Periodic Timer (internal, no transition)

```ebnf
every_internal_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "every" , const_expr , "ms" ,
    ":" , action_list ;
```

## 9.4 Deferred Events

```ebnf
defer_decl = "defer" , identifier ;
```

Holds this event type while in this state. Released on exit to a non-deferring state.
Requires `feature deferred`.

---

# 10. Stable ID Annotation

```ebnf
stable_id = "@id" , "(" , string , ")" ;
```

Unique within the machine. Preserved across renames. Used for history storage,
trace records, and toolchain references. Duplicate IDs: `FSM-E0750`.

---

# 11. Formal Transition Selection Algorithm

This section is normative. Implementations MUST produce identical results.

```
SELECT(active_config, event):

  candidates = []
  for S in active_config, innermost-to-outermost:
    for T in transitions_of(S) where T.trigger == event.name:
      if T.guard is absent OR eval_guard(T.guard, ctx, event.payload) == true:
        candidates.append(T)

  (* Hierarchical filter: if inner state T2 and outer state T1 both have
     a candidate, T1's candidate is removed. *)
  filtered = [T for T in candidates
              where no other T2 in candidates
              has is_proper_descendant(T2.source, T.source)]

  if filtered is empty: return NONE

  (* Priority resolution *)
  min_p = min(T.priority for T in filtered)
  by_p  = [T for T in filtered if T.priority == min_p]

  if len(by_p) > 1:
    (* Compiler MUST have rejected this model with FSM-E0100/E0101 *)
    UNREACHABLE

  return by_p[0]


EXECUTE(T, ctx, event):
  S = T.source
  Tg = T.target
  L = lca(S, Tg)

  (* Exit: innermost first, up to (not including) L *)
  for X in path(S, L, exclusive=true), innermost_first:
    call_exit_actions(X)

  (* Transition actions *)
  execute_action_list(T.actions, ctx, event.payload)

  (* Entry: outermost first, down to target *)
  for X in path(Tg, L, exclusive=true), outermost_first:
    call_entry_actions(X)
  call_entry_actions(Tg)

  (* Enter initial substates of composite target *)
  enter_initial_substates(Tg)


DISPATCH(machine, event):
  for each active region R (ordered by region priority, then lexicographic name):
    T = SELECT(active_states_in(R), event)
    if T is not NONE:
      EXECUTE(T, machine.ctx, event)

  (* Completion pass *)
  for each region R:
    if active_leaf_of(R) is a final_state:
      enqueue_completion_event(R)
      process_completion_event(R)  (* immediately, before external queue *)
```

---

# 12. Complete Industrial Example

A **device connection manager** demonstrating all major language features.

```fsm
language fsm 2.0

feature hsm
feature parallel
feature history
feature timers
feature deferred

/// ── Constants ────────────────────────────────────────────────────────────────

const MAX_RETRIES           = 3
const HEARTBEAT_INTERVAL_MS = 5000
const CONNECT_TIMEOUT_MS    = 10000
const SEND_TIMEOUT_MS       = 2000

/// ── Types ────────────────────────────────────────────────────────────────────

enum PacketType {
    CONTROL   = 0,
    DATA      = 1,
    HEARTBEAT = 2
}

/// ── Extern declarations ──────────────────────────────────────────────────────

/// Guard: retry_count < MAX_RETRIES
pure extern can_retry        (ctx) : bool
/// Guard: retry_count >= MAX_RETRIES
pure extern retry_exhausted  (ctx) : bool

     extern reset_link       ()
     extern initiate_connect ()
     extern close_connect    ()
     extern begin_disconnect ()
     extern send_heartbeat   ()
     extern start_send       (u8 len)
     extern abort_send       ()
     extern handle_data      (u8 len)
     extern handle_control   (u8 len)
     extern on_link_lost     ()
     extern on_max_retries   ()
     extern on_hb_received   ()

/// ── Machine ──────────────────────────────────────────────────────────────────

@id("m-device-mgr")
export machine DeviceManager {

    context {
        retry_count  : u8   = 0
        alarm_active : bool = false
    }

    events {
        @id("ev-cmd-connect")
        CMD_CONNECT   ( host : u32, port : u16 )
        CMD_DISCONNECT
        LINK_UP
        LINK_DOWN
        @id("ev-data-rcv")
        DATA_RECEIVED ( len : u8 )
        PACKET        ( kind : PacketType, len : u8 )
        CMD_SEND      ( len : u8 )
        HEARTBEAT_ACK
    }

    queue {
        capacity = 32
        overflow = drop_oldest
    }

    @id("init-top")
    initial Disconnected

    // ── DISCONNECTED ──────────────────────────────────────────────────────────

    @id("s-disconnected")
    state Disconnected {
        entry : reset_link
        on CMD_CONNECT -> Connecting
    }

    // ── CONNECTING ────────────────────────────────────────────────────────────

    @id("s-connecting")
    state Connecting {
        entry : initiate_connect

        defer DATA_RECEIVED

        after CONNECT_TIMEOUT_MS ms -> Disconnected : on_link_lost

        on LINK_UP                        -> Connected
        on LINK_DOWN [can_retry]          -> Connecting : initiate_connect
        on LINK_DOWN [retry_exhausted]    -> Disconnected : on_max_retries  priority 0
        on CMD_DISCONNECT                 -> Disconnected
    }

    // ── CONNECTED (orthogonal) ────────────────────────────────────────────────

    @id("s-connected")
    state Connected {
        exit : close_connect

        on LINK_DOWN      -> Disconnected
        on CMD_DISCONNECT -> Disconnecting

        // ── Region 1: data path ───────────────────────────────────────────────

        @id("rgn-data")
        region DataPath {

            @id("init-data")
            initial IdleData

            @id("h-data")
            shallow_history HData { initial IdleData }

            @id("s-idle-data")
            state IdleData {
                on DATA_RECEIVED -> Processing
                on CMD_SEND      -> Sending
            }

            @id("s-processing")
            state Processing {
                on PACKET [payload.kind == PacketType.DATA]
                    -> IdleData : handle_data(payload.len)

                on PACKET [payload.kind == PacketType.CONTROL]
                    -> IdleData : handle_control(payload.len)

                on PACKET [payload.kind == PacketType.HEARTBEAT]
                    -> IdleData : on_hb_received
            }

            @id("s-sending")
            state Sending {
                entry : start_send(payload.len)

                done               -> IdleData
                after SEND_TIMEOUT_MS ms -> IdleData : abort_send
            }
        }

        // ── Region 2: heartbeat monitor ───────────────────────────────────────

        @id("rgn-hb")
        region HeartbeatMonitor {

            @id("init-hb")
            initial HBActive

            @id("s-hb-active")
            state HBActive {
                every HEARTBEAT_INTERVAL_MS ms : send_heartbeat
                on HEARTBEAT_ACK               : on_hb_received
            }
        }
    }

    // ── DISCONNECTING ─────────────────────────────────────────────────────────

    @id("s-disconnecting")
    state Disconnecting {
        entry : begin_disconnect
        after 1000 ms -> Disconnected
        on LINK_DOWN  -> Disconnected
    }

    // ── Target profiles ───────────────────────────────────────────────────────

    target C99 {
        strategy    = switch_based
        allow_float = false
    }

    target Cpp17 {
        strategy    = switch_based
        allow_float = false
    }
}
```

---

# 13. Grammar Summary (Normative)

```ebnf
file             = language_decl , { import_decl } , { top_level_decl } ;
language_decl    = "language" , "fsm" , integer , "." , integer ;
import_decl      = "import" , string , [ "as" , identifier ] ,
                   [ "{" , identifier , { "," , identifier } , "}" ] ;
top_level_decl   = feature_decl | const_decl | enum_decl
                 | extern_decl | machine_decl ;
feature_decl     = "feature" , identifier ;
const_decl       = [ doc_comment ] , "const" , identifier , "=" , const_expr ;
const_expr       = integer | boolean | string | identifier
                 | const_expr , ( "+" | "-" | "*" | "/" ) , const_expr
                 | "(" , const_expr , ")" ;

enum_decl        = [ doc_comment ] , [ stable_id ] , "enum" , identifier ,
                   "{" , enum_variant , { "," , enum_variant } , [ "," ] , "}" ;
enum_variant     = [ doc_comment ] , identifier , [ "=" , integer ] ;

extern_decl      = [ doc_comment ] , [ "pure" ] , "extern" , identifier ,
                   "(" , [ param_list ] , ")" , [ ":" , type ] ;
param_list       = param , { "," , param } ;
param            = type , identifier ;

type             = "bool" | "u8" | "u16" | "u32" | "u64"
                 | "i8" | "i16" | "i32" | "i64" | "f32" | "f64"
                 | qualified_name | "opaque" , string ;

machine_decl     = [ doc_comment ] , [ stable_id ] , [ "export" ] ,
                   "machine" , identifier , "{" , { machine_item } , "}" ;
machine_item     = context_block | events_block | queue_block | target_block
                 | state_decl | region_decl | choice_decl | junction_decl
                 | fork_decl | join_decl | initial_decl | extern_decl ;

context_block    = "context" , "{" , { field_decl } , "}" ;
field_decl       = [ doc_comment ] , identifier , ":" , type , [ "=" , const_expr ] ;

events_block     = "events" , "{" , { event_decl } , "}" ;
event_decl       = [ doc_comment ] , [ stable_id ] , identifier ,
                   [ "(" , payload_field , { "," , payload_field } , ")" ] ;
payload_field    = identifier , ":" , type ;

queue_block      = "queue" , "{" , { config_entry } , "}" ;
target_block     = "target" , identifier , "{" , { config_entry } , "}" ;
config_entry     = identifier , "=" , ( integer | boolean | identifier ) ;

initial_decl     = [ stable_id ] , "initial" , identifier ;

state_decl       = [ doc_comment ] , [ stable_id ] , [ "export" ] ,
                   "state" , identifier , "{" , { state_item } , "}" ;
state_item       = entry_decl | exit_decl | transition_decl | internal_decl
                 | local_decl | completion_decl | after_decl | every_decl
                 | every_internal_decl | defer_decl | state_decl | region_decl
                 | choice_decl | junction_decl | initial_decl
                 | shallow_history_decl | deep_history_decl | final_decl
                 | entry_point_decl | exit_point_decl ;

entry_decl       = "entry" , ":" , action_list ;
exit_decl        = "exit"  , ":" , action_list ;
final_decl       = [ doc_comment ] , [ stable_id ] , "final" , identifier ;
entry_point_decl = [ stable_id ] , "entry_point" , identifier , "->" , identifier ;
exit_point_decl  = [ stable_id ] , "exit_point"  , identifier ;

region_decl      = [ doc_comment ] , [ stable_id ] ,
                   "region" , identifier , "{" , { region_item } , "}" ;
region_item      = initial_decl | state_decl | choice_decl | junction_decl
                 | shallow_history_decl | deep_history_decl | final_decl
                 | fork_decl | join_decl ;

shallow_history_decl = [ doc_comment ] , [ stable_id ] ,
                       "shallow_history" , identifier , "{" , initial_decl , "}" ;
deep_history_decl    = [ doc_comment ] , [ stable_id ] ,
                       "deep_history" , identifier , "{" , initial_decl , "}" ;
choice_decl          = [ doc_comment ] , [ stable_id ] ,
                       "choice" , identifier , "{" , choice_branch , { choice_branch } , "}" ;
choice_branch        = "[" , guard_expr , "]" , "->" , identifier , [ ":" , action_list ] ;
junction_decl        = [ doc_comment ] , [ stable_id ] ,
                       "junction" , identifier , "{" , junction_branch , { junction_branch } , "}" ;
junction_branch      = choice_branch ;
fork_decl            = [ doc_comment ] , [ stable_id ] ,
                       "fork" , identifier , "->" , "{" , identifier , { "," , identifier } , "}" ;
join_decl            = [ doc_comment ] , [ stable_id ] ,
                       "join" , identifier , "{" , identifier , { "," , identifier } , "}" ,
                       "->" , identifier ;

transition_decl  = [ doc_comment ] , [ stable_id ] , "on" , identifier ,
                   [ guard_clause ] , [ priority_clause ] ,
                   "->" , identifier , [ ":" , action_list ] ;
internal_decl    = [ doc_comment ] , [ stable_id ] , "on" , identifier ,
                   [ guard_clause ] , [ priority_clause ] ,
                   ":" , action_list ;
local_decl       = [ doc_comment ] , [ stable_id ] , "on" , identifier ,
                   [ guard_clause ] , [ priority_clause ] ,
                   "~>" , identifier , [ ":" , action_list ] ;
completion_decl  = [ doc_comment ] , [ stable_id ] , "done" ,
                   [ guard_clause ] , [ priority_clause ] ,
                   "->" , identifier , [ ":" , action_list ] ;

guard_clause     = "[" , guard_expr , "]" ;
guard_expr       = identifier
                 | "!" , identifier
                 | field_ref , cmp_op , literal_or_field
                 | guard_expr , "&&" , guard_expr
                 | guard_expr , "||" , guard_expr
                 | "(" , guard_expr , ")"
                 | "else" ;
field_ref        = ( "ctx" | "payload" ) , "." , identifier ;
literal_or_field = integer | boolean | qualified_name | field_ref ;
cmp_op           = "==" | "!=" | "<" | ">" | "<=" | ">=" ;

priority_clause  = "priority" , integer ;

action_list      = statement , { ";" , statement } , [ ";" ] ;
statement        = assign_stmt
                 | if_stmt
                 | while_stmt
                 | for_stmt
                 | call_action
                 | raise_action
                 | send_action
                 | defer_stmt ;
assign_stmt      = field_ref , "=" , expr ;
if_stmt          = "if" , "(" , expr , ")" , "{" , action_list , "}"
                   [ "else" , ( "{" , action_list , "}" | if_stmt ) ] ;
while_stmt       = "while" , "(" , expr , ")" , "{" , action_list , "}" ;
for_stmt         = "for" , "(" , assign_stmt , ";" , expr , ";" , assign_stmt , ")"
                   "{" , action_list , "}" ;
call_action      = identifier , [ "(" , call_arg , { "," , call_arg } , ")" ] ;
call_arg         = expr ;
raise_action     = "raise" , identifier , [ "(" , call_arg , { "," , call_arg } , ")" ] ;
send_action      = "send" , identifier , [ "(" , call_arg , { "," , call_arg } , ")" ] ,
                   "to" , identifier ;
defer_stmt       = "defer" , identifier ;

(* Action expression language — richer than guard_expr *)
expr             = unary_expr
                 | expr , bin_op , expr
                 | cast_expr
                 | "(" , expr , ")"
                 | identifier , "(" , [ expr , { "," , expr } ] , ")"
                 | field_ref
                 | literal ;
cast_expr        = unary_expr , "as" , type ;
unary_expr       = ( "!" | "-" | "~" ) , expr ;
bin_op           = "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^"
                 | "<<" | ">>" | "&&" | "||"
                 | "==" | "!=" | "<" | ">" | "<=" | ">=" ;
literal          = integer | float | boolean | string | qualified_name ;

(* Submachine — [future] post-v1.0, requires feature submachines *)
submachine_decl  = [ doc_comment ] , [ stable_id ] ,
                   "submachine" , identifier , "{" , { machine_item } , "}" ;

after_decl           = [ doc_comment ] , [ stable_id ] ,
                       "after" , const_expr , "ms" ,
                       "->" , identifier , [ ":" , action_list ] ;
every_decl           = [ doc_comment ] , [ stable_id ] ,
                       "every" , const_expr , "ms" ,
                       "->" , identifier , [ ":" , action_list ] ;
every_internal_decl  = [ doc_comment ] , [ stable_id ] ,
                       "every" , const_expr , "ms" ,
                       ":" , action_list ;
defer_decl           = "defer" , identifier ;

stable_id        = "@id" , "(" , string , ")" ;
doc_comment      = "///" , { ? not newline ? } ;
```

---

# 14. Annotations — Ordering and Semantics

## 14.1 Annotation Ordering

When both `@id(...)` and `/// doc comment` precede a declaration, the **canonical
order** is:

1. `@id("stable-id")` — first
2. `/// doc comment` — second (one or more lines)
3. Declaration (`state`, `machine`, `on`, etc.)

```fsm
@id("motor-idle")
/// The idle state. Entered on startup and after STOP.
state Idle {
    @id("t-idle-running")
    /// Transition to Running when START event received.
    on START -> Running
}
```

The formatter enforces this order (see FSM-SPEC-FMT §15). The parser accepts either
order but emits `FSM-H0006` (hint) if the order is reversed.

## 14.2 Stable ID Uniqueness

Stable IDs MUST be unique within a machine. Duplicate IDs are `FSM-E0025`.
Stable IDs are preserved across renames and used for history storage, trace records,
and toolchain references.

---

# 15. Submachine Syntax [Future — Post-v1.0]

The `submachine` construct is **feature-flagged** (`feature submachines`) and is
reserved for post-v1.0. The grammar production is included here for forward reference.

```ebnf
submachine_decl =
    [ doc_comment ] ,
    [ stable_id ] ,
    "submachine" , identifier ,
    "{" , machine_body , "}" ;

machine_body = { machine_item } ;
```

A submachine creates an independent execution context that can be referenced from
a parent machine via the `is` keyword:

```fsm
// [future] — not available in v1.0
feature submachines

submachine ConnectionHandler {
    initial Idle
    state Idle { ... }
    state Connected { ... }
}

machine Parent {
    state Link is ConnectionHandler { }
}
```

Implementations MUST reject `submachine` declarations with `FSM-E0600` ("feature
`submachines` is not enabled") unless `feature submachines` is declared. Semantics
are defined in FSM-SPEC-SEM §12.

---

# 16. File Extension and Identifiers

| Attribute | Value |
|---|---|
| File extension | `.fsm` |
| MIME type | `text/x-fsm-lang` |
| VS Code language ID | `fsm-lang` |
| Linguist name | `FSM-Lang` |

---

End of FSM-Lang Specification v2.0.0.
