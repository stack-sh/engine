# Third-party licenses

## Runtime dependencies

| Component | Revision | License | Source | Notes |
| --- | --- | --- | --- | --- |
| `stack-compiler` | `17a0abe9c35e641761ff08fdf59b29a42828d9fd` | Apache-2.0 | <https://github.com/stack-sh/compiler> | Unmodified Rust dependency; its license and notice obligations apply to distributions that include it. |

## Development-only dependencies

| Component | Version | License | Source | Notes |
| --- | --- | --- | --- | --- |
| `serde_json` | `1.0.151` | MIT OR Apache-2.0 | <https://github.com/serde-rs/json> | Canonical JSON fixture comparison only; not linked into `stack-formatter`. Its transitive test-only dependency graph is pinned in `Cargo.lock` and is not shipped. |

No third-party asset is bundled in a Stack Engine distribution yet.

Before publishing a native library, binary-derived artifact, or WASM package, this inventory must list the shipped dependencies and assets, their pinned versions, exact licenses, required license texts, attribution, modifications, and redistribution conditions. Build-only dependencies that are not shipped should be distinguished from distributed code.
