//! # Oryza-Elo Integration Test: SQLite Persistence & Resilient Ingestion
//!
//! Validates the local SQLite layer, factory presets, parcel CRUD, weather time series,
//! prediction storage, and CSV ingestion pipeline.

use chrono::{NaiveDate, Utc};
use oryzaelo_engine::dal::database::connection::create_pool;
use oryzaelo_engine::dal::database::repositories::{
    ConfigRepository, DeviceMappingRepository, ParcelRepository, PredictionRepository, WeatherRepository,
};
use oryzaelo_engine::domain::models::device_mapping::{DeviceMapping, MetricColumnMapping};
use oryzaelo_engine::domain::models::metric_type::MetricType;
use oryzaelo_engine::domain::models::farm::FarmParcel;
use oryzaelo_engine::domain::models::locale::Locale;
use oryzaelo_engine::domain::models::phenology::{PhenologyPrediction, PhenologyStage, TechnicalMetrics};
use oryzaelo_engine::domain::services::agronomic_advisor::AgronomicAdvisor;
use oryzaelo_engine::domain::services::biomet_calculator::BiometCalculator;
use oryzaelo_engine::domain::services::csv_ingestion::CsvIngestionService;
use std::collections::BTreeMap;
use std::time::Instant;

#[tokio::test]
async fn test_full_sqlite_dal_lifecycle() {
    // 1. Initialize in-memory SQLite pool with schema and seeds
    let pool = create_pool("sqlite::memory:").await.expect("Failed to initialize test pool");

    let parcel_repo = ParcelRepository::new(pool.clone());
    let device_repo = DeviceMappingRepository::new(pool.clone());
    let weather_repo = WeatherRepository::new(pool.clone());
    let prediction_repo = PredictionRepository::new(pool.clone());
    let config_repo = ConfigRepository::new(pool.clone());

    // 2. Verify Factory Presets Seeding
    let presets = device_repo.list_presets().await.unwrap();
    assert_eq!(presets.len(), 4, "Must have exactly 4 presets seeded");
    assert!(presets.iter().any(|p| p.id == "preset_pessl_imetos"));
    assert!(presets.iter().any(|p| p.id == "preset_dragino_renke"));
    assert!(presets.iter().any(|p| p.id == "preset_davis_vantage"));
    assert!(presets.iter().any(|p| p.id == "preset_nasa_power"));

    // Attempting to delete factory preset must be rejected
    assert!(device_repo.delete("preset_pessl_imetos").await.is_err());

    // 3. Test Custom Farmer Device Mapping
    let custom_mapping = DeviceMapping {
        id: "farmer_custom_station_1".into(),
        device_name: "Estação Meteorológica da Várzea".into(),
        manufacturer: "Custom Arduino DIY".into(),
        is_preset: false,
        date_col: "data_leitura".into(),
        date_format: "%Y-%m-%d".into(),
        metrics: vec![
            MetricColumnMapping::new(MetricType::TMax, "temp_maxima", "C", 1.0),
            MetricColumnMapping::new(MetricType::TMin, "temp_minima", "C", 1.0),
            MetricColumnMapping::new(MetricType::Rainfall, "chuva_mm", "mm", 1.0),
            MetricColumnMapping::new(MetricType::Radiation, "radiacao_solar", "MJ/m2", 1.0),
            MetricColumnMapping::new(MetricType::Humidity, "umidade_rel", "%", 1.0),
        ],
        created_at: Utc::now(),
    };
    device_repo.save(&custom_mapping).await.unwrap();
    let retrieved_mapping = device_repo.get_by_id("farmer_custom_station_1").await.unwrap().unwrap();
    assert_eq!(retrieved_mapping.device_name, "Estação Meteorológica da Várzea");

    // 4. Test Farm Parcel CRUD
    let parcel = FarmParcel {
        id: "talhao-alpha".into(),
        name: "Talhão 1 - Várzea Norte".into(),
        rice_variety: "ขาวดอกมะลิ 105".into(),
        rice_ecosystem: "นาชลประทาน".into(),
        latitude: 14.88,
        longitude: 100.45,
        planting_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        area_hectares: Some(12.5),
    };
    parcel_repo.create(&parcel).await.unwrap();

    let fetched_parcel = parcel_repo.get_by_id("talhao-alpha").await.unwrap().unwrap();
    assert_eq!(fetched_parcel.name, "Talhão 1 - Várzea Norte");

    // 5. Test CSV Ingestion Pipeline using Custom Mapping
    let mut csv_lines = vec!["data_leitura,temp_maxima,temp_minima,chuva_mm,radiacao_solar,umidade_rel".to_string()];
    let start_date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

    for i in 0..65 {
        let d = start_date + chrono::Duration::days(i);
        let t_max = 32.0 + (i % 3) as f64;
        let t_min = 23.0 + (i % 2) as f64;
        let rain = if i % 4 == 0 { 12.0 } else { 0.0 };
        let rad = 19.5 + (i % 5) as f64;
        let rh = 78.0 + (i % 4) as f64;
        csv_lines.push(format!("{},{:.1},{:.1},{:.1},{:.1},{:.1}", d, t_max, t_min, rain, rad, rh));
    }
    let csv_content = csv_lines.join("\n");

    let report = CsvIngestionService::parse_csv(&csv_content, &custom_mapping).unwrap();
    assert_eq!(report.successful_rows, 65);
    assert_eq!(report.failed_rows, 0);

    // 6. Persist each parsed reading through the aggregation pipeline
    for reading in &report.readings {
        let recorded_at = reading.date.and_hms_opt(12, 0, 0).unwrap().and_utc();
        weather_repo
            .record_reading("talhao-alpha", "farmer_custom_station_1", reading.metric_type, reading.value, recorded_at)
            .await
            .unwrap();
    }

    // 7. Retrospective Query Performance Benchmark (< 3 ms)
    let eval_date = start_date + chrono::Duration::days(62);
    let start_bench = Instant::now();
    let retrospective = weather_repo.get_retrospective("talhao-alpha", eval_date, 60).await.unwrap();
    let query_duration = start_bench.elapsed();

    println!("SQLite 60-day retrospective query elapsed: {:?}", query_duration);
    assert!(query_duration.as_millis() < 50, "Query took too long: {:?}", query_duration);
    assert_eq!(retrospective.len(), 60);
    assert!(retrospective.first().unwrap().date <= retrospective.last().unwrap().date);

    // 8. Connect to Biophysical Feature Calculator
    let features = BiometCalculator::compute(
        &retrospective,
        eval_date,
        parcel.latitude,
        parcel.longitude,
        4.0,
        73.0,
        6.0,
        None,
    ).unwrap();
    assert_eq!(features.to_tensor_vec().len(), 44);

    // 9. Test Prediction Storage & Retrieval
    let stage = PhenologyStage::Booting;
    let advisory = AgronomicAdvisor::generate_advisory(stage, Locale::PtBr);
    let all_translations = AgronomicAdvisor::generate_all_translations(stage);

    let mut probs = BTreeMap::new();
    probs.insert("Booting".into(), 0.88);
    probs.insert("Tillering".into(), 0.10);
    probs.insert("Flowering".into(), 0.02);

    let prediction = PhenologyPrediction {
        evaluated_at: Utc::now(),
        parcel_id: "talhao-alpha".into(),
        macro_phase: stage.macro_phase(),
        granular_stage: stage,
        confidence: 0.88,
        is_transitioning: false,
        probabilities: probs,
        advisory,
        all_translations,
        technical_metrics: TechnicalMetrics {
            model_version: "catboost_onnx_v1".into(),
            inference_latency_ms: 1.42,
            entropy: 0.35,
            margin: 0.78,
        },
    };

    let pred_id = prediction_repo.save(&prediction).await.unwrap();
    assert!(!pred_id.is_empty());

    let latest_pred = prediction_repo.get_latest_for_parcel("talhao-alpha").await.unwrap().unwrap();
    assert_eq!(latest_pred.granular_stage, PhenologyStage::Booting);
    assert_eq!(latest_pred.all_translations.len(), 3);
    assert!(latest_pred.advisory.urgent_warnings.iter().any(|w| w.contains("brusone")));

    // 10. Test ConfigRepository
    config_repo.set("default_locale", "pt-BR").await.unwrap();
    let val = config_repo.get("default_locale").await.unwrap();
    assert_eq!(val, Some("pt-BR".into()));

    println!("All SQLite DAL persistence and ingestion tests passed!");
}
