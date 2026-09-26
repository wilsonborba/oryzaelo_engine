//! # Oryza-Elo Architecture Guardrail: Biophysical Feature Engineering Service
//!
//! High-performance Rust biometeorological calculator deriving 44 agronomic and
//! climate features from daily weather time series with strict anti-leakage guarantees.

use crate::core::error::{AppError, DomainError};
use crate::core::settings::RICE_BASE_TEMPERATURE_CELSIUS;
use crate::domain::models::weather::{BiometFeatures, DailyWeatherRecord};
use chrono::{Datelike, NaiveDate};
use std::f64::consts::PI;

/// Retrospective temporal rolling windows in days
pub const WINDOW_SIZES: [usize; 4] = [7, 14, 30, 60];

/// Biophysical agrometeorological calculator.
pub struct BiometCalculator;

impl BiometCalculator {
    /// Computes the complete 44-feature biometeorological vector for a given evaluation date
    /// and parcel coordinates, enforcing strict anti-leakage protocols.
    ///
    /// # Arguments
    /// * `weather_history` - Chronological slice of daily weather observations.
    /// * `eval_date` - Target observation date ($t_{\text{eval}}$). No data from $t > t_{\text{eval}}$ is consumed.
    /// * `lat` - Field centroid latitude in decimal degrees.
    /// * `lon` - Field centroid longitude in decimal degrees.
    /// * `rice_ecosystem_code` - Encoded agro-ecosystem integer.
    /// * `rice_variety_code` - Encoded rice cultivar integer.
    /// * `province_code` - Encoded administrative province integer.
    /// * `base_temp` - Optional physiological base temperature (defaults to 10.0°C).
    pub fn compute(
        weather_history: &[DailyWeatherRecord],
        eval_date: NaiveDate,
        lat: f64,
        lon: f64,
        rice_ecosystem_code: f64,
        rice_variety_code: f64,
        province_code: f64,
        base_temp: Option<f64>,
    ) -> Result<BiometFeatures, AppError> {
        let t_base = base_temp.unwrap_or(RICE_BASE_TEMPERATURE_CELSIUS);

        // Strict Anti-Data-Leakage: retain only records where date <= eval_date.
        // Only complete days (every sensor reported) ever enter the ML
        // feature vector -- a partial day would corrupt a deterministic
        // window sum/mean, so it's excluded here rather than being fed in
        // with a fabricated value.
        let mut retrospective: Vec<&DailyWeatherRecord> = weather_history
            .iter()
            .filter(|rec| rec.date <= eval_date && rec.is_complete())
            .collect();

        if retrospective.len() < 7 {
            return Err(AppError::Domain(DomainError::InsufficientWeatherData {
                required: 7,
                available: retrospective.len(),
            }));
        }

        // Sort chronologically ascending
        retrospective.sort_by_key(|rec| rec.date);

        // Map date to record for quick backward window lookup
        let window_7 = Self::compute_window(&retrospective, eval_date, 7, t_base);
        let window_14 = Self::compute_window(&retrospective, eval_date, 14, t_base);
        let window_30 = Self::compute_window(&retrospective, eval_date, 30, t_base);
        let window_60 = Self::compute_window(&retrospective, eval_date, 60, t_base);

        // Date-derived features
        let month = eval_date.month() as f64;
        let day_of_year = eval_date.ordinal() as f64;

        // Astronomical Photoperiod (Daylength in hours)
        let lat_rad = lat.to_radians();
        let declination = 0.409 * ((2.0 * PI * day_of_year / 365.25) - 1.39).sin();
        let cos_omega = (-lat_rad.tan() * declination.tan()).clamp(-1.0, 1.0);
        let photoperiod_hours = (24.0 / PI) * cos_omega.acos();

        // Photothermal Quotient (PTQ)
        let ptq_30d = window_30.rad_cum / (window_30.gdd_cum + 1e-5);
        let ptq_60d = window_60.rad_cum / (window_60.gdd_cum + 1e-5);

        // Vapor Pressure Deficit (VPD) Proxies
        let vpd_proxy_14d = window_14.dtr_mean * (100.0 - window_14.rh_mean) / 100.0;
        let vpd_proxy_30d = window_30.dtr_mean * (100.0 - window_30.rh_mean) / 100.0;

        Ok(BiometFeatures {
            gdd_cum_7d: window_7.gdd_cum,
            gdd_cum_14d: window_14.gdd_cum,
            gdd_cum_30d: window_30.gdd_cum,
            gdd_cum_60d: window_60.gdd_cum,

            rain_cum_7d: window_7.rain_cum,
            rain_cum_14d: window_14.rain_cum,
            rain_cum_30d: window_30.rain_cum,
            rain_cum_60d: window_60.rain_cum,

            rain_max_7d: window_7.rain_max,
            rain_max_14d: window_14.rain_max,
            rain_max_30d: window_30.rain_max,
            rain_max_60d: window_60.rain_max,

            cdd_7d: window_7.cdd,
            cdd_14d: window_14.cdd,
            cdd_30d: window_30.cdd,
            cdd_60d: window_60.cdd,

            dtr_mean_7d: window_7.dtr_mean,
            dtr_mean_14d: window_14.dtr_mean,
            dtr_mean_30d: window_30.dtr_mean,
            dtr_mean_60d: window_60.dtr_mean,

            dtr_std_7d: window_7.dtr_std,
            dtr_std_14d: window_14.dtr_std,
            dtr_std_30d: window_30.dtr_std,
            dtr_std_60d: window_60.dtr_std,

            rad_cum_7d: window_7.rad_cum,
            rad_cum_14d: window_14.rad_cum,
            rad_cum_30d: window_30.rad_cum,
            rad_cum_60d: window_60.rad_cum,

            rh_mean_7d: window_7.rh_mean,
            rh_mean_14d: window_14.rh_mean,
            rh_mean_30d: window_30.rh_mean,
            rh_mean_60d: window_60.rh_mean,

            month,
            day_of_year,
            photoperiod_hours,
            ptq_30d,
            ptq_60d,
            vpd_proxy_14d,
            vpd_proxy_30d,
            lat_clean: lat,
            lon_clean: lon,

            rice_ecosystem_code,
            rice_variety_code,
            province_code,
        })
    }

    /// Computes aggregated metrics for a single retrospective window of size $W$.
    fn compute_window(
        records: &[&DailyWeatherRecord],
        eval_date: NaiveDate,
        window_size: usize,
        t_base: f64,
    ) -> WindowMetrics {
        let start_date = eval_date - chrono::Duration::days((window_size - 1) as i64);

        // Filter records strictly within [start_date, eval_date]
        let slice: Vec<&&DailyWeatherRecord> = records
            .iter()
            .filter(|r| r.date >= start_date && r.date <= eval_date)
            .collect();

        let n = slice.len();
        if n == 0 {
            return WindowMetrics::default();
        }

        let scale = if n < window_size {
            window_size as f64 / n as f64
        } else {
            1.0
        };

        let mut gdd_sum = 0.0;
        let mut rain_sum = 0.0;
        let mut rain_max: f64 = 0.0;
        let mut rad_sum = 0.0;
        let mut rh_sum = 0.0;
        let mut cdd_count: f64 = 0.0;

        let mut dtr_vals = Vec::with_capacity(n);

        for &&rec in &slice {
            // Safe to unwrap: `retrospective` was already filtered to
            // complete-only records in `compute()`.
            let gdd = rec.daily_gdd(t_base).unwrap_or(0.0);
            let rain = rec.precipitation_mm.unwrap_or(0.0);
            gdd_sum += gdd;

            rain_sum += rain;
            if rain > rain_max {
                rain_max = rain;
            }
            if rain < 1.0 {
                cdd_count += 1.0;
            }

            rad_sum += rec.radiation_mj_m2.unwrap_or(0.0);
            rh_sum += rec.relative_humidity_pct.unwrap_or(0.0);

            dtr_vals.push(rec.dtr().unwrap_or(0.0));
        }

        let gdd_cum = round_2(gdd_sum * scale);
        let rain_cum = round_2(rain_sum * scale);
        let rain_max = round_2(rain_max);
        let rad_cum = round_2(rad_sum * scale);

        let dtr_mean = round_2(dtr_vals.iter().sum::<f64>() / n as f64);
        let dtr_std = if n > 1 {
            let variance = dtr_vals
                .iter()
                .map(|&v| {
                    let diff = v - (dtr_vals.iter().sum::<f64>() / n as f64);
                    diff * diff
                })
                .sum::<f64>()
                / (n - 1) as f64;
            round_2(variance.sqrt())
        } else {
            0.0
        };

        let rh_mean = round_2(rh_sum / n as f64);
        let cdd = cdd_count;

        WindowMetrics {
            gdd_cum,
            rain_cum,
            rain_max,
            rad_cum,
            dtr_mean,
            dtr_std,
            rh_mean,
            cdd,
        }
    }
}

/// Helper struct for window aggregations.
#[derive(Debug, Default, Clone, Copy)]
struct WindowMetrics {
    gdd_cum: f64,
    rain_cum: f64,
    rain_max: f64,
    rad_cum: f64,
    dtr_mean: f64,
    dtr_std: f64,
    rh_mean: f64,
    cdd: f64,
}

#[inline]
fn round_2(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_synthetic_series(start: NaiveDate, days: usize) -> Vec<DailyWeatherRecord> {
        (0..days)
            .map(|i| {
                let date = start + chrono::Duration::days(i as i64);
                DailyWeatherRecord {
                    date,
                    t_max: Some(32.0 + (i % 3) as f64),
                    t_min: Some(22.0 + (i % 2) as f64),
                    precipitation_mm: Some(if i % 5 == 0 { 15.0 } else { 0.2 }),
                    radiation_mj_m2: Some(18.0 + (i % 4) as f64),
                    relative_humidity_pct: Some(75.0 + (i % 5) as f64),
                    t_max_sensor_id: Some("synthetic".into()),
                    t_min_sensor_id: Some("synthetic".into()),
                    rainfall_sensor_id: Some("synthetic".into()),
                    radiation_sensor_id: Some("synthetic".into()),
                    humidity_sensor_id: Some("synthetic".into()),
                }
            })
            .collect()
    }

    #[test]
    fn test_biomet_calculator_synthetic_benchmark() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let history = create_synthetic_series(start, 70);
        let eval_date = start + chrono::Duration::days(65);

        let start_time = std::time::Instant::now();
        let features = BiometCalculator::compute(
            &history,
            eval_date,
            14.88,
            100.45,
            4.0,
            73.0,
            6.0,
            None,
        )
        .expect("Calculation failed");
        let duration = start_time.elapsed();

        println!("BiometCalculator 60d execution duration: {:?}", duration);
        assert!(duration.as_micros() < 500, "Calculation took too long: {:?}", duration);

        assert_eq!(features.month, 3.0);
        assert!(features.photoperiod_hours > 11.5 && features.photoperiod_hours < 13.0);
        assert!(features.gdd_cum_7d > 0.0);
        assert!(features.gdd_cum_60d > features.gdd_cum_30d);
        assert_eq!(features.to_tensor_vec().len(), 44);
    }

    #[test]
    fn test_anti_leakage_guarantee() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let mut history = create_synthetic_series(start, 60);
        let eval_date = start + chrono::Duration::days(30);

        // Corrupt future record (t > eval_date) with an extreme value
        history.push(DailyWeatherRecord {
            date: eval_date + chrono::Duration::days(1),
            t_max: Some(55.0),
            t_min: Some(45.0),
            precipitation_mm: Some(500.0),
            radiation_mj_m2: Some(35.0),
            relative_humidity_pct: Some(99.0),
            t_max_sensor_id: Some("future-leak".into()),
            t_min_sensor_id: Some("future-leak".into()),
            rainfall_sensor_id: Some("future-leak".into()),
            radiation_sensor_id: Some("future-leak".into()),
            humidity_sensor_id: Some("future-leak".into()),
        });

        let feat = BiometCalculator::compute(
            &history,
            eval_date,
            15.0,
            102.0,
            4.0,
            1.0,
            1.0,
            None,
        )
        .unwrap();

        // The future record must not have influenced rain_max
        assert!(feat.rain_max_7d < 100.0);
        assert!(feat.rain_max_30d < 100.0);
    }
}
