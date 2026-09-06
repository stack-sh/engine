# Stack Engine

`stack-sh/engine` is the shared execution engine behind the [Stack CLI](https://github.com/stack-sh/cli) and [browser Playground](https://stack-diagram.com/). It formats and validates Stack source, calculates deterministic theme-aware layouts, and renders safe standalone SVGs with the same behavior in native Rust and browser WebAssembly.

Use this repository when embedding Stack rendering or language intelligence in an application. If you only want to create a diagram, start with the [CLI](https://github.com/stack-sh/cli#install) or [Playground](https://stack-diagram.com/).

## Choose an interface

| Environment | Package | API documentation |
| --- | --- | --- |
| Rust | [`stack-engine`](https://crates.io/crates/stack-engine) | [docs.rs](https://docs.rs/stack-engine) |
| Browser JavaScript / TypeScript | [`@stack-sh/engine`](https://www.npmjs.com/package/@stack-sh/engine) | [Package guide](./packages/engine/README.md) |

Both packages provide format, check, render, completion, and hover operations over caller-owned source. The browser package exposes the same engine through WebAssembly.

## Rust quick start

```sh
cargo add stack-engine@0.8.0
```

```rust
use stack_engine::Engine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = b"stack 1.0 diagram \"API\" { node api \"API\" { icon \"api\" } }";
    let output = Engine::bundled().render(source)?;

    if let Some(svg) = output.svg {
        std::fs::write("api.svg", svg)?;
    } else {
        for diagnostic in output.diagnostics {
            eprintln!("{}: {}", diagnostic.code, diagnostic.message);
        }
    }

    Ok(())
}
```

The crate supports Rust 1.85 or newer.

## Browser quick start

```sh
npm install @stack-sh/engine@0.8.0
```

```js
import init, { render } from "@stack-sh/engine";

await init();

const result = render(
  'stack 1.0 diagram "API" { node api "API" { icon "api" } }',
);

if (result.svg) {
  console.log(result.svg);
} else {
  console.error(result.diagnostics);
}
```

Initialization loads the WebAssembly module once. Operations are synchronous afterward and accept source already held by the caller; the adapter does not read files, access the DOM, or contact a network service.

## What the engine guarantees

- Deterministic integer layout and orthogonal routing using versioned font metrics and themes.
- Standalone SVG with embedded icons, accessible metadata, escaped authored text, and no script or external URL.
- Ordered, source-mapped diagnostics for invalid source, missing resources, and unsatisfied layout hints.
- Exact native/WebAssembly result parity over shared fixtures.
- Caller-owned provider packs that are validated in memory and never discovered, downloaded, or stored by the engine.

Every result includes engine, language, theme catalog version, and catalog revision metadata. See the [npm package guide](./packages/engine/README.md) for JavaScript types and provider-pack APIs, and [docs.rs](https://docs.rs/stack-engine) for the Rust facade.

## Workspace

- `stack-engine`: the pure operation facade, layout, routing, language intelligence, and SVG rendering.
- `stack-formatter`: canonical comment-preserving Stack source formatting.
- `stack-engine-wasm` and `@stack-sh/engine`: the typed browser adapter.

The engine consumes the public [Stack compiler](https://github.com/stack-sh/compiler) and [theme catalog](https://github.com/stack-sh/theme). Language syntax and normalized IR remain owned by the [Stack specification](https://github.com/stack-sh/specification).

Filesystem behavior, process exit codes, user authentication, billing, entitlement checks, and paid-theme delivery belong to host applications and are outside this repository.

## Architecture

The design records cover the [pure engine facade](./docs/decisions/0002-use-a-pure-versioned-engine-facade.md), [deterministic layout](./docs/decisions/0003-use-integer-ranked-scene-layout.md), [orthogonal routing](./docs/decisions/0004-route-orthogonal-edges-on-a-visibility-grid.md), [safe SVG](./docs/decisions/0005-serialize-safe-standalone-svg.md), [browser adapter](./docs/decisions/0006-expose-one-typed-browser-wasm-adapter.md), [language intelligence](./docs/decisions/0007-adapt-language-intelligence-with-engine-catalogs.md), and [label-aware graph composition](./docs/decisions/0008-compose-graphs-with-reserved-label-geometry.md).

See [CONTRIBUTING.md](./CONTRIBUTING.md) for repository setup, quality gates, the reviewed layout corpus, and release verification.

## Licensing

This repository is licensed under the [Apache License 2.0](./LICENSE). Third-party dependencies and bundled assets are recorded in [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md).
