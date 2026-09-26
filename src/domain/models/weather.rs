//! # Oryza-Elo Architecture Guardrail: Weather & Biomet Models
//!
//! Daily agrometeorological observations and engineered biometeorological feature vectors.
//!
//! Every metric field is nullable: a day is genuinely partial until every
//! sensor covering this parcel has reported for it (a rain-only sensor and a
//! temperature-only sensor may report at completely different times). This
//! is a derived/aggregated view built from `sensor_readings_*` -- never
//! written to directly by a single ingest call.

use crate::core::error::{DomainError, DomainResult};
use crate::domain::models::metric_type::MetricType;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Daily agrometeorological ground record, aggregated from one or more sensors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyWeatherRecord {
    /// Observation date
    pub date: NaiveDate,
    /// Maximum 2-meter air temperature (°C)
    pub t_max: Option<f64>,
    /// Minimum 2-meter air temperature (°C)
    pub t_min: Option<f64>,
    /// Daily cumulative precipitation (mm)
    pub precipitation_mm: Option<f64>,
    /// Daily surface all-sky solar radiation (MJ/m²/day)
    pub radiation_mj_m2: Option<f64>,
    /// Mean 2-meter relative humidity (%)
    pub relative_humidity_pct: Option<f64>,

    /// Which sensor last reported each field -- provenance is per-field,
    /// never a single "source" string for the whole day.
    pub t_max_sensor_id: Option<String>,
    pub t_min_sensor_id: Option<String>,
    pub rainfall_sensor_id: Option<String>,
    pub radiation_sensor_id: Option<String>,
    pub humidity_sensor_id: Option<String>,
}

impl DailyWeatherRecord {
    /// True only when every one of the 5 canonical metrics has been reported
    /// for this day. ML feature engineering (GDD windows, phenology
    /// inference) only ever consumes complete days.
    pub fn is_complete(&self) -> bool {
        self.t_max.is_some()
            && self.t_min.is_some()
            && self.precipitation_mm.is_some()
            && self.radiation_mj_m2.is_some()
            && self.relative_humidity_pct.is_some()
    }

    pub fn is_partial(&self) -> bool {
        !self.is_complete()
    }

    /// Which of the 5 canonical metrics are still missing for this day, so
    /// the UI can tell a farmer exactly what's outstanding (e.g. "waiting on
    /// rainfall") instead of just showing an opaque "incomplete" badge.
    pub fn missing_metrics(&self) -> Vec<MetricType> {
        let mut missing = Vec::new();
        if self.t_max.is_none() {
            missing.push(MetricType::TMax);
        }
        if self.t_min.is_none() {
            missing.push(MetricType::TMin);
        }
        if self.precipitation_mm.is_none() {
            missing.push(MetricType::Rainfall);
        }
        if self.radiation_mj_m2.is_none() {
            missing.push(MetricType::Radiation);
        }
        if self.relative_humidity_pct.is_none() {
            missing.push(MetricType::Humidity);
        }
        missing
    }

    /// Validates physical laws and domain invariants across whichever
    /// fields are actually present -- a partial day can't violate a
    /// cross-field law it doesn't yet have both sides of.
    pub fn validate(&self) -> DomainResult<()> {
        if let (Some(t_min), Some(t_max)) = (self.t_min, self.t_max) {
            if t_min > t_max {
                return Err(DomainError::InvalidWeatherRecord {
                    message: format!(
                        "T_min ({:.2}°C) cannot exceed T_max ({:.2}°C) on date {}",
                        t_min, t_max, self.date
                    ),
                });
            }
            if t_min < -40.0 || t_max > 60.0 {
                return Err(DomainError::InvalidWeatherRecord {
                    message: format!(
                        "Extreme temperature outlier detected: T_min={:.2}°C, T_max={:.2}°C on date {}",
                        t_min, t_max, self.date
                    ),
                });
            }
        }
        if let Some(rain) = self.precipitation_mm {
            if rain < 0.0 {
                return Err(DomainError::InvalidWeatherRecord {
                    message: format!("Precipitation ({:.2} mm) cannot be negative on date {}", rain, self.date),
                });
            }
        }
        if let Some(rad) = self.radiation_mj_m2 {
            if rad < 0.0 {
                return Err(DomainError::InvalidWeatherRecord {
                    message: format!("Solar radiation ({:.2} MJ/m²) cannot be negative on date {}", rad, self.date),
                });
            }
        }
        if let Some(rh) = self.relative_humidity_pct {
            if !(0.0..=100.0).contains(&rh) {
                return Err(DomainError::InvalidWeatherRecord {
                    message: format!(
                        "Relative humidity ({:.1}%) must be within [0.0, 100.0] on date {}",
                        rh, self.date
                    ),
                });
            }
        }
        Ok(())
    }

    /// Calculates Growing Degree Days (GDD) for this single day given base temperature.
    /// Formula: GDD = max(0.0, ((T_max + T_min) / 2.0) - T_base)
    /// `None` unless both T_max and T_min are present for this day.
    pub fn daily_gdd(&self, base_temp_celsius: f64) -> Option<f64> {
        let t_max = self.t_max?;
        let t_min = self.t_min?;
        let t_mean = (t_max + t_min) / 2.0;
        Some((t_mean - base_temp_celsius).max(0.0))
    }

    /// Calculates Diurnal Temperature Range (DTR) = T_max - T_min.
    /// `None` unless both T_max and T_min are present for this day.
    pub fn dtr(&self) -> Option<f64> {
        let t_max = self.t_max?;
        let t_min = self.t_min?;
        Some((t_max - t_min).max(0.0))
    }
}

/// The complete 44-feature vector strictly ordered for the CatBoost ONNX inference session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiometFeatures {
    // 32 Retrospective Rolling Window Features (7d, 14d, 30d, 60d)
    pub gdd_cum_7d: f64,
    pub gdd_cum_14d: f64,
    pub gdd_cum_30d: f64,
    pub gdd_cum_60d: f64,

    pub rain_cum_7d: f64,
    pub rain_cum_14d: f64,
    pub rain_cum_30d: f64,
    pub rain_cum_60d: f64,

    pub rain_max_7d: f64,
    pub rain_max_14d: f64,
    pub rain_max_30d: f64,
    pub rain_max_60d: f64,

    pub cdd_7d: f64,
    pub cdd_14d: f64,
    pub cdd_30d: f64,
    pub cdd_60d: f64,

    pub dtr_mean_7d: f64,
    pub dtr_mean_14d: f64,
    pub dtr_mean_30d: f64,
    pub dtr_mean_60d: f64,

    pub dtr_std_7d: f64,
    pub dtr_std_14d: f64,
    pub dtr_std_30d: f64,
    pub dtr_std_60d: f64,

    pub rad_cum_7d: f64,
    pub rad_cum_14d: f64,
    pub rad_cum_30d: f64,
    pub rad_cum_60d: f64,

    pub rh_mean_7d: f64,
    pub rh_mean_14d: f64,
    pub rh_mean_30d: f64,
    pub rh_mean_60d: f64,

    // 9 Derived & Agronomic Features
    pub month: f64,
    pub day_of_year: f64,
    pub photoperiod_hours: f64,
    pub ptq_30d: f64,
    pub ptq_60d: f64,
    pub vpd_proxy_14d: f64,
    pub vpd_proxy_30d: f64,
    pub lat_clean: f64,
    pub lon_clean: f64,

    // 3 Categorical Encodings (integer codes converted to float for tensor)
    pub rice_ecosystem_code: f64,
    pub rice_variety_code: f64,
    pub province_code: f64,
}

impl BiometFeatures {
    /// Serializes features in exact order of `features_order` defined in `model_metadata.json`
    /// as a flat `Vec<f32>` matching ONNX tensor input layout [1, 44].
    pub fn to_tensor_vec(&self) -> Vec<f32> {
        vec![
            self.gdd_cum_7d as f32,
            self.gdd_cum_14d as f32,
            self.gdd_cum_30d as f32,
            self.gdd_cum_60d as f32,
            self.rain_cum_7d as f32,
            self.rain_cum_14d as f32,
            self.rain_cum_30d as f32,
            self.rain_cum_60d as f32,
            self.rain_max_7d as f32,
            self.rain_max_14d as f32,
            self.rain_max_30d as f32,
            self.rain_max_60d as f32,
            self.cdd_7d as f32,
            self.cdd_14d as f32,
            self.cdd_30d as f32,
            self.cdd_60d as f32,
            self.dtr_mean_7d as f32,
            self.dtr_mean_14d as f32,
            self.dtr_mean_30d as f32,
            self.dtr_mean_60d as f32,
            self.dtr_std_7d as f32,
            self.dtr_std_14d as f32,
            self.dtr_std_30d as f32,
            self.dtr_std_60d as f32,
            self.rad_cum_7d as f32,
            self.rad_cum_14d as f32,
            self.rad_cum_30d as f32,
            self.rad_cum_60d as f32,
            self.rh_mean_7d as f32,
            self.rh_mean_14d as f32,
            self.rh_mean_30d as f32,
            self.rh_mean_60d as f32,
            self.month as f32,
            self.day_of_year as f32,
            self.photoperiod_hours as f32,
            self.ptq_30d as f32,
            self.ptq_60d as f32,
            self.vpd_proxy_14d as f32,
            self.vpd_proxy_30d as f32,
            self.lat_clean as f32,
            self.lon_clean as f32,
            self.rice_ecosystem_code as f32,
            self.rice_variety_code as f32,
            self.province_code as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_record(date: NaiveDate) -> DailyWeatherRecord {
        DailyWeatherRecord {
            date,
            t_max: Some(32.5),
            t_min: Some(24.1),
            precipitation_mm: Some(12.4),
            radiation_mj_m2: Some(19.8),
            relative_humidity_pct: Some(78.5),
            t_max_sensor_id: Some("s1".into()),
            t_min_sensor_id: Some("s1".into()),
            rainfall_sensor_id: Some("s2".into()),
            radiation_sensor_id: Some("s1".into()),
            humidity_sensor_id: Some("s1".into()),
        }
    }

    #[test]
    fn test_valid_weather_record() {
        let record = complete_record(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap());
        assert!(record.validate().is_ok());
        assert!(record.is_complete());
        assert!((record.daily_gdd(10.0).unwrap() - 18.3).abs() < 1e-4);
        assert!((record.dtr().unwrap() - 8.4).abs() < 1e-4);
    }

    #[test]
    fn test_weather_record_tmin_exceeds_tmax() {
        let mut record = complete_record(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap());
        record.t_max = Some(20.0);
        record.t_min = Some(25.0);
        assert!(record.validate().is_err());
    }

    #[test]
    fn a_partial_day_with_only_rainfall_reported_is_not_complete_and_has_no_gdd() {
        let record = DailyWeatherRecord {
            date: NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
            t_max: None,
            t_min: None,
            precipitation_mm: Some(5.0),
            radiation_mj_m2: None,
            relative_humidity_pct: None,
            t_max_sensor_id: None,
            t_min_sensor_id: None,
            rainfall_sensor_id: Some("rain-gauge-01".into()),
            radiation_sensor_id: None,
            humidity_sensor_id: None,
        };

        assert!(record.is_partial());
        assert!(record.daily_gdd(10.0).is_none());
        assert!(record.dtr().is_none());
        assert!(record.validate().is_ok(), "partial data must not fail cross-field checks it can't evaluate");
        assert_eq!(
            record.missing_metrics(),
            vec![
                MetricType::TMax,
                MetricType::TMin,
                MetricType::Radiation,
                MetricType::Humidity
            ]
        );
    }

    #[test]
    fn test_tensor_length_equals_44() {
        let dummy = BiometFeatures {
            gdd_cum_7d: 1.0, gdd_cum_14d: 2.0, gdd_cum_30d: 3.0, gdd_cum_60d: 4.0,
            rain_cum_7d: 5.0, rain_cum_14d: 6.0, rain_cum_30d: 7.0, rain_cum_60d: 8.0,
            rain_max_7d: 9.0, rain_max_14d: 10.0, rain_max_30d: 11.0, rain_max_60d: 12.0,
            cdd_7d: 13.0, cdd_14d: 14.0, cdd_30d: 15.0, cdd_60d: 16.0,
            dtr_mean_7d: 17.0, dtr_mean_14d: 18.0, dtr_mean_30d: 19.0, dtr_mean_60d: 20.0,
            dtr_std_7d: 21.0, dtr_std_14d: 22.0, dtr_std_30d: 23.0, dtr_std_60d: 24.0,
            rad_cum_7d: 25.0, rad_cum_14d: 26.0, rad_cum_30d: 27.0, rad_cum_60d: 28.0,
            rh_mean_7d: 29.0, rh_mean_14d: 30.0, rh_mean_30d: 31.0, rh_mean_60d: 32.0,
            month: 7.0, day_of_year: 190.0, photoperiod_hours: 12.6,
            ptq_30d: 1.2, ptq_60d: 1.1, vpd_proxy_14d: 1.5, vpd_proxy_30d: 1.4,
            lat_clean: 14.0, lon_clean: 100.0,
            rice_ecosystem_code: 4.0, rice_variety_code: 73.0, province_code: 6.0,
        };
        let vec = dummy.to_tensor_vec();
        assert_eq!(vec.len(), 44);
    }
}
