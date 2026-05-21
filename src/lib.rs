//! interview-assist — 实时面试辅助系统
//!
//! 三个二进制：
//!   interview-assist — 固定时长捕获 (测试用)
//!   hotkey           — 按键控制 (主力)
//!   verify           — WAV 格式验证

pub mod api;
pub mod audio;
pub mod llm;
pub mod stt;
