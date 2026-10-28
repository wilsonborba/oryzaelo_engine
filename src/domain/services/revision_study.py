"""Reproduce the approved six-stage study without changing deployed artifacts.

Usage: python -m src.domain.services.revision_study --survey PATH --out PATH
Use --phase prepare/evaluate/scouting/export to resume a completed stage.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import platform
import subprocess
import sys
from pathlib import Path

import numpy as np
import pandas as pd
from catboost import CatBoostClassifier, Pool
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import classification_report, confusion_matrix, f1_score, recall_score
from sklearn.model_selection import GroupKFold, StratifiedKFold
from xgboost import XGBClassifier

from src.domain.services.baseline_ml import CLIMATE_FEATURES

REPO = Path(__file__).resolve().parents[3]
CACHE = REPO / 'src/dal/data/raw/nasa_power_cache'
PARAMS = ['T2M_MAX', 'T2M_MIN', 'T2M', 'PRECTOTCORR', 'ALLSKY_SFC_SW_DWN', 'RH2M']
CONTEXT = ['rice_ecosystem', 'rice_variety', 'province']
DERIVED = ['month', 'day_of_year', 'photoperiod_hours', 'ptq_30d', 'ptq_60d',
           'vpd_proxy_14d', 'vpd_proxy_30d', 'lat_clean', 'lon_clean']
FEATURES = CLIMATE_FEATURES + DERIVED + CONTEXT
NUMERIC_ORDER = CLIMATE_FEATURES + DERIVED + [c + '_code' for c in CONTEXT]
PHASES = {'ระยะกล้า': 'vegetative', 'แตกกอ': 'vegetative', 'ตั้งท้อง': 'reproductive',
          'ออกรวง': 'reproductive', 'ออกดอก': 'reproductive', 'ก่อนเก็บเกี่ยว': 'maturation'}
THREADS = 4


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, data):
    Path(path).write_text(json.dumps(data, ensure_ascii=False, indent=2, allow_nan=False) + '\n')


def save_manifest(out, **fields):
    path = out / 'experiment_manifest.json'
    data = json.loads(path.read_text()) if path.exists() else {}
    data.update(fields)
    write_json(path, data)


def encode(train, test, columns=CONTEXT):
    """Fit maps on training rows; every unseen test value becomes -1."""
    a, b = train.copy(), test.copy()
    maps = {}
    for col in columns:
        maps[col] = {v: i for i, v in enumerate(sorted(a[col].unique()))}
        a[col] = a[col].map(maps[col]).astype(np.float32)
        b[col] = b[col].map(maps[col]).fillna(-1).astype(np.float32)
    return a.astype(np.float32), b.astype(np.float32), maps


def sqrt_weights(y, k):
    counts = np.bincount(y, minlength=k)
    if (counts == 0).any():
        raise ValueError('A training fold lacks a required class; do not silently omit it.')
    return np.sqrt(counts.max() / counts).tolist()


def cat_model(y, k, fold=0, phase=False, numerical=False, spatial=False):
    params = dict(iterations=400 if phase else (350 if numerical or spatial else 450),
                  learning_rate=0.08 if phase else 0.07, depth=6, loss_function='MultiClass',
                  random_seed=42 + fold, verbose=False, thread_count=THREADS,
                  allow_writing_files=False)
    params.update({'auto_class_weights': 'Balanced'} if phase else {'class_weights': sqrt_weights(y, k)})
    return CatBoostClassifier(**params)


def prepare(survey, out):
    source = pd.read_csv(survey)
    if len(source) != 2370 or set(source.rice_stage) != set(PHASES):
        raise ValueError('Expected the approved 2,370-row six-stage sample.')
    if source.source_row_id.duplicated().any():
        raise ValueError('Duplicate source_row_id.')
    dates = source.survey_date.astype(str).map(
        lambda v: pd.Timestamp('1899-12-30') + pd.Timedelta(days=int(v))
        if v.isdigit() else pd.to_datetime(v, format='%Y-%m-%d'))
    source['parsed_date'] = dates
    source['lat_clean'] = pd.to_numeric(source.lat, errors='raise')
    source['lon_clean'] = pd.to_numeric(source.lon, errors='raise')
    cells, hashes, headers = {}, {}, {}
    records = []
    for i, row in source.iterrows():
        lat, lon = row.lat_clean, row.lon_clean
        key = (round(lat * 2) / 2, round(lon * 2) / 2)
        if key not in cells:
            path = CACHE / f'grid_{key[0]:.1f}_{key[1]:.1f}.json'
            raw = json.loads(path.read_text())  # Cache only. No network fallback.
            frame = pd.DataFrame(raw['properties']['parameter'])[PARAMS]
            frame.index = pd.to_datetime(frame.index, format='%Y%m%d')
            frame = frame.sort_index()
            if frame.isna().any().any() or (frame == -999).any().any():
                raise ValueError(f'Missing cached weather requires a documented rule: {path}')
            frame['GDD'] = np.maximum(0, (frame.T2M_MAX + frame.T2M_MIN) / 2 - 10)
            frame['DTR'] = frame.T2M_MAX - frame.T2M_MIN
            cells[key] = frame
            hashes[path.name] = sha(path)
            headers[path.name] = raw['header']
        frame = cells[key]
        features = {}
        for w in [7, 14, 30, 60]:
            dates_required = pd.date_range(row.parsed_date - pd.Timedelta(days=w-1), row.parsed_date)
            sub = frame.reindex(dates_required)
            if len(sub) != w or sub.isna().any().any():
                raise ValueError(f'Incomplete {w}-day window for source row {row.source_row_id}')
            vals = [sub.GDD.sum(), sub.PRECTOTCORR.sum(), sub.PRECTOTCORR.max(),
                    (sub.PRECTOTCORR < 1).sum(), sub.DTR.mean(), sub.DTR.std(ddof=1),
                    sub.ALLSKY_SFC_SW_DWN.sum(), sub.RH2M.mean()]
            names = ['gdd_cum', 'rain_cum', 'rain_max', 'cdd', 'dtr_mean', 'dtr_std', 'rad_cum', 'rh_mean']
            features.update({f'{n}_{w}d': round(float(v), 2) for n, v in zip(names, vals)})
        records.append(features)
    data = pd.concat([source.reset_index(drop=True), pd.DataFrame(records)], axis=1)
    data['month'], data['day_of_year'] = dates.dt.month.values, dates.dt.dayofyear.values
    delta = 0.409 * np.sin(2*np.pi*data.day_of_year/365.25 - 1.39)
    data['photoperiod_hours'] = 24/np.pi*np.arccos(np.clip(-np.tan(np.radians(data.lat_clean))*np.tan(delta),-1,1))
    for w in [30, 60]:
        data[f'ptq_{w}d'] = data[f'rad_cum_{w}d'] / (data[f'gdd_cum_{w}d'] + 1e-5)
    for w in [14, 30]:
        data[f'vpd_proxy_{w}d'] = data[f'dtr_mean_{w}d'] * (100-data[f'rh_mean_{w}d']) / 100
    for col in CONTEXT:
        data[col] = data[col].fillna('Unknown').astype(str)
    data.to_csv(out/'features.csv', index=False)
    write_json(out/'cache_manifest.json', {'hashes': hashes, 'headers': headers})
    versions = {p: importlib.metadata.version(p) for p in [
        'pandas','numpy','scikit-learn','catboost','xgboost','shap','onnx','onnxruntime']}
    save_manifest(out, survey=str(survey.resolve()), survey_sha256=sha(survey),
                  code_sha256=sha(__file__), git_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip(),
                  python=platform.python_version(), versions=versions, command=sys.argv,
                  rows=len(data), classes=sorted(PHASES), features=FEATURES, numeric_order=NUMERIC_ORDER,
                  source_cells_used=len(cells), source_cache_files=len(list(CACHE.glob('*.json'))),
                  missing_weather_values=0, complete_windows=len(data)*4, thread_count=THREADS,
                  feature_units={'gdd_cum':'degree C days','rain_cum':'mm','rain_max':'mm/day','rad_cum':'MJ/m2',
                                 'dtr_mean':'degree C','dtr_std':'degree C, ddof=1','rh_mean':'percent',
                                 'cdd':'count of days with rain <1 mm','photoperiod_hours':'hours',
                                 'ptq':'radiation sum / (GDD sum + 1e-5)','vpd_proxy':'empirical DTR/RH index, not kPa'},
                  features_sha256=sha(out/'features.csv'), phase_map=PHASES)
    print(f'PREPARED {len(data)} rows; {len(cells)} cached query points; complete windows.', flush=True)


def dataset(out):
    d = pd.read_csv(out/'features.csv')
    for c in CONTEXT:
        d[c] = d[c].fillna('Unknown').astype(str)
    return d


def equivalence_fixture(out):
    """Archive representative Python vectors and 61 raw days for Rust checks."""
    d = dataset(out)
    selection = set()
    for _, group in d.groupby('rice_stage'):
        selection.update(group.index[:5])
    selection.update(d.sort_values('parsed_date').index[::100])
    selection.update([d.lat_clean.idxmin(), d.lat_clean.idxmax()])
    records = []
    for i in sorted(selection):
        row = d.loc[i]
        date = pd.Timestamp(row.parsed_date)
        path = CACHE / f'grid_{round(row.lat_clean*2)/2:.1f}_{round(row.lon_clean*2)/2:.1f}.json'
        raw = json.loads(path.read_text())['properties']['parameter']
        weather = []
        for day in pd.date_range(date-pd.Timedelta(days=59), date+pd.Timedelta(days=1)):
            key = day.strftime('%Y%m%d')
            weather.append({'date':str(day.date()), 't_max':raw['T2M_MAX'][key],
                            't_min':raw['T2M_MIN'][key], 'precipitation_mm':raw['PRECTOTCORR'][key],
                            'radiation_mj_m2':raw['ALLSKY_SFC_SW_DWN'][key],
                            'relative_humidity_pct':raw['RH2M'][key]})
        records.append({'source_row_id':int(row.source_row_id), 'eval_date':str(date.date()),
                        'lat':float(row.lat_clean), 'lon':float(row.lon_clean), 'weather':weather,
                        'expected':[float(row[k]) for k in CLIMATE_FEATURES+DERIVED]+[0.,7.,-1.]})
    write_json(out/'feature_equivalence_fixture.json', records)
    print(f'Prepared {len(records)} x 44 cross-language fixtures.', flush=True)


def score(y, pred, classes):
    labels = np.arange(len(classes))
    return {'macro_f1':float(f1_score(y,pred,labels=labels,average='macro',zero_division=0)),
            'balanced_accuracy':float(recall_score(y,pred,labels=labels,average='macro',zero_division=0)),
            'weighted_f1':float(f1_score(y,pred,labels=labels,average='weighted',zero_division=0)),
            'confusion_matrix_row_normalized':confusion_matrix(y,pred,labels=labels,normalize='true').tolist(),
            'classification_report':classification_report(y,pred,labels=labels,target_names=classes,zero_division=0,output_dict=True)}


def folds(d, y, name, out, spatial=False):
    split = GroupKFold(5).split(d,y,groups=d.province) if spatial else StratifiedKFold(5,shuffle=True,random_state=42).split(d,y)
    result = np.full(len(d),-1,dtype=int)
    for i,(_,test) in enumerate(split,1):
        result[test] = i
    pd.DataFrame({'source_row_id':d.source_row_id,'fold':result}).to_csv(out/(name+'_folds.csv'),index=False)
    return result


def run_cv(d,y,classes,fold_ids,name,out,kind='cat',phase=False,spatial=False,scouting=False):
    k=len(classes);probs=np.zeros((len(d),k));results=[];params=[]
    columns=FEATURES.copy();catcols=CONTEXT.copy()
    if scouting:
        base_fields=['source_row_id','survey_date','province','lat','lon','rice_ecosystem','rice_stage','rice_variety']
        original=list(pd.read_csv(json.loads((out/'experiment_manifest.json').read_text())['survey'],nrows=0).columns)
        extra=[c for c in original if c not in base_fields]
        assert len(extra)==35
        catcols+=['dominant_weed_type','dominant_weed_species'];columns+=extra
        d=d.copy()
        for c in catcols:d[c]=d[c].fillna('Unknown').astype(str)
        issues = []
        for c in extra:
            if c not in catcols:
                raw = d[c].copy()
                converted = pd.to_numeric(raw, errors='coerce')
                for i in np.flatnonzero(raw.notna() & converted.isna()):
                    issues.append({'source_row_id': int(d.iloc[i].source_row_id),
                                   'column': c, 'raw_value': str(raw.iloc[i]),
                                   'model_handling': 'missing, not zero'})
                d[c] = converted
        pd.DataFrame(issues, columns=['source_row_id','column','raw_value','model_handling']).to_csv(
            out/'scouting_value_issues.csv', index=False)
        write_json(out/'scouting_missing_values.json', {
            'unparseable_numeric_values': len(issues),
            'numeric_missing_counts': {c:int(d[c].isna().sum()) for c in extra if c not in catcols},
            'handling': 'Unparseable numeric values become missing; raw source records are preserved.'})
    for fold in range(1,6):
        tr=np.flatnonzero(fold_ids!=fold);te=np.flatnonzero(fold_ids==fold)
        sqrt_weights(y[tr],k)
        a,b=d.iloc[tr][columns],d.iloc[te][columns];maps={}
        if kind in ['xgb','rf','numeric']:a,b,maps=encode(a,b)
        if kind=='xgb':
            model=XGBClassifier(n_estimators=350,learning_rate=.06,max_depth=6,tree_method='hist',
                                random_state=42+fold,eval_metric='mlogloss',n_jobs=THREADS)
        elif kind=='rf':
            model=RandomForestClassifier(n_estimators=200,max_depth=14,class_weight='balanced',random_state=42+fold,n_jobs=THREADS)
        else:model=cat_model(y[tr],k,fold,phase,kind=='numeric',spatial)
        if kind=='cat':
            model.fit(Pool(a,y[tr],cat_features=catcols));prediction=model.predict_proba(Pool(b,cat_features=catcols))
        else:model.fit(a,y[tr]);prediction=model.predict_proba(b)
        probs[np.ix_(te,np.asarray(model.classes_,dtype=int))]=prediction
        metrics=score(y[te],prediction.argmax(1),classes)
        results.append({'fold':fold,'train_n':len(tr),'test_n':len(te),'macro_f1':metrics['macro_f1'],
                        'train_class_support':np.bincount(y[tr],minlength=k).tolist(),
                        'test_class_support':np.bincount(y[te],minlength=k).tolist()})
        effective_params = {key: ('NaN' if isinstance(value, float) and np.isnan(value) else value)
                            for key, value in model.get_params().items()}
        params.append({'fold':fold,'parameters':effective_params,'encoding_maps':maps})
        print(f'{name} fold {fold}: Macro-F1 {metrics["macro_f1"]:.4f}',flush=True)
    pred=probs.argmax(1);report=score(y,pred,classes);report.update(classes=classes,folds=results)
    report['pooled_oof']=True;report['macro_f1_threshold']=.75;report['threshold_met']=report['macro_f1']>=.75
    output=pd.DataFrame({'source_row_id':d.source_row_id,'fold':fold_ids,'true':y,'predicted':pred})
    for i in range(k):output[f'probability_{i}']=probs[:,i]
    output.to_csv(out/(name+'_oof.csv'),index=False);write_json(out/(name+'_metrics.json'),report)
    write_json(out/(name+'_parameters.json'),params)
    return report


def evaluate(out):
    d=dataset(out);classes=sorted(d.rice_stage.unique());y=pd.Categorical(d.rice_stage,categories=classes).codes.astype(int)
    f=folds(d,y,'six_stage',out)
    for name,kind in [('catboost_six','cat'),('xgboost_six','xgb'),('random_forest_six','rf')]:
        run_cv(d,y,classes,f,name,out,kind)
    phase=d.rice_stage.map(PHASES);pc=sorted(phase.unique());py=pd.Categorical(phase,categories=pc).codes.astype(int)
    pf=folds(d,py,'three_phase',out);run_cv(d,py,pc,pf,'catboost_three_phase',out,phase=True)
    sf=folds(d,y,'province',out,spatial=True);run_cv(d,y,classes,sf,'catboost_province',out,spatial=True)
    # Descriptive TreeSHAP on the fitted native model. No out-of-sample causal claim.
    import shap
    model=cat_model(y,len(classes));pool=Pool(d[FEATURES],y,cat_features=CONTEXT);model.fit(pool)
    vals=np.asarray(shap.TreeExplainer(model).shap_values(pool))
    if vals.shape==(len(d),len(FEATURES),len(classes)):importance=np.abs(vals).mean(axis=(0,2))
    elif vals.shape==(len(classes),len(d),len(FEATURES)):importance=np.abs(vals).mean(axis=(0,1))
    else:raise ValueError(f'Unexpected SHAP shape {vals.shape}')
    pd.DataFrame({'feature':FEATURES,'mean_abs_shap':importance}).sort_values('mean_abs_shap',ascending=False).to_csv(out/'native_shap.csv',index=False)
    model.save_model(str(out/'native_six.cbm'))
    save_manifest(out,shap={'rows':len(d),'shape':list(vals.shape),'sample':'all retained training rows','interpretation':'descriptive fitted-model association'})


def scouting(out):
    d=dataset(out);classes=sorted(d.rice_stage.unique());y=pd.Categorical(d.rice_stage,categories=classes).codes.astype(int)
    f=pd.read_csv(out/'six_stage_folds.csv');assert f.source_row_id.tolist()==d.source_row_id.tolist()
    extended=run_cv(d,y,classes,f.fold.to_numpy(),'catboost_scouting',out,scouting=True)
    baseline=json.loads((out/'catboost_six_metrics.json').read_text())
    write_json(out/'scouting_comparison.json',{'primary':baseline['macro_f1'],'with_scouting':extended['macro_f1'],
        'difference':extended['macro_f1']-baseline['macro_f1'],
        'paired_fold_differences':[b['macro_f1']-a['macro_f1'] for a,b in zip(baseline['folds'],extended['folds'])],
        'interpretation':'same-survey scouting inputs; descriptive association; not climate-only deployment accuracy'})


def export(out):
    import onnxruntime as ort
    d=dataset(out);classes=sorted(d.rice_stage.unique());y=pd.Categorical(d.rice_stage,categories=classes).codes.astype(int)
    f=pd.read_csv(out/'six_stage_folds.csv');assert f.source_row_id.tolist()==d.source_row_id.tolist()
    run_cv(d,y,classes,f.fold.to_numpy(),'catboost_numeric',out,kind='numeric')
    numeric,_,maps=encode(d[FEATURES],d[FEATURES]);model=cat_model(y,len(classes),numerical=True);model.fit(numeric,y)
    path=out/'rice_stage_six.onnx';model.save_model(str(path),format='onnx');model.save_model(str(out/'numeric_six.cbm'))
    options=ort.SessionOptions();options.intra_op_num_threads=1
    session=ort.InferenceSession(str(path),sess_options=options,providers=['CPUExecutionProvider'])
    selected=np.concatenate([np.flatnonzero(y==c)[:5] for c in range(len(classes))]);sample=numeric.iloc[selected].to_numpy(np.float32)
    unknown=sample[:1].copy();unknown[0,-3:]=-1;sample=np.vstack([sample,unknown])
    outputs=session.run(None,{session.get_inputs()[0].name:sample})
    observed=np.array([[row[i] for i in range(len(classes))] for row in outputs[1]])
    expected=model.predict_proba(sample);diff=float(np.abs(expected-observed).max())
    if diff>1e-5 or not np.array_equal(expected.argmax(1),observed.argmax(1)):raise ValueError('ONNX conversion equivalence failed')
    metadata={'classes':classes,'phase_map':PHASES,'features_order':NUMERIC_ORDER,'categorical_encodings':maps,
              'unknown_category':-1,'input_dtype':'float32','input_shape':[1,44],
              'model_sha256':sha(path),'features_sha256':sha(out/'features.csv'),
              'evaluation':'catboost_numeric_metrics.json','runtime':ort.__version__,
              'conversion_check':{'rows':len(sample),'unknown_category_case':True,'tolerance':1e-5,'max_probability_difference':diff,'all_labels_agree':True}}
    write_json(out/'model_metadata.json',metadata)
    write_json(out/'benchmark_input.json',sample[0:1].tolist())
    write_json(out/'conversion_checks.json',metadata['conversion_check'])
    print('ONNX export passed:',metadata['conversion_check'],flush=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--survey',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True);parser.add_argument('--phase',choices=['all','prepare','fixture','evaluate','scouting','export'],default='all')
    a=parser.parse_args();a.out.mkdir(parents=True,exist_ok=True)
    for name,func in [('prepare',lambda:prepare(a.survey,a.out)),('fixture',lambda:equivalence_fixture(a.out)),('evaluate',lambda:evaluate(a.out)),
                      ('scouting',lambda:scouting(a.out)),('export',lambda:export(a.out))]:
        if a.phase in ['all',name]:func()


if __name__=='__main__':main()
