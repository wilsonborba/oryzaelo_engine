//! # Oryza-Elo Architecture Guardrail: Canonical Metric Type
//!
//! The 5 canonical agrometeorological variables the phenology model consumes.
//! Every sensor, reading, and column mapping in the system is tagged with
//! exactly one of these -- there is no generic/untyped metric.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    TMax,
    TMin,
    Rainfall,
    Radiation,
    Humidity,
}

impl MetricType {
    pub fn all() -> [MetricType; 5] {
        [
            MetricType::TMax,
            MetricType::TMin,
            MetricType::Rainfall,
            MetricType::Radiation,
            MetricType::Humidity,
        ]
    }

    /// Stable wire/DB identifier, also used to select the correct
    /// `sensor_readings_*` physical table (whitelisted, never built from
    /// arbitrary user input).
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::TMax => "t_max",
            MetricType::TMin => "t_min",
            MetricType::Rainfall => "rainfall",
            MetricType::Radiation => "radiation",
            MetricType::Humidity => "humidity",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<MetricType> {
        match s {
            "t_max" => Some(MetricType::TMax),
            "t_min" => Some(MetricType::TMin),
            "rainfall" => Some(MetricType::Rainfall),
            "radiation" => Some(MetricType::Radiation),
            "humidity" => Some(MetricType::Humidity),
            _ => None,
        }
    }

    /// The reading table this metric type is physically stored in.
    pub fn table_name(&self) -> &'static str {
        match self {
            MetricType::TMax => "sensor_readings_t_max",
            MetricType::TMin => "sensor_readings_t_min",
            MetricType::Rainfall => "sensor_readings_rainfall",
            MetricType::Radiation => "sensor_readings_radiation",
            MetricType::Humidity => "sensor_readings_humidity",
        }
    }

    /// Canonical storage unit after conversion (what `DailyWeatherRecord` expects).
    pub fn canonical_unit(&self) -> &'static str {
        match self {
            MetricType::TMax | MetricType::TMin => "C",
            MetricType::Rainfall => "mm",
            MetricType::Radiation => "MJ/m2",
            MetricType::Humidity => "%",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_str() {
        for m in MetricType::all() {
            assert_eq!(MetricType::from_str_opt(m.as_str()), Some(m));
        }
    }

    #[test]
    fn unknown_string_yields_none() {
        assert_eq!(MetricType::from_str_opt("soil_moisture"), None);
    }
}
