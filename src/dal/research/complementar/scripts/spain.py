"""Extract RAIF visits with one recorded stage, parcel-year coordinates and date."""
from collections import Counter, defaultdict
from functools import lru_cache
import csv
import json
from pathlib import Path
import re
import subprocess
from zipfile import ZipFile
from lxml import etree

from paths import RESULTS, SOURCES
ARCHIVE = SOURCES / 'RAIF_Arroz_Andalucia_2006_2026.zip'
STAGES = {2:'seedling', 3:'seedling', 4:'tillering', 5:'tillering', 8:'booting', 9:'heading', 10:'flowering'}
counts = Counter()

@lru_cache(None)
def decode(key):
    return re.sub('_x([0-9A-Fa-f]{4})_', lambda m: chr(int(m[1], 16)), key)

def records(archive, kind, tag):
    for name in archive.namelist():
        if kind not in name:
            continue
        with archive.open(name) as stream:
            for _, elem in etree.iterparse(stream, events=['end'], tag=tag):
                yield {decode(child.tag): child.text or '' for child in elem}
                elem.clear()
                while elem.getprevious() is not None:
                    del elem.getparent()[0]

def parcel(row):
    return tuple(row[k] for k in ('PROVINCIA','MUNICIPIO','CODPARCELA'))

def project(points, source):
    text = ''.join(f'{x} {y}\n' for x,y in points)
    result = subprocess.run(['cs2cs','-f','%.7f',source,'OGC:CRS84'],input=text,text=True,capture_output=True,check=True)
    return [tuple(map(float,line.split()[:2])) for line in result.stdout.splitlines()]

with ZipFile(ARCHIVE) as archive:
    sites = defaultdict(set)
    for row in records(archive,'Parcelas','AAA_01_DATOS_DE_PARCELAS_V1_conCodParc'):
        key = (*parcel(row),row['CAMPAÑA'])
        if not ('2015' <= key[-1] <= '2024' and key[0] in ['Sevilla','Cádiz']):
            continue
        try:
            xy = tuple(float(next((v for k,v in row.items() if 'Coordenada '+a in k),'').replace(',','.')) for a in 'XY')
        except (StopIteration,ValueError):
            continue
        if 100000 < xy[0] < 400000 and 3900000 < xy[1] < 4300000:
            sites[key].add((xy,row.get('301 Variedad','Unknown')))
    coords = sorted({xy for values in sites.values() for xy,_ in values})
    positions = {}
    for xy, p, q in zip(coords,project(coords,'EPSG:25830'),project(coords,'EPSG:23030')):
        if -6.6 <= p[0] <= -5 and 36.0 <= p[1] <= 37.7 and tuple(round(i*2) for i in p)==tuple(round(i*2) for i in q):
            positions[xy] = p
    visits = defaultdict(set)
    for row in records(archive,'Muestreos','AAA_02_DATOS_x0020_DE_x0020_MUESTREOS_V1_conCodParc'):
        date=row['FECHA'][:10]
        key=(*parcel(row),date[:4])
        if not ('2015' <= date[:4] <= '2024' and key[0] in ['Sevilla','Cádiz']):
            continue
        counts['in_scope_rows']+=1
        if len(sites.get(key,()))!=1:
            counts['missing_or_ambiguous_site']+=1
            continue
        xy,variety=next(iter(sites[key]))
        if xy not in positions:
            counts['coordinate_qc']+=1
            continue
        nonzero=[(int(k[:4])-100,v) for k,v in row.items() if re.match(r'^01(?:0[1-9]|1[0-4]) Fenología',k) and v not in ('','0')]
        if not nonzero:
            counts['no_recorded_stage']+=1
            continue
        label=STAGES.get(nonzero[0][0]) if len(nonzero)==1 and nonzero[0][1]=='2' else None
        visits[(key,date)].add(label)
    rows=[]
    for (key,date),labels in visits.items():
        if len(labels)!=1 or None in labels:
            counts['mixed_unmapped_or_atypical_visits']+=1
            continue
        xy,variety=next(iter(sites[key]))
        lon,lat=positions[xy]
        rows.append(['ES','|'.join(key[:-1]),date,lat,lon,next(iter(labels)),variety,'Unknown',key[0]])
    with (SOURCES/'spain.csv').open('w') as f:
        w=csv.writer(f);w.writerow('country site parsed_date lat_clean lon_clean label rice_variety rice_ecosystem province'.split());w.writerows(sorted(rows))
    counts['accepted_parcel_dates']=len(rows)
    (RESULTS/'results/spain_audit.json').write_text(json.dumps(counts,indent=2))
    print(counts)
