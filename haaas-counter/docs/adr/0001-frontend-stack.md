# 0001 - Frontend stack: axum + askama + htmx + Alpine

Date: 2026-10-06

## Context

The future artisan store needs custom and pre-made items, payments, and a chat-like live order tracker. We want a server-rendered stack that stays close to HTML and HTTP, with no JS build step and no HTML-in-macro DSL.

## Options considered

1. **SolidStart as a thin BFF in front of axum.**
   - Pros: rich client reactivity; good ecosystem for stateful widgets.
   - Cons: two servers and two runtimes; npm and a build step; an extra hop for every request; duplicated session/CSRF concerns.
2. **axum + askama + htmx + Alpine (HAAAS).**
   - Pros: one Rust binary; templates are real `.html` files; interactivity through HTML attributes; vendored JS, no build step; SSE is first-class in htmx.
   - Cons: highly stateful widgets are awkward; htmx/Alpine conventions have to be learned; askama errors surface at compile time and can be terse.
3. **Astro with Solid islands.**
   - Pros: mostly-static pages with islands where needed.
   - Cons: still a Node build and server; a second backend is needed for payments and sessions.
4. **Leptos.**
   - Pros: full-stack Rust, one language.
   - Cons: rejected. It uses a custom HTML macro DSL, compile times are long, and interop with JS payment SDKs is awkward.

## Decision

Option 2.

## Consequences

- Escape hatch: a genuinely stateful widget can become a Solid island wrapped as a custom element via `solid-element`, dropped into a template as a plain tag.
- Custom JS stays minimal; see NOTES.md for how much was written.
- The order-tracker demo is the real test of this decision.