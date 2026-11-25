"""Reproduz os recortes de pesquisa a partir dos arquivos locais."""
import argparse
import csv
import io
import re
import subprocess
import zipfile
from collections import Counter, defaultdict
from pathlib import Path
from statistics import mean, median

import hashlib
import sys
from urllib.request import urlretrieve

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from paths import REPO, SOURCES as INPUTS, THAI

ROOT = REPO / ".research-run/extracao-biologica"
SOURCES = ROOT / "fontes"


def download_inputs():
    SOURCES.mkdir(parents=True, exist_ok=True)
    with (INPUTS / "biologia/manifesto.csv").open() as stream:
        for row in csv.DictReader(stream):
            path = SOURCES / row["arquivo"]
            if not path.exists():
                urlretrieve(row["url"], path)
            if hashlib.sha256(path.read_bytes()).hexdigest() != row["sha256"]:
                raise ValueError(f"Checksum mismatch: {path}")

RKB = "https://rkb.ricethailand.go.th/web/content_page.php?code="
CULTIVARS = [
    ("กข6", "RD6", "sensível", "https://newwebs2.ricethailand.go.th/webmain/rkb3/01%E0%B8%81%E0%B8%826%20%28RD6%29.pdf"),
    ("ขาวดอกมะลิ105", "KDML105", "sensível", "https://newwebs2.ricethailand.go.th/upload/doc/57/1658045630.pdf"),
    ("กข41", "RD41", "insensível", RKB + "5M7FAIXGZS8ODOV67WOZLAJCRUKCC"),
    ("ปทุมธานี1", "PTT1", "insensível", RKB + "A-AVVLOJOC9D"),
]


def save(name, columns, rows):
    with (ROOT / name).open("w", encoding="utf-8", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(columns)
        writer.writerows(rows)


def photoperiod(features):
    with features.open(encoding="utf-8-sig", newline="") as stream:
        counts = Counter("".join(row["rice_variety"].split()) for row in csv.DictReader(stream))
    save("cultivares_fotoperiodo.csv", ["chave_sem_espacos", "cultivar", "fotoperiodo", "vistorias", "fonte"],
         ((key, cultivar, trait, counts[key], url) for key, cultivar, trait, url in CULTIVARS))


def leaf_measurements():
    paper = subprocess.check_output(["pdftotext", "-layout", str(SOURCES / "kasetsart_2019.pdf"), "-"], text=True)
    stages = ["muda", "início do perfilhamento", "máximo perfilhamento", "floração", "colheita", "colheita tardia"]
    values = []
    for table, cultivar in enumerate(["RD41", "PTT1", "KDML105"], start=1):
        section = paper.split(f"Table {table} Nutrient concentrations", 1)[1].split("Note:", 1)[0]
        matches = re.findall(r"(?m)^\s*([1-6])\s+Leaf\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)", section)
        if [m[0] for m in matches] != list("123456"):
            raise ValueError(f"Conferir a transcrição da tabela {table}")
        for stage, n, p, k in matches:
            values.append([cultivar, table, int(stage), stages[int(stage)-1], n, p, k])
    save("folha_npk_tailandia.csv", ["cultivar", "tabela", "estadio_estudo", "nome_estadio",
                                    "N_pct_massa_seca", "P_pct_massa_seca", "K_pct_massa_seca"], values)


def rice_face():
    with (SOURCES / "harvard_face.csv").open(encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        fields = reader.fieldnames
        save("face_arroz_japao.csv", fields, ([row[k] for k in fields] for row in reader if row["Crop"] == "Rice"))


def trial_audit():
    summary = []
    flowering = defaultdict(list)
    with zipfile.ZipFile(SOURCES / "cornell_irri_ryt.zip") as archive:
        for name in sorted(archive.namelist()):
            if not name.endswith(".csv") or "__MACOSX" in name:
                continue
            with archive.open(name) as raw, io.TextIOWrapper(raw, encoding="utf-8-sig", newline="") as stream:
                records = [row for row in csv.DictReader(stream) if row["GHID"].strip()]
            pairs = [(float(r["FL"]), float(r["MAT"])) for r in records
                     if all(r[k].strip() not in ("", "NA", ".") for k in ["FL", "MAT"])]
            for row in records:
                if row["FL"].strip() not in ("", "NA", "."):
                    flowering[row["YEAR"], row["GHID"], row["SEASON"]].append(float(row["FL"]))
            summary.append([Path(name).name, len(records), len(pairs), sum(m-f == 30 for f, m in pairs)])
    save("auditoria_filipinas.csv", ["arquivo", "linhas_com_GHID", "pares_FL_MAT", "MAT_igual_FL_mais_30"], summary)
    by_year = defaultdict(list)
    for (year, line, season), observations in flowering.items():
        wet = flowering.get((year, line, "2"))
        if season == "1" and wet:
            by_year[year].append(mean(wet) - mean(observations))
    save("floracao_por_safra_filipinas.csv", ["ano", "linhagens_pareadas", "mediana_dias_chuvosa_menos_seca"],
         ((year, len(deltas), median(deltas)) for year, deltas in sorted(by_year.items())))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("features", type=Path, nargs="?", default=THAI / "features.csv", help="features.csv do benchmark tailandês revision_v2")
    args = parser.parse_args()
    download_inputs()
    photoperiod(args.features)
    leaf_measurements()
    rice_face()
    trial_audit()
