#!/usr/bin/env bash
# Local extras — heavy tools которые на GH free expensive / paid.
# Per ADR-22 software-development: запускается напрямую (без act / Docker), периодически или on-demand.
#
# Usage: ./scripts/local-extras.sh [trufflehog|mutants|coverage|all]
#
# Установка тулзов:
#   brew install trufflehog
#   cargo install cargo-mutants cargo-llvm-cov --locked
#   rustup component add llvm-tools-preview

set -euo pipefail

mode="${1:-all}"

run_trufflehog() {
  echo "=== trufflehog (verified credentials, full history) ==="
  if ! command -v trufflehog >/dev/null; then
    echo "ERROR: trufflehog не установлен. brew install trufflehog" >&2
    exit 1
  fi
  trufflehog filesystem --only-verified . || true
  trufflehog git --only-verified file://. || true
}

run_mutants() {
  echo "=== cargo-mutants (mutation testing) ==="
  if [ ! -f Cargo.toml ]; then
    echo "Skip mutants — не Rust репа"
    return
  fi
  if ! command -v cargo-mutants >/dev/null; then
    echo "ERROR: cargo-mutants не установлен. cargo install cargo-mutants --locked" >&2
    exit 1
  fi
  cargo mutants --no-shuffle --in-place --check
  cat mutants.out/missed.txt 2>/dev/null || echo "no missed mutants"
}

run_coverage() {
  echo "=== cargo-llvm-cov (coverage report) ==="
  if [ ! -f Cargo.toml ]; then
    echo "Skip coverage — не Rust репа"
    return
  fi
  if ! command -v cargo-llvm-cov >/dev/null; then
    echo "ERROR: cargo-llvm-cov не установлен. cargo install cargo-llvm-cov --locked + rustup component add llvm-tools-preview" >&2
    exit 1
  fi
  cargo llvm-cov --all-features --summary-only
  cargo llvm-cov --all-features --lcov --output-path lcov.info >/dev/null
  echo "Coverage report: lcov.info (открыть через VS Code Coverage Gutters / similar)"
}

case "$mode" in
  trufflehog) run_trufflehog ;;
  mutants) run_mutants ;;
  coverage) run_coverage ;;
  all)
    run_trufflehog
    echo
    run_mutants
    echo
    run_coverage
    ;;
  *)
    echo "Usage: $0 [trufflehog|mutants|coverage|all]" >&2
    exit 1
    ;;
esac
