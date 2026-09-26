//! # Oryza-Elo Architecture Guardrail: Device Mapping Handlers

use crate::core::error::AppError;
use crate::domain::models::device_mapping::DeviceMapping;
use crate::presentation::api::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

pub async fn list_presets(State(state): State<AppState>) -> Result<Json<Vec<DeviceMapping>>, AppError> {
    let presets = state.device_repo.list_presets().await?;
    Ok(Json(presets))
}

/// Custom (non-preset) mappings only. Official presets are always available
/// separately via `/devices/presets` — this keeps the two lists disjoint
/// instead of making every caller filter `is_preset` client-side.
pub async fn list_mappings(State(state): State<AppState>) -> Result<Json<Vec<DeviceMapping>>, AppError> {
    let mappings = state
        .device_repo
        .list_all()
        .await?
        .into_iter()
        .filter(|m| !m.is_preset)
        .collect::<Vec<_>>();
    Ok(Json(mappings))
}

pub async fn create_mapping(
    State(state): State<AppState>,
    Json(mut mapping): Json<DeviceMapping>,
) -> Result<(StatusCode, Json<DeviceMapping>), AppError> {
    // This endpoint is for user-defined mappings only. Factory presets are
    // seeded internally at boot (`seed_factory_presets`); a client sending
    // `is_preset: true`, or reusing an existing preset's id, must never be
    // able to plant or silently overwrite one via the public API — `save`
    // is an upsert with no other protection against that.
    mapping.is_preset = false;
    if let Some(existing) = state.device_repo.get_by_id(&mapping.id).await? {
        if existing.is_preset {
            return Err(AppError::BadRequest(format!(
                "Cannot overwrite factory preset device mapping: {}",
                mapping.id
            )));
        }
    }

    state.device_repo.save(&mapping).await?;
    Ok((StatusCode::CREATED, Json(mapping)))
}

pub async fn get_mapping(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeviceMapping>, AppError> {
    let mapping = state
        .device_repo
        .get_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Device mapping {} not found", id)))?;

    Ok(Json(mapping))
}

pub async fn delete_mapping(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let deleted = state.device_repo.delete(&id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!("Device mapping {} not found", id)));
    }
    Ok(Json(json!({ "deleted": true, "id": id })))
}
