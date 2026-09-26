//! # Oryza-Elo Architecture Guardrail: Domain Models
//!
//! ## Responsabilidade:
//! Estruturas de dados (structs), contratos de entrada/saída (inputs/outputs).
//! IMPORTANTE: Modelos de Machine Learning (pesos, estimadores) NUNCA entram aqui.

pub mod advisory;
pub mod device_mapping;
pub mod farm;
pub mod locale;
pub mod metric_type;
pub mod phenology;
pub mod sensor_reading;
pub mod weather;

pub use advisory::*;
pub use device_mapping::*;
pub use farm::*;
pub use locale::*;
pub use metric_type::*;
pub use phenology::*;
pub use sensor_reading::*;
pub use weather::*;
