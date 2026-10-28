//! Verify complete Python/Rust vectors against external revision fixtures.
//! This focused test does not write or replace any benchmark report.
use chrono::NaiveDate;
use oryzaelo_engine::domain::models::weather::DailyWeatherRecord;
use oryzaelo_engine::domain::services::biomet_calculator::BiometCalculator;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    source_row_id: usize,
    eval_date: NaiveDate,
    lat: f64,
    lon: f64,
    weather: Vec<DailyWeatherRecord>,
    expected: Vec<f64>,
}

#[test]
fn complete_vectors_and_future_exclusion_match_python() {
    let path = std::env::var("TCC_REVISION_FIXTURE")
        .expect("Set TCC_REVISION_FIXTURE to the prepared revision fixture JSON");
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(cases.len() >= 30);
    let mut maximum = 0.0_f64;
    for c in &cases {
        let computed = BiometCalculator::compute(
            &c.weather, c.eval_date, c.lat, c.lon, 0.0, 7.0, -1.0, None,
        ).unwrap().to_tensor_vec();
        assert_eq!(computed.len(), 44);
        assert_eq!(c.expected.len(), 44);
        for (i, (got, expected)) in computed.iter().zip(&c.expected).enumerate() {
            // Two-decimal summaries can differ by one rounding quantum.
            // The two empirical RH/DTR indices inherit that quantization.
            let tolerance = if i < 32 || i == 37 || i == 38 { 0.0101 } else { 0.0001 };
            let delta = (*got as f64 - expected).abs();
            maximum = maximum.max(delta);
            assert!(delta <= tolerance,
                "source row {}, input {}: {} != {} (delta {})",
                c.source_row_id, i, got, expected, delta);
        }
        let past: Vec<_> = c.weather.iter().filter(|r| r.date <= c.eval_date).cloned().collect();
        let without_future = BiometCalculator::compute(
            &past, c.eval_date, c.lat, c.lon, 0.0, 7.0, -1.0, None,
        ).unwrap().to_tensor_vec();
        assert_eq!(computed, without_future, "Future weather must not change predictions");
    }
    println!("Verified {} rows x 44 inputs, including unknown category -1; maximum absolute difference {}", cases.len(), maximum);
}
