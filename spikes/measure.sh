#!/usr/bin/env sh
# Spike C host-side build-time measurement: clean release build of report-spike per engine.
# Output: markdown table on stdout. APK sizes come from CI (see .github/workflows/ci.yml).
set -eu
cd "$(dirname "$0")/.."
TARGET_DIR="${CARGO_TARGET_DIR:-target}/measure"
echo "| engine | clean release build (s) | incremental rebuild (s) |"
echo "|---|---|---|"
for engine in typst printpdf; do
  rm -rf "$TARGET_DIR"
  start=$(date +%s)
  CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p report-spike --no-default-features --features "$engine" -q
  clean=$(( $(date +%s) - start ))
  touch spikes/report-spike/src/lib.rs
  start=$(date +%s)
  CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p report-spike --no-default-features --features "$engine" -q
  incr=$(( $(date +%s) - start ))
  echo "| $engine | $clean | $incr |"
done
rm -rf "$TARGET_DIR"

echo
echo "Release-mode render time on this host (unit tests with --nocapture):"
cargo test --release -p report-spike --features typst,printpdf -q -- --nocapture 2>&1 | grep "\[measure\]"
