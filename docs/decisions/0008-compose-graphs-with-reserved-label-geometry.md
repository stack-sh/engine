# ADR-0008: Compose graphs with reserved label geometry

## Status

Accepted for Engine 0.8.0 after visual review. Scene and SVG references record
the approved output with Theme 0.6.0.

## Context

Declaration-order placement can make connected elements run backwards and
flatten nested architectures into long strips. Moving labels after routing can
put them across a route, outside the canvas, or far from the edge they describe.
Node-interior checks alone also admit paths along a block's border.

Collision freedom is a hard requirement, not a score that can be traded for a
smaller canvas. Alignment, spacing, bends, crossings, and overall composition
still require human review; passing geometry checks does not prove beauty.

## Decision

Keep integer, host-independent bottom-up measurement and top-down placement.
Project descendant connections onto direct children of each scope. Collapse
authored disjoint same-rank sets, then strongly connected components, and use
longest-path depth in the resulting DAG to form layers. Declaration index breaks
ties. Authored order sorts members within a layer; diagnostics use the actual
resolved scope direction and cross-axis positions.

Center shorter layers on the common cross-axis. Measure labels with the selected
catalog before allocating inter-layer and within-layer spacing. Reserve a
right-side label band for vertical compositions. Explicit directions remain
exact and independent between scopes. For connected automatic scopes, compare
right and down by maximum dimension, then area, with the historical direction as
the tie-break. Disconnected scopes and simple unlabeled pairs keep the historical
policy. Bidirectional and association endpoints currently contribute their stored
from-to order to composition; marker semantics remain unchanged.

Route through a shared rectilinear visibility grid using four midpoint ports and
normal terminal stubs. Avoid closed node envelopes with an 8px clearance and
measured diagram/group titles. Penalize bends, shared lengths, and crossings with
deterministic integer costs. Crossings are discouraged, not prohibited.

Treat group frames as finite perimeter obstacles, not filled rectangles. A route
may cross a side normally, but must not run parallel within its clearance band
or turn close to a frame. Reserve 18px between centerlines, conservatively
covering a 16px painted gap with current core strokes. Include corner clearance
and apply the same rules to terminal stubs; node-terminal exceptions do not
exempt group frames. Measure 40px group padding to leave room for these internal
lanes, and use that same padding for group-title serialization.

The scene owns both each label rectangle and its attachment on an interior point
of its own route segment. Candidate positions are finite, 8px from that segment
for core strokes, or the rounded-up stroke radius plus 1px when that is larger.
They prefer above horizontal or right of vertical segments. Reject
contact with nodes, titles, other labels, and every route's stroke envelope.
Commit placement only when every label fits. SVG serialization must not relocate
labels or hide route sections beneath label backgrounds.

Reserve separate 9px finite perimeter bands when placing labels, guaranteeing
at least 8px between label backgrounds and current painted group frames. Do not
pass these label-only obstacles to the router: normal frame crossings must stay
possible. Validate label-frame separation against the emitted SVG as well.

Keep millipixel integers inside the scene, but serialize diagram-space SVG
coordinates, viewport, strokes, dashes, and font sizes in CSS pixels using exact
decimal strings with at most three fractional digits. Large millipixel SVG user
coordinates can trigger browser font-size clamping and shape rasterization
defects even when the geometric scene is valid. Nested icon bodies retain their
own local coordinate systems; marker viewport dimensions use diagram pixels.
The independent SVG checker converts strict decimal pixels back to integer
millipixels without floating-point rounding. Browser-rendered shape and font
checks supplement this mathematical geometry validation.

If jointly placing labels on the routed graph fails, route and label edges in
descending label-width order, freezing prior labels as obstacles for later
routes. If the shortest route has no label position, try at most sixteen normal
source/target port pairs in deterministic length, bend-count, and path order.
Keep previous paths and label boxes unchanged and restore authored edge order
in the output. Validate the complete scene afterwards. If necessary,
repeat composition with gap multipliers 2 and 3 and symmetric exterior label
bands. The entire search is bounded to three composition attempts; it never uses
elapsed time or accepts colliding fallback geometry.

## Consequences

- This policy replaces the automatic placement policy in ADR-0003, boundary
  contact and midpoint-label policy in ADR-0004, and SVG-owned label placement in
  ADR-0005, including its millipixel SVG user-space policy; their remaining
  integer-scene, semantic, diagnostic, and safety contracts
  remain intact.
- No syntax, public API, theme contract, dependency, or host capability is added.
- Native/WASM parity, semantic and rank tests, independent final-SVG collision
  gates, representative runtime budgets, and adversarial input regressions are
  required in addition to visual review.
- The frame gate reads the emitted rectangle and both actual stroke widths. It
  requires a 16px painted gap, checks all four finite sides and nearby corners,
  and permits normal crossings away from corners. Hidden, transformed, missing,
  or otherwise unmeasured frame geometry fails closed.
- Exact snapshots record the visually approved output. Future reference changes
  still require explicit visual review rather than automatic acceptance.
- Finite heuristics are not a proof of feasible placement for every valid graph.
  A failure must remain explicit and become a regression fixture, never a hidden
  overlap, missing edge, or unmeasured text box.
