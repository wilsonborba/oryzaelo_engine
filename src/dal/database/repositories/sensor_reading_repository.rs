//! # Oryza-Elo Architecture Guardrail: Sensor Reading Repository
//!
//! Full CRUD over the 5 raw per-metric reading tables. `MetricType::table_name()`
//! is a closed, compile-time-known enum match -- never user input -- so
//! selecting the table by string interpolation here is safe.

use crate::core::error::AppError;
use crate::domain::models::metric_type::MetricType;
use crate::domain::models::sensor_reading::SensorReading;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct SensorReadingRepository {
    pool: SqlitePool,
}

impl SensorReadingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Records one raw reading from one sensor. Never touches any other
    /// sensor's data -- this only ever inserts into the one table for
    /// `metric_type`.
    pub async fn insert(
        &self,
        parcel_id: &str,
        sensor_id: &str,
        metric_type: MetricType,
        value: f64,
        recorded_at: DateTime<Utc>,
    ) -> Result<SensorReading, AppError> {
        let table = metric_type.table_name();
        let received_at = Utc::now();

        let query = format!(
            "INSERT INTO {table} (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
        );

        let result = sqlx::query(&query)
            .bind(parcel_id)
            .bind(sensor_id)
            .bind(value)
            .bind(recorded_at.to_rfc3339())
            .bind(received_at.to_rfc3339())
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to insert {} reading: {}", table, e)))?;

        Ok(SensorReading {
            id: result.last_insert_rowid(),
            parcel_id: parcel_id.to_string(),
            sensor_id: sensor_id.to_string(),
            metric_type,
            value,
            recorded_at,
            received_at,
        })
    }

    /// The most recent reading for (parcel, metric_type, calendar date) --
    /// the authoritative value for that day when a sensor reports more than
    /// once (e.g. hourly updates to a running daily rain total).
    pub async fn latest_for_day(
        &self,
        parcel_id: &str,
        metric_type: MetricType,
        date: NaiveDate,
    ) -> Result<Option<SensorReading>, AppError> {
        let table = metric_type.table_name();
        let day_start = format!("{}T00:00:00", date);
        let day_end = format!("{}T23:59:59.999999999", date);

        let query = format!(
            "SELECT id, sensor_id, value, recorded_at, received_at FROM {table}
             WHERE parcel_id = ? AND recorded_at >= ? AND recorded_at <= ?
             ORDER BY recorded_at DESC LIMIT 1"
        );

        let row = sqlx::query(&query)
            .bind(parcel_id)
            .bind(&day_start)
            .bind(&day_end)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to query {} readings: {}", table, e)))?;

        row.map(|r| Self::row_to_reading(r, parcel_id, metric_type)).transpose()
    }

    /// All readings for (parcel, metric_type) across a date range, newest
    /// first -- used by the sensor CRUD screen to show a sensor's own
    /// history, and by data export.
    pub async fn list_for_parcel(
        &self,
        parcel_id: &str,
        metric_type: MetricType,
        limit: i64,
    ) -> Result<Vec<SensorReading>, AppError> {
        let table = metric_type.table_name();
        let query = format!(
            "SELECT id, sensor_id, value, recorded_at, received_at FROM {table}
             WHERE parcel_id = ? ORDER BY recorded_at DESC LIMIT ?"
        );

        let rows = sqlx::query(&query)
            .bind(parcel_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to list {} readings: {}", table, e)))?;

        rows.into_iter()
            .map(|r| Self::row_to_reading(r, parcel_id, metric_type))
            .collect()
    }

    pub async fn get_by_id(&self, metric_type: MetricType, id: i64) -> Result<Option<SensorReading>, AppError> {
        let table = metric_type.table_name();
        let query = format!("SELECT id, parcel_id, sensor_id, value, recorded_at, received_at FROM {table} WHERE id = ?");

        let row = sqlx::query(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to fetch {} reading {}: {}", table, id, e)))?;

        let Some(row) = row else { return Ok(None) };
        let parcel_id: String = row.get("parcel_id");
        Ok(Some(Self::row_to_reading(row, &parcel_id, metric_type)?))
    }

    /// Deletes one reading and returns what it was (parcel/date needed by
    /// the caller to recompute that day's aggregate afterward).
    pub async fn delete(&self, metric_type: MetricType, id: i64) -> Result<bool, AppError> {
        let table = metric_type.table_name();
        let query = format!("DELETE FROM {table} WHERE id = ?");

        let rows = sqlx::query(&query)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to delete {} reading {}: {}", table, id, e)))?
            .rows_affected();

        Ok(rows > 0)
    }

    /// Cascades a weather-record delete across every metric table for the
    /// given (parcel, dates) -- so deleting "a day" actually clears every
    /// sensor's contribution to it, not just the aggregate row.
    pub async fn delete_for_parcel_dates(&self, parcel_id: &str, dates: &[NaiveDate]) -> Result<(), AppError> {
        if dates.is_empty() {
            return Ok(());
        }

        for metric_type in MetricType::all() {
            let table = metric_type.table_name();
            for date in dates {
                let day_start = format!("{}T00:00:00", date);
                let day_end = format!("{}T23:59:59.999999999", date);
                let query = format!("DELETE FROM {table} WHERE parcel_id = ? AND recorded_at >= ? AND recorded_at <= ?");
                sqlx::query(&query)
                    .bind(parcel_id)
                    .bind(&day_start)
                    .bind(&day_end)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| AppError::Database(format!("Failed to cascade-delete {} readings: {}", table, e)))?;
            }
        }

        Ok(())
    }

    fn row_to_reading(
        r: sqlx::sqlite::SqliteRow,
        parcel_id: &str,
        metric_type: MetricType,
    ) -> Result<SensorReading, AppError> {
        let recorded_str: String = r.get("recorded_at");
        let received_str: String = r.get("received_at");
        let recorded_at = DateTime::parse_from_rfc3339(&recorded_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Database(format!("Corrupt recorded_at timestamp: {}", e)))?;
        let received_at = DateTime::parse_from_rfc3339(&received_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(SensorReading {
            id: r.get("id"),
            parcel_id: parcel_id.to_string(),
            sensor_id: r.get("sensor_id"),
            metric_type,
            value: r.get("value"),
            recorded_at,
            received_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dal::database::connection::create_pool;

    async fn setup_with_parcel() -> SqlitePool {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO parcels (id, name, rice_variety, rice_ecosystem, latitude, longitude, planting_date, created_at, updated_at)
             VALUES ('p1', 'Test', 'RD43', 'Irrigated', 14.0, 100.0, '2026-01-01', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn each_metric_type_is_stored_independently_in_its_own_table() {
        let pool = setup_with_parcel().await;
        let repo = SensorReadingRepository::new(pool);
        let now = Utc::now();

        repo.insert("p1", "rain-sensor", MetricType::Rainfall, 5.0, now).await.unwrap();
        repo.insert("p1", "temp-sensor", MetricType::TMax, 32.0, now).await.unwrap();

        // A rainfall reading must never be visible when querying t_max, and vice versa.
        let rain_readings = repo.list_for_parcel("p1", MetricType::Rainfall, 10).await.unwrap();
        let temp_readings = repo.list_for_parcel("p1", MetricType::TMax, 10).await.unwrap();

        assert_eq!(rain_readings.len(), 1);
        assert_eq!(rain_readings[0].sensor_id, "rain-sensor");
        assert_eq!(temp_readings.len(), 1);
        assert_eq!(temp_readings[0].sensor_id, "temp-sensor");
    }

    #[tokio::test]
    async fn latest_reading_for_the_day_wins_when_a_sensor_reports_more_than_once() {
        let pool = setup_with_parcel().await;
        let repo = SensorReadingRepository::new(pool);
        let date = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
        let morning = date.and_hms_opt(6, 0, 0).unwrap().and_utc();
        let evening = date.and_hms_opt(18, 0, 0).unwrap().and_utc();

        repo.insert("p1", "rain-sensor", MetricType::Rainfall, 2.0, morning).await.unwrap();
        repo.insert("p1", "rain-sensor", MetricType::Rainfall, 7.5, evening).await.unwrap();

        let latest = repo.latest_for_day("p1", MetricType::Rainfall, date).await.unwrap().unwrap();
        assert_eq!(latest.value, 7.5);
    }

    #[tokio::test]
    async fn deleting_a_reading_never_touches_a_different_metric_type() {
        let pool = setup_with_parcel().await;
        let repo = SensorReadingRepository::new(pool);
        let now = Utc::now();

        let rain = repo.insert("p1", "rain-sensor", MetricType::Rainfall, 5.0, now).await.unwrap();
        repo.insert("p1", "temp-sensor", MetricType::TMax, 32.0, now).await.unwrap();

        assert!(repo.delete(MetricType::Rainfall, rain.id).await.unwrap());
        assert_eq!(repo.list_for_parcel("p1", MetricType::Rainfall, 10).await.unwrap().len(), 0);
        assert_eq!(repo.list_for_parcel("p1", MetricType::TMax, 10).await.unwrap().len(), 1);
    }
}
