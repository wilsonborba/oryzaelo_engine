"""Build and run the international CatBoost/analogue candidate from 44 ready features."""
import argparse
import csv
import json
from pathlib import Path
import time
import numpy as np
import onnxruntime as ort
import pandas as pd

from paths import RESULTS
OUT = RESULTS / 'results'
ARM='TH_ES_US_JP'
CLIMATE=['gdd_cum_30d','rain_cum_30d','rad_cum_30d','rh_mean_30d','dtr_mean_30d','gdd_cum_7d','rain_cum_7d']

def build():
    from benchmark import dataset,ARMS
    thai,ext=dataset()
    source=pd.concat([thai[thai.parsed_date.dt.year<=2024],ext[ext.country.isin(ARMS[ARM])]],ignore_index=True)
    schema=json.loads((OUT/(ARM+'__cat_schema.json')).read_text())
    idx={c:{v:i for i,v in enumerate(schema['categories'][c])} for c in schema['category_order']}
    counts=source.loc[source.country=='TH','y'].value_counts()
    weight=source.y.map(np.sqrt(counts.max()/counts)).to_numpy(copy=True)
    for k,n in source.loc[source.country!='TH','y'].value_counts().items():
        weight[(source.country!='TH')&(source.y==k)]*=min(1,.30*counts[k]/n)
    np.savez_compressed(OUT/'analogue_reference.npz',day=source.day_of_year,xy=source[['lat_clean','lon_clean']],climate=source[CLIMATE],iqr=(source[CLIMATE].quantile(.75)-source[CLIMATE].quantile(.25)).replace(0,1),cats=np.column_stack([source[c].map(idx[c]) for c in schema['category_order']]),weights=np.eye(6)[source.y]*weight[:,None])

class Candidate:
    def __init__(self,folder=OUT):
        self.schema=json.loads((folder/(ARM+'__cat_schema.json')).read_text())
        self.params=json.loads((folder/'analogue_selection.json').read_text())[ARM]
        self.codes={k:{v:i for i,v in enumerate(vs)} for k,vs in self.schema['categories'].items()}
        self.cats=self.schema['category_order']
        with np.load(folder/'analogue_reference.npz') as f:self.ref={k:f[k] for k in f}
        self.ref['climate']=self.ref['climate']/self.ref['iqr']
        opts=ort.SessionOptions();opts.intra_op_num_threads=1;opts.inter_op_num_threads=1
        self.session=ort.InferenceSession(str(folder/(ARM+'__cat.onnx')),opts,providers=['CPUExecutionProvider'])
        self.input=self.session.get_inputs()[0].name

    def predict(self,row):
        ref=self.ref
        days,space,alpha=self.params
        num=np.array([row[k] for k in self.schema['numeric']],dtype=np.float32)
        if not np.isfinite(num).all():raise ValueError('Expected 41 finite numeric field/weather features')
        onehot=[]
        day_diff=np.abs(ref['day']-row['day_of_year'])
        weights=1/(1+(np.minimum(day_diff,365.25-day_diff)/days)**2)
        weights/=1+np.square(ref['xy']-[row['lat_clean'],row['lon_clean']]).sum(axis=1)/space**2
        weather=np.array([row[k] for k in CLIMATE])/ref['iqr']
        weights/=1+np.abs(ref['climate']-weather).mean(axis=1)**2
        for j,c in enumerate(self.cats):
            val=row.get(c,'Unknown')
            code=self.codes[c].get(val,-1)
            block=np.zeros(len(self.codes[c]),np.float32)
            if code>=0:
                block[code]=1
                if val!='Unknown':
                    weights*=1+{'rice_variety':4,'rice_ecosystem':2,'province':2}[c]*(ref['cats'][:,j]==code)
            onehot.append(block)
        analog=weights@ref['weights'];analog/=analog.sum()
        x=np.concatenate([num,*onehot]).reshape(1,-1)
        vals=self.session.run(None,{self.input:x})[1][0]
        return alpha*analog+(1-alpha)*np.array([vals[i] for i in range(6)])

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('action',choices=['build','verify','predict','time']);p.add_argument('argument',nargs='?')
    args=p.parse_args()
    if args.action=='build':
        build();raise SystemExit
    model=Candidate()
    if args.action=='predict':
        for r in pd.read_csv(args.argument).to_dict('records'):
            probs=model.predict(r)
            print(json.dumps(dict(label=model.schema['classes'][probs.argmax()],probabilities=dict(zip(model.schema['classes'],probs.tolist())))))
        raise SystemExit
    from benchmark import THDIR
    data=pd.read_csv(THDIR/'features.csv')
    data=data[data.parsed_date>='2025'].reset_index(drop=True)
    records=data.to_dict('records')
    if args.action=='verify':
        p=np.array([model.predict(row) for row in records])
        ref=pd.read_csv(OUT/'analogue_forward_predictions.csv')
        assert np.array_equal(ref.source_row_id,data.source_row_id)
        assert np.array_equal(ref[ARM],np.array(model.schema['classes'])[p.argmax(axis=1)])
        pd.DataFrame(p,columns=model.schema['classes']).assign(source_row_id=data.source_row_id).to_csv(OUT/'analogue_probabilities.csv.gz',index=False)
        print('Verified',len(p),'predictions')
    else:
        for row in records[:50]:model.predict(row)
        times=[]
        for i in range(1000):
            row=records[(13*i)%len(records)]
            start=time.perf_counter_ns();model.predict(row)
            times.append((time.perf_counter_ns()-start)/1e6)
        pd.DataFrame({'elapsed_ms':times}).to_csv(OUT/'latency'/f'MSI_hybrid_{int(args.argument)}.csv',index=False)
        print(np.percentile(times,[50,95,99]))
