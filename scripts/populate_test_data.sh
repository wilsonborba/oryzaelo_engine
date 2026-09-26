#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: Synthetic Test Data Population (Rust Native API)
# ==============================================================================
# Directly consumes the native Rust endpoint POST /api/v1/admin/populate.
# ==============================================================================

set -euo pipefail

PORT="${PORT:-8005}"
DAYS="${1:-75}"
PARCELS="${2:-4}"

BASE_URL="http://127.0.0.1:$PORT"

if ! curl -s "$BASE_URL/health" > /dev/null 2>&1; then
    echo "⚠️  The engine is not running on port $PORT."
    echo "   Start it first with ./run_local_edge.sh or configure PORT=<port>."
    exit 1
fi

echo "🌾 Sending request to populate synthetic test data via native Rust API..."
RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/populate?days=$DAYS&parcels=$PARCELS")
echo "$RESPONSE" | jq . || echo "$RESPONSE"
