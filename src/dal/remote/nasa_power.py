#!/usr/bin/env python3
"""
Oryza-Elo: NASA POWER Agroclimatology API Remote Client & Local Cache Adapter.

Responsible for retrieving daily surface meteorological reanalysis series:
- T2M_MAX: Maximum 2-meter air temperature (°C)
- T2M_MIN: Minimum 2-meter air temperature (°C)
- T2M: Mean 2-meter air temperature (°C)
- PRECTOTCORR: Corrected precipitation (mm/day)
- ALLSKY_SFC_SW_DWN: All-sky surface shortwave downward irradiance (MJ/m^2/day)
- RH2M: 2-meter relative humidity (%)

Optimized with 0.5° x 0.5° spatial grid clustering and persistent atomic caching.
"""

import os
import json
import time
import random
from datetime import datetime, timedelta
import requests
import pandas as pd
import numpy as np

NASA_POWER_BASE_URL = "https://power.larc.nasa.gov/api/temporal/daily/point"
COMMUNITY = "AG"
PARAMETERS = "T2M_MAX,T2M_MIN,T2M,PRECTOTCORR,ALLSKY_SFC_SW_DWN,RH2M"

def get_grid_cell(lat: float, lon: float) -> tuple[float, float]:
    """Rounds coordinates to the native NASA POWER 0.5° x 0.5° atmospheric grid."""
    return (round(lat * 2) / 2, round(lon * 2) / 2)

class NasaPowerClient:
    def __init__(self, cache_dir: str | None = None):
        if cache_dir is None:
            repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '../../..'))
            cache_dir = os.path.join(repo_root, 'src/dal/data/raw/principal/nasa_power_cache')
        self.cache_dir = cache_dir
        os.makedirs(self.cache_dir, exist_ok=True)

    def _cache_path(self, grid_lat: float, grid_lon: float) -> str:
        return os.path.join(self.cache_dir, f"grid_{grid_lat:.1f}_{grid_lon:.1f}.json")

    def fetch_grid_series(
        self,
        grid_lat: float,
        grid_lon: float,
        start_date: str = "20221101",
        end_date: str = "20250930",
        max_retries: int = 5
    ) -> dict[str, dict[str, float]]:
        """
        Fetches or loads from cache the full daily climate time-series for a grid cell.
        Returns a dict of parameters: {param: {YYYYMMDD: value}}
        """
        cache_file = self._cache_path(grid_lat, grid_lon)
        if os.path.exists(cache_file):
            try:
                with open(cache_file, 'r', encoding='utf-8') as f:
                    cached = json.load(f)
                if 'properties' in cached and 'parameter' in cached['properties']:
                    params = cached['properties']['parameter']
                    if all(p in params for p in PARAMETERS.split(',')):
                        return params
            except Exception as e:
                print(f"[NasaPowerClient] Cache read error for ({grid_lat}, {grid_lon}): {e}, refetching...")

        params = {
            'latitude': grid_lat,
            'longitude': grid_lon,
            'start': start_date,
            'end': end_date,
            'parameters': PARAMETERS,
            'community': COMMUNITY,
            'format': 'JSON'
        }

        for attempt in range(1, max_retries + 1):
            try:
                resp = requests.get(NASA_POWER_BASE_URL, params=params, timeout=45)
                if resp.status_code == 200:
                    data = resp.json()
                    # Atomic write to cache
                    temp_file = cache_file + f".tmp.{os.getpid()}"
                    with open(temp_file, 'w', encoding='utf-8') as f:
                        json.dump(data, f)
                    os.replace(temp_file, cache_file)
                    return data['properties']['parameter']
                elif resp.status_code == 429:
                    sleep_s = (2 ** attempt) + random.uniform(0.5, 2.0)
                    print(f"[NasaPowerClient] Rate limit 429 at ({grid_lat}, {grid_lon}). Backing off {sleep_s:.2f}s...")
                    time.sleep(sleep_s)
                else:
                    print(f"[NasaPowerClient] HTTP {resp.status_code} at ({grid_lat}, {grid_lon}): {resp.text[:100]}")
                    time.sleep(1.0)
            except Exception as e:
                sleep_s = (2 ** attempt) + random.uniform(0.5, 1.5)
                print(f"[NasaPowerClient] Request exception attempt {attempt}/{max_retries}: {e}. Retrying in {sleep_s:.2f}s...")
                time.sleep(sleep_s)

        raise RuntimeError(f"Failed to fetch NASA POWER data for grid ({grid_lat}, {grid_lon}) after {max_retries} attempts.")

    def clean_series(self, series_dict: dict[str, float]) -> pd.Series:
        """Converts date strings to datetime index and cleans sentinel values (-999)."""
        s = pd.Series(series_dict)
        s.index = pd.to_datetime(s.index, format='%Y%m%d')
        s = s.sort_index()
        # Replace sentinel -999 or -999.0 with NaN then forward/backward fill
        s = s.replace(-999.0, np.nan).replace(-999, np.nan)
        if s.isna().any():
            s = s.interpolate(method='time').bfill().ffill()
        return s

def run_ingestion_pipeline():
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '../../..'))
    raw_csv = os.path.join(repo_root, 'src/dal/data/raw/principal/ricepest_survey_combined_66_68.csv')

    print("=" * 70)
    print("ORYZA-ELO: INGESTÃO E CACHE CLIMÁTICO NASA POWER (ISSUE #2)")
    print("=" * 70)

    df = pd.read_csv(raw_csv)
    print(f"Observações carregadas: {len(df):,}")

    # Forensic coordinate cleaning
    df['lat_clean'] = pd.to_numeric(df['lat'], errors='coerce')
    df['lon_clean'] = pd.to_numeric(df['lon'], errors='coerce')
    df.loc[df['lat_clean'] > 90, 'lat_clean'] /= 1e6
    df.loc[df['lon_clean'] > 180, 'lon_clean'] /= 1e6

    prov_lon_median = df.groupby('province')['lon_clean'].transform(
        lambda s: s[(s >= 97.0) & (s <= 106.0)].median()
    )
    out_bounds_lon = (df['lon_clean'] < 97.0) | (df['lon_clean'] > 106.0)
    df.loc[out_bounds_lon, 'lon_clean'] = prov_lon_median[out_bounds_lon]

    # Map to 0.5° grid cells
    df['grid_cell'] = [get_grid_cell(r['lat_clean'], r['lon_clean']) for _, r in df.iterrows()]
    unique_cells = sorted(list(set(df['grid_cell'])))
    print(f"Células de grade únicas identificadas: {len(unique_cells)} células")

    client = NasaPowerClient()

    # Progressively fetch all grid cells
    t0 = time.time()
    for idx, (g_lat, g_lon) in enumerate(unique_cells, 1):
        cache_file = client._cache_path(g_lat, g_lon)
        is_cached = os.path.exists(cache_file)
        status_msg = "CACHE HIT" if is_cached else "FETCHING"
        print(f"[{idx:03d}/{len(unique_cells):03d}] Grade ({g_lat:5.1f}°N, {g_lon:5.1f}°E) ... {status_msg}", end="", flush=True)

        t_call = time.time()
        client.fetch_grid_series(g_lat, g_lon)
        elapsed_call = time.time() - t_call
        print(f" ({elapsed_call:.2f}s)")

        # Brief pause between live HTTP calls to be polite to NASA API
        if not is_cached:
            time.sleep(0.3)

    total_time = time.time() - t0
    print("-" * 70)
    print(f"Ingestão concluída com sucesso em {total_time:.2f}s!")
    print(f"100% das {len(unique_cells)} células de grade salvas em: {client.cache_dir}")
    print("=" * 70)

if __name__ == '__main__':
    run_ingestion_pipeline()
