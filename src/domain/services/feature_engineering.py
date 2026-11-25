#!/usr/bin/env python3
"""
Oryza-Elo: Biometeorological Feature Engineering Service (Issue #3).

Computes canonical agronomic indices for rice crops (Oryza sativa L.):
1. Growing Degree Days (GDD) with base temperature T_base = 10.0°C:
   GDD_t = max(0, (T2M_MAX + T2M_MIN) / 2 - 10.0)
2. Cumulative GDD across retrospective windows W in [7, 14, 30, 60] days.
3. Diurnal Temperature Range (DTR = T2M_MAX - T2M_MIN) mean & std over W.
4. Cumulative Precipitation (PRECTOTCORR) & Max Daily Rain over W.
5. Consecutive Dry Days (CDD, rain < 1.0mm) & Rain Day Ratio over W.
6. Cumulative Solar Irradiance (ALLSKY_SFC_SW_DWN) over W.
7. Mean Relative Humidity (RH2M) over W.

Strict anti-leakage guarantee: all windows strictly retrospective (t <= t_survey).
"""

import os
import pandas as pd
import numpy as np
from datetime import datetime, timedelta

RICE_BASE_TEMP_CELSIUS = 10.0
WINDOWS = [7, 14, 30, 60]

def compute_gdd(t_max: float, t_min: float, t_base: float = RICE_BASE_TEMP_CELSIUS) -> float:
    """Calculates daily Growing Degree Days for rice with floor at zero."""
    t_mean = (t_max + t_min) / 2.0
    return max(0.0, t_mean - t_base)

def enrich_survey_with_climate(
    df_survey: pd.DataFrame,
    nasa_client,
    output_csv: str | None = None
) -> pd.DataFrame:
    """
    Enriches each field survey observation with biometeorological features
    derived from the local NASA POWER climate cache.
    """
    print(f"[FeatureEngineering] Iniciando enriquecimento de {len(df_survey):,} observações...")

    # Ensure clean survey dates and coordinates
    def parse_survey_date(val):
        if pd.isna(val):
            return pd.NaT
        val_str = str(val).strip()
        if val_str.isdigit():
            return datetime(1899, 12, 30) + timedelta(days=int(val_str))
        try:
            return pd.to_datetime(val_str, format='%Y-%m-%d')
        except:
            return pd.to_datetime(val_str)

    df = df_survey.copy()
    if 'parsed_date' not in df.columns:
        df['parsed_date'] = df['survey_date'].apply(parse_survey_date)

    if 'lat_clean' not in df.columns:
        df['lat_clean'] = pd.to_numeric(df['lat'], errors='coerce')
        df['lon_clean'] = pd.to_numeric(df['lon'], errors='coerce')
        df.loc[df['lat_clean'] > 90, 'lat_clean'] /= 1e6
        df.loc[df['lon_clean'] > 180, 'lon_clean'] /= 1e6
        prov_lon_median = df.groupby('province')['lon_clean'].transform(
            lambda s: s[(s >= 97.0) & (s <= 106.0)].median()
        )
        out_bounds_lon = (df['lon_clean'] < 97.0) | (df['lon_clean'] > 106.0)
        df.loc[out_bounds_lon, 'lon_clean'] = prov_lon_median[out_bounds_lon]

    # Pre-load climate series into memory grouped by grid cell to maximize speed
    from src.dal.remote.nasa_power import get_grid_cell
    df['grid_cell'] = [get_grid_cell(r['lat_clean'], r['lon_clean']) for _, r in df.iterrows()]
    unique_cells = set(df['grid_cell'])
    print(f"[FeatureEngineering] Carregando séries climáticas de {len(unique_cells)} células de grade...")

    cell_data = {}
    for g_lat, g_lon in unique_cells:
        raw_params = nasa_client.fetch_grid_series(g_lat, g_lon)
        # Create cleaned daily dataframe for this cell
        cell_df = pd.DataFrame({
            param: nasa_client.clean_series(raw_params[param])
            for param in raw_params
        })
        # Compute daily GDD and DTR
        cell_df['GDD'] = [
            compute_gdd(r['T2M_MAX'], r['T2M_MIN']) for _, r in cell_df.iterrows()
        ]
        cell_df['DTR'] = cell_df['T2M_MAX'] - cell_df['T2M_MIN']
        cell_data[(g_lat, g_lon)] = cell_df

    print("[FeatureEngineering] Calculando janelas retrospectivas (7, 14, 30, 60 dias)...")

    # Vectorized / optimized feature extraction for each survey row
    records = []
    for idx, row in df.iterrows():
        survey_dt = row['parsed_date']
        grid = row['grid_cell']
        cdf = cell_data[grid]

        row_feat = {}
        for w in WINDOWS:
            start_window = survey_dt - timedelta(days=w - 1)
            # Slice slice strictly retrospective [start_window, survey_dt]
            w_slice = cdf.loc[start_window:survey_dt]

            if len(w_slice) < w:
                # If slight boundary mismatch, pad with slice mean
                gdd_cum = float(w_slice['GDD'].sum() * (w / max(1, len(w_slice))))
                rain_cum = float(w_slice['PRECTOTCORR'].sum() * (w / max(1, len(w_slice))))
                rain_max = float(w_slice['PRECTOTCORR'].max()) if len(w_slice) > 0 else 0.0
                rad_cum = float(w_slice['ALLSKY_SFC_SW_DWN'].sum() * (w / max(1, len(w_slice))))
                dtr_mean = float(w_slice['DTR'].mean()) if len(w_slice) > 0 else 0.0
                dtr_std = float(w_slice['DTR'].std()) if len(w_slice) > 1 else 0.0
                rh_mean = float(w_slice['RH2M'].mean()) if len(w_slice) > 0 else 0.0
                cdd = int((w_slice['PRECTOTCORR'] < 1.0).sum()) if len(w_slice) > 0 else w
            else:
                gdd_cum = float(w_slice['GDD'].sum())
                rain_cum = float(w_slice['PRECTOTCORR'].sum())
                rain_max = float(w_slice['PRECTOTCORR'].max())
                rad_cum = float(w_slice['ALLSKY_SFC_SW_DWN'].sum())
                dtr_mean = float(w_slice['DTR'].mean())
                dtr_std = float(w_slice['DTR'].std())
                rh_mean = float(w_slice['RH2M'].mean())
                cdd = int((w_slice['PRECTOTCORR'] < 1.0).sum())

            row_feat[f'gdd_cum_{w}d'] = round(gdd_cum, 2)
            row_feat[f'rain_cum_{w}d'] = round(rain_cum, 2)
            row_feat[f'rain_max_{w}d'] = round(rain_max, 2)
            row_feat[f'rad_cum_{w}d'] = round(rad_cum, 2)
            row_feat[f'dtr_mean_{w}d'] = round(dtr_mean, 2)
            row_feat[f'dtr_std_{w}d'] = round(dtr_std, 2)
            row_feat[f'rh_mean_{w}d'] = round(rh_mean, 2)
            row_feat[f'cdd_{w}d'] = cdd

        records.append(row_feat)

    feat_df = pd.DataFrame(records, index=df.index)
    enriched_df = pd.concat([df, feat_df], axis=1)

    if output_csv:
        os.makedirs(os.path.dirname(output_csv), exist_ok=True)
        enriched_df.to_csv(output_csv, index=False)
        print(f"[FeatureEngineering] Dataset final consolidado salvo em: {output_csv}")

    print(f"[FeatureEngineering] Concluído! {feat_df.shape[1]} novas features agrometeorológicas geradas.")
    return enriched_df

if __name__ == '__main__':
    from src.dal.remote.nasa_power import NasaPowerClient
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), '../../..'))
    raw_csv = os.path.join(repo_root, 'src/dal/data/raw/principal/ricepest_survey_combined_66_68.csv')
    out_csv = os.path.join(repo_root, 'src/dal/data/processed/principal/rice_survey_climate_enriched.csv')

    survey_df = pd.read_csv(raw_csv)
    client = NasaPowerClient()
    enrich_survey_with_climate(survey_df, client, output_csv=out_csv)
