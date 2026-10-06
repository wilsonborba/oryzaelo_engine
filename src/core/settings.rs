//! # Oryza-Elo Architecture Guardrail: Settings & Configuration
//!
//! ## Regras Fundamentais (AI_AGENT_Working_Rules.md, Regra 1):
//! 1. **Configurações Reutilizáveis e Não-Secretas Globais**:
//!    - Devem viver aqui no código (`settings.rs`).
//!    - Exemplos: URLs públicas de APIs (como NASA POWER), parâmetros agronômicos (temperatura base
//!      de 10°C) e limites de latência da tese (< 200 ms).
//! 2. **Segredos, Credenciais, Senhas e Caminhos Locais Específicos de Máquina**:
//!    - Paths locais, keys, senhas e tokens DEVEM ser definidos no `.env` e ter valores
//!      estritamente NULOS (`Option::None`) aqui em `settings.rs` por padrão.
//!    - Cada pessoa/máquina que clona o projeto define seus próprios caminhos e credenciais no `.env`.
//!    - Em `settings.rs`, qualquer variável de caminho local ou segredo nasce como `None`.
//! 3. **Arquitetura de Apresentação (Dualidade Flutter Web: Borda vs Nuvem)**:
//!    - No Raspberry Pi (modo local/borda): o binário Rust serve diretamente os assets estáticos
//!      do Flutter Web via `ServeDir` se `STATIC_DIR` estiver configurado no `.env` e a pasta existir.
//!    - Na Nuvem Asodya: `static_dir` permanece `None`, pois o frontend vive e é distribuído
//!      globalmente via Cloudflare Pages e o Axum atua como API headless pura.

use dotenvy::dotenv;
use lazy_static::lazy_static;
use std::env;

/// Nome oficial da aplicação
pub const DEFAULT_APP_NAME: &str = "oryzaelo_engine";

/// Host padrão do servidor
pub const DEFAULT_SERVER_HOST: &str = "0.0.0.0";

/// Porta padrão do servidor de borda Axum
pub const DEFAULT_PORT: u16 = 8005;

/// Nível padrão de log estruturado
pub const DEFAULT_LOG_LEVEL: &str = "INFO";

/// Endpoint base público da API NASA POWER (Constante pública da aplicação)
pub const NASA_POWER_BASE_URL: &str = "https://power.larc.nasa.gov/api/temporal/daily/point";

/// Parâmetros Agronômicos Canônicos (Orizicultura)
/// Temperatura base fisiológica do arroz para cálculo de Graus-Dia Acumulados (GDD)
pub const RICE_BASE_TEMPERATURE_CELSIUS: f64 = 10.0;

/// Janelas temporais retrospectivas padrão para extração de séries climáticas (dias)
pub const RETROSPECTIVE_WINDOWS_DAYS: [u32; 4] = [7, 14, 30, 60];

/// Teto máximo de latência de inferência em hardware de borda (definido na hipótese do pré-projeto do TCC)
pub const EDGE_LATENCY_CEILING_MS: u64 = 200;

/// Meta de latência interna do backend em Rust (submilissegundos / microssegundos)
pub const RUST_EDGE_TARGET_LATENCY_MS: u64 = 5;

/// Único modo de autonomia suportado nesta versão (sincronização em nuvem é proposta futura)
pub const DEFAULT_AUTONOMY_MODE: &str = "local_only";

/// Único motor de inferência suportado nesta versão (sessão ONNX residente em memória)
pub const DEFAULT_INFERENCE_ENGINE: &str = "onnx_resident";

/// Struct para armazenar configurações em tempo de execução
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Settings {
    // Configurações do servidor
    pub app_name: String,
    pub server_host: String,
    pub server_port: u16,
    pub log_level: String,

    // Modos de operação declarados no .env (AUTONOMY_MODE e INFERENCE_ENGINE)
    pub autonomy_mode: String,
    pub inference_engine: String,

    // Parâmetros científicos e de APIs públicas
    pub nasa_power_url: String,
    pub base_temp_celsius: f64,

    // =========================================================================
    // VARIÁVEIS COM VALOR DEFAULT ESTRITAMENTE NULO (Option::None)
    // Devem ser fornecidas exclusivamente via .env local de cada máquina/usuário
    // =========================================================================
    /// Caminho local para o dataset bruto (definido no .env de quem clonou; default NULO)
    pub raw_data_path: Option<String>,

    /// Caminho local para o dataset processado (definido no .env de quem clonou; default NULO)
    pub processed_data_path: Option<String>,

    /// Caminho opcional para os assets estáticos do Flutter Web pré-buildado.
    /// - No Raspberry Pi / Modo Local: apontado no .env (ex: STATIC_DIR=src/presentation/static)
    /// - Na Nuvem Asodya: None (servido externamente via Cloudflare Pages)
    pub static_dir: Option<String>,

    /// Chave ou token de API privada (definido no .env; default NULO)
    pub private_api_token: Option<String>,

    /// Caminho do banco SQLite (DATABASE_PATH no .env; default NULO usa o caminho relativo padrão)
    pub database_path: Option<String>,
}

pub type AppSettings = Settings;

impl Default for Settings {
    fn default() -> Self {
        Self {
            app_name: DEFAULT_APP_NAME.to_string(),
            server_host: DEFAULT_SERVER_HOST.to_string(),
            server_port: DEFAULT_PORT,
            log_level: DEFAULT_LOG_LEVEL.to_string(),
            autonomy_mode: DEFAULT_AUTONOMY_MODE.to_string(),
            inference_engine: DEFAULT_INFERENCE_ENGINE.to_string(),
            nasa_power_url: NASA_POWER_BASE_URL.to_string(),
            base_temp_celsius: RICE_BASE_TEMPERATURE_CELSIUS,
            // Paths locais e segredos são estritamente nulos por padrão no código
            raw_data_path: None,
            processed_data_path: None,
            static_dir: None,
            private_api_token: None,
            database_path: None,
        }
    }
}

lazy_static! {
    pub static ref SETTINGS: Settings = {
        // Carrega variáveis do .env local se existir
        let _ = dotenv();

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| DEFAULT_SERVER_HOST.to_string());
        let server_port = env::var("SERVER_PORT")
            .or_else(|_| env::var("PORT"))
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string());
        let autonomy_mode = env::var("AUTONOMY_MODE").unwrap_or_else(|_| DEFAULT_AUTONOMY_MODE.to_string());
        let inference_engine = env::var("INFERENCE_ENGINE").unwrap_or_else(|_| DEFAULT_INFERENCE_ENGINE.to_string());
        let nasa_power_url = env::var("NASA_POWER_BASE_URL").unwrap_or_else(|_| NASA_POWER_BASE_URL.to_string());
        let base_temp_celsius = env::var("RICE_BASE_TEMPERATURE_CELSIUS")
            .ok()
            .and_then(|t| t.parse::<f64>().ok())
            .unwrap_or(RICE_BASE_TEMPERATURE_CELSIUS);

        // Preenchidos apenas a partir do .env local da máquina (default None / nulo)
        let raw_data_path = env::var("RAW_DATA_PATH").ok();
        let processed_data_path = env::var("PROCESSED_DATA_PATH").ok();
        let static_dir = env::var("STATIC_DIR").ok();
        let private_api_token = env::var("PRIVATE_API_TOKEN").ok();
        let database_path = env::var("DATABASE_PATH").ok().filter(|p| !p.trim().is_empty());

        Settings {
            app_name: DEFAULT_APP_NAME.to_string(),
            server_host,
            server_port,
            log_level,
            autonomy_mode,
            inference_engine,
            nasa_power_url,
            base_temp_celsius,
            raw_data_path,
            processed_data_path,
            static_dir,
            private_api_token,
            database_path,
        }
    };
}

#[allow(dead_code)]
pub fn app_settings() -> &'static Settings {
    &SETTINGS
}
