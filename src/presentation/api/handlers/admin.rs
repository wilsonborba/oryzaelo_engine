//! # Oryza-Elo Architecture Guardrail: Admin & Mock Data Handlers
//!
//! Presentation endpoint handlers to populate realistic agronomic test data and
//! clean/reset the database for evaluators, teachers, and farmers without physical sensors.

use crate::core::error::AppError;
use crate::presentation::api::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminActionResponse {
    pub status: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PopulateQuery {
    pub days: Option<u32>,
    pub parcels: Option<u32>,
}

/// Populates all database tables with high-fidelity agronomic synthetic data.
pub async fn populate_mock_data(
    State(_state): State<AppState>,
    Query(query): Query<PopulateQuery>,
) -> Result<(StatusCode, Json<AdminActionResponse>), AppError> {
    info!("Iniciando população de dados sintéticos de teste via API...");

    let mut cmd = Command::new("python3");
    cmd.arg("scripts/populate_test_data.py");

    if let Some(d) = query.days {
        cmd.arg("--days").arg(d.to_string());
    }
    if let Some(p) = query.parcels {
        cmd.arg("--parcels").arg(p.to_string());
    }

    let output = cmd.output().map_err(|e| {
        error!(error = %e, "Falha ao executar scripts/populate_test_data.py");
        AppError::Internal(format!("Failed to execute populate script: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "Erro na execução do script de população");
        return Err(AppError::Internal(format!("Populate script error: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!("Dados sintéticos populados com sucesso");

    Ok((
        StatusCode::OK,
        Json(AdminActionResponse {
            status: "success".to_string(),
            message: "Dados demonstrativos populados com sucesso (4 talhões e séries históricas ativas)".to_string(),
            details: Some(stdout),
        }),
    ))
}

/// Cleans test data from the database, preserving official factory presets.
pub async fn clean_mock_data(
    State(_state): State<AppState>,
) -> Result<(StatusCode, Json<AdminActionResponse>), AppError> {
    info!("Iniciando limpeza de dados de teste via API...");

    let output = Command::new("python3")
        .arg("scripts/clean_test_data.py")
        .output()
        .map_err(|e| {
            error!(error = %e, "Falha ao executar scripts/clean_test_data.py");
            AppError::Internal(format!("Failed to execute clean script: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "Erro na execução do script de limpeza");
        return Err(AppError::Internal(format!("Clean script error: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!("Base de dados limpa com sucesso (presets preservados)");

    Ok((
        StatusCode::OK,
        Json(AdminActionResponse {
            status: "success".to_string(),
            message: "Base de dados limpa com sucesso. Presets oficiais preservados.".to_string(),
            details: Some(stdout),
        }),
    ))
}
