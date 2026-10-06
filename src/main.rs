//! # Oryza-Elo Engine: Main Entry Point

use oryzaelo_engine::core::settings::{app_settings, DEFAULT_AUTONOMY_MODE, DEFAULT_INFERENCE_ENGINE};
use oryzaelo_engine::dal::database::connection::{
    configured_db_path, get_default_db_path, init_default_pool, is_legacy_db_left_behind,
};
use oryzaelo_engine::dal::inference::onnx_engine::OnnxInferenceEngine;
use oryzaelo_engine::domain::tasks::cron_scheduler::CronScheduler;
use oryzaelo_engine::presentation::api::routes::create_router;
use oryzaelo_engine::presentation::api::state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = app_settings();
    info!(
        app = %settings.app_name,
        "Iniciando Oryzaelo-Engine microserviço de borda..."
    );

    // Operating modes declared in .env (only local_only and onnx_resident exist in this version)
    if settings.autonomy_mode != DEFAULT_AUTONOMY_MODE {
        warn!(
            requested = %settings.autonomy_mode,
            "AUTONOMY_MODE não suportado nesta versão (a sincronização em nuvem é proposta futura); operando em local_only"
        );
    }
    if settings.inference_engine != DEFAULT_INFERENCE_ENGINE {
        warn!(
            requested = %settings.inference_engine,
            "INFERENCE_ENGINE não suportado nesta versão; usando onnx_resident"
        );
    }
    info!(
        autonomy_mode = DEFAULT_AUTONOMY_MODE,
        inference_engine = DEFAULT_INFERENCE_ENGINE,
        "Modos de operação efetivos"
    );

    // 2. Initialize SQLite persistence pool & run migrations/seeds
    let db_path = configured_db_path();
    if is_legacy_db_left_behind(&db_path) {
        warn!(
            configured = %db_path.display(),
            legacy = %get_default_db_path().display(),
            "DATABASE_PATH aponta para um banco novo, mas ainda existe um banco no caminho antigo; mova o arquivo antigo para manter os dados"
        );
    }
    let pool = init_default_pool().await?;
    info!(path = %db_path.display(), "Banco de dados SQLite inicializado");

    // 3. Load ONNX CatBoost model as resident singleton session
    let onnx_engine = Arc::new(OnnxInferenceEngine::init_default()?);
    info!("Modelo de inferência ONNX CatBoost carregado residente em memória");

    // 4. Build application state
    let state = AppState::new(pool.clone(), onnx_engine.clone(), settings);

    // 5. Spawn background Nightly Cron Scheduler (target time configurable via
    //    /api/v1/config, defaulting to 23:59)
    CronScheduler::spawn(
        state.parcel_repo.clone(),
        state.weather_repo.clone(),
        state.prediction_repo.clone(),
        onnx_engine,
        state.config_repo.clone(),
    );

    // 6. Build HTTP Router (with conditional ServeDir for Flutter Web)
    let app = create_router(state);

    // 7. Bind TCP listener & serve
    let addr: SocketAddr = format!("{}:{}", settings.server_host, settings.server_port).parse()?;
    info!("Oryzaelo-Engine escutando em http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
