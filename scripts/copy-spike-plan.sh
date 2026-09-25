#!/usr/bin/env sh
# Copies client plans into public/spike/ for local viewer spikes (A1 plan; A4 plan 1A.pdf if present).
# public/spike/ is gitignored: client PDFs are never committed, CI builds without them, and only
# spike builds (VITE_SPIKES=1) keep them in dist/ (vite.config.ts).
set -eu
cd "$(dirname "$0")/.."
SRC="${1:-Examples/c-JS00-AM01.pdf}"
A4="${2:-Examples/1A.pdf}"
mkdir -p public/spike
cp "$SRC" public/spike/plan-a1.pdf
echo "copied $SRC -> public/spike/plan-a1.pdf ($(wc -c < public/spike/plan-a1.pdf) bytes)"
if [ -f "$A4" ]; then
  cp "$A4" public/spike/plan-a4.pdf
  echo "copied $A4 -> public/spike/plan-a4.pdf ($(wc -c < public/spike/plan-a4.pdf) bytes)"
fi
