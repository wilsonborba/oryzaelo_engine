#!/usr/bin/env bash
# ==============================================================================
# Oryza-Elo: População de Dados Sintéticos de Teste (Alta Fidelidade)
# ==============================================================================
# Preenche todas as tabelas do SQLite (parcels, weather_records, prediction_history,
# device_mappings e edge_config) com dados coerentes, realistas e multilíngues.
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
PYTHON_SCRIPT="$ENGINE_DIR/scripts/populate_test_data.py"

python3 "$PYTHON_SCRIPT" "$@"
