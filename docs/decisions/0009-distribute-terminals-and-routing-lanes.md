# ADR-0009: Distribute terminals and routing lanes

## Status

Accepted for Engine 0.9.0 after visual review. Scene and SVG references record the approved output with Theme 0.7.0.

## Context

One midpoint port per node side can force unrelated edges onto the same terminal and corridor. In dense diagrams this creates ambiguous junctions, nearly overlapping parallel lines, and local clusters even when every route technically avoids node interiors. Declaration-order placement can also put connected peers opposite their upstream neighbors and add unnecessary crossovers.

These defects are not safely hidden by a single weighted beauty score. Text, node, and group-frame collisions remain hard failures. Crossings, reused terminals, shared route length, close parallel lanes, bends, route length, and canvas size remain separate review signals so an improvement in one cannot conceal damage in another.

## Decision

Expose three deterministic ports on each node side: the midpoint and two offset slots. Use offset slots only on sides with demonstrated multi-edge demand and enough geometry to leave normally. Preserve midpoint preference for simple diagrams. Penalize reused terminal points and close or shared route lanes, while retaining deterministic length, bend, crossing, and declaration-order tie breaks.

Route and label edges incrementally when joint placement cannot reserve every label. Each candidate sees already accepted routes and terminals, so fallback routing cannot silently recreate a shared port or corridor. Bound alternative port-pair search and preserve authored edge order in the final scene.

Within a same-rank set without an explicit authored order, compare connected external neighbors and permute only anchored slots to reduce crossovers. Keep unanchored slots stable. An explicit order remains authoritative even when a different visual order would be shorter.

Do not merge independent edges into a shared trunk. A common segment without an explicit junction would obscure edge identity and arrow semantics. Separation is preferred whenever a clear lane exists.

## Consequences

- Dense fan-in, fan-out, and cross-boundary diagrams can use distinct terminals and lanes without changing Stack syntax or the public Engine API.
- Exact SVG geometry intentionally changes. Approved snapshots, native/WASM parity, the seven-case corpus, and release-mode budgets remain required.
- The representative congestion fixture must report zero reused terminals, proper crossings, ambiguous junctions, shared route length, and close parallel length when clear alternatives exist.
- The search remains finite and heuristic. If no valid separated route exists, the Engine returns an operational error instead of hiding a collision or inventing an unlabeled junction.
