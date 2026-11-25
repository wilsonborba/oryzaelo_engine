"""Validate and join daily NASA POWER AG observations to dated field records."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import os
import time
import numpy as np
import pandas as pd
import requests

from paths import REPO, SOURCES, RESULTS, THAI
ROOT = Path(__file__).resolve().parents[1]
RAW = SOURCES
CACHE=RAW/'power'
CACHE.mkdir(exist_ok=True)
ENGINE=REPO
PARAMS=['T2M_MAX','T2M_MIN','T2M','PRECTOTCORR','ALLSKY_SFC_SW_DWN','RH2M']
UNITS=['C','C','C','mm/day','MJ/m^2/day','%']
NAMES=['gdd_cum','rain_cum','rain_max','cdd','dtr_mean','dtr_std','rad_cum','rh_mean']

def daily(raw):
    assert [raw['parameters'][k]['units'] for k in PARAMS]==UNITS
    assert raw['header']['time_standard']=='LST'
    df=pd.DataFrame(raw['properties']['parameter'])[PARAMS].astype(float)
    df.index=pd.to_datetime(df.index,format='%Y%m%d')
    df=df.sort_index().asfreq('D').replace(raw['header']['fill_value'],np.nan)
    missing=df.isna().any(axis=1)
    dtr=df.T2M_MAX-df.T2M_MIN
    series=[((df.T2M_MAX+df.T2M_MIN)/2-10).clip(lower=0),df.PRECTOTCORR,df.PRECTOTCORR,(df.PRECTOTCORR<1).astype(float),dtr,dtr,df.ALLSKY_SFC_SW_DWN,df.RH2M]
    values={}
    for w in (7,14,30,60):
        for name, s, agg in zip(NAMES,series,['sum','sum','max','sum','mean','std','sum','mean']):
            clean=s.mask(missing)
            rolled=clean.rolling(w).agg(agg)
            rounded=rolled.round(2)
            # Match the original per-window Python rounding at half-cent ties.
            ties=np.flatnonzero(np.isclose((rolled.to_numpy()*100)%1,.5,rtol=0,atol=1e-8))
            for pos in ties:
                rounded.iloc[pos]=round(float(getattr(clean.iloc[pos-w+1:pos+1],agg)()),2)
            values[f'{name}_{w}d']=rounded
    return pd.DataFrame(values)

def cell_task(item):
    (lat,lon),rows=item
    start=(rows.parsed_date.min()-pd.Timedelta(days=59)).strftime('%Y%m%d')
    end=rows.parsed_date.max().strftime('%Y%m%d')
    file=CACHE/f'{lat}_{lon}_{start}_{end}.json'
    params=dict(parameters=','.join(PARAMS),community='AG',longitude=lon,latitude=lat,start=start,end=end,format='JSON')
    url='https://power.larc.nasa.gov/api/temporal/daily/point'
    if not file.exists():
        for attempt in range(3):
            try:
                r=requests.get(url,params=params,timeout=(10,90));r.raise_for_status()
                raw=r.json()
                assert raw['geometry']['coordinates'][:2]==[lon,lat]
                daily(raw)
                file.write_bytes(r.content)
                break
            except (requests.RequestException,ValueError):
                if attempt==2:raise
                time.sleep(2**attempt)
    raw=json.loads(file.read_bytes())
    assert raw['geometry']['coordinates'][:2]==[lon,lat]
    vals=daily(raw)
    frame=rows.join(vals,on='parsed_date')
    accepted=frame.dropna(subset=list(vals))
    return accepted,dict(file=file.name,sha256=hashlib.sha256(file.read_bytes()).hexdigest(),url=requests.Request('GET',url,params=params).prepare().url,rows=len(rows),complete=len(accepted))

def calendar(data):
    data['month']=data.parsed_date.dt.month
    data['day_of_year']=data.parsed_date.dt.dayofyear
    delta=.409*np.sin(2*np.pi*data.day_of_year/365.25-1.39)
    data['photoperiod_hours']=24/np.pi*np.arccos(np.clip(-np.tan(np.radians(data.lat_clean))*np.tan(delta),-1,1))
    for w in (30,60):
        data[f'ptq_{w}d']=data[f'rad_cum_{w}d']/(data[f'gdd_cum_{w}d']+1e-5)
    for w in (14,30):
        data[f'vpd_proxy_{w}d']=data[f'dtr_mean_{w}d']*(100-data[f'rh_mean_{w}d'])/100
    return data

if __name__=='__main__':
    (RESULTS / 'results').mkdir(parents=True, exist_ok=True)
    data=pd.concat([pd.read_csv(RAW/f,parse_dates=['parsed_date']) for f in ('spain.csv','trials.csv')],ignore_index=True)
    data['query_lat']=(data.lat_clean*2).round()/2
    data['query_lon']=(data.lon_clean*2).round()/2
    with ThreadPoolExecutor(3) as pool:
        results=list(pool.map(cell_task,data.groupby(['query_lat','query_lon'])))
    final=calendar(pd.concat([r[0] for r in results],ignore_index=True))
    final.to_csv(RAW/'international_features.csv.gz',index=False)
    (RESULTS/'results/weather_manifest.json').write_text(json.dumps([r[1] for r in results],indent=2))
    # Check all 39 weather/calendar features in a stratified sample of the original Thai cache.
    thai=pd.read_csv(THAI/'features.csv',parse_dates=['parsed_date'])
    errors=[]
    for row in thai.sample(40,random_state=41).itertuples():
        lat,lon=[round(x*2)/2 for x in (row.lat_clean,row.lon_clean)]
        fixture=ENGINE/f'src/dal/data/raw/nasa_power_cache/grid_{lat:.1f}_{lon:.1f}.json'
        vals=daily(json.loads(fixture.read_bytes())).loc[[row.parsed_date]].copy()
        vals['parsed_date']=vals.index
        vals['lat_clean']=row.lat_clean
        calculated=calendar(vals)
        errors.extend(abs(calculated[k].iloc[0]-getattr(row,k)) for k in calculated if k not in ('lat_clean','parsed_date'))
    # Match the Thai reference to numerical precision.
    print(final.groupby(['country','label']).size(), 'maximum Thai fixture deviation',max(errors))
    assert max(errors)<1e-8
