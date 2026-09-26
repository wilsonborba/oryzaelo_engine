//! # Oryza-Elo Architecture Guardrail: Sensor Reading Domain Model
//!
//! A single raw measurement from one physical sensor, at whatever cadence
//! that sensor actually reports (once a day, once an hour, irregular pushes
//! from a LoRaWAN gateway...). Never merged or overwritten by another
//! sensor's data -- each metric type lives in its own table
//! (`MetricType::table_name`), so provenance is never ambiguous.

use super::metric_type::MetricType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensorReading {
    pub id: i64,
    pub parcel_id: String,
    pub sensor_id: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub recorded_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
}
