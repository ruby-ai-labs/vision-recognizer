#!/usr/bin/env bash
# pre-push CI gate for vision-recognizer (ADR-30 software-development)
# Equivalent to .github/workflows/ci.yml gates running locally.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(git rev-parse --show-toplevel)"

echo "==> markdown quality"
npx markdownlint-cli2 '**/*.md' '#node_modules' '#target'
npx --yes lychee --offline '**/*.md' --exclude-path .git --exclude-path target
typos

echo "==> workflow lint"
actionlint

echo "==> cargo build"
cargo build --all-targets --all-features --locked

echo "==> tests"
cargo nextest run --all-features --locked --no-tests=warn

echo "==> clippy"
cargo clippy --all-targets --all-features --locked -- -D warnings

echo "==> fmt"
cargo fmt --check

echo "==> doc"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --locked

echo "==> unused deps"
cargo machete

echo "==> gitleaks"
gitleaks detect --config .gitleaks.toml --no-banner --redact --exit-code 1

echo "==> cargo audit"
cargo audit --deny warnings

echo "==> cargo deny"
cargo deny check

echo ""
echo "pre-push-ci: ALL GATES PASSED"
