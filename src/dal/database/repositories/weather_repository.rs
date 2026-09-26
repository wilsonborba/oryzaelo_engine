//! # Oryza-Elo Architecture Guardrail: Weather Repository
//!
//! Persistence for the daily aggregate view. `weather_records` is never
//! written to directly with a batch of fields any more -- it's derived from
//! `sensor_readings_*` via `SensorReadingRepository`, one metric field at a
//! time, so one sensor reporting late can never clobber another sensor's
//! already-stored value for the same day.

use crate::core::error::AppError;
use crate::dal::database::repositories::sensor_reading_repository::SensorReadingRepository;
use crate::domain::models::metric_type::MetricType;
use crate::domain::models::weather::DailyWeatherRecord;
use chrono::{NaiveDate, Utc};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct WeatherRepository {
    pool: SqlitePool,
    readings: SensorReadingRepository,
}

impl WeatherRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            readings: SensorReadingRepository::new(pool.clone()),
            pool,
        }
    }

    /// Records one sensor's reading for one metric, then recomputes the
    /// daily aggregate row for that (parcel, date) from whatever the 5
    /// reading tables now say -- every other field on the aggregate is left
    /// exactly as it was.
    pub async fn record_reading(
        &self,
        parcel_id: &str,
        sensor_id: &str,
        metric_type: MetricType,
        value: f64,
        recorded_at: chrono::DateTime<Utc>,
    ) -> Result<DailyWeatherRecord, AppError> {
        self.readings
            .insert(parcel_id, sensor_id, metric_type, value, recorded_at)
            .await?;

        self.recompute_day(parcel_id, recorded_at.date_naive()).await
    }

    /// Recomputes the aggregate row for (parcel, date) from scratch: queries
    /// the latest reading of each of the 5 metric types independently (each
    /// from its own table, so one sensor's insert or delete never touches
    /// what another sensor already reported), and writes exactly that state
    /// -- including writing a field back to `NULL` if its last reading was
    /// just deleted.
    pub async fn recompute_day(&self, parcel_id: &str, date: NaiveDate) -> Result<DailyWeatherRecord, AppError> {
        let t_max = self.readings.latest_for_day(parcel_id, MetricType::TMax, date).await?;
        let t_min = self.readings.latest_for_day(parcel_id, MetricType::TMin, date).await?;
        let rain = self.readings.latest_for_day(parcel_id, MetricType::Rainfall, date).await?;
        let rad = self.readings.latest_for_day(parcel_id, MetricType::Radiation, date).await?;
        let rh = self.readings.latest_for_day(parcel_id, MetricType::Humidity, date).await?;

        let date_str = date.to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO weather_records (
                parcel_id, record_date,
                t_max, t_min, precipitation_mm, radiation_mj_m2, relative_humidity_pct,
                t_max_sensor_id, t_min_sensor_id, rainfall_sensor_id, radiation_sensor_id, humidity_sensor_id,
                is_partial, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
            ON CONFLICT(parcel_id, record_date) DO UPDATE SET
                t_max = excluded.t_max,
                t_min = excluded.t_min,
                precipitation_mm = excluded.precipitation_mm,
                radiation_mj_m2 = excluded.radiation_mj_m2,
                relative_humidity_pct = excluded.relative_humidity_pct,
                t_max_sensor_id = excluded.t_max_sensor_id,
                t_min_sensor_id = excluded.t_min_sensor_id,
                rainfall_sensor_id = excluded.rainfall_sensor_id,
                radiation_sensor_id = excluded.radiation_sensor_id,
                humidity_sensor_id = excluded.humidity_sensor_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(parcel_id)
        .bind(&date_str)
        .bind(t_max.as_ref().map(|r| r.value))
        .bind(t_min.as_ref().map(|r| r.value))
        .bind(rain.as_ref().map(|r| r.value))
        .bind(rad.as_ref().map(|r| r.value))
        .bind(rh.as_ref().map(|r| r.value))
        .bind(t_max.as_ref().map(|r| r.sensor_id.clone()))
        .bind(t_min.as_ref().map(|r| r.sensor_id.clone()))
        .bind(rain.as_ref().map(|r| r.sensor_id.clone()))
        .bind(rad.as_ref().map(|r| r.sensor_id.clone()))
        .bind(rh.as_ref().map(|r| r.sensor_id.clone()))
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to recompute weather aggregate for {}: {}", date, e)))?;

        // is_partial is derived from the actual field values, computed here
        // in Rust rather than duplicated as SQL logic.
        let record = self
            .get_one(parcel_id, date)
            .await?
            .expect("row was just upserted above");
        let is_partial = record.is_partial();
        sqlx::query("UPDATE weather_records SET is_partial = ? WHERE parcel_id = ? AND record_date = ?")
            .bind(if is_partial { 1 } else { 0 })
            .bind(parcel_id)
            .bind(&date_str)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to update is_partial flag: {}", e)))?;

        Ok(record)
    }

    pub async fn get_one(&self, parcel_id: &str, date: NaiveDate) -> Result<Option<DailyWeatherRecord>, AppError> {
        let row = sqlx::query(
            "SELECT record_date, t_max, t_min, precipitation_mm, radiation_mj_m2, relative_humidity_pct,
                    t_max_sensor_id, t_min_sensor_id, rainfall_sensor_id, radiation_sensor_id, humidity_sensor_id
             FROM weather_records WHERE parcel_id = ? AND record_date = ?",
        )
        .bind(parcel_id)
        .bind(date.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to fetch weather record: {}", e)))?;

        row.map(Self::row_to_record).transpose()
    }

    /// Retrieves up to `limit_days` most recent aggregate rows up to and
    /// including `up_to_date`, ascending chronologically.
    pub async fn get_retrospective(
        &self,
        parcel_id: &str,
        up_to_date: NaiveDate,
        limit_days: usize,
    ) -> Result<Vec<DailyWeatherRecord>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT record_date, t_max, t_min, precipitation_mm, radiation_mj_m2, relative_humidity_pct,
                   t_max_sensor_id, t_min_sensor_id, rainfall_sensor_id, radiation_sensor_id, humidity_sensor_id
            FROM weather_records
            WHERE parcel_id = ? AND record_date <= ?
            ORDER BY record_date DESC
            LIMIT ?
            "#,
        )
        .bind(parcel_id)
        .bind(up_to_date.to_string())
        .bind(limit_days as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to query weather for parcel {}: {}", parcel_id, e)))?;

        let mut records: Vec<DailyWeatherRecord> = rows.into_iter().map(Self::row_to_record).collect::<Result<_, _>>()?;

        // Reverse to return strictly chronological ascending order
        records.reverse();
        Ok(records)
    }

    /// Deletes specific daily records: cascades across every per-metric
    /// reading table so no orphaned sensor data survives, then drops the
    /// aggregate rows themselves.
    pub async fn delete_records(&self, parcel_id: &str, dates: &[NaiveDate]) -> Result<usize, AppError> {
        if dates.is_empty() {
            return Ok(0);
        }

        self.readings.delete_for_parcel_dates(parcel_id, dates).await?;

        let mut deleted = 0;
        for date in dates {
            let rows = sqlx::query("DELETE FROM weather_records WHERE parcel_id = ? AND record_date = ?")
                .bind(parcel_id)
                .bind(date.to_string())
                .execute(&self.pool)
                .await
                .map_err(|e| AppError::Database(format!("Failed to delete weather record on {}: {}", date, e)))?
                .rows_affected();
            deleted += rows as usize;
        }

        Ok(deleted)
    }

    fn row_to_record(r: sqlx::sqlite::SqliteRow) -> Result<DailyWeatherRecord, AppError> {
        let date_str: String = r.get("record_date");
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| AppError::Database(format!("Invalid date in db: {}", e)))?;

        Ok(DailyWeatherRecord {
            date,
            t_max: r.get("t_max"),
            t_min: r.get("t_min"),
            precipitation_mm: r.get("precipitation_mm"),
            radiation_mj_m2: r.get("radiation_mj_m2"),
            relative_humidity_pct: r.get("relative_humidity_pct"),
            t_max_sensor_id: r.get("t_max_sensor_id"),
            t_min_sensor_id: r.get("t_min_sensor_id"),
            rainfall_sensor_id: r.get("rainfall_sensor_id"),
            radiation_sensor_id: r.get("radiation_sensor_id"),
            humidity_sensor_id: r.get("humidity_sensor_id"),
        })
    }
}

/// Count total records stored for a parcel (used by admin/health summaries).
pub async fn count_for_parcel(pool: &SqlitePool, parcel_id: &str) -> Result<usize, AppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM weather_records WHERE parcel_id = ?")
        .bind(parcel_id)
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(row.0 as usize)
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
    async fn two_different_sensors_reporting_different_metrics_never_clobber_each_other() {
        let pool = setup_with_parcel().await;
        let repo = WeatherRepository::new(pool);
        let date = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
        let ts = date.and_hms_opt(12, 0, 0).unwrap().and_utc();

        // Rain sensor reports first.
        let after_rain = repo
            .record_reading("p1", "rain-gauge-01", MetricType::Rainfall, 12.5, ts)
            .await
            .unwrap();
        assert_eq!(after_rain.precipitation_mm, Some(12.5));
        assert!(after_rain.is_partial(), "temperature hasn't arrived yet");

        // A completely separate temperature sensor reports later for the same day.
        let after_temp = repo
            .record_reading("p1", "temp-sensor-02", MetricType::TMax, 33.0, ts)
            .await
            .unwrap();

        // The rain value from the FIRST sensor must still be there, untouched.
        assert_eq!(after_temp.precipitation_mm, Some(12.5));
        assert_eq!(after_temp.rainfall_sensor_id.as_deref(), Some("rain-gauge-01"));
        assert_eq!(after_temp.t_max, Some(33.0));
        assert_eq!(after_temp.t_max_sensor_id.as_deref(), Some("temp-sensor-02"));
    }

    #[tokio::test]
    async fn a_day_becomes_complete_only_once_every_metric_has_reported() {
        let pool = setup_with_parcel().await;
        let repo = WeatherRepository::new(pool);
        let date = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
        let ts = date.and_hms_opt(12, 0, 0).unwrap().and_utc();

        repo.record_reading("p1", "s1", MetricType::TMax, 33.0, ts).await.unwrap();
        repo.record_reading("p1", "s1", MetricType::TMin, 23.0, ts).await.unwrap();
        repo.record_reading("p1", "s1", MetricType::Rainfall, 0.0, ts).await.unwrap();
        repo.record_reading("p1", "s1", MetricType::Radiation, 18.0, ts).await.unwrap();
        let final_state = repo
            .record_reading("p1", "s1", MetricType::Humidity, 75.0, ts)
            .await
            .unwrap();

        assert!(final_state.is_complete());
        assert!(final_state.daily_gdd(10.0).is_some());
    }

    #[tokio::test]
    async fn recompute_after_deleting_the_only_reading_nulls_the_field_back_out() {
        let pool = setup_with_parcel().await;
        let readings = SensorReadingRepository::new(pool.clone());
        let repo = WeatherRepository::new(pool);
        let date = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
        let ts = date.and_hms_opt(12, 0, 0).unwrap().and_utc();

        let reading = readings.insert("p1", "rain-sensor", MetricType::Rainfall, 12.5, ts).await.unwrap();
        let after_insert = repo.recompute_day("p1", date).await.unwrap();
        assert_eq!(after_insert.precipitation_mm, Some(12.5));

        readings.delete(MetricType::Rainfall, reading.id).await.unwrap();
        let after_delete = repo.recompute_day("p1", date).await.unwrap();

        assert_eq!(after_delete.precipitation_mm, None, "stale value must not survive its only reading being deleted");
        assert_eq!(after_delete.rainfall_sensor_id, None);
    }

    #[tokio::test]
    async fn deleting_a_day_removes_the_underlying_readings_too() {
        let pool = setup_with_parcel().await;
        let readings = SensorReadingRepository::new(pool.clone());
        let repo = WeatherRepository::new(pool);
        let date = NaiveDate::from_ymd_opt(2026, 6, 6).unwrap();
        let ts = date.and_hms_opt(12, 0, 0).unwrap().and_utc();

        repo.record_reading("p1", "s1", MetricType::Rainfall, 5.0, ts).await.unwrap();
        repo.delete_records("p1", &[date]).await.unwrap();

        assert!(repo.get_one("p1", date).await.unwrap().is_none());
        assert_eq!(readings.list_for_parcel("p1", MetricType::Rainfall, 10).await.unwrap().len(), 0);
    }
}
