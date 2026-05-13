# `vision-recognizer` — OpenAI Vision API MCP server

## Назначение

Rust MCP stdio server для распознавания изображений через OpenAI Vision API.
Используется как MCP tool `vision.recognize_image` в `personal-assistant`.

Два бинаря:

- `vision-recognizer-mcp` — MCP stdio server (основной, запускается Claude Code через `.mcp.json`).
- `vision-recognizer` — CLI entry: `vision-recognizer mcp` запускает тот же server.

## Run locally

- **Build:** `cargo build --all-targets --all-features`
- **Run MCP server:** `vision-recognizer-mcp` (stdio, JSON-RPC)
- **Run via CLI:** `cargo run --bin vision-recognizer -- mcp`
- **Test:** `cargo nextest run --all-features`
- **Lint:** `cargo clippy --all-targets --all-features -- -D warnings`
- **Format:** `cargo fmt --check`
- **Doc:** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`
- **Gates:** `./scripts/pre-push-ci.sh`

Smoke/e2e тесты — в отделе software-development (per ADR-15), здесь не описываются.

## Структура директорий

- `src/` — исходный код:
  - `lib.rs` — re-exports
  - `main.rs` — CLI binary entry
  - `main_mcp.rs` — MCP binary entry
  - `mcp.rs` — `VisionHandler` с tool `vision.recognize_image`
  - `openai_vision.rs` — `VisionClient` HTTP wrapper
- `tests/` — integration tests (wiremock + black-box MCP spawn)
- `scripts/` — вспомогательные скрипты (pre-push-ci.sh, post-install.sh, local-extras.sh)
- `docs/adr/` — Architecture Decision Records

## Установка

```bash
cargo install --path . --locked
# Code signing (macOS)
./scripts/post-install.sh
```

`OPENAI_API_KEY` должен быть доступен в env при старте сервера.
В контексте `personal-assistant` — через `secrets-vault run -- claude`.
