use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, RecvError, Sender, TryRecvError},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use mercurio_analysis::capability::{
    CapabilityRunReport, SemanticArtifact, SemanticDiagnostic, SemanticWorkspaceSnapshot,
};
use mercurio_authoring::authoring::AuthoringProject;
use mercurio_kir::{KirDocument, KirElement, KirError};
use mercurio_semantic_services::feasibility::{
    CoreMutationFeasibilityService, FeasibilityIssue, FeasibilityIssueKind, FeasibilityStatus,
    MutationContext, MutationFeasibilityService,
};
use mercurio_semantic_services::identity::workspace_revision_for_kir_document;
use mercurio_semantic_services::mutation::{
    ElementRef, ModelChangeEvent, ModelChangeProvenance, MutationEvidence, MutationProposal,
    SemanticDiff, SemanticMutation, WorkspaceRevision, diff_kir_documents,
};
use mercurio_semantic_services::semantic_validation::{
    SemanticValidationReport, validate_kir_semantics,
};
use mercurio_workspace::model_state::{
    InputSource, InputSourceKind, InputSourceSet, ModelBuildRecord, ModelRevision,
    ModelRevisionProducer, ModelState, ModelStateError,
};

const GENERATED_FILE_THRESHOLD: usize = 100;

#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    pub revision: WorkspaceRevision,
    pub kir: Arc<KirDocument>,
    pub validation_report: SemanticValidationReport,
    pub profile_id: Option<String>,
    source_project: Option<AuthoringProject>,
}

#[derive(Debug, Clone)]
pub struct ModelWorkspace {
    current: Arc<RwLock<Arc<WorkspaceSnapshot>>>,
    subscribers: Arc<RwLock<BTreeMap<u64, Sender<ModelChangeEvent>>>>,
    next_subscription: Arc<AtomicU64>,
    next_event_sequence: Arc<AtomicU64>,
    command_stack: Arc<RwLock<CommandStackState>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandStackState {
    #[serde(default)]
    undo: Vec<CommandRecord>,
    #[serde(default)]
    redo: Vec<CommandRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandRecord {
    mutation_id: String,
    before: KirDocument,
    after: KirDocument,
    before_profile_id: Option<String>,
    after_profile_id: Option<String>,
}

impl CommandStackState {
    fn push(&mut self, record: CommandRecord) {
        self.undo.push(record);
        self.redo.clear();
    }
}
#[derive(Debug)]
pub struct ModelChangeSubscription {
    receiver: Receiver<ModelChangeEvent>,
}

impl ModelChangeSubscription {
    pub fn recv(&self) -> Result<ModelChangeEvent, RecvError> {
        self.receiver.recv()
    }

    pub fn try_recv(&self) -> Result<ModelChangeEvent, TryRecvError> {
        self.receiver.try_recv()
    }
}

#[derive(Debug, Clone)]
pub struct ModelSession {
    snapshot: Arc<WorkspaceSnapshot>,
    workspace: Option<ModelWorkspace>,
}

#[derive(Debug, Clone)]
pub struct ModelFork {
    label: Option<String>,
    base: Arc<WorkspaceSnapshot>,
    workspace: Option<ModelWorkspace>,
    overlay: KirOverlay,
    operation_log: ForkOperationLog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellKind {
    Query,
    Action,
    View,
    Script,
    Capability,
    Analysis,
    Build,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellLanguage {
    MercurioDsl,
    Python,
    Rhai,
    Sysml,
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellRunStatus {
    Passed,
    Failed,
    Error,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellOutputKind {
    Table,
    Text,
    Json,
    Stdout,
    Stderr,
    CapabilityReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellOutput {
    pub id: String,
    pub kind: CellOutputKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRunRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_id: Option<String>,
    pub kind: CellKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<CellLanguage>,
    pub source: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRunReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub cell_id: String,
    pub kind: CellKind,
    pub status: CellRunStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<CellOutput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<SemanticArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<SemanticDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_report: Option<CapabilityRunReport>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct KirOverlay {
    pub added_elements: BTreeMap<String, KirElement>,
    pub updated_properties: BTreeMap<String, BTreeMap<String, Value>>,
    pub added_members: BTreeMap<String, Vec<String>>,
    pub removed_elements: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum ForkOperation {
    AddPackage {
        id: String,
        qualified_name: String,
        owner: Option<String>,
        source_file: String,
    },
    RenameDeclaration {
        element: ElementRef,
        new_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
struct ForkOperationLog {
    operations: Vec<ForkOperation>,
}

impl ForkOperationLog {
    fn push(&mut self, operation: ForkOperation) {
        self.operations.push(operation);
    }

    fn is_all_renames(&self) -> bool {
        !self.operations.is_empty()
            && self
                .operations
                .iter()
                .all(|op| matches!(op, ForkOperation::RenameDeclaration { .. }))
    }

    fn rename_operations(&self) -> impl Iterator<Item = (&ElementRef, &str)> {
        self.operations.iter().filter_map(|op| match op {
            ForkOperation::RenameDeclaration { element, new_name } => {
                Some((element, new_name.as_str()))
            }
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitMode {
    PreserveSource,
    RewriteSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkElement {
    pub id: String,
    pub qualified_name: String,
}

impl ForkElement {
    pub fn as_element_ref(&self) -> mercurio_semantic_services::mutation::ElementRef {
        mercurio_semantic_services::mutation::ElementRef::new(self.qualified_name.clone())
    }
}

impl From<ForkElement> for mercurio_semantic_services::mutation::ElementRef {
    fn from(fork: ForkElement) -> Self {
        Self::new(fork.qualified_name)
    }
}

impl From<&ForkElement> for mercurio_semantic_services::mutation::ElementRef {
    fn from(fork: &ForkElement) -> Self {
        Self::new(fork.qualified_name.clone())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForkElementSpec {
    pub id_prefix: String,
    pub kind: String,
    pub name: String,
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommitResult {
    pub mode: CommitMode,
    pub strategy_used: CommitStrategy,
    pub base_revision: WorkspaceRevision,
    pub new_revision: WorkspaceRevision,
    pub changed_files: BTreeSet<String>,
    pub edited_files: BTreeMap<String, String>,
    pub semantic_diff: SemanticDiff,
    pub generated_elements: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitStrategy {
    MutatorPlan,
    GeneratedCompanionFiles,
    RewriteGeneratedSource,
    NoOp,
}

#[derive(Debug)]
pub enum SessionError {
    StaleWorkspace {
        base_revision: WorkspaceRevision,
        current_revision: WorkspaceRevision,
    },
    DuplicateElement(String),
    MissingElement(String),
    InvalidInput(String),
    Unsupported(String),
    Kir(KirError),
    Authoring(mercurio_authoring::authoring::AuthoringError),
    Feasibility(FeasibilityIssue),
}

impl WorkspaceSnapshot {
    pub fn new(kir: KirDocument) -> Result<Self, SessionError> {
        Self::with_profile(kir, None)
    }

    pub fn with_profile(
        kir: KirDocument,
        profile_id: Option<String>,
    ) -> Result<Self, SessionError> {
        let validation_report = validate_kir_semantics(&kir)?;
        let revision = workspace_revision_for_kir_document(&kir)?;
        Ok(Self {
            revision,
            kir: Arc::new(kir),
            validation_report,
            profile_id,
            source_project: None,
        })
    }

    pub fn from_authoring_project(project: AuthoringProject) -> Result<Self, SessionError> {
        let context = MutationContext::from_project(project.clone());
        let kir = project.compile_kir_document()?;
        let validation_report = validate_kir_semantics(&kir)?;
        Ok(Self {
            revision: context.workspace_revision,
            kir: Arc::new(kir),
            validation_report,
            profile_id: None,
            source_project: Some(project),
        })
    }

    pub fn session(self: &Arc<Self>) -> ModelSession {
        ModelSession {
            snapshot: Arc::clone(self),
            workspace: None,
        }
    }

    pub fn source_project(&self) -> Option<&AuthoringProject> {
        self.source_project.as_ref()
    }

    pub fn model_revision(&self) -> Result<ModelRevision, ModelStateError> {
        ModelRevision::from_kir_document_with_profile(
            (*self.kir).clone(),
            self.profile_id.clone(),
            self.model_build_record(),
        )
    }

    pub fn semantic_workspace_snapshot(
        &self,
    ) -> Result<SemanticWorkspaceSnapshot, ModelStateError> {
        Ok(SemanticWorkspaceSnapshot::from_model_revision(
            &self.model_revision()?,
        ))
    }

    fn model_build_record(&self) -> ModelBuildRecord {
        let Some(project) = &self.source_project else {
            return ModelBuildRecord::new(ModelRevisionProducer::WorkspaceSnapshot);
        };

        let sources = project
            .files()
            .map(|(path, _)| InputSource::new(InputSourceKind::SourceFile, path.to_string()))
            .collect::<Vec<_>>();
        ModelBuildRecord::source_compile(InputSourceSet::new(
            Some("authoring project".to_string()),
            sources,
        ))
    }
}

impl ModelWorkspace {
    pub fn new(snapshot: WorkspaceSnapshot) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(snapshot))),
            subscribers: Arc::new(RwLock::new(BTreeMap::new())),
            next_subscription: Arc::new(AtomicU64::new(1)),
            next_event_sequence: Arc::new(AtomicU64::new(1)),
            command_stack: Arc::new(RwLock::new(CommandStackState::default())),
        }
    }

    pub fn current_snapshot(&self) -> Arc<WorkspaceSnapshot> {
        Arc::clone(&self.current.read().expect("workspace lock poisoned"))
    }

    pub fn session(&self) -> ModelSession {
        ModelSession {
            snapshot: self.current_snapshot(),
            workspace: Some(self.clone()),
        }
    }

    pub fn model_state(&self, label: Option<String>) -> Result<ModelState, ModelStateError> {
        Ok(ModelState::from_revision(
            label,
            self.current_snapshot().model_revision()?,
        ))
    }

    pub fn semantic_workspace_snapshot(
        &self,
    ) -> Result<SemanticWorkspaceSnapshot, ModelStateError> {
        self.current_snapshot().semantic_workspace_snapshot()
    }

    pub fn subscribe(&self) -> ModelChangeSubscription {
        let (sender, receiver) = mpsc::channel();
        let id = self.next_subscription.fetch_add(1, Ordering::Relaxed);
        self.subscribers
            .write()
            .expect("workspace subscriber lock poisoned")
            .insert(id, sender);
        ModelChangeSubscription { receiver }
    }

    fn publish_snapshot(&self, snapshot: WorkspaceSnapshot, event: ModelChangeEvent) {
        let current = self.current_snapshot();
        self.command_stack
            .write()
            .expect("workspace command stack lock poisoned")
            .push(CommandRecord {
                mutation_id: event.provenance.mutation_id.clone(),
                before: (*current.kir).clone(),
                after: (*snapshot.kir).clone(),
                before_profile_id: current.profile_id.clone(),
                after_profile_id: snapshot.profile_id.clone(),
            });
        self.publish_snapshot_untracked(snapshot, event);
    }

    fn publish_snapshot_untracked(&self, snapshot: WorkspaceSnapshot, mut event: ModelChangeEvent) {
        *self.current.write().expect("workspace lock poisoned") = Arc::new(snapshot);
        event.sequence = self.next_event_sequence.fetch_add(1, Ordering::SeqCst);
        self.subscribers
            .write()
            .expect("workspace subscriber lock poisoned")
            .retain(|_, sender| sender.send(event.clone()).is_ok());
    }

    /// Publishes a snapshot produced by an applied mutation: the transition is
    /// recorded on the command stack (so it is undoable via [`Self::undo`])
    /// and the change event is emitted to subscribers with the next sequence
    /// number.
    pub fn publish_applied(&self, snapshot: WorkspaceSnapshot, event: ModelChangeEvent) {
        self.publish_snapshot(snapshot, event);
    }

    /// Publishes a snapshot that replaces the current one without recording a
    /// command-stack entry — for out-of-band changes such as text edits or
    /// full workspace rebuilds. The current revision still advances, so a
    /// later [`Self::undo`] of an older command-stack record fails its
    /// staleness guard instead of silently clobbering the replaced state.
    pub fn publish_replaced(&self, snapshot: WorkspaceSnapshot, event: ModelChangeEvent) {
        self.publish_snapshot_untracked(snapshot, event);
    }

    /// Number of records currently on the undo stack.
    pub fn undo_depth(&self) -> usize {
        self.command_stack
            .read()
            .expect("workspace command stack lock poisoned")
            .undo
            .len()
    }

    /// Number of records currently on the redo stack.
    pub fn redo_depth(&self) -> usize {
        self.command_stack
            .read()
            .expect("workspace command stack lock poisoned")
            .redo
            .len()
    }

    pub fn can_undo(&self) -> bool {
        !self
            .command_stack
            .read()
            .expect("workspace command stack lock poisoned")
            .undo
            .is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self
            .command_stack
            .read()
            .expect("workspace command stack lock poisoned")
            .redo
            .is_empty()
    }

    pub fn command_stack_state(&self) -> CommandStackState {
        self.command_stack
            .read()
            .expect("workspace command stack lock poisoned")
            .clone()
    }

    pub fn restore_command_stack_state(&self, state: CommandStackState) {
        *self
            .command_stack
            .write()
            .expect("workspace command stack lock poisoned") = state;
    }

    pub fn undo(&self) -> Result<WorkspaceRevision, SessionError> {
        let record = {
            let mut stack = self
                .command_stack
                .write()
                .expect("workspace command stack lock poisoned");
            stack
                .undo
                .pop()
                .ok_or_else(|| SessionError::Unsupported("nothing to undo".to_string()))?
        };
        let current = self.current_snapshot();
        let expected = workspace_revision_for_kir_document(&record.after)?;
        if current.revision != expected {
            self.command_stack
                .write()
                .expect("workspace command stack lock poisoned")
                .redo
                .clear();
            return Err(SessionError::StaleWorkspace {
                base_revision: expected,
                current_revision: current.revision.clone(),
            });
        }
        let snapshot = WorkspaceSnapshot::with_profile(
            record.before.clone(),
            record.before_profile_id.clone(),
        )?;
        let revision = snapshot.revision.clone();
        let event = ModelChangeEvent::new(
            current.revision.clone(),
            revision.clone(),
            ModelChangeProvenance {
                mutation_id: format!("undo:{}", record.mutation_id),
                actor: None,
            },
            diff_kir_documents(&current.kir, &record.before),
        );
        self.command_stack
            .write()
            .expect("workspace command stack lock poisoned")
            .redo
            .push(record);
        self.publish_snapshot_untracked(snapshot, event);
        Ok(revision)
    }

    pub fn redo(&self) -> Result<WorkspaceRevision, SessionError> {
        let record = {
            let mut stack = self
                .command_stack
                .write()
                .expect("workspace command stack lock poisoned");
            stack
                .redo
                .pop()
                .ok_or_else(|| SessionError::Unsupported("nothing to redo".to_string()))?
        };
        let current = self.current_snapshot();
        let expected = workspace_revision_for_kir_document(&record.before)?;
        if current.revision != expected {
            return Err(SessionError::StaleWorkspace {
                base_revision: expected,
                current_revision: current.revision.clone(),
            });
        }
        let snapshot =
            WorkspaceSnapshot::with_profile(record.after.clone(), record.after_profile_id.clone())?;
        let revision = snapshot.revision.clone();
        let event = ModelChangeEvent::new(
            current.revision.clone(),
            revision.clone(),
            ModelChangeProvenance {
                mutation_id: format!("redo:{}", record.mutation_id),
                actor: None,
            },
            diff_kir_documents(&current.kir, &record.after),
        );
        self.command_stack
            .write()
            .expect("workspace command stack lock poisoned")
            .undo
            .push(record);
        self.publish_snapshot_untracked(snapshot, event);
        Ok(revision)
    }
}

impl ModelSession {
    pub fn snapshot(&self) -> Arc<WorkspaceSnapshot> {
        Arc::clone(&self.snapshot)
    }

    pub fn model_revision(&self) -> Result<ModelRevision, ModelStateError> {
        self.snapshot.model_revision()
    }

    pub fn semantic_workspace_snapshot(
        &self,
    ) -> Result<SemanticWorkspaceSnapshot, ModelStateError> {
        self.snapshot.semantic_workspace_snapshot()
    }

    pub fn fork(&self, label: impl Into<String>) -> ModelFork {
        ModelFork {
            label: Some(label.into()),
            base: Arc::clone(&self.snapshot),
            workspace: self.workspace.clone(),
            overlay: KirOverlay::default(),
            operation_log: ForkOperationLog::default(),
        }
    }
}

impl ModelFork {
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn base_revision(&self) -> &WorkspaceRevision {
        &self.base.revision
    }

    pub fn overlay(&self) -> &KirOverlay {
        &self.overlay
    }

    pub fn package(
        &mut self,
        qualified_name: impl Into<String>,
        owner: Option<&ForkElement>,
    ) -> Result<ForkElement, SessionError> {
        let qualified_name = normalize_qname(&qualified_name.into())?;
        let id = format!("pkg.{qualified_name}");
        self.ensure_not_present(&id)?;
        let source_file = generated_source_file_for(&qualified_name);
        let mut properties = BTreeMap::from([
            (
                "declared_name".to_string(),
                Value::String(qualified_name.clone()),
            ),
            ("metadata".to_string(), source_file_metadata(&source_file)),
        ]);
        if let Some(owner) = owner {
            properties.insert("owner".to_string(), Value::String(owner.id.clone()));
            self.patch_member(owner.id.as_str(), id.as_str());
        }
        let element = KirElement {
            id: id.clone(),
            kind: "Model::Package".to_string(),
            layer: 2,
            properties,
        };
        self.overlay.added_elements.insert(id.clone(), element);
        let operation = ForkOperation::AddPackage {
            id: id.clone(),
            qualified_name: qualified_name.clone(),
            owner: owner.map(|owner| owner.id.clone()),
            source_file,
        };
        self.operation_log.push(operation);
        Ok(ForkElement { id, qualified_name })
    }

    pub fn add_metadata(
        &mut self,
        owner: &ForkElement,
        metadata_type: impl Into<String>,
        properties: BTreeMap<String, String>,
    ) -> Result<ForkElement, SessionError> {
        let metadata_type = normalize_name(&metadata_type.into())?;
        let qualified_name = format!("{}.{}", owner.qualified_name, metadata_type);
        let id = format!("metadata.{qualified_name}");
        self.ensure_not_present(&id)?;
        let source_file = source_file_for_owner(self.overlay.added_elements.get(&owner.id))
            .unwrap_or_else(|| generated_source_file_for(&owner.qualified_name));
        let element = KirElement {
            id: id.clone(),
            kind: "Model::MetadataUsage".to_string(),
            layer: 2,
            properties: BTreeMap::from([
                (
                    "declared_name".to_string(),
                    Value::String(metadata_type.clone()),
                ),
                ("owner".to_string(), Value::String(owner.id.clone())),
                ("metadata".to_string(), source_file_metadata(&source_file)),
                (
                    "doc".to_string(),
                    Value::Object(
                        properties
                            .into_iter()
                            .map(|(key, value)| (key, Value::String(value)))
                            .collect(),
                    ),
                ),
            ]),
        };
        self.overlay.added_elements.insert(id.clone(), element);
        self.patch_member(owner.id.as_str(), id.as_str());
        Ok(ForkElement { id, qualified_name })
    }

    pub fn semantic_element(
        &mut self,
        owner: &ForkElement,
        spec: ForkElementSpec,
    ) -> Result<ForkElement, SessionError> {
        let name = normalize_name(&spec.name)?;
        let id_prefix = normalize_name(&spec.id_prefix)?;
        let qualified_name = format!("{}.{}", owner.qualified_name, name);
        let id = format!("{id_prefix}.{qualified_name}");
        self.ensure_not_present(&id)?;
        let source_file = source_file_for_owner(self.overlay.added_elements.get(&owner.id))
            .unwrap_or_else(|| generated_source_file_for(&owner.qualified_name));
        let mut properties = BTreeMap::from([
            ("declared_name".to_string(), Value::String(name)),
            ("owner".to_string(), Value::String(owner.id.clone())),
            ("metadata".to_string(), source_file_metadata(&source_file)),
        ]);
        properties.extend(spec.properties);
        let element = KirElement {
            id: id.clone(),
            kind: spec.kind,
            layer: 2,
            properties,
        };
        self.overlay.added_elements.insert(id.clone(), element);
        self.patch_member(owner.id.as_str(), id.as_str());
        Ok(ForkElement { id, qualified_name })
    }

    pub fn rename_declaration(
        &mut self,
        element: impl Into<String>,
        new_name: impl Into<String>,
    ) -> Result<(), SessionError> {
        let element = ElementRef::new(normalize_qname(&element.into())?);
        let new_name = normalize_name(&new_name.into())?;
        let element_id = self
            .base
            .kir
            .elements
            .iter()
            .find(|candidate| candidate.id.ends_with(&element.qualified_name))
            .map(|candidate| candidate.id.clone())
            .ok_or_else(|| SessionError::MissingElement(element.qualified_name.clone()))?;
        self.overlay
            .updated_properties
            .entry(element_id)
            .or_default()
            .insert("declared_name".to_string(), Value::String(new_name.clone()));
        let operation = ForkOperation::RenameDeclaration { element, new_name };
        self.operation_log.push(operation);
        Ok(())
    }

    pub fn materialize(&self) -> Result<KirDocument, SessionError> {
        self.overlay.materialize(&self.base.kir)
    }

    pub fn validate(&self) -> Result<(), SessionError> {
        self.materialize()?.validate()?;
        Ok(())
    }

    pub fn diff(&self) -> Result<SemanticDiff, SessionError> {
        Ok(diff_kir_documents(&self.base.kir, &self.materialize()?))
    }

    pub fn commit(&self, mode: CommitMode) -> Result<CommitResult, SessionError> {
        self.ensure_current_workspace_revision()?;
        match mode {
            CommitMode::PreserveSource => self.commit_preserve_source(),
            CommitMode::RewriteSource => self.commit_rewrite_source(),
        }
    }

    fn commit_preserve_source(&self) -> Result<CommitResult, SessionError> {
        if self.overlay.is_empty() {
            return self.noop_commit(CommitMode::PreserveSource);
        }

        if self.can_use_mutator_plan() {
            return self.commit_with_mutator_plan();
        }

        let files = self.render_generated_companion_files()?;
        self.validate_generated_files(&files)?;
        self.finish_commit(
            CommitMode::PreserveSource,
            CommitStrategy::GeneratedCompanionFiles,
            files,
        )
    }

    fn commit_rewrite_source(&self) -> Result<CommitResult, SessionError> {
        if self.overlay.is_empty() {
            return self.noop_commit(CommitMode::RewriteSource);
        }

        let files = self.render_generated_companion_files()?;
        self.validate_generated_files(&files)?;
        self.finish_commit(
            CommitMode::RewriteSource,
            CommitStrategy::RewriteGeneratedSource,
            files,
        )
    }

    fn commit_with_mutator_plan(&self) -> Result<CommitResult, SessionError> {
        let Some(project) = self.base.source_project.clone() else {
            return Err(SessionError::Unsupported(
                "source-preserving mutator commits require source-backed snapshot".to_string(),
            ));
        };
        let context = MutationContext::from_project(project);
        let operations = self
            .operation_log
            .rename_operations()
            .map(|(element, new_name)| SemanticMutation::RenameDeclaration {
                element: element.clone(),
                new_name: new_name.to_string(),
            })
            .collect::<Vec<_>>();
        let proposal = MutationProposal {
            intent: self
                .label
                .clone()
                .unwrap_or_else(|| "Apply source-preserving session fork".to_string()),
            operations,
            evidence: vec![MutationEvidence {
                element: None,
                summary: "Generated from ModelFork preserve-source commit.".to_string(),
            }],
            rationale: None,
            workspace_revision: context.workspace_revision.clone(),
        };
        let service = CoreMutationFeasibilityService::new();
        let report = service.check(&context, &proposal);
        if !matches!(
            report.status,
            FeasibilityStatus::Allowed | FeasibilityStatus::AllowedWithWarnings
        ) {
            return Err(SessionError::Feasibility(
                report
                    .blocking_reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| FeasibilityIssue {
                        kind: FeasibilityIssueKind::ValidationFailure,
                        operation_index: None,
                        message: "mutator plan was not feasible".to_string(),
                    }),
            ));
        }
        let plan = report.normalized_plan.as_ref().ok_or_else(|| {
            SessionError::Unsupported("missing normalized mutation plan".to_string())
        })?;
        let application = service
            .apply_checked_plan(&context, plan)
            .map_err(SessionError::Feasibility)?;
        let semantic_diff = application.semantic_diff;
        let changed_files = application.changed_files;
        let edited_files = application.edited_files;
        let new_kir = self.materialize()?;
        let new_revision = workspace_revision_for_kir_document(&new_kir)?;
        self.publish_if_workspace(new_kir, new_revision.clone(), &semantic_diff)?;
        Ok(CommitResult {
            mode: CommitMode::PreserveSource,
            strategy_used: CommitStrategy::MutatorPlan,
            base_revision: self.base.revision.clone(),
            new_revision,
            changed_files,
            edited_files,
            semantic_diff,
            generated_elements: 0,
        })
    }

    fn finish_commit(
        &self,
        mode: CommitMode,
        strategy_used: CommitStrategy,
        edited_files: BTreeMap<String, String>,
    ) -> Result<CommitResult, SessionError> {
        let new_kir = self.materialize()?;
        new_kir.validate()?;
        let new_revision = workspace_revision_for_kir_document(&new_kir)?;
        let semantic_diff = diff_kir_documents(&self.base.kir, &new_kir);
        let changed_files = edited_files.keys().cloned().collect::<BTreeSet<_>>();
        self.publish_if_workspace(new_kir, new_revision.clone(), &semantic_diff)?;
        Ok(CommitResult {
            mode,
            strategy_used,
            base_revision: self.base.revision.clone(),
            new_revision,
            changed_files,
            edited_files,
            semantic_diff,
            generated_elements: self.overlay.added_elements.len(),
        })
    }

    fn noop_commit(&self, mode: CommitMode) -> Result<CommitResult, SessionError> {
        Ok(CommitResult {
            mode,
            strategy_used: CommitStrategy::NoOp,
            base_revision: self.base.revision.clone(),
            new_revision: self.base.revision.clone(),
            changed_files: BTreeSet::new(),
            edited_files: BTreeMap::new(),
            semantic_diff: SemanticDiff::default(),
            generated_elements: 0,
        })
    }

    fn can_use_mutator_plan(&self) -> bool {
        self.base.source_project.is_some()
            && self.overlay.added_elements.is_empty()
            && self.overlay.added_members.is_empty()
            && self.overlay.removed_elements.is_empty()
            && self.operation_log.is_all_renames()
            && self.operation_log.operations.len() <= GENERATED_FILE_THRESHOLD
    }

    fn render_generated_companion_files(&self) -> Result<BTreeMap<String, String>, SessionError> {
        let overlay_document = KirDocument {
            metadata: BTreeMap::new(),
            elements: self.overlay.added_elements_with_member_patches(),
        };
        let project = AuthoringProject::from_kir_document(&overlay_document)?;
        let mut rendered = BTreeMap::new();
        for (path, _) in project.files() {
            rendered.insert(path.to_string(), project.render_new_file(path)?);
        }
        Ok(rendered)
    }

    fn validate_generated_files(
        &self,
        _files: &BTreeMap<String, String>,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    pub fn relationship(
        &mut self,
        owner: &ForkElement,
        kind: &str,
        target: &ForkElement,
    ) -> Result<ForkElement, SessionError> {
        let normalized = normalize_name(kind)?;
        let name = format!("{}_{}", normalized, target.qualified_name.replace('.', "_"));
        let qualified_name = format!("{}.{}", owner.qualified_name, name);
        let id = format!("relationship.{qualified_name}");
        self.ensure_not_present(&id)?;
        let source_file = source_file_for_owner(self.overlay.added_elements.get(&owner.id))
            .unwrap_or_else(|| generated_source_file_for(&owner.qualified_name));
        let element = KirElement {
            id: id.clone(),
            kind: format!("Model::{}Relationship", pascal_case(&normalized)),
            layer: 2,
            properties: BTreeMap::from([
                ("declared_name".to_string(), Value::String(name)),
                ("owner".to_string(), Value::String(owner.id.clone())),
                ("source".to_string(), Value::String(owner.id.clone())),
                ("target".to_string(), Value::String(target.id.clone())),
                ("metadata".to_string(), source_file_metadata(&source_file)),
            ]),
        };
        self.overlay.added_elements.insert(id.clone(), element);
        self.patch_member(owner.id.as_str(), id.as_str());
        Ok(ForkElement { id, qualified_name })
    }

    fn publish_if_workspace(
        &self,
        kir: KirDocument,
        revision: WorkspaceRevision,
        semantic_diff: &SemanticDiff,
    ) -> Result<(), SessionError> {
        if let Some(workspace) = &self.workspace {
            let validation_report = validate_kir_semantics(&kir)?;
            let event = ModelChangeEvent::new(
                self.base.revision.clone(),
                revision.clone(),
                ModelChangeProvenance {
                    mutation_id: self
                        .label
                        .clone()
                        .unwrap_or_else(|| "model-fork".to_string()),
                    actor: None,
                },
                semantic_diff.clone(),
            );
            workspace.publish_snapshot(
                WorkspaceSnapshot {
                    revision,
                    kir: Arc::new(kir),
                    validation_report,
                    profile_id: self.base.profile_id.clone(),
                    source_project: None,
                },
                event,
            );
        }
        Ok(())
    }

    fn ensure_current_workspace_revision(&self) -> Result<(), SessionError> {
        if let Some(workspace) = &self.workspace {
            let current = workspace.current_snapshot();
            if current.revision != self.base.revision {
                return Err(SessionError::StaleWorkspace {
                    base_revision: self.base.revision.clone(),
                    current_revision: current.revision.clone(),
                });
            }
        }
        Ok(())
    }

    fn ensure_not_present(&self, id: &str) -> Result<(), SessionError> {
        if self
            .base
            .kir
            .elements
            .iter()
            .any(|element| element.id == id)
            || self.overlay.added_elements.contains_key(id)
        {
            return Err(SessionError::DuplicateElement(id.to_string()));
        }
        Ok(())
    }

    fn patch_member(&mut self, owner_id: &str, member_id: &str) {
        let members = self
            .overlay
            .added_members
            .entry(owner_id.to_string())
            .or_default();
        members.push(member_id.to_string());
    }
}

impl KirOverlay {
    pub fn materialize(&self, base: &KirDocument) -> Result<KirDocument, SessionError> {
        let mut elements = Vec::new();
        for element in &base.elements {
            if self.removed_elements.contains(&element.id) {
                continue;
            }
            let mut element = element.clone();
            if let Some(updates) = self.updated_properties.get(&element.id) {
                for (key, value) in updates {
                    element.properties.insert(key.clone(), value.clone());
                }
            }
            self.apply_added_members(&mut element);
            elements.push(element);
        }
        for mut element in self.added_elements.values().cloned() {
            self.apply_added_members(&mut element);
            elements.push(element);
        }
        elements.sort_by(|left, right| left.id.cmp(&right.id));
        let document = KirDocument {
            metadata: base.metadata.clone(),
            elements,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn is_empty(&self) -> bool {
        self.added_elements.is_empty()
            && self.updated_properties.is_empty()
            && self.added_members.is_empty()
            && self.removed_elements.is_empty()
    }

    fn added_elements_with_member_patches(&self) -> Vec<KirElement> {
        self.added_elements
            .values()
            .cloned()
            .map(|mut element| {
                self.apply_added_members(&mut element);
                element
            })
            .collect()
    }

    fn apply_added_members(&self, element: &mut KirElement) {
        let Some(added) = self.added_members.get(&element.id) else {
            return;
        };
        let mut members = element
            .properties
            .get("members")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for member_id in added {
            if !members
                .iter()
                .any(|value| value.as_str() == Some(member_id.as_str()))
            {
                members.push(Value::String(member_id.clone()));
            }
        }
        element
            .properties
            .insert("members".to_string(), Value::Array(members));
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleWorkspace {
                base_revision,
                current_revision,
            } => write!(
                f,
                "workspace changed after fork was created: base {}, current {}",
                base_revision.fingerprint, current_revision.fingerprint
            ),
            Self::DuplicateElement(id) => write!(f, "element `{id}` already exists"),
            Self::MissingElement(id) => write!(f, "element `{id}` does not exist"),
            Self::InvalidInput(message) => f.write_str(message),
            Self::Unsupported(message) => f.write_str(message),
            Self::Kir(err) => write!(f, "{err}"),
            Self::Authoring(err) => write!(f, "{err}"),
            Self::Feasibility(issue) => write!(f, "mutation feasibility failed: {}", issue.message),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<KirError> for SessionError {
    fn from(value: KirError) -> Self {
        Self::Kir(value)
    }
}

impl From<mercurio_authoring::authoring::AuthoringError> for SessionError {
    fn from(value: mercurio_authoring::authoring::AuthoringError) -> Self {
        Self::Authoring(value)
    }
}

fn normalize_qname(value: &str) -> Result<String, SessionError> {
    let normalized = value.trim().replace("::", ".");
    if normalized.is_empty()
        || normalized
            .split('.')
            .any(|segment| segment.trim().is_empty())
    {
        return Err(SessionError::InvalidInput(format!(
            "invalid qualified name `{value}`"
        )));
    }
    Ok(normalized)
}

fn normalize_name(value: &str) -> Result<String, SessionError> {
    let normalized = value.trim();
    if normalized.is_empty()
        || normalized.contains('.')
        || normalized.contains(':')
        || normalized.chars().any(char::is_whitespace)
    {
        return Err(SessionError::InvalidInput(format!(
            "invalid declaration name `{value}`"
        )));
    }
    Ok(normalized.to_string())
}

fn generated_source_file_for(qualified_name: &str) -> String {
    format!(
        "generated/{}.model",
        qualified_name
            .replace("::", ".")
            .split('.')
            .map(to_snake_segment)
            .collect::<Vec<_>>()
            .join("_")
    )
}

fn to_snake_segment(segment: &str) -> String {
    let mut output = String::new();
    for (index, ch) in segment.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            output.push('_');
        }
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    output.trim_matches('_').to_string()
}

fn pascal_case(value: &str) -> String {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!(
                    "{}{}",
                    first.to_ascii_uppercase(),
                    chars.as_str().to_ascii_lowercase()
                ),
                None => String::new(),
            }
        })
        .collect()
}

fn source_file_for_owner(owner: Option<&KirElement>) -> Option<String> {
    owner
        .and_then(|element| element.properties.get("metadata"))
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("source_file"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn source_file_metadata(source_file: &str) -> Value {
    Value::Object(
        BTreeMap::from([(
            "source_file".to_string(),
            Value::String(source_file.to_string()),
        )])
        .into_iter()
        .collect(),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use mercurio_authoring::authoring::load_authoring_project_from_model;

    use super::*;

    fn empty_document() -> KirDocument {
        KirDocument {
            metadata: BTreeMap::new(),
            elements: Vec::new(),
        }
    }

    #[test]
    fn session_and_fork_share_base_kir() {
        let snapshot = Arc::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let session = snapshot.session();
        let fork = session.fork("shared");

        assert!(Arc::ptr_eq(&session.snapshot().kir, &fork.base.kir));
        assert_eq!(fork.base_revision(), &snapshot.revision);
    }

    #[test]
    fn session_exposes_canonical_semantic_workspace_snapshot() {
        let snapshot = Arc::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let session = snapshot.session();

        let semantic_snapshot = session.semantic_workspace_snapshot().unwrap();

        assert_eq!(semantic_snapshot.revision, snapshot.revision);
        assert_eq!(*semantic_snapshot.kir, *snapshot.kir);
        assert_eq!(
            semantic_snapshot.graph.elements().len(),
            snapshot.kir.elements.len()
        );
    }

    #[test]
    fn generic_fork_overlay_adds_large_element_set_without_cloning_base() {
        let snapshot = Arc::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let session = snapshot.session();
        let mut fork = session.fork("bulk generated elements");
        let package = fork.package("SyntheticElements", None).unwrap();
        for index in 0..10_000 {
            fork.semantic_element(
                &package,
                ForkElementSpec {
                    id_prefix: "element".to_string(),
                    kind: "model.GenericUsage".to_string(),
                    name: format!("Generated{index:05}"),
                    properties: BTreeMap::new(),
                },
            )
            .unwrap();
        }

        assert_eq!(fork.overlay().added_elements.len(), 10_001);
        assert_eq!(snapshot.kir.elements.len(), 0);
        assert_eq!(fork.materialize().unwrap().elements.len(), 10_001);
    }

    #[test]
    fn fork_overlay_bulk_semantic_element_addition_is_linear_enough() {
        let snapshot = Arc::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let session = snapshot.session();
        let mut fork = session.fork("bulk elements timing");
        let package = fork.package("SyntheticElements", None).unwrap();

        let started = Instant::now();
        for index in 0..10_000 {
            fork.semantic_element(
                &package,
                ForkElementSpec {
                    id_prefix: "element".to_string(),
                    kind: "model.GenericUsage".to_string(),
                    name: format!("Generated{index:05}"),
                    properties: BTreeMap::new(),
                },
            )
            .unwrap();
        }
        let elapsed = started.elapsed();

        assert_eq!(fork.overlay().added_elements.len(), 10_001);
        assert!(
            elapsed.as_secs_f64() < 2.0,
            "adding 10k semantic elements to an overlay took {elapsed:?}"
        );
    }

    #[test]
    fn rewrite_source_commit_emits_generated_model() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let session = workspace.session();
        let mut fork = session.fork("generated elements");
        let package = fork.package("SyntheticElements", None).unwrap();
        let target = fork
            .semantic_element(
                &package,
                ForkElementSpec {
                    id_prefix: "element".to_string(),
                    kind: "model.GenericUsage".to_string(),
                    name: "Target00001".to_string(),
                    properties: BTreeMap::new(),
                },
            )
            .unwrap();
        let source = fork
            .semantic_element(
                &package,
                ForkElementSpec {
                    id_prefix: "element".to_string(),
                    kind: "model.GenericUsage".to_string(),
                    name: "source".to_string(),
                    properties: BTreeMap::new(),
                },
            )
            .unwrap();
        fork.relationship(&source, "relates", &target).unwrap();

        let result = fork.commit(CommitMode::RewriteSource).unwrap();

        assert_eq!(result.strategy_used, CommitStrategy::RewriteGeneratedSource);
        let source = result
            .edited_files
            .get("generated/synthetic_elements.model")
            .unwrap();
        assert!(source.contains("package SyntheticElements"));
        assert!(source.contains("generic Target00001"));
        assert!(source.contains("generic source"));
        assert!(source.contains("relates Target00001 references"));
        assert_ne!(workspace.current_snapshot().revision, result.base_revision);
    }

    #[test]
    fn preserve_source_bulk_addition_uses_generated_companion_file() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let session = workspace.session();
        let mut fork = session.fork("generated elements");
        let package = fork.package("SyntheticElements", None).unwrap();
        for index in 0..101 {
            fork.semantic_element(
                &package,
                ForkElementSpec {
                    id_prefix: "element".to_string(),
                    kind: "model.GenericUsage".to_string(),
                    name: format!("Generated{index:05}"),
                    properties: BTreeMap::new(),
                },
            )
            .unwrap();
        }

        let result = fork.commit(CommitMode::PreserveSource).unwrap();

        assert_eq!(
            result.strategy_used,
            CommitStrategy::GeneratedCompanionFiles
        );
        assert_eq!(result.generated_elements, 102);
    }

    #[test]
    fn preserve_source_small_rename_uses_mutator_plan() {
        let project = load_authoring_project_from_model(BTreeMap::from([(
            "vehicle.model".to_string(),
            "package Vehicle { part engine; }".to_string(),
        )]))
        .unwrap();
        let workspace =
            ModelWorkspace::new(WorkspaceSnapshot::from_authoring_project(project).unwrap());
        let session = workspace.session();
        let mut fork = session.fork("rename engine");
        fork.rename_declaration("Vehicle.engine", "motor").unwrap();

        let result = fork.commit(CommitMode::PreserveSource).unwrap();

        assert_eq!(result.strategy_used, CommitStrategy::MutatorPlan);
        assert!(result.edited_files["vehicle.model"].contains("part motor;"));
    }

    #[test]
    fn stale_fork_commit_fails() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let first = workspace.session();
        let second = workspace.session();

        let mut first_fork = first.fork("first");
        let first_package = first_fork.package("First", None).unwrap();
        first_fork
            .semantic_element(
                &first_package,
                ForkElementSpec {
                    id_prefix: "element".to_string(),
                    kind: "model.GenericUsage".to_string(),
                    name: "Generated00001".to_string(),
                    properties: BTreeMap::new(),
                },
            )
            .unwrap();
        first_fork.commit(CommitMode::RewriteSource).unwrap();

        let mut stale_fork = second.fork("stale");
        stale_fork.package("Second", None).unwrap();
        let error = stale_fork.commit(CommitMode::RewriteSource).unwrap_err();

        assert!(matches!(error, SessionError::StaleWorkspace { .. }));
    }

    #[test]
    fn workspace_subscribers_receive_diff_parity_in_commit_order() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let subscription = workspace.subscribe();

        let mut first = workspace.session().fork("mutation-1");
        first.package("First", None).unwrap();
        let first_result = first.commit(CommitMode::RewriteSource).unwrap();
        let first_event = subscription.recv().unwrap();

        assert_eq!(first_event.sequence, 1);
        assert_eq!(first_event.revision_before, first_result.base_revision);
        assert_eq!(first_event.revision_after, first_result.new_revision);
        assert_eq!(first_event.provenance.mutation_id, "mutation-1");
        assert_eq!(first_event.diff, first_result.semantic_diff);

        let mut second = workspace.session().fork("mutation-2");
        second.package("Second", None).unwrap();
        let second_result = second.commit(CommitMode::RewriteSource).unwrap();
        let second_event = subscription.recv().unwrap();

        assert_eq!(second_event.sequence, 2);
        assert_eq!(second_event.diff, second_result.semantic_diff);
    }
    #[test]
    fn command_stack_undo_redo_and_persistence_restore_snapshot_digests() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let initial = workspace.current_snapshot().revision.clone();

        let mut fork = workspace.session().fork("add-package");
        fork.package("Added", None).unwrap();
        let committed = fork.commit(CommitMode::RewriteSource).unwrap();
        assert_ne!(committed.new_revision, initial);
        assert!(workspace.can_undo());

        let persisted = serde_json::to_string(&workspace.command_stack_state()).unwrap();
        let restored: CommandStackState = serde_json::from_str(&persisted).unwrap();
        workspace.restore_command_stack_state(restored);

        assert_eq!(workspace.undo().unwrap(), initial);
        assert!(workspace.can_redo());
        assert_eq!(workspace.redo().unwrap(), committed.new_revision);
    }

    fn document_with_package(name: &str) -> KirDocument {
        KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: format!("pkg.{name}"),
                kind: "Model::Package".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "declared_name".to_string(),
                    Value::String(name.to_string()),
                )]),
            }],
        }
    }

    fn change_event(
        before: &WorkspaceSnapshot,
        after: &WorkspaceSnapshot,
        mutation_id: &str,
    ) -> ModelChangeEvent {
        ModelChangeEvent::new(
            before.revision.clone(),
            after.revision.clone(),
            ModelChangeProvenance {
                mutation_id: mutation_id.to_string(),
                actor: None,
            },
            diff_kir_documents(&before.kir, &after.kir),
        )
    }

    #[test]
    fn publish_applied_tracks_command_stack_and_emits_event() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let subscription = workspace.subscribe();
        let initial = workspace.current_snapshot();
        assert_eq!(workspace.undo_depth(), 0);
        assert_eq!(workspace.redo_depth(), 0);

        let applied = WorkspaceSnapshot::new(document_with_package("Applied")).unwrap();
        let event = change_event(&initial, &applied, "apply-1");
        let applied_revision = applied.revision.clone();
        workspace.publish_applied(applied, event);

        let received = subscription.recv().unwrap();
        assert_eq!(received.sequence, 1);
        assert_eq!(received.provenance.mutation_id, "apply-1");
        assert_eq!(received.revision_after, applied_revision);
        assert_eq!(workspace.current_snapshot().revision, applied_revision);
        assert!(workspace.can_undo());
        assert_eq!(workspace.undo_depth(), 1);
        assert_eq!(workspace.redo_depth(), 0);

        assert_eq!(workspace.undo().unwrap(), initial.revision);
        assert_eq!(workspace.undo_depth(), 0);
        assert_eq!(workspace.redo_depth(), 1);
        assert_eq!(workspace.redo().unwrap(), applied_revision);
        assert_eq!(workspace.undo_depth(), 1);
        assert_eq!(workspace.redo_depth(), 0);
    }

    #[test]
    fn publish_replaced_advances_revision_without_command_stack_entry() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let subscription = workspace.subscribe();
        let initial = workspace.current_snapshot();

        let replaced = WorkspaceSnapshot::new(document_with_package("Replaced")).unwrap();
        let event = change_event(&initial, &replaced, "text-edit");
        let replaced_revision = replaced.revision.clone();
        workspace.publish_replaced(replaced, event);

        let received = subscription.recv().unwrap();
        assert_eq!(received.sequence, 1);
        assert_eq!(received.provenance.mutation_id, "text-edit");
        assert_eq!(workspace.current_snapshot().revision, replaced_revision);
        assert!(!workspace.can_undo());
        assert_eq!(workspace.undo_depth(), 0);
        assert_eq!(workspace.redo_depth(), 0);
    }

    #[test]
    fn publish_replaced_after_apply_makes_undo_stale() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let initial = workspace.current_snapshot();

        let applied = WorkspaceSnapshot::new(document_with_package("Applied")).unwrap();
        let event = change_event(&initial, &applied, "apply-1");
        workspace.publish_applied(applied, event);
        assert_eq!(workspace.undo_depth(), 1);

        let current = workspace.current_snapshot();
        let replaced = WorkspaceSnapshot::new(document_with_package("Edited")).unwrap();
        let event = change_event(&current, &replaced, "text-edit");
        workspace.publish_replaced(replaced, event);
        assert_eq!(workspace.undo_depth(), 1);

        let error = workspace.undo().unwrap_err();
        assert!(matches!(error, SessionError::StaleWorkspace { .. }));
        assert_eq!(workspace.undo_depth(), 0);
        assert_eq!(workspace.redo_depth(), 0);
    }

    #[test]
    fn new_mutation_after_undo_invalidates_redo_branch() {
        let workspace = ModelWorkspace::new(WorkspaceSnapshot::new(empty_document()).unwrap());
        let mut first = workspace.session().fork("first");
        first.package("First", None).unwrap();
        first.commit(CommitMode::RewriteSource).unwrap();
        workspace.undo().unwrap();

        let mut second = workspace.session().fork("second");
        second.package("Second", None).unwrap();
        second.commit(CommitMode::RewriteSource).unwrap();
        assert!(!workspace.can_redo());
    }
}
