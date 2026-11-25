"""Secondary nonlinear analogue model using crop calendar, field context and NASA histories.

Use a product of Cauchy kernels. Smoothing scales and blend fraction are ranked
on out-of-location development predictions; the 2025 comparison is exploratory.
"""
import json
import joblib
import numpy as np
import pandas as pd
from scipy.spatial.distance import cdist
from benchmark import ROOT,OUT,CLASSES,FEATURES,CATS,ARMS,SEED,dataset,train,score,dump

CLIMATE=['gdd_cum_30d','rain_cum_30d','rad_cum_30d','rh_mean_30d','dtr_mean_30d','gdd_cum_7d','rain_cum_7d']

def analog(tr,query,days,space):
    diff=np.abs(query.day_of_year.to_numpy()[:,None]-tr.day_of_year.to_numpy())
    dist=np.minimum(diff,365.25-diff)
    loc=cdist(query[['lat_clean','lon_clean']],tr[['lat_clean','lon_clean']])
    weather=tr[CLIMATE].copy()
    iqr=(weather.quantile(.75)-weather.quantile(.25)).replace(0,1)
    weather_dist=cdist(query[CLIMATE]/iqr,weather/iqr,metric='cityblock')/len(CLIMATE)
    weights=1/(1+(dist/days)**2)/(1+(loc/space)**2)/(1+weather_dist**2)
    for cat,strength in [('rice_variety',4),('rice_ecosystem',2),('province',2)]:
        a=query[cat].fillna('Unknown').to_numpy()[:,None]
        b=tr[cat].fillna('Unknown').to_numpy()
        weights*=1+strength*((a==b)&(a!='Unknown'))
    local=tr.country=='TH'
    counts=tr.loc[local,'y'].value_counts()
    factors=tr.y.map(np.sqrt(counts.max()/counts)).to_numpy(copy=True)
    for c,n in tr.loc[~local,'y'].value_counts().items():
        factors[(~local)&(tr.y==c)]*=min(1,.30*counts[c]/n)
    scores=(weights*factors)@np.eye(6)[tr.y]
    return scores/scores.sum(axis=1,keepdims=True)

if __name__=='__main__':
    thai,ext=dataset()
    dev=thai[thai.parsed_date.dt.year<=2024].copy().reset_index(drop=True)
    test=thai[thai.parsed_date.dt.year==2025].copy().reset_index(drop=True)
    fold=dev[['source_row_id']].merge(pd.read_csv(OUT/'development_folds.csv'),on='source_row_id',validate='1:1').fold.to_numpy()
    trials=[]; winners={}
    for arm in ['TH','TH_ES','TH_ES_US_JP']:
        outside=ext[ext.country.isin(ARMS[arm])]
        pred={}
        for k in range(3):
            tr,va=dev[fold!=k],dev[fold==k]
            base=train(tr,ext,arm,'cat').predict_proba(va[FEATURES])
            source=pd.concat([tr,outside],ignore_index=True)
            for days,space in [(10,.45),(22,1.2),(40,2.5)]:
                knn=analog(source,va,days,space)
                for alpha in [.5,1.0]:
                    key=(days,space,alpha)
                    if key not in pred:pred[key]=np.zeros((len(dev),6))
                    pred[key][fold==k]=alpha*knn+(1-alpha)*base
        for key,p in pred.items():
            trials.append(dict(arm=arm,days=key[0],space=key[1],analog_fraction=key[2],macro_f1=score(dev.y,p.argmax(axis=1))))
        winners[arm]=max(pred,key=lambda x:score(dev.y,pred[x].argmax(axis=1)))
    pd.DataFrame(trials).to_csv(OUT/'analogue_development.csv',index=False)
    dump('analogue_selection.json',winners)
    out=test[['source_row_id','label']].copy()
    results={}
    for arm,(days,space,alpha) in winners.items():
        base=train(dev,ext,arm,'cat').predict_proba(test[FEATURES])
        source=pd.concat([dev,ext[ext.country.isin(ARMS[arm])]],ignore_index=True)
        probs=alpha*analog(source,test,days,space)+(1-alpha)*base
        out[arm]=np.array(CLASSES)[probs.argmax(axis=1)]
        results[arm]=dict(macro_f1=score(test.y,probs.argmax(axis=1)),cv_macro_f1=max(r['macro_f1'] for r in trials if r['arm']==arm),days=days,space=space,analog_fraction=alpha)
    out.to_csv(OUT/'analogue_forward_predictions.csv',index=False)
    dump('analogue_forward.json',results)
