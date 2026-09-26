//! # Oryza-Elo Architecture Guardrail: Mock Data Domain Service
//!
//! Pure Rust implementation for high-fidelity synthetic agronomic data generation
//! and database resetting. Eliminates external Python runtimes on edge hardware (Raspberry Pi).

use crate::core::error::AppError;
use crate::domain::models::device_mapping::MetricColumnMapping;
use crate::domain::models::locale::Locale;
use crate::domain::models::metric_type::MetricType;
use crate::domain::models::phenology::{PhenologyStage, TechnicalMetrics};
use crate::domain::services::agronomic_advisor::AgronomicAdvisor;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, SqlitePool};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockDataSummary {
    pub status: String,
    pub message: String,
    pub parcels_count: usize,
    pub weather_records_count: usize,
    pub sensor_readings_count: usize,
    pub predictions_count: usize,
    pub device_mappings_count: usize,
    pub config_entries_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockCleanSummary {
    pub status: String,
    pub message: String,
    pub parcels_remaining: usize,
    pub weather_records_remaining: usize,
    pub sensor_readings_remaining: usize,
    pub predictions_remaining: usize,
    pub device_mappings_remaining: usize,
}

pub struct MockDataService;

struct ParcelPreset {
    id: &'static str,
    name: &'static str,
    rice_variety: &'static str,
    rice_ecosystem: &'static str,
    latitude: f64,
    longitude: f64,
    days_ago: i64,
    area_hectares: f64,
    source: &'static str,
}

impl MockDataService {
    /// Populates all edge SQLite tables with high-fidelity realistic synthetic data.
    pub async fn populate(
        pool: &SqlitePool,
        max_days: Option<usize>,
        parcel_count: Option<usize>,
    ) -> Result<MockDataSummary, AppError> {
        let max_days = max_days.unwrap_or(75);
        let parcel_count = parcel_count.unwrap_or(4).min(4).max(1);

        let now = Utc::now();
        let today = now.date_naive();
        let now_iso = now.to_rfc3339();

        let presets = [
            ParcelPreset {
                id: "talhao-esalq-central",
                name: "Talhão Central Esalq (Suphan Buri)",
                rice_variety: "ขาวดอกมะลิ 105",
                rice_ecosystem: "นาชลประทาน",
                latitude: 14.4745,
                longitude: 100.1177,
                days_ago: 65,
                area_hectares: 15.0,
                source: "preset_davis_vantage",
            },
            ParcelPreset {
                id: "talhao-varzea-chiangmai",
                name: "Talhão Várzea do Vale (Chiang Mai)",
                rice_variety: "กข43",
                rice_ecosystem: "นาชลประทาน",
                latitude: 18.7904,
                longitude: 98.9847,
                days_ago: 40,
                area_hectares: 8.5,
                source: "preset_pessl_imetos",
            },
            ParcelPreset {
                id: "talhao-sequeiro-khonkaen",
                name: "Talhão Sequeiro Nordeste (Khon Kaen)",
                rice_variety: "กข79",
                rice_ecosystem: "นาน้ำฝน",
                latitude: 16.4322,
                longitude: 102.8236,
                days_ago: 95,
                area_hectares: 22.0,
                source: "preset_dragino_renke",
            },
            ParcelPreset {
                id: "talhao-piloto-ubon",
                name: "Talhão Piloto Experimental (Ubon Ratchathani)",
                rice_variety: "ขาวดอกมะลิ 105",
                rice_ecosystem: "นาชลประทาน",
                latitude: 15.2448,
                longitude: 104.8473,
                days_ago: 18,
                area_hectares: 5.0,
                source: "preset_nasa_power",
            },
        ];

        // 1. Inserir Custom Sensor Profiles: 2 bundled multi-metric stations
        //    (matching real commercial products that report all 5 canonical
        //    metrics from one combined feed) plus 2 standalone single-metric
        //    sensors, demonstrating the real topology this schema supports.
        let bundled_stations = [
            (
                "mapping_dragino_sol_nascente",
                "Estação LoRaWAN Fazenda Sol Nascente",
                "Dragino Technology",
                "Timestamp",
                "%Y-%m-%d %H:%M:%S",
                [
                    ("AirTemp_Max_C", "C", 1.0),
                    ("AirTemp_Min_C", "C", 1.0),
                    ("Precip_Accum_mm", "mm", 1.0),
                    ("Pyranometer_Solar_W_m2", "W/m2", 0.0864),
                    ("AirHumidity_Pct", "%", 1.0),
                ],
            ),
            (
                "mapping_campbell_pesquisa",
                "Estação Micrometeorológica Campbell CR1000X",
                "Campbell Scientific",
                "TIMESTAMP",
                "%d/%m/%Y",
                [
                    ("AirTC_Max", "C", 1.0),
                    ("AirTC_Min", "C", 1.0),
                    ("Rain_mm_Tot", "mm", 1.0),
                    ("SlrMJ_Tot", "MJ/m2", 1.0),
                    ("RH_Max", "%", 1.0),
                ],
            ),
        ];

        for d in &bundled_stations {
            let metrics = vec![
                MetricColumnMapping::new(MetricType::TMax, d.5[0].0, d.5[0].1, d.5[0].2),
                MetricColumnMapping::new(MetricType::TMin, d.5[1].0, d.5[1].1, d.5[1].2),
                MetricColumnMapping::new(MetricType::Rainfall, d.5[2].0, d.5[2].1, d.5[2].2),
                MetricColumnMapping::new(MetricType::Radiation, d.5[3].0, d.5[3].1, d.5[3].2),
                MetricColumnMapping::new(MetricType::Humidity, d.5[4].0, d.5[4].1, d.5[4].2),
            ];
            Self::upsert_sensor_profile(pool, d.0, d.1, d.2, d.3, d.4, &metrics, &now_iso).await?;
        }

        let standalone_sensors = [
            (
                "sensor_pluviometro_avulso",
                "Pluviômetro Basculante Avulso (Portão Norte)",
                "Generic LoRaWAN",
                "ts",
                "%Y-%m-%d",
                MetricType::Rainfall,
                "rain_mm",
                "mm",
                1.0,
            ),
            (
                "sensor_piranometro_avulso",
                "Piranômetro Solar Avulso (Mastro Central)",
                "Generic LoRaWAN",
                "ts",
                "%Y-%m-%d",
                MetricType::Radiation,
                "solar_w_m2",
                "W/m2",
                0.0864,
            ),
            (
                "sensor_termohigrometro_talhao",
                "Termo-Higrômetro Digital de Campo (SHT31)",
                "Sensirion / LoRaWAN",
                "ts",
                "%Y-%m-%d %H:%M:%S",
                MetricType::TMax,
                "temp_c",
                "C",
                1.0,
            ),
        ];

        for s in &standalone_sensors {
            let metrics = vec![MetricColumnMapping::new(s.5, s.6, s.7, s.8)];
            Self::upsert_sensor_profile(pool, s.0, s.1, s.2, s.3, s.4, &metrics, &now_iso).await?;
        }

        // 2. Inserir Edge Node Configs
        let edge_configs = [
            ("nightly_cron_enabled", "true"),
            ("cron_time", "23:59"),
            ("default_locale", "pt-BR"),
            ("biomet_window_days", "60"),
            ("thermal_base_temp", "10.0"),
            ("node_operational_mode", "edge_standalone"),
            ("edge_cache_retention_days", "180"),
            ("alert_push_enabled", "true"),
        ];

        for (k, v) in &edge_configs {
            sqlx::query(
                r#"
                INSERT INTO edge_config (key, value, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(k)
            .bind(v)
            .bind(&now_iso)
            .execute(pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        // 3. Inserir Talhões, Séries Climáticas e Inferências
        let mut _total_weather = 0;
        let mut _total_predictions = 0;

        for p in &presets[..parcel_count] {
            let sowing_date = today - Duration::days(p.days_ago);
            let sowing_iso = sowing_date.to_string();

            sqlx::query(
                r#"
                INSERT INTO parcels (
                    id, name, rice_variety, rice_ecosystem,
                    latitude, longitude, planting_date, area_hectares,
                    created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    rice_variety = excluded.rice_variety,
                    rice_ecosystem = excluded.rice_ecosystem,
                    planting_date = excluded.planting_date,
                    area_hectares = excluded.area_hectares,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(p.id)
            .bind(p.name)
            .bind(p.rice_variety)
            .bind(p.rice_ecosystem)
            .bind(p.latitude)
            .bind(p.longitude)
            .bind(&sowing_iso)
            .bind(p.area_hectares)
            .bind(&now_iso)
            .bind(&now_iso)
            .execute(pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            // Séries temporais meteorológicas coerentes
            let days_span = (p.days_ago + 1).min(max_days as i64) as usize;
            for d in 0..days_span {
                let rec_date = sowing_date + Duration::days(d as i64);
                let rec_iso = rec_date.to_string();

                // Onda simulada realista de monções
                let phase = (d as f64 / 30.0) * std::f64::consts::PI;
                let wave = phase.sin();

                let base_max = 32.5 + 2.0 * wave;
                let base_min = 23.0 + 1.2 * wave;

                let pseudo_rand = ((d * 17 + p.days_ago as usize * 31) % 100) as f64 / 100.0;
                let t_max = base_max + (pseudo_rand * 3.7 - 1.5);
                let mut t_min = base_min + ((1.0 - pseudo_rand) * 3.0 - 1.8);

                if t_min >= t_max {
                    t_min = t_max - 5.5;
                }

                let is_rain = (d % 4 == 0) || (pseudo_rand > 0.72);
                let (precip, rad, rh) = if is_rain {
                    (
                        10.0 + pseudo_rand * 35.0,
                        13.5 + pseudo_rand * 4.0,
                        84.0 + pseudo_rand * 12.0,
                    )
                } else {
                    (
                        0.0,
                        19.0 + pseudo_rand * 5.0,
                        65.0 + pseudo_rand * 14.0,
                    )
                };

                let t_max = (t_max * 100.0).round() / 100.0;
                let t_min = (t_min * 100.0).round() / 100.0;
                let precip = (precip * 10.0).round() / 10.0;
                let rad = (rad * 10.0).round() / 10.0;
                let rh = (rh.min(100.0) * 10.0).round() / 10.0;
                let recorded_at = format!("{}T12:00:00Z", rec_iso);

                // 1. Primary bundled station readings (sync at 12:00:00Z)
                for (table, value) in [
                    ("sensor_readings_t_max", t_max),
                    ("sensor_readings_t_min", t_min),
                    ("sensor_readings_rainfall", precip),
                    ("sensor_readings_radiation", rad),
                    ("sensor_readings_humidity", rh),
                ] {
                    let q = format!(
                        "INSERT INTO {table} (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                    );
                    sqlx::query(&q)
                        .bind(p.id)
                        .bind(p.source)
                        .bind(value)
                        .bind(&recorded_at)
                        .bind(&now_iso)
                        .execute(pool)
                        .await
                        .map_err(|e| AppError::Database(e.to_string()))?;
                }

                // 2. Standalone Rain Gauge (sensor_pluviometro_avulso):
                // Reports at 06:30:00Z and 18:45:00Z with sub-daily precipitation
                let rain_morn = if is_rain { ((precip * 0.45) * 10.0).round() / 10.0 } else { 0.0 };
                let rain_eve = if is_rain { ((precip * 0.55) * 10.0).round() / 10.0 } else { 0.0 };
                for (time_str, val) in [
                    (format!("{}T06:30:00Z", rec_iso), rain_morn),
                    (format!("{}T18:45:00Z", rec_iso), rain_eve),
                ] {
                    sqlx::query(
                        "INSERT INTO sensor_readings_rainfall (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                    )
                    .bind(p.id)
                    .bind("sensor_pluviometro_avulso")
                    .bind(val)
                    .bind(&time_str)
                    .bind(&now_iso)
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;
                }

                // 3. Standalone Solar Pyranometer (sensor_piranometro_avulso):
                // Reports at 09:15:00Z and 14:30:00Z
                let rad_morn = ((rad * 0.42) * 10.0).round() / 10.0;
                let rad_aft = ((rad * 0.95) * 10.0).round() / 10.0;
                for (time_str, val) in [
                    (format!("{}T09:15:00Z", rec_iso), rad_morn),
                    (format!("{}T14:30:00Z", rec_iso), rad_aft),
                ] {
                    sqlx::query(
                        "INSERT INTO sensor_readings_radiation (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                    )
                    .bind(p.id)
                    .bind("sensor_piranometro_avulso")
                    .bind(val)
                    .bind(&time_str)
                    .bind(&now_iso)
                    .execute(pool)
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;
                }

                // 4. Digital Field Thermo-Hygrometer (sensor_termohigrometro_talhao):
                // Reports early morning minimum temp + humidity, and afternoon peak temp + humidity
                let morn_time = format!("{}T05:30:00Z", rec_iso);
                let aft_time = format!("{}T15:00:00Z", rec_iso);
                let rh_morn = ((rh * 1.08).min(99.0) * 10.0).round() / 10.0;
                let rh_aft = ((rh * 0.82).max(35.0) * 10.0).round() / 10.0;

                sqlx::query(
                    "INSERT INTO sensor_readings_t_min (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(p.id)
                .bind("sensor_termohigrometro_talhao")
                .bind(t_min)
                .bind(&morn_time)
                .bind(&now_iso)
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                sqlx::query(
                    "INSERT INTO sensor_readings_humidity (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(p.id)
                .bind("sensor_termohigrometro_talhao")
                .bind(rh_morn)
                .bind(&morn_time)
                .bind(&now_iso)
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                sqlx::query(
                    "INSERT INTO sensor_readings_t_max (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(p.id)
                .bind("sensor_termohigrometro_talhao")
                .bind(t_max)
                .bind(&aft_time)
                .bind(&now_iso)
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                sqlx::query(
                    "INSERT INTO sensor_readings_humidity (parcel_id, sensor_id, value, recorded_at, received_at) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(p.id)
                .bind("sensor_termohigrometro_talhao")
                .bind(rh_aft)
                .bind(&aft_time)
                .bind(&now_iso)
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                // Aggregate row with multi-sensor provenance
                sqlx::query(
                    r#"
                    INSERT INTO weather_records (
                        parcel_id, record_date, t_max, t_min,
                        precipitation_mm, radiation_mj_m2, relative_humidity_pct,
                        t_max_sensor_id, t_min_sensor_id, rainfall_sensor_id, radiation_sensor_id, humidity_sensor_id,
                        is_partial, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)
                    ON CONFLICT(parcel_id, record_date) DO UPDATE SET
                        t_max = excluded.t_max,
                        t_min = excluded.t_min,
                        precipitation_mm = excluded.precipitation_mm,
                        radiation_mj_m2 = excluded.radiation_mj_m2,
                        relative_humidity_pct = excluded.relative_humidity_pct,
                        t_max_sensor_id = excluded.t_max_sensor_id,
                        t_min_sensor_id = excluded.t_min_sensor_id,
                        rainfall_sensor_id = excluded.rainfall_sensor_id,
                        radiation_sensor_id = excluded.radiation_sensor_id,
                        humidity_sensor_id = excluded.humidity_sensor_id,
                        is_partial = 0,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(p.id)
                .bind(&rec_iso)
                .bind(t_max)
                .bind(t_min)
                .bind(precip)
                .bind(rad)
                .bind(rh)
                .bind(p.source)
                .bind("sensor_termohigrometro_talhao")
                .bind("sensor_pluviometro_avulso")
                .bind("sensor_piranometro_avulso")
                .bind(p.source)
                .bind(&now_iso)
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                _total_weather += 1;
            }

            // Inferências fenológicas a cada 7 dias
            let mut das = 7i64;
            while das <= p.days_ago {
                let eval_date = sowing_date + Duration::days(das);
                let eval_iso = format!("{}T23:59:00Z", eval_date);

                let stage = Self::get_stage_by_das(das);
                let macro_phase = stage.macro_phase();
                let confidence = 0.84 + ((das as f64 * 3.1) % 10.0) / 100.0;
                let is_trans = if [19, 47, 67, 77, 87].contains(&das) { 1 } else { 0 };

                let probs = Self::build_stage_probabilities(stage, confidence);
                let probs_json = serde_json::to_string(&probs)?;

                let advisory = AgronomicAdvisor::generate_advisory(stage, Locale::PtBr);
                let all_translations = AgronomicAdvisor::generate_all_translations(stage);

                let advisory_envelope = serde_json::json!({
                    "advisory": advisory,
                    "all_translations": all_translations,
                });
                let adv_json = serde_json::to_string(&advisory_envelope)?;

                let metrics = TechnicalMetrics {
                    model_version: "catboost_onnx_v1".to_string(),
                    inference_latency_ms: 0.0218,
                    entropy: 0.284,
                    margin: confidence - 0.08,
                };
                let metrics_json = serde_json::to_string(&metrics)?;

                let pred_id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO prediction_history (
                        id, parcel_id, evaluated_at, macro_phase, granular_stage,
                        confidence, is_transitioning, probabilities_json, advisory_json, metrics_json
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(&pred_id)
                .bind(p.id)
                .bind(&eval_iso)
                .bind(macro_phase.code())
                .bind(stage.english_name())
                .bind(confidence)
                .bind(is_trans)
                .bind(&probs_json)
                .bind(&adv_json)
                .bind(&metrics_json)
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                _total_predictions += 1;
                das += 7;
            }
        }

        // Auditoria final de contagens
        let row_p: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM parcels").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_w: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM weather_records").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_h: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM prediction_history").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_d: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_mappings").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_c: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM edge_config").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;

        let mut total_sensor_readings: i64 = 0;
        for table in [
            "sensor_readings_t_max",
            "sensor_readings_t_min",
            "sensor_readings_rainfall",
            "sensor_readings_radiation",
            "sensor_readings_humidity",
        ] {
            let row_s: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
            total_sensor_readings += row_s.0;
        }

        Ok(MockDataSummary {
            status: "success".to_string(),
            message: format!(
                "Base de borda populada com sucesso: {} talhões, {} registros climáticos, {} leituras de sensores e {} predições.",
                row_p.0, row_w.0, total_sensor_readings, row_h.0
            ),
            parcels_count: row_p.0 as usize,
            weather_records_count: row_w.0 as usize,
            sensor_readings_count: total_sensor_readings as usize,
            predictions_count: row_h.0 as usize,
            device_mappings_count: row_d.0 as usize,
            config_entries_count: row_c.0 as usize,
        })
    }

    /// Cleans test data, resetting database to factory defaults while preserving presets.
    pub async fn clean(pool: &SqlitePool, reset_presets: bool) -> Result<MockCleanSummary, AppError> {
        for table in [
            "sensor_readings_t_max",
            "sensor_readings_t_min",
            "sensor_readings_rainfall",
            "sensor_readings_radiation",
            "sensor_readings_humidity",
        ] {
            sqlx::query(&format!("DELETE FROM {table};")).execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        }
        sqlx::query("DELETE FROM weather_records;").execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        sqlx::query("DELETE FROM prediction_history;").execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        sqlx::query("DELETE FROM parcels;").execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;

        if reset_presets {
            sqlx::query("DELETE FROM device_mappings;").execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
            crate::dal::database::connection::seed_factory_presets(pool).await?;
        } else {
            sqlx::query("DELETE FROM device_mappings WHERE is_preset = 0;").execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        }

        sqlx::query("DELETE FROM edge_config;").execute(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let _ = pool.execute("VACUUM;").await;

        let row_p: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM parcels").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_w: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM weather_records").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_h: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM prediction_history").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
        let row_d: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_mappings").fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;

        let mut sensor_readings_remaining = 0;
        for table in [
            "sensor_readings_t_max",
            "sensor_readings_t_min",
            "sensor_readings_rainfall",
            "sensor_readings_radiation",
            "sensor_readings_humidity",
        ] {
            let row_s: (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(pool).await.map_err(|e| AppError::Database(e.to_string()))?;
            sensor_readings_remaining += row_s.0 as usize;
        }

        Ok(MockCleanSummary {
            status: "success".to_string(),
            message: "Banco de dados limpo com sucesso. Presets oficiais preservados.".to_string(),
            parcels_remaining: row_p.0 as usize,
            weather_records_remaining: row_w.0 as usize,
            sensor_readings_remaining,
            predictions_remaining: row_h.0 as usize,
            device_mappings_remaining: row_d.0 as usize,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn upsert_sensor_profile(
        pool: &SqlitePool,
        id: &str,
        device_name: &str,
        manufacturer: &str,
        date_col: &str,
        date_format: &str,
        metrics: &[MetricColumnMapping],
        now_iso: &str,
    ) -> Result<(), AppError> {
        let metrics_json = serde_json::to_string(metrics)
            .map_err(|e| AppError::Database(format!("Failed to serialize demo sensor metrics: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO device_mappings (
                id, device_name, manufacturer, is_preset, date_col, date_format, metrics_json, created_at
            ) VALUES (?, ?, ?, 0, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                device_name = excluded.device_name,
                manufacturer = excluded.manufacturer,
                metrics_json = excluded.metrics_json
            "#,
        )
        .bind(id)
        .bind(device_name)
        .bind(manufacturer)
        .bind(date_col)
        .bind(date_format)
        .bind(&metrics_json)
        .bind(now_iso)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_stage_by_das(das: i64) -> PhenologyStage {
        if das < 20 {
            PhenologyStage::Seedling
        } else if das < 48 {
            PhenologyStage::Tillering
        } else if das < 68 {
            PhenologyStage::Booting
        } else if das < 78 {
            PhenologyStage::Heading
        } else if das < 88 {
            PhenologyStage::Flowering
        } else if das < 110 {
            PhenologyStage::PreHarvest
        } else {
            PhenologyStage::HarvestReady
        }
    }

    fn build_stage_probabilities(active: PhenologyStage, confidence: f64) -> BTreeMap<String, f64> {
        let mut map = BTreeMap::new();
        let stages = [
            PhenologyStage::Seedling,
            PhenologyStage::Tillering,
            PhenologyStage::Booting,
            PhenologyStage::Heading,
            PhenologyStage::Flowering,
            PhenologyStage::PreHarvest,
            PhenologyStage::HarvestReady,
        ];

        let conf = (confidence * 1000.0).round() / 1000.0;
        let remaining = (1.0 - conf).max(0.001);
        let per_other = (remaining / 6.0 * 1000.0).round() / 1000.0;

        for s in stages {
            let key = s.english_name().to_lowercase().replace(" ", "_");
            if s == active {
                map.insert(key, conf);
            } else {
                map.insert(key, per_other);
            }
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dal::database::connection::create_pool;

    #[tokio::test]
    async fn test_mock_data_service_populate_and_clean() {
        let pool = create_pool("sqlite::memory:").await.unwrap();

        let pop = MockDataService::populate(&pool, Some(30), Some(2)).await.unwrap();
        assert_eq!(pop.parcels_count, 2);
        assert!(pop.weather_records_count > 30);
        assert!(pop.sensor_readings_count > pop.weather_records_count);
        assert!(pop.predictions_count > 0);

        let clean = MockDataService::clean(&pool, false).await.unwrap();
        assert_eq!(clean.parcels_remaining, 0);
        assert_eq!(clean.weather_records_remaining, 0);
        assert_eq!(clean.sensor_readings_remaining, 0);
        assert_eq!(clean.predictions_remaining, 0);
        assert_eq!(clean.device_mappings_remaining, 4, "Must keep 4 factory presets");
    }
}
