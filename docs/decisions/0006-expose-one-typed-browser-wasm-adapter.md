# ADR-0006: Expose one typed browser WebAssembly adapter

## Status

Accepted

## Context

Browser consumers need the same formatting, checking, rendering, diagnostic ordering, and provenance as native consumers. Reimplementing any compiler or renderer stage in JavaScript would create a second contract and allow target drift. A generic `any` interface would also hide the distinction between normal Stack diagnostics and adapter misuse.

## Decision

Add `stack-engine-wasm` as a thin `wasm32-unknown-unknown` adapter over `stack-engine` and package its generated web bindings as `@stack-sh/engine`. Export synchronous `format`, `check`, and `render` operations after module initialization. Each operation accepts only a JavaScript `string` or `Uint8Array` and returns an operation-specific TypeScript result. Preserve nullable artifacts, ordered portable diagnostics, camel-case source positions, and engine / language / catalog metadata.

Validate input type only at the JavaScript boundary. Pass strings as UTF-8 bytes and byte arrays unchanged to the native byte-oriented facade. Invalid Stack bytes, including invalid UTF-8, remain normal diagnostic results. Unsupported JavaScript values throw `TypeError`; engine operational failures throw `Error`.

Keep object conversion in the adapter and all language, formatting, layout, routing, and SVG behavior in `stack-engine`. Generate web-target ECMAScript bindings with a version-matched `wasm-bindgen` library and CLI. Do not import WASI, filesystem, network, DOM, clock, random, or host-font capabilities into the WebAssembly module.

## Consequences

- Native and browser behavior can be compared as complete operation results over one shared fixture set.
- TypeScript consumers receive explicit input, diagnostic, metadata, and operation-result contracts instead of `any`.
- The npm artifact contains generated JavaScript glue, declarations, WebAssembly, package documentation, and the Apache-2.0 license.
- Consumers own module loading and every host interaction outside the pure operations.
- The Rust library version, `wasm-bindgen` CLI version, generated binding files, import allowlist, and package contents must be verified together before publication.
