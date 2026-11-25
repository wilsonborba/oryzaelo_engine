from pathlib import Path
import os

# scripts/ is one level below the complementary research guide.
REPO = Path(__file__).resolve().parents[5]
DATA = REPO / "src/dal/data/tcc"
SOURCES = Path(os.environ.get("ORYZELO_SOURCES", DATA / "complementar/fontes")).resolve()
RESULTS = Path(os.environ.get("ORYZELO_RESULTS", REPO / ".research-run/complementar-train")).resolve()
THAI = DATA / "principal"
