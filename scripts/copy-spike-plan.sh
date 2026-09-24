#!/usr/bin/env sh
# Copies the client A1 plan into public/spike/ for local Spike A testing.
# public/spike/ is gitignored: client PDFs are never committed and CI builds without them.
set -eu
cd "$(dirname "$0")/.."
SRC="${1:-Examples/c-JS00-AM01.pdf}"
mkdir -p public/spike
cp "$SRC" public/spike/plan-a1.pdf
echo "copied $SRC -> public/spike/plan-a1.pdf ($(wc -c < public/spike/plan-a1.pdf) bytes)"
