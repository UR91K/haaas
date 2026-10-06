# 0002 - CSRF: hand-rolled synchronizer token + Origin check

Date: 2026-10-06

## Context

All state changes come from htmx requests, so every non-GET request can carry a header. Sessions already exist (tower-sessions), so a per-session secret is cheap to keep.

## Options considered

1. **Existing crate** (e.g. an axum CSRF layer).
   - Pros: less code to write.
   - Cons: another dependency to vet; most are built around form fields or double-submit cookies and assume their own cookie/session handling; hides behaviour we want to see.
2. **Hand-rolled synchronizer token + Origin check** (chosen).
   - Pros: about 60 lines in `csrf.rs`; the token lives in the server-side session, so it cannot be forged from another origin; `base.html` sets it once through `hx-headers` for every htmx request; the Origin check adds defence in depth.
   - Cons: we own the security code. Limited to header-based requests, so plain HTML forms would need a hidden field (not needed so far).
3. **Double-submit cookie.**
   - Pros: stateless.
   - Cons: weaker against subdomain cookie injection; no benefit since we already have sessions.

## Decision

Option 2. A 32-byte random token (hex) is created on a session's first request. Every method except GET/HEAD must send it in `X-CSRF-Token`, compared in constant time, or get 403. If an `Origin` header is present, it must equal the `Host` header.

## Consequences

- Covered by tests: no token, wrong token, truncated token, bad origin, good origin.
- The Origin check compares strings, so deployment behind a proxy needs the proxy to pass `Host` through unchanged (or this check needs to read `X-Forwarded-Host`).
- Fresh sessions are created on any first GET, even for crawlers; acceptable for now.
- `Expiry::OnSessionEnd` means the token lives only as long as the browser session.