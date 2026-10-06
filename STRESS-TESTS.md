# HAAAS Stress Tests — Does the Stack Survive the Store?

## Goal

The counter slice proved the glue core works. These are the hard things the artisan store (custom + pre-made items, payments, a chat-like order tracker) will need. Each one is a small spike meant to prove or disprove that axum + askama + htmx + Alpine + SQLite is the right stack for a bigger project.

Run them in order. Stop and decide after each one. Record results in `haaas-counter/NOTES.md` and write an ADR for any fork, as in `PLAN.md`.

## Rules

- Same philosophy as `PLAN.md`: no framework, no JS build step, no new dependency without asking first.
- Each spike is thin and throwaway. Do not build real store features.
- The escape hatch is a framework island (Solid wrapped as a custom element via `solid-element`, see ADR 0001). Using it is a valid outcome, not a failure, but note exactly what pushed us there.

## What counts as failure

- The same logic written twice (Rust and JS).
- State kept in sync by hand across layers.
- Custom JS growing faster than the features.
- Fighting swap timing, focus, or scroll instead of building the feature.

## 1. Chat-style order tracker (riskiest, do first)

**Build:** a message list that grows through SSE, a text input, optimistic send, a "typing…" indicator, and scroll that stays pinned to the bottom unless the user has scrolled up.

**Tests:**
- Swapping into a growing list without losing scroll position or input focus.
- The sender's own message appears once, not twice, when SSE echoes it back.
- Reconnect after a server restart shows the full, correct history.

**Pass:** it works with a small, understandable amount of JS (rough budget: about 50 lines of Alpine). **Fail:** swap timing and scroll fixes pile up. Then try a Solid island for just the message list and compare.

## 2. Per-entity live streams

**Build:** `/orders/{id}/events`, where each tab sees only its own order, with several orders updating at once and a check that the user may subscribe.

**Tests:**
- Per-order channels or filtering on the broadcast stay simple.
- Authorization on a long-lived stream is easy to get right, including a session that expires mid-stream.
- Missed events are recoverable by the send-current-state-on-connect rule (ADR 0003).

**Pass:** no bespoke protocol is needed beyond what the counter already has.

## 3. Payments with a JS SDK

**Build:** embed a payment provider's client widget (test mode only) in a page, with the server confirming through a webhook.

**Tests:**
- The third-party widget survives htmx swaps and a Content-Security-Policy.
- The page updates live from the webhook result via SSE.
- Keys stay server-side and test values only.

**Pass:** no workarounds just to keep the widget alive. This was the main reason Leptos was rejected.

## 4. Multi-step custom-order form

**Build:** a form with dependent fields, inline per-field errors, a file upload with preview, and back/forward between steps without losing input.

**Tests:**
- Partial validation without a full-page reload.
- How much form state the server has to carry between steps.
- Out-of-band error placement next to the right fields.

**Pass:** errors land beside the right fields without special-case JS.

## 5. Rich list interactions

**Build:** a filterable, sortable product grid with infinite scroll, and a variant picker that updates price and image instantly.

**Tests:**
- The boundary between server round trips and client-side rendering.
- Business rules (price, availability) are not duplicated in Rust and Alpine.

**Pass:** it feels instant where it should, with one source of truth.

## 6. Cross-cutting checks

- **Auth and sessions:** login, logout, expiry during a live stream.
- **Back button and caching:** htmx history behaviour after swaps.
- **Accessibility:** focus after a swap, `aria-live` announcements for live updates.
- **Load:** a couple of hundred open SSE connections; watch memory and broadcast lag.
- **Mobile Safari and flaky networks:** SSE reconnect on a phone.

## Decision

After the spikes, write one ADR: keep the stack, keep it with islands for named widgets, or move on. Base it on how much custom JS was written, how often the stack was fought, and whether the same logic had to live in two places.

## Out of scope

Real store features, deployment, visual design, anything not needed to answer "does the stack survive this?".
