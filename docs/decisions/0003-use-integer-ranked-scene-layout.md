# ADR-0003: Use integer ranked scene layout

## Status

Accepted

## Date

2026-09-03

## Context

The engine must turn normalized Stack IR into geometry that is identical in native and WebAssembly builds. The language defines `right`, `down`, automatic direction, same-rank constraints, cross-axis order, and independently configured nested groups. The theme catalog supplies versioned font advances, so layout must not depend on platform fonts, browser measurement, floating-point rounding, locale, or host state.

## Decision

Build an SVG-independent scene inside `stack-engine`. Every coordinate and extent is a signed 64-bit integer measured in one-thousandth of a CSS pixel. Text widths use only the selected theme's explicit glyph advances, wide Unicode ranges, default advance, units per em, and line-height ratio. Arithmetic rounds upward with integers.

Layout proceeds in two deterministic passes. A bottom-up pass calculates node and group sizes. A top-down pass places root elements and recursively places group children inside explicit content rectangles. Nodes and nested groups are siblings for collision checks. Parent content rectangles exclude title and padding areas, so geometry validation verifies padding containment instead of checking only outer bounds.

Authored `right` and `down` directions are exact. In a right-directed scope, same-rank elements share the x coordinate and order controls top-to-bottom placement. In a down-directed scope, they share the y coordinate and order controls left-to-right placement. Automatic direction selects right for one to three direct children and down for four or more. Each nested scope resolves its own direction; it never inherits from its parent.

The implementation has no layout dependency and no host access. The canonical complete-semantics fixture has a checked-in exact integer scene snapshot. Unit tests cover rank alignment, cross-axis order, nesting, padding, non-overlap, automatic direction, wide-character metrics, malformed normalized input, and fixed-width coordinates. CI compiles the browser target and executes the same exact numeric fixture in native and `wasm32-wasip1` test binaries. Adapter-level native/WASM output parity remains part of the later WebAssembly integration.

## Consequences

- Repeated layout for the same normalized IR, catalog revision, and options produces the same scene.
- Native and WebAssembly builds share integer geometry code and do not require platform font APIs.
- The automatic policy is intentionally simple and stable; changing it is an observable scene-layout change and requires snapshot review.
- SVG serialization consumes scene rectangles without owning ranking, measurement, or containment semantics.
- More advanced routing and constraint optimization can be added behind the scene boundary without changing compiler IR.
