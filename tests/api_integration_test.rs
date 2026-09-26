//! # Oryza-Elo End-to-End API Integration Test
//!
//! Tests complete Axum HTTP API workflow: health, parcels, device presets,
//! CSV ingestion with grandezas conversion, on-demand phenology prediction,
//! trilingual advisories, offline translations, and cron scheduler.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use oryzaelo_engine::core::settings::app_settings;
use oryzaelo_engine::dal::database::connection::create_pool;
use oryzaelo_engine::dal::inference::onnx_engine::{DEFAULT_ONNX_MODEL_PATH, OnnxInferenceEngine};
use oryzaelo_engine::domain::models::farm::FarmParcel;
use oryzaelo_engine::domain::models::phenology::PhenologyPrediction;
use oryzaelo_engine::domain::tasks::cron_scheduler::CronScheduler;
use oryzaelo_engine::presentation::api::routes::create_router;
use oryzaelo_engine::presentation::api::state::AppState;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState) {
    let pool = create_pool("sqlite::memory:").await.expect("Failed to initialize test pool");
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = repo_root.join(DEFAULT_ONNX_MODEL_PATH);
    let onnx_engine = Arc::new(OnnxInferenceEngine::new(model_path).expect("Failed to load ONNX model"));
    let settings = app_settings();

    let state = AppState::new(pool, onnx_engine, settings);
    let app = create_router(state.clone());
    (app, state)
}

#[tokio::test]
async fn test_full_api_e2e_lifecycle() {
    let (app, state) = setup_test_app().await;

    // 1. Test Differentiated Health Checks
    // 1a. Root Liveness Ping (/health)
    let req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let ping_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(ping_json["status"], "ok");

    // 1b. System Health (/api/v1/health/system)
    let req = Request::builder()
        .uri("/api/v1/health/system")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let sys_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(sys_json["status"], "healthy");
    assert_eq!(sys_json["scope"], "system");
    // Locks the exact wire contract the frontend's SystemHealth model parses.
    // A prior bug: the frontend expected these keys but the backend never
    // sent them, so the HUD silently rendered fake demo numbers on every
    // machine. These must always be present and reflect the real host.
    assert!(sys_json["os"].is_string());
    assert!(sys_json["arch"].is_string());
    assert!(sys_json["cpu_cores"].as_u64().unwrap() >= 1);
    assert!(sys_json["cpu_usage_pct"].is_number());
    assert!(sys_json["ram_total_mb"].as_u64().unwrap() > 0);
    assert!(sys_json["ram_used_mb"].is_number());
    assert!(sys_json["ram_usage_pct"].is_number());
    assert!(sys_json["disk_usage_pct"].is_number());

    // 1c. Application Health (/api/v1/health/app)
    let req = Request::builder()
        .uri("/api/v1/health/app")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let app_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(app_json["status"], "healthy");
    assert_eq!(app_json["scope"], "application");
    assert_eq!(app_json["sqlite"]["status"], "connected");
    assert_eq!(app_json["onnx_engine"]["status"], "loaded");


    // 2. Test Device Presets List
    let req = Request::builder()
        .uri("/api/v1/devices/presets")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let presets: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(presets.len(), 4);

    // 3. Create a Field Parcel (Talhão)
    let parcel = FarmParcel {
        id: "talhao-demo-1".into(),
        name: "Talhão Demonstrativo Esalq".into(),
        rice_variety: "ขาวดอกมะลิ 105".into(),
        rice_ecosystem: "นาชลประทาน".into(),
        latitude: 14.88,
        longitude: 100.45,
        planting_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        area_hectares: Some(8.0),
    };

    let req = Request::builder()
        .uri("/api/v1/parcels")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&parcel).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 4. Ingest Weather Data via JSON endpoint using Davis Preset (°F and Inches)
    // Davis preset: Date, Temp High (°F), Temp Low (°F), Rain (in), Solar Rad (MJ/m2), Hum High (%)
    let mut csv_lines = vec!["Date,Temp High,Temp Low,Rain,Solar Rad,Hum High".to_string()];
    let start_date = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();

    for i in 0..65 {
        let d = start_date + chrono::Duration::days(i);
        let date_str = d.format("%m/%d/%Y").to_string();
        let t_high_f = 89.6 + (i % 3) as f64; // ~32°C
        let t_low_f = 73.4 + (i % 2) as f64;  // ~23°C
        let rain_in = if i % 5 == 0 { 0.4 } else { 0.0 }; // 0.4 in = 10.16 mm
        let rad = 20.0 + (i % 4) as f64;
        let rh = 78.0 + (i % 5) as f64;
        csv_lines.push(format!("{},{:.1},{:.1},{:.1},{:.1},{:.1}", date_str, t_high_f, t_low_f, rain_in, rad, rh));
    }
    let csv_content = csv_lines.join("\n");

    let ingest_payload = serde_json::json!({
        "parcel_id": "talhao-demo-1",
        "device_id": "preset_davis_vantage",
        "csv_content": csv_content,
    });

    let req = Request::builder()
        .uri("/api/v1/weather/ingest")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&ingest_payload).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let ingest_res: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(ingest_res["records_persisted"], 65);
    assert_eq!(ingest_res["report"]["failed_rows"], 0);

    // 5. Query Weather Records Endpoint
    let req = Request::builder()
        .uri("/api/v1/weather/records?parcel_id=talhao-demo-1&days=10")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let records: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(records.len(), 10);
    // Verify that temperature was converted from °F (89.6°F) to Celsius (~32°C)
    let t_max: f64 = records[0]["t_max"].as_f64().unwrap();
    assert!(t_max > 30.0 && t_max < 35.0, "T_max should be in Celsius: {}", t_max);

    // 6. Execute On-Demand Phenology Prediction (Measuring E2E HTTP Latency)
    let eval_date = start_date + chrono::Duration::days(62);
    let predict_payload = serde_json::json!({
        "parcel_id": "talhao-demo-1",
        "eval_date": eval_date.to_string(),
        "locale": "pt-BR"
    });

    let t_start_http = Instant::now();
    let req = Request::builder()
        .uri("/api/v1/phenology/predict")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&predict_payload).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let http_elapsed = t_start_http.elapsed();
    println!("HTTP E2E Prediction Roundtrip Latency: {:?}", http_elapsed);

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(http_elapsed.as_millis() < 50, "E2E prediction took too long: {:?}", http_elapsed);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let pred: PhenologyPrediction = serde_json::from_slice(&body).unwrap();

    // Verify Prediction attributes
    assert_eq!(pred.parcel_id, "talhao-demo-1");
    assert_eq!(pred.probabilities.len(), 7);
    assert!(pred.confidence > 0.0 && pred.confidence <= 1.0);
    assert!(pred.technical_metrics.inference_latency_ms < 5.0);

    // Verify Trilingual Localization & Offline Translations
    assert_eq!(pred.all_translations.len(), 3);
    assert!(pred.all_translations.contains_key("pt-BR"));
    assert!(pred.all_translations.contains_key("en"));
    assert!(pred.all_translations.contains_key("th"));

    // Verify active advisory is in Portuguese
    assert_eq!(pred.advisory.locale, oryzaelo_engine::domain::models::locale::Locale::PtBr);
    assert!(!pred.advisory.headline.is_empty());
    assert!(!pred.advisory.urgent_warnings.is_empty());

    // 7. Test Latest Prediction Query with Language Switching to Thai
    let req = Request::builder()
        .uri("/api/v1/phenology/latest?parcel_id=talhao-demo-1&locale=th")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let latest_pred: PhenologyPrediction = serde_json::from_slice(&body).unwrap();

    assert_eq!(latest_pred.advisory.locale, oryzaelo_engine::domain::models::locale::Locale::Th);
    println!("Thai Headline: {}", latest_pred.advisory.headline);

    // 8. Test Edge Node Config Endpoints
    let update_config_payload = serde_json::json!({
        "configs": {
            "nightly_cron_enabled": "true",
            "cron_time": "23:59",
            "default_locale": "pt-BR"
        }
    });

    let req = Request::builder()
        .uri("/api/v1/config")
        .method("PUT")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_config_payload).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/api/v1/config")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let configs: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(configs["nightly_cron_enabled"], "true");
    assert_eq!(configs["cron_time"], "23:59");

    // 9. Test Autonomous Cron Execution & Recovery
    let eval_count = CronScheduler::execute_evaluation_for_all_parcels(
        &state.parcel_repo,
        &state.weather_repo,
        &state.prediction_repo,
        &state.onnx_engine,
        eval_date,
    )
    .await
    .unwrap();

    assert_eq!(eval_count, 1, "Should have evaluated 1 parcel autonomously");

    // 10. Test Single Weather Record Ingestion & History Query Alias
    let single_record_payload = serde_json::json!({
        "parcel_id": "talhao-demo-1",
        "date": "2026-06-06",
        "t_max": 33.5,
        "t_min": 22.5,
        "precipitation_mm": 5.0,
        "radiation_mj_m2": 21.0,
        "relative_humidity_pct": 82.0,
        "source": "LoRaWAN_Station_A"
    });

    let req = Request::builder()
        .uri("/api/v1/weather/record")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&single_record_payload).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let req = Request::builder()
        .uri("/api/v1/weather/history?parcel_id=talhao-demo-1&days=10")
        .method("GET")
        .body(Body::empty())
        .unwrap();


    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let history_records: Value = serde_json::from_slice(&body).unwrap();
    assert!(history_records.as_array().unwrap().len() >= 1);

    // 11. Test Granular Real-Time Edge Benchmarks (/api/v1/benchmarks/*)
    // 11a. Latency Benchmark (/benchmarks/latency)
    let req = Request::builder()
        .uri("/api/v1/benchmarks/latency?iterations=50")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let benchmark: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(benchmark["status"], "passed");
    assert_eq!(benchmark["iterations"], 50);
    assert!(benchmark["mean_latency_ms"].as_f64().unwrap() < 5.0);
    println!(
        "Live Edge Latency Benchmark: Mean: {:.4} ms, Speedup vs 5ms: {:.1}x",
        benchmark["mean_latency_ms"].as_f64().unwrap(),
        benchmark["speedup_vs_edge_ceiling"].as_f64().unwrap()
    );

    // 11b. Biomet Benchmark (/benchmarks/biomet)
    let req = Request::builder()
        .uri("/api/v1/benchmarks/biomet?iterations=50")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let biomet_bench: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(biomet_bench["status"], "passed");
    assert_eq!(biomet_bench["features_computed"], 44);
    assert!(biomet_bench["mean_duration_ms"].as_f64().unwrap() < 1.0);
    println!(
        "Live Biomet Feature Benchmark: Mean: {:.4} ms ({:.2} µs), Speedup vs 1ms: {:.1}x",
        biomet_bench["mean_duration_ms"].as_f64().unwrap(),
        biomet_bench["mean_duration_us"].as_f64().unwrap(),
        biomet_bench["speedup_vs_target"].as_f64().unwrap()
    );

    // 11c. Storage Benchmark (/benchmarks/storage)
    let req = Request::builder()
        .uri("/api/v1/benchmarks/storage")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let storage_bench: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(storage_bench["status"], "passed");
    println!(
        "Live Storage Benchmark: Query Latency: {:.4} ms, Total Weather Records: {}",
        storage_bench["query_latency_ms"].as_f64().unwrap(),
        storage_bench["total_weather_records"].as_i64().unwrap()
    );

    // 11d. Throughput Benchmark (/benchmarks/throughput)
    let req = Request::builder()
        .uri("/api/v1/benchmarks/throughput?iterations=100")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let tp_bench: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(tp_bench["status"], "passed");
    println!(
        "Live Throughput Benchmark: {:.1} inferences/sec (batch duration: {:.2} ms)",
        tp_bench["inferences_per_second"].as_f64().unwrap(),
        tp_bench["total_duration_ms"].as_f64().unwrap()
    );

    // 12. Test Scalar API Reference and OpenAPI 3.1 JSON Specification
    // 12a. Root /docs (Scalar HTML)
    let req = Request::builder()
        .uri("/docs")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html_body = String::from_utf8(resp.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(html_body.contains("@scalar/api-reference"));
    assert!(html_body.contains("id=\"api-reference\""));
    assert!(html_body.contains("Oryza-Elo Edge Engine API Reference"));

    // 12b. OpenAPI 3.1 JSON Endpoint (/openapi.json)
    let req = Request::builder()
        .uri("/openapi.json")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let openapi_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(openapi_json["openapi"], "3.1.0");
    assert_eq!(openapi_json["info"]["title"], "Oryza-Elo Edge Engine API");
    assert!(openapi_json["paths"]["/api/v1/weather/sensor-readings"].is_object());

    // 12c. API v1 docs alias (/api/v1/docs)
    let req = Request::builder()
        .uri("/api/v1/docs")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    println!("All E2E API integration tests passed successfully!");
}

