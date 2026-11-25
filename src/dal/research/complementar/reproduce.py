"""Recompute comparative tables or refit the CPU experiments in an isolated output directory."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "scripts"))
from paths import DATA, REPO

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("action", choices=["audit", "train"])
action = parser.parse_args().action
out = REPO / "src/dal/data/local/reproduction" / ("complementar-" + action)
source = DATA / "complementar"
for section in ("results", "biologia", "modelos_recentes"):
    if action == "audit" or section == "modelos_recentes":
        shutil.copytree(source / section, out / section, dirs_exist_ok=True)
    else:
        (out / section).mkdir(parents=True, exist_ok=True)
env = dict(os.environ, ORYZELO_RESULTS=str(out), OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS="4")
scripts = []
if action == "train":
    scripts.extend(["scripts/benchmark.py", "scripts/reference.py", "scripts/sensitivity.py",
                    "scripts/seasonal.py", "biologia/medir.py", "biologia/cinco_particoes.py"])
scripts.extend(["scripts/analyse.py", "biologia/analisar.py", "modelos_recentes/resumir.py",
                "modelos_recentes/diagnostico.py"])
for script in scripts:
    subprocess.run([sys.executable, str(HERE / script)], env=env, cwd=REPO, check=True)

if action == "train":
    for command in [["export.py"], ["candidate.py", "build"], ["candidate.py", "verify"]]:
        subprocess.run([sys.executable, str(HERE / "scripts" / command[0]), *command[1:]],
                       env=env, cwd=REPO, check=True)

import pandas as pd
for section, filename in [("biologia", "metricas.csv"), ("biologia", "contrastes_2025.csv"),
                          ("modelos_recentes", "metricas.csv"), ("modelos_recentes", "por_estadio.csv")]:
    pd.testing.assert_frame_equal(pd.read_csv(out / section / filename),
                                  pd.read_csv(source / section / filename), rtol=1e-6, atol=1e-9)
from sklearn.metrics import f1_score
import json

classes = ["seedling", "tillering", "booting", "heading", "flowering", "preharvest"]
rows = pd.read_csv(out / "results/forward_predictions.csv.gz")
expected_rows = pd.read_csv(source / "results/archived_spec_forward.csv")
if len(rows) != 856 or rows.source_row_id.duplicated().any():
    raise ValueError("Expected exactly one forecast per 2025 visit")
pd.testing.assert_series_equal(rows.source_row_id, expected_rows.source_row_id)
reports = json.loads((source / "results/forward_scores.json").read_text())
for model, report in reports.items():
    observed = f1_score(rows.label, rows[model].map(dict(enumerate(classes))), labels=classes, average="macro")
    if abs(observed - report["macro_f1"]) > 1e-6:
        raise ValueError(f"Unexpected international score for {model}")

biology = pd.read_csv(out / "biologia/cinco_particoes_predicoes.csv.gz")
for row in pd.read_csv(source / "biologia/cinco_particoes_metricas.csv").itertuples():
    group = biology[(biology.modelo == row.modelo) & (biology.braco == row.braco)]
    if group.source_row_id.nunique() != 2370 or len(group) != 2370:
        raise ValueError(f"Incomplete biological comparison for {row.modelo}/{row.braco}")
    observed = f1_score(group.label, group.pred, labels=classes, average="macro")
    if abs(observed - row.macro_f1) > 1e-6:
        raise ValueError(f"Unexpected biological score for {row.modelo}/{row.braco}")

for arm, result in json.loads((out / "results/hybrid_metrics.json").read_text()).items():
    expected = json.loads((source / "results/analogue_forward.json").read_text())[arm]["macro_f1"]
    if abs(result["metrics"]["macro avg"]["f1-score"] - expected) > 1e-6:
        raise ValueError(f"Unexpected six-stage Macro-F1 for {arm}")
    print(f"{arm}: {expected:.6f}")
print(f"Verified comparative results in {out}")
