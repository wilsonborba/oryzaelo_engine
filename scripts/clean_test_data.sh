#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: Clean Test Data & Database Reset (Rust Native API)
# ==============================================================================
# Directly consumes the native Rust endpoint POST /api/v1/admin/clean.
# ==============================================================================

set -euo pipefail

PORT="${PORT:-8005}"
BASE_URL="http://127.0.0.1:$PORT"

if ! curl -s "$BASE_URL/health" > /dev/null 2>&1; then
    echo "⚠️  The engine is not running on port $PORT."
    echo "   Start it first with ./run_local_edge.sh or configure PORT=<port>."
    exit 1
fi

echo "🧹 Sending request to clean test database via native Rust API..."
RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/clean")
echo "$RESPONSE" | jq . || echo "$RESPONSE"
