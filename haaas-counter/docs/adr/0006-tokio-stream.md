# 0006 - Add tokio-stream

Date: 2026-10-06

## Context

axum's `Sse` takes a `futures` `Stream`. None of the crates in the plan exports one, and we need `BroadcastStream` and stream combinators.

## Options considered

1. **tokio-stream** (chosen). Maintained by the tokio team, already in the lockfile transitively (via sqlx). Gives `BroadcastStream`, `once`, and `StreamExt`.
2. **async-stream.** Allows a `stream!` loop, but is a new proc-macro crate not in the tree.
3. **Hand-rolled `Stream`.** More code, and still needs `futures-core`, so little saved.

## Decision

Option 1, with the `sync` feature, approved by the project owner.

## Consequences

One extra direct dependency. The SSE handler is a short combinator chain with no spawned tasks.