//! # Oryza-Elo Architecture Guardrail: Granular Edge Benchmarks
//!
//! Real-time edge performance verification endpoints:
//! - `/api/v1/benchmarks/latency`: ONNX CPU inference latency micro-benchmark (< 5 ms target).
//! - `/api/v1/benchmarks/biomet`: Biophysical feature engineering computation latency (< 1 ms target).
//! - `/api/v1/benchmarks/storage`: SQLite local storage I/O and query latency (< 10 ms target).
//! - `/api/v1/benchmarks/throughput`: Peak sustained inference throughput (inferences / sec).

use crate::core::error::AppError;
use crate::dal::inference::onnx_engine::CategoricalEncoder;
use crate::domain::models::weather::{BiometFeatures, DailyWeatherRecord};
use crate::domain::services::biomet_calculator::BiometCalculator;
use crate::presentation::api::state::AppState;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

#[derive(Deserialize)]
pub struct BenchmarkQuery {
    pub iterations: Option<usize>,
}

#[derive(Serialize)]
pub struct LatencyBenchmarkResponse {
    pub hardware_target: String,
    pub iterations: usize,
    pub mean_latency_us: f64,
    pub mean_latency_ms: f64,
    pub min_latency_us: f64,
    pub max_latency_us: f64,
    pub p50_latency_us: f64,
    pub p95_latency_us: f64,
    pub p99_latency_us: f64,
    pub target_ceiling_ms: f64,
    pub speedup_vs_edge_ceiling: f64,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct BiometBenchmarkResponse {
    pub iterations: usize,
    pub retrospective_days: usize,
    pub features_computed: usize,
    pub mean_duration_us: f64,
    pub mean_duration_ms: f64,
    pub min_duration_us: f64,
    pub max_duration_us: f64,
    pub p50_duration_us: f64,
    pub p99_duration_us: f64,
    pub target_ceiling_ms: f64,
    pub speedup_vs_target: f64,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct StorageBenchmarkResponse {
    pub journal_mode: String,
    pub db_size_bytes: u64,
    pub db_size_kb: f64,
    pub query_latency_ms: f64,
    pub total_weather_records: i64,
    pub total_predictions: i64,
    pub target_ceiling_ms: f64,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ThroughputBenchmarkResponse {
    pub hardware_target: String,
    pub batch_size: usize,
    pub total_duration_ms: f64,
    pub inferences_per_second: f64,
    pub target_inferences_per_second: f64,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

fn sample_features() -> BiometFeatures {
    BiometFeatures {
        gdd_cum_7d: 118.45,
        gdd_cum_14d: 239.29,
        gdd_cum_30d: 509.5,
        gdd_cum_60d: 1018.73,
        rain_cum_7d: 9.57,
        rain_cum_14d: 10.25,
        rain_cum_30d: 73.66,
        rain_cum_60d: 287.31,
        rain_max_7d: 3.3,
        rain_max_14d: 3.3,
        rain_max_30d: 17.0,
        rain_max_60d: 48.68,
        cdd_7d: 4.0,
        cdd_14d: 11.0,
        cdd_30d: 18.0,
        cdd_60d: 30.0,
        dtr_mean_7d: 1.29,
        dtr_mean_14d: 1.47,
        dtr_mean_30d: 1.43,
        dtr_mean_60d: 1.46,
        dtr_std_7d: 0.36,
        dtr_std_14d: 0.33,
        dtr_std_30d: 0.32,
        dtr_std_60d: 0.35,
        rad_cum_7d: 130.0,
        rad_cum_14d: 275.83,
        rad_cum_30d: 524.22,
        rad_cum_60d: 998.92,
        rh_mean_7d: 82.83,
        rh_mean_14d: 81.83,
        rh_mean_30d: 81.73,
        rh_mean_60d: 80.81,
        month: 2.0,
        day_of_year: 55.0,
        photoperiod_hours: 11.95,
        ptq_30d: 1.03,
        ptq_60d: 0.98,
        vpd_proxy_14d: 0.27,
        vpd_proxy_30d: 0.26,
        lat_clean: 7.82,
        lon_clean: 100.26,
        rice_ecosystem_code: 4.0,
        rice_variety_code: 55.0,
        province_code: 37.0,
    }
}

fn generate_synthetic_weather_history(days: usize) -> Vec<DailyWeatherRecord> {
    let start_date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    (0..days)
        .map(|i| {
            let date = start_date + chrono::Duration::days(i as i64);
            DailyWeatherRecord {
                date,
                t_max: Some(33.0 + ((i % 3) as f64 * 0.5)),
                t_min: Some(23.0 + ((i % 2) as f64 * 0.5)),
                precipitation_mm: Some(if i % 5 == 0 { 12.0 } else { 0.0 }),
                radiation_mj_m2: Some(19.5 + ((i % 4) as f64 * 0.8)),
                relative_humidity_pct: Some(78.0 + ((i % 5) as f64 * 1.0)),
                t_max_sensor_id: Some("benchmark".into()),
                t_min_sensor_id: Some("benchmark".into()),
                rainfall_sensor_id: Some("benchmark".into()),
                radiation_sensor_id: Some("benchmark".into()),
                humidity_sensor_id: Some("benchmark".into()),
            }
        })
        .collect()
}

/// 1. ONNX CPU inference latency micro-benchmark (`GET /api/v1/benchmarks/latency`).
pub async fn run_latency_benchmark(
    State(state): State<AppState>,
    Query(query): Query<BenchmarkQuery>,
) -> Result<Json<LatencyBenchmarkResponse>, AppError> {
    let iterations = query.iterations.unwrap_or(100).clamp(10, 1000);
    let sample = sample_features();

    // Warm-up
    for _ in 0..5 {
        let _ = state.onnx_engine.predict(&sample)?;
    }

    let mut latencies_us: Vec<f64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let t0 = Instant::now();
        let _ = state.onnx_engine.predict(&sample)?;
        let elapsed = t0.elapsed().as_nanos() as f64 / 1_000.0;
        latencies_us.push(elapsed);
    }

    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sum: f64 = latencies_us.iter().sum();
    let mean_us = sum / iterations as f64;
    let mean_ms = mean_us / 1000.0;
    let min_us = latencies_us[0];
    let max_us = latencies_us[iterations - 1];
    let p50_us = latencies_us[(iterations as f64 * 0.50) as usize];
    let p95_us = latencies_us[((iterations as f64 * 0.95) as usize).min(iterations - 1)];
    let p99_us = latencies_us[((iterations as f64 * 0.99) as usize).min(iterations - 1)];

    let target_ceiling_ms = 5.0;
    let speedup = (target_ceiling_ms * 1000.0) / mean_us;
    let status = if mean_ms < target_ceiling_ms {
        "passed".to_string()
    } else {
        "degraded".to_string()
    };

    Ok(Json(LatencyBenchmarkResponse {
        hardware_target: std::env::consts::ARCH.to_string(),
        iterations,
        mean_latency_us: (mean_us * 100.0).round() / 100.0,
        mean_latency_ms: (mean_ms * 10000.0).round() / 10000.0,
        min_latency_us: (min_us * 100.0).round() / 100.0,
        max_latency_us: (max_us * 100.0).round() / 100.0,
        p50_latency_us: (p50_us * 100.0).round() / 100.0,
        p95_latency_us: (p95_us * 100.0).round() / 100.0,
        p99_latency_us: (p99_us * 100.0).round() / 100.0,
        target_ceiling_ms,
        speedup_vs_edge_ceiling: (speedup * 10.0).round() / 10.0,
        status,
        timestamp: Utc::now(),
    }))
}

/// 2. Biophysical feature engineering benchmark (`GET /api/v1/benchmarks/biomet`).
pub async fn run_biomet_benchmark(
    Query(query): Query<BenchmarkQuery>,
) -> Result<Json<BiometBenchmarkResponse>, AppError> {
    let iterations = query.iterations.unwrap_or(100).clamp(10, 1000);
    let history = generate_synthetic_weather_history(60);
    let eval_date = history.last().unwrap().date;
    let eco_code = CategoricalEncoder::encode_ecosystem("Wetland / Lowland");
    let var_code = CategoricalEncoder::encode_variety("KDML105");
    let prov_code = CategoricalEncoder::encode_province("Suphan Buri");

    // Warm-up
    for _ in 0..5 {
        let _ = BiometCalculator::compute(&history, eval_date, 14.5, 100.1, eco_code, var_code, prov_code, None)?;
    }

    let mut durations_us: Vec<f64> = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let t0 = Instant::now();
        let _ = BiometCalculator::compute(&history, eval_date, 14.5, 100.1, eco_code, var_code, prov_code, None)?;
        durations_us.push(t0.elapsed().as_nanos() as f64 / 1_000.0);
    }

    durations_us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sum: f64 = durations_us.iter().sum();
    let mean_us = sum / iterations as f64;
    let mean_ms = mean_us / 1000.0;
    let min_us = durations_us[0];
    let max_us = durations_us[iterations - 1];
    let p50_us = durations_us[(iterations as f64 * 0.50) as usize];
    let p99_us = durations_us[((iterations as f64 * 0.99) as usize).min(iterations - 1)];

    let target_ceiling_ms = 1.0;
    let speedup = (target_ceiling_ms * 1000.0) / mean_us;
    let status = if mean_ms < target_ceiling_ms {
        "passed".to_string()
    } else {
        "degraded".to_string()
    };

    Ok(Json(BiometBenchmarkResponse {
        iterations,
        retrospective_days: 60,
        features_computed: 44,
        mean_duration_us: (mean_us * 100.0).round() / 100.0,
        mean_duration_ms: (mean_ms * 10000.0).round() / 10000.0,
        min_duration_us: (min_us * 100.0).round() / 100.0,
        max_duration_us: (max_us * 100.0).round() / 100.0,
        p50_duration_us: (p50_us * 100.0).round() / 100.0,
        p99_duration_us: (p99_us * 100.0).round() / 100.0,
        target_ceiling_ms,
        speedup_vs_target: (speedup * 10.0).round() / 10.0,
        status,
        timestamp: Utc::now(),
    }))
}

/// 3. Local SQLite database storage and query benchmark (`GET /api/v1/benchmarks/storage`).
pub async fn run_storage_benchmark(
    State(state): State<AppState>,
) -> Result<Json<StorageBenchmarkResponse>, AppError> {
    let t0 = Instant::now();
    let _ = sqlx::query("SELECT parcel_id, record_date, t_max, t_min FROM weather_records ORDER BY record_date DESC LIMIT 60")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    let query_latency_ms = t0.elapsed().as_secs_f64() * 1000.0;


    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode;")
        .fetch_one(&state.pool)
        .await
        .unwrap_or_else(|_| "unknown".to_string());

    let weather_records_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weather_records")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let predictions_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prediction_history")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    // Extract file size from database path if file-based on disk
    let file_path = crate::dal::database::connection::DEFAULT_DB_REL_PATH;
    let (db_size_bytes, db_size_kb) = if Path::new(file_path).exists() {
        let meta = std::fs::metadata(file_path).ok();
        let bytes = meta.map(|m| m.len()).unwrap_or(0);
        (bytes, (bytes as f64) / 1024.0)
    } else {
        (0, 0.0)
    };


    let target_ceiling_ms = 10.0;
    let status = if query_latency_ms < target_ceiling_ms {
        "passed".to_string()
    } else {
        "degraded".to_string()
    };

    Ok(Json(StorageBenchmarkResponse {
        journal_mode,
        db_size_bytes,
        db_size_kb: (db_size_kb * 100.0).round() / 100.0,
        query_latency_ms: (query_latency_ms * 1000.0).round() / 1000.0,
        total_weather_records: weather_records_count,
        total_predictions: predictions_count,
        target_ceiling_ms,
        status,
        timestamp: Utc::now(),
    }))
}

/// 4. Peak CPU inference throughput benchmark (`GET /api/v1/benchmarks/throughput`).
pub async fn run_throughput_benchmark(
    State(state): State<AppState>,
    Query(query): Query<BenchmarkQuery>,
) -> Result<Json<ThroughputBenchmarkResponse>, AppError> {
    let batch_size = query.iterations.unwrap_or(500).clamp(50, 2000);
    let sample = sample_features();

    // Warm-up
    for _ in 0..5 {
        let _ = state.onnx_engine.predict(&sample)?;
    }

    let t0 = Instant::now();
    for _ in 0..batch_size {
        let _ = state.onnx_engine.predict(&sample)?;
    }
    let elapsed = t0.elapsed();
    let total_duration_ms = elapsed.as_secs_f64() * 1000.0;
    let inferences_per_sec = (batch_size as f64) / elapsed.as_secs_f64();
    let target_inferences_per_second = 200.0; // Corresponds to < 5ms latency

    let status = if inferences_per_sec >= target_inferences_per_second {
        "passed".to_string()
    } else {
        "degraded".to_string()
    };

    Ok(Json(ThroughputBenchmarkResponse {
        hardware_target: std::env::consts::ARCH.to_string(),
        batch_size,
        total_duration_ms: (total_duration_ms * 100.0).round() / 100.0,
        inferences_per_second: (inferences_per_sec * 10.0).round() / 10.0,
        target_inferences_per_second,
        status,
        timestamp: Utc::now(),
    }))
}
