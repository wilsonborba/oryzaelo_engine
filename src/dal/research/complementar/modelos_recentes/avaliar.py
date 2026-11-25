"""Evaluates a single fixed architecture on the exact Thai six-stage folds."""
import argparse
import hashlib
import json
import os
from pathlib import Path

import numpy as np
import pandas as pd
from sklearn.preprocessing import StandardScaler

import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from paths import REPO as ENGINE, RESULTS, THAI as TH

HERE = RESULTS / "modelos_recentes"
HERE.mkdir(parents=True, exist_ok=True)
CLASSES = ["seedling", "tillering", "booting", "heading", "flowering", "preharvest"]
THAI = ["ระยะกล้า", "แตกกอ", "ตั้งท้อง", "ออกรวง", "ออกดอก", "ก่อนเก็บเกี่ยว"]
CATS = ["rice_ecosystem", "rice_variety", "province"]
SHA = "bdc7dbd5e4ff21f8f0456fcf90c6b7cdf72dbea960f2d05b19bec19f9b3d4ed0"
CHECKPOINT = Path.home()/".cache/oryzelo-models/4dcd344ece2c00be9e831fdd35bed57b5ad83e19/tabicl-classifier-v2-20260212.ckpt"


def tabm_predict(train, labels, test):
    import torch
    import torch.nn.functional as F
    from tabm import TabM

    torch.manual_seed(42)
    nums = train.columns.difference(CATS, sort=False)
    scaler = StandardScaler().fit(train[nums])
    def numeric(df):
        return torch.tensor(scaler.transform(df[nums]), device="cuda", dtype=torch.float32)
    mapping = {col: {v:i+1 for i,v in enumerate(sorted(train[col].unique()))} for col in CATS}
    def categories(df):
        codes = np.column_stack([df[c].map(mapping[c]).fillna(0).astype(int) for c in CATS])
        return torch.tensor(codes, device="cuda", dtype=torch.long)
    x, c, y = numeric(train), categories(train), torch.tensor(labels, device="cuda", dtype=torch.long)
    model = TabM.make(n_num_features=len(nums), cat_cardinalities=[len(m)+1 for m in mapping.values()],
                      d_out=6, k=16, n_blocks=2, d_block=128, dropout=.1).cuda()
    opt = torch.optim.AdamW(model.parameters(), lr=.001, weight_decay=.01)
    for _ in range(64):
        for batch in torch.randperm(len(y), device="cuda").split(128):
            logits = model(x[batch], c[batch])
            loss = F.cross_entropy(logits.flatten(0,1), y[batch].repeat_interleave(logits.shape[1]))
            opt.zero_grad()
            loss.backward()
            opt.step()
    model.eval()
    with torch.inference_mode():
        return model(numeric(test), categories(test)).softmax(-1).mean(1).cpu().numpy()


def predict(model, train, labels, test, seed):
    if model == "tabpfn35":
        from tabpfn import TabPFNClassifier
        checkpoint = Path.home()/".cache/oryzelo-models/06bf2ba35c80a92a3b9abb436b99cf49e7a0365e/tabpfn-v3.5-20260909_multiclass.safetensors"
        clf = TabPFNClassifier(model_path=checkpoint, device="cuda", n_estimators=4,
                               auto_scale_n_estimators=False, random_state=42,
                               categorical_features_indices=[train.columns.get_loc(c) for c in CATS])
        clf.fit(train, labels)
        return clf.predict_proba(test)
    if model == "tabm":
        return tabm_predict(train, labels, test)
    if model == "tabicl":
        from tabicl import TabICLClassifier
        clf = TabICLClassifier(device="cuda", n_estimators=8, batch_size=2, model_path=CHECKPOINT,
                               allow_auto_download=False, n_jobs=2, random_state=42)
        clf.fit(train, labels)
        return clf.predict_proba(test)
    import sys
    from xgboost import XGBClassifier
    sys.path.insert(0, str(ENGINE))
    from src.domain.services.revision_study import encode
    a, b, _ = encode(train, test)
    original_order = np.argsort(THAI)
    clf = XGBClassifier(n_estimators=350, learning_rate=.06, max_depth=6, tree_method="hist",
                        random_state=seed, eval_metric="mlogloss", n_jobs=4)
    clf.fit(a, np.argsort(original_order)[labels])
    return clf.predict_proba(b)[:, np.argsort(original_order)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("model", choices=["tabm", "tabicl", "tabpfn35", "xgb"])
    args = parser.parse_args()
    if args.model == "tabicl":
        with CHECKPOINT.open("rb") as stream:
            assert hashlib.sha256(stream.read()).hexdigest() == SHA
    frame = pd.read_csv(TH/"features.csv", parse_dates=["parsed_date"])
    features = json.loads((TH/"experiment_manifest.json").read_text())["features"]
    frame[CATS] = frame[CATS].fillna("Unknown").astype(str)
    y = frame.rice_stage.map(dict(zip(THAI, range(6)))).to_numpy()
    assert set(y)==set(range(6)) and len(features)==44 and frame.source_row_id.nunique()==len(frame)==2370
    splits=[]
    for name,file in [("principal","six_stage_folds.csv"),("provincia","province_folds.csv")]:
        fold = frame.source_row_id.map(pd.read_csv(TH/file).set_index("source_row_id").fold)
        assert set(fold)==set(range(1,6))
        for i in range(1,6):
            if name=="provincia": assert set(frame.province[fold==i]).isdisjoint(frame.province[fold!=i])
            splits.append((name,i,fold!=i,fold==i))
    splits.append(("2025",1,frame.parsed_date.dt.year.le(2024),frame.parsed_date.dt.year.eq(2025)))
    results=[]
    for name, fold, tr, va in splits:
        p = predict(args.model, frame.loc[tr,features], y[tr], frame.loc[va,features], 42+fold)
        assert p.shape==(va.sum(),6) and np.isfinite(p).all() and np.allclose(p.sum(1),1,atol=1e-4)
        rows = frame.loc[va,["source_row_id","parsed_date","lat_clean","lon_clean"]].copy()
        rows = rows.assign(protocol=name,fold=fold,y=y[va],pred=p.argmax(1))
        rows[CLASSES]=p
        results.append(rows)
        print(args.model,name,fold,"ok",flush=True)
    data=pd.concat(results,ignore_index=True)
    assert not data.duplicated(["protocol","source_row_id"]).any()
    data.to_csv(HERE/(args.model+"_predicoes.csv.gz"),index=False,compression={"method":"gzip","mtime":0})


if __name__=="__main__":
    main()
