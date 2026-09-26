//! # Oryza-Elo Architecture Guardrail: Nightly Cron Scheduler
//!
//! Autonomous background scheduler evaluating phenological progression for all parcels daily.

use crate::core::error::AppError;
use crate::dal::database::repositories::{
    ConfigRepository, ParcelRepository, PredictionRepository, WeatherRepository,
};
use crate::dal::inference::onnx_engine::{CategoricalEncoder, OnnxInferenceEngine};
use crate::domain::models::farm::FarmParcel;
use crate::domain::models::locale::Locale;
use crate::domain::models::phenology::PhenologyPrediction;
use crate::domain::services::agronomic_advisor::AgronomicAdvisor;
use crate::domain::services::biomet_calculator::BiometCalculator;
use chrono::{Local, NaiveDate, Timelike, Utc};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Config-store key the farmer can set from the dashboard's Settings screen
/// to move the nightly evaluation window off the 23:59 default. Matches the
/// key name the demo seeder (`mock_data.rs`) already pre-populates — there
/// was no need for a second, parallel key for the same concept.
pub const CRON_TARGET_TIME_CONFIG_KEY: &str = "cron_time";
pub const DEFAULT_CRON_TARGET_TIME: &str = "23:59";

/// Parses a config-stored "HH:MM" string into (hour, minute), falling back to
/// the default whenever the value is absent or malformed — the scheduler must
/// never panic or stall just because someone wrote garbage into the config
/// store directly (the API layer validates on write, but this is the last
/// line of defense for a background loop that must keep running).
pub fn parse_target_time(raw: Option<&str>) -> (u32, u32) {
    let fallback = || {
        let mut parts = DEFAULT_CRON_TARGET_TIME.split(':');
        let h: u32 = parts.next().unwrap().parse().unwrap();
        let m: u32 = parts.next().unwrap().parse().unwrap();
        (h, m)
    };

    let Some(raw) = raw else { return fallback() };
    let mut parts = raw.split(':');
    let (Some(h_str), Some(m_str), None) = (parts.next(), parts.next(), parts.next()) else {
        return fallback();
    };
    match (h_str.parse::<u32>(), m_str.parse::<u32>()) {
        (Ok(h), Ok(m)) if h < 24 && m < 60 => (h, m),
        _ => fallback(),
    }
}

pub struct CronScheduler;

impl CronScheduler {
    /// Spawns the background scheduler thread inside the Tokio runtime.
    ///
    /// The target evaluation time is re-read from `config_repo` on every tick
    /// (cheap: one indexed SQLite lookup every 30s) so a farmer changing it
    /// from the Settings screen takes effect immediately, with no restart.
    pub fn spawn(
        parcel_repo: ParcelRepository,
        weather_repo: WeatherRepository,
        prediction_repo: PredictionRepository,
        onnx_engine: Arc<OnnxInferenceEngine>,
        config_repo: ConfigRepository,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("Iniciando Nightly Cron Scheduler de borda (Alvo padrão: {DEFAULT_CRON_TARGET_TIME})...");

            // Resilient recovery: check if machine was turned off during previous scheduled cycles
            if let Err(e) = Self::recover_missing_days(&parcel_repo, &weather_repo, &prediction_repo, &onnx_engine).await {
                warn!(error = %e, "Aviso durante recuperação de avaliações fenológicas pendentes no boot");
            }

            let mut last_executed_date: Option<NaiveDate> = None;

            loop {
                // Check every 30 seconds
                tokio::time::sleep(Duration::from_secs(30)).await;

                let now_local = Local::now();
                let current_date = now_local.date_naive();
                let hour = now_local.hour();
                let minute = now_local.minute();

                let target_raw = config_repo.get(CRON_TARGET_TIME_CONFIG_KEY).await.ok().flatten();
                let (target_hour, target_minute) = parse_target_time(target_raw.as_deref());

                // Trigger once per day, at or after the configured target time
                let should_run = (hour > target_hour || (hour == target_hour && minute >= target_minute))
                    && (last_executed_date != Some(current_date));

                if should_run {
                    info!(
                        date = %current_date,
                        target = format!("{:02}:{:02}", target_hour, target_minute),
                        "Disparando rotina noturna de avaliação fenológica autônoma..."
                    );

                    match Self::execute_evaluation_for_all_parcels(
                        &parcel_repo,
                        &weather_repo,
                        &prediction_repo,
                        &onnx_engine,
                        current_date,
                    )
                    .await
                    {
                        Ok(count) => {
                            info!(
                                parcels_evaluated = count,
                                date = %current_date,
                                "Avaliação fenológica noturna concluída com sucesso para todos os talhões!"
                            );
                            last_executed_date = Some(current_date);
                        }
                        Err(e) => {
                            error!(
                                error = %e,
                                date = %current_date,
                                "Erro durante execução da rotina noturna fenológica"
                            );
                        }
                    }
                }
            }
        })
    }

    /// Evaluates a single parcel for a specific date if enough retrospective weather exists.
    pub async fn evaluate_parcel_for_date(
        parcel: &FarmParcel,
        weather_repo: &WeatherRepository,
        prediction_repo: &PredictionRepository,
        onnx_engine: &OnnxInferenceEngine,
        eval_date: NaiveDate,
    ) -> Result<bool, AppError> {
        let history = weather_repo.get_retrospective(&parcel.id, eval_date, 60).await?;
        if history.len() < 7 {
            return Ok(false);
        }

        let eco_code = CategoricalEncoder::encode_ecosystem(&parcel.rice_ecosystem);
        let var_code = CategoricalEncoder::encode_variety(&parcel.rice_variety);
        let prov_code = CategoricalEncoder::encode_province("Suphan Buri");

        let features = match BiometCalculator::compute(
            &history,
            eval_date,
            parcel.latitude,
            parcel.longitude,
            eco_code,
            var_code,
            prov_code,
            None,
        ) {
            Ok(f) => f,
            Err(e) => {
                warn!(parcel_id = %parcel.id, error = %e, "Falha no cálculo de features biometeorológicas");
                return Ok(false);
            }
        };

        let inference = match onnx_engine.predict(&features) {
            Ok(inf) => inf,
            Err(e) => {
                warn!(parcel_id = %parcel.id, error = %e, "Falha na inferência do modelo ONNX");
                return Ok(false);
            }
        };

        let stage = inference.predicted_stage;
        let advisory = AgronomicAdvisor::generate_advisory(stage, Locale::PtBr);
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

        prediction_repo.save(&prediction).await?;
        Ok(true)
    }

    /// Recovers missing evaluations if the machine was offline during past scheduled windows.
    pub async fn recover_missing_days(
        parcel_repo: &ParcelRepository,
        weather_repo: &WeatherRepository,
        prediction_repo: &PredictionRepository,
        onnx_engine: &OnnxInferenceEngine,
    ) -> Result<usize, AppError> {
        let parcels = parcel_repo.list_all().await?;
        let today = Local::now().date_naive();
        let yesterday = match today.pred_opt() {
            Some(d) => d,
            None => return Ok(0),
        };
        let mut recovered_count = 0;

        for p in parcels {
            let latest_pred = prediction_repo.get_latest_for_parcel(&p.id).await?;
            let start_date = match latest_pred {
                Some(pred) => pred.evaluated_at.date_naive().succ_opt(),
                None => {
                    let history = weather_repo.get_retrospective(&p.id, yesterday, 60).await?;
                    if history.len() >= 7 {
                        Some(history[0].date)
                    } else {
                        None
                    }
                }
            };

            if let Some(mut curr) = start_date {
                while curr <= yesterday {
                    if Self::evaluate_parcel_for_date(&p, weather_repo, prediction_repo, onnx_engine, curr).await? {
                        info!(
                            parcel_id = %p.id,
                            date = %curr,
                            "Recuperada com sucesso predição retroativa pendente (rotina pós-boot)"
                        );
                        recovered_count += 1;
                    }
                    match curr.succ_opt() {
                        Some(next) => curr = next,
                        None => break,
                    }
                }
            }
        }

        Ok(recovered_count)
    }

    /// Evaluates all registered farm parcels for a specific observation date.
    pub async fn execute_evaluation_for_all_parcels(
        parcel_repo: &ParcelRepository,
        weather_repo: &WeatherRepository,
        prediction_repo: &PredictionRepository,
        onnx_engine: &OnnxInferenceEngine,
        eval_date: NaiveDate,
    ) -> Result<usize, AppError> {
        let parcels = parcel_repo.list_all().await?;
        let mut processed = 0;

        for p in parcels {
            match Self::evaluate_parcel_for_date(
                &p,
                weather_repo,
                prediction_repo,
                onnx_engine,
                eval_date,
            )
            .await

            {
                Ok(true) => processed += 1,
                Ok(false) => {
                    warn!(
                        parcel_id = %p.id,
                        "Talhão com histórico meteorológico insuficiente (< 7 dias). Pulando avaliação."
                    );
                }
                Err(e) => {
                    warn!(parcel_id = %p.id, error = %e, "Falha na avaliação do talhão");
                }
            }
        }

        Ok(processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_hh_mm_string() {
        assert_eq!(parse_target_time(Some("06:30")), (6, 30));
        assert_eq!(parse_target_time(Some("00:00")), (0, 0));
        assert_eq!(parse_target_time(Some("23:59")), (23, 59));
    }

    #[test]
    fn falls_back_to_default_when_absent() {
        assert_eq!(parse_target_time(None), (23, 59));
    }

    #[test]
    fn falls_back_to_default_on_malformed_input() {
        assert_eq!(parse_target_time(Some("garbage")), (23, 59));
        assert_eq!(parse_target_time(Some("25:00")), (23, 59));
        assert_eq!(parse_target_time(Some("10:60")), (23, 59));
        assert_eq!(parse_target_time(Some("10")), (23, 59));
        assert_eq!(parse_target_time(Some("10:30:00")), (23, 59));
        assert_eq!(parse_target_time(Some("")), (23, 59));
    }
}
