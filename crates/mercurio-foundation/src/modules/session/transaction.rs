use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::semantic_services::identity::stable_digest;
use crate::semantic_services::mutation::{SemanticDiff, SemanticMutation, WorkspaceRevision};
use crate::session::CommitResult;

pub const SEMANTIC_CHANGE_SET_SCHEMA: &str = "mercurio.semantic_change_set.v1";
pub const SEMANTIC_TRANSACTION_SCHEMA: &str = "mercurio.semantic_transaction.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionIsolation {
    OptimisticRevision,
}

impl Default for TransactionIsolation {
    fn default() -> Self {
        Self::OptimisticRevision
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Planned,
    Previewed,
    Committed,
    RolledBack,
    Rejected,
}

/// Severity for a [`TransactionDiagnostic`]. Alias of the canonical
/// [`crate::kir::Severity`].
pub use crate::kir::Severity as TransactionDiagnosticSeverity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionDiagnostic {
    pub code: String,
    pub severity: TransactionDiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_index: Option<usize>,
}

impl TransactionDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: TransactionDiagnosticSeverity::Error,
            message: message.into(),
            operation_index: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionArtifact {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticChangeSet {
    pub schema: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, alias = "actions")]
    pub operations: Vec<SemanticMutation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl SemanticChangeSet {
    pub fn new(label: impl Into<String>, operations: Vec<SemanticMutation>) -> Self {
        Self::from_optional_label(Some(label.into()), operations)
    }

    pub fn from_operations(operations: Vec<SemanticMutation>) -> Self {
        Self::from_optional_label(None, operations)
    }

    fn from_optional_label(label: Option<String>, operations: Vec<SemanticMutation>) -> Self {
        let id = deterministic_change_set_id(label.as_deref(), &operations);
        Self {
            schema: SEMANTIC_CHANGE_SET_SCHEMA.to_string(),
            id,
            label,
            operations,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransactionOperation {
    ChangeSet {
        change_set: SemanticChangeSet,
    },
    CapabilityRun {
        capability_id: String,
        #[serde(default)]
        parameters: Value,
    },
    BuildTask {
        task: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        dependencies: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        operations: Vec<String>,
    },
    DslScript {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        script_name: Option<String>,
        script_digest: String,
    },
}

impl TransactionOperation {
    pub fn change_set(change_set: SemanticChangeSet) -> Self {
        Self::ChangeSet { change_set }
    }

    pub fn change_set_from_operations(
        label: impl Into<String>,
        operations: Vec<SemanticMutation>,
    ) -> Self {
        Self::change_set(SemanticChangeSet::new(label, operations))
    }

    pub fn capability_run(capability_id: impl Into<String>, parameters: Value) -> Self {
        Self::CapabilityRun {
            capability_id: capability_id.into(),
            parameters,
        }
    }

    pub fn build_task(
        task: impl Into<String>,
        dependencies: Vec<String>,
        operations: Vec<String>,
    ) -> Self {
        Self::BuildTask {
            task: task.into(),
            dependencies,
            operations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticTransaction {
    pub schema: String,
    pub id: String,
    pub label: String,
    pub isolation: TransactionIsolation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<WorkspaceRevision>,
    #[serde(default)]
    pub operations: Vec<TransactionOperation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl SemanticTransaction {
    pub fn new(
        label: impl Into<String>,
        base_revision: Option<WorkspaceRevision>,
        operations: Vec<TransactionOperation>,
    ) -> Self {
        let label = label.into();
        let id = deterministic_transaction_id(&label, base_revision.as_ref(), &operations);
        Self {
            schema: SEMANTIC_TRANSACTION_SCHEMA.to_string(),
            id,
            label,
            isolation: TransactionIsolation::default(),
            base_revision,
            operations,
            metadata: BTreeMap::new(),
        }
    }

    pub fn preview_report(&self, semantic_diff: SemanticDiff) -> SemanticTransactionReport {
        SemanticTransactionReport {
            schema: SEMANTIC_TRANSACTION_SCHEMA.to_string(),
            transaction_id: self.id.clone(),
            label: self.label.clone(),
            status: TransactionStatus::Previewed,
            isolation: self.isolation.clone(),
            base_revision: self.base_revision.clone(),
            new_revision: None,
            operation_count: self.operations.len(),
            operations: self.operations.clone(),
            semantic_diff,
            applied: false,
            diagnostics: Vec::new(),
            artifacts: Vec::new(),
            metadata: self.metadata.clone(),
        }
    }

    pub fn rejected_report(
        &self,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> SemanticTransactionReport {
        let mut report = self.preview_report(SemanticDiff::default());
        report.status = TransactionStatus::Rejected;
        report
            .diagnostics
            .push(TransactionDiagnostic::error(code, message));
        report
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticTransactionReport {
    pub schema: String,
    pub transaction_id: String,
    pub label: String,
    pub status: TransactionStatus,
    pub isolation: TransactionIsolation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<WorkspaceRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_revision: Option<WorkspaceRevision>,
    pub operation_count: usize,
    #[serde(default)]
    pub operations: Vec<TransactionOperation>,
    #[serde(default)]
    pub semantic_diff: SemanticDiff,
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<TransactionDiagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<TransactionArtifact>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl SemanticTransactionReport {
    pub fn from_commit_result(transaction: &SemanticTransaction, result: &CommitResult) -> Self {
        let payload = json!({
            "mode": result.mode,
            "strategy": result.strategy_used,
            "changedFiles": result.changed_files,
            "generatedElements": result.generated_elements,
        });
        Self {
            schema: SEMANTIC_TRANSACTION_SCHEMA.to_string(),
            transaction_id: transaction.id.clone(),
            label: transaction.label.clone(),
            status: TransactionStatus::Committed,
            isolation: transaction.isolation.clone(),
            base_revision: Some(result.base_revision.clone()),
            new_revision: Some(result.new_revision.clone()),
            operation_count: transaction.operations.len(),
            operations: transaction.operations.clone(),
            semantic_diff: result.semantic_diff.clone(),
            applied: true,
            diagnostics: Vec::new(),
            artifacts: vec![TransactionArtifact {
                id: format!("artifact.{}.commit", transaction.id),
                kind: "commit_result".to_string(),
                digest: None,
                payload,
            }],
            metadata: transaction.metadata.clone(),
        }
    }
}

fn deterministic_change_set_id(label: Option<&str>, actions: &[SemanticMutation]) -> String {
    let action_bytes = match serde_json::to_vec(actions) {
        Ok(bytes) => bytes,
        Err(error) => error.to_string().into_bytes(),
    };
    let digest = stable_digest([
        (b"label".as_slice(), label.unwrap_or("unlabeled").as_bytes()),
        (b"actions".as_slice(), action_bytes.as_slice()),
    ]);
    format!("chg.{}", digest.replace(':', "_"))
}

fn deterministic_transaction_id(
    label: &str,
    base_revision: Option<&WorkspaceRevision>,
    operations: &[TransactionOperation],
) -> String {
    let revision = base_revision
        .map(|revision| revision.fingerprint.as_str())
        .unwrap_or("unrevisioned");
    let operation_bytes = match serde_json::to_vec(operations) {
        Ok(bytes) => bytes,
        Err(error) => error.to_string().into_bytes(),
    };
    let digest = stable_digest([
        (b"label".as_slice(), label.as_bytes()),
        (b"base_revision".as_slice(), revision.as_bytes()),
        (b"operations".as_slice(), operation_bytes.as_slice()),
    ]);
    format!("txn.{}", digest.replace(':', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_services::mutation::ElementRef;
    use crate::session::{CommitMode, CommitStrategy};

    #[test]
    fn transaction_id_is_deterministic_for_same_operation_set() {
        let operations = vec![TransactionOperation::change_set_from_operations(
            "rename changes",
            vec![SemanticMutation::RenameDeclaration {
                element: ElementRef::new("Demo.Vehicle"),
                new_name: "Vehicle2".to_string(),
            }],
        )];
        let left = SemanticTransaction::new("rename", None, operations.clone());
        let right = SemanticTransaction::new("rename", None, operations);

        assert_eq!(left.id, right.id);
        assert!(left.id.starts_with("txn.fnv1a64_"));
    }

    #[test]
    fn transaction_edit_operation_is_a_change_set() {
        let operation = TransactionOperation::change_set_from_operations(
            "rename changes",
            vec![SemanticMutation::RenameDeclaration {
                element: ElementRef::new("Demo.Vehicle"),
                new_name: "Vehicle2".to_string(),
            }],
        );
        let encoded = serde_json::to_value(operation).expect("operation serializes");

        assert_eq!(encoded["kind"], Value::String("change_set".to_string()));
        assert_eq!(
            encoded["change_set"]["schema"],
            Value::String(SEMANTIC_CHANGE_SET_SCHEMA.to_string())
        );
        assert_eq!(
            encoded["change_set"]["operations"].as_array().map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn rejected_report_never_marks_transaction_applied() {
        let transaction = SemanticTransaction::new(
            "host-gated",
            None,
            vec![TransactionOperation::capability_run(
                "mercurio.dsl.analysis",
                Value::Null,
            )],
        );

        let report = transaction.rejected_report("HOST_PERMISSION", "commit denied");

        assert_eq!(report.status, TransactionStatus::Rejected);
        assert!(!report.applied);
        assert_eq!(report.diagnostics.len(), 1);
    }

    #[test]
    fn commit_result_converts_to_committed_transaction_report() {
        let transaction = SemanticTransaction::new(
            "commit",
            Some(WorkspaceRevision {
                fingerprint: "base".to_string(),
            }),
            vec![TransactionOperation::change_set_from_operations(
                "commit changes",
                vec![SemanticMutation::RenameDeclaration {
                    element: ElementRef::new("Demo.Vehicle"),
                    new_name: "Vehicle2".to_string(),
                }],
            )],
        );
        let result = CommitResult {
            mode: CommitMode::PreserveSource,
            strategy_used: CommitStrategy::MutatorPlan,
            base_revision: WorkspaceRevision {
                fingerprint: "base".to_string(),
            },
            new_revision: WorkspaceRevision {
                fingerprint: "next".to_string(),
            },
            changed_files: ["vehicle.model".to_string()].into_iter().collect(),
            edited_files: BTreeMap::new(),
            semantic_diff: SemanticDiff::default(),
            generated_elements: 0,
        };

        let report = SemanticTransactionReport::from_commit_result(&transaction, &result);

        assert_eq!(report.status, TransactionStatus::Committed);
        assert!(report.applied);
        assert_eq!(
            report
                .new_revision
                .as_ref()
                .map(|revision| revision.fingerprint.as_str()),
            Some("next")
        );
        assert_eq!(report.artifacts.len(), 1);
    }
}
