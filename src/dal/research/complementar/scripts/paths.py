from pathlib import Path
import os

# scripts/ is one level below the complementary research guide.
REPO = Path(__file__).resolve().parents[5]
DATA = REPO / "src/dal/data/tcc"
SOURCES = DATA / "complementar/fontes"
RESULTS = Path(os.environ.get("ORYZELO_RESULTS", DATA / "complementar")).resolve()
THAI = DATA / "principal"
