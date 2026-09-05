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
