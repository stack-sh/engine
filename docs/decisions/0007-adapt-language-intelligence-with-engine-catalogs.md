# ADR-0007: Adapt language intelligence with Engine-owned catalogs

## Status

Accepted

## Context

Browser editors need the same completion, hover, diagnostic ranges, and source edits as native tools. The compiler owns those language semantics through its protocol-neutral language-intelligence 1.0 contract, but intentionally receives icon metadata from its caller. The Engine already owns the effective core theme catalog and validated, caller-owned provider packs used by check and render operations.

Building keyword or property suggestions in a Web application would create a second grammar. Building icon suggestions in each consumer would also allow completion to drift from the resources the Engine can actually resolve.

## Decision

Expose stateless `completion` and `hover` methods from the native Engine facade and its typed browser WebAssembly adapter. Forward source analysis to the pinned compiler without changing its semantics. Convert positions and outputs explicitly at the Engine boundary and preserve language-intelligence schema version 1.0, end-exclusive UTF-8 ranges, plain-text documentation, and the caller-owned document version.

Derive completion catalog entries from the Engine's validated core catalog and provider packs. Use exact icon IDs as completion labels, filter text, and inserted text; use core subjects or provider product names as secondary detail. Provider assets remain local caller-owned inputs and are validated through the same pack constructor used by check and render. Enforce the compiler's bounded catalog size before analysis.

The browser exports accept string snapshots for position-based operations, safe-integer document versions, and `{ byteOffset, line, column }` positions. Adapter misuse and inconsistent positions use the operational error channel. Source diagnostics remain normal results. The adapter stays synchronous and stateless; hosts own debounce, document lifecycle, cancellation, and stale-result suppression.

## Consequences

- Browser and native consumers share compiler-owned completion, hover, diagnostics, text edits, and exact parity fixtures.
- Core and uploaded provider icon completion cannot drift from Engine resource resolution.
- Web applications do not own or duplicate Stack grammar rules.
- Parsing provider-pack JSON on each provider-aware call has a measurable cost. A stateful cached adapter may be added later if browser latency evidence requires it, without changing the language-intelligence result contract.
- Position-aware operations reject byte arrays because a host must supply coordinates for decoded UTF-8 text.
