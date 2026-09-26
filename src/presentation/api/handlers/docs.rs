//! # Oryza-Elo Architecture Guardrail: API Documentation (Scalar + OpenAPI 3.1)
//!
//! Exposes a modern, interactive Scalar API Reference and raw OpenAPI 3.1 JSON
//! specification for all edge microservice routes, schemas, and diagnostics.

use axum::http::header;
use axum::response::{Html, IntoResponse};
use serde_json::Value;

/// Raw OpenAPI 3.1 JSON specification.
pub const OPENAPI_SPEC_RAW: &str = r##"{
  "openapi": "3.1.0",
  "info": {
    "title": "Oryza-Elo Edge Engine API",
    "version": "0.1.0",
    "description": "High-performance edge computing microservice for rice phenological stage classification, biometeorological analytics, and offline sensor telemetry ingestion on rural Raspberry Pi / edge hardware.",
    "contact": {
      "name": "Asodya Agritech / Oryza-Elo Research Team",
      "email": "wilson@asodya.com"
    },
    "license": {
      "name": "MIT"
    }
  },
  "servers": [
    {
      "url": "/",
      "description": "Current Edge Station Host"
    },
    {
      "url": "http://127.0.0.1:8005",
      "description": "Local Edge Loopback (Port 8005)"
    }
  ],
  "tags": [
    {
      "name": "Health & Diagnostics",
      "description": "Probes for hardware, host OS, SQLite DB, ONNX runtime, and background cron scheduler."
    },
    {
      "name": "Farm Parcels",
      "description": "CRUD management of agricultural rice parcels, cultivar varieties, ecosystems, and geo coordinates."
    },
    {
      "name": "Devices & Sensors",
      "description": "Factory hardware presets (Davis, Pessl, Dragino, Agro IoT) and custom column/telemetry mappings."
    },
    {
      "name": "Weather & Telemetry",
      "description": "Ingestion of time-series weather records, CSV batch processing, and individual raw sensor readings."
    },
    {
      "name": "Phenology & AI Inference",
      "description": "Edge AI inference with ONNX tract runtime, phenological phase classification, and counterfactual simulation."
    },
    {
      "name": "Edge Configuration",
      "description": "Runtime key-value configuration for cron schedules, offline synchronization, and farm metadata."
    },
    {
      "name": "Edge Benchmarks",
      "description": "Microsecond-level latency, biometeorology, flash storage, and throughput telemetry benchmarks."
    },
    {
      "name": "Admin & Testing",
      "description": "Turnkey population of synthetic multi-sensor datasets and database purging for field evaluators."
    },
    {
      "name": "API Documentation",
      "description": "OpenAPI 3.1 specification and Scalar interactive reference UI."
    }
  ],
  "paths": {
    "/health": {
      "get": {
        "tags": [
          "Health & Diagnostics"
        ],
        "summary": "Liveness Probe (Infrastructure)",
        "description": "Lightweight ping endpoint for Docker, systemd watchdog, and network availability checks.",
        "responses": {
          "200": {
            "description": "Engine is alive",
            "content": {
              "application/json": {
                "schema": {
                  "type": "object",
                  "properties": {
                    "status": {
                      "type": "string",
                      "example": "ok"
                    },
                    "timestamp": {
                      "type": "string",
                      "format": "date-time"
                    }
                  }
                }
              }
            }
          }
        }
      }
    },
    "/ping": {
      "get": {
        "tags": [
          "Health & Diagnostics"
        ],
        "summary": "Liveness Ping Alias",
        "description": "Alias for /health liveness ping.",
        "responses": {
          "200": {
            "description": "Engine is alive"
          }
        }
      }
    },
    "/docs": {
      "get": {
        "tags": [
          "API Documentation"
        ],
        "summary": "Scalar Interactive API Documentation",
        "description": "Renders modern Scalar API Reference web interface with embedded OpenAPI 3.1 schema and interactive playground.",
        "responses": {
          "200": {
            "description": "Scalar API documentation HTML page",
            "content": {
              "text/html": {}
            }
          }
        }
      }
    },
    "/openapi.json": {
      "get": {
        "tags": [
          "API Documentation"
        ],
        "summary": "OpenAPI 3.1 JSON Specification",
        "description": "Returns the complete machine-readable OpenAPI 3.1 schema in JSON format.",
        "responses": {
          "200": {
            "description": "OpenAPI 3.1 JSON document",
            "content": {
              "application/json": {}
            }
          }
        }
      }
    },
    "/api/v1/health": {
      "get": {
        "tags": [
          "Health & Diagnostics"
        ],
        "summary": "Consolidated System & Application Health",
        "description": "Returns full diagnosis including host metrics (CPU, RAM, Disk, Uptime) and application components (DB, ONNX, Cron).",
        "responses": {
          "200": {
            "description": "Consolidated health report",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/ConsolidatedHealth"
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/health/system": {
      "get": {
        "tags": [
          "Health & Diagnostics"
        ],
        "summary": "Hardware & OS System Health",
        "description": "Returns CPU load, memory usage, swap, disk free space, and edge host metadata.",
        "responses": {
          "200": {
            "description": "System hardware telemetry",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SystemHealth"
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/health/app": {
      "get": {
        "tags": [
          "Health & Diagnostics"
        ],
        "summary": "Application Subsystems Health",
        "description": "Reports SQLite connection status, ONNX model readiness, record counts, and background cron scheduler status.",
        "responses": {
          "200": {
            "description": "Application diagnostic metrics",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/AppHealth"
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/parcels": {
      "get": {
        "tags": [
          "Farm Parcels"
        ],
        "summary": "List All Rice Parcels",
        "description": "Retrieves all agricultural rice parcels registered on this edge station.",
        "responses": {
          "200": {
            "description": "List of parcels",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/FarmParcel"
                  }
                }
              }
            }
          }
        }
      },
      "post": {
        "tags": [
          "Farm Parcels"
        ],
        "summary": "Create New Rice Parcel",
        "description": "Registers a new rice parcel with its cultivar variety, planting date, and coordinates.",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/FarmParcel"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Parcel created successfully",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/FarmParcel"
                }
              }
            }
          },
          "400": {
            "description": "Invalid input coordinates or missing parameters"
          }
        }
      }
    },
    "/api/v1/parcels/{id}": {
      "get": {
        "tags": [
          "Farm Parcels"
        ],
        "summary": "Get Parcel by ID",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Parcel details",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/FarmParcel"
                }
              }
            }
          },
          "404": {
            "description": "Parcel not found"
          }
        }
      },
      "put": {
        "tags": [
          "Farm Parcels"
        ],
        "summary": "Update Parcel",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/FarmParcel"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Parcel updated",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/FarmParcel"
                }
              }
            }
          },
          "400": {
            "description": "Validation error"
          },
          "404": {
            "description": "Parcel not found"
          }
        }
      },
      "delete": {
        "tags": [
          "Farm Parcels"
        ],
        "summary": "Delete Parcel",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Parcel deleted"
          },
          "404": {
            "description": "Parcel not found"
          }
        }
      }
    },
    "/api/v1/devices/presets": {
      "get": {
        "tags": [
          "Devices & Sensors"
        ],
        "summary": "List Official Device Presets",
        "description": "Returns factory profiles for known weather stations (Davis Vantage Pro2, Pessl iMetos 3.3, Dragino/Renke RS485 Modbus, Agro IoT Station).",
        "responses": {
          "200": {
            "description": "List of factory presets",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/DevicePreset"
                  }
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/devices/mappings": {
      "get": {
        "tags": [
          "Devices & Sensors"
        ],
        "summary": "List Custom Device Mappings",
        "description": "Lists custom CSV/telemetry field mappings registered by the user.",
        "responses": {
          "200": {
            "description": "List of device mappings",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/DeviceMapping"
                  }
                }
              }
            }
          }
        }
      },
      "post": {
        "tags": [
          "Devices & Sensors"
        ],
        "summary": "Create Custom Device Mapping",
        "description": "Registers a new custom telemetry mapping for non-standard IoT sensors or CSV formats.",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/DeviceMapping"
              }
            }
          }
        },
        "responses": {
          "201": {
            "description": "Mapping created",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/DeviceMapping"
                }
              }
            }
          },
          "400": {
            "description": "Cannot overwrite preset or invalid"
          }
        }
      }
    },
    "/api/v1/devices/mappings/{id}": {
      "get": {
        "tags": [
          "Devices & Sensors"
        ],
        "summary": "Get Device Mapping by ID",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Device mapping",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/DeviceMapping"
                }
              }
            }
          },
          "404": {
            "description": "Mapping not found"
          }
        }
      },
      "delete": {
        "tags": [
          "Devices & Sensors"
        ],
        "summary": "Delete Device Mapping",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Mapping deleted"
          },
          "400": {
            "description": "Cannot delete official factory preset"
          },
          "404": {
            "description": "Mapping not found"
          }
        }
      }
    },
    "/api/v1/weather/upload-csv": {
      "post": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Upload Weather CSV File (Multipart)",
        "description": "Parses and ingests a multipart CSV file with column mapping matching a preset or custom device mapping.",
        "requestBody": {
          "required": true,
          "content": {
            "multipart/form-data": {
              "schema": {
                "type": "object",
                "required": [
                  "parcel_id",
                  "mapping_id",
                  "file"
                ],
                "properties": {
                  "parcel_id": {
                    "type": "string"
                  },
                  "mapping_id": {
                    "type": "string"
                  },
                  "file": {
                    "type": "string",
                    "format": "binary"
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Ingestion summary",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/IngestionSummary"
                }
              }
            }
          },
          "400": {
            "description": "Parsing failure"
          },
          "404": {
            "description": "Not found"
          }
        }
      }
    },
    "/api/v1/weather/ingest": {
      "post": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Ingest Weather CSV Content (JSON Payload)",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "parcel_id",
                  "mapping_id",
                  "csv_content"
                ],
                "properties": {
                  "parcel_id": {
                    "type": "string"
                  },
                  "mapping_id": {
                    "type": "string"
                  },
                  "csv_content": {
                    "type": "string"
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Ingestion summary",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/IngestionSummary"
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/weather/record": {
      "post": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Ingest Single Daily Weather Record",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "$ref": "#/components/schemas/SingleRecordInput"
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Record ingested",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/WeatherRecord"
                }
              }
            }
          },
          "400": {
            "description": "Physical anomaly"
          }
        }
      }
    },
    "/api/v1/weather/sensor-reading": {
      "post": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Ingest Single Raw Sensor Reading",
        "description": "Stores a raw sub-daily telemetry reading in the dedicated metric table and updates daily consolidated aggregates.",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "parcel_id",
                  "sensor_id",
                  "metric_type",
                  "value"
                ],
                "properties": {
                  "parcel_id": {
                    "type": "string"
                  },
                  "sensor_id": {
                    "type": "string"
                  },
                  "metric_type": {
                    "type": "string",
                    "enum": [
                      "t_max",
                      "t_min",
                      "rainfall",
                      "radiation",
                      "humidity"
                    ]
                  },
                  "value": {
                    "type": "number",
                    "format": "double"
                  },
                  "recorded_at": {
                    "type": "string",
                    "format": "date-time"
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Sensor reading stored",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/SensorReading"
                }
              }
            }
          },
          "400": {
            "description": "Anomaly or invalid metric"
          }
        }
      }
    },
    "/api/v1/weather/sensor-readings": {
      "get": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Query Raw Sensor Readings",
        "parameters": [
          {
            "name": "parcel_id",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "metric_type",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string",
              "enum": [
                "t_max",
                "t_min",
                "rainfall",
                "radiation",
                "humidity"
              ]
            }
          },
          {
            "name": "limit",
            "in": "query",
            "required": false,
            "schema": {
              "type": "integer",
              "default": 200
            }
          }
        ],
        "responses": {
          "200": {
            "description": "List of sensor readings",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/SensorReading"
                  }
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/weather/sensor-readings/{id}": {
      "delete": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Delete Raw Sensor Reading by ID",
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": {
              "type": "integer"
            }
          },
          {
            "name": "metric_type",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string",
              "enum": [
                "t_max",
                "t_min",
                "rainfall",
                "radiation",
                "humidity"
              ]
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Reading deleted and aggregate recomputed"
          },
          "404": {
            "description": "Reading not found"
          }
        }
      }
    },
    "/api/v1/weather/records": {
      "get": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Get Daily Weather Records",
        "parameters": [
          {
            "name": "parcel_id",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string"
            }
          },
          {
            "name": "start_date",
            "in": "query",
            "required": false,
            "schema": {
              "type": "string",
              "format": "date"
            }
          },
          {
            "name": "end_date",
            "in": "query",
            "required": false,
            "schema": {
              "type": "string",
              "format": "date"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Daily records",
            "content": {
              "application/json": {
                "schema": {
                  "type": "array",
                  "items": {
                    "$ref": "#/components/schemas/WeatherRecord"
                  }
                }
              }
            }
          }
        }
      },
      "delete": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Delete Weather Records by Dates",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "parcel_id",
                  "dates"
                ],
                "properties": {
                  "parcel_id": {
                    "type": "string"
                  },
                  "dates": {
                    "type": "array",
                    "items": {
                      "type": "string",
                      "format": "date"
                    }
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Records deleted"
          }
        }
      }
    },
    "/api/v1/weather/analytics": {
      "get": {
        "tags": [
          "Weather & Telemetry"
        ],
        "summary": "Biometeorological Analytics Summary",
        "parameters": [
          {
            "name": "parcel_id",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Biomet analytics",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/WeatherAnalyticsResponse"
                }
              }
            }
          },
          "422": {
            "description": "Insufficient history"
          }
        }
      }
    },
    "/api/v1/phenology/predict": {
      "post": {
        "tags": [
          "Phenology & AI Inference"
        ],
        "summary": "Predict Rice Phenological Stage (AI Tract ONNX)",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "parcel_id"
                ],
                "properties": {
                  "parcel_id": {
                    "type": "string"
                  },
                  "locale": {
                    "type": "string",
                    "default": "pt-BR"
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Stage prediction",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/PredictionResult"
                }
              }
            }
          },
          "422": {
            "description": "Insufficient weather history"
          }
        }
      }
    },
    "/api/v1/phenology/simulate": {
      "post": {
        "tags": [
          "Phenology & AI Inference"
        ],
        "summary": "Simulate Phenology Weather Scenario",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "required": [
                  "parcel_id",
                  "temperature_delta_c"
                ],
                "properties": {
                  "parcel_id": {
                    "type": "string"
                  },
                  "temperature_delta_c": {
                    "type": "number",
                    "format": "double"
                  },
                  "rainfall_factor": {
                    "type": "number",
                    "format": "double",
                    "default": 1.0
                  },
                  "days_ahead": {
                    "type": "integer",
                    "default": 14
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Simulation trajectory"
          }
        }
      }
    },
    "/api/v1/phenology/latest": {
      "get": {
        "tags": [
          "Phenology & AI Inference"
        ],
        "summary": "Get Latest Prediction for Parcel",
        "parameters": [
          {
            "name": "parcel_id",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Latest prediction or null"
          }
        }
      }
    },
    "/api/v1/phenology/history": {
      "get": {
        "tags": [
          "Phenology & AI Inference"
        ],
        "summary": "Get Prediction History for Parcel",
        "parameters": [
          {
            "name": "parcel_id",
            "in": "query",
            "required": true,
            "schema": {
              "type": "string"
            }
          }
        ],
        "responses": {
          "200": {
            "description": "Stage predictions history"
          }
        }
      }
    },
    "/api/v1/config": {
      "get": {
        "tags": [
          "Edge Configuration"
        ],
        "summary": "Get All Edge Node Configurations",
        "responses": {
          "200": {
            "description": "Configuration dictionary",
            "content": {
              "application/json": {
                "schema": {
                  "type": "object",
                  "additionalProperties": {
                    "type": "string"
                  }
                }
              }
            }
          }
        }
      },
      "put": {
        "tags": [
          "Edge Configuration"
        ],
        "summary": "Update Edge Node Configuration",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "additionalProperties": {
                  "type": "string"
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Configuration updated"
          },
          "400": {
            "description": "Validation error"
          }
        }
      }
    },
    "/api/v1/benchmarks/latency": {
      "get": {
        "tags": [
          "Edge Benchmarks"
        ],
        "summary": "ONNX Model CPU Inference Latency Benchmark",
        "description": "Runs 1000 unit iterations measuring mean, p50, p95, and p99 latency in microseconds on edge CPU.",
        "responses": {
          "200": {
            "description": "Latency benchmark stats",
            "content": {
              "application/json": {
                "schema": {
                  "$ref": "#/components/schemas/BenchmarkLatencyResult"
                }
              }
            }
          }
        }
      }
    },
    "/api/v1/benchmarks/biomet": {
      "get": {
        "tags": [
          "Edge Benchmarks"
        ],
        "summary": "Biometeorology Calculator Benchmark",
        "responses": {
          "200": {
            "description": "Calculation speed metrics"
          }
        }
      }
    },
    "/api/v1/benchmarks/storage": {
      "get": {
        "tags": [
          "Edge Benchmarks"
        ],
        "summary": "SQLite Storage Flash/SD Benchmark",
        "responses": {
          "200": {
            "description": "IOPS and latency"
          }
        }
      }
    },
    "/api/v1/benchmarks/throughput": {
      "get": {
        "tags": [
          "Edge Benchmarks"
        ],
        "summary": "End-to-End Pipeline Throughput Benchmark",
        "responses": {
          "200": {
            "description": "Records per second processed"
          }
        }
      }
    },
    "/api/v1/admin/populate": {
      "post": {
        "tags": [
          "Admin & Testing"
        ],
        "summary": "Populate Synthetic Multi-Sensor Test Data",
        "requestBody": {
          "required": false,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "properties": {
                  "days": {
                    "type": "integer",
                    "default": 75
                  },
                  "parcels": {
                    "type": "integer",
                    "default": 4
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Database populated with multi-sensor readings"
          }
        }
      }
    },
    "/api/v1/admin/clean": {
      "post": {
        "tags": [
          "Admin & Testing"
        ],
        "summary": "Clean Test Database & Restore Presets",
        "requestBody": {
          "required": false,
          "content": {
            "application/json": {
              "schema": {
                "type": "object",
                "properties": {
                  "reset_presets": {
                    "type": "boolean",
                    "default": true
                  }
                }
              }
            }
          }
        },
        "responses": {
          "200": {
            "description": "Database cleansed, presets intact"
          }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "ConsolidatedHealth": {
        "type": "object",
        "properties": {
          "status": {
            "type": "string",
            "example": "healthy"
          },
          "system": {
            "$ref": "#/components/schemas/SystemHealth"
          },
          "app": {
            "$ref": "#/components/schemas/AppHealth"
          }
        }
      },
      "SystemHealth": {
        "type": "object",
        "properties": {
          "status": {
            "type": "string",
            "example": "healthy"
          },
          "os_name": {
            "type": "string",
            "example": "Linux"
          },
          "os_version": {
            "type": "string"
          },
          "hostname": {
            "type": "string"
          },
          "cpu_cores": {
            "type": "integer"
          },
          "cpu_usage_pct": {
            "type": "number",
            "format": "float"
          },
          "memory_total_bytes": {
            "type": "integer"
          },
          "memory_used_bytes": {
            "type": "integer"
          },
          "memory_usage_pct": {
            "type": "number",
            "format": "float"
          },
          "disk_total_bytes": {
            "type": "integer"
          },
          "disk_available_bytes": {
            "type": "integer"
          },
          "uptime_seconds": {
            "type": "integer"
          }
        }
      },
      "AppHealth": {
        "type": "object",
        "properties": {
          "status": {
            "type": "string",
            "example": "healthy"
          },
          "sqlite_connected": {
            "type": "boolean"
          },
          "onnx_model_loaded": {
            "type": "boolean"
          },
          "parcels_count": {
            "type": "integer"
          },
          "weather_records_count": {
            "type": "integer"
          },
          "cron_status": {
            "type": "string"
          },
          "cron_target_time": {
            "type": "string",
            "example": "23:59"
          }
        }
      },
      "FarmParcel": {
        "type": "object",
        "required": [
          "id",
          "name",
          "rice_variety",
          "rice_ecosystem",
          "latitude",
          "longitude",
          "planting_date",
          "area_hectares"
        ],
        "properties": {
          "id": {
            "type": "string",
            "example": "talhao-central-esalq"
          },
          "name": {
            "type": "string",
            "example": "Talh\u00e3o Central Esalq (Suphan Buri)"
          },
          "rice_variety": {
            "type": "string",
            "example": "\u0e02\u0e32\u0e27\u0e14\u0e2d\u0e01\u0e21\u0e30\u0e25\u0e34 105"
          },
          "rice_ecosystem": {
            "type": "string",
            "example": "\u0e19\u0e32\u0e0a\u0e25\u0e1b\u0e23\u0e30\u0e17\u0e32\u0e19"
          },
          "latitude": {
            "type": "number",
            "format": "double",
            "example": 14.4745
          },
          "longitude": {
            "type": "number",
            "format": "double",
            "example": 100.1177
          },
          "planting_date": {
            "type": "string",
            "format": "date",
            "example": "2026-07-23"
          },
          "area_hectares": {
            "type": "number",
            "format": "double",
            "example": 15.0
          }
        }
      },
      "DevicePreset": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string",
            "example": "preset_davis_vantage"
          },
          "name": {
            "type": "string",
            "example": "Davis Vantage Pro2 (Standard Export)"
          },
          "vendor": {
            "type": "string",
            "example": "Davis Instruments"
          },
          "date_format": {
            "type": "string",
            "example": "YYYY-MM-DD"
          },
          "delimiter": {
            "type": "string",
            "example": ","
          }
        }
      },
      "DeviceMapping": {
        "type": "object",
        "properties": {
          "id": {
            "type": "string"
          },
          "name": {
            "type": "string"
          },
          "vendor": {
            "type": "string"
          },
          "is_preset": {
            "type": "boolean"
          },
          "date_format": {
            "type": "string"
          },
          "delimiter": {
            "type": "string"
          }
        }
      },
      "WeatherRecord": {
        "type": "object",
        "properties": {
          "date": {
            "type": "string",
            "format": "date",
            "example": "2026-09-26"
          },
          "t_max": {
            "type": "number",
            "format": "double",
            "nullable": true,
            "example": 33.2
          },
          "t_min": {
            "type": "number",
            "format": "double",
            "nullable": true,
            "example": 22.8
          },
          "precipitation_mm": {
            "type": "number",
            "format": "double",
            "nullable": true,
            "example": 12.0
          },
          "radiation_mj_m2": {
            "type": "number",
            "format": "double",
            "nullable": true,
            "example": 18.5
          },
          "relative_humidity_pct": {
            "type": "number",
            "format": "double",
            "nullable": true,
            "example": 78.0
          },
          "daily_gdd": {
            "type": "number",
            "format": "double",
            "nullable": true,
            "example": 18.0
          }
        }
      },
      "SensorReading": {
        "type": "object",
        "properties": {
          "id": {
            "type": "integer",
            "example": 400
          },
          "parcel_id": {
            "type": "string",
            "example": "talhao-central-esalq"
          },
          "sensor_id": {
            "type": "string",
            "example": "sensor_pluviometro_avulso"
          },
          "metric_type": {
            "type": "string",
            "example": "rainfall"
          },
          "value": {
            "type": "number",
            "format": "double",
            "example": 6.5
          },
          "recorded_at": {
            "type": "string",
            "format": "date-time",
            "example": "2026-09-26T18:45:00Z"
          },
          "received_at": {
            "type": "string",
            "format": "date-time"
          }
        }
      },
      "SingleRecordInput": {
        "type": "object",
        "required": [
          "parcel_id",
          "date"
        ],
        "properties": {
          "parcel_id": {
            "type": "string"
          },
          "date": {
            "type": "string",
            "format": "date"
          },
          "t_max": {
            "type": "number",
            "format": "double",
            "nullable": true
          },
          "t_min": {
            "type": "number",
            "format": "double",
            "nullable": true
          },
          "precipitation_mm": {
            "type": "number",
            "format": "double",
            "nullable": true
          },
          "radiation_mj_m2": {
            "type": "number",
            "format": "double",
            "nullable": true
          },
          "relative_humidity_pct": {
            "type": "number",
            "format": "double",
            "nullable": true
          }
        }
      },
      "IngestionSummary": {
        "type": "object",
        "properties": {
          "total_rows": {
            "type": "integer"
          },
          "valid_rows": {
            "type": "integer"
          },
          "invalid_rows": {
            "type": "integer"
          },
          "errors": {
            "type": "array",
            "items": {
              "type": "string"
            }
          }
        }
      },
      "WeatherAnalyticsResponse": {
        "type": "object",
        "properties": {
          "days_count": {
            "type": "integer"
          },
          "gdd_sum": {
            "type": "number",
            "format": "double"
          },
          "rainfall_sum_mm": {
            "type": "number",
            "format": "double"
          },
          "t_max_peak_c": {
            "type": "number",
            "format": "double"
          },
          "t_min_valley_c": {
            "type": "number",
            "format": "double"
          },
          "heat_stress_days": {
            "type": "integer"
          }
        }
      },
      "PredictionResult": {
        "type": "object",
        "properties": {
          "parcel_id": {
            "type": "string"
          },
          "stage_index": {
            "type": "integer"
          },
          "bbch_scale": {
            "type": "integer",
            "example": 65
          },
          "macro_phase": {
            "type": "string",
            "example": "Reproductive"
          },
          "stage_name": {
            "type": "string",
            "example": "Flowering / Anthesis"
          },
          "confidence_score": {
            "type": "number",
            "format": "float",
            "example": 0.94
          },
          "prediction_date": {
            "type": "string",
            "format": "date"
          }
        }
      },
      "BenchmarkLatencyResult": {
        "type": "object",
        "properties": {
          "iterations": {
            "type": "integer",
            "example": 1000
          },
          "mean_latency_us": {
            "type": "number",
            "format": "double",
            "example": 29.2
          },
          "p50_latency_us": {
            "type": "number",
            "format": "double",
            "example": 22.1
          },
          "p95_latency_us": {
            "type": "number",
            "format": "double",
            "example": 42.9
          },
          "p99_latency_us": {
            "type": "number",
            "format": "double",
            "example": 140.9
          }
        }
      }
    }
  }
}"##;

/// Handler serving the OpenAPI 3.1 JSON specification.
pub async fn openapi_json_handler() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/json; charset=utf-8")], OPENAPI_SPEC_RAW)
}

/// Handler serving the modern Scalar API Reference documentation UI.
pub async fn scalar_docs_handler() -> impl IntoResponse {
    let html = format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <title>Oryza-Elo Edge Engine | Scalar API Reference</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="description" content="Oryza-Elo Edge Engine OpenAPI 3.1 interactive specification rendered with Scalar" />
    <link rel="icon" type="image/png" href="/favicon.png" />
    <style>
      body {{
        margin: 0;
        background-color: #0d110d;
      }}
    </style>
  </head>
  <body>
    <script
      id="api-reference"
      type="application/json"
      data-configuration='{{"theme":"deepSpace","layout":"modern","darkMode":true,"showSidebar":true,"searchHotKey":"k","metaData":{{"title":"Oryza-Elo Edge Engine API Reference","description":"Offline-first Edge Station API for Rice Phenological Classification & Biometeorology"}}}}'>
      {spec_json}
    </script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>"#,
        spec_json = OPENAPI_SPEC_RAW
    );

    Html(html)
}

/// Helper function to parse the spec into a serde_json::Value if needed.
pub fn openapi_spec() -> Value {
    serde_json::from_str(OPENAPI_SPEC_RAW).unwrap_or_else(|_| serde_json::json!({}))
}
