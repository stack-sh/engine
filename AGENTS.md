# Repository Guide

## Language

Write repository content, code comments, commit messages, issues, and pull requests in English.

## Architecture

- Keep the engine and formatter pure, deterministic, and independent of host I/O.
- Reuse `stack-sh/compiler` for language parsing and compiler-stage diagnostics.
- Reuse `stack-sh/theme` for the versioned core catalog, icons, and font metrics.
- Keep layout and SVG rendering as internal `stack-engine` modules unless a proven public boundary requires otherwise.
- Keep the native and WASM adapters aligned through shared fixtures rather than duplicate implementations.
- Do not add CLI filesystem behavior, user authentication, billing, entitlement, paid-theme delivery, or network access.

## Delivery

- Use a topic branch and pull request; squash merge after approval.
- Work in small increments that keep the workspace buildable and tested.
- Add repository-specific formatting, linting, tests, target builds, and artifact checks with the code that needs them.
- Record third-party dependency and bundled-asset obligations in `THIRD_PARTY_LICENSES.md` before publishing native or WASM artifacts.
- Keep credentials, tokens, private keys, signing material, and customer data out of Git.
