"""Secondary comparisons with a frozen model family; these do not select the candidate."""
import joblib
import numpy as np
import pandas as pd
from sklearn.metrics import recall_score
from paths import OBS
from benchmark import CATS, ARMS, CLASSES, THDIR, FEATURES, ROOT, OUT, dataset, train, score, dump
import json

thai,ext=dataset()
primary=json.loads((OUT/'selection.json').read_text())['primary_international']
rows=[]
predictions=[]
for protocol,file in [('random','six_stage_folds.csv'),('province','province_folds.csv')]:
    folds=thai[['source_row_id']].merge(pd.read_csv(THDIR/file),on='source_row_id',validate='1:1').fold
    for arm in ('TH','TH_ES_US_JP'):
        pred=np.full(len(thai),-1)
        for fold in sorted(folds.unique()):
            tr=folds!=fold
            if protocol=='province': assert set(thai.loc[tr,'province']).isdisjoint(thai.loc[~tr,'province'])
            pipe=train(thai.loc[tr],ext,arm,primary)
            pred[~tr]=pipe.predict(thai.loc[~tr,FEATURES]).reshape(-1)
        rows.append(dict(protocol=protocol,arm=arm,model=primary,n=len(pred),macro_f1=score(thai.y,pred)))
        predictions.append(pd.DataFrame({'protocol':protocol,'arm':arm,'source_row_id':thai.source_row_id,'fold':folds,'label':thai.label,'pred':np.array(CLASSES)[pred]}))
pd.concat(predictions).to_csv(OUT/'sensitivity_predictions.csv.gz',index=False)
pd.DataFrame(rows).to_csv(OUT/'sensitivity_scores.csv',index=False)

# Hold out the final campaign for each external country. This is retrospective transfer.
raw=pd.read_csv(OBS/'international_features.csv.gz',parse_dates=['parsed_date'])
rows=[]
for country in ('ES','US','JP'):
    last=raw.loc[raw.country==country,'parsed_date'].dt.year.max()
    future=raw.loc[(raw.country==country)&(raw.parsed_date.dt.year==last)].copy()
    future[CATS]=future[CATS].fillna('Unknown')
    # Thailand's 2023 sample lacks preharvest. Fix six-class training by instead holding
    # out external parcel-years only, keeping the full Thai development period till 2024.
    older=thai.loc[thai.parsed_date.dt.year<=2024]
    foreign=ext.loc[(ext.country!=country)|(ext.parsed_date.dt.year<last)]
    for arm in ('TH','TH_ES_US_JP'):
        pipe=train(older,foreign,arm,primary)
        pred=pipe.predict(future[FEATURES]).reshape(-1)
        for k,cls in enumerate(CLASSES):
            mask=future.label==cls
            if mask.any():
                rows.append(dict(country=country,year=int(last),arm=arm,stage=cls,n=int(mask.sum()),recall=float((pred[mask]==k).mean())))
pd.DataFrame(rows).to_csv(OUT/'external_last_campaign.csv',index=False)
