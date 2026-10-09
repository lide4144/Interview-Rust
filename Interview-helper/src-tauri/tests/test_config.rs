use interview_helper_lib::config;

#[test]
fn test_load_config() {
    let cfg = config::load();
    println!("Config loaded");
    println!("  SiliconFlow: {:?}", cfg.siliconflow_key.is_some());
    println!("  DeepSeek: {:?}", cfg.deepseek_key.is_some());
    assert!(cfg.siliconflow_key.is_some(), "SiliconFlow key missing");
    assert!(cfg.deepseek_key.is_some(), "DeepSeek key missing");
    println!("PASS: Config verified");
}
