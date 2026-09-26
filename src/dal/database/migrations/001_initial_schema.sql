-- Oryza-Elo Edge SQLite Schema Migration: 001_initial_schema.sql
-- Confinement: strictly inside src/dal/data/local/oryza_elo_edge.db

CREATE TABLE IF NOT EXISTS parcels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    rice_variety TEXT NOT NULL,
    rice_ecosystem TEXT NOT NULL,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    planting_date TEXT NOT NULL,
    area_hectares REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- A registered sensor profile: a physical device (or a manual/synthetic
-- source) mapping its own raw column(s) onto one or more canonical metrics.
-- `metrics_json` is a JSON array of {metric_type, column_name, unit, scale}
-- objects: a standalone rain gauge declares exactly one entry, a bundled
-- weather station (Davis Vantage Pro2, Pessl iMetos) declares all 5 --
-- the schema never forces a sensor to claim a metric it doesn't measure.
CREATE TABLE IF NOT EXISTS device_mappings (
    id TEXT PRIMARY KEY,
    device_name TEXT NOT NULL,
    manufacturer TEXT NOT NULL,
    is_preset INTEGER NOT NULL DEFAULT 0,
    date_col TEXT NOT NULL,
    date_format TEXT NOT NULL DEFAULT '%Y-%m-%d',
    metrics_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Raw per-sensor readings, one physical table per canonical metric so each
-- sensor's own data (and its exact reporting cadence) is fully traceable
-- back to the sensor that produced it, independent of any other sensor.
CREATE TABLE IF NOT EXISTS sensor_readings_t_max (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parcel_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    value REAL NOT NULL,
    recorded_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_readings_t_max_parcel_time ON sensor_readings_t_max(parcel_id, recorded_at DESC);

CREATE TABLE IF NOT EXISTS sensor_readings_t_min (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parcel_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    value REAL NOT NULL,
    recorded_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_readings_t_min_parcel_time ON sensor_readings_t_min(parcel_id, recorded_at DESC);

CREATE TABLE IF NOT EXISTS sensor_readings_rainfall (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parcel_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    value REAL NOT NULL,
    recorded_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_readings_rainfall_parcel_time ON sensor_readings_rainfall(parcel_id, recorded_at DESC);

CREATE TABLE IF NOT EXISTS sensor_readings_radiation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parcel_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    value REAL NOT NULL,
    recorded_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_readings_radiation_parcel_time ON sensor_readings_radiation(parcel_id, recorded_at DESC);

CREATE TABLE IF NOT EXISTS sensor_readings_humidity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parcel_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    value REAL NOT NULL,
    recorded_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_readings_humidity_parcel_time ON sensor_readings_humidity(parcel_id, recorded_at DESC);

-- Daily aggregate: derived from the 5 raw reading tables above (latest
-- reading per metric per calendar day), never written to directly by an
-- ingestion call. Every metric field is nullable -- a day is legitimately
-- partial until every sensor that covers this parcel has reported for it --
-- and each field records exactly which sensor produced it.
CREATE TABLE IF NOT EXISTS weather_records (
    parcel_id TEXT NOT NULL,
    record_date TEXT NOT NULL,
    t_max REAL,
    t_min REAL,
    precipitation_mm REAL,
    radiation_mj_m2 REAL,
    relative_humidity_pct REAL,
    t_max_sensor_id TEXT,
    t_min_sensor_id TEXT,
    rainfall_sensor_id TEXT,
    radiation_sensor_id TEXT,
    humidity_sensor_id TEXT,
    is_partial INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (parcel_id, record_date),
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_weather_parcel_date ON weather_records(parcel_id, record_date DESC);

CREATE TABLE IF NOT EXISTS prediction_history (
    id TEXT PRIMARY KEY,
    parcel_id TEXT NOT NULL,
    evaluated_at TEXT NOT NULL,
    macro_phase TEXT NOT NULL,
    granular_stage TEXT NOT NULL,
    confidence REAL NOT NULL,
    is_transitioning INTEGER NOT NULL,
    probabilities_json TEXT NOT NULL,
    advisory_json TEXT NOT NULL,
    metrics_json TEXT NOT NULL,
    FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_predictions_parcel_eval ON prediction_history(parcel_id, evaluated_at DESC);

CREATE TABLE IF NOT EXISTS edge_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
