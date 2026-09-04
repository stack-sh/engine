# `@stack-sh/engine`

Browser WebAssembly adapter for the pure Stack diagram engine.

```js
import init, { check, format, render, renderWithProviderPacks } from "@stack-sh/engine";

await init();

const formatted = format('stack 1.0 diagram "API" { node api "API" }');
const checked = check(new TextEncoder().encode('stack 1.0 diagram "API" { node api "API" }'));
const rendered = render('stack 1.0 diagram "API" { node api "API" { icon "api" } }');
```

`renderWithProviderPacks(source, packs)` and `checkWithProviderPacks(source, packs)` accept JSON-compatible, caller-owned provider manifests and processed SVG strings. They resolve namespaced identifiers such as `example:storage`, preserve the authored node kind, and return provider notices containing the exact pack revision, source release, archive hash, terms URL, and used icons. The adapter performs no filesystem, network, storage, clock, or DOM access.

Each operation is synchronous after module initialization and accepts either a JavaScript string or `Uint8Array`. Invalid Stack source, including invalid UTF-8 bytes, returns normal portable diagnostics. Diagnostics include the primary range, ordered `expected` values, corrective help, and related source locations. A JavaScript value of any other type throws `TypeError` at the package boundary.

The package does not read files, contact a network service, inspect the DOM, observe a clock, or measure host fonts. Consumers own module loading and all host I/O.

The bundled catalog resolves 30 first-party explicit icons in the `default`, `light`, and `dark` themes: `api`, `web`, `mobile`, `desktop`, `server`, `container`, `cluster`, `cloud`, `scheduler`, `webhook`, `identity`, `observability`, `gateway`, `load-balancer`, `dns`, `cdn`, `firewall`, `network`, `event`, `stream`, `search`, `analytics`, `repository`, `pipeline`, `secret`, `document`, `task`, `chat`, `email`, and `ai`. Missing authored icon identifiers still produce `STK5001` and render the theme's fallback icon.
