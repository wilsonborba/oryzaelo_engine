//! # Oryza-Elo Architecture Guardrail: Phenology Handlers

use crate::core::error::AppError;
use crate::dal::inference::onnx_engine::CategoricalEncoder;
use crate::domain::models::locale::Locale;
use crate::domain::models::phenology::PhenologyPrediction;
use crate::domain::services::agronomic_advisor::AgronomicAdvisor;
use crate::domain::services::biomet_calculator::BiometCalculator;
use crate::presentation::api::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Deserialize)]
pub struct PredictRequest {
    pub parcel_id: String,
    pub eval_date: Option<NaiveDate>,
    pub locale: Option<String>,
}

#[derive(Deserialize)]
pub struct LatestQuery {
    pub parcel_id: String,
    pub locale: Option<String>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub parcel_id: String,
    pub limit: Option<usize>,
}

/// Executes on-demand phenological stage prediction for a parcel.
pub async fn predict_stage(
    State(state): State<AppState>,
    Json(req): Json<PredictRequest>,
) -> Result<(StatusCode, Json<PhenologyPrediction>), AppError> {
    // 1. Fetch parcel
    let parcel = state
        .parcel_repo
        .get_by_id(&req.parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", req.parcel_id)))?;

    // 2. Resolve evaluation date (fallback to today)
    let eval_date = req.eval_date.unwrap_or_else(|| Utc::now().date_naive());

    // 3. Resolve target locale (fallback to Portuguese ESALQ standard)
    let target_locale = req
        .locale
        .as_deref()
        .and_then(|l| Locale::from_str(l).ok())
        .unwrap_or(Locale::PtBr);

    // 4. Fetch retrospective weather series up to eval_date (up to 60 days)
    let history = state
        .weather_repo
        .get_retrospective(&parcel.id, eval_date, 60)
        .await?;

    if history.len() < 7 {
        return Err(AppError::Domain(
            crate::core::error::DomainError::InsufficientWeatherData {
                required: 7,
                available: history.len(),
            },
        ));
    }

    // 5. Encode categoricals using domain dictionaries
    let eco_code = CategoricalEncoder::encode_ecosystem(&parcel.rice_ecosystem);
    let var_code = CategoricalEncoder::encode_variety(&parcel.rice_variety);
    let prov_code = CategoricalEncoder::encode_province("Suphan Buri");

    // 6. Compute 44 biometeorological features
    let features = BiometCalculator::compute(
        &history,
        eval_date,
        parcel.latitude,
        parcel.longitude,
        eco_code,
        var_code,
        prov_code,
        None,
    )?;

    // 7. Execute ONNX inference (< 1 ms)
    let inference = state.onnx_engine.predict(&features)?;

    // 8. Generate trilingual advisories and full offline translations
    let stage = inference.predicted_stage;
    let advisory = AgronomicAdvisor::generate_advisory(stage, target_locale);
    let all_translations = AgronomicAdvisor::generate_all_translations(stage);

    let prediction = PhenologyPrediction {
        evaluated_at: Utc::now(),
        parcel_id: parcel.id.clone(),
        macro_phase: stage.macro_phase(),
        granular_stage: stage,
        confidence: inference.confidence,
        is_transitioning: inference.is_transitioning,
        probabilities: inference.probabilities,
        advisory,
        all_translations,
        technical_metrics: inference.technical_metrics,
    };

    // 9. Persist prediction in SQLite history
    state.prediction_repo.save(&prediction).await?;

    Ok((StatusCode::OK, Json(prediction)))
}

/// Retrieves latest prediction consolidated in database for a parcel.
pub async fn get_latest_prediction(
    State(state): State<AppState>,
    Query(query): Query<LatestQuery>,
) -> Result<Json<PhenologyPrediction>, AppError> {
    let mut pred = state
        .prediction_repo
        .get_latest_for_parcel(&query.parcel_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "No prediction history found for parcel {}",
                query.parcel_id
            ))
        })?;

    // If a specific locale was requested, adapt active advisory
    if let Some(ref l_str) = query.locale {
        if let Ok(loc) = Locale::from_str(l_str) {
            pred.advisory = AgronomicAdvisor::generate_advisory(pred.granular_stage, loc);
        }
    }

    Ok(Json(pred))
}

/// Retrieves prediction history for a parcel.
pub async fn get_prediction_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<PhenologyPrediction>>, AppError> {
    let limit = query.limit.unwrap_or(30);
    let history = state
        .prediction_repo
        .list_for_parcel(&query.parcel_id, limit)
        .await?;

    Ok(Json(history))
}

#[derive(Deserialize)]
pub struct SimulationRequest {
    pub parcel_id: String,
    pub cultivar: Option<String>,
    pub das: f64,
    pub t_min: f64,
    pub t_max: f64,
    pub water_depth_cm: f64,
    pub relative_humidity_pct: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub radiation_mj_m2: Option<f64>,
    pub locale: Option<String>,
}

#[derive(Serialize)]
pub struct SimulationResponse {
    pub parcel_id: String,
    pub cultivar: String,
    pub das: f64,
    pub t_mean: f64,
    pub daily_gdd: f64,
    pub accumulated_gdd: f64,
    pub dtr: f64,
    pub bbch_code: String,
    pub stage_name: String,
    pub water_depth_cm: f64,
    pub water_status: String,
    pub thermal_risk: String,
    pub advisory: String,
    pub recommendations: Vec<String>,
}

/// Simulates agronomic and phenological responses for custom microclimatic scenarios.
pub async fn simulate_scenario(
    State(state): State<AppState>,
    Json(req): Json<SimulationRequest>,
) -> Result<Json<SimulationResponse>, AppError> {
    let parcel = state
        .parcel_repo
        .get_by_id(&req.parcel_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Parcel {} not found", req.parcel_id)))?;

    let cultivar = req
        .cultivar
        .clone()
        .unwrap_or_else(|| parcel.rice_variety.clone());

    let target_locale = req
        .locale
        .as_deref()
        .and_then(|l| Locale::from_str(l).ok())
        .unwrap_or(Locale::PtBr);

    let t_mean = (req.t_max + req.t_min) / 2.0;
    let daily_gdd = (t_mean - 10.0).max(0.0);
    let accum_gdd = daily_gdd * req.das;
    let dtr = (req.t_max - req.t_min).max(0.0);
    let rh = req.relative_humidity_pct.unwrap_or(70.0);
    let rain = req.precipitation_mm.unwrap_or(0.0);
    let rad = req.radiation_mj_m2.unwrap_or(18.0);

    // 1. Phenological Stage Determination
    let (bbch_code, stage_name, base_advisory) = match target_locale {
        Locale::En => {
            if accum_gdd < 180.0 {
                ("BBCH 09 - 12", "Emergence & Seedling Establishment", "Maintain saturated soil without deep flood to promote early rooting and avoid seedling anoxia.")
            } else if accum_gdd < 520.0 {
                ("BBCH 21 - 29", "Active Tillering & Foliar Development", "Maintain 5 to 8 cm water depth. Optimal window for topdress nitrogen fertilization.")
            } else if accum_gdd < 950.0 {
                ("BBCH 30 - 39", "Stem Elongation & Panicle Initiation", "Critical water-sensitive window. Maintain 10 cm water layer to buffer young panicles against night cold.")
            } else if accum_gdd < 1350.0 {
                ("BBCH 51 - 69", "Booting, Flowering & Anthesis", "Avoid severe water and thermal stress. Essential protective flood for spikelet fertility and pollen viability.")
            } else {
                ("BBCH 71 - 89", "Grain Ripening (Milky to Hard Dough)", "Gradually initiate paddy drainage 15 days before harvest to facilitate harvester transit.")
            }
        }
        Locale::Th => {
            if accum_gdd < 180.0 {
                ("BBCH 09 - 12", "การงอกและการตั้งตัวของต้นกล้า", "รักษาระดับดินให้อิ่มตัวด้วยน้ำโดยไม่มีน้ำขังลึก เพื่อกระตุ้นการพัฒนาระบบรากเริ่มต้น")
            } else if accum_gdd < 520.0 {
                ("BBCH 21 - 29", "การแตกกอและการเจริญเติบโตทางใบ", "รักษาระดับน้ำสม่ำเสมอที่ 5 ถึง 8 ซม. ช่วงเวลาที่เหมาะสมที่สุดสำหรับการใส่ปุ๋ยไนโตรเจนแต่งหน้า")
            } else if accum_gdd < 950.0 {
                ("BBCH 30 - 39", "การยืดปล้องและการกำเนิดช่อดอก", "ระยะวิกฤตที่ไวต่อน้ำและอุณหภูมิ ควรรักษาระดับน้ำไว้ที่ 10 ซม. เพื่อป้องกันความหนาวเย็นในเวลากลางคืน")
            } else if accum_gdd < 1350.0 {
                ("BBCH 51 - 69", "ระยะตั้งท้อง การออกดอก และการผสมเกสร", "หลีกเลี่ยงความเครียดจากความร้อนและขาดน้ำ ระดับน้ำป้องกันมีความสำคัญยิ่งต่อความสมบูรณ์ของละอองเกสร")
            } else {
                ("BBCH 71 - 89", "ระยะการสุกแก่ของเมล็ดข้าว", "เริ่มระบายน้ำออกจากแปลงนาทีละน้อยก่อนการเก็บเกี่ยว 15 วัน เพื่อความสะดวกในการใช้รถเกี่ยวข้าว")
            }
        }
        Locale::PtBr => {
            if accum_gdd < 180.0 {
                ("BBCH 09 - 12", "Emergência & Estabelecimento de Plântulas", "Manter solo saturado sem lâmina profunda para favorecer o enraizamento inicial e evitar anoxia.")
            } else if accum_gdd < 520.0 {
                ("BBCH 21 - 29", "Perfilhamento Ativo & Desenvolvimento Foliar", "Manter lâmina constante de 5 a 8 cm. Momento ideal para adubação nitrogenada de cobertura.")
            } else if accum_gdd < 950.0 {
                ("BBCH 30 - 39", "Alongamento do Colmo & Iniciação da Panícula", "Fase crítica de sensibilidade hídrica e térmica. Manter lâmina a 10 cm para proteger a panícula jovem.")
            } else if accum_gdd < 1350.0 {
                ("BBCH 51 - 69", "Emborrachamento, Floração & Antese", "Evitar estresse térmico e hídrico. Lâmina protetora indispensável para fertilidade das espiguetas.")
            } else {
                ("BBCH 71 - 89", "Maturação dos Grãos (Leitoso a Pastoso)", "Iniciar drenagem gradual do arrozal 15 dias antes da colheita para facilitar tráfego de colhedoras.")
            }
        }
    };

    // 2. Hydrological Status
    let water_status = match target_locale {
        Locale::En => {
            if req.water_depth_cm < 2.0 {
                "Critical Drought Deficit: Water layer depleted (< 2 cm). Severe root aeration stress and rapid weed competition."
            } else if req.water_depth_cm < 5.0 {
                "Sub-Optimal Water Layer: Depth between 2-5 cm. Adequate for early vegetative but vulnerable to thermal swings."
            } else if req.water_depth_cm <= 9.0 {
                "Optimal Physiological Depth: Water layer between 5-9 cm maximizes nutrient uptake and daytime thermal buffer."
            } else if req.water_depth_cm <= 12.0 {
                "Deep Water Buffer: Water layer 9-12 cm. Excellent thermal shield against cold nights; check spillway gates."
            } else {
                "Excessive Submersion Alert: Water depth > 12 cm. Induces abnormal internode elongation and culm weakening."
            }
        }
        Locale::Th => {
            if req.water_depth_cm < 2.0 {
                "วิกฤตขาดน้ำรุนแรง: ระดับน้ำลดลงต่ำกว่า 2 ซม. ระบบรากเสี่ยงต่อการขาดน้ำและวัชพืชจะระบาดอย่างรวดเร็ว"
            } else if req.water_depth_cm < 5.0 {
                "ระดับน้ำต่ำกว่าเกณฑ์: ระดับน้ำ 2-5 ซม. เพียงพอต่อระยะต้นกล้าแต่อาจได้รับผลกระทบจากอุณหภูมิผันผวน"
            } else if req.water_depth_cm <= 9.0 {
                "ระดับน้ำเหมาะสมสูงสุด: ระดับน้ำ 5-9 ซม. ส่งเสริมการดูดซึมธาตุอาหารและการปรับสมดุลความร้อนในแปลงนา"
            } else if req.water_depth_cm <= 12.0 {
                "ระดับน้ำป้องกันความหนาว: ระดับน้ำ 9-12 ซม. ป้องกันความหนาวเย็นยามค่ำคืนได้อย่างมีประสิทธิภาพ"
            } else {
                "ระดับน้ำสูงเกินไป: ระดับน้ำ > 12 ซม. เสี่ยงต่อการยืดยาวผิดปกติของลำต้นและลดการแตกกอ"
            }
        }
        Locale::PtBr => {
            if req.water_depth_cm < 2.0 {
                "Déficit Hídrico Crítico: Lâmina d'água exaurida (< 2 cm). Estresse radicular agudo e perda de controle de invasoras."
            } else if req.water_depth_cm < 5.0 {
                "Lâmina Rasa Sub-ótima: Profundidade de 2 a 5 cm. Aceitável no perfilhamento inicial, mas vulnerável a calor extremo."
            } else if req.water_depth_cm <= 9.0 {
                "Lâmina Fisiológica Ideal: Profundidade de 5 a 9 cm garante absorção nutricional e inércia térmica equilibrada."
            } else if req.water_depth_cm <= 12.0 {
                "Lâmina Alta Protetora: Profundidade de 9 a 12 cm. Excelente isolamento contra geadas e frio noturno."
            } else {
                "Submersão Excessiva: Lâmina > 12 cm. Risco de estiolamento, quebra de colmos e abortamento de afilhos basais."
            }
        }
    };

    // 3. Thermal Risk
    let thermal_risk = match target_locale {
        Locale::En => {
            if req.t_min < 12.0 {
                "Severe Cold Shock Hazard: Night minimum < 12°C. High probability of microspore degeneration and empty panicles."
            } else if req.t_min < 16.0 {
                "Moderate Chilling Alert: Night minimum 12-16°C. Raise paddy flood to 10 cm before dusk to protect growing points."
            } else if req.t_max > 36.0 {
                "Critical Heat Spike Hazard: Maximum > 36°C during pollination causes irreversible pollen desiccation and sterility."
            } else if req.t_max > 33.0 {
                "Moderate Heat Advisory: Daily maximum 33-36°C. Ensure steady canal irrigation to lower canopy temperature."
            } else {
                "Optimal Thermal Window: Temperatures within 18°C-31°C support peak biochemical Rubisco activation."
            }
        }
        Locale::Th => {
            if req.t_min < 12.0 {
                "เตือนภัยความเย็นขั้นรุนแรง: อุณหภูมิต่ำสุด < 12°C เกสรดอกข้าวอาจเสื่อมสภาพและทำให้รวงข้าวลีบ"
            } else if req.t_min < 16.0 {
                "เตือนภัยความเย็นปานกลาง: อุณหภูมิต่ำสุด 12-16°C ควรเพิ่มระดับน้ำเป็น 10 ซม. ก่อนค่ำเพื่อปกป้องจุดเจริญเติบโต"
            } else if req.t_max > 36.0 {
                "เตือนภัยคลื่นความร้อนวิกฤต: อุณหภูมิ > 36°C ในช่วงผสมเกสรอาจทำให้ละอองเกสรแห้งตายและเป็นหมัน"
            } else if req.t_max > 33.0 {
                "เตือนความร้อนปานกลาง: อุณหภูมิ 33-36°C ควรหมุนเวียนน้ำในแปลงนาเพื่อลดอุณหภูมิของทรงพุ่ม"
            } else {
                "ช่วงอุณหภูมิที่เหมาะสม: อุณหภูมิระหว่าง 18°C-31°C ส่งเสริมการสังเคราะห์แสงและการสะสมแป้งสูงสุด"
            }
        }
        Locale::PtBr => {
            if req.t_min < 12.0 {
                "Perigo de Choque Térmico Severo: Mínima < 12°C. Risco crítico de esterilidade na meiose e espiguetas chochas."
            } else if req.t_min < 16.0 {
                "Alerta de Frio Noturno: Mínima entre 12°C e 16°C. Eleve a lâmina d'água para 10 cm antes do entardecer."
            } else if req.t_max > 36.0 {
                "Perigo de Estresse Térmico Crítico: Máxima > 36°C na antese provoca desidratação irreversível dos grãos de pólen."
            } else if req.t_max > 33.0 {
                "Alerta de Calor Moderado: Máxima entre 33°C e 36°C. Mantenha circulação d'água contínua no quadro."
            } else {
                "Regime Térmico Fisiológico Ideal: Faixa de 18°C a 31°C maximiza taxa fotossintética líquida e fotossintatos."
            }
        }
    };

    // 4. Dynamic Multi-Axis Agronomic Recommendations (Combinatorial Expert Matrix)
    let mut recs = Vec::new();

    match target_locale {
        Locale::En => {
            // Rec 1: Thermal Progression & Cultivar
            recs.push(format!(
                "Thermal accumulation reaches {:.1} GDD (°C-day) for cultivar {}. Daily thermal accumulation is currently {:.1} °C-day.",
                accum_gdd, cultivar, daily_gdd
            ));

            // Rec 2: Hydrological action
            recs.push(water_status.to_string());

            // Rec 3: Thermal risk action
            recs.push(thermal_risk.to_string());

            // Rec 4: DTR & Respiration
            if dtr > 13.0 {
                recs.push(format!("Extreme Diurnal Thermal Range ({:.1}°C): Rapid night cooling drives carbohydrate translocation toward panicles; avoid late fertilization.", dtr));
            } else if dtr < 6.0 {
                recs.push(format!("Low Diurnal Range ({:.1}°C): Persistent cloudiness or humidity reduces respiration differential; keep flood level moderate.", dtr));
            } else {
                recs.push(format!("Balanced Diurnal Thermal Range ({:.1}°C): Stable microclimatic gradient favorable for active vascular transport.", dtr));
            }

            // Rec 5: Pathology & Microclimate Risk
            if rh > 82.0 && t_mean > 24.0 {
                recs.push("Pathology Alert: Elevated humidity (> 82%) combined with warm canopy temperatures creates prime conditions for Rice Blast (Pyricularia oryzae) and Sheath Blight. Inspect lower leaf sheaths immediately.".to_string());
            } else if rh < 48.0 {
                recs.push("Atmospheric Desiccation Alert: Low relative humidity (< 48%) accelerates leaf transpiration; ensure paddy inflow valves are unobstructed.".to_string());
            } else {
                recs.push("Pathological Risk Low: Atmospheric vapor pressure deficit remains within safe non-conducive thresholds.".to_string());
            }

            // Rec 6: Solar Radiation & Photosynthesis
            if rad > 22.0 {
                recs.push(format!("High Solar Irradiance ({:.1} MJ/m²): Peak photosynthetic active radiation. Optimal conditions for vegetative canopy light interception.", rad));
            } else if rad < 11.0 {
                recs.push(format!("Low Light Restriction ({:.1} MJ/m²): Persistent cloudy overcast restricts starch synthesis; postpone chemical applications until light levels recover.", rad));
            }

            // Rec 7: Precipitation & Inflow Management
            if rain > 20.0 {
                recs.push(format!("Heavy Rainfall Event ({:.1} mm): Inspect perimeter levees and adjust overflow spillways to prevent wall breaches.", rain));
            } else if rain == 0.0 && req.water_depth_cm < 5.0 {
                recs.push("Zero Precipitation Recorded: With shallow water depth, schedule supplemental canal pump activation within 24 hours.".to_string());
            }
        }
        Locale::Th => {
            recs.push(format!(
                "การสะสมความร้อนอยู่ที่ {:.1} GDD (°C-วัน) สำหรับพันธุ์ข้าว {} อัตราสะสมรายวันคือ {:.1} °C-วัน",
                accum_gdd, cultivar, daily_gdd
            ));
            recs.push(water_status.to_string());
            recs.push(thermal_risk.to_string());

            if dtr > 13.0 {
                recs.push(format!("ช่วงอุณหภูมิรายวันกว้างมาก ({:.1}°C): การลดลงของอุณหภูมิในตอนกลางคืนช่วยเร่งการเคลื่อนย้ายคาร์โบไฮเดรตสู่รวงข้าว ควรหลีกเลี่ยงการใส่ปุ๋ยไนโตรเจนช้า", dtr));
            } else if dtr < 6.0 {
                recs.push(format!("ช่วงอุณหภูมิรายวันแคบ ({:.1}°C): ฟ้าหลัวหรือความชื้นสูงต่อเนื่อง ควรรักษาระดับน้ำปานกลาง", dtr));
            } else {
                recs.push(format!("ช่วงอุณหภูมิรายวันสมดุล ({:.1}°C): สภาพอากาศเหมาะสมสำหรับการลำเลียงสารอาหารของพืช", dtr));
            }

            if rh > 82.0 && t_mean > 24.0 {
                recs.push("เตือนภัยโรคพืช: ความชื้นสัมพัทธ์สูง (> 82%) ร่วมกับอุณหภูมิอบอุ่น เป็นสภาวะเสี่ยงต่อการเกิดโรคไหม้ข้าว (Pyricularia oryzae) และโรคกาบใบแห้ง ควรสำรวจโคนกอข้าวทันที".to_string());
            } else if rh < 48.0 {
                recs.push("เตือนอากาศแห้งจัด: ความชื้นสัมพัทธ์ต่ำ (< 48%) เร่งการคายน้ำของใบข้าว ควรตรวจสอบให้แน่ใจว่าทางน้ำเข้านาไม่อุดตัน".to_string());
            } else {
                recs.push("ความเสี่ยงโรคพืชต่ำ: ความดันไอน้ำในบรรยากาศอยู่ในเกณฑ์ปลอดภัย ไม่เอื้อต่อการแพร่กระจายของเชื้อรา".to_string());
            }

            if rad > 22.0 {
                recs.push(format!("รังสีดวงอาทิตย์สูงมาก ({:.1} MJ/m²): พลังงานแสงเหมาะสมสูงสุดสำหรับการสังเคราะห์แสงและการสร้างผลผลิต", rad));
            } else if rad < 11.0 {
                recs.push(format!("แสงแดดจำกัด ({:.1} MJ/m²): ท้องฟ้ามืดครึ้มต่อเนื่องจำกัดการสร้างแป้ง ควรเลื่อนการพ่นสารเคมีจนกว่าแสงจะฟื้นตัว", rad));
            }

            if rain > 20.0 {
                recs.push(format!("ฝนตกหนัก ({:.1} มม.): ตรวจสอบคันนาและปรับระดับทางระบายน้ำล้นเพื่อป้องกันคันนาพังทลาย", rain));
            } else if rain == 0.0 && req.water_depth_cm < 5.0 {
                recs.push("ไม่มีฝนตก: ระดับน้ำในแปลงนาต่ำ ควรกำหนดเวลาเปิดน้ำเข้าแปลงนาภายใน 24 ชั่วโมง".to_string());
            }
        }
        Locale::PtBr => {
            recs.push(format!(
                "Soma térmica acumulada estimada em {:.1} °C·dia (GDD base 10°C) para o cultivar {}. Acúmulo diário atual: {:.1} °C·dia.",
                accum_gdd, cultivar, daily_gdd
            ));
            recs.push(water_status.to_string());
            recs.push(thermal_risk.to_string());

            if dtr > 13.0 {
                recs.push(format!("Amplitude Térmica Extrema ({:.1}°C): Resfriamento noturno rápido favorece o particionamento de fotoassimilados para a panícula; evitar doses tardias de nitrogênio.", dtr));
            } else if dtr < 6.0 {
                recs.push(format!("Baixa Amplitude Térmica ({:.1}°C): Nebulosidade contínua reduz gradiente metabólico; mantenha lâmina hídrica moderada.", dtr));
            } else {
                recs.push(format!("Amplitude Térmica Equilibrada ({:.1}°C): Gradiente microclimático favorável ao transporte vascular e enchimento de grãos.", dtr));
            }

            if rh > 82.0 && t_mean > 24.0 {
                recs.push("Alerta Fitossanitário: Umidade relativa (> 82%) e calor propiciam condições ideais para Brusone (Pyricularia oryzae) e Queima das Bainhas (Rhizoctonia solani). Vistoriar bainhas e folhas superiores imediatamente.".to_string());
            } else if rh < 48.0 {
                recs.push("Alerta de Dessecação Atmosférica: Baixa umidade (< 48%) eleva déficit de pressão de vapor (VPD) e estresse transpiratório; garanta canais desobstruídos.".to_string());
            } else {
                recs.push("Baixo Risco de Patologias Fúngicas: Pressão de vapor d'água dentro de limiares não conducentes a epidemias.".to_string());
            }

            if rad > 22.0 {
                recs.push(format!("Alta Radiação Solar Global ({:.1} MJ/m²): Eficiência quântica e interceptação de radiação fotossinteticamente ativa no ápice.", rad));
            } else if rad < 11.0 {
                recs.push(format!("Restrição Luminosa ({:.1} MJ/m²): Nebulosidade persistente reduz síntese de amido; postergar defensivos até recuperação luminosa.", rad));
            }

            if rain > 20.0 {
                recs.push(format!("Precipitação Pluviométrica Intensa ({:.1} mm): Inspecionar taipas e ajustar deságues para evitar rompimento das bacias.", rain));
            } else if rain == 0.0 && req.water_depth_cm < 5.0 {
                recs.push("Estiagem Local: Lâmina rasa sem chuva nas últimas 24h; programar abertura de comportas nas próximas 24 horas.".to_string());
            }
        }
    };

    Ok(Json(SimulationResponse {
        parcel_id: parcel.id,
        cultivar,
        das: req.das,
        t_mean,
        daily_gdd,
        accumulated_gdd: accum_gdd,
        dtr,
        bbch_code: bbch_code.to_string(),
        stage_name: stage_name.to_string(),
        water_depth_cm: req.water_depth_cm,
        water_status: water_status.to_string(),
        thermal_risk: thermal_risk.to_string(),
        advisory: base_advisory.to_string(),
        recommendations: recs,
    }))
}


