//! Diagnostic privacy oracles: no broker type that carries user, agent or
//! model content may print that content through `{:?}` or `{:#?}`.
//!
//! Each test builds a real value through the type's normal construction
//! path where one exists (the Files door for a prepared file action, its
//! commitment view and the owner's dialog; the planner, the command grammar
//! and the catalogs for intents, tool invocations and operation requests;
//! the pipeline over a real store for effect output, evidence records and
//! rows), puts a unique canary in every content field (ASCII, then the
//! Unicode word `面包`) and asserts that neither `{:?}` nor `{:#?}` shows
//! any of it, in any form it takes in a diagnostic: raw, percent-encoded,
//! hex-escaped or as the decimal list a `Vec<u8>` prints.
//!
//! Where a type has a display or dialog surface, the same test asserts that
//! the surface still shows the canary: redacting a diagnostic must not
//! change what the owner reads or what is recorded.

use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use crate::authority::clock::{ManualClock, SystemClock};
use crate::authority::commitment::{
    CommitmentView, FailureClass, Outcome, PreparedAction, TargetIdentity,
};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{EvidencePhase, EvidenceRecord, MemoryEvidence};
use crate::authority::ids::{AgentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::broker::Vault;
use crate::browser::{BrowserIntent, BrowserStep};
use crate::connector::{self, ConnectorIntent, DestinationIdentity, OperationRequest};
use crate::control::{Control, EffectOutput, PendingEffect, Preparation};
use crate::display::{InputIntent, PerceptionIntent, WindowInfo};
use crate::egress::destination::Destination;
use crate::egress::transport::{HttpResponse, Method, PinnedRequest};
use crate::egress::EgressIntent;
use crate::evidence_store::tests::Scratch;
use crate::evidence_store::{PrimaryEvidence, Row};
use crate::files::{Exclusions, FileCreate, WorkspaceInfo};
use crate::governed::{AgentDoor, AgentOutcome, GovernedControl, GrantRequest, Intent};
use crate::harness_tests::{temp_root, TempRoot, Yes};
use crate::ingress::{understand, Attachment, Attachments, CommandEnvelope, Understood};
use crate::planned::{classify, Disposition};
use crate::tool::{self, ToolIntent, ToolInvocation};
use nexus_kernel::cognitive::types::BrowserAction;
use nexus_kernel::cognitive::PlannedAction;
use serde_json::{json, Value};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ------------------------------------------------------------ canaries

/// A canary for one content field: ASCII, then the Unicode word.
fn canary(field: &str) -> String {
    format!("NXCANARY-{field}-4d1e8b-面包")
}

/// The ASCII part of the same canary, for a field that only takes ASCII
/// (a file name).
fn ascii(field: &str) -> String {
    format!("NXCANARY-{field}-4d1e8b")
}

/// What no diagnostic may contain, whatever the field: the canary marker,
/// the Unicode word raw, percent-encoded and hex-escaped, and the decimal
/// lists a byte vector prints for both.
fn markers() -> Vec<String> {
    let list = |text: &str| {
        text.bytes()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    vec![
        "NXCANARY".to_string(),
        "面包".to_string(),
        "%E9%9D%A2".to_string(),
        "%e9%9d%a2".to_string(),
        "\\xe9\\x9d\\xa2".to_string(),
        list("NXCANARY"),
        list("面包"),
    ]
}

/// Whether `shown` contains `needle`, either as it is or, for a decimal
/// list, with the whitespace of the pretty form taken out.
fn shows(shown: &str, needle: &str) -> bool {
    if shown.contains(needle) {
        return true;
    }
    let squeezed: String = shown.chars().filter(|c| !c.is_whitespace()).collect();
    squeezed.contains(needle)
}

/// Neither `{:?}` nor `{:#?}` of `value` shows a canary of any field.
fn assert_content_free<T: Debug + ?Sized>(what: &str, value: &T, fields: &[&str]) {
    for (form, shown) in [
        ("{:?}", format!("{value:?}")),
        ("{:#?}", format!("{value:#?}")),
    ] {
        let mut leaked: Vec<String> = markers()
            .into_iter()
            .filter(|marker| shows(&shown, marker))
            .collect();
        leaked.extend(
            fields
                .iter()
                .filter(|field| shown.contains(&format!("NXCANARY-{field}-")))
                .map(|field| format!("field `{field}`")),
        );
        assert!(
            leaked.is_empty(),
            "{what}: {form} shows content ({leaked:?}):\n{shown}"
        );
    }
}

/// A display or dialog surface still shows the canary.
fn assert_surface(surface: &str, text: &str, canary: &str) {
    assert!(
        text.contains(canary),
        "{surface} no longer shows `{canary}`:\n{text}"
    );
}

/// A diagnostic keeps a non-content fact (an id, a class, a host): the
/// redaction removes content, not usefulness.
fn assert_keeps<T: Debug + ?Sized>(what: &str, value: &T, fact: &str) {
    let shown = format!("{value:?}");
    assert!(shown.contains(fact), "{what}: `{fact}` is gone:\n{shown}");
}

// ------------------------------------------------------- the owner's yes

/// The owner: yes to every dialog but an action's, which it records and
/// declines (so nothing is written).
#[derive(Default)]
struct Owner {
    last_action: Mutex<Option<ActionConfirmation>>,
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        *self.last_action.lock().unwrap() = Some(request.clone());
        false
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        true
    }
}

// ---------------------------------------------------- the Files door flow

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const LABEL: &str = "msn-000001/1/1";

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

/// A private temporary tree: `home/<workspace>` is the registered workspace.
struct Dirs {
    base: PathBuf,
    workspace: PathBuf,
}

impl Dirs {
    fn new(workspace_name: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "nexus-debug-privacy-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        let workspace = base.join("home").join(workspace_name);
        for dir in [workspace.clone(), base.join("ident"), base.join("reserved")] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let base = std::fs::canonicalize(&base).unwrap();
        let workspace = base.join("home").join(workspace_name);
        Self { base, workspace }
    }

    fn path(&self, sub: &str) -> PathBuf {
        self.base.join(sub)
    }

    fn exclusions(&self) -> Exclusions {
        Exclusions {
            home: self.path("home"),
            identity_home: self.path("ident"),
            reserved: vec![self.path("reserved"), self.path("ident/.nexus")],
            uid: crate::launcher::effective_uid(),
        }
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// An agent's request to create one file, proposed through the door the
/// way production does it: the commitment waits for the owner's approval.
struct FilesFlow {
    gov: Arc<GovernedControl>,
    owner: Arc<Owner>,
    agent: AgentId,
    run: RunId,
    workspace: WorkspaceInfo,
    outcome: AgentOutcome,
    view: CommitmentView,
    name: String,
    content: String,
    grant: GrantId,
    _door: AgentDoor,
    _dirs: Dirs,
    _root: TempRoot,
}

fn files_flow() -> FilesFlow {
    let root = temp_root("debug-privacy");
    let evidence = Arc::new(MemoryEvidence::new(100_000));
    let gov = Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            evidence,
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    );
    let dirs = Dirs::new(&canary("workspace_name"));
    let workspace = gov
        .register_workspace_in(dirs.workspace(), &dirs.exclusions())
        .expect("a workspace in $HOME");
    let owner = Arc::new(Owner::default());
    let agent = AgentId::new("mother").unwrap();
    let door = gov.issue_agent_door(agent.clone(), owner.clone()).unwrap();
    gov.request_grant(
        &GrantRequest::Files {
            workspace: workspace.id.clone(),
        },
        Duration::from_secs(600),
        &*owner,
    )
    .expect("the owner grants Files on the workspace");
    door.confirm_mission(&MissionConfirmation {
        contract_hash: HASH.into(),
        request: LABEL.into(),
        summary: vec!["A one-page site".into()],
        page_facts: None,
    })
    .expect("the owner consents");
    let run = door.open_run().unwrap();
    door.admit_files(run, HASH, LABEL).unwrap();
    let name = format!("{}.txt", ascii("file_name"));
    let content = format!("{}\n", canary("file_content"));
    let outcome = door
        .agent_action(
            run,
            &Intent::File(FileCreate {
                name: name.clone(),
                content: content.clone(),
            }),
        )
        .expect("the file action is proposed");
    let AgentOutcome::AwaitingApproval(view) = &outcome else {
        panic!("a file action waits for the owner: {outcome:?}");
    };
    let view = view.clone();
    let grant = gov.grants().first().expect("the Files grant").id;
    FilesFlow {
        gov,
        owner,
        agent,
        run,
        workspace,
        outcome,
        view,
        name,
        content,
        grant,
        _door: door,
        _dirs: dirs,
        _root: root,
    }
}

impl FilesFlow {
    /// The action as the broker held it: the production kind, class,
    /// operation, target display and summary.
    fn prepared(&self) -> PreparedAction {
        PreparedAction {
            kind: self.view.kind,
            class: self.view.class,
            operation: self.view.operation,
            target: TargetIdentity {
                display: self.view.target.clone(),
                digest: Digest::of("test.target", &[self.view.target.as_bytes()]),
            },
            parameters: Digest::of("test.parameters", &[b"p"]),
            grants: vec![self.grant],
            leases: vec![],
            summary: self.view.summary.clone(),
        }
    }

    /// The owner's dialog for the commitment, as it was shown.
    fn dialog(&self) -> ActionConfirmation {
        let _ = self
            .gov
            .authorize(self.view.id, &self.agent, self.run, &*self.owner);
        self.owner
            .last_action
            .lock()
            .unwrap()
            .clone()
            .expect("the owner was asked")
    }
}

#[test]
fn a_prepared_file_action_prints_no_file_content() {
    let flow = files_flow();
    let prepared = flow.prepared();
    // The real summary carries the file, in full.
    assert_surface(
        "the prepared file action's summary",
        &prepared.summary.join("\n"),
        &canary("file_content"),
    );
    assert_surface(
        "the prepared file action's target",
        &prepared.target.display,
        &flow.name,
    );
    assert_content_free("PreparedAction", &prepared, &["file_content", "file_name"]);
    assert_keeps("PreparedAction", &prepared, "create");
}

#[test]
fn a_commitment_view_of_a_file_action_prints_no_file_content() {
    let flow = files_flow();
    assert_surface(
        "the commitment view's summary",
        &flow.view.summary.join("\n"),
        &flow.content.trim_end().to_string(),
    );
    assert_surface(
        "the commitment view's target",
        &flow.view.target,
        &flow.name,
    );
    assert_content_free(
        "CommitmentView",
        &flow.view,
        &["file_content", "file_name", "workspace_name"],
    );
    assert_keeps("CommitmentView", &flow.view, &format!("{:?}", flow.view.id));
    // The owner's dialog for the same commitment still shows the whole
    // file, its name and the workspace.
    let dialog = flow.dialog();
    assert_surface(
        "the owner's dialog",
        &dialog.message(),
        &canary("file_content"),
    );
    assert_surface("the owner's dialog", &dialog.message(), &flow.name);
    assert_surface("the owner's dialog", &dialog.message(), &flow.workspace.id);
}

#[test]
fn the_outcome_of_a_file_action_prints_no_file_content() {
    let flow = files_flow();
    assert_content_free(
        "AgentOutcome::AwaitingApproval",
        &flow.outcome,
        &["file_content", "file_name", "workspace_name"],
    );
}

#[test]
fn the_workspace_info_prints_no_workspace_name() {
    let flow = files_flow();
    assert_surface(
        "the workspace's name",
        &flow.workspace.name,
        &canary("workspace_name"),
    );
    assert_content_free("WorkspaceInfo", &flow.workspace, &["workspace_name"]);
    assert_keeps("WorkspaceInfo", &flow.workspace, &flow.workspace.id);
    let json = serde_json::to_string(&flow.workspace).unwrap();
    assert_surface("the workspace's JSON", &json, &canary("workspace_name"));
}

// ------------------------------------------------- the pipeline over a store

struct CanaryEffect {
    text: String,
    bytes: Vec<u8>,
    meta: Vec<(String, String)>,
}

fn effect_target() -> Digest {
    Digest::of("test.target", &[b"here"])
}

fn effect_parameters() -> Digest {
    Digest::of("test.parameters", &[b"p"])
}

impl PendingEffect for CanaryEffect {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(effect_target())
    }

    fn parameters(&self) -> Digest {
        effect_parameters()
    }

    fn execute(
        self: Box<Self>,
        _guard: &crate::authority::commitment::ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        Ok(EffectOutput {
            text: Some(self.text),
            bytes: Some(self.bytes),
            meta: self.meta,
            evidence: None,
        })
    }
}

/// One owner command run through the pipeline over a real evidence store.
struct Piped {
    _scratch: Scratch,
    store: Arc<PrimaryEvidence>,
    recent: Arc<MemoryEvidence>,
    output: EffectOutput,
    view: CommitmentView,
}

fn piped() -> Piped {
    let scratch = Scratch::new();
    let recent = Arc::new(MemoryEvidence::new(10_000));
    let store = PrimaryEvidence::open(&scratch.store(), vec![recent.clone()]).unwrap();
    let control = Arc::new(Control::new(Authority::new(
        store.clone(),
        Arc::new(ManualClock::default()),
    )));
    let (agent, run) = control
        .authority()
        .open_owner_run(vec!["text".into()])
        .unwrap();
    let grant = control
        .authority()
        .grants()
        .request(
            GrantScope::Perception {
                display: "agent".into(),
            },
            Duration::from_secs(600),
            &Yes::new(true),
        )
        .unwrap();
    let preparation = Preparation {
        action: PreparedAction {
            kind: CapabilityKind::Perception,
            class: EffectClass::R1,
            operation: "fake.observe",
            target: TargetIdentity {
                display: canary("target"),
                digest: effect_target(),
            },
            parameters: effect_parameters(),
            grants: vec![grant],
            leases: vec![],
            summary: vec![canary("summary")],
        },
        effect: Box::new(CanaryEffect {
            text: canary("output_text"),
            bytes: canary("output_bytes").into_bytes(),
            meta: vec![("note".into(), canary("output_meta"))],
        }),
        ttl: Duration::from_secs(60),
    };
    let view = control.propose(&agent, run, preparation).unwrap();
    control
        .authorize(view.id, &agent, run, &Yes::new(true))
        .unwrap();
    let output = control.execute(view.id, &agent, run).unwrap();
    Piped {
        _scratch: scratch,
        store,
        recent,
        output,
        view,
    }
}

#[test]
fn effect_output_prints_no_text_bytes_or_metadata() {
    let piped = piped();
    let output = &piped.output;
    assert_eq!(output.text.as_deref(), Some(canary("output_text").as_str()));
    assert_eq!(
        output.bytes.as_deref(),
        Some(canary("output_bytes").as_bytes())
    );
    assert_eq!(
        output.meta.first().map(|(_, v)| v.as_str()),
        Some(canary("output_meta").as_str())
    );
    assert_content_free(
        "EffectOutput",
        output,
        &["output_text", "output_bytes", "output_meta"],
    );
    assert_keeps("EffectOutput", output, "evidence");
}

#[test]
fn a_commitment_view_of_an_owner_command_prints_no_target_or_summary() {
    let piped = piped();
    assert_eq!(piped.view.target, canary("target"));
    assert_eq!(piped.view.summary, vec![canary("summary")]);
    assert_content_free("CommitmentView", &piped.view, &["target", "summary"]);
}

#[test]
fn an_evidence_record_prints_no_target_or_detail() {
    let piped = piped();
    let records = piped.recent.records();
    let prepared: &EvidenceRecord = records
        .iter()
        .find(|r| r.phase == EvidencePhase::Prepared)
        .expect("a prepared record");
    let finished: &EvidenceRecord = records
        .iter()
        .find(|r| r.phase == EvidencePhase::Finished)
        .expect("a finished record");
    // The owner's evidence keeps its target and its detail...
    assert_eq!(prepared.target.as_deref(), Some(canary("target").as_str()));
    assert!(
        finished
            .detail
            .iter()
            .any(|(_, value)| value == &canary("output_meta")),
        "the finished record keeps the metadata: {:?}",
        finished.to_json()
    );
    assert_surface(
        "the prepared record's JSON",
        &prepared.to_json().to_string(),
        &canary("target"),
    );
    assert_surface(
        "the finished record's JSON",
        &finished.to_json().to_string(),
        &canary("output_meta"),
    );
    // ...but its diagnostic does not.
    for record in &records {
        assert_content_free("EvidenceRecord", record, &["target", "output_meta"]);
    }
    assert_keeps("EvidenceRecord", prepared, "Prepared");
}

#[test]
fn a_stored_evidence_row_prints_no_body() {
    let piped = piped();
    let rows: Vec<Row> = piped.store.store().rows();
    assert!(
        rows.iter().any(|row| row.body.contains(&canary("target"))),
        "the stored body keeps the owner's target"
    );
    for row in &rows {
        assert_content_free("Row", row, &["target", "output_meta"]);
    }
    let prepared = rows
        .iter()
        .find(|row| row.phase == "prepared")
        .expect("a prepared row");
    assert_keeps("Row", prepared, "prepared");
}

#[test]
fn an_outcome_prints_no_metadata_or_failure_detail() {
    let succeeded = Outcome::Succeeded {
        meta: vec![("note".into(), canary("meta"))],
    };
    let failed = Outcome::Failed {
        class: FailureClass::Actuator,
        detail: canary("failure_detail"),
    };
    let cancelled = Outcome::Cancelled {
        detail: Some(canary("cancel_detail")),
    };
    assert_content_free("Outcome::Succeeded", &succeeded, &["meta"]);
    assert_content_free("Outcome::Failed", &failed, &["failure_detail"]);
    assert_content_free("Outcome::Cancelled", &cancelled, &["cancel_detail"]);
    assert_keeps("Outcome::Failed", &failed, "Actuator");
    // They still compare and carry their values.
    assert_ne!(succeeded, Outcome::Succeeded { meta: vec![] });
    let Outcome::Failed { detail, .. } = &failed else {
        unreachable!()
    };
    assert_eq!(detail, &canary("failure_detail"));
}

#[test]
fn a_target_identity_prints_no_display_line() {
    let target = TargetIdentity {
        display: canary("display"),
        digest: Digest::of("test.target", &[b"t"]),
    };
    assert_surface(
        "the target's display line",
        &target.display,
        &canary("display"),
    );
    assert_content_free("TargetIdentity", &target, &["display"]);
    assert_keeps("TargetIdentity", &target, "Digest(");
}

// ------------------------------------------------------------ tools

#[test]
fn a_tool_invocation_prints_no_args_files_or_summary() {
    // The production builders, over text carrying the canary.
    let tools = tool::catalog::production();
    for key in ["text.sha256", "speech.synthesize"] {
        let definition = tools.iter().find(|d| d.key == key).unwrap();
        let invocation = (definition.build)(&json!({ "text": canary("tool_text") })).unwrap();
        assert_eq!(
            invocation.files[0].1,
            canary("tool_text").into_bytes(),
            "{key} keeps the text for the launch"
        );
        assert_content_free(key, &invocation, &["tool_text"]);
    }
    // And one carrying a canary in every field.
    let invocation = ToolInvocation {
        args: vec![canary("tool_arg")],
        files: vec![("input.txt", canary("tool_file").into_bytes())],
        summary: vec![canary("tool_summary")],
    };
    assert_content_free(
        "ToolInvocation",
        &invocation,
        &["tool_arg", "tool_file", "tool_summary"],
    );
    assert_eq!(invocation.args[0], canary("tool_arg"));
}

#[test]
fn a_tool_intent_prints_no_tool_name_or_input() {
    let intent: ToolIntent = serde_json::from_value(json!({
        "tool": canary("tool_name"),
        "input": { "text": canary("tool_input"), "list": [canary("tool_item")] },
    }))
    .unwrap();
    assert_content_free(
        "ToolIntent",
        &intent,
        &["tool_name", "tool_input", "tool_item"],
    );
    // Its data surface, the JSON a planner or a form writes, is whole.
    let json = serde_json::to_string(&intent).unwrap();
    assert_surface("the tool intent's JSON", &json, &canary("tool_input"));
    assert_surface("the tool intent's JSON", &json, &canary("tool_name"));
    // Free-form input of every kind.
    for input in [
        Value::Null,
        json!(true),
        json!(7),
        json!(canary("tool_input")),
        json!([canary("tool_input")]),
        {
            let mut map = serde_json::Map::new();
            map.insert(canary("tool_key"), json!(canary("tool_input")));
            Value::Object(map)
        },
    ] {
        let intent = ToolIntent {
            tool: "text.sha256".into(),
            input,
        };
        assert_content_free("ToolIntent", &intent, &["tool_input", "tool_key"]);
    }
}

// -------------------------------------------------------- connectors

#[test]
fn an_operation_request_prints_no_path_body_or_summary() {
    let connectors = connector::catalog::production();
    let build = |id: &str, input: Value| -> OperationRequest {
        let operation = connectors
            .iter()
            .flat_map(|c| c.operations.iter())
            .find(|o| o.id == id)
            .unwrap_or_else(|| panic!("no operation {id}"));
        (operation.build)(&input).unwrap()
    };
    // A send: the body and the summary carry the message.
    let send = build(
        "outlook.messages.send",
        json!({
            "to": "friend@example.com",
            "subject": canary("subject"),
            "body": canary("message"),
        }),
    );
    assert_surface(
        "the send request's summary",
        &send.summary.join("\n"),
        &canary("message"),
    );
    assert_surface(
        "the send request's body",
        send.body.as_deref().unwrap(),
        &canary("message"),
    );
    assert_content_free("OperationRequest (send)", &send, &["message", "subject"]);
    // A search: the path and the summary carry the query.
    let search = build(
        "outlook.messages.search",
        json!({ "query": canary("query") }),
    );
    assert_surface(
        "the search request's summary",
        &search.summary.join("\n"),
        &canary("query"),
    );
    assert!(
        search.path.contains("%E9%9D%A2"),
        "the search request's path keeps the query: {}",
        search.path
    );
    assert_content_free("OperationRequest (search)", &search, &["query"]);
    assert_keeps("OperationRequest", &send, "application/json");
    // And one with a canary in every field.
    let request = OperationRequest {
        path: format!("/{}", canary("path")),
        body: Some(canary("body")),
        content_type: Some("text/plain"),
        summary: vec![canary("summary")],
    };
    assert_content_free("OperationRequest", &request, &["path", "body", "summary"]);
}

#[test]
fn a_destination_identity_prints_no_lines_or_display() {
    let connectors = connector::catalog::production();
    let slack = connectors.iter().find(|c| c.id == "slack").unwrap();
    let post = slack
        .operations
        .iter()
        .find(|o| o.id == "slack.chat.post")
        .unwrap();
    let resolver = post.destination.as_ref().unwrap();
    let answer = |body: Value| EffectOutput {
        text: Some(body.to_string()),
        meta: vec![("status".into(), "200".into())],
        ..Default::default()
    };
    let team = answer(json!({ "ok": true, "team_id": "T0123", "team": canary("team_name") }));
    let conversation = answer(json!({
        "ok": true,
        "channel": {
            "id": "C0123",
            "name": canary("channel_name"),
            "is_channel": true,
        },
    }));
    let identity = (resolver.identify)(&json!({ "channel": "C0123" }), &[team, conversation])
        .expect("the production resolver identifies the destination");
    assert_surface(
        "the destination's lines",
        &identity.lines.join("\n"),
        &canary("team_name"),
    );
    assert_surface(
        "the destination's lines",
        &identity.lines.join("\n"),
        &canary("channel_name"),
    );
    assert_surface(
        "the destination's display",
        &identity.display,
        &canary("channel_name"),
    );
    assert_content_free(
        "DestinationIdentity",
        &identity,
        &["team_name", "channel_name"],
    );
    let literal = DestinationIdentity {
        lines: vec![canary("line")],
        display: canary("display"),
    };
    assert_content_free("DestinationIdentity", &literal, &["line", "display"]);
}

#[test]
fn a_connector_intent_prints_no_operation_or_input() {
    let intent: ConnectorIntent = serde_json::from_value(json!({
        "operation": canary("operation"),
        "input": { "body": canary("input"), "list": [canary("item")] },
    }))
    .unwrap();
    assert_content_free("ConnectorIntent", &intent, &["operation", "input", "item"]);
    let json = serde_json::to_string(&intent).unwrap();
    assert_surface("the connector intent's JSON", &json, &canary("input"));
    // The command grammar builds one too.
    let command = format!(
        "connector gmail.messages.list {}",
        json!({ "q": canary("input") })
    );
    let Understood::Intent(Intent::Connector(built)) = understood(&command) else {
        panic!("a connector command");
    };
    assert_content_free("ConnectorIntent (command)", &built, &["input"]);
}

// ------------------------------------------------------- intents

fn understood(command: &str) -> Understood {
    understand(
        &CommandEnvelope {
            text: Some(command.into()),
            ..Default::default()
        },
        &Attachments::default(),
    )
    .unwrap()
}

fn governed(action: PlannedAction) -> Intent {
    match classify(&action) {
        Disposition::Governed(intent) => intent,
        other => panic!("not governed: {other:?}"),
    }
}

/// Every content-carrying intent, built by the planner the way an agent's
/// action becomes one.
fn planned_intents() -> Vec<(&'static str, Intent)> {
    let headers = std::collections::HashMap::from([("accept".to_string(), canary("header"))]);
    vec![
        (
            "request",
            governed(PlannedAction::ApiCall {
                method: "POST".into(),
                url: format!(
                    "https://example.com/{}?q={}",
                    canary("url_path"),
                    canary("url_query")
                ),
                body: Some(canary("request_body")),
                headers: Some(headers),
            }),
        ),
        (
            "browse",
            governed(PlannedAction::BrowserAutomate {
                start_url: format!("https://example.com/{}", canary("start_url")),
                actions: vec![
                    BrowserAction::Navigate {
                        url: canary("navigate_url"),
                    },
                    BrowserAction::Click {
                        selector: canary("click_selector"),
                    },
                    BrowserAction::Fill {
                        selector: canary("fill_selector"),
                        text: canary("fill_text"),
                    },
                    BrowserAction::Press {
                        selector: canary("press_selector"),
                        key: canary("press_key"),
                    },
                    BrowserAction::WaitFor {
                        selector: Some(canary("wait_selector")),
                        timeout_ms: Some(1000),
                    },
                    BrowserAction::ExtractText {
                        selector: canary("extract_selector"),
                    },
                ],
                screenshot_dir: None,
            }),
        ),
        (
            "observe",
            governed(PlannedAction::CaptureWindow {
                window_title: canary("window_title"),
            }),
        ),
        (
            "input (type)",
            governed(PlannedAction::KeyboardType {
                text: canary("typed_text"),
            }),
        ),
        (
            "input (press)",
            governed(PlannedAction::KeyboardPress {
                key: canary("pressed_key"),
            }),
        ),
        (
            "input (shortcut)",
            governed(PlannedAction::KeyboardShortcut {
                keys: vec![canary("shortcut_a"), canary("shortcut_b")],
            }),
        ),
        (
            "file",
            Intent::File(FileCreate {
                name: canary("file_name"),
                content: canary("file_content"),
            }),
        ),
    ]
}

const INTENT_FIELDS: &[&str] = &[
    "url_path",
    "url_query",
    "header",
    "request_body",
    "start_url",
    "navigate_url",
    "click_selector",
    "fill_selector",
    "fill_text",
    "press_selector",
    "press_key",
    "wait_selector",
    "extract_selector",
    "window_title",
    "typed_text",
    "pressed_key",
    "shortcut_a",
    "shortcut_b",
    "file_name",
    "file_content",
];

#[test]
fn an_intent_prints_no_content_in_any_variant() {
    let mut intents = planned_intents();
    // The command grammar builds the tool and connector intents.
    for (label, command) in [
        ("tool (hash)", format!("hash {}", canary("hash_text"))),
        ("tool (say)", format!("say {}", canary("say_text"))),
        (
            "connector",
            format!(
                "connector gmail.messages.list {}",
                json!({ "q": canary("connector_input") })
            ),
        ),
        ("fetch", "fetch https://example.com/page".to_string()),
    ] {
        let Understood::Intent(intent) = understood(&command) else {
            panic!("{label} is understood");
        };
        intents.push((label, intent));
    }
    let mut fields: Vec<&str> = INTENT_FIELDS.to_vec();
    fields.extend(["hash_text", "say_text", "connector_input"]);
    for (label, intent) in &intents {
        assert_content_free(&format!("Intent ({label})"), intent, &fields);
        // The intent is still whole data: its JSON carries the canary.
        if *label != "fetch" {
            let json = serde_json::to_string(intent).unwrap();
            assert_surface(&format!("the {label} intent's JSON"), &json, "NXCANARY");
        }
    }
    // The synthetic probe intent has nothing to hide.
    assert_keeps(
        "Intent::Synthetic",
        &Intent::Synthetic(crate::synthetic::SyntheticIntent {}),
        "Synthetic",
    );
}

#[test]
fn a_command_envelope_prints_no_text_transcript_or_intent() {
    let envelope: CommandEnvelope = serde_json::from_value(json!({
        "text": canary("command_text"),
        "voice_transcript": canary("transcript"),
        "attachments": ["att-0011223344556677"],
        "intent": {
            "domain": "tool",
            "intent": { "tool": "text.sha256", "input": { "text": canary("envelope_input") } },
        },
    }))
    .unwrap();
    assert_content_free(
        "CommandEnvelope",
        &envelope,
        &["command_text", "transcript", "envelope_input"],
    );
    assert_keeps("CommandEnvelope", &envelope, "Tool");
    let json = serde_json::to_string(&envelope).unwrap();
    assert_surface("the envelope's JSON", &json, &canary("command_text"));
    assert_surface("the envelope's JSON", &json, &canary("transcript"));
    assert_surface("the envelope's JSON", &json, &canary("envelope_input"));
    assert_eq!(envelope.modalities().len(), 4);
}

#[test]
fn an_understood_command_prints_no_content() {
    let command = understood(&format!("hash {}", canary("hash_text")));
    assert_content_free("Understood", &command, &["hash_text"]);
    let Understood::Intent(intent) = &command else {
        panic!("understood");
    };
    assert_surface(
        "the understood intent's JSON",
        &serde_json::to_string(intent).unwrap(),
        &canary("hash_text"),
    );
}

#[test]
fn a_planned_disposition_prints_no_content() {
    for (label, intent) in planned_intents() {
        let disposition = Disposition::Governed(intent);
        assert_content_free(
            &format!("Disposition ({label})"),
            &disposition,
            INTENT_FIELDS,
        );
    }
}

#[test]
fn an_agent_outcome_prints_no_output_or_commitment_content() {
    let piped = piped();
    let done = AgentOutcome::Done(piped.output.clone());
    assert_content_free(
        "AgentOutcome::Done",
        &done,
        &["output_text", "output_bytes", "output_meta"],
    );
    let waiting = AgentOutcome::AwaitingApproval(piped.view.clone());
    assert_content_free(
        "AgentOutcome::AwaitingApproval",
        &waiting,
        &["target", "summary"],
    );
}

#[test]
fn an_attachment_prints_no_name_or_bytes() {
    let attachments = Attachments::default();
    let attachment: Attachment = attachments
        .import(
            &canary("attachment_name"),
            canary("attachment_bytes").into_bytes(),
        )
        .unwrap();
    assert_surface(
        "the attachment's name",
        &attachment.name,
        &canary("attachment_name"),
    );
    assert_eq!(attachment.bytes, canary("attachment_bytes").into_bytes());
    assert_content_free(
        "Attachment",
        &attachment,
        &["attachment_name", "attachment_bytes"],
    );
    assert_keeps("Attachment", &attachment, &attachment.id);
}

// ------------------------------------------------------------ egress

#[test]
fn an_egress_intent_prints_no_url_header_or_body() {
    let Intent::Request(intent) = planned_intents().remove(0).1 else {
        panic!("a request intent");
    };
    assert_surface("the request's URL", &intent.url, &canary("url_path"));
    assert_surface(
        "the request's body",
        intent.body.as_deref().unwrap(),
        &canary("request_body"),
    );
    assert_surface(
        "the request's header",
        &intent.headers[0].1,
        &canary("header"),
    );
    assert_content_free(
        "EgressIntent",
        &intent,
        &["url_path", "url_query", "header", "request_body"],
    );
    let literal = EgressIntent {
        method: canary("method"),
        url: canary("url"),
        headers: vec![(canary("header_name"), canary("header_value"))],
        body: Some(canary("body")),
    };
    assert_content_free(
        "EgressIntent",
        &literal,
        &["method", "url", "header_name", "header_value", "body"],
    );
}

#[test]
fn a_destination_prints_no_path_or_query() {
    let destination = Destination::parse(&format!(
        "https://example.com/{}?q={}",
        canary("path"),
        canary("query")
    ))
    .unwrap();
    assert_surface(
        "the destination's URL",
        destination.url().as_str(),
        "NXCANARY-path-",
    );
    assert_surface(
        "the destination's URL",
        destination.url().as_str(),
        "NXCANARY-query-",
    );
    assert_content_free("Destination", &destination, &["path", "query"]);
    // The host, scheme and port are the policy's identity, not content.
    assert_keeps("Destination", &destination, "example.com");
    assert_keeps("Destination", &destination, "443");
}

#[test]
fn a_pinned_request_prints_no_url_header_or_body() {
    let request = PinnedRequest {
        method: Method::Post,
        url: url::Url::parse(&format!(
            "https://example.com/{}?q={}",
            canary("path"),
            canary("query")
        ))
        .unwrap(),
        domain: Some("example.com".into()),
        addresses: vec!["93.184.216.34:443".parse().unwrap()],
        headers: vec![(
            reqwest::header::HeaderName::from_static("x-note"),
            reqwest::header::HeaderValue::from_bytes(canary("header").as_bytes()).unwrap(),
        )],
        body: Some(canary("body").into_bytes()),
        secret: None,
        timeout: Duration::from_secs(5),
        max_body: 1024,
    };
    assert_content_free(
        "PinnedRequest",
        &request,
        &["path", "query", "header", "body"],
    );
    assert_keeps("PinnedRequest", &request, "Post");
    assert_keeps("PinnedRequest", &request, "example.com");
}

#[test]
fn an_http_response_prints_no_body_or_location() {
    let response = HttpResponse {
        status: 200,
        peer: Some("93.184.216.34:443".parse().unwrap()),
        content_type: Some(canary("content_type")),
        location: Some(format!("https://example.com/{}", canary("location"))),
        body: canary("response_body").into_bytes(),
        redacted: false,
    };
    assert_content_free(
        "HttpResponse",
        &response,
        &["content_type", "location", "response_body"],
    );
    assert_keeps("HttpResponse", &response, "200");
}

// ----------------------------------------------------------- browser

#[test]
fn a_browser_step_and_intent_print_no_url_selector_or_text() {
    let Intent::Browse(intent) = planned_intents().remove(1).1 else {
        panic!("a browse intent");
    };
    assert_eq!(intent.steps.len(), 6);
    assert_content_free(
        "BrowserIntent",
        &intent,
        &[
            "start_url",
            "navigate_url",
            "click_selector",
            "fill_selector",
            "fill_text",
            "press_selector",
            "press_key",
            "wait_selector",
            "extract_selector",
        ],
    );
    for step in &intent.steps {
        assert_content_free(
            "BrowserStep",
            step,
            &[
                "navigate_url",
                "click_selector",
                "fill_selector",
                "fill_text",
                "press_selector",
                "press_key",
                "wait_selector",
                "extract_selector",
            ],
        );
    }
    // A step keeps its kind and its numbers.
    assert_keeps("BrowserStep", &intent.steps[4], "WaitFor");
    assert_keeps("BrowserStep", &intent.steps[4], "1000");
    let json = serde_json::to_string(&intent).unwrap();
    assert_surface("the browser intent's JSON", &json, &canary("fill_text"));
    assert_surface("the browser intent's JSON", &json, &canary("start_url"));
    let literal = BrowserIntent {
        start_url: canary("literal_url"),
        steps: vec![BrowserStep::Fill {
            selector: canary("literal_selector"),
            text: canary("literal_text"),
        }],
    };
    assert_content_free(
        "BrowserIntent",
        &literal,
        &["literal_url", "literal_selector", "literal_text"],
    );
}

// ----------------------------------------------------------- display

#[test]
fn a_window_info_prints_no_title() {
    let window = WindowInfo {
        id: 41,
        title: canary("window_title"),
        rect: crate::display::Rect {
            x: 1,
            y: 2,
            width: 300,
            height: 200,
        },
    };
    assert_surface("the window's title", &window.title, &canary("window_title"));
    assert_content_free("WindowInfo", &window, &["window_title"]);
    assert_keeps("WindowInfo", &window, "41");
}

#[test]
fn a_perception_intent_prints_no_window_title() {
    let Intent::Observe(intent) = planned_intents().remove(2).1 else {
        panic!("an observe intent");
    };
    assert_content_free("PerceptionIntent", &intent, &["window_title"]);
    assert_surface(
        "the perception intent's JSON",
        &serde_json::to_string(&intent).unwrap(),
        &canary("window_title"),
    );
    let screen = PerceptionIntent::Screen { region: None };
    assert_keeps("PerceptionIntent", &screen, "Screen");
}

#[test]
fn an_input_intent_prints_no_typed_text_key_or_shortcut() {
    for (index, field) in [(3, "typed_text"), (4, "pressed_key"), (5, "shortcut_a")] {
        let Intent::Input(intent) = planned_intents().remove(index).1 else {
            panic!("an input intent");
        };
        assert_content_free(
            "InputIntent",
            &intent,
            &["typed_text", "pressed_key", "shortcut_a", "shortcut_b"],
        );
        assert_surface(
            "the input intent's JSON",
            &serde_json::to_string(&intent).unwrap(),
            &canary(field),
        );
    }
    // Coordinates are not content.
    let click: InputIntent =
        serde_json::from_value(json!({ "kind": "click", "x": 12, "y": 34 })).unwrap();
    assert_keeps("InputIntent", &click, "12");
    assert_keeps("InputIntent", &click, "34");
}
