use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn log_dir() -> PathBuf {
    let base = if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("interview-helper")
    } else {
        PathBuf::from(".interview-helper")
    };
    base.join("logs")
}

fn date_stamp() -> String {
    // Simple YYYY-MM-DD without external crate
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Approximate date from Unix epoch (2025-01-01 = 1735689600)
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let days_in_year = if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) { 366 } else { 365 };
        if d < days_in_year { break; }
        d -= days_in_year;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let mdays = [31, if leap {29} else {28}, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    while d >= mdays[m] { d -= mdays[m]; m += 1; }
    format!("{:04}-{:02}-{:02}", y, m + 1, d + 1)
}

fn time_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn log_path() -> PathBuf {
    log_dir().join(format!("{}.log", date_stamp()))
}

fn write(level: &str, module: &str, message: &str) {
    let dir = log_dir();
    let _ = fs::create_dir_all(&dir);
    let path = log_path();
    let line = format!("{} [{}] [{}] {}\n", time_stamp(), level, module, message);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn info(module: &str, message: &str) {
    write("INFO", module, message);
}

pub fn warn(module: &str, message: &str) {
    write("WARN", module, message);
}

pub fn error(module: &str, message: &str) {
    write("ERROR", module, message);
}

pub fn api_call(api: &str, request: &str, result: &Result<String, String>, duration_ms: u64) {
    let status = match result {
        Ok(_) => "OK",
        Err(_) => "FAIL",
    };
    let result_preview = match result {
        Ok(r) => {
            if r.len() > 200 { format!("{}... ({} chars)", &r[..200], r.len()) }
            else { r.clone() }
        }
        Err(e) => e.clone(),
    };
    write("API", api, &format!(
        "[{}] {}ms | Req: {} | Res: {}",
        status, duration_ms,
        if request.len() > 100 { format!("{}...", &request[..100]) } else { request.to_string() },
        result_preview
    ));
}

pub fn log_path_str() -> String {
    log_path().to_string_lossy().to_string()
}
