//! # Oryza-Elo Architecture Guardrail: Application State
//!
//! Shared state for Axum API handlers.

use crate::core::settings::Settings;
use crate::dal::database::repositories::{
    ConfigRepository, DeviceMappingRepository, ParcelRepository, PredictionRepository,
    SensorReadingRepository, WeatherRepository,
};
use crate::dal::inference::onnx_engine::OnnxInferenceEngine;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub parcel_repo: ParcelRepository,
    pub device_repo: DeviceMappingRepository,
    pub weather_repo: WeatherRepository,
    pub sensor_reading_repo: SensorReadingRepository,
    pub prediction_repo: PredictionRepository,
    pub config_repo: ConfigRepository,
    pub onnx_engine: Arc<OnnxInferenceEngine>,
    pub settings: &'static Settings,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        onnx_engine: Arc<OnnxInferenceEngine>,
        settings: &'static Settings,
    ) -> Self {
        Self {
            parcel_repo: ParcelRepository::new(pool.clone()),
            device_repo: DeviceMappingRepository::new(pool.clone()),
            weather_repo: WeatherRepository::new(pool.clone()),
            sensor_reading_repo: SensorReadingRepository::new(pool.clone()),
            prediction_repo: PredictionRepository::new(pool.clone()),
            config_repo: ConfigRepository::new(pool.clone()),
            pool,
            onnx_engine,
            settings,
        }
    }
}
