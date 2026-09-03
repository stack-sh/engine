# Stack Engine

`stack-sh/engine` is the pure Rust execution engine for Stack architecture diagrams.

The workspace now provides canonical Stack source formatting, the pure `stack-engine` operation facade, deterministic theme-aware scene layout, and orthogonal edge routing. SVG rendering and WebAssembly adapters remain planned work.

## Planned workspace

- `stack-engine`: implemented operation/output boundary, theme resolution, deterministic scene layout, edge routing, and validation beyond the compiler stage, plus planned standalone SVG rendering;
- `stack-formatter`: comment-preserving canonical formatting for Stack source files (implemented);
- a WebAssembly adapter exposing the same pure operations to browser consumers.

The native CLI will link the Rust engine directly. Web clients will use the WASM adapter. Shared fixtures will verify that both targets produce equivalent diagnostics, formatted source, and SVG output.

## Boundaries

The engine may depend on `stack-sh/compiler` and `stack-sh/theme`. Core operations must be deterministic and must not require filesystem, environment, clock, random, or network access.

CLI filesystem behavior, process exit codes, user authentication, billing, entitlement checks, and paid-theme delivery are outside this repository.

## Development

The workspace uses Rust 2024 with Rust 1.85 as its minimum supported version. Run:

```sh
cargo test --workspace
STACK_SPECIFICATION_DIR=../specification cargo test -p stack-formatter --features conformance --test conformance
STACK_SPECIFICATION_DIR=../specification cargo test -p stack-engine --features conformance
cargo build -p stack-engine --target wasm32-unknown-unknown
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p stack-engine --lib --target wasm32-wasip1 cross_target_numeric_fixture
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
```

`stack-formatter` is pure and accepts source bytes or UTF-8 text. Lexical and syntax errors return diagnostics without formatted output. Syntactically valid source remains formattable when semantic diagnostics exist.

`stack-engine` exposes byte-oriented `format`, `check`, and reserved `render` methods through an engine bound to the embedded or a caller-provided validated catalog. Every normal output carries engine, authored language, theme catalog version, and theme catalog revision metadata. User-source failures stay in ordered portable diagnostics. Invalid provided catalogs, invalid normalized containment, routing failure, and unavailable pipeline stages use a separate operational-error channel. Checks and compiler-valid render attempts resolve the requested theme, validate deterministic integer geometry, and route ordered edges outside node interiors. An unsatisfied authored order hint produces `STK4001` at its source-map range; a satisfied hint does not. Rendering remains unavailable until SVG integration lands. CI executes one exact numeric geometry fixture in both the native suite and a WASI build.

## Architecture

- [`docs/decisions/0001-build-the-formatter-from-compiler-models.md`](./docs/decisions/0001-build-the-formatter-from-compiler-models.md)
- [`docs/decisions/0002-use-a-pure-versioned-engine-facade.md`](./docs/decisions/0002-use-a-pure-versioned-engine-facade.md)
- [`docs/decisions/0003-use-integer-ranked-scene-layout.md`](./docs/decisions/0003-use-integer-ranked-scene-layout.md)
- [`docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md`](./docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md)
- [`docs/dependency-audit.md`](./docs/dependency-audit.md)

## Licensing

This repository is licensed under the [Apache License 2.0](./LICENSE). Third-party dependencies or bundled assets must be tracked in [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) before a distributable native or WASM artifact is published.
