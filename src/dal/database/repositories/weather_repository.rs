//! # Oryza-Elo Architecture Guardrail: Weather Repository
//!
//! Persistence operations for daily agrometeorological time series in SQLite.

use crate::core::error::AppError;
use crate::domain::models::weather::DailyWeatherRecord;
use chrono::{NaiveDate, Utc};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct WeatherRepository {
    pool: SqlitePool,
}

impl WeatherRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts a batch of daily weather records using SQLite transaction and UPSERT.
    pub async fn insert_batch(
        &self,
        parcel_id: &str,
        records: &[DailyWeatherRecord],
    ) -> Result<usize, AppError> {
        if records.is_empty() {
            return Ok(0);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Database(format!("Failed to begin transaction: {}", e)))?;

        let now = Utc::now().to_rfc3339();
        let mut inserted_count = 0;

        for rec in records {
            rec.validate().map_err(AppError::Domain)?;

            let rows_affected = sqlx::query(
                r#"
                INSERT INTO weather_records (
                    parcel_id, record_date, t_max, t_min,
                    precipitation_mm, radiation_mj_m2, relative_humidity_pct,
                    source, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(parcel_id, record_date) DO UPDATE SET
                    t_max = excluded.t_max,
                    t_min = excluded.t_min,
                    precipitation_mm = excluded.precipitation_mm,
                    radiation_mj_m2 = excluded.radiation_mj_m2,
                    relative_humidity_pct = excluded.relative_humidity_pct,
                    source = excluded.source,
                    created_at = excluded.created_at
                "#,
            )
            .bind(parcel_id)
            .bind(rec.date.to_string())
            .bind(rec.t_max)
            .bind(rec.t_min)
            .bind(rec.precipitation_mm)
            .bind(rec.radiation_mj_m2)
            .bind(rec.relative_humidity_pct)
            .bind(&rec.source)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Database(format!("Failed to insert weather record on {}: {}", rec.date, e)))?
            .rows_affected();

            if rows_affected > 0 {
                inserted_count += 1;
            }
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Database(format!("Failed to commit weather transaction: {}", e)))?;

        Ok(inserted_count)
    }

    /// Retrieves up to `limit_days` retrospective records ending on `up_to_date` (t <= up_to_date),
    /// returned in chronological ascending order.
    pub async fn get_retrospective(
        &self,
        parcel_id: &str,
        up_to_date: NaiveDate,
        limit_days: usize,
    ) -> Result<Vec<DailyWeatherRecord>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT record_date, t_max, t_min, precipitation_mm, radiation_mj_m2, relative_humidity_pct, source
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

        let mut records = Vec::with_capacity(rows.len());
        for r in rows {
            let d_str: String = r.get("record_date");
            let date = NaiveDate::parse_from_str(&d_str, "%Y-%m-%d")
                .map_err(|e| AppError::Database(format!("Invalid date in db: {}", e)))?;

            records.push(DailyWeatherRecord {
                date,
                t_max: r.get("t_max"),
                t_min: r.get("t_min"),
                precipitation_mm: r.get("precipitation_mm"),
                radiation_mj_m2: r.get("radiation_mj_m2"),
                relative_humidity_pct: r.get("relative_humidity_pct"),
                source: r.get("source"),
            });
        }

        // Reverse to return strictly chronological ascending order
        records.reverse();
        Ok(records)
    }

    /// Count total records stored for a parcel.
    pub async fn count_for_parcel(&self, parcel_id: &str) -> Result<usize, AppError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM weather_records WHERE parcel_id = ?")
            .bind(parcel_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to count weather records: {}", e)))?;

        Ok(row.0 as usize)
    }

    /// Deletes specific records for a parcel by dates.
    pub async fn delete_records(&self, parcel_id: &str, dates: &[NaiveDate]) -> Result<usize, AppError> {
        if dates.is_empty() {
            return Ok(0);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Database(format!("Failed to begin delete transaction: {}", e)))?;

        let mut total_deleted = 0;
        for date in dates {
            let deleted = sqlx::query("DELETE FROM weather_records WHERE parcel_id = ? AND record_date = ?")
                .bind(parcel_id)
                .bind(date.to_string())
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::Database(format!("Failed to delete record on {}: {}", date, e)))?
                .rows_affected();

            total_deleted += deleted as usize;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Database(format!("Failed to commit delete transaction: {}", e)))?;

        Ok(total_deleted)
    }
}

