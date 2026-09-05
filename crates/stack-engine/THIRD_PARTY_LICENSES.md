# Third-party licenses

## Runtime dependencies

| Component | Revision | License | Source | Notes |
| --- | --- | --- | --- | --- |
| `stack-compiler` | `0.1.0 (crates.io)` | Apache-2.0 | <https://github.com/stack-sh/compiler> | Unmodified Rust dependency; its license and notice obligations apply to distributions that include it. |
| `stack-theme` | `0.5.0 (crates.io)` | Apache-2.0 | <https://github.com/stack-sh/theme> | Unmodified Rust dependency with repository-authored core assets and the asset-free provider-pack contract. |
| `roxmltree` | `0.21.1` | MIT OR Apache-2.0 | <https://github.com/RazrFalcon/roxmltree> | Parses caller-owned processed provider SVG before allowlisted in-memory embedding. |
| `sha2`, `digest`, `block-buffer`, `crypto-common`, `hybrid-array`, `const-oid`, `typenum` | `0.11.0`, `0.11.3`, `0.12.1`, `0.2.2`, `0.4.14`, `0.10.2`, `1.20.1` | MIT OR Apache-2.0 | <https://github.com/RustCrypto> | Verifies provider asset hashes and computes deterministic provider-pack revisions. |
| `libc` / `cpufeatures` | `0.2.189`, `0.3.1` | MIT OR Apache-2.0 | <https://github.com/rust-lang/libc>, <https://github.com/RustCrypto/utils> | Target-specific SHA-256 acceleration support. |
| `serde` / `serde_core` | `1.0.229` | MIT OR Apache-2.0 | <https://github.com/serde-rs/serde> | Runtime catalog data types through `stack-theme`. |
| `serde_json` | `1.0.151` | MIT OR Apache-2.0 | <https://github.com/serde-rs/json> | Runtime embedded-catalog decoding through `stack-theme`; also used by formatter conformance tests. |
| `itoa` | `1.0.18` | MIT OR Apache-2.0 | <https://github.com/dtolnay/itoa> | Transitive runtime dependency of `serde_json`. |
| `memchr` | `2.8.3` | Unlicense OR MIT | <https://github.com/BurntSushi/memchr> | Transitive runtime dependency of `serde_json`. |
| `zmij` | `1.0.23` | MIT | <https://github.com/dtolnay/zmij> | Transitive runtime dependency of `serde_json`. |
| `wasm-bindgen` / `wasm-bindgen-shared` | `0.2.127` | MIT OR Apache-2.0 | <https://github.com/wasm-bindgen/wasm-bindgen> | JavaScript ABI and generated glue shipped by `@stack-sh/engine`. |
| `js-sys` | `0.3.104` | MIT OR Apache-2.0 | <https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/js-sys> | Typed-array input and plain JavaScript result objects; built without default features. |
| `cfg-if` | `1.0.4` | MIT OR Apache-2.0 | <https://github.com/alexcrichton/cfg-if> | Transitive runtime dependency of `wasm-bindgen` and `js-sys`. |
| `once_cell` | `1.21.4` | MIT OR Apache-2.0 | <https://github.com/matklad/once_cell> | Transitive runtime dependency of `wasm-bindgen`. |

## Build-only dependencies

| Component | Version | License | Source | Notes |
| --- | --- | --- | --- | --- |
| `serde_derive` | `1.0.229` | MIT OR Apache-2.0 | <https://github.com/serde-rs/serde> | Procedural macro used to build `stack-theme`. |
| `proc-macro2` | `1.0.107` | MIT OR Apache-2.0 | <https://github.com/dtolnay/proc-macro2> | Transitive procedural-macro build dependency. |
| `quote` | `1.0.47` | MIT OR Apache-2.0 | <https://github.com/dtolnay/quote> | Transitive procedural-macro build dependency. |
| `syn` | `3.0.4` | MIT OR Apache-2.0 | <https://github.com/dtolnay/syn> | Transitive procedural-macro build dependency. |
| `unicode-ident` | `1.0.24` | (MIT OR Apache-2.0) AND Unicode-3.0 | <https://github.com/dtolnay/unicode-ident> | Transitive procedural-macro build dependency. |
| `wasm-bindgen-macro` / `wasm-bindgen-macro-support` | `0.2.127` | MIT OR Apache-2.0 | <https://github.com/wasm-bindgen/wasm-bindgen> | Procedural macro and support code used to build the browser adapter. |
| `bumpalo` | `3.20.3` | MIT OR Apache-2.0 | <https://github.com/fitzgen/bumpalo> | Transitive build dependency of `wasm-bindgen-macro-support`. |
| `rustversion` | `1.0.23` | MIT OR Apache-2.0 | <https://github.com/dtolnay/rustversion> | Transitive build dependency of `wasm-bindgen`. |
| `syn` | `2.0.119` | MIT OR Apache-2.0 | <https://github.com/dtolnay/syn> | Transitive procedural-macro build dependency of `wasm-bindgen`. |
| `wasm-bindgen-cli` | `0.2.127` | MIT OR Apache-2.0 | <https://github.com/wasm-bindgen/wasm-bindgen> | Version-matched build tool; not shipped in the npm package. |
| `typescript` | `7.0.2` | Apache-2.0 | <https://github.com/microsoft/TypeScript> | Type-check tool; not shipped in the npm package. |
| `ajv` | `8.20.0` | MIT | <https://github.com/ajv-validator/ajv> | Validates the checked-in layout corpus schema during development and CI; not shipped in the npm package. |
| `fast-deep-equal` | `3.1.3` | MIT | <https://github.com/epoberezkin/fast-deep-equal> | Transitive build-only dependency of `ajv`. |
| `fast-uri` | `3.1.7` | BSD-3-Clause | <https://github.com/fastify/fast-uri> | Transitive build-only dependency of `ajv`. |
| `json-schema-traverse` | `1.0.0` | MIT | <https://github.com/epoberezkin/json-schema-traverse> | Transitive build-only dependency of `ajv`. |
| `require-from-string` | `2.0.2` | MIT | <https://github.com/floatdrop/require-from-string> | Transitive build-only dependency of `ajv`. |

No third-party visual asset is bundled in a Stack Engine distribution. The bundled fallback and 30 explicit icons are Stack-authored Apache-2.0 assets from `stack-theme`. The npm package includes this inventory and the Apache-2.0, MIT, and Unicode-3.0 license texts required by its compiled dependency choices.

Before publishing a native library, binary-derived artifact, or WASM package, this inventory must list the shipped dependencies and assets, their pinned versions, exact licenses, required license texts, attribution, modifications, and redistribution conditions. Build-only dependencies that are not shipped should be distinguished from distributed code.
