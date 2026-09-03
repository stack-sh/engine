# ADR-0001: Build the Formatter from Compiler Models

## Status

Accepted

## Date

2026-09-03

## Context

The Stack canonical formatter must normalize source bytes while preserving line comments, their token gaps, declaration order, property order, and decoded string values. The compiler already owns lexical rules, recursive-descent parsing, semantic diagnostics, source spans, and the lossless token model.

A formatter-specific parser would duplicate contextual-keyword and string behavior. Formatting only from the semantic AST would lose whitespace, comments, and authored escape spelling. Formatting only from flat lossless tokens would require rediscovering declaration and property boundaries to place canonical line breaks.

## Decision

Implement `stack-formatter` in the engine workspace with a commit-pinned `stack-compiler` dependency.

The formatter uses both compiler representations:

- AST spans identify version, member, property, and layout-statement boundaries that require a canonical line or blank line;
- lossless tokens retain comments, their preceding/following token gap, original line classification, CRLF, and decoded string values.

The formatter emits tokens in authored order. It replaces trivia with canonical spaces, two-space indentation, LF line endings, and one final newline. It emits decoded strings directly as UTF-8 except for quotes and backslashes. Comments retain their exact lexeme and token gap; a comment inside a normally single-line construct forces the specified continuation indentation.

Lexical or syntax errors return diagnostics without formatted source. Semantic and complexity diagnostics do not prevent formatting. The public result includes the original compiler diagnostics, and tests require the portable diagnostic-code set to remain stable after formatting.

All operations accept in-memory bytes or text. The crate performs no filesystem, environment, clock, random, or network access.

## Alternatives Considered

### Implement a second parser

- Pros: The formatter could build a purpose-specific tree.
- Cons: Grammar, string decoding, and error behavior could drift from the canonical compiler.
- Rejected: One compiler must own language interpretation.

### Add comments to normalized IR

- Pros: Formatting could consume one downstream representation.
- Cons: Portable semantic equality would depend on source spelling and trivia.
- Rejected: Normalized IR remains source-independent.

### Format semantic-valid documents only

- Pros: Fewer malformed semantic states to test.
- Cons: Editors and `stack fmt` could not repair whitespace in a file while the author resolves name or layout errors.
- Rejected: Syntax validity is the formatting boundary defined by the public specification.

## Consequences

- Canonical fixture, idempotence, example, and semantic-preservation tests share the pinned public specification revision.
- The formatter executes compiler parsing more than once to obtain separate AST, lossless, and diagnostic outputs; this is bounded by Stack's source limits and avoids duplicating parser code.
- The compiler and specification revisions must be advanced deliberately after their provider changes merge.
