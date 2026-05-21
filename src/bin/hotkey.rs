//! interview-assist — 按键控制模式 v2
//!
//! 按住 F2：直接 WASAPI 捕获
//! 松开 F2：保存 WAV → STT (mono 16kHz in-memory) → LLM
//! 每步带延迟计时

use interview_assist::{api, audio, llm, stt};
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════╗");
    println!("║   面试辅助 - 按键控制模式 v2          ║");
    println!("╚══════════════════════════════════════╝\n");

    let config = api::Config::from_file("api.txt")?;
    let history: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || { r.store(false, Ordering::SeqCst); })?;

    println!("═══ 按键控制 v2 ═══");
    println!("   按住 F2：录制 → 松开：识别 → 生成回答");
    println!("   按 Ctrl+C 退出\n");

    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F2};

    let mut recording = false;
    let mut answer_id = 1u32;
    let mut cap: Option<audio::LoopbackCapture> = None;

    println!("▶️  等待 F2...");

    while running.load(Ordering::SeqCst) {
        let f2 = unsafe { GetAsyncKeyState(VK_F2.0 as i32) } < 0;

        if f2 {
            if !recording {
                println!("\n🔴 录制中... (松开 F2 结束)");
                cap = Some(audio::LoopbackCapture::new_for_stream()?);
                recording = true;
            }
            if let Some(ref c) = cap {
                let chunk = c.read_direct(50)?;
                if !chunk.is_empty() {
                    c.append_to_buffer(&chunk);
                }
                let len = c.buffer_len();
                let secs = len as f64 / c.bytes_per_second() as f64;
                let db = c.buffer_vu_db();
                let bar_n = ((db + 60.0).clamp(0.0, 60.0) / 60.0 * 30.0) as usize;
                let bar = "█".repeat(bar_n.min(30));
                print!("\r  [{bar:<30}] {db:+.0}dB | {secs:.1}s  ");
                use std::io::{self, Write};
                let _ = io::stdout().flush();
            }
        } else if recording {
            recording = false;
            let c = cap.take().unwrap();
            let audio = c.into_buffer();

            if audio.is_empty() || audio.len() < c.bytes_per_second() / 2 {
                println!("\r  ⏭️  太短，忽略          \x1b[K");
                continue;
            }

            let t_total = Instant::now();
            let secs = audio.len() as f64 / c.bytes_per_second() as f64;
            println!("\r\x1b[K\n📦 {secs:.1}s → 识别中...");

            // 1. 保存 WAV（BufWriter 加速，调试用）
            let wav_path = format!("q_{answer_id}.wav");
            let t_save = {
                let t = Instant::now();
                c.save_wav(&audio, &wav_path)?;
                t.elapsed()
            };

            // 2. STT：mono 16kHz in-memory（跳过磁盘 IO，上传量缩小 6 倍）
            let (text, t_stt) = {
                let t = Instant::now();
                let mono = c.to_mono_16k(&audio);
                let text = stt::transcribe_bytes(&config, &mono, 16000, 1, 16)?;
                (text, t.elapsed())
            };
            println!("🎤 [{answer_id}] \"{text}\"");

            // 3. LLM
            let (ans, t_llm) = {
                let t = Instant::now();
                let ans = llm::ask_with_history(&config, &text, &history)?;
                (ans, t.elapsed())
            };

            let t_all = t_total.elapsed();
            println!("\n💡 #{answer_id}:\n\n{ans}\n");
            println!("⏱️  保存:{t_save:.1?} | STT:{t_stt:.1?} | LLM:{t_llm:.1?} | 总计:{t_all:.1?}");
            println!("{}", "─".repeat(60));

            {
                let mut h = history.lock().unwrap();
                h.push((text, ans));
                if h.len() > 5 { h.remove(0); }
            }

            answer_id += 1;
            println!("\n▶️  等待 F2...");
        } else {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    println!("\n👋 已停止");
    Ok(())
}
