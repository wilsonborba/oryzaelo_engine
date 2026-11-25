"""Ablate fixed cultivar traits in the published Oryzelo V2 temporal benchmark."""
import hashlib
import json
import sys
from importlib.metadata import version
from pathlib import Path

import numpy as np
import pandas as pd

HERE = Path(__file__).resolve().parent
V2 = HERE.parent
sys.path.insert(0, str(V2 / "scripts"))
import benchmark as bench
from seasonal import analog

SOURCES = bench.SOURCES / "biologia"
PHOTO = ["has_photo", "sensitive"]
INTERACTION = PHOTO + ["sensitive_daylength"]
IDS = ["identity_" + name for name in ["RD41", "PTT1", "KDML105"]]
NPK = ["leaf_" + x for x in ["N", "P", "K"]]
ARMS = {"referencia": [], "cobertura": ["has_photo"], "fotoperiodo": PHOTO,
        "interacao": INTERACTION, "identidade": INTERACTION + IDS,
        "perfil_foliar": INTERACTION + ["has_leaf"] + NPK}
GROUPS = ["TH", "TH_ES_US_JP"]
OUT = bench.RESULTS / "biologia"


def enrich(df):
    lookup = pd.read_csv(SOURCES / "cultivares_fotoperiodo.csv").set_index("chave_sem_espacos", verify_integrity=True)
    traits = pd.read_csv(SOURCES / "folha_npk_tailandia.csv").query("estadio_estudo == 3").set_index("cultivar", verify_integrity=True)
    key = df.rice_variety.str.replace(r"\s+", "", regex=True).where(df.country.eq("TH"))
    cultivar = key.map(lookup.cultivar)
    sensitivity = key.map(lookup.fotoperiodo)
    assert set(lookup.fotoperiodo) == {"sensível", "insensível"}
    enriched = df.copy()
    enriched["has_photo"] = sensitivity.notna().astype(int)
    enriched["sensitive"] = sensitivity.eq("sensível").astype(int)
    enriched["sensitive_daylength"] = enriched.sensitive * (df.photoperiod_hours - 12)
    enriched["has_leaf"] = cultivar.isin(traits.index).astype(int)
    for name in traits.index:
        enriched["identity_" + name] = cultivar.eq(name).astype(int)
    for element in ["N", "P", "K"]:
        enriched["leaf_" + element] = cultivar.map(traits[element + "_pct_massa_seca"]).fillna(0)
    return enriched


def run():
    OUT.mkdir(exist_ok=True)
    thai, external = map(enrich, bench.dataset())
    dev = thai.loc[thai.parsed_date.dt.year <= 2024].reset_index(drop=True)
    test = thai.loc[thai.parsed_date.dt.year == 2025].reset_index(drop=True)
    folds = pd.read_csv(bench.OUT / "development_folds.csv").set_index("source_row_id", verify_integrity=True)
    folds = folds.loc[dev.source_row_id].reset_index()
    assert np.array_equal(folds.group, bench.cluster(dev))
    assert folds.groupby("group").fold.nunique().max() == 1
    assert set(dev.source_row_id).isdisjoint(test.source_row_id)
    assert len(test) == 856 and len(dev) == len(folds) == 1514
    assert thai.has_photo.sum() == 463 and thai.has_leaf.sum() == 298
    assert external.has_photo.sum() == external.has_leaf.sum() == 0
    expected = pd.read_csv(bench.OUT / "analogue_forward_predictions.csv").set_index("source_row_id")
    prior = json.loads((bench.OUT / "analogue_forward.json").read_text())
    params = json.loads((bench.OUT / "analogue_selection.json").read_text())
    records = []
    splits = [(f"fold_{int(k)}", dev.loc[folds.fold.ne(k)], dev.loc[folds.fold.eq(k)]) for k in sorted(folds.fold.unique())]
    splits.append(("2025", dev, test))
    for group in GROUPS:
        days, space, fraction = params[group]
        assert (days, space, fraction) == (10, .45, .5)
        outside = external[external.country.isin(bench.ARMS[group])]
        for split, train, query in splits:
            similarity = analog(pd.concat([train, outside], ignore_index=True), query, days, space)
            for arm, extra in ARMS.items():
                features = bench.FEATURES + extra
                pipeline = bench.train(train, external, group, "cat", features=features)
                forest = pipeline.predict_proba(query[features])
                probabilities = fraction * similarity + (1 - fraction) * forest
                predicted = probabilities.argmax(axis=1)
                assert np.isfinite(probabilities).all() and np.allclose(probabilities.sum(axis=1), 1)
                if arm == "referencia" and split == "2025":
                    assert np.array_equal(np.array(bench.CLASSES)[predicted], expected.loc[query.source_row_id, group])
                result = query[["source_row_id", "parsed_date", "y", "has_photo", "sensitive", "has_leaf"]].copy()
                result = result.assign(group=group, arm=arm, split=split, cell=bench.cluster(query), pred=predicted)
                result[["p_" + c for c in bench.CLASSES]] = probabilities
                records.append(result)
            print(group, split, "finished", flush=True)
    data = pd.concat(records, ignore_index=True)
    for group in GROUPS:
        oof = data.query("group == @group and arm == 'referencia' and split != '2025'")
        assert np.isclose(bench.score(oof.y, oof.pred), prior[group]["cv_macro_f1"])
    assert not data.duplicated(["arm", "group", "source_row_id"]).any()
    data.to_csv(OUT / "predicoes.csv.gz", index=False, compression={"method": "gzip", "mtime": 0})
    paths = {"thai": bench.THDIR / "features.csv", "external": bench.SOURCES / "international_features.csv.gz",
             "folds": bench.OUT / "development_folds.csv", "photoperiod": SOURCES / "cultivares_fotoperiodo.csv",
             "nutrition": SOURCES / "folha_npk_tailandia.csv", "protocol": HERE / "protocolo.md",
             "fit_script": Path(__file__), "benchmark_script": V2 / "scripts/benchmark.py",
             "analogue_script": V2 / "scripts/seasonal.py"}
    manifest = {"features": ARMS, "seed": bench.SEED, "external_rows_after_sampling": external.country.value_counts().to_dict(),
                "classes": bench.CLASSES, "versions": {k: version(k) for k in ["catboost", "numpy", "pandas", "scikit-learn", "scipy"]},
                "sha256": {k: hashlib.sha256(p.read_bytes()).hexdigest() for k, p in paths.items()}}
    (OUT / "manifesto.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    run()
