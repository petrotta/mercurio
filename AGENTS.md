# mercurio-foundation — Agent Orientation

Source-language-neutral KIR substrate, graph, runtime, and simulation core. This is the **innermost workspace** — all other Mercurio workspaces depend on it; it depends on nothing Mercurio-specific.

---

## Public Package and Internal Modules

`mercurio-foundation` is the repository's only publishable Cargo package. The
architecture remains split into focused modules inside that package: `kir`,
`language_contracts`, `model`, `runtime`, `authoring`, `semantic_services`,
`workspace`, `analysis`, `query_dsl`, `codegen`, `session`, `simulation_core`,
and `views`.

The former package directories remain workspace members only as
`publish = false` compatibility shims for source-tree consumers during the
transition. They reexport the canonical `mercurio-foundation` APIs and must not
acquire new implementation code or be published again.

Key file locations:

```
crates/mercurio-foundation/src/modules/kir/ — KIR schema and validation
crates/mercurio-foundation/src/modules/model/graph.rs — graph projection and traversal API
crates/mercurio-foundation/src/modules/runtime/ — runtime construction and semantic queries
crates/mercurio-foundation/src/modules/language_contracts/ — diagnostics, LanguageService trait
crates/mercurio-foundation/src/facade/ — root-level compatibility facade
```

---

## Forbidden Dependencies

The `mercurio-foundation` package and its internal modules must **never** import
from:

```
mercurio-adapter    mercurio-ai           mercurio-console-api
mercurio-plugin-api mercurio-product      mercurio-python
mercurio-reasoner-api mercurio-reasoner-host
mercurio-reasoning-services mercurio-views mercurio-wasm
```

After any `Cargo.toml` change, run:

```powershell
cargo run --manifest-path ..\mercurio-sysml\Cargo.toml -p mercurio-tools --bin check_repo_boundaries -- --manifest repo-boundaries.json
cargo run --manifest-path ..\mercurio-sysml\Cargo.toml -p mercurio-tools --bin check_repo_boundaries -- --manifest repo-boundaries.json --strict
```

The machine-readable constraints live in [`repo-boundaries.json`](repo-boundaries.json).

---

## WASM Portability

Foundation modules must compile to `wasm32-unknown-unknown` without changes. Avoid `std::fs`, `std::thread`, system time, or OS-specific APIs. Abstract I/O behind trait boundaries.

---

## Build & Test

```powershell
cargo build
cargo test
cargo test --no-run      # compile-only smoke check
```

---

## Key Constraints

- The `runtime` module must remain deterministic — no randomness, wall-clock reads, or I/O in core evaluation paths.
- The `kir` module is a **stable data contract** — adding `Option` fields is safe; removing or renaming fields breaks all consumers.
- KIR `kind` values must correspond to KerML/SysML v2 metaclass names — never invent proprietary kinds.
- Do not add concrete language parsers or version-specific metamodel bundles here; those belong in `mercurio-sysml`.

---

## Further Reading

- [docs/crates.md](docs/crates.md) — package and module responsibilities
- [docs/kir.md](docs/kir.md) — KIR format and schema
- [docs/philosophy.md](docs/philosophy.md) — design philosophy and boundary rationale
- [docs/language-services.md](docs/language-services.md) — `LanguageService` contract
- [repo-boundaries.json](repo-boundaries.json) — machine-readable dependency constraints
