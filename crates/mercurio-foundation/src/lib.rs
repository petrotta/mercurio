//! Mercurio's language-neutral semantic modeling foundation.
//!
//! `mercurio-foundation` is the only publishable package in this repository.
//! Focused modules preserve the implementation boundaries that were previously
//! separate Cargo packages. The curated root facade remains the recommended
//! integration surface and preserves the existing `mercurio-core` API while
//! consumers migrate to this package.

mod modules;

pub use modules::{
    analysis, authoring, codegen, kir, language_contracts, model, query_dsl, runtime,
    semantic_services, session, simulation_core, views, workspace,
};

mod facade;

pub use facade::*;

#[doc(hidden)]
pub use analysis::kir_canonical;
pub use analysis::kir_canonical::{
    CanonicalizedKir, KIR_EQUIVALENCE_REPORT_SCHEMA_VERSION, KirEquivalenceReport,
    canonicalize_kir_document, kir_documents_equivalent, kir_equivalence_diff,
    kir_equivalence_report, semantic_diff_is_empty,
};

#[cfg(test)]
mod tests {
    use super::{Graph, Runtime};

    #[test]
    fn facade_exposes_model_and_runtime_types() {
        fn accepts_public_types(_: Option<Graph>, _: Option<Runtime>) {}

        accepts_public_types(None, None);
    }
}
