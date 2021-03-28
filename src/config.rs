use std::{fs::File, io::Read, path::Path};

use serde_derive::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub threema: ThreemaConfig,
    pub server: ServerConfig,
    pub logging: Option<LoggingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThreemaConfig {
    /// The Threema Gateway ID (starts with a `*`)
    pub gateway_id: String,
    /// The Threema Gateway secret (can be found on gateway.threema.ch)
    pub gateway_secret: String,
    /// The hex-encoded private key
    pub private_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// The HTTP server listening host:port string
    pub listen: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    /// The log filter (tracing syntax). Default: `info,sqlx::query=warn`. For development, you
    /// could set it to `debug,sqlx::query=warn`.
    pub filter: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| e.to_string())?;
        toml::from_str(&contents).map_err(|e| e.to_string())
    }
}
