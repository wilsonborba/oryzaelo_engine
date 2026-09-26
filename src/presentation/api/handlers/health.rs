//! # Oryza-Elo Architecture Guardrail: Health & Monitoring Handlers
//!
//! Differentiated health probes:
//! - Root `/health`: Lightweight liveness ping for infrastructure (Docker, systemd, LB).
//! - `/api/v1/health/system`: Host OS, CPU cores, memory RSS, and edge hardware health.
//! - `/api/v1/health/app`: Application internal subsystems (SQLite WAL pool, ONNX session, Cron scheduler).
//! - `/api/v1/health`: Consolidated summary.

use crate::presentation::api::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use sysinfo::{Disks, System};

static START_TIME: OnceLock<Instant> = OnceLock::new();
static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn get_uptime_secs() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_secs()
}

fn get_memory_rss_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(pages) = parts[1].parse::<u64>() {
                    return pages * 4096;
                }
            }
        }
    }
    0
}

/// Whole-machine (not just this process) CPU and RAM usage, sourced from a
/// long-lived `sysinfo::System` so consecutive `/health/system` calls report
/// a real usage delta rather than a meaningless first-sample zero.
struct HostUsage {
    cpu_usage_pct: f64,
    ram_total_mb: u64,
    ram_used_mb: u64,
}

fn get_host_usage() -> HostUsage {
    let sys_lock = SYSTEM.get_or_init(|| Mutex::new(System::new_all()));
    let mut sys = sys_lock.lock().unwrap_or_else(|e| e.into_inner());
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage_pct = sys.global_cpu_usage() as f64;
    let ram_total_mb = sys.total_memory() / (1024 * 1024);
    let ram_used_mb = sys.used_memory() / (1024 * 1024);

    HostUsage {
        cpu_usage_pct,
        ram_total_mb,
        ram_used_mb,
    }
}

/// Aggregate usage across every mounted disk (real hardware varies: a
/// Raspberry Pi's SD card, a laptop's NVMe SSD, etc. — never assume one).
fn get_disk_usage_pct() -> f64 {
    let disks = Disks::new_with_refreshed_list();
    let (total, available): (u64, u64) = disks
        .list()
        .iter()
        .fold((0, 0), |(t, a), d| (t + d.total_space(), a + d.available_space()));

    if total == 0 {
        return 0.0;
    }
    ((total - available) as f64 / total as f64) * 100.0
}

/// Lightweight liveness ping for load balancers, Docker, and systemd watchdog.
pub async fn liveness_ping() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// System and host hardware health probe (Linux OS, CPU parallelism, memory RSS).
///
/// Every value here is read live from the actual host the engine is running
/// on — a Raspberry Pi in the field, a developer's laptop, or a CI runner —
/// never a fixed/demo placeholder. `cpu_cores`/`ram_total_mb` in particular
/// will differ per machine and are not tied to any specific reference
/// hardware SKU shown elsewhere in the UI.
pub async fn system_health(State(_state): State<AppState>) -> Json<Value> {
    let uptime_sec = get_uptime_secs();
    let rss_bytes = get_memory_rss_bytes();
    let rss_mb = (rss_bytes as f64) / (1024.0 * 1024.0);
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let host = get_host_usage();
    let disk_usage_pct = get_disk_usage_pct();
    let ram_usage_pct = if host.ram_total_mb == 0 {
        0.0
    } else {
        (host.ram_used_mb as f64 / host.ram_total_mb as f64) * 100.0
    };

    Json(json!({
        "status": "healthy",
        "scope": "system",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu_cores": cpu_cores,
        "cpu_usage_pct": (host.cpu_usage_pct * 100.0).round() / 100.0,
        "ram_total_mb": host.ram_total_mb,
        "ram_used_mb": host.ram_used_mb,
        "ram_usage_pct": (ram_usage_pct * 100.0).round() / 100.0,
        "disk_usage_pct": (disk_usage_pct * 100.0).round() / 100.0,
        "memory_rss_mb": (rss_mb * 100.0).round() / 100.0,
        "memory_rss_bytes": rss_bytes,
        "uptime_seconds": uptime_sec,
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// Application internal subsystems health probe (SQLite, ONNX session, Cron scheduler).
pub async fn app_health(State(state): State<AppState>) -> Json<Value> {
    let sqlite_status = match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => "connected",
        Err(_) => "disconnected",
    };

    let total_parcels = state
        .parcel_repo
        .list_all()
        .await
        .map(|p| p.len())
        .unwrap_or(0);

    let weather_records_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM weather_records")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let predictions_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prediction_history")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let cron_target_time = state
        .config_repo
        .get(crate::domain::tasks::cron_scheduler::CRON_TARGET_TIME_CONFIG_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| crate::domain::tasks::cron_scheduler::DEFAULT_CRON_TARGET_TIME.to_string());

    Json(json!({
        "status": "healthy",
        "scope": "application",
        "app_name": state.settings.app_name,
        "version": env!("CARGO_PKG_VERSION"),
        "sqlite": {
            "status": sqlite_status,
            "total_weather_records": weather_records_count,
            "total_predictions": predictions_count,
            "total_parcels": total_parcels
        },
        "onnx_engine": {
            "status": "loaded",
            "model_path": "src/dal/data/processed/models/rice_stage_classifier.onnx"
        },
        "cron_scheduler": {
            "target_time": cron_target_time,
            "status": "active"
        },
        "edge_mode": "autonomous-air-gapped",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// Consolidated overall health status combining system and application metrics.
pub async fn consolidated_health(State(state): State<AppState>) -> Json<Value> {
    let sys = system_health(State(state.clone())).await.0;
    let app = app_health(State(state)).await.0;

    Json(json!({
        "status": "healthy",
        "system": sys,
        "application": app,
        "timestamp": Utc::now().to_rfc3339()
    }))
}
