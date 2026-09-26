//! # Oryza-Elo Architecture Guardrail: Config Handlers

use crate::core::error::{AppError, DomainError};
use crate::domain::tasks::cron_scheduler::CRON_TARGET_TIME_CONFIG_KEY;
use crate::presentation::api::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub configs: BTreeMap<String, String>,
}

pub async fn get_all_config(State(state): State<AppState>) -> Result<Json<BTreeMap<String, String>>, AppError> {
    let configs = state.config_repo.list_all().await?;
    Ok(Json(configs))
}

pub async fn update_config(
    State(state): State<AppState>,
    Json(payload): Json<UpdateConfigPayload>,
) -> Result<Json<BTreeMap<String, String>>, AppError> {
    for (k, v) in &payload.configs {
        if k == CRON_TARGET_TIME_CONFIG_KEY && !is_valid_hh_mm(v) {
            return Err(AppError::Domain(DomainError::ValidationError(format!(
                "Invalid {}: '{}' (expected HH:MM, 00:00-23:59)",
                CRON_TARGET_TIME_CONFIG_KEY, v
            ))));
        }
    }
    for (k, v) in payload.configs {
        state.config_repo.set(&k, &v).await?;
    }
    let updated = state.config_repo.list_all().await?;
    Ok(Json(updated))
}

fn is_valid_hh_mm(value: &str) -> bool {
    let mut parts = value.split(':');
    let (Some(h), Some(m), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    matches!((h.parse::<u32>(), m.parse::<u32>()), (Ok(h), Ok(m)) if h < 24 && m < 60)
}
