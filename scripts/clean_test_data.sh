#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: Limpeza de Dados de Teste & Reset de Fábrica
# ==============================================================================
# Remove registros de teste em todas as tabelas e preserva os presets oficiais.
# ==============================================================================

set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"

if [ -f "$SCRIPT_DIR/Cargo.toml" ]; then
    ENGINE_DIR="$SCRIPT_DIR"
elif [ -f "$SCRIPT_DIR/../Cargo.toml" ]; then
    ENGINE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
else
    ENGINE_DIR="/home/wilsonborba/Documents/Others/Asodya/oryzaelo_engine"
fi

cd "$ENGINE_DIR"
PYTHON_SCRIPT="$ENGINE_DIR/scripts/clean_test_data.py"

python3 "$PYTHON_SCRIPT" "$@"
