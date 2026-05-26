//! DeepSeek LLM — 面试回答生成
//!
//! 使用 DeepSeek Chat API，流式输出面试候选答案

use crate::api::Config;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// 面试助手 System Prompt
const SYSTEM_PROMPT: &str = "\
你是一个专业的面试辅助助手。你的任务是根据面试官的问题，快速给出简洁、有深度的候选回答。

## 规则
1. 回答要口语化、自然，像是候选人当场说出来的
2. 分点列出要点（但不要太长，2-4 点即可）
3. 如果涉及技术问题，给出关键的术语和思路，不需要长篇教程
4. 如果是行为面试题（如\"讲一个你解决过的难题\"），用 STAR 框架给出要点
5. 如果是反问环节（\"你有什么想问的吗\"），给出 2-3 个高质量问题
6. 总长度控制在 200 字以内

## 面试上下文
当前面试是实时进行的在线面试，你听到的问题是语音识别转录的，
可能有小错误，请根据语义理解修正。";

/// 将面试官问题发送给 DeepSeek，获取候选回答
pub fn ask(config: &Config, question: &str) -> Result<String> {
    let url = format!("{}/chat/completions", config.llm_url());

    let request = ChatRequest {
        model: config.llm_model_id().to_string(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: SYSTEM_PROMPT.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!("面试官问：{question}"),
            },
        ],
        stream: false,
        temperature: 0.7,
        max_tokens: 400,
    };

    let body = serde_json::to_string(&request)?;

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", config.llm_key()))
        .set("Content-Type", "application/json")
        .send_string(&body)
        .map_err(|e| anyhow!("LLM 请求失败: {e}"))?;

    if resp.status() != 200 {
        let status = resp.status();
        let err_body = resp.into_string().unwrap_or_default();
        return Err(anyhow!("LLM 返回错误 ({status}): {err_body}"));
    }

    let result: ChatResponse = resp.into_json()
        .map_err(|e| anyhow!("LLM 响应解析失败: {e}"))?;

    let answer = result
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("LLM 返回空响应"))?;

    Ok(answer)
}

/// 带对话历史的多轮面试回答
pub fn ask_with_history(
    config: &Config,
    question: &str,
    history: &std::sync::Mutex<Vec<(String, String)>>,
) -> Result<String> {
    let url = format!("{}/chat/completions", config.llm_url());

    let mut messages = vec![Message {
        role: "system".to_string(),
        content: SYSTEM_PROMPT.to_string(),
    }];

    // 注入对话历史（最近 5 轮）
    {
        let h = history.lock().unwrap();
        let start = if h.len() > 5 { h.len() - 5 } else { 0 };
        for (q, a) in &h[start..] {
            messages.push(Message {
                role: "user".to_string(),
                content: format!("面试官问：{q}"),
            });
            messages.push(Message {
                role: "assistant".to_string(),
                content: a.clone(),
            });
        }
    }

    // 当前问题
    messages.push(Message {
        role: "user".to_string(),
        content: format!("面试官问：{question}"),
    });

    let request = ChatRequest {
        model: config.llm_model_id().to_string(),
        messages,
        stream: false,
        temperature: 0.7,
        max_tokens: 400,
    };

    let body = serde_json::to_string(&request)?;

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", config.llm_key()))
        .set("Content-Type", "application/json")
        .send_string(&body)
        .map_err(|e| anyhow!("LLM 请求失败: {e}"))?;

    if resp.status() != 200 {
        let status = resp.status();
        let err_body = resp.into_string().unwrap_or_default();
        return Err(anyhow!("LLM 返回错误 ({status}): {err_body}"));
    }

    let result: ChatResponse = resp.into_json()
        .map_err(|e| anyhow!("LLM 响应解析失败: {e}"))?;

    let answer = result
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("LLM 返回空响应"))?;

    Ok(answer)
}
