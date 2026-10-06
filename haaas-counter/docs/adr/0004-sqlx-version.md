# 0004 — sqlx version: stay on 0.8.x

Date: 2026-10-06

## Context

The project uses sqlx for SQLite and `tower-sessions-sqlx-store` for the session store. On nightly Rust (1.101) sqlx 0.9.0 is usable, but the latest `tower-sessions-sqlx-store` (0.15.0) depends on sqlx 0.8. With sqlx 0.9 in `Cargo.toml`, Cargo resolves two sqlx versions (`cargo tree -i sqlx` becomes ambiguous), and the store only accepts a 0.8 `SqlitePool`, so our 0.9 pool can't be passed to it.

## Options considered

1. **Stay on sqlx 0.8.x.**
   - Pros: works with the planned crates; no custom session code; one sqlx version in the tree.
   - Cons: one major version behind; we must upgrade later.
2. **Use sqlx 0.9 and hand-write a `SessionStore` over our own pool.**
   - Pros: current sqlx; drops the `tower-sessions-sqlx-store` dependency; consistent with the "small understandable glue" philosophy.
   - Cons: ~80 lines of security-relevant session storage code we own (expiry handling, concurrent upserts, cleanup task, id collision); needs its own tests.
3. **Run both sqlx versions side by side** (0.9 for the app, 0.8 for the store, separate pools).
   - Pros: no custom code.
   - Cons: two pools on one SQLite file and two compiled sqlx copies; more WAL/busy-timeout contention. Rejected.

## Decision

Option 1: pin `sqlx = "0.8"` and use `tower-sessions-sqlx-store` 0.15.

## Consequences

- Revisit when `tower-sessions-sqlx-store` releases sqlx 0.9 support: bump both together.
- If we want sqlx 0.9 sooner, implement option 2: write a `SessionStore` (create/save/load/delete over a `sessions(id TEXT PRIMARY KEY, data BLOB, expiry_date INTEGER)` table, plus a periodic expired-row deletion task) and remove the store crate. Its migration would then live in our `migrations/` directory instead of being created by the store's `migrate()`.
