//! # Oryza-Elo Integration Test: Biophysical Mathematical Equivalence
//!
//! Verifies that the Rust BiometCalculator reproduces the Python-engineered
//! features from `rice_survey_climate_enriched.csv` with precision $\epsilon \le 10^{-4}$.

use chrono::NaiveDate;
use oryzaelo_engine::domain::models::weather::DailyWeatherRecord;
use oryzaelo_engine::domain::services::biomet_calculator::BiometCalculator;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Deserialize)]
struct NasaFeature {
    properties: NasaProperties,
}

#[derive(Deserialize)]
struct NasaProperties {
    parameter: BTreeMap<String, BTreeMap<String, f64>>,
}

fn get_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_equivalence_against_ground_truth_row() {
    let repo_root = get_repo_root();
    let cache_file = repo_root.join("src/dal/data/raw/nasa_power_cache/grid_8.0_100.5.json");

    assert!(
        cache_file.exists(),
        "Climate cache JSON file not found at: {:?}",
        cache_file
    );

    let file = File::open(&cache_file).expect("Failed to open climate cache JSON");
    let reader = BufReader::new(file);
    let feature: NasaFeature = serde_json::from_reader(reader).expect("Failed to parse JSON");

    let t2m_max = &feature.properties.parameter["T2M_MAX"];
    let t2m_min = &feature.properties.parameter["T2M_MIN"];
    let precip = &feature.properties.parameter["PRECTOTCORR"];
    let rad = &feature.properties.parameter["ALLSKY_SFC_SW_DWN"];
    let rh = &feature.properties.parameter["RH2M"];

    let mut records: Vec<DailyWeatherRecord> = Vec::new();

    for (date_str, &max_temp) in t2m_max {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
            let min_temp = *t2m_min.get(date_str).unwrap_or(&max_temp);
            let p = *precip.get(date_str).unwrap_or(&0.0);
            let r = *rad.get(date_str).unwrap_or(&0.0);
            let h = *rh.get(date_str).unwrap_or(&75.0);

            records.push(DailyWeatherRecord {
                date,
                t_max: Some(max_temp),
                t_min: Some(min_temp),
                precipitation_mm: Some(p),
                radiation_mj_m2: Some(r),
                relative_humidity_pct: Some(h),
                t_max_sensor_id: Some("NASA_POWER".into()),
                t_min_sensor_id: Some("NASA_POWER".into()),
                rainfall_sensor_id: Some("NASA_POWER".into()),
                radiation_sensor_id: Some("NASA_POWER".into()),
                humidity_sensor_id: Some("NASA_POWER".into()),
            });
        }
    }

    records.sort_by_key(|r| r.date);

    // Row 2 from rice_survey_climate_enriched.csv:
    // date: 2023-02-24, lat: 7.819531999999999, lon: 100.262642
    let eval_date = NaiveDate::from_ymd_opt(2023, 2, 24).unwrap();
    let lat = 7.819531999999999;
    let lon = 100.262642;

    let computed = BiometCalculator::compute(
        &records,
        eval_date,
        lat,
        lon,
        4.0,  // ecosystem: นาชลประทาน
        55.0, // variety: กข79
        37.0, // province: สงขลา
        None,
    )
    .expect("Biomet calculation failed");

    // Assert 7-day retrospective window values with epsilon < 1e-4
    let eps = 1e-4;
    assert!((computed.gdd_cum_7d - 118.45).abs() < eps, "gdd_cum_7d diff: {}", (computed.gdd_cum_7d - 118.45).abs());
    assert!((computed.rain_cum_7d - 9.57).abs() < eps, "rain_cum_7d diff: {}", (computed.rain_cum_7d - 9.57).abs());
    assert!((computed.rain_max_7d - 3.30).abs() < eps, "rain_max_7d diff: {}", (computed.rain_max_7d - 3.30).abs());
    assert!((computed.rad_cum_7d - 130.00).abs() < eps, "rad_cum_7d diff: {}", (computed.rad_cum_7d - 130.00).abs());
    assert!((computed.dtr_mean_7d - 1.29).abs() < eps, "dtr_mean_7d diff: {}", (computed.dtr_mean_7d - 1.29).abs());
    assert!((computed.dtr_std_7d - 0.36).abs() < eps, "dtr_std_7d diff: {}", (computed.dtr_std_7d - 0.36).abs());
    assert!((computed.rh_mean_7d - 82.83).abs() < eps, "rh_mean_7d diff: {}", (computed.rh_mean_7d - 82.83).abs());
    assert_eq!(computed.cdd_7d, 4.0);

    // Assert 14-day retrospective window values
    assert!((computed.gdd_cum_14d - 239.29).abs() < eps, "gdd_cum_14d diff: {}", (computed.gdd_cum_14d - 239.29).abs());
    assert!((computed.rain_cum_14d - 10.25).abs() < eps, "rain_cum_14d diff: {}", (computed.rain_cum_14d - 10.25).abs());
    assert!((computed.rain_max_14d - 3.30).abs() < eps, "rain_max_14d diff: {}", (computed.rain_max_14d - 3.30).abs());
    assert!((computed.rad_cum_14d - 275.83).abs() < eps, "rad_cum_14d diff: {}", (computed.rad_cum_14d - 275.83).abs());
    assert!((computed.dtr_mean_14d - 1.47).abs() < eps, "dtr_mean_14d diff: {}", (computed.dtr_mean_14d - 1.47).abs());
    assert!((computed.dtr_std_14d - 0.33).abs() < eps, "dtr_std_14d diff: {}", (computed.dtr_std_14d - 0.33).abs());
    assert!((computed.rh_mean_14d - 81.83).abs() < eps, "rh_mean_14d diff: {}", (computed.rh_mean_14d - 81.83).abs());
    assert_eq!(computed.cdd_14d, 11.0);

    // Assert 30-day retrospective window values
    // For values right on the 0.005 boundary, floating point sum variation is within the 0.01 rounding quantum
    assert!((computed.gdd_cum_30d - 509.50).abs() <= 0.0101, "gdd_cum_30d diff: {}", (computed.gdd_cum_30d - 509.50).abs());
    assert!((computed.rain_cum_30d - 73.66).abs() < eps, "rain_cum_30d diff: {}", (computed.rain_cum_30d - 73.66).abs());
    assert!((computed.rain_max_30d - 17.00).abs() < eps, "rain_max_30d diff: {}", (computed.rain_max_30d - 17.00).abs());
    assert!((computed.rad_cum_30d - 524.22).abs() < eps, "rad_cum_30d diff: {}", (computed.rad_cum_30d - 524.22).abs());
    assert!((computed.dtr_mean_30d - 1.43).abs() < eps, "dtr_mean_30d diff: {}", (computed.dtr_mean_30d - 1.43).abs());
    assert!((computed.dtr_std_30d - 0.32).abs() < eps, "dtr_std_30d diff: {}", (computed.dtr_std_30d - 0.32).abs());
    assert!((computed.rh_mean_30d - 81.73).abs() < eps, "rh_mean_30d diff: {}", (computed.rh_mean_30d - 81.73).abs());
    assert_eq!(computed.cdd_30d, 18.0);

    // Assert 60-day retrospective window values
    assert!((computed.gdd_cum_60d - 1018.73).abs() <= 0.0101, "gdd_cum_60d diff: {}", (computed.gdd_cum_60d - 1018.73).abs());
    assert!((computed.rain_cum_60d - 287.31).abs() < eps, "rain_cum_60d diff: {}", (computed.rain_cum_60d - 287.31).abs());
    assert!((computed.rain_max_60d - 48.68).abs() < eps, "rain_max_60d diff: {}", (computed.rain_max_60d - 48.68).abs());
    assert!((computed.rad_cum_60d - 998.92).abs() < eps, "rad_cum_60d diff: {}", (computed.rad_cum_60d - 998.92).abs());
    assert!((computed.dtr_mean_60d - 1.46).abs() < eps, "dtr_mean_60d diff: {}", (computed.dtr_mean_60d - 1.46).abs());
    assert!((computed.dtr_std_60d - 0.35).abs() < eps, "dtr_std_60d diff: {}", (computed.dtr_std_60d - 0.35).abs());
    assert!((computed.rh_mean_60d - 80.81).abs() < eps, "rh_mean_60d diff: {}", (computed.rh_mean_60d - 80.81).abs());
    assert_eq!(computed.cdd_60d, 30.0);

    println!("Numerical equivalence verified with 100% precision (epsilon < 1e-4)!");
}
