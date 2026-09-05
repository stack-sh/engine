# `@stack-sh/engine`

Browser WebAssembly adapter for the pure Stack diagram engine.

```js
import init, { check, completion, format, hover, render, renderWithProviderPacks } from "@stack-sh/engine";

await init();

const formatted = format('stack 1.0 diagram "API" { node api "API" }');
const checked = check(new TextEncoder().encode('stack 1.0 diagram "API" { node api "API" }'));
const rendered = render('stack 1.0 diagram "API" { node api "API" { icon "api" } }');
const draft = 'stack 1.0 diagram "API" { no';
const position = {
  byteOffset: new TextEncoder().encode(draft).length,
  line: 1,
  column: Array.from(draft).length + 1,
};
const completions = completion(draft, 1, position);

const hoverSource = 'stack 1.0 diagram "API" { node api "API" }';
const apiOffset = hoverSource.indexOf("api");
const semanticHover = hover(hoverSource, 1, {
  byteOffset: apiOffset,
  line: 1,
  column: apiOffset + 1,
});
```

`renderWithProviderPacks(source, packs)`, `checkWithProviderPacks(source, packs)`, and `completionWithProviderPacks(source, documentVersion, position, packs)` accept JSON-compatible, caller-owned provider manifests and processed SVG strings. They resolve or complete namespaced identifiers such as `example:storage`, while render preserves the authored node kind and returns provider notices containing the exact pack revision, source release, archive hash, terms URL, and used icons. The adapter performs no filesystem, network, storage, clock, or DOM access.

`completion` and `hover` implement Stack language-intelligence schema 1.0 over one complete string snapshot. Positions contain a zero-based UTF-8 byte offset plus one-based Unicode scalar line and column. Results echo `documentVersion`, allowing an editor to discard stale work, and expose only plain-text labels and documentation. Completion obtains keywords, properties, enum values, and document identifiers from the compiler; icon entries come from the Engine's core and validated provider catalogs.

Each operation is synchronous after module initialization. Format, check, and render accept either a JavaScript string or `Uint8Array`; completion and hover accept a string because their positions are defined over UTF-8 text. Invalid Stack source, including invalid UTF-8 bytes in byte-oriented operations, returns normal portable diagnostics. Diagnostics include the primary range, ordered `expected` values, corrective help, and related source locations. Unsupported values and malformed number or position objects throw `TypeError` at the package boundary.

The package does not read files, contact a network service, inspect the DOM, observe a clock, or measure host fonts. Consumers own module loading and all host I/O.

The bundled catalog resolves 30 first-party explicit icons in the `default`, `light`, and `dark` themes: `api`, `web`, `mobile`, `desktop`, `server`, `container`, `cluster`, `cloud`, `scheduler`, `webhook`, `identity`, `observability`, `gateway`, `load-balancer`, `dns`, `cdn`, `firewall`, `network`, `event`, `stream`, `search`, `analytics`, `repository`, `pipeline`, `secret`, `document`, `task`, `chat`, `email`, and `ai`. Missing authored icon identifiers still produce `STK5001` and render the theme's fallback icon.
