"""Calcula os contrastes pareados nas previsões congeladas de medir.py."""
import json

import numpy as np
import pandas as pd
from sklearn.metrics import confusion_matrix

from medir import ARMS, GROUPS, OUT, bench

N = len(bench.CLASSES)


def measures(cm):
    support = cm.sum(axis=-1)
    predicted = cm.sum(axis=-2)
    hit = np.diagonal(cm, axis1=-2, axis2=-1)
    denom = support + predicted
    f1 = np.divide(2 * hit, denom, out=np.zeros_like(hit, dtype=float), where=denom > 0)
    return f1, f1.mean(axis=-1), hit.sum(axis=-1) / support.sum(axis=-1)


def matrix(frame):
    return confusion_matrix(frame.y, frame.pred, labels=list(range(N)))


def main():
    data = pd.read_csv(OUT / "predicoes.csv.gz")
    overall, by_class, coverage, contrasts, confusions = [], [], [], [], {}
    cell_influence = []
    for group in GROUPS:
        group_data = data[data.group == group]
        for split in ["fold_0", "fold_1", "fold_2", "oof", "2025"]:
            section = group_data[group_data.split != "2025"] if split == "oof" else group_data[group_data.split == split]
            for arm, rows in section.groupby("arm", sort=False):
                cm = matrix(rows)
                f1, macro, acc = measures(cm)
                overall.append(dict(treino=group, amostra=split, braco=arm, n=len(rows), macro_f1=macro, acuracia=acc))
                if split not in ("oof", "2025"):
                    continue
                confusions[f"{group}/{split}/{arm}"] = cm.tolist()
                for i, cls in enumerate(bench.CLASSES):
                    by_class.append(dict(treino=group, amostra=split, braco=arm, classe=cls, n=cm[i].sum(),
                                         preditas=cm[:, i].sum(), acertos=cm[i, i], f1=f1[i]))
                for trait in ["has_photo", "has_leaf"]:
                    for flag, block in rows.groupby(trait):
                        subcm = matrix(block)
                        _, subscore, subacc = measures(subcm)
                        coverage.append(dict(treino=group, amostra=split, braco=arm, traco=trait, presente=flag,
                                             n=len(block), celulas=block.cell.nunique(), classes_observadas=block.y.nunique(),
                                             macro_f1_seis_classes=subscore, acuracia=subacc))
        forward = group_data[group_data.split == "2025"]
        base = forward[forward.arm == "referencia"].set_index("source_row_id").sort_index()
        cells, geo = np.unique(base.cell, return_inverse=True)
        draws = np.random.default_rng(bench.SEED).multinomial(len(cells), np.full(len(cells), 1 / len(cells)), size=5000)
        distributions, scores, without_cell = {}, {}, {}
        for arm in ARMS:
            rows = forward[forward.arm == arm].set_index("source_row_id").loc[base.index]
            assert np.array_equal(rows.y, base.y) and np.array_equal(rows.cell, base.cell)
            cms = np.zeros((len(cells), N, N), dtype=int)
            np.add.at(cms, (geo, rows.y, rows.pred), 1)
            boot = (draws @ cms.reshape(len(cells), -1)).reshape(-1, N, N)
            all_classes = (boot.sum(axis=2) > 0).all(axis=1)
            distributions[arm] = measures(boot[all_classes])[1]
            scores[arm] = measures(cms.sum(axis=0))[1]
            without_cell[arm] = measures(cms.sum(axis=0)[None, :, :] - cms)[1]
        pairs = [(a, "referencia") for a in ARMS if a != "referencia"]
        pairs.extend([("fotoperiodo", "cobertura"), ("interacao", "fotoperiodo"),
                      ("perfil_foliar", "interacao"), ("perfil_foliar", "identidade")])
        for arm, control in pairs:
            dist = distributions[arm] - distributions[control]
            lo, hi = np.quantile(dist, [.025, .975])
            contrasts.append(dict(treino=group, braco=arm, comparador=control, delta=scores[arm]-scores[control],
                                  ic95_inf=lo, ic95_sup=hi, celulas=len(cells), replicas_validas=len(dist)))
        for cell, delta in zip(cells, without_cell["perfil_foliar"] - without_cell["referencia"]):
            cell_influence.append(dict(treino=group, celula_omitida=cell, delta_perfil_menos_referencia=delta))
    pd.DataFrame(cell_influence).to_csv(OUT / "influencia_celulas.csv", index=False)
    for name, records in [("metricas", overall), ("por_classe", by_class), ("por_cobertura", coverage), ("contrastes_2025", contrasts)]:
        pd.DataFrame(records).to_csv(OUT / (name + ".csv"), index=False)
    (OUT / "confusoes.json").write_text(json.dumps(confusions, indent=2))


if __name__ == "__main__":
    main()
