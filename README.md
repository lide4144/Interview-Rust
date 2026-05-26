# 🎯 Interview-Assist — 实时面试辅助系统

Rust 实现的实时面试辅助工具：WASAPI Loopback 捕获系统音频 → STT 语音转文字 → LLM 生成候选回答 → 悬浮窗显示。

> **屏幕共享时悬浮窗自动隐身** — Zoom / 腾讯会议 / 飞书看不到答案窗口。

## 快速开始

```bash
# 1. 配置
cp config.yaml.example config.yaml
# 编辑 config.yaml，至少填入 STT 和 LLM 的 API Key

# 2. 运行
cargo run --release
```

### 操作方式

| 操作 | 按键 |
|------|------|
| 录制面试官说话 | 按住 **F2** |
| 自动识别 + 生成回答 | 松开 F2 |
| 拖动/缩放悬浮窗 | 标准窗口操作 |
| 退出 | **Ctrl+C** 或点击窗口 ✕ |

## API 配置

`config.yaml` 支持多提供商，按需开启：

```yaml
# LLM — 必选，支持 OpenAI 兼容接口
llm_provider: deepseek        # deepseek | openai
llm_model: deepseek-v4-flash

deepseek:                     # DeepSeek 官方
  url: https://api.deepseek.com/v1
  key: sk-xxx

openai:                       # 任意 OpenAI 兼容接口
  url: https://api.openai.com/v1    # Groq / Together / vLLM 等
  key: sk-xxx
  model: gpt-4o-mini

# STT — 必选
siliconflow:                  # 硅基流动 (SiliconFlow)
  url: https://api.siliconflow.cn/v1
  key: sk-xxx

stt_model: FunAudioLLM/SenseVoiceSmall   # 或换其他 STT 模型
```

### SiliconFlow（硅基流动）

STT 语音识别使用 [硅基流动](https://siliconflow.cn) 的 SenseVoiceSmall 模型：
- **国内访问低延迟**，无需翻墙
- 注册即送免费额度：https://cloud.siliconflow.cn
- 支持中英混合识别
- API 兼容 OpenAI 格式，也可在 `bench` 工具中测试其托管的各种 LLM

### LLM 提供商切换

| 提供商 | 配置 | 延迟参考 |
|--------|------|----------|
| DeepSeek 官方 | `llm_provider: deepseek` | ~250ms |
| OpenAI 兼容 | `llm_provider: openai` | 取决于接口 |

用 `cargo run --release --bin bench` 可自动拉取所有可用模型并测试延迟。

## 架构

```
┌──────────┐    ┌──────────────┐    ┌─────────────┐    ┌────────────┐
│ WASAPI   │───▶│ SiliconFlow  │───▶│ DeepSeek v4  │───▶│  悬浮窗     │
│ Loopback │    │ STT (REST)   │    │ Flash (LLM) │    │ (Win32 GUI) │
└──────────┘    └──────────────┘    └─────────────┘    └─────────────┘
 捕获系统音频      语音→文字          生成候选回答       屏幕共享隐身
 (48kHz stereo)   (mono 16kHz)       (~250ms)
```

### 延迟

```
松开 F2 → WAV(5ms) → STT(660ms) → LLM(250ms) → 显示
总计: ~1s
```

## 项目结构

```
src/
├── main.rs           # 固定时长捕获（测试用）
├── bin/
│   ├── hotkey.rs     # 按键控制 + 内置悬浮窗（主力）
│   ├── bench.rs      # LLM 延迟基准测试
│   └── verify.rs     # WAV 格式诊断
├── api.rs            # YAML 配置读取
├── audio.rs          # WASAPI Loopback 捕获
├── stt.rs            # SiliconFlow REST STT
└── llm.rs            # LLM 回答生成（支持多提供商）
```

## 可用命令

| 命令 | 功能 |
|------|------|
| `cargo run --release` | 主力：按键控制 + 悬浮窗 |
| `cargo run --release --bin bench` | 拉取可用模型 → 延迟排名 |
| `cargo run --release --bin verify captured.wav` | WAV 文件格式验证 |

## 技术栈

| 组件 | 技术 |
|------|------|
| 音频捕获 | WASAPI Loopback (Win32 COM) |
| 语音识别 | SiliconFlow SenseVoiceSmall (REST) |
| 文本生成 | DeepSeek V4 Flash / OpenAI 兼容接口 |
| 悬浮窗 | 原生 Win32 窗口 + `WDA_EXCLUDEFROMCAPTURE` |
| 配置 | YAML (`config.yaml`) |
| 语言 | Rust |
