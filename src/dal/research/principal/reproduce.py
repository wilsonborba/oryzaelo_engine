"""Rebuild the Thai study from versioned inputs, then check the reported scores."""
import csv
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path
from statistics import mean

from prepare_revision import ROOT, prepare

REFERENCE = ROOT / "src/dal/data/processed/principal"
OUTPUT = ROOT / "src/dal/data/local/reproduction/principal"


def check_predictions(directory):
    with (REFERENCE / "six_stage_folds.csv").open() as stream:
        ids = {row["source_row_id"] for row in csv.DictReader(stream)}
    for archive in sorted(REFERENCE.glob("*_oof.csv")):
        with (directory / archive.name).open() as stream:
            rows = list(csv.DictReader(stream))
        if len(rows) != 2370 or len(ids) != 2370 or {r["source_row_id"] for r in rows} != ids:
            raise ValueError(f"Incomplete or duplicate observations in {archive.name}")
        labels = sorted({r["true"] for r in rows})
        f1 = []
        for label in labels:
            support = sum(r["true"] == label for r in rows)
            predicted = sum(r["predicted"] == label for r in rows)
            correct = sum(r["true"] == r["predicted"] == label for r in rows)
            f1.append(2 * correct / (support + predicted))
        report = archive.name.replace("_oof.csv", "_metrics.json")
        target = json.loads((REFERENCE / report).read_text())["macro_f1"]
        if not math.isclose(mean(f1), target, abs_tol=1e-6):
            raise ValueError(f"{archive.name}: observed {mean(f1)}, expected {target}")
        print(f"{archive.stem}: {mean(f1):.6f} ({len(labels)} stages)", flush=True)


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["audit", "train"])
    action = parser.parse_args().action
    if action == "train":
        prepare(OUTPUT)
        subprocess.run([sys.executable, "-m", "src.domain.services.revision_study",
                        "--survey", str(OUTPUT / "rice_survey_analysis_v2.csv"),
                        "--out", str(OUTPUT), "--phase", "all"], cwd=ROOT, check=True)
        if hashlib.sha256((OUTPUT / "features.csv").read_bytes()).digest() != hashlib.sha256((REFERENCE / "features.csv").read_bytes()).digest():
            raise ValueError("Regenerated features differ from the published reference")
    check_predictions(OUTPUT if action == "train" else REFERENCE)
