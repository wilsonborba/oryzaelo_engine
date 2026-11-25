"""Recalculate macro-F1 uncertainty, class errors and dataset shift from saved predictions."""
import json
import platform
from importlib.metadata import version
import numpy as np
import pandas as pd
from sklearn.metrics import confusion_matrix,classification_report
from benchmark import CLASSES,OUT,THDIR,SEED,cluster,dump

df=pd.read_csv(OUT/'analogue_forward_predictions.csv')
details=pd.read_csv(OUT/'forward_predictions.csv.gz')
for col in ['lat_clean','lon_clean','parsed_date','province']:df[col]=details[col]
assert np.array_equal(df.source_row_id,details.source_row_id)
groups=list(df.groupby(cluster(df)).indices.values())
draws=np.random.default_rng(SEED).multinomial(len(groups),np.ones(len(groups))/len(groups),size=2500)
scores={}
reps={}
for arm in ['TH','TH_ES','TH_ES_US_JP']:
    cms=np.array([confusion_matrix(df.iloc[ix].label,df.iloc[ix][arm],labels=CLASSES) for ix in groups])
    sample=(draws@cms.reshape(len(groups),-1)).reshape(-1,6,6)
    denom=sample.sum(axis=1)+sample.sum(axis=2)
    valid=(sample.sum(axis=2)>0).all(axis=1)
    reps[arm]=np.divide(2*np.diagonal(sample,axis1=1,axis2=2),denom,out=np.zeros_like(denom,dtype=float),where=denom>0).mean(axis=1)[valid]
    scores[arm]=dict(metrics=classification_report(df.label,df[arm],labels=CLASSES,output_dict=True,zero_division=0),matrix=cms.sum(axis=0).tolist(),ci=np.percentile(reps[arm],[2.5,97.5]).tolist(),delta_vs_thai_ci=np.percentile(reps[arm]-reps['TH'],[2.5,97.5]).tolist())
dump('hybrid_metrics.json',scores)
thai=pd.read_csv(THDIR/'features.csv')
dev=thai.loc[thai.parsed_date<'2025']
t=thai.loc[thai.parsed_date>='2025']
shift={k:int((~t[k].isin(dev[k])).sum()) for k in ('province','rice_variety','rice_ecosystem')}
dump('environment.json',dict(host=platform.node(),architecture=platform.machine(),python=platform.python_version(),versions={k:version(k) for k in ['pandas','numpy','scipy','scikit-learn','xgboost','catboost','onnxruntime','joblib','requests']},unseen_in_test=shift,development_groups=int(cluster(dev).nunique()),test_groups=len(groups)))
