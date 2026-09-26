//! # Oryza-Elo Architecture Guardrail: Sensor Profile Repository
//!
//! Persistence operations for registered sensor profiles and factory presets.

use crate::core::error::AppError;
use crate::domain::models::device_mapping::DeviceMapping;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct DeviceMappingRepository {
    pool: SqlitePool,
}

impl DeviceMappingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn save(&self, m: &DeviceMapping) -> Result<(), AppError> {
        let metrics_json = serde_json::to_string(&m.metrics)
            .map_err(|e| AppError::Database(format!("Failed to serialize sensor metrics: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO device_mappings (
                id, device_name, manufacturer, is_preset,
                date_col, date_format, metrics_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                device_name = excluded.device_name,
                manufacturer = excluded.manufacturer,
                is_preset = excluded.is_preset,
                date_col = excluded.date_col,
                date_format = excluded.date_format,
                metrics_json = excluded.metrics_json
            "#,
        )
        .bind(&m.id)
        .bind(&m.device_name)
        .bind(&m.manufacturer)
        .bind(if m.is_preset { 1 } else { 0 })
        .bind(&m.date_col)
        .bind(&m.date_format)
        .bind(&metrics_json)
        .bind(m.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to save sensor profile {}: {}", m.id, e)))?;

        Ok(())
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<DeviceMapping>, AppError> {
        let row = sqlx::query(
            "SELECT id, device_name, manufacturer, is_preset, date_col, date_format, metrics_json, created_at
             FROM device_mappings WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to fetch sensor profile {}: {}", id, e)))?;

        row.map(Self::row_to_mapping).transpose()
    }

    pub async fn list_all(&self) -> Result<Vec<DeviceMapping>, AppError> {
        let rows = sqlx::query(
            "SELECT id, device_name, manufacturer, is_preset, date_col, date_format, metrics_json, created_at
             FROM device_mappings ORDER BY is_preset DESC, device_name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to list sensor profiles: {}", e)))?;

        rows.into_iter().map(Self::row_to_mapping).collect()
    }

    pub async fn list_presets(&self) -> Result<Vec<DeviceMapping>, AppError> {
        let rows = sqlx::query(
            "SELECT id, device_name, manufacturer, is_preset, date_col, date_format, metrics_json, created_at
             FROM device_mappings WHERE is_preset = 1 ORDER BY device_name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to list presets: {}", e)))?;

        rows.into_iter().map(Self::row_to_mapping).collect()
    }

    pub async fn delete(&self, id: &str) -> Result<bool, AppError> {
        // Prevent deletion of factory presets
        let mapping = self.get_by_id(id).await?;
        if let Some(m) = mapping {
            if m.is_preset {
                return Err(AppError::BadRequest(format!(
                    "Cannot delete factory preset sensor profile: {}",
                    id
                )));
            }
        } else {
            return Ok(false);
        }

        let rows = sqlx::query("DELETE FROM device_mappings WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to delete sensor profile {}: {}", id, e)))?
            .rows_affected();

        Ok(rows > 0)
    }

    fn row_to_mapping(r: sqlx::sqlite::SqliteRow) -> Result<DeviceMapping, AppError> {
        let is_preset_int: i64 = r.get("is_preset");
        let created_str: String = r.get("created_at");
        let created_at = DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let metrics_json: String = r.get("metrics_json");
        let metrics = serde_json::from_str(&metrics_json)
            .map_err(|e| AppError::Database(format!("Corrupt metrics_json for sensor profile: {}", e)))?;

        Ok(DeviceMapping {
            id: r.get("id"),
            device_name: r.get("device_name"),
            manufacturer: r.get("manufacturer"),
            is_preset: is_preset_int == 1,
            date_col: r.get("date_col"),
            date_format: r.get("date_format"),
            metrics,
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dal::database::connection::create_pool;
    use crate::domain::models::metric_type::MetricType;

    #[tokio::test]
    async fn saves_and_reloads_a_single_metric_sensor_profile_round_trip() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let repo = DeviceMappingRepository::new(pool);

        let rain_gauge = DeviceMapping::single_metric(
            "sensor-rain-01",
            "Standalone Rain Gauge",
            "Generic",
            "ts",
            "%Y-%m-%d",
            MetricType::Rainfall,
            "rain_mm",
            "mm",
            1.0,
        );
        repo.save(&rain_gauge).await.unwrap();

        let loaded = repo.get_by_id("sensor-rain-01").await.unwrap().unwrap();
        assert_eq!(loaded.metric_types(), vec![MetricType::Rainfall]);
        assert_eq!(loaded.metric(MetricType::Rainfall).unwrap().column_name, "rain_mm");
    }
}
