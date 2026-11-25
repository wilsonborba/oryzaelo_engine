"""Compare fixed training arms on Thailand with site-season grouping and a forward test."""
import hashlib
import json
import os
from pathlib import Path
import time
import joblib
import numpy as np
import pandas as pd
from catboost import CatBoostClassifier
from sklearn.compose import ColumnTransformer
from sklearn.ensemble import ExtraTreesClassifier
from sklearn.metrics import f1_score, classification_report, confusion_matrix, accuracy_score
from sklearn.model_selection import StratifiedGroupKFold
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import OneHotEncoder
from xgboost import XGBClassifier

from paths import REPO, SOURCES, RESULTS, THAI
ROOT = Path(__file__).resolve().parents[1]
OUT = RESULTS / 'results'
OUT.mkdir(parents=True, exist_ok=True)
ENGINE=REPO
THDIR=THAI
CLASSES=['seedling','tillering','booting','heading','flowering','preharvest']
LABEL=dict(zip(['ระยะกล้า','แตกกอ','ตั้งท้อง','ออกรวง','ออกดอก','ก่อนเก็บเกี่ยว'],CLASSES))
CATS=['rice_variety','province','rice_ecosystem']
FEATURES=json.loads((THDIR/'experiment_manifest.json').read_text())['features']
ARMS={'TH': [],'TH_ES':['ES'],'TH_US_JP':['US','JP'],'TH_ES_US_JP':['ES','US','JP']}
MODELS=['xgb','extra','cat']
SEED=67

def dump(name,obj):
    (OUT/name).write_text(json.dumps(obj,indent=2,ensure_ascii=False,allow_nan=False))

def dataset():
    thai=pd.read_csv(THDIR/'features.csv',parse_dates=['parsed_date'])
    thai['label']=thai.rice_stage.map(LABEL)
    assert thai.label.notna().all()
    thai['country']='TH'
    data=pd.read_csv(SOURCES/'international_features.csv.gz',parse_dates=['parsed_date'])
    assert data.parsed_date.max()<pd.Timestamp('2025-01-01')
    for frame in (data,thai):
        frame[CATS]=frame[CATS].fillna('Unknown').astype(str)
        frame['y']=frame.label.map(dict(zip(CLASSES,range(6))))
        assert frame[FEATURES].notna().all().all() and frame.y.notna().all()
    # Each cell/date/stage/variety contributes at most one external row.
    clean=data.sort_values(['country','site','parsed_date']).drop_duplicates(['country','query_lat','query_lon','parsed_date','label','rice_variety'])
    groups=[g.sample(min(320,len(g)),random_state=SEED) for _,g in clean.groupby(['country','label'])]
    ext=pd.concat(groups,ignore_index=True)
    return thai,ext

def train(frame,ext,arm,kind,features=FEATURES):
    outside=ext.loc[ext.country.isin(ARMS[arm])]
    data=pd.concat([frame,outside],ignore_index=True)
    thai_counts=frame.y.value_counts()
    weights=(np.sqrt(thai_counts.max()/thai_counts)).to_dict()
    w=data.y.map(weights).to_numpy(copy=True)
    for cls,n in outside.y.value_counts().items():
        mask=(data.country!='TH')&(data.y==cls)
        w[mask]*=min(1,0.30*thai_counts[cls]/n)
    prep=ColumnTransformer([('num','passthrough',[k for k in features if k not in CATS]),('cat',OneHotEncoder(handle_unknown='ignore',sparse_output=False,dtype=np.float32),CATS)],sparse_threshold=0)
    if kind=='xgb':
        clf=XGBClassifier(n_estimators=260,max_depth=5,learning_rate=.065,n_jobs=4,tree_method='hist',random_state=SEED)
    elif kind=='extra':
        clf=ExtraTreesClassifier(n_estimators=280,max_features=.8,min_samples_leaf=2,n_jobs=4,random_state=SEED)
    else:
        clf=CatBoostClassifier(iterations=300,depth=6,learning_rate=.075,thread_count=4,verbose=False,random_seed=SEED,allow_writing_files=False)
    pipe=Pipeline([('prepare',prep),('model',clf)])
    return pipe.fit(data[features],data.y,model__sample_weight=w)

def score(y,p):
    return float(f1_score(y,p,labels=range(6),average='macro',zero_division=0))

def cluster(df):
    return (df.lat_clean*10).round().astype(int).astype(str)+'|'+(df.lon_clean*10).round().astype(int).astype(str)

def quantiles(v):
    return np.quantile(v,[.025,.975]).tolist()

if __name__=='__main__':
    thai,ext=dataset()
    ext.to_csv(OUT/'training_external.csv.gz',index=False,compression={'method':'gzip','mtime':0})
    dev=thai.loc[thai.parsed_date.dt.year<=2024].reset_index(drop=True)
    test=thai.loc[thai.parsed_date.dt.year==2025].reset_index(drop=True)
    # Keep repeated visits to nearby sites together irrespective of calendar year.
    groups=cluster(dev)
    folds=list(StratifiedGroupKFold(n_splits=3,shuffle=True,random_state=SEED).split(dev,dev.y,groups))
    indices=dev[['source_row_id','parsed_date','province']].copy()
    indices['group']=groups
    for fold,(tr,va) in enumerate(folds):
        assert not set(groups.iloc[tr])&set(groups.iloc[va])
        assert dev.iloc[tr].y.nunique()==6
        indices.loc[va,'fold']=fold
    indices.to_csv(OUT/'development_folds.csv',index=False)
    cv=[]
    for arm in ARMS:
        for kind in MODELS:
            pred=np.full(len(dev),-1)
            for tr,va in folds:
                pipe=train(dev.iloc[tr],ext,arm,kind)
                pred[va]=pipe.predict(dev.iloc[va][FEATURES]).reshape(-1)
            cv.append(dict(arm=arm,model=kind,cv_macro_f1=score(dev.y,pred)))
            print(cv[-1],flush=True)
    ranking=pd.DataFrame(cv)
    ranking.to_csv(OUT/'development_scores.csv',index=False)
    primary=ranking.loc[ranking.arm=='TH_ES_US_JP'].sort_values('cv_macro_f1',ascending=False).iloc[0].model
    best=ranking.loc[ranking.arm=='TH'].sort_values('cv_macro_f1',ascending=False).iloc[0].model
    dump('selection.json',dict(primary_international=primary,best_thai=best,selected_by='pooled out-of-fold six-class macro-F1 before evaluating 2025',target=.75))
    pred=test[['source_row_id','parsed_date','province','lat_clean','lon_clean','label']].copy()
    results={}
    runs=[(arm,primary) for arm in ARMS]
    if primary!=best:runs.append(('TH',best))
    truth=test.y.to_numpy()
    for arm,kind in runs:
        name=arm+'__'+kind
        pipe=train(dev,ext,arm,kind)
        probs=pipe.predict_proba(test[FEATURES])
        labels=probs.argmax(axis=1)
        pred[name]=labels
        for i,cls in enumerate(CLASSES):
            pred[name+'_p_'+cls]=probs[:,i]
        results[name]=dict(macro_f1=score(truth,labels),accuracy=accuracy_score(truth,labels),
            metrics=classification_report(truth,labels,labels=range(6),target_names=CLASSES,output_dict=True,zero_division=0),
            confusion=confusion_matrix(truth,labels,labels=range(6)).tolist())
    common='TH__'+primary
    indices=list(test.groupby(cluster(test)).indices.values())
    # Paired sampling of Thai sites; compute metrics from cluster-level confusion matrices.
    rng=np.random.default_rng(SEED)
    draws=rng.multinomial(len(indices),np.ones(len(indices))/len(indices),size=2500)
    replicas={}
    for name in results:
        cms=np.stack([confusion_matrix(truth[ix],pred[name].to_numpy()[ix],labels=range(6)) for ix in indices])
        cm=(draws@cms.reshape(len(indices),-1)).reshape(-1,6,6)
        den=cm.sum(axis=1)+cm.sum(axis=2)
        valid=(cm.sum(axis=2)>0).all(axis=1)
        boot=np.divide(2*np.diagonal(cm,axis1=1,axis2=2),den,out=np.zeros_like(den,dtype=float),where=den>0).mean(axis=1)[valid]
        replicas[name]=boot
        results[name]['site_ci95']=quantiles(boot)
        results[name]['delta_vs_matched_thai_ci95']=quantiles(boot-replicas[common])
        results[name]['delta_vs_matched_thai']=results[name]['macro_f1']-results[common]['macro_f1']
        results[name]['clusters']=len(indices)
        results[name]['valid_bootstrap_draws']=len(boot)
    pred.to_csv(OUT/'forward_predictions.csv.gz',index=False)
    dump('forward_scores.json',results)
    print([(n,r['macro_f1']) for n,r in results.items()],flush=True)
    rows=[]
    for prov,g in pred.groupby('province'):
        for name in results:
            y=g.label.map(dict(zip(CLASSES,range(6))))
            rows.append([prov,len(g),y.nunique(),name,accuracy_score(y,g[name]),score(y,g[name])])
    pd.DataFrame(rows,columns=['province','n','observed_classes','run','accuracy','macro_f1_fixed_six']).to_csv(OUT/'forward_provinces.csv',index=False)
