# ADR-0002: Use a pure versioned engine facade

## Status

Accepted

## Context

Native CLI and browser consumers need the same meaning for `format`, `check`, and `render`. Returning compiler structs directly would leak one pipeline stage, make later engine diagnostics harder to add, and leave no stable place for render provenance. Treating invalid source as a Rust error would also mix normal user feedback with failures in execution inputs or host coordination.

The engine must remain deterministic. It may consume source bytes and a bundled or caller-provided validated catalog, but it must not discover files, environment variables, network resources, clocks, random values, locales, DOM state, or host font measurements.

## Decision

Add a `stack-engine` crate with an `Engine` bound to one validated catalog and content revision. `Engine::bundled` uses the catalog embedded by `stack-theme`; `Engine::with_catalog` accepts an already schema- and asset-validated catalog and verifies the fallback and revision invariants required by execution.

The facade owns portable diagnostics and these result records:

- `FormatOutput` contains optional canonical source, ordered diagnostics, and metadata.
- `CheckOutput` contains ordered diagnostics and metadata and never contains SVG.
- `RenderOutput` contains optional standalone SVG, ordered diagnostics, and metadata.

Every metadata record identifies the engine version, authored language version when parsing can recover it, theme catalog version, and theme catalog content revision. Diagnostics use owned codes and messages plus end-exclusive ranges whose offsets, lines, and columns are fixed-width unsigned integers for native/WASM parity.

Invalid user bytes or source return a successful operation result with diagnostics and no unavailable artifact. Invalid provided catalog state or an internal normalized pipeline invariant returns `OperationalError`. Host filesystem and process failures remain responsibilities of adapters such as the private CLI and never become Stack diagnostics.

The facade delegates formatting to `stack-formatter` and compilation to `stack-compiler`. Valid rendering continues through theme and icon resolution, deterministic layout and routing, and standalone SVG serialization. Resource and layout warnings preserve an SVG; compiler error diagnostics prevent it. The temporary pipeline-unavailable branch used before renderer integration has been removed.

## Consequences

- Native and WASM adapters can share one output and diagnostic model without copying compiler or formatter logic.
- Render provenance is embedded in standalone SVG output.
- Ordered compiler diagnostics survive conversion byte-for-byte in sequence and source location.
- New engine-stage diagnostics can be appended behind the same public type.
- The facade stays host-independent, while adapters retain responsibility for I/O and process semantics.
- Rendering remains deterministic and host-independent because all catalog assets and metrics are supplied in memory.
