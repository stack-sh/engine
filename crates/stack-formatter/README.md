# Stack Formatter

The pure Rust canonical formatter for the Stack diagram language. It consumes `stack-compiler` from crates.io and does not perform filesystem or network I/O.

```toml
[dependencies]
stack-formatter = "=0.1.0"
```

Rust 1.85 or newer is supported. See the [API documentation](https://docs.rs/stack-formatter) and the [canonical formatting contract](https://github.com/stack-sh/specification) for behavior. Cross-repository conformance tests require the pinned specification checkout and run in repository CI.
