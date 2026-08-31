use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::authoring::source_set::SourceDocument;
use crate::kir::{KirDocument, KirError};
use crate::model::Graph;
use crate::model::MetamodelAttributeRegistry;
use crate::semantic_services::identity::{stable_digest, workspace_revision_for_kir_document};
use crate::semantic_services::mutation::WorkspaceRevision;

pub const MODEL_SERVICE_API_VERSION: &str = "mercurio.model-service.v2";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRevisionId {
    pub fingerprint: String,
}

impl ModelRevisionId {
    pub fn new(fingerprint: impl Into<String>) -> Self {
        Self {
            fingerprint: fingerprint.into(),
        }
    }

    pub fn from_workspace_revision(revision: &WorkspaceRevision) -> Self {
        Self::new(revision.fingerprint.clone())
    }

    pub fn as_str(&self) -> &str {
        &self.fingerprint
    }
}

impl fmt::Display for ModelRevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.fingerprint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStateId {
    pub value: String,
}

impl ModelStateId {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ModelStateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSourceKind {
    SourceDocument,
    SourceFile,
    KirDocument,
    KparPackage,
    RemoteModel,
    DslScript,
    InMemoryOverlay,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSource {
    pub id: String,
    pub kind: InputSourceKind,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl InputSource {
    pub fn new(kind: InputSourceKind, uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let id = source_id(&kind, &uri, None);
        Self {
            id,
            kind,
            uri,
            language: None,
            digest: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn from_source_document(source: &SourceDocument) -> Self {
        let digest = stable_digest([("source-document".as_bytes(), source.content.as_bytes())]);
        let mut input = Self::new(InputSourceKind::SourceDocument, source.path.clone());
        input.id = source_id(&input.kind, &input.uri, Some(&digest));
        input.digest = Some(digest);
        input
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn with_digest(mut self, digest: impl Into<String>) -> Self {
        let digest = digest.into();
        self.id = source_id(&self.kind, &self.uri, Some(&digest));
        self.digest = Some(digest);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSourceSet {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<InputSource>,
}

impl InputSourceSet {
    pub fn new(label: Option<String>, sources: Vec<InputSource>) -> Self {
        let id = input_source_set_id(label.as_deref(), &sources);
        Self { id, label, sources }
    }

    pub fn from_source_documents(label: Option<String>, sources: &[SourceDocument]) -> Self {
        Self::new(
            label,
            sources
                .iter()
                .map(InputSource::from_source_document)
                .collect(),
        )
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteModelRef {
    pub service_uri: String,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_id: Option<ModelStateId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_id: Option<ModelRevisionId>,
}

impl RemoteModelRef {
    pub fn new(service_uri: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            service_uri: service_uri.into(),
            model_id: model_id.into(),
            state_id: None,
            revision_id: None,
        }
    }

    pub fn with_state_id(mut self, state_id: ModelStateId) -> Self {
        self.state_id = Some(state_id);
        self
    }

    pub fn with_revision_id(mut self, revision_id: ModelRevisionId) -> Self {
        self.revision_id = Some(revision_id);
        self
    }

    pub fn model_uri(&self) -> String {
        format!(
            "{}/models/{}",
            self.service_uri.trim_end_matches('/'),
            self.model_id
        )
    }

    pub fn input_source(&self) -> InputSource {
        let mut source = InputSource::new(InputSourceKind::RemoteModel, self.model_uri())
            .with_metadata("service_uri", self.service_uri.clone())
            .with_metadata("model_id", self.model_id.clone());
        if let Some(state_id) = &self.state_id {
            source = source.with_metadata("state_id", state_id.as_str());
        }
        if let Some(revision_id) = &self.revision_id {
            source = source.with_metadata("revision_id", revision_id.as_str());
        }
        source
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRevisionProducer {
    SourceCompile,
    KirImport,
    RemotePull,
    DslEvaluation,
    InMemoryMutation,
    WorkspaceSnapshot,
    SemanticSnapshot,
    Capability,
    Unknown,
}

impl Default for ModelRevisionProducer {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBuildRecord {
    pub producer: ModelRevisionProducer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_source_set: Option<InputSourceSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_revision: Option<ModelRevisionId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl ModelBuildRecord {
    pub fn new(producer: ModelRevisionProducer) -> Self {
        Self {
            producer,
            input_source_set: None,
            parent_revision: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn source_compile(input_source_set: InputSourceSet) -> Self {
        Self {
            producer: ModelRevisionProducer::SourceCompile,
            input_source_set: Some(input_source_set),
            parent_revision: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn remote_pull(remote: &RemoteModelRef) -> Self {
        Self {
            producer: ModelRevisionProducer::RemotePull,
            input_source_set: Some(InputSourceSet::new(
                Some(format!("remote model {}", remote.model_id)),
                vec![remote.input_source()],
            )),
            parent_revision: remote.revision_id.clone(),
            metadata: BTreeMap::from([
                (
                    "service_uri".to_string(),
                    Value::String(remote.service_uri.clone()),
                ),
                (
                    "model_id".to_string(),
                    Value::String(remote.model_id.clone()),
                ),
            ]),
        }
    }

    pub fn with_input_source_set(mut self, input_source_set: InputSourceSet) -> Self {
        self.input_source_set = Some(input_source_set);
        self
    }

    pub fn with_parent_revision(mut self, parent_revision: ModelRevisionId) -> Self {
        self.parent_revision = Some(parent_revision);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

impl Default for ModelBuildRecord {
    fn default() -> Self {
        Self::new(ModelRevisionProducer::Unknown)
    }
}

#[derive(Debug, Clone)]
pub struct ModelRevision {
    id: ModelRevisionId,
    workspace_revision: WorkspaceRevision,
    kir: Arc<KirDocument>,
    graph: Arc<Graph>,
    metamodel_registry: Arc<MetamodelAttributeRegistry>,
    profile_id: Option<String>,
    build: ModelBuildRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRevisionDescriptor {
    pub id: ModelRevisionId,
    pub workspace_revision: WorkspaceRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub producer: ModelRevisionProducer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_source_set_id: Option<String>,
    pub element_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRevisionEnvelope {
    pub id: ModelRevisionId,
    pub workspace_revision: WorkspaceRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub build: ModelBuildRecord,
    pub kir: KirDocument,
}

impl ModelRevisionEnvelope {
    pub fn from_model_revision(revision: &ModelRevision) -> Self {
        let kir = revision.kir();
        Self {
            id: revision.id().clone(),
            workspace_revision: revision.workspace_revision().clone(),
            profile_id: revision.profile_id.clone(),
            build: revision.build().clone(),
            kir: (*kir).clone(),
        }
    }

    pub fn into_model_revision(self) -> Result<ModelRevision, ModelStateError> {
        let expected = self.id;
        let revision =
            ModelRevision::from_kir_document_with_profile(self.kir, self.profile_id, self.build)?;
        if revision.id() != &expected {
            return Err(ModelStateError::RevisionMismatch {
                expected,
                actual: revision.id().clone(),
            });
        }
        Ok(revision)
    }

    pub fn descriptor(&self) -> ModelRevisionDescriptor {
        ModelRevisionDescriptor {
            id: self.id.clone(),
            workspace_revision: self.workspace_revision.clone(),
            profile_id: self.profile_id.clone(),
            producer: self.build.producer.clone(),
            input_source_set_id: self
                .build
                .input_source_set
                .as_ref()
                .map(|source_set| source_set.id.clone()),
            element_count: self.kir.elements.len(),
        }
    }
}

impl ModelRevision {
    pub fn from_kir_document(
        kir: KirDocument,
        build: ModelBuildRecord,
    ) -> Result<Self, ModelStateError> {
        Self::from_kir_document_with_profile(kir, None, build)
    }

    pub fn from_kir_document_with_profile(
        kir: KirDocument,
        profile_id: Option<String>,
        build: ModelBuildRecord,
    ) -> Result<Self, ModelStateError> {
        kir.validate()?;
        let workspace_revision = workspace_revision_for_kir_document(&kir)?;
        let graph = Graph::from_document(kir.clone())
            .map_err(|error| ModelStateError::Graph(error.to_string()))?;
        let metamodel_registry = MetamodelAttributeRegistry::build(&graph);
        Ok(Self::from_parts(
            Arc::new(kir),
            Arc::new(graph),
            Arc::new(metamodel_registry),
            workspace_revision,
            profile_id,
            build,
        ))
    }

    pub fn from_parts(
        kir: Arc<KirDocument>,
        graph: Arc<Graph>,
        metamodel_registry: Arc<MetamodelAttributeRegistry>,
        workspace_revision: WorkspaceRevision,
        profile_id: Option<String>,
        build: ModelBuildRecord,
    ) -> Self {
        let id = ModelRevisionId::from_workspace_revision(&workspace_revision);
        Self {
            id,
            workspace_revision,
            kir,
            graph,
            metamodel_registry,
            profile_id,
            build,
        }
    }

    pub fn id(&self) -> &ModelRevisionId {
        &self.id
    }

    pub fn workspace_revision(&self) -> &WorkspaceRevision {
        &self.workspace_revision
    }

    pub fn kir(&self) -> Arc<KirDocument> {
        Arc::clone(&self.kir)
    }

    pub fn graph(&self) -> Arc<Graph> {
        Arc::clone(&self.graph)
    }

    pub fn metamodel_registry(&self) -> Arc<MetamodelAttributeRegistry> {
        Arc::clone(&self.metamodel_registry)
    }

    pub fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }

    pub fn build(&self) -> &ModelBuildRecord {
        &self.build
    }

    pub fn descriptor(&self) -> ModelRevisionDescriptor {
        ModelRevisionDescriptor {
            id: self.id.clone(),
            workspace_revision: self.workspace_revision.clone(),
            profile_id: self.profile_id.clone(),
            producer: self.build.producer.clone(),
            input_source_set_id: self
                .build
                .input_source_set
                .as_ref()
                .map(|source_set| source_set.id.clone()),
            element_count: self.graph.elements().len(),
        }
    }

    pub fn envelope(&self) -> ModelRevisionEnvelope {
        ModelRevisionEnvelope::from_model_revision(self)
    }
}

#[derive(Debug, Clone)]
pub struct ModelState {
    id: ModelStateId,
    label: Option<String>,
    current: Arc<RwLock<Arc<ModelRevision>>>,
    history: Arc<RwLock<Vec<ModelRevisionId>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStateDescriptor {
    pub id: ModelStateId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub current_revision: ModelRevisionDescriptor,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revision_history: Vec<ModelRevisionId>,
}

impl ModelState {
    pub fn new(id: ModelStateId, label: Option<String>, current_revision: ModelRevision) -> Self {
        let current_revision = Arc::new(current_revision);
        Self {
            id,
            label,
            current: Arc::new(RwLock::new(Arc::clone(&current_revision))),
            history: Arc::new(RwLock::new(vec![current_revision.id().clone()])),
        }
    }

    pub fn from_revision(label: Option<String>, current_revision: ModelRevision) -> Self {
        let id = model_state_id(label.as_deref(), current_revision.id());
        Self::new(id, label, current_revision)
    }

    pub fn id(&self) -> &ModelStateId {
        &self.id
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn current_revision(&self) -> Result<Arc<ModelRevision>, ModelStateError> {
        let current = self
            .current
            .read()
            .map_err(|_| ModelStateError::LockPoisoned)?;
        Ok(Arc::clone(&current))
    }

    pub fn current_revision_id(&self) -> Result<ModelRevisionId, ModelStateError> {
        Ok(self.current_revision()?.id().clone())
    }

    pub fn revision_history(&self) -> Result<Vec<ModelRevisionId>, ModelStateError> {
        let history = self
            .history
            .read()
            .map_err(|_| ModelStateError::LockPoisoned)?;
        Ok(history.clone())
    }

    pub fn replace_current(
        &self,
        expected_revision: Option<&ModelRevisionId>,
        next_revision: ModelRevision,
    ) -> Result<Arc<ModelRevision>, ModelStateError> {
        let mut current = self
            .current
            .write()
            .map_err(|_| ModelStateError::LockPoisoned)?;
        if let Some(expected_revision) = expected_revision {
            if current.id() != expected_revision {
                return Err(ModelStateError::StaleRevision {
                    expected: expected_revision.clone(),
                    current: current.id().clone(),
                });
            }
        }

        let next_revision = Arc::new(next_revision);
        *current = Arc::clone(&next_revision);
        self.history
            .write()
            .map_err(|_| ModelStateError::LockPoisoned)?
            .push(next_revision.id().clone());
        Ok(next_revision)
    }

    pub fn descriptor(&self) -> Result<ModelStateDescriptor, ModelStateError> {
        let current_revision = self.current_revision()?;
        Ok(ModelStateDescriptor {
            id: self.id.clone(),
            label: self.label.clone(),
            current_revision: current_revision.descriptor(),
            revision_history: self.revision_history()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelArtifact {
    pub id: String,
    pub revision_id: ModelRevisionId,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub payload: Value,
}

impl ModelArtifact {
    pub fn new(
        revision_id: ModelRevisionId,
        kind: impl Into<String>,
        label: Option<String>,
        payload: Value,
    ) -> Result<Self, ModelStateError> {
        let kind = kind.into();
        let id = model_artifact_id(&revision_id, &kind, label.as_deref(), &payload)?;
        Ok(Self {
            id,
            revision_id,
            kind,
            label,
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelServicePullRequest {
    pub remote: RemoteModelRef,
    #[serde(default)]
    pub include_artifacts: bool,
}

impl ModelServicePullRequest {
    pub fn new(remote: RemoteModelRef) -> Self {
        Self {
            remote,
            include_artifacts: true,
        }
    }

    pub fn without_artifacts(mut self) -> Self {
        self.include_artifacts = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelServicePullResponse {
    pub api_version: String,
    pub remote: RemoteModelRef,
    pub revision: ModelRevisionEnvelope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ModelArtifact>,
}

impl ModelServicePullResponse {
    pub fn new(
        remote: RemoteModelRef,
        revision: ModelRevisionEnvelope,
        artifacts: Vec<ModelArtifact>,
    ) -> Self {
        Self {
            api_version: MODEL_SERVICE_API_VERSION.to_string(),
            remote,
            revision,
            artifacts,
        }
    }

    pub fn model_revision(&self) -> Result<ModelRevision, ModelStateError> {
        self.revision.clone().into_model_revision()
    }

    pub fn model_state(&self, label: Option<String>) -> Result<ModelState, ModelStateError> {
        Ok(ModelState::from_revision(label, self.model_revision()?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRevisionPushMode {
    Propose,
    Publish,
}

impl Default for ModelRevisionPushMode {
    fn default() -> Self {
        Self::Propose
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelServicePushRevisionRequest {
    pub api_version: String,
    pub remote: RemoteModelRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<ModelRevisionId>,
    pub revision: ModelRevisionEnvelope,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ModelArtifact>,
    pub mode: ModelRevisionPushMode,
}

impl ModelServicePushRevisionRequest {
    pub fn propose(
        remote: RemoteModelRef,
        base_revision: Option<ModelRevisionId>,
        revision: ModelRevisionEnvelope,
        artifacts: Vec<ModelArtifact>,
    ) -> Self {
        Self {
            api_version: MODEL_SERVICE_API_VERSION.to_string(),
            remote,
            base_revision,
            revision,
            artifacts,
            mode: ModelRevisionPushMode::Propose,
        }
    }

    pub fn publish(
        remote: RemoteModelRef,
        base_revision: Option<ModelRevisionId>,
        revision: ModelRevisionEnvelope,
        artifacts: Vec<ModelArtifact>,
    ) -> Self {
        Self {
            mode: ModelRevisionPushMode::Publish,
            ..Self::propose(remote, base_revision, revision, artifacts)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelServicePushStatus {
    Accepted,
    Rejected,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelServicePushRevisionResponse {
    pub api_version: String,
    pub status: ModelServicePushStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_revision_id: Option<ModelRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

impl ModelServicePushRevisionResponse {
    pub fn accepted(revision_id: ModelRevisionId) -> Self {
        Self {
            api_version: MODEL_SERVICE_API_VERSION.to_string(),
            status: ModelServicePushStatus::Accepted,
            accepted_revision_id: Some(revision_id),
            proposal_id: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn rejected(diagnostics: Vec<String>) -> Self {
        Self {
            api_version: MODEL_SERVICE_API_VERSION.to_string(),
            status: ModelServicePushStatus::Rejected,
            accepted_revision_id: None,
            proposal_id: None,
            diagnostics,
        }
    }

    pub fn conflict(current_revision_id: ModelRevisionId) -> Self {
        Self {
            api_version: MODEL_SERVICE_API_VERSION.to_string(),
            status: ModelServicePushStatus::Conflict,
            accepted_revision_id: Some(current_revision_id),
            proposal_id: None,
            diagnostics: vec!["base revision does not match remote current revision".to_string()],
        }
    }
}

#[derive(Debug)]
pub enum ModelStateError {
    Kir(KirError),
    Graph(String),
    Serialization(String),
    LockPoisoned,
    StaleRevision {
        expected: ModelRevisionId,
        current: ModelRevisionId,
    },
    RevisionMismatch {
        expected: ModelRevisionId,
        actual: ModelRevisionId,
    },
}

impl fmt::Display for ModelStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kir(error) => write!(f, "{error}"),
            Self::Graph(error) => write!(f, "graph projection failed: {error}"),
            Self::Serialization(error) => write!(f, "model state serialization failed: {error}"),
            Self::LockPoisoned => write!(f, "model state lock poisoned"),
            Self::StaleRevision { expected, current } => write!(
                f,
                "model state is stale: expected revision {expected}, current revision {current}"
            ),
            Self::RevisionMismatch { expected, actual } => write!(
                f,
                "model revision envelope mismatch: expected revision {expected}, actual revision {actual}"
            ),
        }
    }
}

impl std::error::Error for ModelStateError {}

impl From<KirError> for ModelStateError {
    fn from(value: KirError) -> Self {
        Self::Kir(value)
    }
}

impl From<serde_json::Error> for ModelStateError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}

fn source_id(kind: &InputSourceKind, uri: &str, digest: Option<&str>) -> String {
    let kind = format!("{kind:?}");
    let digest = digest.unwrap_or("");
    stable_digest([
        ("input-source-kind".as_bytes(), kind.as_bytes()),
        ("input-source-uri".as_bytes(), uri.as_bytes()),
        ("input-source-digest".as_bytes(), digest.as_bytes()),
    ])
}

fn input_source_set_id(label: Option<&str>, sources: &[InputSource]) -> String {
    let mut owned_chunks = vec![(
        "input-source-set-label".as_bytes().to_vec(),
        label.unwrap_or("").as_bytes().to_vec(),
    )];
    for source in sources {
        owned_chunks.push((
            "input-source-id".as_bytes().to_vec(),
            source.id.as_bytes().to_vec(),
        ));
        if let Some(digest) = &source.digest {
            owned_chunks.push((
                "input-source-digest".as_bytes().to_vec(),
                digest.as_bytes().to_vec(),
            ));
        }
    }

    stable_digest(
        owned_chunks
            .iter()
            .map(|(label, value)| (label.as_slice(), value.as_slice())),
    )
}

fn model_state_id(label: Option<&str>, revision_id: &ModelRevisionId) -> ModelStateId {
    ModelStateId::new(stable_digest([
        (
            "model-state-label".as_bytes(),
            label.unwrap_or("").as_bytes(),
        ),
        (
            "model-state-initial-revision".as_bytes(),
            revision_id.as_str().as_bytes(),
        ),
    ]))
}

fn model_artifact_id(
    revision_id: &ModelRevisionId,
    kind: &str,
    label: Option<&str>,
    payload: &Value,
) -> Result<String, ModelStateError> {
    let payload = serde_json::to_vec(payload)?;
    Ok(stable_digest([
        (
            "model-artifact-revision".as_bytes(),
            revision_id.as_str().as_bytes(),
        ),
        ("model-artifact-kind".as_bytes(), kind.as_bytes()),
        (
            "model-artifact-label".as_bytes(),
            label.unwrap_or("").as_bytes(),
        ),
        ("model-artifact-payload".as_bytes(), payload.as_slice()),
    ]))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::kir::KirElement;

    #[test]
    fn source_set_records_files_that_built_revision() {
        let sources = vec![
            SourceDocument::new("models/a.sysml", "package A;"),
            SourceDocument::new("models/b.sysml", "package B;"),
        ];
        let source_set =
            InputSourceSet::from_source_documents(Some("open project".to_string()), &sources);
        let revision = ModelRevision::from_kir_document(
            sample_document("A"),
            ModelBuildRecord::source_compile(source_set.clone()),
        )
        .unwrap();

        assert_eq!(
            revision.id().as_str(),
            revision.workspace_revision().fingerprint
        );
        assert_eq!(
            revision.build().input_source_set.as_ref(),
            Some(&source_set)
        );
        assert_eq!(revision.descriptor().element_count, 1);
        assert_eq!(
            revision.descriptor().input_source_set_id.as_deref(),
            Some(source_set.id.as_str())
        );
    }

    #[test]
    fn model_state_tracks_current_revision_and_rejects_stale_replacement() {
        let first = ModelRevision::from_kir_document(
            sample_document("A"),
            ModelBuildRecord::new(ModelRevisionProducer::KirImport),
        )
        .unwrap();
        let second = ModelRevision::from_kir_document(
            sample_document("B"),
            ModelBuildRecord::new(ModelRevisionProducer::InMemoryMutation)
                .with_parent_revision(first.id().clone()),
        )
        .unwrap();
        let stale = ModelRevisionId::new("stale");
        let first_id = first.id().clone();
        let second_id = second.id().clone();
        let state = ModelState::from_revision(Some("variant".to_string()), first);

        let error = state
            .replace_current(Some(&stale), second.clone())
            .unwrap_err();
        match error {
            ModelStateError::StaleRevision { expected, current } => {
                assert_eq!(expected, stale);
                assert_eq!(current, first_id.clone());
            }
            other => panic!("expected stale revision error, got {other:?}"),
        }

        let published = state.replace_current(Some(&first_id), second).unwrap();
        assert_eq!(published.id(), &second_id);
        assert_eq!(
            state.revision_history().unwrap(),
            vec![first_id, second_id.clone()]
        );
        assert_eq!(state.descriptor().unwrap().current_revision.id, second_id);
    }

    #[test]
    fn artifact_is_attached_to_model_revision() {
        let revision = ModelRevision::from_kir_document(
            sample_document("A"),
            ModelBuildRecord::new(ModelRevisionProducer::KirImport),
        )
        .unwrap();
        let artifact = ModelArtifact::new(
            revision.id().clone(),
            "mercurio.artifact.analysis/query",
            Some("part count".to_string()),
            json!({ "count": 1 }),
        )
        .unwrap();

        assert_eq!(artifact.revision_id, revision.id().clone());
        assert_eq!(artifact.kind, "mercurio.artifact.analysis/query");
        assert!(!artifact.id.is_empty());
    }

    #[test]
    fn revision_envelope_round_trips_to_in_memory_revision() {
        let revision = ModelRevision::from_kir_document_with_profile(
            sample_document("A"),
            Some("sysml-v2".to_string()),
            ModelBuildRecord::new(ModelRevisionProducer::KirImport),
        )
        .unwrap();
        let envelope = revision.envelope();
        let round_tripped = envelope.clone().into_model_revision().unwrap();

        assert_eq!(envelope.id, revision.id().clone());
        assert_eq!(round_tripped.id(), revision.id());
        assert_eq!(round_tripped.profile_id(), Some("sysml-v2"));
        assert_eq!(round_tripped.descriptor().element_count, 1);
        assert_eq!(envelope.descriptor().element_count, 1);
    }

    #[test]
    fn pulled_remote_revision_can_be_analyzed_in_memory() {
        let remote = RemoteModelRef::new("https://models.example.test/api/v2", "demo-model");
        let revision = ModelRevision::from_kir_document(
            sample_document("A"),
            ModelBuildRecord::remote_pull(&remote),
        )
        .unwrap();
        let response = ModelServicePullResponse::new(
            remote.with_revision_id(revision.id().clone()),
            revision.envelope(),
            Vec::new(),
        );
        let pulled = response.model_revision().unwrap();
        let part_count = pulled
            .kir()
            .elements
            .iter()
            .filter(|element| element.kind.ends_with("PartDefinition"))
            .count();

        assert_eq!(response.api_version, MODEL_SERVICE_API_VERSION);
        assert_eq!(pulled.build().producer, ModelRevisionProducer::RemotePull);
        assert_eq!(part_count, 1);
    }

    #[test]
    fn push_revision_request_defaults_to_proposal_mode() {
        let remote = RemoteModelRef::new("https://models.example.test/api/v2", "demo-model");
        let base = ModelRevision::from_kir_document(
            sample_document("A"),
            ModelBuildRecord::new(ModelRevisionProducer::RemotePull),
        )
        .unwrap();
        let next = ModelRevision::from_kir_document(
            sample_document("B"),
            ModelBuildRecord::new(ModelRevisionProducer::InMemoryMutation)
                .with_parent_revision(base.id().clone()),
        )
        .unwrap();
        let artifact = ModelArtifact::new(
            next.id().clone(),
            "mercurio.artifact.analysis/query",
            Some("variant check".to_string()),
            json!({ "status": "ok" }),
        )
        .unwrap();
        let request = ModelServicePushRevisionRequest::propose(
            remote,
            Some(base.id().clone()),
            next.envelope(),
            vec![artifact.clone()],
        );

        assert_eq!(request.api_version, MODEL_SERVICE_API_VERSION);
        assert_eq!(request.mode, ModelRevisionPushMode::Propose);
        assert_eq!(request.base_revision.as_ref(), Some(base.id()));
        assert_eq!(request.revision.id, next.id().clone());
        assert_eq!(request.artifacts, vec![artifact]);
    }

    fn sample_document(name: &str) -> KirDocument {
        KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: format!("part_def.{name}"),
                kind: "PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "declared_name".to_string(),
                    Value::String(name.to_string()),
                )]),
            }],
        }
    }
}
