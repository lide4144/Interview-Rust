//! 音频捕获模块 — WASAPI Loopback 实现
//!
//! 原理：Windows WASAPI 允许以输入流方式打开输出设备（Loopback 模式）。
//! 操作系统将正在播放的音频数据原样复制给我们。
//! 面试场景：你戴耳机听 → 系统把同样的 PCM 数据给程序 → STT → LLM

use anyhow::Result;
use windows::Win32::Media::Audio::{
    eConsole, eRender, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use std::sync::atomic::{AtomicBool, Ordering};

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

/// 列出所有音频输出设备
pub fn list_devices() -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let count = collection.GetCount()?;
            println!("📋 输出设备 ({count} 个):");
            for i in 0..count {
                let dev = collection.Item(i)?;
                println!("  [{i}] {}", dev.GetId()?.to_string()?);
            }
        }
        CoUninitialize();
    }
    Ok(())
}

/// WASAPI Loopback 捕获器
pub struct LoopbackCapture {
    audio_client: IAudioClient,
    capture_client: IAudioCaptureClient,
    _event: isize,
    pub sample_rate: u32,
    pub channels: u16,
    /// 16 = PCM int16, 32 = IEEE float
    pub bits_per_sample: u16,
    frame_size: usize,
}

impl LoopbackCapture {
    pub fn new() -> Result<Self> {
        unsafe {
            // 1. COM 初始化
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

            // 2. 创建 MMDeviceEnumerator
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            // 3. 获取默认扬声器
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            println!("🎯 设备: {}", device.GetId()?.to_string()?);

            // 4. 激活 IAudioClient
            let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            // 5. 读取格式信息（packed struct → read_unaligned）
            let fmt_ptr = audio_client.GetMixFormat()?;
            let sample_rate = std::ptr::addr_of!((*fmt_ptr).nSamplesPerSec).read_unaligned();
            let channels = std::ptr::addr_of!((*fmt_ptr).nChannels).read_unaligned();
            let bits = std::ptr::addr_of!((*fmt_ptr).wBitsPerSample).read_unaligned();
            let raw_tag = std::ptr::addr_of!((*fmt_ptr).wFormatTag).read_unaligned();
            let cb_size = std::ptr::addr_of!((*fmt_ptr).cbSize).read_unaligned();

            // WAVE_FORMAT_EXTENSIBLE (0xFFFE) 的实际格式在 SubFormat GUID 里
            let (format_tag, bits) = if raw_tag == 0xFFFE && cb_size >= 22 {
                // WAVEFORMATEXTENSIBLE 在 WAVEFORMATEX 之后：
                //   cbSize  (u16) — 已在 WAVEFORMATEX 中 (offset 16)
                //   wValidBitsPerSample (u16) — offset 18
                //   dwChannelMask (u32)        — offset 20
                //   SubFormat (GUID, 16 bytes) — offset 24
                // HACK: cbSize 通常为 22，SubFormat 位于 WAVEFORMATEX 后 offset 24 处
                // 但 fmt_ptr 是 WAVEFORMATEX 的起始 (20 bytes)，所以 SubFormat 在 +24
                let guid_ptr = fmt_ptr.byte_add(24);
                let data1 = guid_ptr.cast::<u32>().read_unaligned();    // GUID Data1
                let data2 = guid_ptr.byte_add(4).cast::<u16>().read_unaligned(); // Data2
                let data3 = guid_ptr.byte_add(6).cast::<u16>().read_unaligned(); // Data3

                // KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: {00000003-0000-0010-8000-00AA00389B71}
                if data1 == 3 && data2 == 0 && data3 == 0x10 {
                    (3u16, 32u16) // WAVE_FORMAT_IEEE_FLOAT
                } else if data1 == 1 && data2 == 0 && data3 == 0x10 {
                    (1u16, bits)  // WAVE_FORMAT_PCM
                } else {
                    eprintln!("   未知 SubFormat: {data1:08X}-{data2:04X}-{data3:04X}");
                    (raw_tag, bits)
                }
            } else {
                (raw_tag, bits)
            };

            println!("   格式: {sample_rate}Hz {channels}ch {bits}bit (fmt={format_tag})");

            let frame_size = (channels as usize * bits as usize / 8) as usize;

            // 6. 创建事件对象（自复位，初始无信号）
            let event = CreateEventW(None, false, false, None)?;
            let event_raw = event.0 as isize;

            // 7. 初始化 — LOOPBACK + EVENT_CALLBACK
            let hns_period: i64 = 10_0000; // 10ms
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                hns_period,
                hns_period,
                fmt_ptr,
                None,
            )?;

            audio_client.SetEventHandle(event)?;

            let buf_frames = audio_client.GetBufferSize()?;
            println!("   缓冲区: {buf_frames} 帧 ({:.1}ms)",
                buf_frames as f64 / sample_rate as f64 * 1000.0);

            // 8. 获取 IAudioCaptureClient
            let capture_client: IAudioCaptureClient = audio_client.GetService()?;

            Ok(Self {
                audio_client,
                capture_client,
                _event: event_raw,
                sample_rate,
                channels,
                bits_per_sample: bits,
                frame_size,
            })
        }
    }

    /// 启动捕获，持续 duration_secs 秒
    pub fn capture(&self, duration_secs: u64) -> Result<Vec<u8>> {
        unsafe {
            self.audio_client.Start()?;
            println!("▶️  捕获 {duration_secs}s...\n");

            let mut data = Vec::new();
            let start = std::time::Instant::now();
            let mut last_print = start;

            loop {
                let elapsed = (std::time::Instant::now() - start).as_secs();
                if elapsed >= duration_secs || STOP_FLAG.load(Ordering::Relaxed) {
                    break;
                }

                // 等待事件（100ms 超时）
                let wait_result = WaitForSingleObject(
                    windows::Win32::Foundation::HANDLE(self._event as *mut _),
                    100,
                );
                if wait_result.0 != 0 {
                    continue;
                }

                // 读取所有可用数据包
                loop {
                    let packet_size = self.capture_client.GetNextPacketSize()?;
                    if packet_size == 0 {
                        break;
                    }

                    let mut ptr: *mut u8 = std::ptr::null_mut();
                    let mut frames: u32 = 0;
                    let mut flags: u32 = 0;

                    self.capture_client.GetBuffer(
                        &mut ptr, &mut frames, &mut flags, None, None,
                    )?;

                    if frames > 0 && !ptr.is_null() {
                        let bytes = frames as usize * self.frame_size;
                        data.extend_from_slice(
                            std::slice::from_raw_parts(ptr, bytes),
                        );
                    }

                    self.capture_client.ReleaseBuffer(frames)?;
                }

                // 刷新显示
                let now = std::time::Instant::now();
                if (now - last_print).as_millis() > 500 {
                    let elapsed_s = (now - start).as_secs_f64();
                    let captured_s = data.len() as f64
                        / self.sample_rate as f64
                        / self.channels as f64
                        / (self.bits_per_sample / 8) as f64;
                    let db = if data.len() > self.frame_size * 100 {
                        let slice = &data[data.len() - self.frame_size * 100..];
                        let rms = rms_from_bytes(slice, self.bits_per_sample);
                        20.0 * (rms.max(1e-10)).log10()
                    } else {
                        -60.0
                    };
                    let bar_n = ((db + 60.0).clamp(0.0, 60.0) / 60.0 * 30.0) as usize;
                    let bar = "█".repeat(bar_n.min(30));
                    print!("\r  [{bar:<30}] {elapsed_s:5.1}s/{duration_secs}s | {captured_s:.1}s audio  ");
                    use std::io::{self, Write};
                    let _ = io::stdout().flush();
                    last_print = now;
                }
            }

            self.audio_client.Stop()?;
            println!("\n");
            Ok(data)
        }
    }

    /// 保存 WAV 文件（float32 → int16 PCM，兼容所有播放器）
    pub fn save_wav(
        &self, data: &[u8], path: &str,
    ) -> Result<()> {
        use std::io::{Write, Seek};

        // 将捕获数据转换为 16-bit PCM
        let pcm = if self.bits_per_sample == 32 && data.len() >= 4 {
            let samples: &[f32] = bytemuck::cast_slice(data);
            samples.iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect::<Vec<i16>>()
        } else if self.bits_per_sample == 16 && data.len() >= 2 {
            bytemuck::cast_slice::<u8, i16>(data).to_vec()
        } else {
            return Ok(());
        };

        if pcm.is_empty() {
            return Ok(());
        }

        let mut f = std::fs::File::create(path)?;

        // 先写占位 header，再写数据，最后回到开头修正大小
        let byte_rate = self.sample_rate * self.channels as u32 * 16 / 8;
        let block_align: u16 = (self.channels as u32 * 16 / 8) as u16;

        // RIFF header（占位）
        f.write_all(b"RIFF")?;
        f.write_all(&0u32.to_le_bytes())?; // 稍后修正
        f.write_all(b"WAVE")?;

        // fmt chunk
        f.write_all(b"fmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?;   // PCM
        f.write_all(&self.channels.to_le_bytes())?;
        f.write_all(&self.sample_rate.to_le_bytes())?;
        f.write_all(&byte_rate.to_le_bytes())?;
        f.write_all(&block_align.to_le_bytes())?;
        f.write_all(&16u16.to_le_bytes())?;  // 16-bit

        // data chunk header（占位）
        f.write_all(b"data")?;
        let data_pos = f.stream_position()?; // 记住 data size 字段的位置
        f.write_all(&0u32.to_le_bytes())?;   // 稍后修正

        // 写入实际音频数据
        let mut bytes_written: u32 = 0;
        for s in &pcm {
            f.write_all(&s.to_le_bytes())?;
            bytes_written += 2;
        }

        // 回到文件头，修正 RIFF size 和 data size
        let riff_size = 36 + bytes_written;
        f.seek(std::io::SeekFrom::Start(4))?;
        f.write_all(&riff_size.to_le_bytes())?;
        f.seek(std::io::SeekFrom::Start(data_pos))?;
        f.write_all(&bytes_written.to_le_bytes())?;

        let duration = bytes_written as f64 / byte_rate as f64;
        println!("💾 已保存: {path} ({duration:.1}s, {:.2}MB, PCM 16-bit)",
            bytes_written as f64 / 1048576.0);
        Ok(())
    }
}

/// 计算 RMS 振幅
fn rms_from_bytes(data: &[u8], bits: u16) -> f64 {
    match bits {
        16 => {
            let samples: &[i16] = bytemuck::cast_slice(data);
            let sq: f64 = samples.iter().map(|&s| (s as f64 / 32768.0).powi(2)).sum();
            (sq / samples.len() as f64).sqrt()
        }
        32 => {
            let samples: &[f32] = bytemuck::cast_slice(data);
            let sq: f64 = samples.iter().map(|&s| s as f64 * s as f64).sum();
            (sq / samples.len() as f64).sqrt()
        }
        _ => 0.0,
    }
}
