"""Check the historical controls and compute paired uncertainty from saved predictions."""
import sys

import numpy as np
import pandas as pd
from sklearn.metrics import confusion_matrix

from avaliar import CLASSES, HERE, TH, THAI


def metrics(matrix):
    hit = np.diagonal(matrix, axis1=-2, axis2=-1)
    support = matrix.sum(axis=-1)
    denom = support + matrix.sum(axis=-2)
    f1 = np.divide(2*hit, denom, out=np.zeros_like(hit, dtype=float), where=denom>0)
    recall = np.divide(hit, support, out=np.zeros_like(hit, dtype=float), where=support>0)
    return f1, recall


def main():
    control = pd.read_csv(HERE/"xgb_predicoes.csv.gz", dtype={"protocol":str})
    archive = pd.read_csv(TH/"xgboost_six_oof.csv").set_index("source_row_id")
    original = control[control.protocol=="principal"]
    assert np.array_equal(np.argsort(THAI)[archive.loc[original.source_row_id,"predicted"]], original.pred)
    forward = pd.read_csv(HERE.parent/"results/archived_spec_forward.csv").set_index("source_row_id")
    year = control[control.protocol=="2025"]
    assert np.array_equal(forward.loc[year.source_row_id,"pred"], np.asarray(CLASSES)[year.pred])
    tables, detail = [], []
    for name in ["xgb", *(sys.argv[1:] or ["tabicl", "tabm", "tabpfn35"])]:
        data = pd.read_csv(HERE/(name+"_predicoes.csv.gz"), dtype={"protocol":str})
        for protocol, rows in data.groupby("protocol"):
            base = control[control.protocol==protocol].set_index("source_row_id").sort_index()
            rows = rows.set_index("source_row_id").loc[base.index]
            assert (rows.y==base.y).all() and (rows.fold==base.fold).all()
            sites = rows[["lat_clean","lon_clean"]].mul(10).round().astype(int)
            cells = pd.factorize(pd.MultiIndex.from_frame(sites))[0]
            size = cells.max()+1
            draws = np.random.default_rng(67).multinomial(size, np.full(size,1/size), size=5000)
            samples = []
            for pred in [base.pred,rows.pred]:
                cm = np.zeros((size,6,6),dtype=int)
                np.add.at(cm,(cells,rows.y,pred),1)
                boot = (draws@cm.reshape(size,-1)).reshape(-1,6,6)
                valid = (boot.sum(axis=-1)>0).all(axis=-1)
                samples.append(metrics(boot[valid])[0].mean(axis=1))
            cm = confusion_matrix(rows.y,rows.pred,labels=range(6))
            f1,recall = metrics(cm)
            score=f1.mean()
            baseline=metrics(confusion_matrix(rows.y,base.pred,labels=range(6)))[0].mean()
            lo,hi = np.quantile(samples[1]-samples[0],[.025,.975])
            tables.append(dict(model=name,protocol=protocol,n=len(rows),macro_f1=score,balanced_accuracy=recall.mean(),
                               delta_vs_xgb=score-baseline,ci_lo=lo,ci_hi=hi,bootstrap_valid=int(valid.sum())))
            for i,c in enumerate(CLASSES):
                detail.append(dict(model=name,protocol=protocol,stage=c,support=cm[i].sum(),correct=cm[i,i],
                                   predicted=cm[:,i].sum(),f1=f1[i],recall=recall[i]))
    table=pd.DataFrame(tables)
    table.to_csv(HERE/"metricas.csv",index=False)
    pd.DataFrame(detail).to_csv(HERE/"por_estadio.csv",index=False)
    print(table.to_string(index=False),flush=True)


if __name__=="__main__":
    main()
