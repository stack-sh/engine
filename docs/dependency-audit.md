# Dependency and host-I/O audit

Audit date: 2026-09-04

## Runtime graph

`stack-engine` has six direct dependencies:

- `stack-compiler` at `4a18fac42afc2256a1bb3a6ff13d12d732a391e7` for byte decoding, parsing, validation, normalized IR, source maps, and compiler diagnostics;
- the workspace-local `stack-formatter` for canonical source output;
- `stack-theme` at `2347315e6e86ab9d2708e05fd3f9b5f3d87e1241` for the `0.4.0` embedded core catalog, 30 provider-neutral explicit icons, the local-only provider-pack contract, SVG bytes, deterministic font metrics, catalog version, and catalog revision;
- `roxmltree`, `serde_json`, and `sha2` for pure in-memory provider manifest serialization, processed-asset hash verification, pack revision computation, and defensive SVG validation. Vendor asset bytes are not included.

`stack-engine-wasm` adds `serde`, `serde_json`, and the asset-free `stack-theme` types for its serializable native parity model and local provider-pack input, plus, only on `wasm32`, version-matched `wasm-bindgen` and `js-sys` for the JavaScript ABI, typed-array input, JSON-compatible local data, and plain object construction. It does not use `web-sys` or a WASI target.

Exact versions and licenses are recorded in [`THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md) and pinned in `Cargo.lock`. Scene layout is implemented locally with fixed-width integer arithmetic and versioned catalog metrics. Standalone SVG serialization is also local and embeds only validated catalog or caller-owned provider icon bodies and local marker references. No third-party layout or SVG serializer, filesystem, network, asynchronous runtime, random, clock, locale, DOM, or platform-font dependency is present.

## Runtime access boundary

- `stack-engine` accepts source bytes plus a borrowed validated catalog and revision, or selects the catalog already embedded by `stack-theme`. It may also accept validated caller-owned provider manifests and SVG strings; it hashes and validates them entirely in memory.
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
