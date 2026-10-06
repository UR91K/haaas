# 0003 - Live updates: SSE + broadcast channel

Date: 2026-10-06

## Context

The order tracker will push updates to open pages. The counter is the spike for it: any tab must see the global count change, and must be correct after reconnecting.

## Options considered

1. **WebSockets.**
   - Pros: bidirectional.
   - Cons: more protocol and state to manage; we only need server to client; htmx's SSE extension is simpler; no automatic reconnect.
2. **SSE + `tokio::sync::broadcast`** (chosen).
   - Pros: plain HTTP; the browser's EventSource reconnects by itself; htmx's `sse-swap` swaps the fragment with no custom JS; axum has `Sse` with keep-alive.
   - Cons: one-way only (fine, since actions are normal POSTs); each tab holds an open connection; the broadcast channel is in-process, so more than one server instance needs a different fan-out.
3. **Polling with `hx-trigger="every 2s"`.**
   - Pros: simplest.
   - Cons: latency, wasted requests, not representative of the tracker.

## Decision

Option 2. Details that matter for correctness:

- **Send current state on connect.** The stream starts with the current DB value, so a client that reconnects after a restart or network drop never shows stale data. The handler subscribes to the channel before reading the value so an update in between cannot be lost.
- **Lagged receivers resync.** On `RecvError::Lagged` the stream re-reads the value from the DB and sends that.
- The event name is `count`, the data is the rendered `count.html` fragment as a single line.
- The channel carries only the new value; the source of truth stays SQLite.

## Consequences

- **htmx events (verified in Chrome with htmx 2.0.11):** `htmx:afterSwap` fires for both SSE-driven swaps and for out-of-band swaps, so the rosette's `@htmx:after-swap.camel.window` listener works unchanged. `htmx:sseMessage` and `htmx:oobAfterSwap` also fire but are not needed.
- A restarted server shows a console error (the dropped EventSource) until reconnect; harmless.
- The stream needs a `Stream` type, which required `tokio-stream` (ADR 0006).
- Multiple server instances would need a shared bus (Postgres LISTEN, Redis, or similar).