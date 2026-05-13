---
id: ADR-0001
title: base64 crate — non-canon dependency
status: accepted
date: 2026-05-13
---

# ADR-0001: base64 crate — non-canon dependency

## Status

Accepted

## Context

`vision-recognizer` requires base64 encoding to construct `data:image/jpeg;base64,...` URLs for the
OpenAI Vision API `image_url` content type. The Rust standard library does not expose a stable
public API for base64 encoding as of Rust 1.87 (stable, May 2026).

The `rust-crate-canon` in `software-development/handbook` does not include a base64 crate.

## Decision

Use `base64 = "0.22"` (the `base64` crate on crates.io). This is the canonical third-party base64
library for Rust: maintained by Alice Maz, widely used (>200M downloads), most recent release
0.22.1 (2024). API used: `base64::engine::general_purpose::STANDARD.encode(&bytes)`.

## Rationale

- No stable stdlib alternative exists in Rust.
- `base64` 0.22 is well-maintained, actively patched, MIT-licensed, and widely adopted.
- Scope is narrow: one callsite in `openai_vision.rs::recognize()`.
- Adding to canon would require a 3rd project with the same need (YAGNI per ADR-24). This is the
  first project in the department requiring image encoding.
- Context7 lookup returned no library ID for `base64`; manual verification confirmed crate health.

## Alternatives considered

- Inline base64 implementation: rejected (security and maintenance risk for a well-solved problem).
- `data-encoding` crate: also not in canon; `base64` has higher adoption and simpler API for this use case.
- Wait for stdlib: no timeline for stable public base64 API.

## Consequences

- `cargo deny check` passes because `base64` is MIT-licensed (in the allowed list).
- `cargo audit` finds no advisories for `base64` 0.22.x.
- If a second department project needs base64, nominate `base64` for canon inclusion.
