//! LLM 延迟基准测试 v5 — 筛选快速模型 + 实时显示回答

use anyhow::Result;
use std::io::{self, Write};
use std::time::Instant;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════╗");
    println!("║   LLM 延迟基准测试 v5                ║");
    println!("╚══════════════════════════════════════╝\n");

    let providers = [
        ("DeepSeek",    "https://api.deepseek.com/v1",           "sk-681ff60480c74638ac0bba02c9420a73"),
        ("SiliconFlow", "https://api.siliconflow.cn/v1",         "sk-uoijizemjbmqxcaycjuumqzcjuhstjawjtwmoagwoiszznnv"),
        ("Bailian",     "https://dashscope.aliyuncs.com/compatible-mode/v1", "sk-f3fd1d1f7af34f7db9cc57fc7f58f260"),
        ("Ark",         "https://ark.cn-beijing.volces.com/api/v3", "6fd36b9c-4ad3-4b61-91e0-7df23b14476d"),
    ];

    // Step 1: 拉模型列表
    let mut all: Vec<(String, String, String, String)> = Vec::new();
    for (name, url, key) in &providers {
        print!("📋 {name}: ");
        let _ = io::stdout().flush();
        match ureq::get(&format!("{url}/models"))
            .set("Authorization", &format!("Bearer {key}"))
            .call()
        {
            Ok(resp) => {
                let json: serde_json::Value = resp.into_json()?;
                let models = json["data"].as_array().cloned().unwrap_or_default();
                let mut added = 0;
                for m in &models {
                    let id = m["id"].as_str().unwrap_or("").to_string();
                    if is_chat_model(&id) {
                        let short = id.rsplit('/').next().unwrap_or(&id);
                        all.push((format!("{name}-{short}"), url.to_string(), key.to_string(), id));
                        added += 1;
                    }
                }
                println!("{}/{} 文本", added, models.len());
            }
            Err(e) => println!("失败: {e}"),
        }
    }

    // Step 2: 筛选快速模型
    let fast_kw = ["flash","turbo","lite","mini","small","tiny","nano",
        "1.5b","3b","4b","7b","8b","14b","30b","32b","0.5b","1.8b",
        "qwen3","seed","doubao-lite","doubao-1.5","kimi"];
    let candidates: Vec<_> = all.iter()
        .filter(|(label, _, _, id)| {
            let s = format!("{label} {id}").to_lowercase();
            fast_kw.iter().any(|kw| s.contains(kw))
        })
        .cloned()
        .collect();

    println!("\n🧪 筛选出 {} 个快速模型\n", candidates.len());

    let q = "用一句话回答：C++11最重要的三个新特性是什么？";

    let mut results: Vec<(String, f64, u32, String)> = Vec::new();
    for (i, (label, url, key, model_id)) in candidates.iter().enumerate() {
        print!("[{}/{}] {:<40} ", i + 1, candidates.len(), label);
        let _ = io::stdout().flush();

        let body = serde_json::json!({
            "model": model_id,
            "messages": [{"role": "user", "content": q}],
            "stream": false,
            "temperature": 0.7,
            "max_tokens": 60
        });

        let t0 = Instant::now();
        match ureq::post(&format!("{url}/chat/completions"))
            .set("Authorization", &format!("Bearer {key}"))
            .set("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .send_string(&body.to_string())
        {
            Ok(resp) => {
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                let status = resp.status();
                if status == 200 {
                    if let Ok(j) = resp.into_json::<serde_json::Value>() {
                        let content = j["choices"][0]["message"]["content"]
                            .as_str().unwrap_or("").to_string();
                        let tk = j["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
                        // 一行显示：延迟 + 回答摘要
                        let preview: String = content.chars().take(50).collect();
                        println!("{ms:.0}ms → \"{preview}...\"");
                        results.push((label.clone(), ms, tk, content));
                        continue;
                    }
                } else {
                    println!("❌ HTTP{status}");
                }
            }
            Err(e) => println!("❌ {e:.0}"),
        }
    }

    // Step 3: 排名
    results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    println!("\n═══════════════════════════════════════════");
    println!("  排名（按延迟）");
    println!("═══════════════════════════════════════════");
    for (i, (name, ms, tk, _)) in results.iter().enumerate() {
        let flag = if *ms < 1000.0 { "⚡" } else if *ms < 2000.0 { "✅" } else { "🐢" };
        println!("  {:2}. {flag} {name:<40} {ms:6.0}ms {tk}tk", i + 1);
    }

    // 输出最快模型的完整回答
    if let Some((name, ms, _, content)) = results.first() {
        println!("\n═══════════════════════════════════════════");
        println!("  冠军 {name} ({ms:.0}ms) 完整回答：");
        println!("═══════════════════════════════════════════");
        println!("{content}");
    }

    Ok(())
}

fn is_chat_model(id: &str) -> bool {
    let low = id.to_lowercase();
    if low.contains("embed") || low.contains("rerank") || low.contains("audio")
        || low.contains("whisper") || low.contains("sensevoice") || low.contains("tts")
        || low.contains("image") || low.contains("video") || low.contains("speech")
        || low.contains("stt") || low.contains("asr") { return false; }
    low.contains("chat") || low.contains("instruct") || low.contains("deepseek")
        || low.contains("qwen") || low.contains("glm") || low.contains("llama")
        || low.contains("yi-") || low.contains("mistral") || low.contains("r1")
        || low.contains("flash") || low.contains("turbo") || low.contains("plus")
        || low.contains("doubao") || low.contains("kimi") || low.contains("seed")
}
