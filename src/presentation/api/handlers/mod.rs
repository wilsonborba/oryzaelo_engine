//! # Oryza-Elo Architecture Guardrail: API Handlers
//!
//! Presentation endpoint handlers for REST API.

pub mod admin;
pub mod benchmark;
pub mod config;
pub mod devices;
pub mod health;
pub mod parcels;
pub mod phenology;
pub mod weather;

pub use admin::*;
pub use benchmark::*;
pub use config::*;
pub use devices::*;
pub use health::*;
pub use parcels::*;
pub use phenology::*;
pub use weather::*;


