//! # Oryza-Elo Architecture Guardrail: Sensor Profile Domain Model
//!
//! A registered sensor's column mapping: vendor-specific column names,
//! physical units, and scale multipliers, per canonical metric it actually
//! measures. A profile declares 1..5 metrics -- a standalone rain gauge
//! declares exactly `Rainfall`; a bundled multi-sensor weather station
//! (Davis Vantage Pro2, Pessl iMetos) declares all 5, matching how these
//! real commercial products actually report (one combined feed). The schema
//! never forces a sensor to claim a metric it doesn't physically measure.

use super::metric_type::MetricType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Column specification, unit, and scale multiplier for one metric this
/// sensor profile reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricColumnMapping {
    pub metric_type: MetricType,
    pub column_name: String,
    pub unit: String,
    pub scale: f64,
}

impl MetricColumnMapping {
    pub fn new(metric_type: MetricType, column_name: impl Into<String>, unit: impl Into<String>, scale: f64) -> Self {
        Self {
            metric_type,
            column_name: column_name.into(),
            unit: unit.into(),
            scale,
        }
    }
}

/// Sensor profile: a physical device (or manual/synthetic source) mapping
/// its own raw column(s) onto one or more canonical metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceMapping {
    pub id: String,
    pub device_name: String,
    pub manufacturer: String,
    pub is_preset: bool,

    pub date_col: String,
    pub date_format: String,

    /// 1..5 entries -- never assumes a sensor reports every canonical metric.
    pub metrics: Vec<MetricColumnMapping>,

    pub created_at: DateTime<Utc>,
}

impl DeviceMapping {
    pub fn metric(&self, metric_type: MetricType) -> Option<&MetricColumnMapping> {
        self.metrics.iter().find(|m| m.metric_type == metric_type)
    }

    pub fn metric_types(&self) -> Vec<MetricType> {
        self.metrics.iter().map(|m| m.metric_type).collect()
    }

    /// Converts a raw measurement for `metric_type` into its canonical unit
    /// (°C, mm, MJ/m², or %). Returns the raw*scale value unconverted if the
    /// unit string isn't a recognized alternate (already-canonical case).
    pub fn convert(&self, metric_type: MetricType, raw: f64) -> Option<f64> {
        let m = self.metric(metric_type)?;
        let scaled = raw * m.scale;
        let unit = m.unit.trim().to_lowercase();

        Some(match metric_type {
            MetricType::TMax | MetricType::TMin => match unit.as_str() {
                "f" | "fahrenheit" | "deg_f" => (scaled - 32.0) * 5.0 / 9.0,
                _ => scaled,
            },
            MetricType::Rainfall => match unit.as_str() {
                "in" | "inch" | "inches" => scaled * 25.4,
                _ => scaled,
            },
            MetricType::Radiation => match unit.as_str() {
                // Instantaneous or daily mean W/m² integrated over 24h (86,400s / 1e6 = 0.0864 MJ/m²)
                "w/m2" | "w/m^2" | "watt/m2" => scaled * 0.0864,
                // Kilowatt-hours per square meter (1 kWh = 3.6 MJ)
                "kwh/m2" | "kwh/m^2" => scaled * 3.6,
                _ => scaled,
            },
            MetricType::Humidity => scaled,
        })
    }

    /// Convenience constructor for the common real-world case: a standalone
    /// sensor that only measures one metric (a rain gauge, a pyranometer...).
    pub fn single_metric(
        id: impl Into<String>,
        device_name: impl Into<String>,
        manufacturer: impl Into<String>,
        date_col: impl Into<String>,
        date_format: impl Into<String>,
        metric_type: MetricType,
        column_name: impl Into<String>,
        unit: impl Into<String>,
        scale: f64,
    ) -> Self {
        Self {
            id: id.into(),
            device_name: device_name.into(),
            manufacturer: manufacturer.into(),
            is_preset: false,
            date_col: date_col.into(),
            date_format: date_format.into(),
            metrics: vec![MetricColumnMapping::new(metric_type, column_name, unit, scale)],
            created_at: Utc::now(),
        }
    }

    /// Factory preset: Pessl Instruments (iMetos) - Austria / Germany
    pub fn pessl_preset() -> Self {
        Self {
            id: "preset_pessl_imetos".into(),
            device_name: "Pessl iMetos 3.3 (Standard Agro)".into(),
            manufacturer: "Pessl Instruments (Austria/Germany)".into(),
            is_preset: true,
            date_col: "Timestamp".into(),
            date_format: "%Y-%m-%d".into(),
            metrics: vec![
                MetricColumnMapping::new(MetricType::TMax, "AirTemp_Max", "C", 1.0),
                MetricColumnMapping::new(MetricType::TMin, "AirTemp_Min", "C", 1.0),
                MetricColumnMapping::new(MetricType::Rainfall, "Precipitation", "mm", 1.0),
                MetricColumnMapping::new(MetricType::Radiation, "SolarRad", "W/m2", 1.0),
                MetricColumnMapping::new(MetricType::Humidity, "RelHumidity", "%", 1.0),
            ],
            created_at: Utc::now(),
        }
    }

    /// Factory preset: Dragino / Renke RS485 Modbus Telemetry - China
    pub fn dragino_renke_preset() -> Self {
        Self {
            id: "preset_dragino_renke".into(),
            device_name: "Dragino / Renke RS485 Modbus Telemetry".into(),
            manufacturer: "Dragino / Renke (China)".into(),
            is_preset: true,
            date_col: "date".into(),
            date_format: "%Y-%m-%d".into(),
            metrics: vec![
                MetricColumnMapping::new(MetricType::TMax, "temp_max_raw", "C", 0.1),
                MetricColumnMapping::new(MetricType::TMin, "temp_min_raw", "C", 0.1),
                MetricColumnMapping::new(MetricType::Rainfall, "pulse_count", "mm", 0.2),
                MetricColumnMapping::new(MetricType::Radiation, "radiation_raw", "MJ/m2", 0.1),
                MetricColumnMapping::new(MetricType::Humidity, "humidity_raw", "%", 0.1),
            ],
            created_at: Utc::now(),
        }
    }

    /// Factory preset: Davis Instruments (WeatherLink) - USA
    pub fn davis_preset() -> Self {
        Self {
            id: "preset_davis_vantage".into(),
            device_name: "Davis Vantage Pro2 (WeatherLink CSV)".into(),
            manufacturer: "Davis Instruments (USA)".into(),
            is_preset: true,
            date_col: "Date".into(),
            date_format: "%m/%d/%Y".into(),
            metrics: vec![
                MetricColumnMapping::new(MetricType::TMax, "Temp High", "F", 1.0),
                MetricColumnMapping::new(MetricType::TMin, "Temp Low", "F", 1.0),
                MetricColumnMapping::new(MetricType::Rainfall, "Rain", "in", 1.0),
                MetricColumnMapping::new(MetricType::Radiation, "Solar Rad", "MJ/m2", 1.0),
                MetricColumnMapping::new(MetricType::Humidity, "Hum High", "%", 1.0),
            ],
            created_at: Utc::now(),
        }
    }

    /// Factory preset: NASA POWER Daily Agrometeorological CSV
    pub fn nasa_power_preset() -> Self {
        Self {
            id: "preset_nasa_power".into(),
            device_name: "NASA POWER Daily Point CSV".into(),
            manufacturer: "NASA Langley Research Center".into(),
            is_preset: true,
            date_col: "YEARMODA".into(),
            date_format: "%Y%m%d".into(),
            metrics: vec![
                MetricColumnMapping::new(MetricType::TMax, "T2M_MAX", "C", 1.0),
                MetricColumnMapping::new(MetricType::TMin, "T2M_MIN", "C", 1.0),
                MetricColumnMapping::new(MetricType::Rainfall, "PRECTOTCORR", "mm", 1.0),
                MetricColumnMapping::new(MetricType::Radiation, "ALLSKY_SFC_SW_DWN", "MJ/m2", 1.0),
                MetricColumnMapping::new(MetricType::Humidity, "RH2M", "%", 1.0),
            ],
            created_at: Utc::now(),
        }
    }

    /// Returns the collection of all 4 official factory presets.
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::pessl_preset(),
            Self::dragino_renke_preset(),
            Self::davis_preset(),
            Self::nasa_power_preset(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_davis_unit_conversions() {
        let davis = DeviceMapping::davis_preset();

        // 86°F -> 30°C
        let t_c = davis.convert(MetricType::TMax, 86.0).unwrap();
        assert!((t_c - 30.0).abs() < 1e-4);

        // 1.0 inch -> 25.4 mm
        let rain_mm = davis.convert(MetricType::Rainfall, 1.0).unwrap();
        assert!((rain_mm - 25.4).abs() < 1e-4);
    }

    #[test]
    fn test_dragino_renke_scale_conversions() {
        let dragino = DeviceMapping::dragino_renke_preset();

        // Raw 325 -> 32.5°C
        let t_max = dragino.convert(MetricType::TMax, 325.0).unwrap();
        assert!((t_max - 32.5).abs() < 1e-4);

        // 15 pulses * 0.2 = 3.0 mm
        let rain = dragino.convert(MetricType::Rainfall, 15.0).unwrap();
        assert!((rain - 3.0).abs() < 1e-4);

        // Raw 815 -> 81.5%
        let rh = dragino.convert(MetricType::Humidity, 815.0).unwrap();
        assert!((rh - 81.5).abs() < 1e-4);
    }

    #[test]
    fn a_standalone_single_metric_sensor_only_declares_the_one_metric_it_measures() {
        let rain_gauge = DeviceMapping::single_metric(
            "sensor-rain-01",
            "Standalone Tipping Bucket Rain Gauge",
            "Generic LoRaWAN",
            "ts",
            "%Y-%m-%d",
            MetricType::Rainfall,
            "rain_mm",
            "mm",
            1.0,
        );

        assert_eq!(rain_gauge.metric_types(), vec![MetricType::Rainfall]);
        assert!(rain_gauge.metric(MetricType::TMax).is_none());
        assert!(rain_gauge.convert(MetricType::TMax, 10.0).is_none());
    }
}
