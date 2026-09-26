//! # Oryza-Elo API Negative-Path & Error-Handling Integration Tests
//!
//! The happy-path E2E test (`api_integration_test.rs`) never exercises a
//! single error branch. This file specifically covers what a real farmer's
//! sensor/CSV upload and device-mapping workflow looks like when things go
//! wrong: bad CSV columns, unknown parcels/devices, invalid physical values,
//! and the device-mapping CRUD's protections around factory presets.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use oryzaelo_engine::core::settings::app_settings;
use oryzaelo_engine::dal::database::connection::create_pool;
use oryzaelo_engine::dal::inference::onnx_engine::{DEFAULT_ONNX_MODEL_PATH, OnnxInferenceEngine};
use oryzaelo_engine::domain::models::farm::FarmParcel;
use oryzaelo_engine::presentation::api::routes::create_router;
use oryzaelo_engine::presentation::api::state::AppState;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

async fn setup_test_app() -> axum::Router {
    let pool = create_pool("sqlite::memory:").await.expect("Failed to initialize test pool");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = repo_root.join(DEFAULT_ONNX_MODEL_PATH);
    let onnx_engine = Arc::new(OnnxInferenceEngine::new(model_path).expect("Failed to load ONNX model"));
    let settings = app_settings();
    let state = AppState::new(pool, onnx_engine, settings);
    create_router(state)
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        panic!("Response body was not valid JSON: {e}. Raw: {:?}", String::from_utf8_lossy(&bytes))
    })
}

async fn create_test_parcel(app: &axum::Router, id: &str) {
    let parcel = FarmParcel {
        id: id.into(),
        name: "Test Parcel".into(),
        rice_variety: "RD43".into(),
        rice_ecosystem: "Irrigated".into(),
        latitude: 14.88,
        longitude: 100.45,
        planting_date: chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        area_hectares: Some(1.0),
    };
    let req = Request::builder()
        .uri("/api/v1/parcels")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&parcel).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "test setup: failed to create parcel");
}

// ── CSV / weather ingestion error paths ─────────────────────────────────────

#[tokio::test]
async fn ingest_csv_with_missing_column_returns_400_naming_the_column() {
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-missing-col").await;

    // Pessl preset expects "AirTemp_Max"; this CSV has a typo'd column name.
    let csv = "Timestamp,AirTemp_MAX_TYPO,AirTemp_Min,Precipitation,SolarRad,RelHumidity\n2026-07-01,31.5,23.2,5.4,210.0,78.0\n";
    let payload = serde_json::json!({
        "parcel_id": "p-missing-col",
        "device_id": "preset_pessl_imetos",
        "csv_content": csv,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/ingest")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "BAD_REQUEST");
    assert!(body["error"].as_str().unwrap().contains("Missing column 'AirTemp_Max'"));
}

#[tokio::test]
async fn ingest_csv_for_unknown_parcel_returns_404() {
    let app = setup_test_app().await;
    // No parcel created — "ghost-parcel" does not exist.
    let csv = "Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity\n2026-07-01,31.5,23.2,5.4,210.0,78.0\n";
    let payload = serde_json::json!({
        "parcel_id": "ghost-parcel",
        "device_id": "preset_pessl_imetos",
        "csv_content": csv,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/ingest")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_json(resp).await;
    assert!(body["error"].as_str().unwrap().contains("ghost-parcel"));
}

#[tokio::test]
async fn ingest_csv_for_unknown_device_mapping_returns_404() {
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-unknown-device").await;

    let csv = "Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity\n2026-07-01,31.5,23.2,5.4,210.0,78.0\n";
    let payload = serde_json::json!({
        "parcel_id": "p-unknown-device",
        "device_id": "does-not-exist-device",
        "csv_content": csv,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/ingest")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = body_json(resp).await;
    assert!(body["error"].as_str().unwrap().contains("does-not-exist-device"));
}

#[tokio::test]
async fn ingest_csv_with_all_invalid_rows_returns_200_with_full_failure_report() {
    // A CSV that parses structurally but every row fails a physical-invariant
    // check must NOT be reported as an HTTP error — it's a successful request
    // that ingested zero rows, and the client must see that via the report.
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-all-bad-rows").await;

    let csv = "Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity\n2026-07-01,20.0,25.0,0.0,15.0,75.0\n";
    let payload = serde_json::json!({
        "parcel_id": "p-all-bad-rows",
        "device_id": "preset_pessl_imetos",
        "csv_content": csv,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/ingest")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["records_persisted"], 0);
    assert_eq!(body["report"]["failed_rows"], 1);
    assert_eq!(body["report"]["successful_rows"], 0);
    assert!(body["report"]["errors"][0].as_str().unwrap().contains("T_min"));
}

#[tokio::test]
async fn ingest_single_record_with_tmin_exceeding_tmax_returns_400() {
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-bad-single").await;

    let payload = serde_json::json!({
        "parcel_id": "p-bad-single",
        "date": "2026-06-06",
        "t_max": 20.0,
        "t_min": 25.0,
        "precipitation_mm": 5.0,
        "radiation_mj_m2": 21.0,
        "relative_humidity_pct": 82.0,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/record")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "DOMAIN_VALIDATION_ERROR");
    assert!(body["error"].as_str().unwrap().contains("cannot exceed"));
}

#[tokio::test]
async fn ingest_single_record_with_invalid_humidity_returns_400() {
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-bad-rh").await;

    let payload = serde_json::json!({
        "parcel_id": "p-bad-rh",
        "date": "2026-06-06",
        "t_max": 30.0,
        "t_min": 20.0,
        "precipitation_mm": 5.0,
        "radiation_mj_m2": 21.0,
        "relative_humidity_pct": 150.0,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/record")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_json(resp).await;
    assert!(body["error"].as_str().unwrap().contains("100.0"));
}

#[tokio::test]
async fn ingest_single_record_for_unknown_parcel_returns_404() {
    let app = setup_test_app().await;

    let payload = serde_json::json!({
        "parcel_id": "no-such-parcel",
        "date": "2026-06-06",
        "t_max": 30.0,
        "t_min": 20.0,
        "precipitation_mm": 5.0,
        "radiation_mj_m2": 21.0,
        "relative_humidity_pct": 80.0,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/record")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn ingest_single_record_response_includes_daily_gdd() {
    // Locks the daily_gdd field added to WeatherRecordResponse this session —
    // regresses loudly if the response shape ever drifts from the frontend's
    // expectations again (this was the whole point of unifying GDD sourcing).
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-gdd-check").await;

    let payload = serde_json::json!({
        "parcel_id": "p-gdd-check",
        "date": "2026-06-06",
        "t_max": 34.0,
        "t_min": 22.0,
        "precipitation_mm": 0.0,
        "radiation_mj_m2": 20.0,
        "relative_humidity_pct": 70.0,
    });
    let req = Request::builder()
        .uri("/api/v1/weather/record")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let req = Request::builder()
        .uri("/api/v1/weather/records?parcel_id=p-gdd-check&days=5")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let records = body_json(resp).await;
    let record = &records.as_array().unwrap()[0];

    // (34.0 + 22.0) / 2 - 10.0 (RICE_BASE_TEMPERATURE_CELSIUS) = 18.0
    assert!((record["daily_gdd"].as_f64().unwrap() - 18.0).abs() < 1e-6);
}

#[tokio::test]
async fn malformed_json_body_returns_4xx_not_500() {
    let app = setup_test_app().await;
    let req = Request::builder()
        .uri("/api/v1/weather/record")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from("{ this is not valid json"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert!(
        resp.status().is_client_error(),
        "malformed JSON should be a 4xx client error, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn analytics_with_insufficient_history_returns_422() {
    let app = setup_test_app().await;
    create_test_parcel(&app, "p-no-history").await;
    // Zero weather records ingested — analytics requires at least 3.

    let req = Request::builder()
        .uri("/api/v1/weather/analytics?parcel_id=p-no-history&days=60")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "INSUFFICIENT_DATA");
}

// ── Device mapping CRUD + factory preset protections ────────────────────────

#[tokio::test]
async fn create_custom_device_mapping_round_trips_with_frontend_schema_shape() {
    // Exact JSON shape the Dart `DeviceMapping.toJson()` sends -- if this
    // ever drifts from the backend struct again, this test catches it
    // immediately instead of the "add sensor" button silently failing in
    // production.
    let app = setup_test_app().await;
    let payload = serde_json::json!({
        "id": "custom-lora-01",
        "device_name": "LoRa Field Station 01",
        "manufacturer": "Custom",
        "is_preset": false,
        "date_col": "date",
        "date_format": "%Y-%m-%d",
        "metrics": [
            {"metric_type": "t_max", "column_name": "t_max", "unit": "C", "scale": 1.0},
            {"metric_type": "t_min", "column_name": "t_min", "unit": "C", "scale": 1.0},
            {"metric_type": "rainfall", "column_name": "rain", "unit": "mm", "scale": 1.0},
            {"metric_type": "radiation", "column_name": "rad", "unit": "MJ/m2", "scale": 1.0},
            {"metric_type": "humidity", "column_name": "rh", "unit": "%", "scale": 1.0},
        ],
        "created_at": "2026-06-06T00:00:00Z",
    });

    let req = Request::builder()
        .uri("/api/v1/devices/mappings")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Round-trip via GET.
    let req = Request::builder()
        .uri("/api/v1/devices/mappings/custom-lora-01")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["device_name"], "LoRa Field Station 01");
    assert_eq!(body["is_preset"], false);
}

#[tokio::test]
async fn custom_mappings_list_excludes_factory_presets() {
    let app = setup_test_app().await;
    let payload = serde_json::json!({
        "id": "custom-02", "device_name": "Custom 02", "manufacturer": "Custom", "is_preset": false,
        "date_col": "date", "date_format": "%Y-%m-%d",
        "metrics": [
            {"metric_type": "t_max", "column_name": "t_max", "unit": "C", "scale": 1.0},
            {"metric_type": "t_min", "column_name": "t_min", "unit": "C", "scale": 1.0},
            {"metric_type": "rainfall", "column_name": "rain", "unit": "mm", "scale": 1.0},
            {"metric_type": "radiation", "column_name": "rad", "unit": "MJ/m2", "scale": 1.0},
            {"metric_type": "humidity", "column_name": "rh", "unit": "%", "scale": 1.0},
        ],
        "created_at": "2026-06-06T00:00:00Z",
    });
    let req = Request::builder()
        .uri("/api/v1/devices/mappings")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::CREATED);

    let req = Request::builder()
        .uri("/api/v1/devices/mappings")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let mappings = body_json(resp).await;
    let list = mappings.as_array().unwrap();

    assert_eq!(list.len(), 1, "list_mappings must exclude the 4 factory presets");
    assert!(!list[0]["is_preset"].as_bool().unwrap());
}

#[tokio::test]
async fn creating_a_mapping_cannot_overwrite_an_existing_factory_preset() {
    let app = setup_test_app().await;
    // Attempt to smuggle in `is_preset: true` and reuse a known preset id
    // with corrupted values — must be rejected outright, not silently
    // upserted over the real preset.
    let payload = serde_json::json!({
        "id": "preset_pessl_imetos",
        "device_name": "HACKED", "manufacturer": "Attacker", "is_preset": true,
        "date_col": "x", "date_format": "%Y-%m-%d",
        "metrics": [
            {"metric_type": "t_max", "column_name": "x", "unit": "C", "scale": 1.0},
        ],
        "created_at": "2026-06-06T00:00:00Z",
    });
    let req = Request::builder()
        .uri("/api/v1/devices/mappings")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Confirm the real preset survived untouched.
    let req = Request::builder()
        .uri("/api/v1/devices/mappings/preset_pessl_imetos")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["device_name"], "Pessl iMetos 3.3 (Standard Agro)");
    assert_eq!(body["manufacturer"], "Pessl Instruments (Austria/Germany)");
}

#[tokio::test]
async fn deleting_a_factory_preset_is_blocked() {
    let app = setup_test_app().await;
    let req = Request::builder()
        .uri("/api/v1/devices/mappings/preset_davis_vantage")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_json(resp).await;
    assert!(body["error"].as_str().unwrap().contains("Cannot delete factory preset"));
}

#[tokio::test]
async fn deleting_unknown_mapping_returns_404() {
    let app = setup_test_app().await;
    let req = Request::builder()
        .uri("/api/v1/devices/mappings/does-not-exist")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn getting_unknown_mapping_returns_404() {
    let app = setup_test_app().await;
    let req = Request::builder()
        .uri("/api/v1/devices/mappings/does-not-exist")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_custom_mapping_then_delete_succeeds() {
    let app = setup_test_app().await;
    let payload = serde_json::json!({
        "id": "custom-delete-me", "device_name": "Temp", "manufacturer": "Custom", "is_preset": false,
        "date_col": "date", "date_format": "%Y-%m-%d",
        "metrics": [
            {"metric_type": "t_max", "column_name": "t_max", "unit": "C", "scale": 1.0},
        ],
        "created_at": "2026-06-06T00:00:00Z",
    });
    let req = Request::builder()
        .uri("/api/v1/devices/mappings")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::CREATED);

    let req = Request::builder()
        .uri("/api/v1/devices/mappings/custom-delete-me")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/api/v1/devices/mappings/custom-delete-me")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "deleted mapping must actually be gone");
}

// ── Parcel validation ────────────────────────────────────────────────────────

#[tokio::test]
async fn create_parcel_with_invalid_coordinates_returns_400() {
    let app = setup_test_app().await;
    let parcel = serde_json::json!({
        "id": "bad-coords",
        "name": "Invalid",
        "rice_variety": "RD43",
        "rice_ecosystem": "Irrigated",
        "latitude": 999.0,
        "longitude": 100.45,
        "planting_date": "2026-04-01",
        "area_hectares": 1.0,
    });
    let req = Request::builder()
        .uri("/api/v1/parcels")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&parcel).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "DOMAIN_VALIDATION_ERROR");
}

// ── Cron scheduler configurability ──────────────────────────────────────────

#[tokio::test]
async fn updating_cron_target_time_with_a_valid_hh_mm_is_reflected_in_app_health() {
    let app = setup_test_app().await;

    let payload = serde_json::json!({"configs": {"cron_time": "06:30"}});
    let req = Request::builder()
        .uri("/api/v1/config")
        .method("PUT")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    assert_eq!(app.clone().oneshot(req).await.unwrap().status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/api/v1/health/app")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["cron_scheduler"]["target_time"], "06:30");
}

#[tokio::test]
async fn updating_cron_target_time_with_a_malformed_value_returns_400() {
    let app = setup_test_app().await;

    for bad_value in ["25:00", "10:60", "not-a-time", "10"] {
        let payload = serde_json::json!({"configs": {"cron_time": bad_value}});
        let req = Request::builder()
            .uri("/api/v1/config")
            .method("PUT")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&payload).unwrap()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "expected 400 for '{bad_value}'");
    }
}

#[tokio::test]
async fn app_health_reports_the_default_cron_target_time_when_never_configured() {
    let app = setup_test_app().await;

    let req = Request::builder()
        .uri("/api/v1/health/app")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["cron_scheduler"]["target_time"], "23:59");
}
