# HAAAS Glue Core + Counter Slice — Implementation Plan

## Goal

Build the **glue core** for a server-rendered web stack, then prove it with **one thin end-to-end slice**: a counter. This is a spike for a future artisan store (custom + pre-made items, payments, and a chat-like order tracker). The counter is deliberately designed to touch every layer the store will need, especially live updates, which are the riskiest part of the future order tracker.

Do not build any store features. The deliverable is a small, clean, working reference implementation.

## Philosophy (read this first)

This project avoids frameworks on purpose. Every piece should stay close to the underlying technology:

- Plain HTML files as templates (askama). **No HTML-in-macro DSLs** (no maud, no Leptos-style `view!`).
- No JS build step, no npm, no bundler. JS libraries are vendored as static files.
- Prefer small, understandable hand-written glue over pulling in a crate that hides behaviour. If a crate is clearly the sane choice (sessions, DB driver), use it.
- **Do not add any dependency not listed below without asking first.** When you hit a fork in the road with more than one valid option, stop and present the options rather than picking silently. These get recorded as ADRs.

## Stack: HAAAS

| Layer | Choice | Notes |
|---|---|---|
| H | htmx 2 + htmx SSE extension | Vendored into `static/vendor/` |
| A | axum | Latest stable |
| A | Alpine.js 3 | Vendored, client-side sprinkles only |
| A | askama | Real `.html` template files, compiled in |
| S | SQLite via sqlx | WAL mode, migrations via `sqlx::migrate!` |

Supporting crates: `tokio`, `tower-http` (ServeDir, TraceLayer), `tower-sessions` + `tower-sessions-sqlx-store` (SQLite store), `tracing` + `tracing-subscriber`, `thiserror` or a hand-rolled error enum, `rand` (CSRF tokens). Check current versions and APIs before writing code; askama's axum integration has changed across versions, so if there's no maintained integration crate, implement `IntoResponse` via a small helper that calls `.render()` and wraps it in `Html`.

## Project layout

```
haaas-counter/
  Cargo.toml
  flake.nix                 # dev shell: rust toolchain, sqlx-cli, sqlite
  .env.example              # DATABASE_URL, BIND_ADDR
  migrations/
  src/
    main.rs                 # config, tracing, startup
    app.rs                  # AppState, router assembly, layers
    error.rs                # AppError -> IntoResponse (HTML error page / fragment)
    render.rs               # askama -> axum response helper
    csrf.rs                 # synchronizer-token middleware
    routes/
      mod.rs
      counter.rs
  templates/
    base.html               # layout, loads vendored JS, sets CSRF header
    index.html              # extends base
    partials/
      count.html            # global count fragment
      session_count.html    # per-session count fragment
  static/
    vendor/                 # htmx.min.js, sse.js, alpine.min.js (pinned versions noted in a README)
    app.css
  docs/adr/
  tests/
    counter.rs
```

## Phase 0 — Scaffold

1. `cargo new`, add dependencies.
2. `flake.nix` with a dev shell providing the Rust toolchain, `sqlx-cli`, and `sqlite`. Keep it minimal. The project must also build on plain `cargo` without Nix.
3. Config from env (`DATABASE_URL`, default `sqlite://data.db?mode=rwc`; `BIND_ADDR`, default `127.0.0.1:3000`). No config crate needed.
4. `tracing` with `TraceLayer` on the router.

**Stop and check in after this phase.**

## Phase 1 — Glue core

Build these as reusable foundations, not counter-specific code.

1. **AppState**: `SqlitePool` and a `tokio::sync::broadcast::Sender` for live events. Cloneable, passed via `State`.
2. **Database**: pool with WAL journal mode and a busy timeout; run migrations on startup.
3. **Sessions**: `tower-sessions` with the SQLite store. HTTP-only, `SameSite=Lax` cookies. `Secure` configurable (off for localhost).
4. **CSRF**: hand-rolled synchronizer token.
   - Generate a random token per session on first request, store it in the session.
   - `base.html` sets it for every htmx request: `<body hx-headers='{"X-CSRF-Token": "{{ csrf_token }}"}'>`.
   - Middleware rejects any non-GET/HEAD request whose `X-CSRF-Token` header doesn't match the session token (constant-time compare), returning 403.
   - Also reject non-GET requests whose `Origin` header is present and doesn't match the host, as defence in depth.
5. **Rendering helper**: one function or trait so handlers return templates directly. HTML escaping must stay on (askama's default). Never use `|safe` on user-controlled data.
6. **Errors**: an `AppError` type that renders an HTML error page for full-page requests and a small fragment for htmx requests (detect via the `HX-Request` header).
7. **Static files**: `ServeDir` at `/static`. Vendored JS loaded in `base.html` in this order: htmx, SSE extension, then Alpine with `defer`.
8. **Base layout**: `base.html` with header/main/footer blocks that pages `{% extends %}`.

**Stop and check in after this phase.**

## Phase 2 — Counter slice

### Data

Migration:

```sql
CREATE TABLE counter (
  id    INTEGER PRIMARY KEY CHECK (id = 1),
  value INTEGER NOT NULL
);
INSERT INTO counter (id, value) VALUES (1, 0);
```

Increment atomically: `UPDATE counter SET value = value + 1 WHERE id = 1 RETURNING value`.

### Routes

| Method | Path | Returns |
|---|---|---|
| GET | `/` | Full page: global count, session count, button, rosette |
| POST | `/counter/increment` | `session_count.html` fragment, plus the global count as an `hx-swap-oob` fragment |
| GET | `/counter/events` | SSE stream of global count updates |

### Behaviour

- **POST increment**: increment in SQLite, increment a per-session click count stored in the session, broadcast the new global value, and return the session-count fragment with the global count swapped out-of-band. The clicking tab updates even without SSE.
- **SSE**: subscribe to the broadcast channel. On connect, **immediately send the current count** so a reconnecting client never shows stale data. Each update is an event named `count` whose data is the rendered `count.html` fragment (single line; strip newlines). Handle `RecvError::Lagged` by re-reading the current value from the DB and sending that. Use axum's built-in keep-alive.
- **Fragment shape** (`count.html`): `<span id="count" data-count="{{ count }}">{{ count }}</span>`. Rendered inside a container with `sse-swap="count"`.
- Use `hx-disabled-elt="this"` on the button so double-clicks during an in-flight request are ignored.

### Alpine rosette

The art must live **outside** the htmx swap target. If it lived inside, each swap would destroy and re-create the Alpine component. It is deterministic from the count, so every synced tab shows an identical picture, which visually verifies SSE sync.

```html
<div x-data="{
       n: {{ count }},
       petals(n) {
         let s = '';
         for (let i = 1; i <= Math.min(n, 80); i++)
           s += `<ellipse rx='${20 + i}' ry='6' transform='rotate(${i * 137.5})'
                  fill='hsl(${(i * 23 + n * 7) % 360} 70% 60% / .35)'/>`;
         return s;
       }
     }"
     @htmx:after-swap.camel.window="n = +document.getElementById('count').dataset.count">
  <svg viewBox="-110 -110 220 220" x-html="petals(n)"></svg>
</div>
```

Notes:
- `.camel` is required: htmx fires `htmx:afterSwap`, but HTML attributes are lowercased.
- `x-html` is used instead of `x-for` because `<template>` inside SVG is unreliable across browsers.
- **Verify** that SSE-driven swaps and OOB swaps both fire `htmx:afterSwap` in the vendored htmx version. If either doesn't, listen for the appropriate event (e.g. `htmx:sse-message` or `htmx:oobAfterSwap`) and note which in the ADR.

**Stop and check in after this phase.**

## Phase 3 — Verification

### Automated (tests/counter.rs)

Use the router directly with `tower::ServiceExt::oneshot` against an in-memory or temp-file SQLite DB.

- POST without a CSRF token → 403.
- POST with a wrong token → 403.
- POST with a valid token (obtained from a GET in the same session) → 200, response contains the incremented count and an OOB fragment.
- Global count persists across separate sessions; session count does not.
- A template rendered with a value like `<script>` comes out escaped (sanity check that escaping is on).

### Manual acceptance checklist

- [ ] Clicking increments both counts; refresh keeps the global count and the session count.
- [ ] A second tab (or a private window, i.e. a different session) sees the global count and rosette update live, with its own session count.
- [ ] Both tabs show an identical rosette at all times.
- [ ] Stopping and restarting the server: tabs reconnect automatically and show the correct current count.
- [ ] Rapid clicking doesn't double-submit or desync.
- [ ] Browser devtools show no JS errors and no requests to external hosts.

## Phase 4 — ADRs

Write ADRs in `docs/adr/` using a simple format: Context, Options considered (each with pros/cons), Decision, Consequences.

- **0001 — Frontend stack.** Options: SolidStart as a thin BFF in front of axum; axum + askama + htmx + Alpine (chosen); Astro with Solid islands; Leptos (rejected: custom HTML macro DSL, compile times, awkward JS payment SDK interop). Note the escape hatch: genuinely stateful widgets can become Solid islands wrapped as custom elements via `solid-element`.
- **0002 — CSRF approach.** Hand-rolled synchronizer token + Origin check vs an existing crate.
- **0003 — Live updates.** SSE + broadcast channel vs websockets; send-current-state-on-connect for reconnect correctness.
- Plus an ADR for any other fork you hit during implementation.

Also record in a short `NOTES.md`: how much custom JS was written (target: only the rosette), and anything that felt like fighting the stack. This is input for deciding whether the stack survives the order-tracker demo.

## Out of scope

Authentication, payments, image uploads, the order tracker itself, styling beyond basic readable CSS, deployment.