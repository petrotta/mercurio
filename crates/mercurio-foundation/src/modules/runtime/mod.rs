//! Runtime evaluation and datalog APIs.
//!
//! Prefer the root-level re-exports as the supported API. Implementation
//! modules remain public for compatibility, but are hidden from rustdoc.

#[doc(hidden)]
mod datalog;
#[doc(hidden)]
mod runtime;

pub use datalog::{
    Atom, CORE_RULEPACK_ID, CORE_RULEPACK_VERSION, DatalogError, DerivedIndexes, DiagnosticRule,
    Evaluation, Explanation, Fact, Rule, RuleDiagnostic, RuleDiagnosticSeverity, RulePack, Term,
    evaluate, evaluate_diagnostics, extract_graph_facts, load_default_rulepacks,
    materialize_core_indexes,
};
pub use runtime::{
    ExecutionContext, LayeredRuntime, LayeredRuntimeAssembly, QueryResult, Runtime,
    RuntimeArtifact, RuntimeBase, RuntimeError, RuntimeOverlay, RuntimeProfile,
    RuntimeProfileTimings,
};
