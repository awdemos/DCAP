use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub discovery: DiscoveryConfig,
    pub settlement: SettlementConfig,
    pub trust: TrustConfig,
    pub llm: LLMConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
    pub max_connections: Option<usize>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: Option<u32>,
    pub min_connections: Option<u32>,
    pub acquire_timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DiscoveryConfig {
    pub endpoint: String,
    pub cache_ttl_seconds: Option<u64>,
    pub max_cache_size: Option<usize>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct SettlementConfig {
    pub stripe_secret_key: Option<String>,
    pub solana_rpc_url: Option<String>,
    pub escrow_service_url: Option<String>,
    pub webhook_secret: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct TrustConfig {
    pub jwt_secret: Option<String>,
    pub min_reputation_threshold: Option<u32>,
    pub reputation_decay_rate: Option<f64>,
    pub cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct LLMConfig {
    pub model: String,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: Option<String>,
    pub file: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            discovery: DiscoveryConfig::default(),
            settlement: SettlementConfig::default(),
            trust: TrustConfig::default(),
            llm: LLMConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8000,
            workers: Some(4),
            max_connections: Some(1000),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://negotiation.db".to_string(),
            max_connections: Some(10),
            min_connections: Some(1),
            acquire_timeout_seconds: Some(30),
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8000".to_string(),
            cache_ttl_seconds: Some(300),
            max_cache_size: Some(1000),
        }
    }
}

impl Default for SettlementConfig {
    fn default() -> Self {
        Self {
            stripe_secret_key: None,
            solana_rpc_url: None,
            escrow_service_url: None,
            webhook_secret: None,
        }
    }
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            jwt_secret: None,
            min_reputation_threshold: Some(50),
            reputation_decay_rate: Some(0.01),
            cache_ttl_seconds: Some(1800),
        }
    }
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            api_key: None,
            api_base: None,
            max_tokens: Some(1000),
            temperature: Some(0.7),
            timeout_seconds: Some(30),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: Some("json".to_string()),
            file: None,
        }
    }
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = std::fs::read_to_string(path)
            .map_err(|e| crate::error::NegotiationError::Config(format!("Failed to read config file: {}", e)))?;

        let config: AppConfig = toml::from_str(&config_str)
            .map_err(|e| crate::error::NegotiationError::Config(format!("Failed to parse config file: {}", e)))?;

        Ok(config)
    }

    pub fn load_with_env_overrides<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut config = Self::load(path)?;

        // Override with environment variables
        if let Ok(stripe_key) = std::env::var("STRIPE_SECRET_KEY") {
            config.settlement.stripe_secret_key = Some(stripe_key);
        }

        if let Ok(solana_url) = std::env::var("SOLANA_RPC_URL") {
            config.settlement.solana_rpc_url = Some(solana_url);
        }

        if let Ok(jwt_secret) = std::env::var("JWT_SECRET") {
            config.trust.jwt_secret = Some(jwt_secret);
        }

        if let Ok(llm_key) = std::env::var("OPENAI_API_KEY") {
            config.llm.api_key = Some(llm_key);
        }

        if let Ok(log_level) = std::env::var("RUST_LOG") {
            config.logging.level = log_level;
        }

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        // Validate server config
        if self.server.port == 0 {
            return Err(crate::error::NegotiationError::Config("Server port cannot be 0".to_string()));
        }

        // Validate database URL
        if self.database.url.is_empty() {
            return Err(crate::error::NegotiationError::Config("Database URL cannot be empty".to_string()));
        }

        // Validate discovery endpoint
        if self.discovery.endpoint.is_empty() {
            return Err(crate::error::NegotiationError::Config("Discovery endpoint cannot be empty".to_string()));
        }

        // Validate LLM config
        if self.llm.model.is_empty() {
            return Err(crate::error::NegotiationError::Config("LLM model cannot be empty".to_string()));
        }

        Ok(())
    }

    pub fn get_database_url(&self) -> &str {
        &self.database.url
    }

    pub fn get_server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn get_discovery_endpoint(&self) -> &str {
        &self.discovery.endpoint
    }

    pub fn is_stripe_configured(&self) -> bool {
        self.settlement.stripe_secret_key.is_some()
    }

    pub fn is_solana_configured(&self) -> bool {
        self.settlement.solana_rpc_url.is_some()
    }

    pub fn get_jwt_secret(&self) -> Option<&str> {
        self.trust.jwt_secret.as_deref()
    }

    pub fn get_llm_api_key(&self) -> Option<&str> {
        self.llm.api_key.as_deref()
    }
}

pub fn create_default_config_file<P: AsRef<Path>>(path: P) -> Result<()> {
    let default_config = AppConfig::default();
    let toml_str = toml::to_string_pretty(&default_config)
        .map_err(|e| crate::error::NegotiationError::Config(format!("Failed to serialize default config: {}", e)))?;

    std::fs::write(path, toml_str)
        .map_err(|e| crate::error::NegotiationError::Config(format!("Failed to write default config file: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.port, 8000);
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.llm.model, "gpt-3.5-turbo");
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());

        config.server.port = 0;
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_config_file_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        create_default_config_file(path).unwrap();
        assert!(path.exists());

        let loaded_config = AppConfig::load(path).unwrap();
        assert_eq!(loaded_config.server.port, 8000);
    }
}