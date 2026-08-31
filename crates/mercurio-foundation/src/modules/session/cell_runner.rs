use std::collections::BTreeMap;
use std::fmt;

use crate::analysis::capability::{CapabilityRunReport, CapabilityRunStatus};
use serde_json::Value;

use crate::session::{
    CellKind, CellLanguage, CellOutput, CellOutputKind, CellRunReport, CellRunRequest,
    CellRunStatus,
};

pub const DSL_QUERY_MIME_TYPE: &str = "application/vnd.mercurio.dsl.query+json";
pub const DSL_ACTION_PREVIEW_MIME_TYPE: &str = "application/vnd.mercurio.dsl.action-preview+json";
pub const CAPABILITY_RUN_MIME_TYPE: &str = "application/vnd.mercurio.capability-run+json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslCellAnalysisRequest {
    pub query: String,
    pub run_id: Option<String>,
    pub capability_id: Option<String>,
    pub subject_element_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonCellScriptRequest {
    pub source: String,
    pub stdin: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonCellScriptOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub ok: bool,
}

pub struct CellRunHandlers<
    Query,
    Action,
    DslAnalysis,
    SysmlAnalysis,
    PythonScript,
    Unsupported,
    Error,
> where
    Query: FnMut(String) -> Result<Value, Error>,
    Action: FnMut(String) -> Result<Value, Error>,
    DslAnalysis: FnMut(DslCellAnalysisRequest) -> Result<CapabilityRunReport, Error>,
    SysmlAnalysis: FnMut(String) -> Result<CapabilityRunReport, Error>,
    PythonScript: FnMut(PythonCellScriptRequest) -> Result<PythonCellScriptOutput, Error>,
    Unsupported: FnMut(UnsupportedCellRun) -> Error,
{
    pub dsl_query: Query,
    pub dsl_action: Action,
    pub dsl_analysis: DslAnalysis,
    pub sysml_analysis: Option<SysmlAnalysis>,
    pub python_script: Option<PythonScript>,
    pub unsupported: Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedCellRun {
    pub kind: CellKind,
    pub language: Option<CellLanguage>,
}

impl fmt::Display for UnsupportedCellRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported cell kind/language combination: kind={:?} language={:?}",
            self.kind, self.language
        )
    }
}

impl std::error::Error for UnsupportedCellRun {}

pub fn run_cell_with_handlers<
    Query,
    Action,
    DslAnalysis,
    SysmlAnalysis,
    PythonScript,
    Unsupported,
    Error,
>(
    request: CellRunRequest,
    mut handlers: CellRunHandlers<
        Query,
        Action,
        DslAnalysis,
        SysmlAnalysis,
        PythonScript,
        Unsupported,
        Error,
    >,
) -> Result<CellRunReport, Error>
where
    Query: FnMut(String) -> Result<Value, Error>,
    Action: FnMut(String) -> Result<Value, Error>,
    DslAnalysis: FnMut(DslCellAnalysisRequest) -> Result<CapabilityRunReport, Error>,
    SysmlAnalysis: FnMut(String) -> Result<CapabilityRunReport, Error>,
    PythonScript: FnMut(PythonCellScriptRequest) -> Result<PythonCellScriptOutput, Error>,
    Unsupported: FnMut(UnsupportedCellRun) -> Error,
{
    let cell_id = request
        .cell_id
        .clone()
        .unwrap_or_else(|| default_cell_id(&request));
    match (&request.kind, request.language.as_ref()) {
        (CellKind::Query, None | Some(CellLanguage::MercurioDsl)) => {
            let value = (handlers.dsl_query)(request.source)?;
            Ok(CellRunReport {
                session_id: request.session_id,
                cell_id,
                kind: CellKind::Query,
                status: CellRunStatus::Passed,
                outputs: vec![CellOutput {
                    id: "result".to_string(),
                    kind: CellOutputKind::Table,
                    mime_type: Some(DSL_QUERY_MIME_TYPE.to_string()),
                    value,
                }],
                artifacts: Vec::new(),
                diagnostics: Vec::new(),
                capability_report: None,
                metadata: BTreeMap::new(),
            })
        }
        (CellKind::Action, None | Some(CellLanguage::MercurioDsl)) => {
            let value = (handlers.dsl_action)(request.source)?;
            Ok(CellRunReport {
                session_id: request.session_id,
                cell_id,
                kind: CellKind::Action,
                status: CellRunStatus::Passed,
                outputs: vec![CellOutput {
                    id: "result".to_string(),
                    kind: CellOutputKind::Json,
                    mime_type: Some(DSL_ACTION_PREVIEW_MIME_TYPE.to_string()),
                    value,
                }],
                artifacts: Vec::new(),
                diagnostics: Vec::new(),
                capability_report: None,
                metadata: BTreeMap::new(),
            })
        }
        (CellKind::Analysis, None | Some(CellLanguage::MercurioDsl)) => {
            let report = (handlers.dsl_analysis)(DslCellAnalysisRequest {
                query: request.source,
                run_id: string_parameter(&request.parameters, "runId", "run_id"),
                capability_id: string_parameter(
                    &request.parameters,
                    "capabilityId",
                    "capability_id",
                ),
                subject_element_id: string_parameter(
                    &request.parameters,
                    "subjectElementId",
                    "subject_element_id",
                ),
            })?;
            capability_cell_report(request.session_id, cell_id, report)
        }
        (CellKind::Analysis, Some(CellLanguage::Sysml)) => {
            let Some(sysml_analysis) = handlers.sysml_analysis.as_mut() else {
                return Err((handlers.unsupported)(UnsupportedCellRun {
                    kind: request.kind,
                    language: request.language,
                }));
            };
            let report = sysml_analysis(request.source)?;
            capability_cell_report(request.session_id, cell_id, report)
        }
        (CellKind::Script, Some(CellLanguage::Python)) => {
            let Some(python_script) = handlers.python_script.as_mut() else {
                return Err((handlers.unsupported)(UnsupportedCellRun {
                    kind: request.kind,
                    language: request.language,
                }));
            };
            let output = python_script(PythonCellScriptRequest {
                source: request.source,
                stdin: string_parameter(&request.parameters, "stdin", "stdin"),
                timeout_ms: u64_parameter(&request.parameters, "timeoutMs", "timeout_ms"),
            })?;
            let mut metadata = BTreeMap::new();
            metadata.insert("timedOut".to_string(), Value::Bool(output.timed_out));
            metadata.insert(
                "exitCode".to_string(),
                serde_json::to_value(output.exit_code).unwrap_or(Value::Null),
            );
            Ok(CellRunReport {
                session_id: request.session_id,
                cell_id,
                kind: CellKind::Script,
                status: if output.ok {
                    CellRunStatus::Passed
                } else {
                    CellRunStatus::Failed
                },
                outputs: vec![
                    CellOutput {
                        id: "stdout".to_string(),
                        kind: CellOutputKind::Stdout,
                        mime_type: Some("text/plain".to_string()),
                        value: Value::String(output.stdout),
                    },
                    CellOutput {
                        id: "stderr".to_string(),
                        kind: CellOutputKind::Stderr,
                        mime_type: Some("text/plain".to_string()),
                        value: Value::String(output.stderr),
                    },
                ],
                artifacts: Vec::new(),
                diagnostics: Vec::new(),
                capability_report: None,
                metadata,
            })
        }
        _ => Err((handlers.unsupported)(UnsupportedCellRun {
            kind: request.kind,
            language: request.language,
        })),
    }
}

pub fn default_cell_id(request: &CellRunRequest) -> String {
    match (&request.kind, request.language.as_ref()) {
        (CellKind::Query, None | Some(CellLanguage::MercurioDsl)) => "dsl.query".to_string(),
        (CellKind::Action, None | Some(CellLanguage::MercurioDsl)) => "dsl.action".to_string(),
        (CellKind::Analysis, None | Some(CellLanguage::MercurioDsl)) => "dsl.analysis".to_string(),
        (CellKind::Script, Some(CellLanguage::Python)) => "python.script".to_string(),
        _ => "cell".to_string(),
    }
}

pub fn cell_status_from_capability(status: CapabilityRunStatus) -> CellRunStatus {
    match status {
        CapabilityRunStatus::Passed => CellRunStatus::Passed,
        CapabilityRunStatus::Failed => CellRunStatus::Failed,
        CapabilityRunStatus::Error => CellRunStatus::Error,
        CapabilityRunStatus::Inconclusive
        | CapabilityRunStatus::Partial
        | CapabilityRunStatus::NotApplicable => CellRunStatus::Partial,
    }
}

pub fn string_parameter(
    parameters: &BTreeMap<String, Value>,
    camel_key: &str,
    snake_key: &str,
) -> Option<String> {
    parameters
        .get(camel_key)
        .or_else(|| parameters.get(snake_key))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub fn u64_parameter(
    parameters: &BTreeMap<String, Value>,
    camel_key: &str,
    snake_key: &str,
) -> Option<u64> {
    parameters
        .get(camel_key)
        .or_else(|| parameters.get(snake_key))
        .and_then(Value::as_u64)
}

fn capability_cell_report<Error>(
    session_id: Option<String>,
    cell_id: String,
    report: CapabilityRunReport,
) -> Result<CellRunReport, Error> {
    Ok(CellRunReport {
        session_id,
        cell_id,
        kind: CellKind::Analysis,
        status: cell_status_from_capability(report.status),
        outputs: vec![CellOutput {
            id: "capability_report".to_string(),
            kind: CellOutputKind::CapabilityReport,
            mime_type: Some(CAPABILITY_RUN_MIME_TYPE.to_string()),
            value: serde_json::to_value(&report).unwrap_or(Value::Null),
        }],
        artifacts: report.artifacts.clone(),
        diagnostics: report.diagnostics.clone(),
        capability_report: Some(report),
        metadata: BTreeMap::new(),
    })
}
