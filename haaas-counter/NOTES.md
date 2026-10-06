# Notes for the order-tracker decision

## How much custom JS

Only the rosette: one Alpine `x-data` object (~10 lines: `n` and a `petals(n)` function) and one `@htmx:after-swap` listener line, in `templates/index.html`; plus a one-line `hx-on::before-request` in `base.html` that clears the error area. No other JS was written. Everything else is htmx attributes (`hx-post`, `hx-target`, `hx-swap`, `hx-disabled-elt`, `sse-connect`, `sse-swap`, `hx-headers`) and vendored libraries.

## What felt like fighting the stack

- **Crate version skew, not the HAAAS pieces.** `tower-sessions-sqlx-store` 0.15.0 is built against `tower-sessions-core` 0.14 and sqlx 0.8, so we are pinned to `tower-sessions` 0.14 and sqlx 0.8 (ADR 0004, 0005). This is the biggest friction and would be removed by a ~80-line hand-written session store.
- **SSE needs a `Stream` crate** that no listed dependency provided (ADR 0006).
- **Error handling vs. htmx.** `IntoResponse` can't see the request, so "page vs. fragment" is decided by a middleware reading `HX-Request` and an extension set by `AppError`. It works, but it is indirection.
- **htmx does not swap 4xx/5xx by default.** The error fragment (retargeted to `#errors` by `HX-Retarget`) never appeared in Chrome until `base.html` got an `htmx-config` meta tag with `responseHandling` set to swap error responses. A second `hx-on::before-request` on `<body>` clears `#errors` so a stale error doesn't outlive the next success. That is one more line of custom JS. htmx still logs the 4xx to the console.
- **Out-of-band swaps are by element id**, so `#count` appears once on the page and the SSE container wraps it; the same fragment is used for SSE and OOB with an `oob` flag. Fine, but ids are global and easy to collide.
- **Askama inside inline JS/attributes**: the rosette's JS has to avoid `{{`, `{%` and `{#` sequences. Not hit yet, but a trap.
- **Not stack-related:** Windows PowerShell wrote non-ASCII template text in a codepage askama rejected ("not valid UTF-8").

## What worked well

- The SSE reconnect story: no custom code; the server restart test passed with the tab recovering by itself.
- Tests run against the router directly with no browser (8 passing).
- One binary, no build step, and the rosette survives swaps because it sits outside every swap target.

## For the order tracker

Per-order streams will need per-order channels or filtering on the broadcast, and the in-process channel won't scale past one instance. Chat-like input (typing, scroll anchoring) is where Alpine or a Solid island may be needed.