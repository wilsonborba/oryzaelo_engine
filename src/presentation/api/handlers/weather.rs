//! # Oryza-Elo Architecture Guardrail: Weather Handlers

use crate::core::error::AppError;
use crate::core::settings::RICE_BASE_TEMPERATURE_CELSIUS;
use crate::domain::models::metric_type::MetricType;
use crate::domain::models::weather::DailyWeatherRecord;
use crate::domain::services::csv_ingestion::{CsvIngestionService, IngestionReport};
use crate::domain::services::weather_analytics::{WeatherAnalyticsReport, WeatherAnalyticsService};
use crate::presentation::api::state::AppState;
use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct IngestCsvPayload {
    pub parcel_id: String,
    pub device_id: String,
    pub csv_content: String,
}

#[derive(Deserialize)]
pub struct WeatherQuery {
    pub parcel_id: String,
    pub days: Option<usize>,
    pub up_to_date: Option<NaiveDate>,
}

#[derive(Serialize)]
pub struct UploadCsvResponse {
    pub message: String,
    pub report: IngestionReport,
    pub records_persisted: usize,
}

/// Ingests CSV sent as JSON payload.
pub async fn ingest_csv_json(
    State(state): State<AppState>,
    Json(payload): Json<IngestCsvPayload>,
) -> Result<(StatusCode, Json<UploadCsvResponse>), AppError> {
    process_csv_ingestion(&state, &payload.parcel_id, &payload.device_id, &payload.csv_content).await
}

/// Ingests CSV uploaded via HTTP Multipart Form (`multipart/form-data`).
pub async fn upload_csv_multipart(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadCsvResponse>), AppError> {
    let mut parcel_id: Option<String> = None;
    let mut device_id: Option<String> = None;
    let mut csv_content: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read multipart field: {}", e)))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "parcel_id" => {
                parcel_id = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?);
            }
            "device_id" => {
                device_id = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?);
            }
            "file" | "csv" => {
                csv_content = Some(field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?);
            }
            _ => {}
        }
    }

    let p_id = parcel_id.ok_or_else(|| AppError::BadRequest("Missing 'parcel_id' in form data".into()))?;
    let d_id = device_id.ok_or_else(|| AppError::BadRequest("Missing 'device_id' in form data".into()))?;
    let content = csv_content.ok_or_else(|| AppError::BadRequest("Missing 'file' in form data".into()))?;

    process_csv_ingestion(&state, &p_id, &d_id, &content).await
}

async fn process_csv_ingestion(
    state: &AppState,
    parcel_id: &str,
    device_id: &str,
    csv_content: &str,
) -> Result<(StatusCode, Json<UploadCsvResponse>), AppError> {
    // 1. Verify parcel exists
    let _ = state
        .parcel_repo
        .get_by_id(parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", parcel_id)))?;

    // 2. Fetch sensor profile
    let mapping = state
        .device_repo
        .get_by_id(device_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Sensor profile {} not found", device_id)))?;

    // 3. Parse and convert units deterministically
    let report = CsvIngestionService::parse_csv(csv_content, &mapping)?;

    // 4. Persist each reading through the aggregation pipeline -- each
    //    metric this sensor reports lands only in its own table, and the
    //    daily aggregate is recomputed per (date) without touching fields
    //    other sensors already contributed.
    let mut distinct_dates = std::collections::BTreeSet::new();
    for reading in &report.readings {
        let recorded_at = reading
            .date
            .and_hms_opt(12, 0, 0)
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        state
            .weather_repo
            .record_reading(parcel_id, device_id, reading.metric_type, reading.value, recorded_at)
            .await?;
        distinct_dates.insert(reading.date);
    }
    let records_persisted = distinct_dates.len();

    Ok((
        StatusCode::OK,
        Json(UploadCsvResponse {
            message: format!(
                "Successfully processed {} rows ({} daily records persisted, {} rows rejected)",
                report.total_rows, records_persisted, report.failed_rows
            ),
            report,
            records_persisted,
        }),
    ))
}

/// Flattened weather record with a server-computed `daily_gdd`, so every client
/// (chart, table, CSV export) reads the exact same GDD value the biomet
/// aggregates use, instead of each recomputing it independently.
/// `is_partial`/`missing_metrics` let the UI show exactly what a day is
/// still waiting on, instead of hiding an incomplete day or faking a value.
#[derive(Serialize)]
pub struct WeatherRecordResponse {
    pub date: NaiveDate,
    pub t_max: Option<f64>,
    pub t_min: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub radiation_mj_m2: Option<f64>,
    pub relative_humidity_pct: Option<f64>,
    pub t_max_sensor_id: Option<String>,
    pub t_min_sensor_id: Option<String>,
    pub rainfall_sensor_id: Option<String>,
    pub radiation_sensor_id: Option<String>,
    pub humidity_sensor_id: Option<String>,
    pub is_partial: bool,
    pub missing_metrics: Vec<MetricType>,
    pub daily_gdd: Option<f64>,
}

impl From<DailyWeatherRecord> for WeatherRecordResponse {
    fn from(r: DailyWeatherRecord) -> Self {
        let daily_gdd = r.daily_gdd(RICE_BASE_TEMPERATURE_CELSIUS);
        let is_partial = r.is_partial();
        let missing_metrics = r.missing_metrics();
        Self {
            date: r.date,
            t_max: r.t_max,
            t_min: r.t_min,
            precipitation_mm: r.precipitation_mm,
            radiation_mj_m2: r.radiation_mj_m2,
            relative_humidity_pct: r.relative_humidity_pct,
            t_max_sensor_id: r.t_max_sensor_id,
            t_min_sensor_id: r.t_min_sensor_id,
            rainfall_sensor_id: r.rainfall_sensor_id,
            radiation_sensor_id: r.radiation_sensor_id,
            humidity_sensor_id: r.humidity_sensor_id,
            is_partial,
            missing_metrics,
            daily_gdd,
        }
    }
}

/// Retrieves retrospective weather records for a parcel.
pub async fn get_records(
    State(state): State<AppState>,
    Query(query): Query<WeatherQuery>,
) -> Result<Json<Vec<WeatherRecordResponse>>, AppError> {
    let up_to_date = query.up_to_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let days = query.days.unwrap_or(60);

    let records = state
        .weather_repo
        .get_retrospective(&query.parcel_id, up_to_date, days)
        .await?;

    Ok(Json(records.into_iter().map(WeatherRecordResponse::from).collect()))
}

/// A single sensor's reading for one metric -- the genuine real-world
/// ingestion path for standalone single-metric sensors (a rain gauge, a
/// pyranometer...), each reporting independently through its own gateway.
#[derive(Deserialize)]
pub struct SensorReadingRequest {
    pub parcel_id: String,
    pub sensor_id: String,
    pub metric_type: MetricType,
    pub value: f64,
    pub recorded_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct SensorReadingResponse {
    pub message: String,
    pub record: DailyWeatherRecord,
}

/// Ingests one raw reading from one physical sensor (LoRaWAN / Modbus / MQTT
/// gateway push). Never overwrites what any other sensor has already
/// contributed to the same day.
pub async fn ingest_sensor_reading(
    State(state): State<AppState>,
    Json(payload): Json<SensorReadingRequest>,
) -> Result<(StatusCode, Json<SensorReadingResponse>), AppError> {
    let _ = state
        .parcel_repo
        .get_by_id(&payload.parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", payload.parcel_id)))?;

    let recorded_at = payload.recorded_at.unwrap_or_else(Utc::now);

    let record = state
        .weather_repo
        .record_reading(&payload.parcel_id, &payload.sensor_id, payload.metric_type, payload.value, recorded_at)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(SensorReadingResponse {
            message: "Sensor reading ingested successfully".to_string(),
            record,
        }),
    ))
}

/// Manual multi-field entry (the "New Record" dialog: a human typing in all
/// 5 values at once, not a physical sensor). Persisted through the same
/// aggregation pipeline, tagged with a synthetic "Manual_Terminal_Entry"
/// sensor id per field.
#[derive(Deserialize)]
pub struct SingleRecordRequest {
    pub parcel_id: String,
    pub date: NaiveDate,
    pub t_max: f64,
    pub t_min: f64,
    pub precipitation_mm: f64,
    pub radiation_mj_m2: f64,
    pub relative_humidity_pct: f64,
    pub source: Option<String>,
}

#[derive(Serialize)]
pub struct SingleRecordResponse {
    pub message: String,
    pub parcel_id: String,
    pub date: NaiveDate,
    pub record: DailyWeatherRecord,
}

/// Ingests a manually entered daily weather record (human-typed, all 5 fields at once).
pub async fn ingest_single_record(
    State(state): State<AppState>,
    Json(payload): Json<SingleRecordRequest>,
) -> Result<(StatusCode, Json<SingleRecordResponse>), AppError> {
    let _ = state
        .parcel_repo
        .get_by_id(&payload.parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", payload.parcel_id)))?;

    let candidate = DailyWeatherRecord {
        date: payload.date,
        t_max: Some(payload.t_max),
        t_min: Some(payload.t_min),
        precipitation_mm: Some(payload.precipitation_mm),
        radiation_mj_m2: Some(payload.radiation_mj_m2),
        relative_humidity_pct: Some(payload.relative_humidity_pct),
        t_max_sensor_id: None,
        t_min_sensor_id: None,
        rainfall_sensor_id: None,
        radiation_sensor_id: None,
        humidity_sensor_id: None,
    };
    candidate.validate().map_err(AppError::Domain)?;

    let sensor_id = payload.source.unwrap_or_else(|| "Manual_Terminal_Entry".to_string());
    let recorded_at = payload
        .date
        .and_hms_opt(12, 0, 0)
        .map(|dt| dt.and_utc())
        .unwrap_or_else(Utc::now);

    state
        .weather_repo
        .record_reading(&payload.parcel_id, &sensor_id, MetricType::TMax, payload.t_max, recorded_at)
        .await?;
    state
        .weather_repo
        .record_reading(&payload.parcel_id, &sensor_id, MetricType::TMin, payload.t_min, recorded_at)
        .await?;
    state
        .weather_repo
        .record_reading(&payload.parcel_id, &sensor_id, MetricType::Rainfall, payload.precipitation_mm, recorded_at)
        .await?;
    state
        .weather_repo
        .record_reading(&payload.parcel_id, &sensor_id, MetricType::Radiation, payload.radiation_mj_m2, recorded_at)
        .await?;
    let record = state
        .weather_repo
        .record_reading(&payload.parcel_id, &sensor_id, MetricType::Humidity, payload.relative_humidity_pct, recorded_at)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(SingleRecordResponse {
            message: "Weather record ingested successfully".to_string(),
            parcel_id: payload.parcel_id,
            date: payload.date,
            record,
        }),
    ))
}

#[derive(Deserialize)]
pub struct AnalyticsQuery {
    pub parcel_id: String,
    pub days: Option<usize>,
    pub up_to_date: Option<NaiveDate>,
}

/// Retrieves agrometeorological analytics, correlation matrix, and dynamic trilingual alerts.
pub async fn get_weather_analytics(
    State(state): State<AppState>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<WeatherAnalyticsReport>, AppError> {
    let _ = state
        .parcel_repo
        .get_by_id(&query.parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", query.parcel_id)))?;

    let up_to_date = query.up_to_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
    let days = query.days.unwrap_or(60);

    let records = state
        .weather_repo
        .get_retrospective(&query.parcel_id, up_to_date, days)
        .await?;

    let report = WeatherAnalyticsService::compute_analytics(&query.parcel_id, &records, up_to_date, days)?;

    Ok(Json(report))
}

#[derive(Deserialize)]
pub struct DeleteRecordsPayload {
    pub parcel_id: String,
    pub dates: Vec<NaiveDate>,
}

#[derive(Serialize)]
pub struct DeleteRecordsResponse {
    pub message: String,
    pub parcel_id: String,
    pub deleted_count: usize,
}

/// Deletes specific daily weather records by date (cascades across every
/// sensor's reading tables for those dates, not just the aggregate row).
pub async fn delete_records(
    State(state): State<AppState>,
    Json(payload): Json<DeleteRecordsPayload>,
) -> Result<(StatusCode, Json<DeleteRecordsResponse>), AppError> {
    let _ = state
        .parcel_repo
        .get_by_id(&payload.parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", payload.parcel_id)))?;

    let count = state
        .weather_repo
        .delete_records(&payload.parcel_id, &payload.dates)
        .await?;

    Ok((
        StatusCode::OK,
        Json(DeleteRecordsResponse {
            message: format!("Deleted {} weather record(s)", count),
            parcel_id: payload.parcel_id,
            deleted_count: count,
        }),
    ))
}

#[derive(Deserialize)]
pub struct SensorReadingsQuery {
    pub parcel_id: String,
    pub metric_type: MetricType,
    pub limit: Option<i64>,
}

/// Lists a specific sensor metric's own raw reading history for a parcel --
/// full provenance/audit trail, independent of any other metric's data.
pub async fn list_sensor_readings(
    State(state): State<AppState>,
    Query(query): Query<SensorReadingsQuery>,
) -> Result<Json<Vec<crate::domain::models::sensor_reading::SensorReading>>, AppError> {
    let readings = state
        .sensor_reading_repo
        .list_for_parcel(&query.parcel_id, query.metric_type, query.limit.unwrap_or(200))
        .await?;
    Ok(Json(readings))
}

#[derive(Deserialize)]
pub struct DeleteSensorReadingQuery {
    pub metric_type: MetricType,
}

/// Deletes one raw sensor reading by id, then recomputes that day's
/// aggregate (the deleted reading's field may now fall back to an older
/// reading for that day, or become null again).
pub async fn delete_sensor_reading(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(query): Query<DeleteSensorReadingQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let existing = state
        .sensor_reading_repo
        .get_by_id(query.metric_type, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Sensor reading {} not found", id)))?;

    state.sensor_reading_repo.delete(query.metric_type, id).await?;
    state
        .weather_repo
        .recompute_day(&existing.parcel_id, existing.recorded_at.date_naive())
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true, "id": id })))
}
