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
out = REPO / ".research-run" / ("complementar-" + action)
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
import json
for arm, result in json.loads((out / "results/hybrid_metrics.json").read_text()).items():
    expected = json.loads((source / "results/analogue_forward.json").read_text())[arm]["macro_f1"]
    if abs(result["metrics"]["macro avg"]["f1-score"] - expected) > 1e-6:
        raise ValueError(f"Unexpected six-stage Macro-F1 for {arm}")
    print(f"{arm}: {expected:.6f}")
print(f"Verified comparative results in {out}")
