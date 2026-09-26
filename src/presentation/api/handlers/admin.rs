//! # Oryza-Elo Architecture Guardrail: Admin & Mock Data Handlers
//!
//! Pure Rust presentation handlers to populate realistic agronomic test data and
//! clean/reset the database for evaluators, teachers, and farmers without physical sensors.

use crate::core::error::AppError;
use crate::domain::services::mock_data::{MockCleanSummary, MockDataService, MockDataSummary};
use crate::presentation::api::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct PopulateQuery {
    pub days: Option<usize>,
    pub parcels: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CleanQuery {
    pub reset_presets: Option<bool>,
}

/// Populates all database tables with high-fidelity agronomic synthetic data in pure Rust.
pub async fn populate_mock_data(
    State(state): State<AppState>,
    Query(query): Query<PopulateQuery>,
) -> Result<(StatusCode, Json<MockDataSummary>), AppError> {
    info!(
        days = ?query.days,
        parcels = ?query.parcels,
        "Iniciando população de dados sintéticos de teste via MockDataService (Rust nativo)..."
    );

    let summary = MockDataService::populate(&state.pool, query.days, query.parcels).await?;

    info!(
        parcels = summary.parcels_count,
        weather = summary.weather_records_count,
        predictions = summary.predictions_count,
        "Dados sintéticos populados com sucesso via Rust nativo"
    );

    Ok((StatusCode::OK, Json(summary)))
}

/// Cleans test data from the database, preserving official factory presets in pure Rust.
pub async fn clean_mock_data(
    State(state): State<AppState>,
    Query(query): Query<CleanQuery>,
) -> Result<(StatusCode, Json<MockCleanSummary>), AppError> {
    let reset_presets = query.reset_presets.unwrap_or(false);
    info!(
        reset_presets,
        "Iniciando limpeza de dados de teste via MockDataService (Rust nativo)..."
    );

    let summary = MockDataService::clean(&state.pool, reset_presets).await?;

    info!(
        parcels_remaining = summary.parcels_remaining,
        presets_preserved = summary.device_mappings_remaining,
        "Base de dados limpa com sucesso via Rust nativo"
    );

    Ok((StatusCode::OK, Json(summary)))
}
