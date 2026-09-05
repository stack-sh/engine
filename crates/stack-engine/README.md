# Stack Engine

Pure Rust layout and SVG rendering for Stack diagrams. The engine reuses the registry-published Stack compiler, formatter, and theme catalog without host I/O or network access.

```toml
[dependencies]
stack-engine = "=0.7.0"
```

Rust 1.85 or newer is supported. See the [API documentation](https://docs.rs/stack-engine) and [repository documentation](https://github.com/stack-sh/engine) for rendering, limits, and compatibility. The browser adapter is distributed separately as `@stack-sh/engine` on npm; this crate is the native library.

The source package includes unit-test fixtures and license notices. Cross-repository conformance and layout-corpus integration tests run from the repository checkout in CI. Generated native and browser snapshots must remain unchanged when preparing a Cargo release.
