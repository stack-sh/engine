# Dependency and host-I/O audit

Audit date: 2026-09-04

## Runtime graph

`stack-engine` has three direct dependencies:

- `stack-compiler` at `3d2379483da1edaeb24a26d43743587a4f5bd645` for byte decoding, parsing, validation, normalized IR, source maps, and compiler diagnostics;
- the workspace-local `stack-formatter` for canonical source output;
- `stack-theme` at `d25b883884420adcc124e4c9c786ad92925eae60` for the `0.2.0` embedded core catalog, 12 provider-neutral explicit icons, SVG bytes, deterministic font metrics, catalog version, and catalog revision.

`stack-engine-wasm` adds `serde` for its serializable native parity model and, only on `wasm32`, version-matched `wasm-bindgen` and `js-sys` for the JavaScript ABI, typed-array input, and plain object construction. It does not use `web-sys` or a WASI target.

The resolved normal dependency graph adds only the `serde` and `serde_json` graph required by `stack-theme`. Exact versions and licenses are recorded in [`THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md) and pinned in `Cargo.lock`. Scene layout is implemented locally with fixed-width integer arithmetic and versioned catalog metrics. Standalone SVG serialization is also local and embeds only validated catalog icon bodies and local marker references. No third-party layout or SVG serializer, filesystem, network, asynchronous runtime, random, clock, locale, DOM, or platform-font dependency is present.

## Runtime access boundary

- `stack-engine` accepts source bytes plus a borrowed validated catalog and revision, or selects the catalog already embedded by `stack-theme`.
- `stack-compiler` and `stack-formatter` operate entirely on caller-owned bytes and in-memory values.
- `stack-theme` embeds catalog, schema, and SVG bytes at compile time and parses the trusted generated catalog through an in-memory singleton.
- No runtime dependency discovers a path, opens a socket, reads process state, observes time, samples randomness, queries a DOM, or measures a system font.
- The generated WebAssembly imports only the audited `wasm-bindgen` object, string, array, typed-array, exception, and extern-reference glue from its sibling JavaScript module. The import validator rejects WASI and names associated with filesystem, network, DOM, clock, random, process, environment, or storage capabilities.

Tests and CI may read the pinned specification checkout and invoke toolchains. Those development actions are outside the runtime library boundary.

## Reproduction

```sh
cargo tree -p stack-engine --edges normal --locked
cargo metadata --format-version 1 --locked
cargo test --workspace --locked
STACK_SPECIFICATION_DIR=../specification cargo test -p stack-engine --features conformance --locked
python3 scripts/validate-svg.py
cargo build -p stack-engine-wasm --target wasm32-unknown-unknown --release --locked
npm ci
npm run build:wasm
npm test
npm run typecheck
npm run pack:check
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p stack-engine --lib --target wasm32-wasip1 cross_target_numeric_fixture --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```
