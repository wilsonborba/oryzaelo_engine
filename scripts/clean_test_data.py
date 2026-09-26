#!/usr/bin/env python3
"""
Oryza-Elo Edge Engine: Database Cleaner & Factory Reset Script
Cleans test data across all tables (parcels, weather_records, prediction_history,
custom device mappings, edge_config) while preserving official factory presets.
"""

import argparse
import os
import sqlite3
import sys

def clean_database(db_path: str, reset_presets: bool = False):
    abs_db = os.path.abspath(db_path)
    if not os.path.exists(abs_db):
        print(f"ℹ️  Base de dados não encontrada em {abs_db}. Nada a limpar.")
        return

    print(f"🧹 Conectando ao banco SQLite para limpeza: {abs_db}")
    conn = sqlite3.connect(abs_db)
    cursor = conn.cursor()

    # Coletar estatísticas antes da limpeza
    tables = ["parcels", "device_mappings", "weather_records", "prediction_history", "edge_config"]
    before = {}
    for t in tables:
        try:
            cursor.execute(f"SELECT COUNT(*) FROM {t}")
            before[t] = cursor.fetchone()[0]
        except sqlite3.OperationalError:
            before[t] = 0

    print("\n[1/3] Removendo registros operacionais de teste...")
    cursor.execute("DELETE FROM weather_records;")
    cursor.execute("DELETE FROM prediction_history;")
    cursor.execute("DELETE FROM parcels;")

    if reset_presets:
        print("  • Removendo todos os device mappings (inclusive presets)...")
        cursor.execute("DELETE FROM device_mappings;")
    else:
        print("  • Removendo apenas device mappings customizados (preservando presets oficiais)...")
        cursor.execute("DELETE FROM device_mappings WHERE is_preset = 0;")

    cursor.execute("DELETE FROM edge_config;")

    conn.commit()

    print("\n[2/3] Executando VACUUM para recuperação de espaço em disco...")
    cursor.execute("VACUUM;")
    conn.commit()

    # Coletar estatísticas após a limpeza
    after = {}
    for t in tables:
        try:
            cursor.execute(f"SELECT COUNT(*) FROM {t}")
            after[t] = cursor.fetchone()[0]
        except sqlite3.OperationalError:
            after[t] = 0

    conn.close()
    db_size = os.path.getsize(abs_db) / 1024.0

    print("\n[3/3] Resumo da Limpeza:")
    print("==================================================================")
    print("🧹  BANCO DE DADOS LIMPO COM SUCESSO (ESTADO DE FÁBRICA)  🧹")
    print("==================================================================")
    print(f"  {'Tabela':<22} | {'Antes':>8} | {'Depois':>8} | {'Removidos':>10}")
    print("  " + "-" * 56)
    for t in tables:
        rem = before[t] - after[t]
        print(f"  {t:<22} | {before[t]:>8} | {after[t]:>8} | {rem:>10}")
    print("==================================================================")
    print(f"  Tamanho final no disco: {db_size:.1f} KB")
    print(f"  Arquivo: {abs_db}")
    print("==================================================================")

def main():
    parser = argparse.ArgumentParser(description="Oryza-Elo Database Cleanup & Reset")
    parser.add_argument("--db", default="src/dal/data/local/oryza_elo_edge.db", help="Caminho do banco SQLite")
    parser.add_argument("--all", action="store_true", help="Remove também os presets de fábrica de device_mappings")
    args = parser.parse_args()

    clean_database(args.db, args.all)

if __name__ == "__main__":
    main()
