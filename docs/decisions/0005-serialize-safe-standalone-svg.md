# ADR-0005: Serialize safe standalone SVG from resolved resources

## Status

Accepted

## Context

Native and browser consumers need the same portable render artifact. Depending on a DOM, host fonts, external images, or runtime resource discovery would make output target-dependent and would weaken the pure engine boundary. Authored labels and details are untrusted text, and catalog icons must not introduce script or external references into the generated document.

## Decision

Serialize standalone SVG directly from the deterministic scene and the already validated theme catalog. Use integer scene coordinates in the `viewBox`, explicit CSS-pixel dimensions, catalog font metrics and palette tokens, embedded catalog icon bodies, and one local marker definition. Escape all authored text and attributes. Do not emit script, event attributes, `href`, `src`, `foreignObject`, or arbitrary `url(...)` references.

Give the root document an accessible title and description. Represent groups, nodes, and edges with semantic labels derived from authored display labels rather than identifiers. Render edge routes behind nodes and place visible edge labels in the nearest deterministic rectangle that does not overlap a node or an earlier edge label.

Missing requested themes and icons produce source-mapped warnings and use catalog fallbacks. They do not suppress the SVG. Compiler error diagnostics continue to prevent render output.

## Consequences

- Native and future WASM adapters can return identical SVG bytes without browser APIs.
- Output carries the engine version and exact theme catalog version and revision.
- Canonical fixtures cover every node kind, edge kind, and direction in exact SVG snapshots.
- CI parses snapshots as XML and rejects executable elements, event handlers, external references, duplicate identifiers, or non-local URL references.
- Adding richer text shaping, external assets, or arbitrary catalog SVG requires a new validation and determinism decision rather than an implicit renderer change.
