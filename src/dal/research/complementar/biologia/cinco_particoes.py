"""Apply the frozen six-stage TCC folds to the original XGBoost and the V2 hybrid."""
import sys
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import pandas as pd
from sklearn.metrics import confusion_matrix
from xgboost import XGBClassifier

from medir import ARMS, GROUPS, OUT, bench, enrich
from seasonal import analog

sys.path.insert(0, str(bench.ENGINE))
from src.domain.services.revision_study import encode


def evaluate(model, thai, ext, folds):
    raw_labels = np.array(sorted(thai.rice_stage.unique()))
    y_native = pd.Categorical(thai.rice_stage, categories=raw_labels).codes
    historical = pd.read_csv(bench.THDIR / "xgboost_six_oof.csv").set_index("source_row_id")
    predictions = []
    for fold in sorted(folds.unique()):
        fit = folds.ne(fold)
        tr, val = thai[fit], thai[~fit]
        if model in GROUPS:
            source = pd.concat([tr, ext[ext.country.isin(bench.ARMS[model])]], ignore_index=True)
            neighbors = analog(source, val, 10, .45)
        for arm in ["referencia", "interacao", "perfil_foliar"]:
            cols = bench.FEATURES + ARMS[arm]
            if model == "xgboost_original_TH":
                a, b, _ = encode(tr[cols], val[cols])
                clf = XGBClassifier(n_estimators=350, learning_rate=.06, max_depth=6, tree_method="hist",
                                    random_state=42+int(fold), eval_metric="mlogloss", n_jobs=4)
                clf.fit(a, y_native[fit])
                pred = clf.predict(b)
                if arm == "referencia":
                    assert np.array_equal(pred, historical.loc[val.source_row_id, "predicted"])
                labels = [bench.LABEL[s] for s in raw_labels[pred]]
            else:
                pipeline = bench.train(tr, ext, model, "cat", features=cols)
                pred = (.5*neighbors + .5*pipeline.predict_proba(val[cols])).argmax(axis=1)
                labels = np.asarray(bench.CLASSES)[pred]
            predictions.append(val[["source_row_id", "label"]].assign(modelo=model, braco=arm, fold=fold, pred=labels))
        print(model, int(fold), flush=True)
    return pd.concat(predictions)


if __name__ == "__main__":
    thai, ext = map(enrich, bench.dataset())
    mapping = pd.read_csv(bench.THDIR / "six_stage_folds.csv").set_index("source_row_id")
    assert mapping.index.is_unique and len(mapping) == len(thai) == thai.source_row_id.nunique() == 2370
    folds = thai.source_row_id.map(mapping.fold)
    assert set(folds) == {1, 2, 3, 4, 5}
    assert set(thai.parsed_date.dt.year) == {2023, 2024, 2025}
    with ThreadPoolExecutor(max_workers=3) as workers:
        runs = [workers.submit(evaluate, model, thai, ext, folds) for model in ["xgboost_original_TH", *GROUPS]]
        data = pd.concat([run.result() for run in runs], ignore_index=True)
    assert not data.duplicated(["modelo", "braco", "source_row_id"]).any() and len(data) == 2370*9
    data.to_csv(OUT / "cinco_particoes_predicoes.csv.gz", index=False, compression={"method":"gzip","mtime":0})
    scores = []
    for (model, arm), rows in data.groupby(["modelo", "braco"], sort=False):
        cm = confusion_matrix(rows.label, rows.pred, labels=bench.CLASSES)
        scores.append(dict(modelo=model, braco=arm, n=int(cm.sum()),
                           macro_f1=(2*cm.diagonal()/(cm.sum(axis=0)+cm.sum(axis=1))).mean()))
    summary = pd.DataFrame(scores)
    summary.to_csv(OUT / "cinco_particoes_metricas.csv", index=False)
    print(summary.to_string(index=False), flush=True)
