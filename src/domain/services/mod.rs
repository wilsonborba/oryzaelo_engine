//! # Oryza-Elo Architecture Guardrail: Domain Services
//!
//! ## Responsabilidade:
//! Lógica de negócio pura: serviço de cálculo agronômico de GDD, serviço de inferência
//! fenológica, orquestração entre adaptadores DAL e respostas para presentation.

pub mod agronomic_advisor;
pub mod biomet_calculator;
pub mod csv_ingestion;
pub mod weather_analytics;

pub use agronomic_advisor::*;
pub use biomet_calculator::*;
pub use csv_ingestion::*;
pub use weather_analytics::*;
