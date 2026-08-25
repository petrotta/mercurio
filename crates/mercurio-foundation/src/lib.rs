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

#[cfg(test)]
mod tests {
    use super::{Graph, Runtime};

    #[test]
    fn facade_exposes_model_and_runtime_types() {
        fn accepts_public_types(_: Option<Graph>, _: Option<Runtime>) {}

        accepts_public_types(None, None);
    }
}
