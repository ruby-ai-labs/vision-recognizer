#!/bin/bash
# Подписать бинари после cargo install.
#
# Для vision-recognizer-mcp Claude Code запускает через stdio — TCC обычно не требуется.
# Codesign предотвращает Gatekeeper-блокировку и даёт стабильную identity для TCC tracking
# (Microphone / Accessibility если когда-то понадобятся).
#
# Стратегия выбора подписи:
#   1. Если есть именованный cert "vision-recognizer-dev" — используем его (preferred).
#   2. Иначе если есть общий family cert "voice-transcribe-dev" — используем его (общий cert на семейные бинари OK для home-use).
#   3. Иначе ad-hoc подпись (`codesign -s -`) — стабильная identity без Keychain, fallback.
set -e

BINS=(~/.cargo/bin/vision-recognizer ~/.cargo/bin/vision-recognizer-mcp)

pick_cert() {
  if security find-certificate -c "vision-recognizer-dev" >/dev/null 2>&1; then
    echo "vision-recognizer-dev"
  elif security find-certificate -c "voice-transcribe-dev" >/dev/null 2>&1; then
    echo "voice-transcribe-dev"
  else
    echo "-"  # ad-hoc
  fi
}

CERT=$(pick_cert)
if [ "$CERT" = "-" ]; then
  echo "INFO: именованный сертификат не найден, использую ad-hoc подпись (стабильная identity, без Keychain prompt)."
fi

for bin in "${BINS[@]}"; do
  [ -f "$bin" ] || { echo "skip $bin (not installed)"; continue; }
  codesign -f -s "$CERT" "$bin"
  echo "Signed: $(basename "$bin") (cert: $CERT)"
done
