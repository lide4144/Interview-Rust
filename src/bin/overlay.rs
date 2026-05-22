//! 透明悬浮窗 — 面试答案显示器
//! 使用：cargo run --release --bin overlay

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM, HINSTANCE};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, EndPaint, FillRect, InvalidateRect, SetBkMode,
    SetTextColor, TextOutW, DeleteObject, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect,
    GetSystemMetrics, PeekMessageW, PostQuitMessage, RegisterClassW,
    SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowPos, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
    HWND_TOPMOST, LWA_ALPHA, MSG, PM_REMOVE, SM_CXSCREEN, SM_CYSCREEN,
    SWP_NOZORDER, SW_SHOW, WDA_EXCLUDEFROMCAPTURE,
    WM_DESTROY, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOPMOST, WS_POPUP,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F8};
use windows::core::PCWSTR;

fn color(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let instance = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?
    };
    let hi = HINSTANCE(instance.0);

    let cn = wide("InterviewOverlay");
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
            WS_EX_LAYERED | WS_EX_TOPMOST,
            PCWSTR::from_raw(cn.as_ptr()),
            PCWSTR::from_raw(wide("面试辅助").as_ptr()),
            WS_POPUP,
            CW_USEDEFAULT, CW_USEDEFAULT, 700, 300,
            None, None, Some(hi), None,
        )?
    };

    unsafe {
        SetLayeredWindowAttributes(hwnd, color(0, 0, 0), 200, LWA_ALPHA)?;
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)?;
    }

    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    unsafe { SetWindowPos(hwnd, Some(HWND_TOPMOST), sw - 720, sh - 360, 700, 300, SWP_NOZORDER)?; }
    unsafe { let _ = ShowWindow(hwnd, SW_SHOW); }

    println!("🪟 悬浮窗右下角 | 屏幕共享隐身 | F8 退出");

    let mut last = String::new();
    let mut msg = MSG::default();

    loop {
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_DESTROY { return Ok(()); }
            unsafe { let _ = TranslateMessage(&msg); let _ = DispatchMessageW(&msg); }
        }

        if unsafe { GetAsyncKeyState(VK_F8.0 as i32) } < 0 { break; }

        if let Ok(mut f) = std::fs::File::open("answer.txt") {
            let mut s = String::new();
            if std::io::Read::read_to_string(&mut f, &mut s).is_ok() && s != last {
                last = s;
                unsafe { let _ = InvalidateRect(Some(hwnd), None, true); }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    Ok(())
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => { unsafe { PostQuitMessage(0) }; LRESULT(0) }
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);

                let bg = CreateSolidBrush(color(15, 15, 35));
                FillRect(hdc, &r, bg);
                let _ = DeleteObject(bg.into());

                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, color(180, 210, 255));

                let text = std::fs::read_to_string("answer.txt").unwrap_or_default();
                if text.is_empty() {
                    let w = wide("⌛ 等待回答...");
                    let _ = TextOutW(hdc, 15, 15, &w);
                } else {
                    for (i, line) in text.lines().enumerate() {
                        let w = wide(line);
                        let _ = TextOutW(hdc, 15, 12 + (i as i32) * 22, &w);
                    }
                }
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}
