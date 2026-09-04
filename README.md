# Stack Engine

`stack-sh/engine` is the pure Rust execution engine for Stack architecture diagrams.

The workspace provides canonical Stack source formatting, the pure `stack-engine` operation facade, deterministic theme-aware scene layout and orthogonal edge routing, safe standalone SVG rendering, and a typed browser WebAssembly adapter.

## Workspace

- `stack-engine`: operation/output boundary, theme and icon fallback resolution, deterministic scene layout, edge routing, validation beyond the compiler stage, and standalone SVG rendering;
- `stack-formatter`: comment-preserving canonical formatting for Stack source files (implemented);
- `stack-engine-wasm` and npm `@stack-sh/engine`: a thin browser adapter exposing the same pure operations and portable result model.

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
rustup target add wasm32-unknown-unknown wasm32-wasip1
cargo build -p stack-engine-wasm --target wasm32-unknown-unknown
wasm-bindgen --version
npm ci
npm run build:wasm
npm test
npm run typecheck
npm run pack:check
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p stack-engine --lib --target wasm32-wasip1 cross_target_numeric_fixture
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
```

`stack-formatter` is pure and accepts source bytes or UTF-8 text. Lexical and syntax errors return diagnostics without formatted output. Syntactically valid source remains formattable when semantic diagnostics exist.

`stack-engine` exposes byte-oriented `format`, `check`, and `render` methods through an engine bound to the embedded or a caller-provided validated catalog. `ProviderPack::new` accepts a typed user-imported manifest and caller-owned SVG strings, verifies exact asset hashes and safe SVG structure, and computes a deterministic content revision before `Engine::with_provider_packs` can resolve namespaced IDs. Every normal output carries engine, authored language, theme catalog version, and theme catalog revision metadata. User-source failures stay in ordered portable diagnostics. Invalid provided catalogs or provider packs and violated normalized pipeline invariants use a separate operational-error channel. Checks and renders resolve the requested theme and provider packs, validate deterministic integer geometry, and route ordered edges outside node interiors. Missing themes and icons produce source-mapped `STK6001` and `STK5001` warnings while a fallback SVG remains available. An unsatisfied authored order hint produces `STK4001` at its source-map range; a satisfied hint does not.

The renderer emits fixed-dimension standalone SVG with embedded catalog or provider icons, local marker references, escaped authored text, accessible title and description metadata, and no script, event handler, external URL, host font measurement, or runtime I/O. Provider artwork preserves the authored node `kind`; each render returns the exact used-asset notices and writes provider ID, icon IDs, and pack revision into SVG metadata. The bundled catalog provides the first-party explicit icon identifiers `api`, `web`, `mobile`, `desktop`, `server`, `container`, `cluster`, `cloud`, `scheduler`, `webhook`, `identity`, and `observability` in every core theme. Canonical SVG snapshots are byte-stable and parsed by `scripts/validate-svg.py`; set `UPDATE_STACK_SNAPSHOTS=1` only when intentionally regenerating them. CI also executes one exact numeric geometry fixture in both the native suite and a WASI build.

The npm package exports synchronous `format`, `check`, `render`, `checkWithProviderPacks`, and `renderWithProviderPacks` functions after asynchronous module initialization. Provider-pack operations accept JSON-compatible local manifest and SVG data; they never discover a path or initiate a request. Each operation accepts `string | Uint8Array` source and returns a specific typed result with camel-case metadata and portable diagnostics. Diagnostics preserve the compiler's primary range, ordered `expected` values, corrective help, and related source locations. Invalid UTF-8 remains a normal `STK1001` result. Unsupported JavaScript input types and internal operational failures throw at the adapter boundary. Shared fixtures exercise native and WebAssembly provider resolution. Artifact validation audits WebAssembly imports and package contents; browser consumers retain responsibility for loading the module and performing any DOM, filesystem, network, or clock work.

Public npm releases are produced from GitHub Releases after the repository checks pass. See [RELEASING.md](./RELEASING.md) for the first-release bootstrap and subsequent trusted-publishing flow.

## Architecture

- [`docs/decisions/0001-build-the-formatter-from-compiler-models.md`](./docs/decisions/0001-build-the-formatter-from-compiler-models.md)
- [`docs/decisions/0002-use-a-pure-versioned-engine-facade.md`](./docs/decisions/0002-use-a-pure-versioned-engine-facade.md)
- [`docs/decisions/0003-use-integer-ranked-scene-layout.md`](./docs/decisions/0003-use-integer-ranked-scene-layout.md)
- [`docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md`](./docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md)
- [`docs/decisions/0005-serialize-safe-standalone-svg.md`](./docs/decisions/0005-serialize-safe-standalone-svg.md)
- [`docs/decisions/0006-expose-one-typed-browser-wasm-adapter.md`](./docs/decisions/0006-expose-one-typed-browser-wasm-adapter.md)
- [`docs/dependency-audit.md`](./docs/dependency-audit.md)

## Licensing

This repository is licensed under the [Apache License 2.0](./LICENSE). Third-party dependencies or bundled assets must be tracked in [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) before a distributable native or WASM artifact is published.
