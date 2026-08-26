# Package and Module Architecture

## Publication Model

`mercurio-foundation` is the only public, publishable Cargo package in this
repository. All source-language-neutral Foundation implementation ships in that
crate. Consumers should depend on it directly and import either the root facade
or a focused module path:

```rust
use mercurio_foundation::{Graph, KirDocument};
use mercurio_foundation::runtime::RuntimeArtifact;
```

The focused architecture is preserved as modules under
`crates/mercurio-foundation/src/modules/`. Module boundaries still define
ownership and dependency direction even though crates.io sees one package.

The workspace retains packages with the former internal crate names as
`publish = false` compatibility shims. They support source-tree migrations and
repository-only tools by reexporting canonical APIs from `mercurio-foundation`.
They are not independent products, must not gain new implementation ownership,
and must not be published again. Historical crates.io versions remain available
but frozen.

## Root Facade

The `mercurio_foundation` crate root provides the recommended compatibility
facade. Small cross-module glue such as the KIR model-stack loader, logging
helpers, proposal helpers, and semantic-target resolution lives under
`src/facade/`. New subsystem behavior belongs in the focused module that owns
the corresponding noun.

## Module Ownership

### `kir`

Owns the stable KIR data contract:

- `KirDocument` and `KirElement`,
- schema version constants,
- validation diagnostics,
- KIR merge and IO,
- field registry metadata used to classify references.

This module should stay small and stable. Adding optional fields is generally
compatible; removing or renaming contract fields breaks consumers.

### `language_contracts`

Owns contracts that language-specific repositories implement:

- lexical and parsed-module data structures used by shared tooling,
- diagnostics and parse/compile reports,
- `LanguageService` and `LanguageRegistry`,
- expression IR shared by runtime and language compilers.

It defines the language boundary without depending on a concrete language.

### `model`

Owns source-language-neutral model structures and graph projection:

- graph projection from KIR,
- metamodel and metadata views,
- derived-model primitives,
- expression evaluation primitives shared by runtime services.

It must not contain source parsing, host behavior, UI behavior, AI
orchestration, or plugin-host contracts.

### `runtime`

Owns deterministic runtime services over graph artifacts:

- runtime construction from KIR or graph artifacts,
- derived indexes and rulepack materialization,
- semantic queries,
- expression IR evaluation,
- runtime artifacts and profiling.

Evaluation paths must not use randomness, wall-clock reads, or IO.

### `authoring`

Owns source-language-neutral authoring services:

- source sets and language registry integration,
- source-preserving semantic edits,
- generated companion-file fallback edits,
- semantic and editor outlines,
- lightweight frontend helpers and test language support.

Concrete production parsers and version-specific metamodel bundles remain in
language repositories.

### `semantic_services`

Owns semantic operations over KIR, graph, and authoring contexts:

- semantic anchors and workspace revisions,
- mutation plans and semantic diffs,
- feasibility and legality checks,
- semantic validation,
- next-action and variant-preview services.

Language-specific rules enter through profiles, rulepacks, registries, or
explicit host contracts.

### `workspace`

Owns workspace and package infrastructure:

- repository paths and default resource lookup,
- package descriptors and repositories,
- workspace descriptors and resolved contexts,
- model state and revision envelopes,
- persistent compile cache,
- plugin registry helpers,
- local performance harnesses.

Filesystem-aware workspace behavior belongs here; deterministic model
evaluation remains in `runtime`.

### `analysis`

Owns reusable semantic analysis contracts and reports:

- AI review request and feedback contracts,
- semantic assessment and evidence,
- generic inspection and impact capabilities,
- cognitive context and quality goals,
- semantic comparison reports.

It describes analysis over Foundation data without owning AI orchestration,
product workflows, or language-specific lowering.

### `query_dsl`

Owns user-facing query execution surfaces:

- structured query parsing and execution,
- Rhai DSL bindings,
- DSL schemas and reports,
- capability-backed query artifacts.

It consumes model, runtime, session, workspace, and semantic-service APIs
without owning the underlying graph or runtime primitives.

### `codegen`

Owns code-generation and profile helpers:

- language profiles,
- metamodel concept registry,
- library-context helpers,
- Python wrapper and typed-facade generation.

It generates from KIR and profile data without becoming a language compiler.

### `session`

Owns interactive semantic sessions:

- session state,
- forks and overlays,
- host-authorized commit operations,
- transaction reports.

It coordinates authoring, workspace, and semantic services while keeping host
authorization explicit.

### `simulation_core`

Owns source-neutral deterministic simulation primitives:

- event-step simulation over KIR-projected behavior facts,
- guard evaluation through runtime services,
- source-neutral trace evidence.

SysML-specific behavior lowering, library interpretation, and UI naming live in
language or product layers that call this module.

### `views`

Owns source-language-neutral view DTOs and rendering helpers:

- element and model summary views,
- explorer graph DTOs,
- table and diagram view documents,
- deterministic SVG rendering helpers.

## Compatibility Package Map

| Non-publishable package | Canonical API |
|---|---|
| `mercurio-kir` | `mercurio_foundation::kir` |
| `mercurio-language-contracts` | `mercurio_foundation::language_contracts` |
| `mercurio-model` | `mercurio_foundation::model` |
| `mercurio-runtime` | `mercurio_foundation::runtime` |
| `mercurio-authoring` | `mercurio_foundation::authoring` |
| `mercurio-semantic-services` | `mercurio_foundation::semantic_services` |
| `mercurio-workspace` | `mercurio_foundation::workspace` |
| `mercurio-analysis` | `mercurio_foundation::analysis` |
| `mercurio-query-dsl` | `mercurio_foundation::query_dsl` |
| `mercurio-codegen` | `mercurio_foundation::codegen` |
| `mercurio-session` | `mercurio_foundation::session` |
| `mercurio-simulation-core` | `mercurio_foundation::simulation_core` |
| `mercurio-views` | `mercurio_foundation::views` |
| `mercurio-core` | `mercurio_foundation` root facade |
