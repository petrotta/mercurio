# Releasing Mercurio Foundation

`mercurio-foundation` is the repository's only public release unit and its only
publishable Cargo package. The focused implementation ships as modules inside
that crate.

Packages with the former internal crate names are `publish = false`
compatibility shims. They are tested as workspace members but are never
packaged or published. Existing crates.io releases of those names are historical
artifacts: leave them available, do not publish new versions, and do not yank
them except for a separate security or legal reason.

The release version comes from `[workspace.package]` and is inherited by the
`mercurio-foundation` manifest.

## Qualification

From this repository:

```powershell
cargo test --workspace --locked
cargo doc --package mercurio-foundation --all-features --no-deps --locked
cargo package --package mercurio-foundation --locked
cargo publish --dry-run --package mercurio-foundation --locked
```

CI also inspects `cargo metadata` and fails unless `mercurio-foundation` is the
complete publishable package set.
It then extracts the generated crate archive and runs the full all-features
test suite against that standalone artifact so bundled resources cannot be omitted.

From the sibling `mercurio-sysml` repository:

```powershell
cargo run -p mercurio-tools --bin check_repo_boundaries -- --manifest ..\mercurio-foundation\repo-boundaries.json --strict
```

## Release Prerequisites

1. Configure crates.io Trusted Publishing for the
   `mercurio-labs/mercurio-foundation` GitHub repository, or provide a scoped
   `CARGO_REGISTRY_TOKEN` Actions secret.
2. Protect the optional `crates-io` GitHub environment if release approval is
   desired.
3. Merge the qualified release commit to `main`.
4. Create and push `foundation-v<version>`, for example
   `foundation-v0.86.0`.

The release workflow validates that the tag matches the workspace version,
repeats qualification, and publishes only:

```text
mercurio-foundation
```

The workflow is resumable. If that exact version is already visible on
crates.io, it exits successfully; otherwise it retries publication while the
registry settles.

Do not publish Foundation and SysML concurrently. Start a SysML release only
after its required `mercurio-foundation` version is visible on crates.io.
