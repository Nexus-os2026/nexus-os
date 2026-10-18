//! The sealed dispatch context, its working set and the unit-taint algebra
//! (P3-HM-CONTRACT-02 §2.2, §2.5, §2.11, §2.12, §6.3, §7.5, §11.0, §11.1).
//!
//! `DispatchContext`, `WorkingSet`, `DerivedItem`, `RecalledRecord` and
//! `PromptEnvelope` have private fields and no public constructor. They are
//! minted only inside `nexus_kernel::hm`, and none of them implements
//! `Default`, `Clone`, `Serialize`, `Deserialize`, `From<&str>` or `FromStr`.
//!
//! Positive control: the public accessors compile from outside the module,
//! so each `compile_fail` example below fails only on its forbidden line.
//!
//! ```
//! use nexus_kernel::hm::{
//!     DerivedItem, DispatchContext, OriginLabel, PromptEnvelope, RecalledRecord, WorkingSet,
//! };
//! fn observe(
//!     c: &DispatchContext,
//!     w: &WorkingSet,
//!     d: &DerivedItem,
//!     r: &RecalledRecord,
//!     e: &PromptEnvelope,
//! ) -> OriginLabel {
//!     let _ = (w.derived_labels(), d.text(), r.label(), e.record_count());
//!     c.taint().join(d.label()).join(e.raw_label())
//! }
//! let _ = observe;
//! assert_eq!(OriginLabel::LocalOnlyOrigin.class_name(), "LOCAL_ONLY_ORIGIN");
//! ```

use uuid::Uuid;

use super::{EvaluatedLabel, OriginLabel, RecordTable};

// ── Route class, principal, unit ────────────────────────────────────────────

/// Route class of a unit or of a transport (§2.4, §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    Local,
    NonLocal,
}

impl RouteClass {
    /// The class name used in storage and in event fields.
    pub fn class_name(self) -> &'static str {
        match self {
            RouteClass::Local => "local",
            RouteClass::NonLocal => "non_local",
        }
    }
}

/// The backend-authenticated party a dispatch acts for (§2.5). A caller
/// string is never a principal: `Agent` is checked against the
/// `AgentRegistry` at unit start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Principal {
    Agent(Uuid),
    OwnerDirect,
    System(&'static str),
}

/// The unit of work a context belongs to (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Goal {
        goal_id: Uuid,
    },
    HiveSubtask {
        session_id: Uuid,
        subtask_id: u32,
    },
    TeamTask {
        workflow_id: Uuid,
        task_index: u32,
    },
    ChatTurn {
        turn_id: Uuid,
    },
    PostGoal {
        goal_id: Uuid,
    },
    Dream {
        entry_id: Uuid,
    },
    /// One backend request of a non-agent principal (`OwnerDirect` or
    /// `System`): a human-direct exchange, a fixed system prompt or an
    /// excluded-store operation (§12.1 HD, HB, SY, XS).
    Request {
        request_id: Uuid,
    },
}

impl Unit {
    /// `hm_dispatch_admission.unit_kind`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Unit::Goal { .. } => "goal",
            Unit::HiveSubtask { .. } => "hive_subtask",
            Unit::TeamTask { .. } => "team_task",
            Unit::ChatTurn { .. } => "chat_turn",
            Unit::PostGoal { .. } => "post_goal",
            Unit::Dream { .. } => "dream",
            Unit::Request { .. } => "request",
        }
    }
}

// ── Taint algebra (§2.12, §6.3, §7.5) ───────────────────────────────────────

/// Starting taint and route class of a unit (§6.3, §7.5): `LOCAL_ONLY_ORIGIN`
/// and `local` if the agent or its HiveMind/team floor is local-only at unit
/// start, else `UNRESTRICTED_ORIGIN` and `non_local`.
#[doc(hidden)]
pub fn unit_start_state(local_only: bool, floor_local_only: bool) -> (OriginLabel, RouteClass) {
    if local_only || floor_local_only {
        (OriginLabel::LocalOnlyOrigin, RouteClass::Local)
    } else {
        (OriginLabel::UnrestrictedOrigin, RouteClass::NonLocal)
    }
}

/// One taint step (§2.12, §7.5): the taint joins `label`; it never falls.
/// The route class tightens to `local` once the taint is restricted and
/// never loosens. An observed tightening is the step with
/// `LOCAL_ONLY_ORIGIN`.
#[doc(hidden)]
pub fn unit_taint_step(
    taint: OriginLabel,
    route_class: RouteClass,
    label: OriginLabel,
) -> (OriginLabel, RouteClass) {
    let taint = taint.join(label);
    let route_class = if taint == OriginLabel::UnrestrictedOrigin {
        route_class
    } else {
        RouteClass::Local
    };
    (taint, route_class)
}

// ── Record identity ─────────────────────────────────────────────────────────

/// The identity of one retained record, as read or written by the backend.
/// It has no public constructor; it can only raise a label (as a parent),
/// never lower one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRef {
    pub(in crate::hm) table: RecordTable,
    pub(in crate::hm) record_id: String,
}

impl RecordRef {
    pub fn table(&self) -> RecordTable {
        self.table
    }

    pub fn record_id(&self) -> &str {
        &self.record_id
    }
}

// ── Policy snapshot and restrictions (§11.1) ────────────────────────────────

/// The §4.2 policy a context was minted or rebuilt with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicySnapshot {
    pub(in crate::hm) agent_id: Uuid,
    pub(in crate::hm) local_only: bool,
    pub(in crate::hm) policy_generation: i64,
    pub(in crate::hm) revocation_seq: i64,
}

impl PolicySnapshot {
    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    pub fn local_only(&self) -> bool {
        self.local_only
    }

    pub fn policy_generation(&self) -> i64 {
        self.policy_generation
    }

    pub fn revocation_seq(&self) -> i64 {
        self.revocation_seq
    }
}

/// `restrictions` of §11.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restrictions {
    pub(in crate::hm) local_only: bool,
    pub(in crate::hm) retained_data_permitted: bool,
    pub(in crate::hm) max_records: usize,
}

impl Restrictions {
    pub fn local_only(&self) -> bool {
        self.local_only
    }

    pub fn retained_data_permitted(&self) -> bool {
        self.retained_data_permitted
    }

    pub fn max_records(&self) -> usize {
        self.max_records
    }
}

// ── Working set and derived items (§2.11) ───────────────────────────────────

/// Everything a unit can place in a later prompt (§2.11): verbatim items
/// (record identity and the label evaluated when the record entered) and the
/// fixed labels of the derived items that entered.
///
/// ```compile_fail
/// use nexus_kernel::hm::WorkingSet;
/// let _ = WorkingSet { verbatim: Vec::new(), derived: Vec::new() };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::WorkingSet;
/// let _: WorkingSet = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::WorkingSet;
/// let _ = "x".parse::<WorkingSet>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::WorkingSet;
/// let _ = WorkingSet::from("x");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::WorkingSet;
/// fn dup(w: &WorkingSet) -> WorkingSet { WorkingSet::clone(w) }
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::WorkingSet;
/// let _ = serde_json::from_str::<WorkingSet>("{}");
/// ```
#[derive(Debug)]
pub struct WorkingSet {
    pub(in crate::hm) verbatim: Vec<(RecordRef, EvaluatedLabel)>,
    pub(in crate::hm) derived: Vec<OriginLabel>,
}

impl WorkingSet {
    /// The verbatim items: record identity and the label evaluated when the
    /// record entered. Every decision re-evaluates the label.
    pub fn verbatim(&self) -> impl Iterator<Item = (&RecordRef, EvaluatedLabel)> {
        self.verbatim.iter().map(|(r, l)| (r, *l))
    }

    /// The fixed labels of the derived items that entered the working set.
    pub fn derived_labels(&self) -> &[OriginLabel] {
        &self.derived
    }
}

/// A derived item (§2.11): a model reply, plan, actuator output, summary or
/// merge. Its label is fixed when it is produced and never lowered.
///
/// ```compile_fail
/// use nexus_kernel::hm::{DerivedItem, OriginLabel};
/// let _ = DerivedItem { label: OriginLabel::UnrestrictedOrigin, text: String::new() };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DerivedItem;
/// let _: DerivedItem = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DerivedItem;
/// let _ = "x".parse::<DerivedItem>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DerivedItem;
/// let _ = DerivedItem::from("x");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DerivedItem;
/// fn dup(d: &DerivedItem) -> DerivedItem { DerivedItem::clone(d) }
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DerivedItem;
/// let _ = serde_json::from_str::<DerivedItem>("{}");
/// ```
pub struct DerivedItem {
    pub(in crate::hm) label: OriginLabel,
    pub(in crate::hm) text: String,
}

/// Content-free: the text is never formatted.
impl std::fmt::Debug for DerivedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedItem")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl DerivedItem {
    pub fn label(&self) -> OriginLabel {
        self.label
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

// ── DispatchContext (§11.1) ─────────────────────────────────────────────────

/// The sealed dispatch context (§11.1), minted only by `DispatchIssuer`.
/// Child contexts come only from methods on an existing context.
///
/// Every field is listed, so only their privacy stops the literal:
/// ```compile_fail
/// use nexus_kernel::hm::DispatchContext;
/// let _ = DispatchContext {
///     nonce: todo!(),
///     issuer_id: todo!(),
///     principal: todo!(),
///     unit: todo!(),
///     policy: todo!(),
///     floor_local_only: todo!(),
///     route_class: todo!(),
///     taint: todo!(),
///     working_set: todo!(),
///     restrictions: todo!(),
///     class: todo!(),
///     local_transport_used: todo!(),
/// };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DispatchContext;
/// let _: DispatchContext = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DispatchContext;
/// let _ = "x".parse::<DispatchContext>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DispatchContext;
/// let _ = DispatchContext::from("x");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DispatchContext;
/// fn dup(c: &DispatchContext) -> DispatchContext { DispatchContext::clone(c) }
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::DispatchContext;
/// let _ = serde_json::from_str::<DispatchContext>("{}");
/// ```
#[derive(Debug)]
pub struct DispatchContext {
    /// 128-bit identity of this context instance; binds envelopes to it.
    pub(in crate::hm) nonce: [u8; 16],
    /// Identity of the issuer that minted it.
    pub(in crate::hm) issuer_id: [u8; 16],
    pub(in crate::hm) principal: Principal,
    pub(in crate::hm) unit: Unit,
    pub(in crate::hm) policy: PolicySnapshot,
    pub(in crate::hm) floor_local_only: bool,
    pub(in crate::hm) route_class: RouteClass,
    pub(in crate::hm) taint: OriginLabel,
    pub(in crate::hm) working_set: WorkingSet,
    pub(in crate::hm) restrictions: Restrictions,
    /// How this context dispatches (§12.1).
    pub(in crate::hm) class: DispatchClass,
    /// A dispatch of this context was sent on an attested local transport:
    /// the route restriction of the entries it produces (§11.3).
    pub(in crate::hm) local_transport_used: bool,
}

/// How a context dispatches (§12.1, mission R2): an agent's (AR, AC: §12.4
/// and §12.6 in full), an Owner's retained entry (HR: §12.4 steps 2, 4 and
/// 5, with no agent), or a plain non-agent request (HD, HB, SY, XS: no
/// admission row, no inclusion, unrestricted carried items on a non-local
/// route).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::hm) enum DispatchClass {
    Agent,
    OwnerRetained,
    Plain,
}

impl DispatchContext {
    pub fn principal(&self) -> Principal {
        self.principal
    }

    pub fn unit(&self) -> Unit {
        self.unit
    }

    pub fn policy(&self) -> PolicySnapshot {
        self.policy
    }

    /// The HiveMind/team floor: true when an ancestor context was local-only.
    pub fn floor_local_only(&self) -> bool {
        self.floor_local_only
    }

    pub fn route_class(&self) -> RouteClass {
        self.route_class
    }

    /// The unit taint (§2.12). It never decreases.
    pub fn taint(&self) -> OriginLabel {
        self.taint
    }

    pub fn working_set(&self) -> &WorkingSet {
        &self.working_set
    }

    pub fn restrictions(&self) -> Restrictions {
        self.restrictions
    }

    /// Whether this context and `other` were minted by the same issuer.
    pub fn same_issuer(&self, other: &DispatchContext) -> bool {
        self.issuer_id == other.issuer_id
    }

    /// Whether `envelope` was built from this context instance.
    pub fn owns(&self, envelope: &PromptEnvelope) -> bool {
        envelope.ctx_nonce == self.nonce && envelope.issuer_id == self.issuer_id
    }

    /// The label of a retained in-memory entry this chat turn produces
    /// (§6.5, §11.3): the ⊔ of the turn's taint and its route restriction
    /// (`LOCAL_ONLY_ORIGIN` when the turn is local-only or one of its
    /// dispatches went to an attested local transport), with the turn's
    /// principal. `None` outside a chat turn, or for a `System` principal.
    pub fn entry_label(&self) -> Option<EntryLabel> {
        if !matches!(self.unit, Unit::ChatTurn { .. })
            || matches!(self.principal, Principal::System(_))
        {
            return None;
        }
        let route = if self.restrictions.local_only || self.local_transport_used {
            OriginLabel::LocalOnlyOrigin
        } else {
            OriginLabel::UnrestrictedOrigin
        };
        Some(EntryLabel {
            principal: self.principal,
            label: self.taint.join(route),
        })
    }

    /// Absorbs an envelope this context owns (mission R1 (ii)), whatever its
    /// admission outcome: every inclusion enters the working set as a
    /// verbatim item with its recall-time label, and every carried item's
    /// label enters the working set and joins the taint (§6.3). Nothing is
    /// lowered.
    pub(in crate::hm) fn absorb(
        &mut self,
        inclusions: &[(RecordRef, EvaluatedLabel)],
        carried: &[OriginLabel],
    ) {
        for item in inclusions {
            if !self.working_set.verbatim.contains(item) {
                self.working_set.verbatim.push(item.clone());
            }
        }
        for l in carried {
            if !self.working_set.derived.contains(l) {
                self.working_set.derived.push(*l);
            }
            let (taint, route_class) = unit_taint_step(self.taint, self.route_class, *l);
            self.taint = taint;
            self.route_class = route_class;
        }
    }
}

// ── Retained in-memory entries (§2.1, §6.5, §11.3) ──────────────────────────

/// The label and principal of a retained in-memory entry (a dream-queue or
/// auto-evolution entry). It is set only from the chat turn that produced
/// the entry ([`DispatchContext::entry_label`]) and is never decoded from
/// text. A dream unit of the entry starts from it
/// (`DispatchIssuer::begin_entry_unit`).
///
/// ```compile_fail
/// use nexus_kernel::hm::{EntryLabel, OriginLabel, Principal};
/// let _ = EntryLabel { principal: Principal::OwnerDirect, label: OriginLabel::UnrestrictedOrigin };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::EntryLabel;
/// let _: EntryLabel = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::EntryLabel;
/// let _ = "x".parse::<EntryLabel>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::EntryLabel;
/// let _ = EntryLabel::from("x");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::EntryLabel;
/// let _ = serde_json::from_str::<EntryLabel>("{}");
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct EntryLabel {
    pub(in crate::hm) principal: Principal,
    pub(in crate::hm) label: OriginLabel,
}

impl EntryLabel {
    pub fn principal(&self) -> Principal {
        self.principal
    }

    pub fn label(&self) -> OriginLabel {
        self.label
    }
}

// ── RecalledRecord and PromptEnvelope (§11.2) ───────────────────────────────

/// One retained record returned by route-first recall (§12.3): a verbatim
/// item. Its content is private; `PromptBuilder::include` is the only way to
/// place it in a prompt.
///
/// Every field is listed, so only their privacy stops the literal:
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// let _ = RecalledRecord {
///     reference: todo!(),
///     agent_id: todo!(),
///     label: todo!(),
///     source_kind: todo!(),
///     content: todo!(),
/// };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// let _: RecalledRecord = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// let _ = "x".parse::<RecalledRecord>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// let _ = RecalledRecord::from("x");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// fn dup(r: &RecalledRecord) -> RecalledRecord { RecalledRecord::clone(r) }
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// let _ = serde_json::from_str::<RecalledRecord>("{}");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::RecalledRecord;
/// fn peek(r: &RecalledRecord) -> &str { &r.content }
/// ```
pub struct RecalledRecord {
    pub(in crate::hm) reference: RecordRef,
    pub(in crate::hm) agent_id: String,
    pub(in crate::hm) label: EvaluatedLabel,
    pub(in crate::hm) source_kind: String,
    pub(in crate::hm) content: String,
}

/// Content-free: the content is never formatted.
impl std::fmt::Debug for RecalledRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecalledRecord")
            .field("reference", &self.reference)
            .field("label", &self.label)
            .field("source_kind", &self.source_kind)
            .finish_non_exhaustive()
    }
}

impl RecalledRecord {
    pub fn record_ref(&self) -> &RecordRef {
        &self.reference
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// The label evaluated when the record was recalled.
    pub fn label(&self) -> EvaluatedLabel {
        self.label
    }

    pub fn source_kind(&self) -> &str {
        &self.source_kind
    }
}

/// A prompt under construction (§11.2). Retained-record content enters only
/// through `include`, derived content only through `carry`.
pub struct PromptBuilder<'c> {
    ctx: &'c DispatchContext,
    inclusions: Vec<(RecordRef, EvaluatedLabel)>,
    carried: Vec<OriginLabel>,
    text: String,
}

impl<'c> PromptBuilder<'c> {
    pub fn new(ctx: &'c DispatchContext) -> Self {
        Self {
            ctx,
            inclusions: Vec::new(),
            carried: Vec::new(),
            text: String::new(),
        }
    }

    /// Backend text that carries no retained record: constant instructions
    /// or the unit's own current input.
    pub fn text(&mut self, text: &str) -> &mut Self {
        self.text.push_str(text);
        self
    }

    /// Adds one verbatim item.
    pub fn include(&mut self, record: &RecalledRecord) -> &mut Self {
        self.text.push_str(&record.content);
        self.text.push('\n');
        self.inclusions
            .push((record.reference.clone(), record.label));
        self
    }

    /// Adds one derived item.
    pub fn carry(&mut self, item: &DerivedItem) -> &mut Self {
        self.text.push_str(&item.text);
        self.text.push('\n');
        self.carried.push(item.label);
        self
    }

    /// The envelope. `raw_label` is the ⊔ of the unit taint, every
    /// inclusion's evaluated label (granted or not) and every carried item's
    /// label; nothing lowers it.
    pub fn build(self) -> PromptEnvelope {
        let raw_label = self
            .inclusions
            .iter()
            .map(|(_, l)| l.effective())
            .chain(self.carried.iter().copied())
            .fold(self.ctx.taint, OriginLabel::join);
        PromptEnvelope {
            ctx_nonce: self.ctx.nonce,
            issuer_id: self.ctx.issuer_id,
            inclusions: self.inclusions,
            carried: self.carried,
            raw_label,
            text: self.text,
        }
    }
}

/// A built prompt (§11.2): its inclusions, carried labels and `raw_label`.
/// Under AD-4 it has no grant dependencies. Its bytes are released only by
/// an admission (§12.4).
///
/// Every field is listed, so only their privacy stops the literal:
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// let _ = PromptEnvelope {
///     ctx_nonce: todo!(),
///     issuer_id: todo!(),
///     inclusions: todo!(),
///     carried: todo!(),
///     raw_label: todo!(),
///     text: todo!(),
/// };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// let _: PromptEnvelope = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// let _ = "x".parse::<PromptEnvelope>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// let _ = PromptEnvelope::from("x");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// fn dup(e: &PromptEnvelope) -> PromptEnvelope { PromptEnvelope::clone(e) }
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// let _ = serde_json::from_str::<PromptEnvelope>("{}");
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::PromptEnvelope;
/// fn peek(e: &PromptEnvelope) -> &str { &e.text }
/// ```
pub struct PromptEnvelope {
    pub(in crate::hm) ctx_nonce: [u8; 16],
    pub(in crate::hm) issuer_id: [u8; 16],
    pub(in crate::hm) inclusions: Vec<(RecordRef, EvaluatedLabel)>,
    pub(in crate::hm) carried: Vec<OriginLabel>,
    pub(in crate::hm) raw_label: OriginLabel,
    pub(in crate::hm) text: String,
}

/// Content-free: the prompt text is never formatted.
impl std::fmt::Debug for PromptEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptEnvelope")
            .field("inclusions", &self.inclusions)
            .field("carried", &self.carried)
            .field("raw_label", &self.raw_label)
            .finish_non_exhaustive()
    }
}

impl PromptEnvelope {
    /// The label of this prompt's reply (§11.2), as built.
    pub fn raw_label(&self) -> OriginLabel {
        self.raw_label
    }

    pub fn record_count(&self) -> usize {
        self.inclusions.len()
    }

    pub fn carried_count(&self) -> usize {
        self.carried.len()
    }

    /// Byte length of the prompt (not its content).
    pub fn prompt_bytes(&self) -> usize {
        self.text.len()
    }
}
