//! Canonical IR data model — Doc 09 + Doc 00 §7.4 reconciler additions.
//!
//! Every public type is `Clone + Debug + PartialEq` and serde-serialisable
//! with `camelCase` field names so JSON shape matches Doc 09 exactly.
//!
//! The shape comments inline reference the wire-format clauses they
//! implement. Do not rename fields without bumping the IR major version per
//! Doc 09 §17.

#![allow(clippy::too_many_arguments)]

use fsm_diagnostics::{Diagnostic, SourceLocation};
use serde::{Deserialize, Serialize};

/// Default `TransitionObject.priority` when a transition declares no
/// `priority` clause.
///
/// Doc 04 §8.6 and Doc 09 §6 both specify **100**. Selection is min-wins on
/// `(priority, document_order)` (Doc 08 §4.2): a *lower* number is *higher*
/// priority. An unprioritized transition therefore sits at low priority so
/// that assigning a small explicit number floats a specific/guarded
/// transition above the default herd; a default of `0` would make every
/// unprioritized transition out-prioritise an explicitly-deprioritized one
/// and no transition could ever be placed below the default.
///
/// This is the single source of truth for the default; the analyzer's four
/// `lower_*` arms and the serde fallback below both reference it. Doc 00
/// §11.27 records the doc-vs-impl reconciliation (the lowering previously
/// materialized `0`, contradicting the spec — W7-FU-2).
pub const DEFAULT_TRANSITION_PRIORITY: u16 = 100;

#[doc(hidden)]
pub const fn default_transition_priority() -> u16 {
    DEFAULT_TRANSITION_PRIORITY
}

// ---------------------------------------------------------------------------
// Top-level IR document — Doc 09 §2 (with Doc 00 §7.4 additions).
// ---------------------------------------------------------------------------

/// The canonical IR document produced by `fsm ir` and consumed by every other
/// pipeline stage (codegen, simulator, formatter, LSP).
///
/// Field names map 1:1 onto the JSON wire form documented in Doc 09 §2:
/// `irVersion`, `sourceHash`, `sourceFiles`, `machines`, `diagnostics`. The
/// `irVersion` field is the authoritative version string and consumers MUST
/// reject documents whose major version disagrees with the consumer's
/// expected version (Doc 09 §17, enforced by [`crate::json::from_json`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ir {
    pub ir_version: String,
    pub source_hash: String,
    pub source_files: Vec<String>,
    pub machines: Vec<MachineObject>,
    /// Diagnostics emitted during compilation. The IR MAY be partial when
    /// errors are present (Doc 09 §1 design principle 5).
    pub diagnostics: Vec<DiagnosticObject>,
}

impl Default for Ir {
    /// Empty, valid IR document seeded with the v1.0.0 schema version.
    fn default() -> Self {
        Self {
            ir_version: CURRENT_IR_VERSION.to_owned(),
            source_hash: String::new(),
            source_files: Vec::new(),
            machines: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

/// Current IR schema version embedded in every produced [`Ir`] (Doc 09 §17).
pub const CURRENT_IR_VERSION: &str = "1.0.0";

// ---------------------------------------------------------------------------
// Diagnostics — Doc 09 §14.
// ---------------------------------------------------------------------------

/// A diagnostic embedded in the IR document.
///
/// The IR carries the existing [`fsm_diagnostics::Diagnostic`] verbatim so
/// that the wire form, severity table, and code catalogue have a single
/// source of truth (Doc 00 §7.1). The wrapper is a `transparent` newtype so
/// JSON output is identical to the inner shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiagnosticObject(pub Diagnostic);

impl From<Diagnostic> for DiagnosticObject {
    fn from(d: Diagnostic) -> Self {
        Self(d)
    }
}

// ---------------------------------------------------------------------------
// Machine — Doc 09 §3 (+ Doc 00 §7.4 const/import/feature/queue/target).
// ---------------------------------------------------------------------------

/// One `machine` declaration plus everything it owns.
///
/// Per the task brief, file-scope `imports` and `features` are attached to
/// each machine (rather than the top-level [`Ir`]); this keeps codegen
/// per-machine and avoids inter-machine coupling. `queue` and `targets`
/// remain per-machine per Doc 00 §7.4 (each machine has its own runtime
/// queue and code-emission profile).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineObject {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    pub context: ContextSchema,
    pub events: Vec<EventObject>,
    pub externs: Vec<ExternObject>,
    pub root: RegionObject,
    pub submachines: Vec<MachineObject>,
    /// `const` declarations (Doc 00 §7.4 item 1).
    pub consts: Vec<ConstDecl>,
    /// `import` declarations associated with this machine (Doc 00 §7.4 item 2).
    pub imports: Vec<ImportDecl>,
    /// `feature` flag names declared for this machine (Doc 00 §7.4 item 2).
    pub features: Vec<FeatureDecl>,
    /// Runtime queue configuration (Doc 00 §7.4 item 3).
    pub queue: QueueConfig,
    /// Codegen targets — one entry per `target NAME { ... }` block in the
    /// DSL (Doc 04 §4.4). Vec, not single value, so multi-profile builds can
    /// emit C99 + Cpp17 + Simulation from one IR document.
    pub targets: Vec<TargetConfig>,
    pub loc: SourceLocation,
}

// ---------------------------------------------------------------------------
// Context — Doc 09 §10.
// ---------------------------------------------------------------------------

/// Wrapper around the list of context fields. Doc 09 §10 wraps the list in
/// a `{ fields: [...] }` envelope rather than emitting the array directly,
/// so consumers can add other context-scope properties later without a
/// breaking schema change.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSchema {
    pub fields: Vec<ContextField>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextField {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    /// Default value literal; absent when the DSL field has no `=` clause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Literal>,
    pub loc: SourceLocation,
}

// ---------------------------------------------------------------------------
// Types — Doc 09 §10 (with Doc 00 §7.4 `array` extension reflecting the
// brief's "supports … array" requirement).
// ---------------------------------------------------------------------------

/// Type reference. Discriminated on `kind`.
///
/// `primitive` names match Doc 04 §3 token set: `bool`, `i8`..`i64`,
/// `u8`..`u64`, `f32`, `f64`, `string`. The `string` primitive is rejected
/// as a context-field type by the analyzer per Doc 04 §3.3, but is included
/// in the IR vocabulary so the formatter and decompile path can carry
/// `@id("...")` / `import "..."` argument strings round-trip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Type {
    /// `bool`, `u8`..`u64`, `i8`..`i64`, `f32`, `f64`, `string`.
    Primitive { name: String },
    /// Reference to an `enum` declaration. `enumId` is the stable enum ID.
    Enum {
        #[serde(rename = "enumId")]
        enum_id: String,
    },
    /// Verbatim C type — emitted unchanged into generated headers.
    Opaque {
        #[serde(rename = "cType")]
        c_type: String,
    },
    /// Fixed-size array. `element` is the contained type; `size` is the
    /// element count. Bounded so the C runtime stays heap-free (Doc 09 §1
    /// design principle 6).
    Array { element: Box<Type>, size: u32 },
}

// ---------------------------------------------------------------------------
// Events — Doc 09 §11.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventObject {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    pub payload: Vec<Param>,
    pub loc: SourceLocation,
}

// ---------------------------------------------------------------------------
// Externs — Doc 09 §12.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternObject {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    /// `pure` extern — usable in guard expressions per Doc 04 §2.5.
    pub pure: bool,
    pub params: Vec<Param>,
    /// `void` return is represented as `None`; matches Doc 04 §2.5 wording
    /// "return type omitted → void".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<Type>,
    pub loc: SourceLocation,
}

/// Shared parameter shape used by [`EventObject::payload`] and
/// [`ExternObject::params`]. Doc 09 §11 lists `id` + `loc` on event payload
/// fields; Doc 09 §12 omits them from extern params. Both shapes serialise
/// identically here, with the optional fields skipped when absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Param {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc: Option<SourceLocation>,
}

// ---------------------------------------------------------------------------
// Regions — Doc 09 §5.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionObject {
    pub id: String,
    /// Optional stable ID; the root region of a machine and auto-generated
    /// regions (`__region_N`) may not have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    pub name: String,
    pub initial: String,
    pub states: Vec<StateNode>,
    pub priority: u32,
    pub loc: SourceLocation,
}

// ---------------------------------------------------------------------------
// State node discriminated union — Doc 09 §4 (all twelve variants).
// ---------------------------------------------------------------------------

/// Every state and pseudo-state, tagged on `kind` per Doc 09 §4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StateNode {
    /// Simple leaf state (§4.1).
    Simple(SimpleState),
    /// Composite state with a single inner region (§4.2).
    Composite(CompositeState),
    /// Parallel state — `regions` MUST contain ≥ 2 entries (§4.3).
    Parallel(ParallelState),
    /// `initial` pseudo-state (§4.4).
    Initial(InitialPseudo),
    /// `final` state (§4.5).
    Final(FinalState),
    /// `choice` pseudo-state with runtime branch evaluation (§4.6).
    Choice(ChoiceState),
    /// `junction` pseudo-state with compile-time branch evaluation (§4.7).
    Junction(JunctionState),
    /// History pseudo-state — `default_target` is mandatory per Doc 00 B-14.
    History(HistoryObject),
    /// `fork` pseudo-state (§4.9).
    Fork(ForkPseudo),
    /// `join` pseudo-state (§4.10).
    Join(JoinPseudo),
    /// Cross-machine submachine reference (§4.11, gated by
    /// `feature submachines` per Doc 04 §15).
    #[serde(rename = "submachine_ref")]
    Submachine(SubmachineRef),
    /// `entry_point` pseudo-state (§4.12).
    #[serde(rename = "entry_point")]
    EntryPoint(EntryPointPseudo),
    /// `exit_point` pseudo-state (§4.12).
    #[serde(rename = "exit_point")]
    ExitPoint(ExitPointPseudo),
}

/// Simple state body (Doc 09 §4.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleState {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    pub entry: Vec<Statement>,
    pub exit: Vec<Statement>,
    pub transitions: Vec<TransitionObject>,
    pub timers: Vec<TimerObject>,
    pub defers: Vec<DeferDecl>,
    pub loc: SourceLocation,
}

/// Composite state body (Doc 09 §4.2). `regions` MUST contain exactly one
/// region; `history` is present when the state declares a history
/// pseudo-state directly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeState {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    pub entry: Vec<Statement>,
    pub exit: Vec<Statement>,
    pub transitions: Vec<TransitionObject>,
    pub timers: Vec<TimerObject>,
    pub defers: Vec<DeferDecl>,
    pub regions: Vec<RegionObject>,
    /// History pseudo-state if declared; `None` otherwise.
    #[serde(default)]
    pub history: Option<HistoryObject>,
    pub loc: SourceLocation,
}

/// Parallel state body (Doc 09 §4.3). `regions` MUST contain ≥ 2 entries
/// (analyzer enforces this — see Doc 10 H0004 for the one-region warning).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelState {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    pub entry: Vec<Statement>,
    pub exit: Vec<Statement>,
    pub transitions: Vec<TransitionObject>,
    pub timers: Vec<TimerObject>,
    pub defers: Vec<DeferDecl>,
    pub regions: Vec<RegionObject>,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialPseudo {
    pub id: String,
    pub target: String,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalState {
    pub id: String,
    pub stable_id: String,
    /// DSL-level name (`final PaymentFinal` → `"PaymentFinal"`). Used by
    /// codegen to derive the C enum / helper-function suffix; defaults to
    /// the empty string for historical fixtures that pre-date the field
    /// (consumers fall back to deriving a suffix from `id`).
    #[serde(default)]
    pub name: String,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceState {
    pub id: String,
    pub stable_id: String,
    pub branches: Vec<ChoiceBranch>,
    pub loc: SourceLocation,
}

/// Junction shares the choice branch shape (Doc 09 §4.7) — the difference
/// between `choice` and `junction` is when branches are evaluated, not the
/// JSON layout. Same struct intentionally.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JunctionState {
    pub id: String,
    pub stable_id: String,
    pub branches: Vec<ChoiceBranch>,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceBranch {
    pub guard: GuardExpr,
    pub target: String,
    pub actions: Vec<Statement>,
    pub loc: SourceLocation,
}

/// History pseudo-state (Doc 09 §4.8, Doc 00 B-14).
///
/// `default_target` is mandatory (not nullable). The analyzer rejects
/// missing defaults at compile time with `FSM-E0111`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryObject {
    pub id: String,
    pub stable_id: String,
    pub history_kind: HistoryKind,
    pub default_target: String,
    pub loc: SourceLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryKind {
    Shallow,
    Deep,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkPseudo {
    pub id: String,
    pub stable_id: String,
    pub targets: Vec<String>,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinPseudo {
    pub id: String,
    pub stable_id: String,
    pub sources: Vec<String>,
    pub target: String,
    pub actions: Vec<Statement>,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmachineRef {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    pub submachine_id: String,
    /// Map from declared `entry_point` name to a state ID inside the
    /// referenced submachine. Order-preserving via `Vec<(K, V)>` rather
    /// than `HashMap` so JSON output is deterministic.
    pub entry_points: Vec<(String, String)>,
    pub exit_points: Vec<(String, String)>,
    pub transitions: Vec<TransitionObject>,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPointPseudo {
    pub id: String,
    pub name: String,
    pub loc: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitPointPseudo {
    pub id: String,
    pub name: String,
    pub loc: SourceLocation,
}

// ---------------------------------------------------------------------------
// Transition — Doc 09 §6 (with Doc 00 §7.4 `kind` discriminator added;
// `internal: bool` retained for one cycle per B-06).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionObject {
    pub id: String,
    pub stable_id: String,
    pub source: String,
    pub target: String,
    /// `None` for completion transitions.
    #[serde(default)]
    pub trigger: Option<Trigger>,
    /// `None` is an unconditional transition.
    #[serde(default)]
    pub guard: Option<GuardExpr>,
    pub actions: Vec<Statement>,
    /// Doc 04 §8.6 / Doc 09 §6: lower value = higher priority; the default
    /// when no `priority` clause is declared is
    /// [`DEFAULT_TRANSITION_PRIORITY`] (100). `serde(default = …)` so IR
    /// JSON omitting the key (and the analyzer's `lower_*` arms) materialize
    /// the spec value, not `0` (Doc 00 §11.27 / W7-FU-2).
    #[serde(default = "default_transition_priority")]
    pub priority: u16,
    /// New required discriminator per Doc 00 §7.4 item 8.
    pub kind: TransitionKind,
    /// Retained for one minor cycle per B-06. Consumers MUST treat `kind ==
    /// Internal` as authoritative; this field is derivable from `kind`.
    #[deprecated(note = "derive from `kind`")]
    #[serde(default)]
    pub internal: bool,
    /// Optional branch-prediction hint (v1.1-W4 — Doc 04 §8.8, Doc 11 §28).
    /// `Some(Likely)` / `Some(Rare)` lower to a `__builtin_expect`-backed
    /// `<MACRO>_LIKELY` / `<MACRO>_UNLIKELY` wrapper around the transition's
    /// guard condition in the generated C; `None` (the default) emits the
    /// plain condition. This is a **pure codegen layout optimization with
    /// zero semantic effect**: it never changes which transition fires, only
    /// the hot/cold instruction placement the C compiler chooses. The
    /// simulator therefore accepts and ignores it. `#[serde(default)]` so
    /// pre-W4 IR JSON (no `hint` key) deserializes unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<BranchHint>,
    pub loc: SourceLocation,
}

/// Branch-prediction hint on a transition (v1.1-W4). Surface syntax is the
/// optional `likely` / `rare` prefix on any transition trigger form
/// (`likely on EVT …`, `rare after N ms …`, `rare done …`, `likely … ~> …`).
/// Lowered verbatim from the AST; the analyzer adds no diagnostic (a hint is
/// always syntactically valid and the parser admits at most one prefix).
///
/// Semantically inert — see [`TransitionObject::hint`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BranchHint {
    /// `likely` — the transition's guard is expected to hold (hot path).
    /// Lowers to `__builtin_expect((cond), 1)` (via the `_LIKELY` macro).
    Likely,
    /// `rare` — the transition's guard is expected to fail (cold path).
    /// Lowers to `__builtin_expect((cond), 0)` (via the `_UNLIKELY` macro).
    Rare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionKind {
    /// `->` exit / re-entry of source if source == target (Doc 04 §8.1).
    External,
    /// `~>` no exit of source, target must be a proper descendant of source
    /// (Doc 04 §8.3, Doc 00 B-09).
    Local,
    /// `internal on EVENT:` — no exit/entry actions execute (Doc 04 §8.2).
    Internal,
    /// `done ->` — completion transition (Doc 04 §8.4).
    Completion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Trigger {
    /// Event trigger (Doc 09 §6 `TriggerObject`).
    #[serde(rename = "event")]
    Event {
        #[serde(rename = "eventId")]
        event_id: String,
        /// Optional binding name for the payload inside the action list —
        /// e.g. `on PACKET(p) -> X` binds `p`. `None` when the DSL did not
        /// supply a name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload_binding: Option<String>,
    },
    /// `after` one-shot timer (Doc 09 §13). `timer_id` is the IR
    /// [`TimerObject::id`] that this transition is bound to — codegen uses it
    /// to emit a distinct `MOTOR_EVENT_TIMER_<ID>_FIRED` enum variant so the
    /// transition is dispatchable independently of `done` completion
    /// (audit P0-4).
    #[serde(rename = "after")]
    After {
        duration_ms: u32,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        timer_id: String,
    },
    /// `every` periodic timer. `timer_id` plays the same role as in `After`.
    #[serde(rename = "every")]
    Every {
        period_ms: u32,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        timer_id: String,
    },
    /// Completion of a referenced state (Doc 08 §4.4).
    #[serde(rename = "completion")]
    Completion { from: String },
}

// ---------------------------------------------------------------------------
// Guard expressions — Doc 09 §7.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuardExpr {
    /// `lhs OP rhs` — field comparison (Doc 04 §8.5).
    FieldCmp {
        lhs: FieldRef,
        op: CmpOp,
        rhs: GuardOperand,
    },
    /// Call to a `pure extern` (Doc 04 §8.5).
    ExternCall { callee: String, args: Vec<Expr> },
    /// Unary negation.
    Not { operand: Box<GuardExpr> },
    And {
        left: Box<GuardExpr>,
        right: Box<GuardExpr>,
    },
    Or {
        left: Box<GuardExpr>,
        right: Box<GuardExpr>,
    },
    /// Catch-all `[else]` branch.
    Else,
}

/// Right-hand side of a [`GuardExpr::FieldCmp`] — a literal or another
/// field reference (Doc 04 §8.5 `literal_or_field`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GuardOperand {
    Literal(Literal),
    FieldRef(FieldRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    NotEq,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<=")]
    LtEq,
    #[serde(rename = ">=")]
    GtEq,
}

// ---------------------------------------------------------------------------
// Action-block expression language — Doc 09 §9 (+ Doc 00 §7.4 cast).
// ---------------------------------------------------------------------------

/// Expression form used in action statements. Distinct from [`GuardExpr`]
/// because action expressions may call non-pure externs and use arithmetic
/// / bitwise operators.
///
/// Doc 09 §9 represents `field_ref` as a nested `{ kind: "field_ref", ref:
/// {...} }` envelope so the inner `FieldRef.kind` ("ctx" / "payload")
/// does not collide with the outer `Expr.kind` discriminator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expr {
    FieldRef {
        #[serde(rename = "ref")]
        field_ref: FieldRef,
    },
    Literal(Literal),
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `expr as T` — Doc 00 §7.4 item 5.
    Cast(CastExpr),
}

/// `ctx.field` or `payload.field` — Doc 09 §9 `FieldRef`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FieldRef {
    #[serde(rename = "ctx")]
    Ctx { field: String },
    #[serde(rename = "payload")]
    Payload { field: String },
}

/// `expr as T` — operand and target type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastExpr {
    pub operand: Box<Expr>,
    pub target_type: Type,
    pub loc: SourceLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    #[serde(rename = "!")]
    Not,
    #[serde(rename = "-")]
    Neg,
    #[serde(rename = "~")]
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Sub,
    #[serde(rename = "*")]
    Mul,
    #[serde(rename = "/")]
    Div,
    #[serde(rename = "%")]
    Mod,
    #[serde(rename = "&")]
    BitAnd,
    #[serde(rename = "|")]
    BitOr,
    #[serde(rename = "^")]
    BitXor,
    #[serde(rename = "<<")]
    Shl,
    #[serde(rename = ">>")]
    Shr,
    #[serde(rename = "&&")]
    LogAnd,
    #[serde(rename = "||")]
    LogOr,
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    NotEq,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = "<=")]
    LtEq,
    #[serde(rename = ">=")]
    GtEq,
}

// ---------------------------------------------------------------------------
// Literals — Doc 09 §9 + Doc 00 §7.4 (float, enum_variant).
// ---------------------------------------------------------------------------

/// Discriminated literal type. The wire tag is `literalKind` (per Doc 09
/// §9 example) rather than the more common `kind`, so the parent
/// expression's `kind: "literal"` tag remains unambiguous.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "literalKind", rename_all = "snake_case")]
pub enum Literal {
    /// 64-bit signed integer. The analyzer narrows to the target field's
    /// width when type-checking (Doc 04 §3.2).
    Int(IntLit),
    /// Doc 00 §7.4 item 6.
    Float(FloatLit),
    Bool(BoolLit),
    String(StringLit),
    /// Doc 00 §7.4 item 7 — qualified enum literal like `PacketType.DATA`.
    #[serde(rename = "enum_variant")]
    EnumVariant(EnumVariantLit),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntLit {
    pub value: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatLit {
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoolLit {
    pub value: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringLit {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumVariantLit {
    pub enum_name: String,
    pub variant_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loc: Option<SourceLocation>,
}

// ---------------------------------------------------------------------------
// Statements (action block) — Doc 09 §8.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Statement {
    Assign {
        target: FieldRef,
        value: Expr,
    },
    If {
        condition: Expr,
        then: Vec<Statement>,
        /// `else_` because `else` is a Rust keyword. Doc 09 §8 also uses
        /// the trailing underscore in its example JSON.
        #[serde(rename = "else")]
        else_: Vec<Statement>,
    },
    While {
        condition: Expr,
        body: Vec<Statement>,
    },
    For {
        init: Box<Statement>,
        condition: Expr,
        update: Box<Statement>,
        body: Vec<Statement>,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    Raise {
        event_id: String,
        args: Vec<Expr>,
    },
    Send {
        event_id: String,
        args: Vec<Expr>,
        machine_id: String,
    },
    Defer {
        event_id: String,
    },
}

// ---------------------------------------------------------------------------
// Timers — Doc 09 §13.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerObject {
    pub id: String,
    pub stable_id: String,
    pub kind: TimerKind,
    /// Resolved duration in milliseconds. Doc 09 §13 promises future
    /// support for identifier-referenced durations; for v1.0 every parser
    /// resolves the const-expression to an integer at IR emission time.
    pub duration_ms: u32,
    pub owner_state_id: String,
    /// Target state for `after` / `every`; `None` for `every_internal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub actions: Vec<Statement>,
    pub loc: SourceLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerKind {
    /// `after N ms -> X` — one-shot.
    After,
    /// `every N ms -> X` — periodic with transition.
    Every,
    /// `every N ms:` — periodic, action-only, no transition (Doc 04 §9.3).
    EveryInternal,
}

// ---------------------------------------------------------------------------
// Defer declarations — Doc 04 §9.4.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeferDecl {
    pub event_id: String,
    pub loc: SourceLocation,
}

// ---------------------------------------------------------------------------
// Reconciler additions — Doc 00 §7.4.
// ---------------------------------------------------------------------------

/// `const NAME = value` declaration (Doc 00 §7.4 item 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstDecl {
    pub id: String,
    pub stable_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub value: Literal,
    pub loc: SourceLocation,
}

/// `import "..." [as X] [{ named, items }]` (Doc 00 §7.4 item 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDecl {
    pub path: String,
    /// `as X` rename; `None` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// `{ A, B, C }` named-imports list; `None` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named_imports: Option<Vec<String>>,
    pub loc: SourceLocation,
}

/// `feature NAME` declaration (Doc 04 §2.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureDecl {
    pub name: String,
    pub loc: SourceLocation,
}

/// `queue { capacity = N, overflow = ... }` block (Doc 00 §7.4 item 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueConfig {
    pub capacity: u32,
    pub overflow_policy: OverflowPolicy,
    pub loc: SourceLocation,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            capacity: 16,
            overflow_policy: OverflowPolicy::Assert,
            loc: SourceLocation::new("<default>", fsm_diagnostics::Span::empty(0), 0, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowPolicy {
    Assert,
    DropOldest,
    DropNewest,
    Error,
}

/// `target NAME { ... }` block (Doc 00 §7.4 item 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetConfig {
    /// User-defined block label (`target C99 { ... }` → `"C99"`).
    pub name: String,
    /// Doc 00 §7.4 enumerates the built-ins; free-form to allow third-party
    /// targets in 1.x without a schema bump.
    pub profile: String,
    /// Free-form `key = value` config — see Doc 04 §4.4 standard keys
    /// (`strategy`, `allow_float`, `max_nesting`). Order-preserving Vec so
    /// JSON output is deterministic.
    pub options: Vec<(String, TargetOptionValue)>,
    pub loc: SourceLocation,
}

/// Permitted right-hand sides of a `target` block config entry — per Doc
/// 04 §4.4 `config_entry = identifier "=" ( integer | boolean | identifier )`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TargetOptionValue {
    Int(i64),
    Bool(bool),
    Ident(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::Span;

    fn loc() -> SourceLocation {
        SourceLocation::new("test.fsm", Span::new(0, 1), 1, 1)
    }

    #[test]
    fn ir_default_seeds_version() {
        let ir = Ir::default();
        assert_eq!(ir.ir_version, "1.0.0");
        assert!(ir.machines.is_empty());
        assert!(ir.diagnostics.is_empty());
    }

    #[test]
    fn transition_kind_round_trips_as_lowercase_string() {
        // External serialises to "external" so the JSON shape matches Doc
        // 00 §7.4 item 8.
        let json = serde_json::to_string(&TransitionKind::External).unwrap();
        assert_eq!(json, "\"external\"");
        let back: TransitionKind = serde_json::from_str("\"completion\"").unwrap();
        assert_eq!(back, TransitionKind::Completion);
    }

    #[test]
    fn history_default_target_is_required() {
        // No `Option<String>` wrapper — value must be present. This is the
        // type-system embodiment of B-14.
        let h = HistoryObject {
            id: "ps-history-0".into(),
            stable_id: "M:history:H".into(),
            history_kind: HistoryKind::Shallow,
            default_target: "s-idle".into(),
            loc: loc(),
        };
        let json = serde_json::to_string(&h).unwrap();
        assert!(json.contains("\"defaultTarget\":\"s-idle\""));
    }

    #[test]
    fn state_node_kind_tags_per_doc_09() {
        let initial = StateNode::Initial(InitialPseudo {
            id: "ps-initial-0".into(),
            target: "s-idle".into(),
            loc: loc(),
        });
        let json = serde_json::to_string(&initial).unwrap();
        assert!(json.contains("\"kind\":\"initial\""), "got: {json}");

        let sub = StateNode::Submachine(SubmachineRef {
            id: "s-sub-0".into(),
            stable_id: "M:state:Sub".into(),
            name: "Sub".into(),
            submachine_id: "m-sub".into(),
            entry_points: vec![],
            exit_points: vec![],
            transitions: vec![],
            loc: loc(),
        });
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("\"kind\":\"submachine_ref\""), "got: {json}");
    }

    #[test]
    fn cmp_op_serializes_as_symbol() {
        assert_eq!(serde_json::to_string(&CmpOp::Eq).unwrap(), "\"==\"");
        assert_eq!(serde_json::to_string(&CmpOp::GtEq).unwrap(), "\">=\"");
    }

    #[test]
    fn literal_kind_tag_uses_literalkind_field() {
        let lit = Literal::Int(IntLit {
            value: 42,
            loc: None,
        });
        let json = serde_json::to_string(&lit).unwrap();
        // Doc 09 §9 uses `literalKind` as the tag name, NOT `kind`, so the
        // outer expression can carry `kind: "literal"`.
        assert!(json.contains("\"literalKind\":\"int\""), "got: {json}");
    }

    #[test]
    fn float_and_enum_variant_literals() {
        let f = Literal::Float(FloatLit {
            value: 1.5,
            loc: None,
        });
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"literalKind\":\"float\""), "got: {json}");

        let ev = Literal::EnumVariant(EnumVariantLit {
            enum_name: "PacketType".into(),
            variant_name: "HEARTBEAT".into(),
            loc: None,
        });
        let json = serde_json::to_string(&ev).unwrap();
        assert!(
            json.contains("\"literalKind\":\"enum_variant\""),
            "got: {json}"
        );
        assert!(json.contains("\"enumName\":\"PacketType\""), "got: {json}");
    }

    #[test]
    fn cast_expr_carries_target_type() {
        let cast = Expr::Cast(CastExpr {
            operand: Box::new(Expr::Literal(Literal::Int(IntLit {
                value: 7,
                loc: None,
            }))),
            target_type: Type::Primitive { name: "u32".into() },
            loc: loc(),
        });
        let json = serde_json::to_string(&cast).unwrap();
        assert!(json.contains("\"kind\":\"cast\""), "got: {json}");
        assert!(json.contains("\"targetType\""), "got: {json}");
    }

    #[test]
    fn array_type_round_trip() {
        let ty = Type::Array {
            element: Box::new(Type::Primitive { name: "u8".into() }),
            size: 16,
        };
        let json = serde_json::to_string(&ty).unwrap();
        let back: Type = serde_json::from_str(&json).unwrap();
        assert_eq!(ty, back);
    }
}
