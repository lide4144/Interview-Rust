//! API 配置 — 从 config.yaml 读取

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_llm_provider")]
    pub llm_provider: String,
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    #[serde(default = "default_stt_model")]
    pub stt_model: String,

    pub deepseek: Provider,
    #[serde(default)]
    pub openai: OpenAiProvider,
    pub siliconflow: Provider,
    pub bailian: Provider,
    #[serde(default)]
    pub ark: Provider,
}

#[derive(Deserialize, Clone, Default)]
pub struct Provider {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub key: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct OpenAiProvider {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub model: String,
}

fn default_llm_provider() -> String { "deepseek".into() }
fn default_llm_model() -> String { "deepseek-v4-flash".into() }
fn default_stt_model() -> String { "FunAudioLLM/SenseVoiceSmall".into() }

impl Config {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("无法读取 config.yaml: {e}"))?;

        let config: Self = serde_yaml::from_str(&content)
            .map_err(|e| anyhow!("config.yaml 格式错误: {e}"))?;

        if config.llm_provider == "openai" && config.openai.key.is_empty() {
            return Err(anyhow!("config.yaml: openai.key 未配置"));
        }

        Ok(config)
    }

    /// 获取当前 LLM 的 URL 和 Key
    pub fn llm_url(&self) -> &str {
        match self.llm_provider.as_str() {
            "openai" => &self.openai.url,
            _ => &self.deepseek.url,
        }
    }

    pub fn llm_key(&self) -> &str {
        match self.llm_provider.as_str() {
            "openai" => &self.openai.key,
            _ => &self.deepseek.key,
        }
    }

    pub fn llm_model_id(&self) -> &str {
        match self.llm_provider.as_str() {
            "openai" if !self.openai.model.is_empty() => &self.openai.model,
            _ => &self.llm_model,
        }
    }

    // 兼容旧接口
    pub fn siliconflow_url(&self) -> &str { &self.siliconflow.url }
    pub fn siliconflow_key(&self) -> &str { &self.siliconflow.key }
    pub fn bailian_key(&self) -> &str { &self.bailian.key }
}
