//! # Oryza-Elo Architecture Guardrail: Weather Analytics & Biomet Statistics Service
//!
//! Provides deterministic agrometeorological correlation matrices, biomet radar indices,
//! time-series statistical aggregations, and dynamic trilingual agronomic alerts (pt-BR, en, th)
//! computed on the edge node with zero network dependency.

use crate::core::error::{AppError, DomainError};
use crate::core::settings::RICE_BASE_TEMPERATURE_CELSIUS;
use crate::domain::models::locale::Locale;
use crate::domain::models::weather::DailyWeatherRecord;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrelationCell {
    pub row_variable: String,
    pub col_variable: String,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub variables: Vec<String>,
    pub matrix: Vec<Vec<f64>>,
    pub pairs: Vec<CorrelationCell>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RadarDimensionScore {
    pub key: String,
    pub label: String,
    pub score: f64, // 0.0 to 100.0
    pub benchmark_optimal: f64,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiometSummary {
    pub total_records: usize,
    pub total_gdd: f64,
    pub total_rain_mm: f64,
    pub max_rain_mm: f64,
    pub mean_t_max_celsius: f64,
    pub mean_t_min_celsius: f64,
    pub mean_dtr_celsius: f64,
    pub mean_radiation_mj_m2: f64,
    pub mean_relative_humidity_pct: f64,
    pub consecutive_dry_days: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DynamicAdvisoryAlert {
    pub level: String,     // "critical", "warning", "info"
    pub category: String,  // "thermal", "water", "phytosanitary", "radiation"
    pub title: String,
    pub message: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherAnalyticsReport {
    pub parcel_id: String,
    pub evaluation_date: NaiveDate,
    pub window_days: usize,
    pub summary: BiometSummary,
    pub correlation_matrix: CorrelationMatrix,
    pub biomet_radar: Vec<RadarDimensionScore>,
    pub dynamic_advisories: BTreeMap<String, Vec<DynamicAdvisoryAlert>>,
}

pub struct WeatherAnalyticsService;

impl WeatherAnalyticsService {
    /// Computes full agrometeorological analytics for a parcel's weather history.
    pub fn compute_analytics(
        parcel_id: &str,
        records: &[DailyWeatherRecord],
        eval_date: NaiveDate,
        window_days: usize,
    ) -> Result<WeatherAnalyticsReport, AppError> {
        let mut slice: Vec<&DailyWeatherRecord> = records
            .iter()
            .filter(|r| r.date <= eval_date)
            .collect();

        if slice.len() < 3 {
            return Err(AppError::Domain(DomainError::InsufficientWeatherData {
                required: 3,
                available: slice.len(),
            }));
        }

        slice.sort_by_key(|r| r.date);

        // Keep at most window_days
        let n = slice.len();
        let start_idx = if n > window_days { n - window_days } else { 0 };
        let window_slice = &slice[start_idx..];

        let summary = Self::compute_summary(window_slice);
        let correlation_matrix = Self::compute_correlation_matrix(window_slice);
        let biomet_radar = Self::compute_radar(&summary);
        let dynamic_advisories = Self::generate_dynamic_alerts(&summary);

        Ok(WeatherAnalyticsReport {
            parcel_id: parcel_id.to_string(),
            evaluation_date: eval_date,
            window_days,
            summary,
            correlation_matrix,
            biomet_radar,
            dynamic_advisories,
        })
    }

    /// Aggregates biometric summary metrics.
    fn compute_summary(records: &[&DailyWeatherRecord]) -> BiometSummary {
        let n = records.len() as f64;
        let mut total_gdd = 0.0;
        let mut total_rain = 0.0;
        let mut max_rain = 0.0;
        let mut sum_t_max = 0.0;
        let mut sum_t_min = 0.0;
        let mut sum_dtr = 0.0;
        let mut sum_rad = 0.0;
        let mut sum_rh = 0.0;
        let mut cdd_count = 0;

        for r in records {
            total_gdd += r.daily_gdd(RICE_BASE_TEMPERATURE_CELSIUS);
            total_rain += r.precipitation_mm;
            if r.precipitation_mm > max_rain {
                max_rain = r.precipitation_mm;
            }
            if r.precipitation_mm < 1.0 {
                cdd_count += 1;
            }
            sum_t_max += r.t_max;
            sum_t_min += r.t_min;
            sum_dtr += r.dtr();
            sum_rad += r.radiation_mj_m2;
            sum_rh += r.relative_humidity_pct;
        }

        BiometSummary {
            total_records: records.len(),
            total_gdd: (total_gdd * 100.0).round() / 100.0,
            total_rain_mm: (total_rain * 100.0).round() / 100.0,
            max_rain_mm: (max_rain * 100.0).round() / 100.0,
            mean_t_max_celsius: ((sum_t_max / n) * 10.0).round() / 10.0,
            mean_t_min_celsius: ((sum_t_min / n) * 10.0).round() / 10.0,
            mean_dtr_celsius: ((sum_dtr / n) * 10.0).round() / 10.0,
            mean_radiation_mj_m2: ((sum_rad / n) * 10.0).round() / 10.0,
            mean_relative_humidity_pct: ((sum_rh / n) * 10.0).round() / 10.0,
            consecutive_dry_days: cdd_count,
        }
    }

    /// Computes Pearson correlation matrix between all 6 environmental drivers.
    fn compute_correlation_matrix(records: &[&DailyWeatherRecord]) -> CorrelationMatrix {
        let variables = vec![
            "T_max".to_string(),
            "T_min".to_string(),
            "DTR".to_string(),
            "Rain".to_string(),
            "Radiation".to_string(),
            "Humidity".to_string(),
        ];

        let series: Vec<Vec<f64>> = vec![
            records.iter().map(|r| r.t_max).collect(),
            records.iter().map(|r| r.t_min).collect(),
            records.iter().map(|r| r.dtr()).collect(),
            records.iter().map(|r| r.precipitation_mm).collect(),
            records.iter().map(|r| r.radiation_mj_m2).collect(),
            records.iter().map(|r| r.relative_humidity_pct).collect(),
        ];

        let num_vars = variables.len();
        let mut matrix = vec![vec![1.0; num_vars]; num_vars];
        let mut pairs = Vec::with_capacity(num_vars * num_vars);

        for i in 0..num_vars {
            for j in 0..num_vars {
                let r_val = if i == j {
                    1.0
                } else {
                    Self::pearson_correlation(&series[i], &series[j])
                };
                let rounded = (r_val * 100.0).round() / 100.0;
                matrix[i][j] = rounded;
                pairs.push(CorrelationCell {
                    row_variable: variables[i].clone(),
                    col_variable: variables[j].clone(),
                    coefficient: rounded,
                });
            }
        }

        CorrelationMatrix {
            variables,
            matrix,
            pairs,
        }
    }

    /// Calculates Pearson correlation coefficient $r \in [-1.0, 1.0]$.
    fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
        let n = x.len() as f64;
        if n < 2.0 {
            return 0.0;
        }

        let mean_x = x.iter().sum::<f64>() / n;
        let mean_y = y.iter().sum::<f64>() / n;

        let mut numer = 0.0;
        let mut denom_x = 0.0;
        let mut denom_y = 0.0;

        for i in 0..x.len() {
            let dx = x[i] - mean_x;
            let dy = y[i] - mean_y;
            numer += dx * dy;
            denom_x += dx * dx;
            denom_y += dy * dy;
        }

        let denom = (denom_x * denom_y).sqrt();
        if denom < 1e-9 {
            0.0
        } else {
            (numer / denom).clamp(-1.0, 1.0)
        }
    }

    /// Calculates Biomet Multidimensional Radar Index normalized to rice physiology.
    fn compute_radar(summary: &BiometSummary) -> Vec<RadarDimensionScore> {
        // 1. Thermal Suitability (Optimum 25-32°C)
        let mean_temp = (summary.mean_t_max_celsius + summary.mean_t_min_celsius) / 2.0;
        let thermal_score = if (22.0..=32.0).contains(&mean_temp) {
            100.0 - (mean_temp - 27.0).abs() * 5.0
        } else if mean_temp < 22.0 {
            (50.0 + (mean_temp - 15.0) * 7.0).clamp(0.0, 100.0)
        } else {
            (100.0 - (mean_temp - 32.0) * 15.0).clamp(0.0, 100.0)
        };

        // 2. Radiation Energy (Optimum >= 18 MJ/m²/day)
        let rad_score = ((summary.mean_radiation_mj_m2 / 20.0) * 100.0).clamp(0.0, 100.0);

        // 3. Water Security (Based on total rainfall and dry days penalty)
        let rain_base = (summary.total_rain_mm / 100.0 * 80.0).clamp(20.0, 100.0);
        let dry_penalty = (summary.consecutive_dry_days as f64 * 3.0).min(40.0);
        let water_score = (rain_base - dry_penalty).clamp(10.0, 100.0);

        // 4. Humidity Balance (Optimum 70-85%)
        let rh = summary.mean_relative_humidity_pct;
        let rh_score = if (70.0..=85.0).contains(&rh) {
            95.0
        } else if rh < 70.0 {
            (95.0 - (70.0 - rh) * 2.0).clamp(20.0, 100.0)
        } else {
            (95.0 - (rh - 85.0) * 3.0).clamp(20.0, 100.0)
        };

        // 5. Thermal Stability (Optimum DTR 7-10°C)
        let dtr = summary.mean_dtr_celsius;
        let dtr_score = if (6.0..=11.0).contains(&dtr) {
            95.0
        } else if dtr < 6.0 {
            80.0
        } else {
            (95.0 - (dtr - 11.0) * 8.0).clamp(20.0, 100.0)
        };

        vec![
            RadarDimensionScore {
                key: "thermal_suitability".into(),
                label: "Adequação Térmica".into(),
                score: (thermal_score * 10.0).round() / 10.0,
                benchmark_optimal: 100.0,
                description: "Proximidade do intervalo térmico ótimo para fotossíntese do arroz".into(),
            },
            RadarDimensionScore {
                key: "radiation_energy".into(),
                label: "Energia Radiativa".into(),
                score: (rad_score * 10.0).round() / 10.0,
                benchmark_optimal: 100.0,
                description: "Radiação solar fotossinteticamente ativa acumulada no dossel".into(),
            },
            RadarDimensionScore {
                key: "water_security".into(),
                label: "Segurança Hídrica".into(),
                score: (water_score * 10.0).round() / 10.0,
                benchmark_optimal: 100.0,
                description: "Balanço hídrico e atenuação de dias consecutivos de estiagem".into(),
            },
            RadarDimensionScore {
                key: "humidity_balance".into(),
                label: "Equilíbrio de Umidade".into(),
                score: (rh_score * 10.0).round() / 10.0,
                benchmark_optimal: 100.0,
                description: "Faixa microclimática favorável contra choque de dessecação e patógenos".into(),
            },
            RadarDimensionScore {
                key: "thermal_stability".into(),
                label: "Estabilidade Térmica (DTR)".into(),
                score: (dtr_score * 10.0).round() / 10.0,
                benchmark_optimal: 100.0,
                description: "Amplitude térmica diurna moderada para respiração noturna eficiente".into(),
            },
        ]
    }

    /// Generates dynamic trilingual agronomic alerts from time-series statistics.
    fn generate_dynamic_alerts(
        summary: &BiometSummary,
    ) -> BTreeMap<String, Vec<DynamicAdvisoryAlert>> {
        let mut map = BTreeMap::new();
        for &locale in Locale::all() {
            let mut alerts = Vec::new();

            // Thermal Stress Alert
            if summary.mean_t_max_celsius >= 34.0 {
                match locale {
                    Locale::PtBr => alerts.push(DynamicAdvisoryAlert {
                        level: "critical".into(),
                        category: "thermal".into(),
                        title: "Alerta de Pico Térmico no Dossel".into(),
                        message: format!(
                            "Média de T_max atingiu {:.1}°C. Temperaturas elevadas aceleram o consumo respiratório e induzem abortamento floral se coincidentes com a antese.",
                            summary.mean_t_max_celsius
                        ),
                        action: "Elevar lâmina de irrigação para 7–10 cm para amortecimento microclimático.".into(),
                    }),
                    Locale::En => alerts.push(DynamicAdvisoryAlert {
                        level: "critical".into(),
                        category: "thermal".into(),
                        title: "Canopy Heat Spike Advisory".into(),
                        message: format!(
                            "Mean T_max reached {:.1}°C. Extreme temperatures accelerate nighttime respiration and induce spikelet sterility during anthesis.",
                            summary.mean_t_max_celsius
                        ),
                        action: "Raise irrigation flood depth to 7–10 cm to provide thermal buffering.".into(),
                    }),
                    Locale::Th => alerts.push(DynamicAdvisoryAlert {
                        level: "critical".into(),
                        category: "thermal".into(),
                        title: "เตือนภัยอุณหภูมิสูงในแปลงข้าว".into(),
                        message: format!(
                            "อุณหภูมิสูงสุดเฉลี่ยแตะ {:.1}°C อากาศร้อนจัดทำให้ข้าวหายใจสูญเสียพลังงานและเพิ่มความเสี่ยงต่อการผสมไม่ติดในระยะออกดอก",
                            summary.mean_t_max_celsius
                        ),
                        action: "เพิ่มระดับน้ำในแปลงเป็น 7–10 เซนติเมตร เพื่อลดความร้อนรอบช่อดอก".into(),
                    }),
                }
            }

            // High Disease Pressure Alert (Blast / Sheath Blight)
            if summary.mean_relative_humidity_pct >= 82.0 && summary.mean_t_min_celsius >= 21.0 {
                match locale {
                    Locale::PtBr => alerts.push(DynamicAdvisoryAlert {
                        level: "warning".into(),
                        category: "phytosanitary".into(),
                        title: "Condição Favorável a Doenças Fúngicas (Brusone)".into(),
                        message: format!(
                            "Umidade média de {:.1}% com noites amenas ({:.1}°C) cria microclima propício para esporulação de Magnaporthe oryzae.",
                            summary.mean_relative_humidity_pct, summary.mean_t_min_celsius
                        ),
                        action: "Inspecionar colares das folhas e nós das panículas; preparar fungicida preventivo se houver histórico.".into(),
                    }),
                    Locale::En => alerts.push(DynamicAdvisoryAlert {
                        level: "warning".into(),
                        category: "phytosanitary".into(),
                        title: "Fungal Disease Microclimate Risk (Blast)".into(),
                        message: format!(
                            "Mean relative humidity of {:.1}% combined with warm nights ({:.1}°C) promotes Magnaporthe oryzae sporulation.",
                            summary.mean_relative_humidity_pct, summary.mean_t_min_celsius
                        ),
                        action: "Inspect flag leaf collars and panicle necks; prepare preventive fungicide if disease pressure persists.".into(),
                    }),
                    Locale::Th => alerts.push(DynamicAdvisoryAlert {
                        level: "warning".into(),
                        category: "phytosanitary".into(),
                        title: "ความเสี่ยงการระบาดของโรคไหม้และกาบใบแห้ง".into(),
                        message: format!(
                            "ความชื้นสัมพัทธ์สูงเฉลี่ย {:.1}% และอากาศอุ่นชื้นตอนกลางคืน ({:.1}°C) เหมาะอย่างยิ่งต่อการเจริญเติบโตของเชื้อราก่อโรคไหม้",
                            summary.mean_relative_humidity_pct, summary.mean_t_min_celsius
                        ),
                        action: "สำรวจข้อต่อใบธงและคอรวง หากพบแผลรูปตา ให้เตรียมฉีดพ่นสารป้องกันกำจัดเชื้อรา".into(),
                    }),
                }
            }

            // Dry Spell / Irrigation Alert
            if summary.consecutive_dry_days >= 5 {
                match locale {
                    Locale::PtBr => alerts.push(DynamicAdvisoryAlert {
                        level: "warning".into(),
                        category: "water".into(),
                        title: "Veranico Detectado (Ausência de Precipitação)".into(),
                        message: format!(
                            "Identificados {} dias secos na janela analisada. Monitorar rigorosamente a lâmina d'água no talhão.",
                            summary.consecutive_dry_days
                        ),
                        action: "Assegurar reposição dos canais de irrigação e verificar permeabilidade das taipas.".into(),
                    }),
                    Locale::En => alerts.push(DynamicAdvisoryAlert {
                        level: "warning".into(),
                        category: "water".into(),
                        title: "Dry Spell In Progress (No Significant Rainfall)".into(),
                        message: format!(
                            "Recorded {} dry days within current evaluation window. Close monitoring of flood depth is advised.",
                            summary.consecutive_dry_days
                        ),
                        action: "Verify irrigation gate inflow and ensure levee integrity to prevent water loss.".into(),
                    }),
                    Locale::Th => alerts.push(DynamicAdvisoryAlert {
                        level: "warning".into(),
                        category: "water".into(),
                        title: "สภาวะฝนทิ้งช่วงในแปลง".into(),
                        message: format!(
                            "ตรวจพบวันที่ไม่มีฝนตก {} วันในรอบการประเมิน ต้องควบคุมระดับน้ำอย่างใกล้ชิด",
                            summary.consecutive_dry_days
                        ),
                        action: "ตรวจเช็คคันนาและช่องเปิดน้ำเข้าแปลงเพื่อป้องกันน้ำรั่วซึม".into(),
                    }),
                }
            }

            // Favorable Growth Conditions Info
            if alerts.is_empty() {
                match locale {
                    Locale::PtBr => alerts.push(DynamicAdvisoryAlert {
                        level: "info".into(),
                        category: "radiation".into(),
                        title: "Ambiente Agrometeorológico Estável e Favorável".into(),
                        message: format!(
                            "GDD acumulado de {:.1}°C-dia com radiação solar média de {:.1} MJ/m²/dia. O dossel vegetativo apresenta pleno potencial fotossintético.",
                            summary.total_gdd, summary.mean_radiation_mj_m2
                        ),
                        action: "Manter manejo regular de adubação e monitoramento de rotina.".into(),
                    }),
                    Locale::En => alerts.push(DynamicAdvisoryAlert {
                        level: "info".into(),
                        category: "radiation".into(),
                        title: "Stable Agrometeorological Conditions".into(),
                        message: format!(
                            "Cumulative GDD of {:.1}°C-day with average solar radiation of {:.1} MJ/m²/day. Canopy is operating at prime photosynthetic efficiency.",
                            summary.total_gdd, summary.mean_radiation_mj_m2
                        ),
                        action: "Continue standard nutrient topdressing and routine scouting.".into(),
                    }),
                    Locale::Th => alerts.push(DynamicAdvisoryAlert {
                        level: "info".into(),
                        category: "radiation".into(),
                        title: "สภาพอากาศเอื้ออำนวยต่อการเจริญเติบโต".into(),
                        message: format!(
                            "ความร้อนสะสม GDD {:.1}°C-วัน และแสงแดดเฉลี่ย {:.1} MJ/m²/วัน ต้นข้าวสังเคราะห์แสงได้อย่างเต็มที่",
                            summary.total_gdd, summary.mean_radiation_mj_m2
                        ),
                        action: "ดูแลระดับน้ำและใส่ปุ๋ยตามรอบเวลาปกติ".into(),
                    }),
                }
            }

            map.insert(locale.as_str().to_string(), alerts);
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_series() -> Vec<DailyWeatherRecord> {
        let base_date = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        (0..15)
            .map(|i| DailyWeatherRecord {
                date: base_date + chrono::Duration::days(i),
                t_max: 33.0 + (i as f64 * 0.2),
                t_min: 23.0 + (i as f64 * 0.1),
                precipitation_mm: if i % 4 == 0 { 20.0 } else { 0.0 },
                radiation_mj_m2: 18.0 + (i as f64 * 0.3),
                relative_humidity_pct: 80.0 + (i as f64 * 0.5),
                source: "TestStation".into(),
            })
            .collect()
    }

    #[test]
    fn test_weather_analytics_computation() {
        let records = create_test_series();
        let eval_date = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap();

        let report = WeatherAnalyticsService::compute_analytics("P01", &records, eval_date, 14)
            .expect("Analytics computation failed");

        assert_eq!(report.parcel_id, "P01");
        assert_eq!(report.summary.total_records, 14);
        assert!(report.summary.total_gdd > 0.0);
        assert_eq!(report.correlation_matrix.variables.len(), 6);
        assert_eq!(report.correlation_matrix.matrix.len(), 6);

        // Diagonal of correlation matrix must be 1.0
        for i in 0..6 {
            assert_eq!(report.correlation_matrix.matrix[i][i], 1.0);
        }

        // Radar dimensions
        assert_eq!(report.biomet_radar.len(), 5);
        for dim in &report.biomet_radar {
            assert!(dim.score >= 0.0 && dim.score <= 100.0);
        }

        // Trilingual alerts
        assert!(report.dynamic_advisories.contains_key("pt-BR"));
        assert!(report.dynamic_advisories.contains_key("en"));
        assert!(report.dynamic_advisories.contains_key("th"));
    }
}
