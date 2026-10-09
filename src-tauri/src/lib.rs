pub mod api;
pub mod config;
pub mod logger;

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
async fn ocr_screenshot(image_base64: String) -> Result<String, String> {
    let t0 = std::time::Instant::now();
    logger::info("cmd", "ocr_screenshot called");
    let cfg = config::load();
    let key = cfg.siliconflow_key.ok_or_else(|| {
        logger::error("cmd", "ocr_screenshot: missing SiliconFlow key");
        "未配置 SiliconFlow API Key".to_string()
    })?;
    let req_preview = if image_base64.len() > 50 { format!("base64:{}chars", image_base64.len()) } else { image_base64.clone() };
    let result = api::ocr_image(&image_base64, &key).await;
    logger::api_call("SiliconFlow", &req_preview, &result, t0.elapsed().as_millis() as u64);
    result
}

#[tauri::command]
async fn solve_code(code: String) -> Result<String, String> {
    let t0 = std::time::Instant::now();
    logger::info("cmd", "solve_code called");
    let cfg = config::load();
    let key = cfg.deepseek_key.ok_or_else(|| {
        logger::error("cmd", "solve_code: missing DeepSeek key");
        "未配置 DeepSeek API Key".to_string()
    })?;
    let req_preview = if code.len() > 200 { format!("{}... ({} chars)", &code[..200], code.len()) } else { code.clone() };
    let result = api::solve_problem(&code, &key).await;
    logger::api_call("DeepSeek", &req_preview, &result, t0.elapsed().as_millis() as u64);
    result
}

#[tauri::command]
async fn analyze_screenshot(image_base64: String) -> Result<AnalyzeResult, String> {
    let t0 = std::time::Instant::now();
    logger::info("cmd", "analyze_screenshot called");
    let cfg = config::load();
    let silicon_key = cfg.siliconflow_key.ok_or_else(|| {
        logger::error("cmd", "analyze_screenshot: missing SiliconFlow key");
        "未配置 SiliconFlow API Key".to_string()
    })?;
    let deepseek_key = cfg.deepseek_key.ok_or_else(|| {
        logger::error("cmd", "analyze_screenshot: missing DeepSeek key");
        "未配置 DeepSeek API Key".to_string()
    })?;
    let req_preview = format!("base64:{}chars", image_base64.len());
    let result = api::analyze_screenshot(&image_base64, &silicon_key, &deepseek_key).await;
    match &result {
        Ok((code, _)) => logger::api_call("analyze", &req_preview, &Ok(code.clone()), t0.elapsed().as_millis() as u64),
        Err(e) => logger::api_call("analyze", &req_preview, &Err(e.clone()), t0.elapsed().as_millis() as u64),
    }
    let (code, solution) = result?;
    Ok(AnalyzeResult { code, solution })
}

#[tauri::command]
fn get_config() -> ConfigStatus {
    let cfg = config::load();
    logger::info("cmd", "get_config");
    ConfigStatus {
        has_siliconflow: cfg.siliconflow_key.is_some(),
        has_deepseek: cfg.deepseek_key.is_some(),
    }
}

#[tauri::command]
fn save_config(siliconflow_key: Option<String>, deepseek_key: Option<String>) -> Result<(), String> {
    logger::info("cmd", &format!("save_config: sf={}, ds={}",
        if siliconflow_key.is_some() { "set" } else { "unchanged" },
        if deepseek_key.is_some() { "set" } else { "unchanged" }));
    let mut cfg = config::load();
    if let Some(k) = siliconflow_key { cfg.siliconflow_key = Some(k); }
    if let Some(k) = deepseek_key { cfg.deepseek_key = Some(k); }
    config::save(&cfg)
}

#[tauri::command]
fn get_log_path() -> String {
    logger::log_path_str()
}

#[derive(serde::Serialize)]
pub struct AnalyzeResult {
    pub code: String,
    pub solution: String,
}

#[derive(serde::Serialize)]
pub struct ConfigStatus {
    pub has_siliconflow: bool,
    pub has_deepseek: bool,
}

fn toggle_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let show = MenuItemBuilder::with_id("show", "显示/隐藏窗口").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show)
                .separator()
                .item(&quit)
                .build()?;

            TrayIconBuilder::new()
                .tooltip("Interview Helper")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => toggle_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let shortcut = Shortcut::new(
                    Some(Modifiers::CONTROL | Modifiers::SHIFT),
                    Code::KeyH,
                );
                app.global_shortcut().register(shortcut)?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            ocr_screenshot,
            solve_code,
            analyze_screenshot,
            get_config,
            save_config,
            get_log_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
