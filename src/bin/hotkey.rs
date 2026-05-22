//! interview-assist v3 — 内置悬浮窗
//! 按住 F2 录制 → 松开识别 → 悬浮窗显示答案（屏幕共享隐身）

use interview_assist::{api, audio, llm, stt};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use std::thread;

// ── 共享状态 ──
static OVERLAY_TEXT: Mutex<String> = Mutex::new(String::new());
static OVERLAY_HWND: Mutex<isize> = Mutex::new(0);

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════╗");
    println!("║   面试辅助 v3 — 内置悬浮窗           ║");
    println!("╚══════════════════════════════════════╝\n");

    let config = api::Config::from_file("api.txt")?;
    let history: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let running = Arc::new(AtomicBool::new(true));

    // Ctrl+C
    let r = running.clone();
    ctrlc::set_handler(move || { r.store(false, Ordering::SeqCst); })?;

    // ── 后台线程：音频捕获 + API ──
    let running_bg = running.clone();
    let cfg = config.clone();
    let hist = history.clone();
    thread::spawn(move || {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F2};
        let mut recording = false;
        let mut cap: Option<audio::LoopbackCapture> = None;
        let mut answer_id = 1u32;

        while running_bg.load(Ordering::SeqCst) {
            let f2 = unsafe { GetAsyncKeyState(VK_F2.0 as i32) } < 0;

            if f2 {
                if !recording {
                    println!("\n🔴 录制中...");
                    if let Ok(c) = audio::LoopbackCapture::new_for_stream() {
                        cap = Some(c);
                        recording = true;
                    }
                }
                if let Some(ref c) = cap {
                    if let Ok(chunk) = c.read_direct(50) {
                        if !chunk.is_empty() { c.append_to_buffer(&chunk); }
                        let s = c.buffer_len() as f64 / c.bytes_per_second() as f64;
                        let db = c.buffer_vu_db();
                        let bar = "█".repeat(((db + 60.0).clamp(0.0, 60.0) / 60.0 * 30.0) as usize);
                        print!("\r  [{bar:<30}] {db:+.0}dB | {s:.1}s  ");
                        use std::io::{self, Write};
                        let _ = io::stdout().flush();
                    }
                }
            } else if recording {
                recording = false;
                let c = cap.take().unwrap();
                let audio = c.into_buffer();
                if audio.len() < c.bytes_per_second() / 2 {
                    println!("\r  ⏭️  太短    \x1b[K");
                    continue;
                }

                let t0 = Instant::now();
                let secs = audio.len() as f64 / c.bytes_per_second() as f64;
                println!("\r\x1b[K\n📦 {secs:.1}s → 识别...");

                let _ = c.save_wav(&audio, &format!("q_{answer_id}.wav"));
                let text = {
                    let mono = c.to_mono_16k(&audio);
                    stt::transcribe_bytes(&cfg, &mono, 16000, 1, 16).unwrap_or_default()
                };
                println!("🎤 [{answer_id}] \"{text}\"");

                let ans = llm::ask_with_history(&cfg, &text, &hist).unwrap_or_default();
                let t_all = t0.elapsed();
                println!("\n💡 #{answer_id}:\n\n{ans}\n");
                println!("⏱️  总计:{t_all:.1?}");
                println!("{}", "─".repeat(60));

                // 更新悬浮窗文本
                *OVERLAY_TEXT.lock().unwrap() = format!(
                    "🎤 {text}\n{}\n⏱️ {:.1?}", ans, t_all
                );

                // 触发重绘
                let h = *OVERLAY_HWND.lock().unwrap();
                if h != 0 {
                    unsafe {
                        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(
                            Some(windows::Win32::Foundation::HWND(h as *mut _)), None, true
                        );
                    }
                }

                {
                    let mut h = hist.lock().unwrap();
                    h.push((text, ans));
                    if h.len() > 5 { h.remove(0); }
                }
                answer_id += 1;
                println!("\n▶️  等待 F2...");
            } else {
                thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    });

    // ── 主线程：悬浮窗 ──
    run_overlay(running.clone());
    Ok(())
}

// ════════════════════════════════════════════
// 悬浮窗（运行在主线程）
// ════════════════════════════════════════════

fn run_overlay(running: Arc<AtomicBool>) {
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM, HINSTANCE};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode,
        SetTextColor, TextOutW, PAINTSTRUCT, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
        GetSystemMetrics, PostQuitMessage, RegisterClassW,
        SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowPos, ShowWindow,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
        HWND_TOPMOST, LWA_ALPHA, SM_CXSCREEN, SM_CYSCREEN, SWP_NOZORDER,
        SW_SHOW, WDA_EXCLUDEFROMCAPTURE, WM_DESTROY, WM_PAINT,
        WNDCLASSW, WS_EX_LAYERED, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        WS_THICKFRAME,
    };
    use windows::core::PCWSTR;

    fn c(r: u8, g: u8, b: u8) -> COLORREF { COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16) }
    fn w(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }

    let instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap() };
    let hi = HINSTANCE(instance.0);

    let cn = w("InterviewOv3");
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: hi,
        lpszClassName: PCWSTR::from_raw(cn.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT,  // TRANSPARENT = 点击穿透
            PCWSTR::from_raw(cn.as_ptr()),
            PCWSTR::from_raw(w("🎯").as_ptr()),
            WS_POPUP | WS_THICKFRAME,  // 可缩放边框
            CW_USEDEFAULT, CW_USEDEFAULT, 700, 350,
            None, None, Some(hi), None,
        ).unwrap()
    };

    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, c(0, 0, 0), 200, LWA_ALPHA);
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
    }

    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    unsafe { let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), sw - 720, sh - 360, 700, 300, SWP_NOZORDER); }
    unsafe { let _ = ShowWindow(hwnd, SW_SHOW); }

    // 存储 HWND 供后台线程触发重绘
    *OVERLAY_HWND.lock().unwrap() = hwnd.0 as isize;

    println!("🪟 悬浮窗 → 右下角 | 屏幕共享隐身 ✅ | F9 切换拖动模式");

    // F9 切换点击穿透
    let mut click_through = true;
    let hwnd_raw = hwnd.0 as isize;
    thread::spawn(move || {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F9};
        let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut _);
        let mut last_f9 = false;
        loop {
            let f9 = unsafe { GetAsyncKeyState(VK_F9.0 as i32) } < 0;
            if f9 && !last_f9 {
                click_through = !click_through;
                let ex = if click_through {
                    WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT
                } else {
                    WS_EX_LAYERED | WS_EX_TOPMOST
                };
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowLongW(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE,
                        ex.0 as i32,
                    );
                    let _ = SetWindowPos(
                        hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0,
                        SWP_NOZORDER | windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE
                            | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE
                            | windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED,
                    );
                }
                println!("\n🪟 点击穿透: {}", if click_through { "ON" } else { "OFF (可拖动)" });
            }
            last_f9 = f9;
            thread::sleep(std::time::Duration::from_millis(200));
        }
    });

    let mut msg: windows::Win32::UI::WindowsAndMessaging::MSG = unsafe { std::mem::zeroed() };
    while running.load(Ordering::SeqCst) {
        unsafe { GetMessageW(&mut msg, None, 0, 0) };
        unsafe { let _ = TranslateMessage(&msg); DispatchMessageW(&msg); }
    }

    extern "system" fn wndproc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match m {
            WM_DESTROY => { unsafe { PostQuitMessage(0) }; LRESULT(0) }
            WM_PAINT => {
                unsafe {
                    let mut ps = PAINTSTRUCT::default();
                    let hdc = BeginPaint(h, &mut ps);
                    let mut r = RECT::default();
                    let _ = GetClientRect(h, &mut r);
                    let bg = CreateSolidBrush(c(15, 15, 35));
                    FillRect(hdc, &r, bg);
                    let _ = DeleteObject(bg.into());
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, c(180, 210, 255));
                    let text = OVERLAY_TEXT.lock().unwrap().clone();
                    if text.is_empty() {
                        let _ = TextOutW(hdc, 15, 15, &w("按住 F2 开始录制"));
                    } else {
                        for (i, line) in text.lines().take(20).enumerate() {
                            let _ = TextOutW(hdc, 15, 12 + (i as i32) * 22, &w(line));
                        }
                    }
                    let _ = EndPaint(h, &ps);
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(h, m, wp, lp) },
        }
    }
}
