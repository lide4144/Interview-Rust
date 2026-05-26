//! interview-assist — 固定时长捕获模式
//!
//! Pipeline: 捕获 → STT → LLM
//!
//! 使用：cargo run --release

use interview_assist::{api, audio, llm, stt};
use anyhow::Result;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════╗");
    println!("║   面试辅助 - 固定时长捕获             ║");
    println!("╚══════════════════════════════════════╝\n");

    let config = api::Config::from_file("config.yaml")?;
    println!("🔑 DeepSeek:  {}", config.deepseek.url);
    println!("🔑 SiliconFlow: {}\n", config.siliconflow_url());

    // 捕获音频
    println!("═══ 第一步：音频捕获 ═══");
    let capture = audio::LoopbackCapture::new()?;
    println!("\n⏱️  现在请播放面试提问音频...\n");
    println!("📌 捕获 15 秒\n");

    let data = capture.capture(15)?;
    if data.is_empty() {
        eprintln!("❌ 未捕获到音频");
        return Ok(());
    }
    capture.save_wav(&data, "captured.wav")?;

    // STT
    println!("\n═══ 第二步：语音识别 ═══");
    let transcript = stt::transcribe(&config, "captured.wav")?;
    println!("📝 识别: \"{}\"", transcript);

    // LLM
    println!("\n═══ 第三步：生成回答 ═══");
    let answer = llm::ask(&config, &transcript)?;
    println!("\n💡 候选回答:\n");
    println!("{}", answer);
    println!("\n{}\n✅ 完成", "=".repeat(50));

    Ok(())
}
