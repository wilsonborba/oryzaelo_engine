//! # Oryza-Elo Architecture Guardrail: API Routes
//!
//! Route registration, middlewares and fallback static service mounting.

use crate::presentation::api::handlers::{
    admin, benchmark, config, devices, health, parcels, phenology, weather,
};
use crate::presentation::api::state::AppState;
use axum::routing::{get, post};
use axum::Router;
use std::path::Path;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

/// Builds the complete Axum Router with all v1 endpoints and conditional Flutter Web ServeDir.
pub fn create_router(state: AppState) -> Router {
    let settings = state.settings;

    let api_v1 = Router::new()
        // Health Probes: System (Host OS/Hardware) vs Application (SQLite, ONNX, Cron)
        .route("/health", get(health::consolidated_health))
        .route("/health/system", get(health::system_health))
        .route("/health/app", get(health::app_health))
        // Parcels CRUD
        .route("/parcels", get(parcels::list_parcels).post(parcels::create_parcel))
        .route(
            "/parcels/:id",
            get(parcels::get_parcel)
                .put(parcels::update_parcel)
                .delete(parcels::delete_parcel),
        )
        // Device Presets & Custom Mappings
        .route("/devices/presets", get(devices::list_presets))
        .route(
            "/devices/mappings",
            get(devices::list_mappings).post(devices::create_mapping),
        )
        .route(
            "/devices/mappings/:id",
            get(devices::get_mapping).delete(devices::delete_mapping),
        )
        // Weather Ingestion & Time Series
        .route("/weather/upload-csv", post(weather::upload_csv_multipart))
        .route("/weather/ingest", post(weather::ingest_csv_json))
        .route("/weather/record", post(weather::ingest_single_record))
        .route("/weather/records", get(weather::get_records).delete(weather::delete_records))
        .route("/weather/history", get(weather::get_records))
        .route("/weather/analytics", get(weather::get_weather_analytics))
        // Phenology Inference, Simulation & History
        .route("/phenology/predict", post(phenology::predict_stage))
        .route("/phenology/simulate", post(phenology::simulate_scenario))
        .route("/phenology/latest", get(phenology::get_latest_prediction))
        .route("/phenology/history", get(phenology::get_prediction_history))
        // Edge Node Configuration
        .route("/config", get(config::get_all_config).put(config::update_config))
        // Granular Real-Time Edge Benchmarks
        .route("/benchmarks/latency", get(benchmark::run_latency_benchmark))
        .route("/benchmarks/biomet", get(benchmark::run_biomet_benchmark))
        .route("/benchmarks/storage", get(benchmark::run_storage_benchmark))
        .route("/benchmarks/throughput", get(benchmark::run_throughput_benchmark))
        // Admin & Mock Data (Turnkey Testing for Evaluators and Farmers)
        .route("/admin/populate", post(admin::populate_mock_data))
        .route("/admin/clean", post(admin::clean_mock_data));


    let mut router = Router::new()
        // Root Liveness Ping (Infrastructure, Docker, systemd)
        .route("/health", get(health::liveness_ping))
        .route("/ping", get(health::liveness_ping))
        .nest("/api/v1", api_v1)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Conditional ServeDir for Pre-built Flutter Web Assets (Raspberry Pi Edge Mode)
    if let Some(ref static_path) = settings.static_dir {
        if Path::new(static_path).exists() {
            info!(
                static_dir = %static_path,
                "Montando fallback de assets estáticos do Flutter Web (modo local/Raspberry Pi)"
            );
            router = router.fallback_service(ServeDir::new(static_path));
        } else {
            warn!(
                static_dir = %static_path,
                "STATIC_DIR configurado, mas diretório não encontrado no disco. Servindo apenas API."
            );
        }
    }

    router
}
