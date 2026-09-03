# Third-party licenses

## Runtime dependencies

| Component | Revision | License | Source | Notes |
| --- | --- | --- | --- | --- |
| `stack-compiler` | `17a0abe9c35e641761ff08fdf59b29a42828d9fd` | Apache-2.0 | <https://github.com/stack-sh/compiler> | Unmodified Rust dependency; its license and notice obligations apply to distributions that include it. |
| `stack-theme` | `ed6c500762fc9ccffc8777172ac672a716dcd916` | Apache-2.0 | <https://github.com/stack-sh/theme> | Unmodified Rust dependency with repository-authored core SVG assets and deterministic metrics; its license and notice obligations apply to distributions that include it. |
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

No third-party visual asset is bundled in a Stack Engine distribution. The npm package includes this inventory and the Apache-2.0, MIT, and Unicode-3.0 license texts required by its compiled dependency choices.

Before publishing a native library, binary-derived artifact, or WASM package, this inventory must list the shipped dependencies and assets, their pinned versions, exact licenses, required license texts, attribution, modifications, and redistribution conditions. Build-only dependencies that are not shipped should be distinguished from distributed code.
