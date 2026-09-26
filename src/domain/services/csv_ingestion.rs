//! # Oryza-Elo Architecture Guardrail: CSV Ingestion Service
//!
//! Deterministic CSV parser enforcing explicit device mapping schemas, unit conversions,
//! and physical invariants without heuristic string guesswork.

use crate::core::error::AppError;
use crate::domain::models::device_mapping::DeviceMapping;
use crate::domain::models::weather::DailyWeatherRecord;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Detailed summary report of a CSV ingestion run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionReport {
    pub total_rows: usize,
    pub successful_rows: usize,
    pub failed_rows: usize,
    pub records: Vec<DailyWeatherRecord>,
    pub errors: Vec<String>,
}

pub struct CsvIngestionService;

impl CsvIngestionService {
    /// Parses a CSV string according to the explicit device schema mapping,
    /// converting units and validating physical invariants.
    pub fn parse_csv(csv_content: &str, mapping: &DeviceMapping) -> Result<IngestionReport, AppError> {
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
        let t_max_idx = Self::find_column_index(&headers, &mapping.t_max_col, "T_max", &mapping.device_name)?;
        let t_min_idx = Self::find_column_index(&headers, &mapping.t_min_col, "T_min", &mapping.device_name)?;
        let rain_idx = Self::find_column_index(&headers, &mapping.rain_col, "precipitation", &mapping.device_name)?;
        let rad_idx = Self::find_column_index(&headers, &mapping.rad_col, "radiation", &mapping.device_name)?;
        let rh_idx = Self::find_column_index(&headers, &mapping.rh_col, "relative humidity", &mapping.device_name)?;

        let mut records = Vec::new();
        let mut errors = Vec::new();
        let mut total_rows = 0;

        for (row_idx, result) in rdr.records().enumerate() {
            total_rows += 1;
            let row_num = row_idx + 2; // 1-based, accounts for header row

            let record = match result {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("Row {}: malformed CSV line: {}", row_num, e));
                    continue;
                }
            };

            // Parse Date
            let raw_date_str = match record.get(date_idx) {
                Some(s) if !s.trim().is_empty() => s.trim(),
                _ => {
                    errors.push(format!("Row {}: missing or empty date value", row_num));
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
                    continue;
                }
            };

            // Parse Floats
            let raw_t_max = match Self::parse_numeric(record.get(t_max_idx), row_num, "T_max", &mut errors) {
                Some(v) => v,
                None => continue,
            };
            let raw_t_min = match Self::parse_numeric(record.get(t_min_idx), row_num, "T_min", &mut errors) {
                Some(v) => v,
                None => continue,
            };
            let raw_rain = match Self::parse_numeric(record.get(rain_idx), row_num, "Rain", &mut errors) {
                Some(v) => v,
                None => continue,
            };
            let raw_rad = match Self::parse_numeric(record.get(rad_idx), row_num, "Radiation", &mut errors) {
                Some(v) => v,
                None => continue,
            };
            let raw_rh = match Self::parse_numeric(record.get(rh_idx), row_num, "Relative Humidity", &mut errors) {
                Some(v) => v,
                None => continue,
            };

            // Convert Units & Scales according to mapping configuration
            let t_max = mapping.convert_temp(raw_t_max, true);
            let t_min = mapping.convert_temp(raw_t_min, false);
            let rain = mapping.convert_rain(raw_rain);
            let rad = mapping.convert_radiation(raw_rad);
            let rh = mapping.convert_rh(raw_rh);

            // Validate Physical Invariants
            if t_min > t_max {
                errors.push(format!(
                    "Row {}: physical violation: T_min ({:.2}°C) cannot exceed T_max ({:.2}°C)",
                    row_num, t_min, t_max
                ));
                continue;
            }

            if t_min < -10.0 || t_max > 55.0 {
                errors.push(format!(
                    "Row {}: biological threshold exceeded: T_min={:.2}°C, T_max={:.2}°C outside [-10°C, 55°C]",
                    row_num, t_min, t_max
                ));
                continue;
            }

            if rain < 0.0 {
                errors.push(format!("Row {}: precipitation cannot be negative ({:.2} mm)", row_num, rain));
                continue;
            }

            if rad < 0.0 {
                errors.push(format!("Row {}: solar radiation cannot be negative ({:.2} MJ/m²)", row_num, rad));
                continue;
            }

            if !(0.0..=100.0).contains(&rh) {
                errors.push(format!(
                    "Row {}: relative humidity ({:.1}%) must be within [0.0, 100.0]%",
                    row_num, rh
                ));
                continue;
            }

            records.push(DailyWeatherRecord {
                date: parsed_date,
                t_max: (t_max * 100.0).round() / 100.0,
                t_min: (t_min * 100.0).round() / 100.0,
                precipitation_mm: (rain * 100.0).round() / 100.0,
                radiation_mj_m2: (rad * 100.0).round() / 100.0,
                relative_humidity_pct: (rh * 100.0).round() / 100.0,
                source: mapping.device_name.clone(),
            });
        }

        // Sort records ascending chronologically
        records.sort_by_key(|r| r.date);

        Ok(IngestionReport {
            total_rows,
            successful_rows: records.len(),
            failed_rows: errors.len(),
            records,
            errors,
        })
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
            "Missing column '{}' for canonical field '{}' configured in device profile '{}'. Available columns: [{}]",
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
        assert_eq!(report.records[0].t_max, 31.5);
        // SolarRad 210 W/m² * 0.0864 = 18.14 MJ/m²
        assert!((report.records[0].radiation_mj_m2 - 18.14).abs() < 0.05);
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
        assert!((report.records[0].t_max - 30.0).abs() < 1e-2);
        // 68°F -> 20°C
        assert!((report.records[0].t_min - 20.0).abs() < 1e-2);
        // 1 inch -> 25.4 mm
        assert!((report.records[0].precipitation_mm - 25.4).abs() < 1e-2);
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
        assert_eq!(report.records[0].t_max, 33.5);
        assert_eq!(report.records[0].t_min, 23.5);
        assert_eq!(report.records[0].precipitation_mm, 3.0); // 15 pulses * 0.2
        assert_eq!(report.records[0].radiation_mj_m2, 19.5); // 195 * 0.1
        assert_eq!(report.records[0].relative_humidity_pct, 81.5); // 815 * 0.1
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
        assert_eq!(report.successful_rows, 0);
        assert_eq!(report.failed_rows, 3);
        assert!(report.errors[0].contains("T_min (25.00°C) cannot exceed T_max (20.00°C)"));
        assert!(report.errors[1].contains("precipitation cannot be negative"));
        assert!(report.errors[2].contains("relative humidity (105.0%) must be within [0.0, 100.0]%"));
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

        assert_eq!(report.successful_rows, 0);
        assert_eq!(report.failed_rows, 1);
        assert!(report.errors[0].contains("unable to parse numeric value 'not_a_number'"));
    }

    #[test]
    fn test_ingestion_rejects_missing_numeric_value() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,,23.2,5.4,210.0,78.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.failed_rows, 1);
        assert!(report.errors[0].contains("missing or empty value for required field 'T_max'"));
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
    fn test_ingestion_rejects_extreme_temperature_outside_biological_threshold() {
        let csv_data = "\
Timestamp,AirTemp_Max,AirTemp_Min,Precipitation,SolarRad,RelHumidity
2026-07-01,60.0,58.0,0.0,15.0,75.0
";
        let mapping = DeviceMapping::pessl_preset();
        let report = CsvIngestionService::parse_csv(csv_data, &mapping).unwrap();

        assert_eq!(report.failed_rows, 1);
        assert!(report.errors[0].contains("biological threshold exceeded"));
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
        assert!(report.errors[0].contains("solar radiation cannot be negative"));
    }

    #[test]
    fn test_ingestion_partial_batch_mixes_valid_and_invalid_rows() {
        // A realistic upload: some rows clean, some broken in different ways.
        // successful_rows/failed_rows must reflect an accurate per-row split,
        // not fail (or succeed) the whole batch on the first bad row.
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
        // Successful rows are still sorted chronologically.
        assert!(report.records[0].date < report.records[1].date);
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
