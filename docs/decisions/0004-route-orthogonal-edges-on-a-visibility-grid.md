# ADR-0004: Route orthogonal edges on a visibility grid

## Status

Accepted

## Date

2026-09-03

## Context

The internal scene must preserve every normalized edge in declaration order, distinguish forward, bidirectional, and association semantics, place optional labels, and keep paths out of node interiors. Routing must remain deterministic and host-independent for native and WebAssembly builds. The engine must also report `STK4001` only when its concrete placement does not satisfy an authored order hint, using the compiler-owned source-map range rather than reconstructing source locations.

## Decision

Represent each scene edge with authored endpoints, normalized kind and direction, optional label, an ordered orthogonal point sequence, explicit start and end markers, and an optional label anchor. Forward edges have an arrow only at the target, bidirectional edges have arrows at both endpoints, and associations have no arrows. A label anchor is the integer half-length point along the routed path.

Build one rectilinear visibility grid per scene from canvas margins plus every node boundary, midpoint, and clearance line. Grid points strictly inside nodes are unavailable. Adjacent visible points form possible horizontal or vertical path segments. For each edge, a deterministic shortest-path search starts from all four source boundary midpoints and accepts all four target boundary midpoints. Cost combines Manhattan distance with a fixed bend penalty; stable grid and state order resolves ties. Geometry validation independently verifies boundary endpoints, axis-aligned nonzero segments, canvas containment, marker semantics, label-anchor membership, and absence of node-interior intersections.

After placement, evaluate each authored order list using doubled cross-axis center coordinates. Every consecutive entry must increase strictly along the resolved scope cross-axis. Satisfied hints produce no diagnostic. Unsatisfied hints produce warning `STK4001` at the complete authored order-statement span supplied by `stack-compiler::source_map::SourceMap`. Compiler diagnostics remain first; layout diagnostics follow in diagram-then-depth-first-group order.

## Consequences

- Routing depends only on normalized IR and existing integer scene geometry.
- Paths may touch or follow a node boundary but never enter a node interior.
- Edge declaration order, kind, label, direction, path, markers, and anchors remain snapshot-testable independently of SVG serialization.
- The bend penalty favors readable routes without making crossing minimization a semantic guarantee.
- Layout diagnostics reuse the public compiler sidecar and do not add source spans to portable IR.
