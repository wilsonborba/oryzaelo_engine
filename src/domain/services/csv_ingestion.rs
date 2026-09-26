//! # Oryza-Elo Architecture Guardrail: CSV Ingestion Service
//!
//! Deterministic CSV parser enforcing explicit sensor profile schemas, unit
//! conversions, and physical invariants without heuristic string guesswork.
//! Parses exactly the metric columns the sensor profile declares (1..5) --
//! a standalone single-metric sensor's CSV only needs that one column.

use crate::core::error::AppError;
use crate::domain::models::device_mapping::DeviceMapping;
use crate::domain::models::metric_type::MetricType;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// One successfully parsed (date, metric_type, value) reading, ready to be
/// persisted via `WeatherRepository::record_reading`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedReading {
    pub date: NaiveDate,
    pub metric_type: MetricType,
    pub value: f64,
}

/// Detailed summary report of a CSV ingestion run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionReport {
    pub total_rows: usize,
    pub successful_rows: usize,
    pub failed_rows: usize,
    pub readings: Vec<ParsedReading>,
    pub errors: Vec<String>,
}

pub struct CsvIngestionService;

impl CsvIngestionService {
    /// Parses a CSV string according to the sensor profile's declared
    /// metric column(s), converting units and validating physical
    /// invariants per metric.
    pub fn parse_csv(csv_content: &str, mapping: &DeviceMapping) -> Result<IngestionReport, AppError> {
        if mapping.metrics.is_empty() {
            return Err(AppError::BadRequest(format!(
                "Sensor profile '{}' declares no metrics to ingest",
                mapping.device_name
            )));
        }

        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .flexible(true)
            .from_reader(csv_content.as_bytes());

        let headers = rdr
            .headers()
            .map_err(|e| AppError::BadRequest(format!("Failed to parse CSV headers: {}", e)))?
            .clone();

        // Resolve column indices strictly matching the schema (Zero Heuristics!)
        let date_idx = Self::find_column_index(&headers, &mapping.date_col, "date", &mapping.device_name)?;

        let mut metric_indices = Vec::with_capacity(mapping.metrics.len());
        for m in &mapping.metrics {
            let idx = Self::find_column_index(&headers, &m.column_name, m.metric_type.as_str(), &mapping.device_name)?;
            metric_indices.push((m.metric_type, idx));
        }

        let mut readings = Vec::new();
        let mut errors = Vec::new();
        let mut total_rows = 0;
        let mut failed_row_numbers = std::collections::HashSet::new();
        let mut row_num_for_date: std::collections::HashMap<NaiveDate, usize> = std::collections::HashMap::new();

        for (row_idx, result) in rdr.records().enumerate() {
            total_rows += 1;
            let row_num = row_idx + 2; // 1-based, accounts for header row

            let record = match result {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("Row {}: malformed CSV line: {}", row_num, e));
                    failed_row_numbers.insert(row_num);
                    continue;
                }
            };

            // Parse Date
            let raw_date_str = match record.get(date_idx) {
                Some(s) if !s.trim().is_empty() => s.trim(),
                _ => {
                    errors.push(format!("Row {}: missing or empty date value", row_num));
                    failed_row_numbers.insert(row_num);
                    continue;
                }
            };

            let parsed_date = match Self::parse_date(raw_date_str, &mapping.date_format) {
                Some(d) => d,
                None => {
                    errors.push(format!(
                        "Row {}: unable to parse date '{}' with format '{}'",
                        row_num, raw_date_str, mapping.date_format
                    ));
                    failed_row_numbers.insert(row_num);
                    continue;
                }
            };

            row_num_for_date.insert(parsed_date, row_num);

            for &(metric_type, idx) in &metric_indices {
                let raw_val = match Self::parse_numeric(record.get(idx), row_num, metric_type.as_str(), &mut errors) {
                    Some(v) => v,
                    None => {
                        failed_row_numbers.insert(row_num);
                        continue;
                    }
                };

                let value = match mapping.convert(metric_type, raw_val) {
                    Some(v) => v,
                    None => continue,
                };

                if let Some(err) = Self::validate_physical(metric_type, value, row_num) {
                    errors.push(err);
                    failed_row_numbers.insert(row_num);
                    continue;
                }

                readings.push(ParsedReading {
                    date: parsed_date,
                    metric_type,
                    value: (value * 100.0).round() / 100.0,
                });
            }
        }

        // Cross-field check: within the SAME row, T_min cannot exceed T_max.
        // Group readings back up by date to check this pairwise.
        let mut by_date: std::collections::BTreeMap<NaiveDate, Vec<&ParsedReading>> = std::collections::BTreeMap::new();
        for r in &readings {
            by_date.entry(r.date).or_default().push(r);
        }
        let mut dates_with_tmin_tmax_violation = Vec::new();
        for (date, group) in &by_date {
            let t_max = group.iter().find(|r| r.metric_type == MetricType::TMax).map(|r| r.value);
            let t_min = group.iter().find(|r| r.metric_type == MetricType::TMin).map(|r| r.value);
            if let (Some(t_max), Some(t_min)) = (t_max, t_min) {
                if t_min > t_max {
                    errors.push(format!(
                        "Row for {}: physical violation: T_min ({:.2}°C) cannot exceed T_max ({:.2}°C)",
                        date, t_min, t_max
                    ));
                    dates_with_tmin_tmax_violation.push(*date);
                    if let Some(&row_num) = row_num_for_date.get(date) {
                        failed_row_numbers.insert(row_num);
                    }
                }
            }
        }
        if !dates_with_tmin_tmax_violation.is_empty() {
            readings.retain(|r| !dates_with_tmin_tmax_violation.contains(&r.date));
        }

        readings.sort_by_key(|r| r.date);

        let failed_rows = failed_row_numbers.len();
        Ok(IngestionReport {
            total_rows,
            successful_rows: total_rows.saturating_sub(failed_rows),
            failed_rows,
            readings,
            errors,
        })
    }

    fn validate_physical(metric_type: MetricType, value: f64, row_num: usize) -> Option<String> {
        match metric_type {
            MetricType::TMax | MetricType::TMin => {
                if value < -10.0 || value > 55.0 {
                    Some(format!(
                        "Row {}: biological threshold exceeded: {} = {:.2}°C outside [-10°C, 55°C]",
                        row_num,
                        metric_type.as_str(),
                        value
                    ))
                } else {
                    None
                }
            }
            MetricType::Rainfall => {
                if value < 0.0 {
                    Some(format!("Row {}: precipitation cannot be negative ({:.2} mm)", row_num, value))
                } else {
                    None
                }
            }
            MetricType::Radiation => {
                if value < 0.0 {
                    Some(format!("Row {}: solar radiation cannot be negative ({:.2} MJ/m²)", row_num, value))
                } else {
                    None
                }
            }
            MetricType::Humidity => {
                if !(0.0..=100.0).contains(&value) {
                    Some(format!(
                        "Row {}: relative humidity ({:.1}%) must be within [0.0, 100.0]%",
                        row_num, value
                    ))
                } else {
                    None
                }
            }
        }
    }

    fn find_column_index(
        headers: &csv::StringRecord,
        target_name: &str,
        canonical_label: &str,
        device_name: &str,
    ) -> Result<usize, AppError> {
        let target_clean = target_name.trim().to_lowercase();
        for (i, h) in headers.iter().enumerate() {
            if h.trim().to_lowercase() == target_clean {
                return Ok(i);
            }
        }

        Err(AppError::BadRequest(format!(
            "Missing column '{}' for canonical field '{}' configured in sensor profile '{}'. Available columns: [{}]",
            target_name,
            canonical_label,
            device_name,
            headers.iter().collect::<Vec<&str>>().join(", ")
        )))
    }

    fn parse_date(s: &str, format: &str) -> Option<NaiveDate> {
        // Try configured format first
        if let Ok(d) = NaiveDate::parse_from_str(s, format) {
            return Some(d);
        }

        // Common agronomic fallbacks if date has timestamp or alternative separators
        let first_word = s.split_whitespace().next().unwrap_or(s);
        if let Ok(d) = NaiveDate::parse_from_str(first_word, "%Y-%m-%d") {
            return Some(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(first_word, "%Y/%m/%d") {
            return Some(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(first_word, "%m/%d/%Y") {
            return Some(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(first_word, "%d/%m/%Y") {
            return Some(d);
        }

        None
    }

    fn parse_numeric(
        val: Option<&str>,
        row_num: usize,
        field_name: &str,
        errors: &mut Vec<String>,
    ) -> Option<f64> {
        match val {
            Some(s) if !s.trim().is_empty() => {
                let clean = s.trim().replace(',', ".");
                match clean.parse::<f64>() {
                    Ok(v) => Some(v),
                    Err(_) => {
                        errors.push(format!(
                            "Row {}: unable to parse numeric value '{}' for field '{}'",
                            row_num, s, field_name
                        ));
                        None
                    }
                }
            }
            _ => {
                errors.push(format!(
                    "Row {}: missing or empty value for required field '{}'",
                    row_num, field_name
                ));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_for(report: &IngestionReport, metric: MetricType) -> f64 {
        report.readings.iter().find(|r| r.metric_type == metric).unwrap().value
    }

    #[test]
    fn test_ingestion_pessl_csv() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,31.5,23.2,5.4,210.0,78.0
2026-07-02,32.0,24.0,0.0,225.0,74.5
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.total_rows, 2);
        assert_eq!(report.successful_rows, 2);
        assert_eq!(report.failed_rows, 0);
        assert_eq!(report.readings.len(), 10); // 5 metrics x 2 rows

        let row1: Vec<_> = report.readings.iter().filter(|r| r.date == NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()).collect();
        let t_max = row1.iter().find(|r| r.metric_type == MetricType::TMax).unwrap();
        assert_eq!(t_max.value, 31.5);
        // SolarRad 210 W/m² * 0.0864 = 18.14 MJ/m²
        let rad = row1.iter().find(|r| r.metric_type == MetricType::Radiation).unwrap();
        assert!((rad.value - 18.14).abs() < 0.05);
    }

    #[test]
    fn test_ingestion_davis_fahrenheit_and_inches() {
        let csv_data = "\
Date,Temp High,Temp Low,Rain,Solar Rad,Hum High
07/01/2026,86.0,68.0,1.0,18.5,80.0
";
        let mapping = DeviceMapping::davis_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.successful_rows, 1);
        // 86°F -> 30°C
        assert!((value_for(&report, MetricType::TMax) - 30.0).abs() < 1e-2);
        // 68°F -> 20°C
        assert!((value_for(&report, MetricType::TMin) - 20.0).abs() < 1e-2);
        // 1 inch -> 25.4 mm
        assert!((value_for(&report, MetricType::Rainfall) - 25.4).abs() < 1e-2);
    }

    #[test]
    fn test_ingestion_dragino_renke_modbus_scale() {
        let csv_data = "\
date,temp_max_raw,temp_min_raw,pulse_count,radiation_raw,humidity_raw
2026-07-01,335,235,15,195,815
";
        let mapping = DeviceMapping::dragino_renke_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.successful_rows, 1);
        assert_eq!(value_for(&report, MetricType::TMax), 33.5);
        assert_eq!(value_for(&report, MetricType::TMin), 23.5);
        assert_eq!(value_for(&report, MetricType::Rainfall), 3.0); // 15 pulses * 0.2
        assert_eq!(value_for(&report, MetricType::Radiation), 19.5); // 195 * 0.1
        assert_eq!(value_for(&report, MetricType::Humidity), 81.5); // 815 * 0.1
    }

    #[test]
    fn a_standalone_single_metric_sensor_only_ingests_its_one_column() {
        let csv_data = "\
ts,rain_mm
2026-07-01,12.4
2026-07-02,0.0
";
        let mapping = DeviceMapping::single_metric(
            "sensor-rain-01",
            "Standalone Rain Gauge",
            "Generic",
            "ts",
            "%Y-%m-%d",
            MetricType::Rainfall,
            "rain_mm",
            "mm",
            1.0,
        );
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.successful_rows, 2);
        assert_eq!(report.readings.len(), 2);
        assert!(report.readings.iter().all(|r| r.metric_type == MetricType::Rainfall));
    }

    #[test]
    fn test_ingestion_rejects_physical_anomalies() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,20.0,25.0,0.0,15.0,75.0
2026-07-02,30.0,20.0,-5.0,15.0,75.0
2026-07-03,30.0,20.0,0.0,15.0,105.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.total_rows, 3);
        assert_eq!(report.failed_rows, 3);
        assert!(report.errors.iter().any(|e| e.contains("T_min (25.00°C) cannot exceed T_max (20.00°C)")));
        assert!(report.errors.iter().any(|e| e.contains("precipitation cannot be negative")));
        assert!(report.errors.iter().any(|e| e.contains("relative humidity (105.0%) must be within [0.0, 100.0]%")));
    }

    #[test]
    fn test_ingestion_missing_column_returns_transparent_error() {
        let csv_data = "\
Timestamp,WrongCol,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,31.5,23.2,5.4,210.0,78.0
";
        let mapping = DeviceMapping::pessl_preset();
        let err = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap_err();

        assert!(err.to_string().contains("Missing column 'AirTemp_Max'"));
    }

    #[test]
    fn test_ingestion_rejects_non_numeric_value() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,not_a_number,23.2,5.4,210.0,78.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.failed_rows, 1);
        assert!(report.errors.iter().any(|e| e.contains("unable to parse numeric value 'not_a_number'")));
    }

    #[test]
    fn test_ingestion_rejects_missing_date() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
,31.5,23.2,5.4,210.0,78.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.failed_rows, 1);
        assert!(report.errors[0].contains("missing or empty date value"));
    }

    #[test]
    fn test_ingestion_rejects_unparseable_date() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
not-a-date,31.5,23.2,5.4,210.0,78.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.failed_rows, 1);
        assert!(report.errors[0].contains("unable to parse date"));
    }

    #[test]
    fn test_ingestion_negative_radiation_rejected() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,30.0,20.0,0.0,-5.0,75.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.failed_rows, 1);
        assert!(report.errors.iter().any(|e| e.contains("solar radiation cannot be negative")));
    }

    #[test]
    fn test_ingestion_partial_batch_mixes_valid_and_invalid_rows() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,31.5,23.2,5.4,210.0,78.0
2026-07-02,bad_value,23.2,5.4,210.0,78.0
2026-07-03,32.0,24.0,0.0,225.0,74.5
,30.0,20.0,0.0,200.0,70.0
2026-07-05,20.0,25.0,0.0,200.0,70.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.total_rows, 5);
        assert_eq!(report.successful_rows, 2);
        assert_eq!(report.failed_rows, 3);
    }

    #[test]
    fn test_ingestion_empty_csv_body_yields_zero_rows_not_an_error() {
        let csv_data = "Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity\n";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.total_rows, 0);
        assert_eq!(report.successful_rows, 0);
        assert_eq!(report.failed_rows, 0);
    }
}
