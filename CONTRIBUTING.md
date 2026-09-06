# Contributing

Thank you for helping improve the Stack Engine. Keep native Rust and browser WebAssembly behavior aligned, and include regression coverage for observable changes.

## Development setup

The workspace uses Rust 2024 with Rust 1.85 as its minimum supported version. Browser package work also requires Node.js and `wasm-bindgen`.

```sh
rustup target add wasm32-unknown-unknown wasm32-wasip1
npm ci
```

## Quality gates

Run the checks relevant to your change, then the full repository suite before opening a pull request:

```sh
cargo test --workspace
STACK_SPECIFICATION_DIR=../specification cargo test -p stack-formatter --features conformance --test conformance
STACK_SPECIFICATION_DIR=../specification cargo test -p stack-engine --features conformance
python3 scripts/validate-svg.py
cargo build -p stack-engine-wasm --target wasm32-unknown-unknown
npm run layout:validate
npm run build:wasm
npm test
npm run typecheck
npm run pack:check
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p stack-engine --lib --target wasm32-wasip1 cross_target_numeric_fixture
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
```

CI includes the repository-only conformance corpus in its 95% line, function, and region coverage gates. Packaged crate unit tests remain independent of the repository corpus.

## Layout changes

The versioned layout corpus covers small, medium, and dense diagrams; groups and nested groups; authored rank and order; cross-boundary edges; labels; and caller-owned provider icons.

```sh
cargo test -p stack-engine --test layout_corpus --locked
cargo test --release -p stack-engine --test layout_corpus layout_runtime_stays_within_budget --locked -- --ignored --nocapture
cargo test --release -p stack-engine --test language_intelligence language_intelligence_runtime_stays_within_budget --locked -- --ignored --nocapture
npm run layout:gallery
```

Follow the [review-first snapshot policy](./layout-corpus/README.md). Set `UPDATE_STACK_SNAPSHOTS=1` or `UPDATE_STACK_LAYOUT_SNAPSHOTS=1` only when intentionally regenerating a reviewed reference, and inspect the resulting SVGs before committing them.

## Releases

Public Cargo and npm artifacts are immutable and must come from an exact reviewed commit. Follow [RELEASING.md](./RELEASING.md) and the [Cargo release procedure](./docs/cargo-releasing.md); do not overwrite an existing tag or package version.

Keep changes focused, use English commit and pull request descriptions, and do not commit credentials, tokens, customer data, signing material, build output, or downloaded runtimes.
