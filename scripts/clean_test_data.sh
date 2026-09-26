#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: Limpeza de Dados de Teste & Reset (Reutilização da API Rust)
# ==============================================================================
# Consome diretamente o endpoint Rust nativo POST /api/v1/admin/clean.
# ==============================================================================

set -euo pipefail

PORT="${PORT:-8005}"
BASE_URL="http://127.0.0.1:$PORT"

if ! curl -s "$BASE_URL/health" > /dev/null 2>&1; then
    echo "⚠️  O engine não está em execução na porta $PORT."
    echo "   Inicie primeiro com ./run_local_edge.sh ou configure PORT=<porta>."
    exit 1
fi

echo "🧹 Enviando requisição para limpar base de teste via API Rust nativa..."
RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/clean")
echo "$RESPONSE" | jq . || echo "$RESPONSE"
