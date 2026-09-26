//! # Oryza-Elo Architecture Guardrail: Database Connection Pool
//!
//! SQLite connection pool manager strictly confined to `src/dal/data/local/oryza_elo_edge.db`.

use crate::core::error::AppError;
use crate::domain::models::device_mapping::DeviceMapping;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Executor, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

/// Standard relative path to edge SQLite database.
pub const DEFAULT_DB_REL_PATH: &str = "src/dal/data/local/oryza_elo_edge.db";

/// Embedded SQL migration script
pub const MIGRATION_SQL: &str = include_str!("migrations/001_initial_schema.sql");

/// Resolves the absolute database path for local edge storage.
pub fn get_default_db_path() -> PathBuf {
    PathBuf::from(DEFAULT_DB_REL_PATH)
}

/// Initializes an SQLite connection pool and ensures schema & presets are up to date.
pub async fn create_pool(db_url_or_path: &str) -> Result<SqlitePool, AppError> {
    let is_memory = db_url_or_path.contains(":memory:");

    let options = if is_memory {
        SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|e| AppError::Database(e.to_string()))?
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
    } else {
        let clean_path = db_url_or_path
            .trim_start_matches("sqlite://")
            .split('?')
            .next()
            .unwrap_or(db_url_or_path);

        let path = Path::new(clean_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(if is_memory { 1 } else { 5 })
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|e| AppError::Database(format!("Failed to connect to SQLite: {}", e)))?;

    // Set Pragmas for performance and resilience
    if !is_memory {
        pool.execute("PRAGMA journal_mode = WAL;").await.map_err(|e| AppError::Database(e.to_string()))?;
        pool.execute("PRAGMA synchronous = NORMAL;").await.map_err(|e| AppError::Database(e.to_string()))?;
    }

    // Run migrations
    pool.execute(MIGRATION_SQL)
        .await
        .map_err(|e| AppError::Database(format!("Failed to execute migrations: {}", e)))?;

    // Seed factory presets
    seed_factory_presets(&pool).await?;

    Ok(pool)
}

/// Initializes the default edge production database pool at `src/dal/data/local/oryza_elo_edge.db`.
pub async fn init_default_pool() -> Result<SqlitePool, AppError> {
    create_pool(DEFAULT_DB_REL_PATH).await
}

/// Seeds official vendor presets (Pessl, Dragino/Renke, Davis, NASA) if not already present.
pub async fn seed_factory_presets(pool: &SqlitePool) -> Result<(), AppError> {
    let presets = DeviceMapping::all_presets();

    for p in presets {
        let metrics_json = serde_json::to_string(&p.metrics)
            .map_err(|e| AppError::Database(format!("Failed to serialize preset metrics: {}", e)))?;

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
        .bind(&p.id)
        .bind(&p.device_name)
        .bind(&p.manufacturer)
        .bind(if p.is_preset { 1 } else { 0 })
        .bind(&p.date_col)
        .bind(&p.date_format)
        .bind(&metrics_json)
        .bind(p.created_at.to_rfc3339())
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to seed preset {}: {}", p.id, e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_pool_initialization() {
        let pool = create_pool("sqlite::memory:").await.expect("Memory pool failed");

        let row_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_mappings")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(row_count.0, 4, "Must seed exactly 4 factory presets");
    }
}
