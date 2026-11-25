"""Checks representational ambiguity and retrospectively compares two 2025 candidates."""
import json
import numpy as np
import pandas as pd
from avaliar import TH, HERE
from resumir import metrics

def main():
    frame = pd.read_csv(TH/"features.csv")
    cols=json.loads((TH/"experiment_manifest.json").read_text())["features"]
    frame["pattern"]=frame.groupby(cols,dropna=False,sort=False).ngroup()
    counts=frame.groupby("pattern").rice_stage.agg(["size","nunique"])
    folds=pd.read_csv(TH/"six_stage_folds.csv").set_index("source_row_id").fold
    seen=frame.groupby("pattern").source_row_id.transform(lambda s: folds.loc[s].nunique())>1
    result={"n":len(frame),"distinct_44_input_vectors":len(counts),"rows_in_repeated_vectors":int(counts.loc[counts["size"]>1,"size"].sum()),
            "rows_in_conflicting_vectors":int(counts.loc[counts["nunique"]>1,"size"].sum()),
            "conflicting_vectors":int((counts["nunique"]>1).sum()),"rows_with_exact_vector_in_other_random_fold":int(seen.sum())}
    new=pd.read_csv(HERE/"tabpfn35_predicoes.csv.gz",dtype={"protocol":str}).query("protocol=='2025'").set_index("source_row_id")
    old=pd.read_csv(HERE.parent/"biologia/predicoes.csv.gz",dtype={"split":str})
    old=old.query("group=='TH' and arm=='perfil_foliar' and split=='2025'").set_index("source_row_id").loc[new.index]
    assert len(new)==856 and np.array_equal(old.y,new.y)
    cells=pd.factorize(old.cell)[0]
    n=cells.max()+1;draws=np.random.default_rng(67).multinomial(n,np.full(n,1/n),size=5000)
    samples=[]
    point=[]
    for pred in [old.pred,new.pred]:
        cm=np.zeros((n,6,6),dtype=int);np.add.at(cm,(cells,new.y,pred),1)
        boot=(draws@cm.reshape(n,-1)).reshape(-1,6,6);valid=(boot.sum(axis=-1)>0).all(axis=1)
        samples.append(metrics(boot[valid])[0].mean(axis=1))
        point.append(metrics(cm.sum(axis=0))[0].mean())
    result.update(bio_thai_2025=float(point[0]),tabpfn35_2025=float(point[1]),delta=float(point[1]-point[0]),
                  ci95=np.quantile(samples[1]-samples[0],[.025,.975]).tolist(),cells=int(n))
    correct=new[(new.y==5)&(new.pred==5)]
    result["pfn_correct_preharvest_cells"]=int(old.loc[correct.index].cell.nunique())
    result["pfn_correct_preharvest_dates"]=int(correct.parsed_date.nunique())
    (HERE/"diagnostico.json").write_text(json.dumps(result,indent=2))
    print(result)

if __name__=="__main__":
    main()
