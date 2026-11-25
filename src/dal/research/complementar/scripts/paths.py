"""Locations of versioned inputs and disposable experiment outputs."""
import os
from pathlib import Path

REPO = Path(__file__).resolve().parents[5]
DATA = REPO / "src/dal/data/processed"
RAW = Path(os.environ.get("ORYZELO_SOURCES", REPO / "src/dal/data/raw/complementar")).resolve()
OBS = Path(os.environ.get("ORYZELO_OBSERVATIONS", DATA / "complementar/observacoes")).resolve()
RESULTS = Path(os.environ.get("ORYZELO_RESULTS", REPO / "src/dal/data/local/reproduction/complementar-train")).resolve()
THAI = DATA / "principal"
