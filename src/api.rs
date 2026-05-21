//! API 配置 — 从 api.txt 读取各平台凭据

use anyhow::{anyhow, Result};
use std::path::Path;

#[derive(Clone)]
pub struct Config {
    pub deepseek_url: String,
    pub deepseek_key: String,
    pub siliconflow_url: String,
    pub siliconflow_key: String,
    pub bailian_key: String,
}

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("无法读取 api.txt: {e}"))?;

        let lines: Vec<&str> = content.lines().collect();

        // 前4行：deepseek url, deepseek key, 空行, siliconflow url
        let deepseek_url = lines.get(0).map(|s| s.trim()).unwrap_or("");
        let deepseek_key = lines.get(1).map(|s| s.trim()).unwrap_or("");
        let siliconflow_url = lines.get(3).map(|s| s.trim()).unwrap_or("");
        let siliconflow_key = lines.get(4).map(|s| s.trim()).unwrap_or("");

        // 找 阿里云api-key: 开头的行
        let bailian_key = lines.iter()
            .find(|l| l.starts_with("阿里云api-key:"))
            .and_then(|l| l.split(':').nth(1))
            .map(|s| s.trim())
            .unwrap_or("");

        if deepseek_key.is_empty() || deepseek_url.is_empty() {
            return Err(anyhow!("api.txt 中未找到 DeepSeek 配置"));
        }

        Ok(Self {
            deepseek_url: deepseek_url.to_string(),
            deepseek_key: deepseek_key.to_string(),
            siliconflow_url: siliconflow_url.to_string(),
            siliconflow_key: siliconflow_key.to_string(),
            bailian_key: bailian_key.to_string(),
        })
    }
}
