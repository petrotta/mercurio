# Mercurio Foundation

Mercurio Foundation is the KerML-aligned, source-language-neutral modeling substrate for Mercurio.

It stores models as KIR, projects them into graphs, runs deterministic semantic services, and exposes contracts that source-language repositories can implement. It does not own concrete source syntax, versioned KerML/SysML metamodel bundles, command-line host behavior, UI adapters, or product workflows.

Mercurio is also a semantic operations framework for AI-assisted systems engineering. Rather than treating models as static files or editor-only artifacts, Mercurio represents engineering systems as source-linked semantic workspaces: queryable, explainable, validated, transactional, and auditable. This lets humans, tools, and AI agents operate over the same model substrate.

In that centaur-style workflow, Foundation provides the generic workspace services: stable model identity and source spans, semantic graph and derived features, validation and diagnostics, semantic diff and workspace revisioning, change-set previews, host-authorized transactions, and evidence-producing reasoning services. Domain repositories declare their own semantic capabilities on top of this substrate, with SysML as the first rich profile.

## Philosophy

Foundation is meant to work like a small reflective modeling core:

- models are persisted as structured semantic data,
- model elements are inspectable without a parser,
- references become graph edges,
- runtime services operate over the graph,
- source languages plug in through explicit services,
- hosts decide how models are edited, stored, packaged, and presented.

The current foundation vocabulary is intentionally aligned with the OMG Kernel Modeling Language (KerML) concepts of packages, types, features, definitions, usages, specialization, typing, and ownership. Foundation should not import a concrete KerML parser or bundled version-specific KerML library; those belong in language repositories and packages.

See [Foundation Philosophy](docs/philosophy.md) for the longer version, including a short comparison to classic modeling-framework responsibilities.

## Core Terms

- **KIR**: the kernel interchange representation. KIR is the validated JSON model format consumed by graph, runtime, query, package, and adapter APIs.
- **Element**: a KIR node with an `id`, `kind`, and `properties`; semantic layer is derived by readers.
- **Graph**: a relationship view derived from KIR reference properties.
- **Runtime**: deterministic evaluation over a graph, derived indexes, expression IR, and rulepacks.
- **Language service**: a registered compiler boundary that turns source text into KIR.
- **KerML alignment**: the foundation semantic vocabulary follows the shape of the OMG Kernel Modeling Language while leaving version-specific metamodel packages outside foundation.

See [KIR](docs/kir.md), [Language Services](docs/language-services.md), and the [Sample Language](docs/sample-language.md).

## Package and Modules

`mercurio-foundation` is this repository's only public, publishable Cargo
package. Add it as the sole Foundation dependency:

```toml
[dependencies]
mercurio-foundation = "0.86"
```

The implementation remains organized by responsibility as modules within that
package. Consumers can use the root facade or focused paths such as
`mercurio_foundation::kir`, `mercurio_foundation::runtime`, and
`mercurio_foundation::semantic_services`.

The workspace still contains packages with the former internal crate names.
They are `publish = false` compatibility shims for source-tree consumers; they
do not define independent release units and must not receive new implementation
code. Existing crates.io versions remain available but are frozen.

See [Package and Module Architecture](docs/crates.md).

## Boundary Check

The repository boundary manifest lives at [repo-boundaries.json](repo-boundaries.json). The checker is
hosted in the SysML tooling workspace so it can audit foundation policy while remaining outside the
foundation crate set:

```powershell
cargo run --manifest-path ..\mercurio-sysml\Cargo.toml -p mercurio-tools --bin check_repo_boundaries -- --manifest repo-boundaries.json
cargo run --manifest-path ..\mercurio-sysml\Cargo.toml -p mercurio-tools --bin check_repo_boundaries -- --manifest repo-boundaries.json --strict
```

## Quick Example

```rust
use std::collections::BTreeMap;

use mercurio_foundation::{Graph, KIR_SCHEMA_VERSION, KirDocument, KirElement};
use serde_json::json;

let document = KirDocument {
    metadata: BTreeMap::from([
        ("kir_schema_version".to_string(), json!(KIR_SCHEMA_VERSION)),
    ]),
    elements: vec![KirElement {
        id: "pkg.Demo".to_string(),
        kind: "model.Package".to_string(),
        properties: BTreeMap::from([
            ("qualified_name".to_string(), json!("Demo")),
            ("declared_name".to_string(), json!("Demo")),
        ]),
    }],
};

document.validate()?;
let graph = Graph::from_document(document)?;
```

More snippets are in [Examples](docs/examples.md).

Large-model timing and memory checks are documented in [Performance](docs/performance.md).

## Build

```powershell
cargo build
```

## Test

```powershell
cargo test --no-run
```

Foundation tests use language-neutral KIR fixtures and a small test-only toy language service for registry, cache, graph, and runtime coverage.

## Release

Mercurio Foundation is one release unit. Only the `mercurio-foundation` package
is published; internal modules ship as part of that crate, and compatibility
shim packages remain unpublished.

See [Releasing Mercurio Foundation](RELEASING.md).
