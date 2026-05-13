#!/bin/bash
# Подписать бинари после cargo install
# Для vision-recognizer-mcp запускается Claude Code через stdio — TCC не требуется.
# Codesign предотвращает Gatekeeper-блокировку при первом запуске.
CERT="vision-recognizer-dev"
if ! security find-certificate -c "$CERT" >/dev/null 2>&1; then
  echo "WARN: сертификат '$CERT' не найден. Gatekeeper может запросить подтверждение при первом запуске." >&2
  echo "Создать сертификат: Keychain Access → Certificate Assistant → Create a Certificate → Code Signing." >&2
  exit 0
fi
codesign -f -s "$CERT" ~/.cargo/bin/vision-recognizer && echo "Signed: vision-recognizer"
codesign -f -s "$CERT" ~/.cargo/bin/vision-recognizer-mcp && echo "Signed: vision-recognizer-mcp"
