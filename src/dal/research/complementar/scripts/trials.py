"""Build observed heading dates from two primary field studies."""
import csv
import datetime as dt
import subprocess
from pathlib import Path
from paths import SOURCES as RAW
from urllib.request import urlretrieve

def download(name, url):
    if not (RAW / name).is_file():
        urlretrieve(url, RAW / name)


SHA='2e9a01bee7d2c604ef16e7ed1c56361e7de5406f'
rows=[]
for year in (2021,2022):
    file=f'us{year}.csv'
    url=f'https://raw.githubusercontent.com/FareedFarag/TPPJ-Modeling-Code/{SHA}/Hypothesis_2/Datasets/Agronomic/{year}/Dataset_{year}.csv'
    download(file,url)
    for row in csv.DictReader((RAW/file).open()):
        if row['Heading_50'] in ('NA','') or row['Emergence_Date_DOY'] in ('NA',''):
            continue
        emergence=int(row['Emergence_Date_DOY'])
        heading=int(row['Heading_50'])
        assert 1<=emergence<=366 and 30<=heading<=190
        date=dt.date(year,1,1)+dt.timedelta(days=emergence-1+heading)
        rows.append(['US','Stuttgart|'+row['Plot_Number'],date,34.5,-91.5,'heading',row['Rice_Cultivar'],'Unknown','Arkansas'])
url='https://www.nounavi-aomori.jp/farmer/wp-content/uploads/2024/08/4b55ec8b241fd9b0b9eff0bec315f5c6-1.pdf'
download('aomori_2024.pdf',url)
table=subprocess.check_output(['pdftotext','-layout',RAW/'aomori_2024.pdf','-'],text=True).split('葉耳間長')[1].split('注 1')[0]
# The 本年 and 前年 columns report these values for 2024 and 2023, respectively.
for name,day in [('はれわたり',29),('まっしぐら',28),('青天の霹靂',30)]:
    line=next(s for s in table.splitlines() if name in s)
    assert f'7/{day}' in line and line.strip().endswith(f'7/{day}')
    for year in (2023,2024):
        rows.append(['JP','Kuroishi|'+name,dt.date(year,7,day),40.665489,140.588745,'heading',name,'Unknown','Aomori'])
with (RAW/'trials.csv').open('w') as f:
    w=csv.writer(f);w.writerow('country site parsed_date lat_clean lon_clean label rice_variety rice_ecosystem province'.split());w.writerows(rows)
