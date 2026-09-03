# Stack Engine

`stack-sh/engine` is the pure Rust execution engine for Stack architecture diagrams.

The workspace now provides canonical Stack source formatting, the pure `stack-engine` operation facade, deterministic theme-aware scene layout and orthogonal edge routing, and safe standalone SVG rendering. A WebAssembly adapter remains planned work.

## Workspace

- `stack-engine`: operation/output boundary, theme and icon fallback resolution, deterministic scene layout, edge routing, validation beyond the compiler stage, and standalone SVG rendering;
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
python3 scripts/validate-svg.py
cargo build -p stack-engine --target wasm32-unknown-unknown
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p stack-engine --lib --target wasm32-wasip1 cross_target_numeric_fixture
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
```

`stack-formatter` is pure and accepts source bytes or UTF-8 text. Lexical and syntax errors return diagnostics without formatted output. Syntactically valid source remains formattable when semantic diagnostics exist.

`stack-engine` exposes byte-oriented `format`, `check`, and `render` methods through an engine bound to the embedded or a caller-provided validated catalog. Every normal output carries engine, authored language, theme catalog version, and theme catalog revision metadata. User-source failures stay in ordered portable diagnostics. Invalid provided catalogs and violated normalized pipeline invariants use a separate operational-error channel. Checks and renders resolve the requested theme, validate deterministic integer geometry, and route ordered edges outside node interiors. Missing themes and icons produce source-mapped `STK6001` and `STK5001` warnings while a fallback SVG remains available. An unsatisfied authored order hint produces `STK4001` at its source-map range; a satisfied hint does not.

The renderer emits fixed-dimension standalone SVG with embedded catalog icons, local marker references, escaped authored text, accessible title and description metadata, and no script, event handler, external URL, host font measurement, or runtime I/O. Canonical SVG snapshots are byte-stable and parsed by `scripts/validate-svg.py`; set `UPDATE_STACK_SNAPSHOTS=1` only when intentionally regenerating them. CI also executes one exact numeric geometry fixture in both the native suite and a WASI build.

## Architecture

- [`docs/decisions/0001-build-the-formatter-from-compiler-models.md`](./docs/decisions/0001-build-the-formatter-from-compiler-models.md)
- [`docs/decisions/0002-use-a-pure-versioned-engine-facade.md`](./docs/decisions/0002-use-a-pure-versioned-engine-facade.md)
- [`docs/decisions/0003-use-integer-ranked-scene-layout.md`](./docs/decisions/0003-use-integer-ranked-scene-layout.md)
- [`docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md`](./docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md)
- [`docs/decisions/0005-serialize-safe-standalone-svg.md`](./docs/decisions/0005-serialize-safe-standalone-svg.md)
- [`docs/dependency-audit.md`](./docs/dependency-audit.md)

## Licensing

This repository is licensed under the [Apache License 2.0](./LICENSE). Third-party dependencies or bundled assets must be tracked in [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) before a distributable native or WASM artifact is published.
