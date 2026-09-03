# Stack Engine

`stack-sh/engine` is the pure Rust execution engine for Stack architecture diagrams.

The first workspace increment provides canonical Stack source formatting. Layout, theme resolution, rendering, and WebAssembly adapters remain planned work.

## Planned workspace

- `stack-engine`: compilation orchestration, theme resolution, deterministic layout, validation beyond the compiler stage, and standalone SVG rendering;
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
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
```

`stack-formatter` is pure and accepts source bytes or UTF-8 text. Lexical and syntax errors return diagnostics without formatted output. Syntactically valid source remains formattable when semantic diagnostics exist.

## Architecture

- [`docs/decisions/0001-build-the-formatter-from-compiler-models.md`](./docs/decisions/0001-build-the-formatter-from-compiler-models.md)

## Licensing

This repository is licensed under the [Apache License 2.0](./LICENSE). Third-party dependencies or bundled assets must be tracked in [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) before a distributable native or WASM artifact is published.
