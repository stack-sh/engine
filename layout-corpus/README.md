# Layout regression corpus

This versioned corpus makes layout changes measurable before they become approved Engine behavior. It is an evaluation contract, not a language conformance suite or a replacement for the user-facing example gallery.

## Coverage

[`catalog.json`](./catalog.json) covers small, medium, and dense diagrams plus the layout failure modes that have the highest review value:

- direct and nested groups, containment, and padding;
- same-rank and cross-axis order constraints;
- fan-out, fan-in, cross-boundary edges, mixed edge kinds, and labels;
- wide multilingual node and edge labels;
- caller-owned provider artwork embedded through the existing audited fixture.

Every case declares exact node, group, edge, and provider-notice counts. The Rust integration test renders through the public `Engine` facade, confirms clean diagnostics and positive SVG bounds, checks accessible and local-only SVG structure, verifies element inventory and unique Stack identifiers, and writes the current candidate before comparing it byte-for-byte with the approved snapshot. Internal scene validation continues to reject node overlap, group-containment, and route geometry failures before SVG serialization.

## Review and snapshot updates

Run the current Engine first without changing approved references:

```sh
cargo test -p stack-engine --test layout_corpus --locked
cargo test --release -p stack-engine --test layout_corpus layout_runtime_stays_within_budget --locked -- --ignored --nocapture
npm run layout:gallery
```

The first command writes current candidates to `target/layout-corpus/candidate`, even when exact comparison fails. The release-mode benchmark writes `target/layout-corpus/performance.json`. The gallery then builds at `target/layout-gallery/index.html` with the approved reference and current candidate side by side under identical source, Engine, theme, and provider-pack inputs.

Review every changed case in the gallery and the SVG diff. Only after the geometry is intentionally approved, replace the references explicitly:

```sh
UPDATE_STACK_LAYOUT_SNAPSHOTS=1 \
  cargo test -p stack-engine --test layout_corpus --locked
```

Commit the source, catalog, and updated snapshots together. Never update a snapshot only to make CI green. Schema version 1 rejects undeclared fields, unsafe paths, duplicate inventory, missing density coverage, and missing required failure-mode coverage.

## Runtime budget

The benchmark warms each case three times and measures twenty release-mode renders. Every case must remain at or below 50 ms p95, and the complete seven-case suite including fixture loading must remain at or below 2,500 ms. These intentionally broad CI-safe ceilings detect algorithmic regressions without treating small host timing differences as product changes. The report shown in the gallery records observed timings; it is not a checked-in benchmark claim.

## Geometry quality gates

Snapshot equality does not prove legibility. Independent test-only gates examine
the final SVG together with its prepared scene and versioned text metrics:

```sh
cargo test -p stack-engine --lib layout_quality::geometry --locked
cargo test -p stack-engine --lib layout_quality::detector --locked
cargo test -p stack-engine --lib layout_quality::parser --locked
cargo test -p stack-engine --lib layout_quality::metrics --locked
cargo test -p stack-engine --lib layout_quality::corpus_text_quality --locked -- --nocapture
cargo test -p stack-engine --lib layout_quality::corpus_edge_quality --locked -- --nocapture
```

The text gate detects overlapping logical text envelopes (including diagram and
group titles and edge-label backgrounds) and clipping. The edge gate detects
stroke-envelope intersections with text and node layout envelopes. Only the
actual first/last segment's normal contact with its own source/target boundary
is exempt. Walking along a node boundary or re-entering it is not exempt. An
edge's own label is not exempt either: painting an opaque rectangle over an edge
is not proof that the route reserves space for that label.

Each corpus command writes all candidate SVGs and an entity/geometry report to
`target/layout-quality/text/` or `target/layout-quality/edge/` before failing on
violations. Reports also record canvas dimensions, route length, and bends as
separate composition measurements, not a weighted beauty score. Candidates do
not overwrite approved snapshots. Current output intentionally fails these
quality gates until the measured defects are resolved; skipping or allowing the
existing violations would hide the work still needed.

Checker unit tests cover colliding and separated boxes, stroke-only contact,
endpoint exceptions, reversed segments, boundary traversal, re-entry, and
unsupported geometry/content mutations. The parser rejects unmeasured transforms
and unsupported text/route structures, CSS stylesheets, and inherited text
positioning. Semantic text must occur once per owner and role; edge-label text
must fit its background. Node shapes must exist and their SVG envelope must
agree with the prepared scene. Rectangles, circles, polygons, and the current
cylinder path grammar are supported; cylinder Bezier control points provide a
conservative envelope, not exact curve extrema.

Text uses theme advances and line height. Alphabetic reservations use the
catalog's ascent/descent; `middle` reserves a centered line-height box. The latter
is deliberately not an exact SVG glyph bound: SVG's middle baseline depends on
font x-height, which the current metrics do not provide. Browser font fallback
and actual glyph positioning still need visual validation.

These are logical geometry checks, not
pixel-level glyph measurements or arbitrary SVG validation. Exact decorative
shape outlines, arrowheads, group-frame crossings, all theme variants, and a
complete global composition metric remain outside this first gate. Passing it
is necessary but not sufficient for visual approval: inspect the full diagram
and review its composition before approving new snapshots or publishing a
release. Candidate diagrams may be redesigned globally; label displacement
alone is not a required implementation strategy.
