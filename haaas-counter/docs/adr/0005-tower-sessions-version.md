# 0005 ? tower-sessions 0.14 with tower-sessions-sqlx-store 0.15

Date: 2026-10-06

## Context

`tower-sessions-sqlx-store` 0.15.0 depends on `tower-sessions-core` 0.14, while `tower-sessions` 0.15 uses core 0.15. Using the latest of both gives two incompatible `SessionStore` traits, and `SqliteStore` fails to satisfy `SessionManagerLayer`'s bound.

## Options considered

1. **tower-sessions 0.14 + store 0.15.0** (both on core 0.14). Pros: compiles, no custom code. Cons: one minor behind.
2. **Hand-written store** (see ADR 0004, option 2). Pros: current versions, and it would also unlock sqlx 0.9. Cons: owning session storage code.

## Decision

Option 1: `tower-sessions = "0.14"`.

## Consequences

Upgrade tower-sessions, the store and sqlx together once the store is published against core 0.15 and sqlx 0.9. If that stalls, ADR 0004 option 2 resolves both pins at once.
