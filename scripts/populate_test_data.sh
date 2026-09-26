#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: População de Dados Sintéticos de Teste (Reutilização da API Rust)
# ==============================================================================
# Consome diretamente o endpoint Rust nativo POST /api/v1/admin/populate.
# ==============================================================================

set -euo pipefail

PORT="${PORT:-8005}"
DAYS="${1:-75}"
PARCELS="${2:-4}"

BASE_URL="http://127.0.0.1:$PORT"

if ! curl -s "$BASE_URL/health" > /dev/null 2>&1; then
    echo "⚠️  O engine não está em execução na porta $PORT."
    echo "   Inicie primeiro com ./run_local_edge.sh ou configure PORT=<porta>."
    exit 1
fi

echo "🌾 Enviando requisição para popular dados sintéticos via API Rust nativa..."
RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/populate?days=$DAYS&parcels=$PARCELS")
echo "$RESPONSE" | jq . || echo "$RESPONSE"
