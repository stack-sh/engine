# Stack Engine

`stack-sh/engine` is the pure Rust execution engine for Stack architecture diagrams.

This repository currently contains only its repository foundation. Its crates and public APIs will be introduced incrementally after the compiler, formatter, and theme contracts they depend on are stable.

## Planned workspace

- `stack-engine`: compilation orchestration, theme resolution, deterministic layout, validation beyond the compiler stage, and standalone SVG rendering;
- `stack-formatter`: comment-preserving canonical formatting for Stack source files;
- a WebAssembly adapter exposing the same pure operations to browser consumers.

The native CLI will link the Rust engine directly. Web clients will use the WASM adapter. Shared fixtures will verify that both targets produce equivalent diagnostics, formatted source, and SVG output.

## Boundaries

The engine may depend on `stack-sh/compiler` and `stack-sh/theme`. Core operations must be deterministic and must not require filesystem, environment, clock, random, or network access.

CLI filesystem behavior, process exit codes, user authentication, billing, entitlement checks, and paid-theme delivery are outside this repository.

## Development

Repository checks currently validate the foundation files on every push and pull request. Rust formatting, linting, tests, target builds, and WASM package validation will be added with the first workspace increment.

## Licensing

This repository is licensed under the [Apache License 2.0](./LICENSE). Third-party dependencies or bundled assets must be tracked in [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) before a distributable native or WASM artifact is published.
