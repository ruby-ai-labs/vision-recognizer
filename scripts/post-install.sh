#!/bin/bash
# Подписать бинари после cargo install.
#
# Для vision-recognizer-mcp Claude Code запускает через stdio — TCC обычно не требуется.
# Codesign предотвращает Gatekeeper-блокировку и даёт стабильную identity для TCC tracking
# (Microphone / Accessibility если когда-то понадобятся).
#
# Стратегия выбора подписи:
#   1. Если есть семейный cert "ruby-ai-labs-dev" — используем его (preferred).
#      Подробнее: handbook/manuals/family-codesign-cert.md
#   2. Иначе если есть legacy cert "voice-transcribe-dev" — используем его.
#   3. Иначе ad-hoc подпись (`codesign -s -`) — стабильная identity без Keychain, fallback.
set -e

BINS=(~/.cargo/bin/vision-recognizer ~/.cargo/bin/vision-recognizer-mcp)

pick_cert() {
  if security find-certificate -c "ruby-ai-labs-dev" >/dev/null 2>&1; then
    echo "ruby-ai-labs-dev"
  elif security find-certificate -c "voice-transcribe-dev" >/dev/null 2>&1; then
    echo "voice-transcribe-dev"
  else
    echo "-"  # ad-hoc
  fi
}

CERT=$(pick_cert)
if [ "$CERT" = "-" ]; then
  echo "WARN: именованный сертификат не найден, использую ad-hoc подпись (стабильная identity, без Keychain prompt)." >&2
  echo "Создать сертификат: handbook/manuals/family-codesign-cert.md" >&2
else
  echo "INFO: используется сертификат '$CERT'."
fi

for bin in "${BINS[@]}"; do
  [ -f "$bin" ] || { echo "skip $(basename "$bin") (not installed)"; continue; }
  codesign -f -s "$CERT" "$bin"
  echo "Signed: $(basename "$bin") (cert: $CERT)"
done
