use base64::Engine;
use interview_helper_lib::{api, config};
use std::path::PathBuf;

fn test_image_path() -> PathBuf {
    let temp = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Temp".to_string());
    PathBuf::from(temp).join("test_code.png")
}

#[tokio::test]
async fn test_end_to_end() {
    // 1. Load config
    let cfg = config::load();
    let sf_key = cfg.siliconflow_key.expect("SiliconFlow key missing in config");
    let ds_key = cfg.deepseek_key.expect("DeepSeek key missing in config");
    println!("[OK] Config loaded");

    // 2. Read test image
    let path = test_image_path();
    assert!(path.exists(), "Test image not found at {:?}", path);
    let img = std::fs::read(&path).expect("Failed to read test image");
    let b64 = base64::engine::general_purpose::STANDARD.encode(&img);
    println!("[OK] Image loaded: {} bytes, base64: {}", img.len(), b64.len());

    // 3. OCR
    println!("[OCR] Sending to SiliconFlow...");
    let code = api::ocr_image(&b64, &sf_key).await.expect("OCR failed");
    println!("[OCR Result]\n{}\n", code);
    assert!(!code.is_empty(), "OCR returned empty");
    assert!(code.contains("two_sum") || code.contains("hashmap"), "OCR should contain expected code");

    // 4. Solve
    println!("[Solve] Sending to DeepSeek...");
    let solution = api::solve_problem(&code, &ds_key).await.expect("Solve failed");
    println!("[Solution]\n{}\n", solution);
    assert!(!solution.is_empty(), "Solution returned empty");

    println!("[PASS] Full pipeline verified!");
}
