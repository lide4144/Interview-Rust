//! interview-assist — 实时面试辅助系统
//!
//! 第一步：系统音频 Loopback 捕获 (WASAPI)
//!
//! 使用说明：
//! 1. 打开任何音频内容（YouTube/音乐/面试软件）
//! 2. 运行本程序
//! 3. 程序自动捕获系统输出音频
//! 4. 保存为 captured.wav

mod audio;

use anyhow::Result;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════╗");
    println!("║   面试辅助 - 系统音频捕获测试         ║");
    println!("╚══════════════════════════════════════╝\n");

    // 列出所有音频输出设备
    audio::list_devices()?;
    println!();

    // 创建 WASAPI Loopback 捕获器
    let capture = audio::LoopbackCapture::new()?;

    // 捕获 10 秒系统音频
    let data = capture.capture(10)?;

    // 保存为 WAV 文件
    capture.save_wav(&data, "captured.wav")?;

    if data.is_empty() {
        println!();
        println!("⚠️  未捕获到音频 — 请确保系统正在播放音频！");
        println!("   提示：打开 YouTube/B站 播放视频后重试");
    } else {
        println!();
        println!("✅ 成功！captured.wav 可直接播放");
    }

    Ok(())
}
