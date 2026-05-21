//! SiliconFlow STT — 语音转文字（文件 + 内存 bytes）
//!
//! transcribe()        — 从 WAV 文件
//! transcribe_bytes()  — 从内存 PCM bytes（自动构建 WAV header）

use crate::api::Config;
use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct SttResponse {
    text: String,
}

/// 从 WAV 文件进行语音识别
pub fn transcribe(config: &Config, wav_path: &str) -> Result<String> {
    let wav_data = std::fs::read(wav_path)
        .map_err(|e| anyhow!("读取音频文件失败 ({wav_path}): {e}"))?;
    if wav_data.is_empty() {
        return Err(anyhow!("音频文件为空"));
    }
    transcribe_inner(config, &wav_data, wav_path)
}

/// 从内存 PCM 字节进行语音识别（自动包装 WAV header）
pub fn transcribe_bytes(
    config: &Config,
    pcm: &[u8],
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
) -> Result<String> {
    if pcm.is_empty() {
        return Err(anyhow!("音频数据为空"));
    }

    // 将 raw PCM 包装成 WAV（在内存中）
    let wav = build_wav_in_memory(pcm, sample_rate, channels, bits_per_sample)?;
    transcribe_inner(config, &wav, "stream.wav")
}

/// 公共发送逻辑
fn transcribe_inner(config: &Config, wav_data: &[u8], filename: &str) -> Result<String> {
    let url = format!("{}/audio/transcriptions", config.siliconflow_url);

    let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let mut body = Vec::new();

    // model 字段
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
    body.extend_from_slice(b"FunAudioLLM/SenseVoiceSmall\r\n");

    // file 字段
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n").as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(wav_data);
    body.extend_from_slice(b"\r\n");

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let content_type = format!("multipart/form-data; boundary={boundary}");

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", config.siliconflow_key))
        .set("Content-Type", &content_type)
        .send_bytes(&body)
        .map_err(|e| anyhow!("STT 请求失败: {e}"))?;

    if resp.status() != 200 {
        let status = resp.status();
        let err_body = resp.into_string().unwrap_or_default();
        return Err(anyhow!("STT 返回错误 ({status}): {err_body}"));
    }

    let result: SttResponse = resp
        .into_json()
        .map_err(|e| anyhow!("STT 响应解析失败: {e}"))?;

    if result.text.is_empty() {
        return Err(anyhow!("识别结果为空"));
    }

    Ok(result.text)
}

/// 在内存中构建 WAV 文件（16-bit PCM）
fn build_wav_in_memory(
    pcm: &[u8],
    sample_rate: u32,
    channels: u16,
    bits: u16,
) -> Result<Vec<u8>> {
    let mut wav = Vec::with_capacity(44 + pcm.len());

    let byte_rate = sample_rate * channels as u32 * bits as u32 / 8;
    let block_align = (channels as u32 * bits as u32 / 8) as u16;
    let data_size = pcm.len() as u32;

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36u32 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits.to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm);

    Ok(wav)
}
