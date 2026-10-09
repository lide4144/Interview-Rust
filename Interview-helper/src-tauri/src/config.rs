use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct AppConfig {
    pub siliconflow_key: Option<String>,
    pub deepseek_key: Option<String>,
}

fn config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("interview-helper")
    } else if let Some(base) = dirs::config_dir() {
        base.join("interview-helper")
    } else {
        PathBuf::from(".interview-helper")
    }
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load() -> AppConfig {
    let path = config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir failed: {}", e))?;
    let content = toml::to_string_pretty(config).map_err(|e| format!("serialize failed: {}", e))?;
    std::fs::write(config_path(), content).map_err(|e| format!("write failed: {}", e))?;
    Ok(())
}
