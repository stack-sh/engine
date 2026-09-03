# Dependency and host-I/O audit

Audit date: 2026-09-03

## Runtime graph

`stack-engine` has three direct dependencies:

- `stack-compiler` at `17a0abe9c35e641761ff08fdf59b29a42828d9fd` for byte decoding, parsing, validation, normalized IR, source maps, and compiler diagnostics;
- the workspace-local `stack-formatter` for canonical source output;
- `stack-theme` at `ed6c500762fc9ccffc8777172ac672a716dcd916` for the embedded core catalog, SVG bytes, deterministic font metrics, catalog version, and catalog revision.

The resolved normal dependency graph adds only the `serde` and `serde_json` graph required by `stack-theme`. Exact versions and licenses are recorded in [`THIRD_PARTY_LICENSES.md`](../THIRD_PARTY_LICENSES.md) and pinned in `Cargo.lock`. Scene layout is implemented locally with fixed-width integer arithmetic and versioned catalog metrics. Standalone SVG serialization is also local and embeds only validated catalog icon bodies and local marker references. No third-party layout or SVG serializer, filesystem, network, asynchronous runtime, random, clock, locale, DOM, or platform-font dependency is present.

## Runtime access boundary

- `stack-engine` accepts source bytes plus a borrowed validated catalog and revision, or selects the catalog already embedded by `stack-theme`.
- `stack-compiler` and `stack-formatter` operate entirely on caller-owned bytes and in-memory values.
- `stack-theme` embeds catalog, schema, and SVG bytes at compile time and parses the trusted generated catalog through an in-memory singleton.
- No runtime dependency discovers a path, opens a socket, reads process state, observes time, samples randomness, queries a DOM, or measures a system font.

Tests and CI may read the pinned specification checkout and invoke toolchains. Those development actions are outside the runtime library boundary.

## Reproduction

```sh
cargo tree -p stack-engine --edges normal --locked
cargo metadata --format-version 1 --locked
cargo test --workspace --locked
STACK_SPECIFICATION_DIR=../specification cargo test -p stack-engine --features conformance --locked
python3 scripts/validate-svg.py
cargo build -p stack-engine --target wasm32-unknown-unknown --locked
CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime cargo test -p stack-engine --lib --target wasm32-wasip1 cross_target_numeric_fixture --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```
