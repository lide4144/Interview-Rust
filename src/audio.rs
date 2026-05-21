//! 音频捕获模块 — WASAPI Loopback 实现
//!
//! 模式：
//!   capture(N)      — 固定时长捕获（测试用）
//!   new_for_stream / read_direct / into_buffer — 按键控制捕获（主力）

use anyhow::Result;
use std::sync::Mutex;
use std::time::Instant;
use windows::Win32::Media::Audio::{
    eConsole, eRender, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

pub struct LoopbackCapture {
    audio_client: IAudioClient,
    capture_client: IAudioCaptureClient,
    event_raw: isize,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    frame_size: usize,
    capture_buffer: Mutex<Vec<u8>>,
}

impl LoopbackCapture {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            println!("🎯 设备: {}", device.GetId()?.to_string()?);

            let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            let fmt_ptr = audio_client.GetMixFormat()?;
            let sample_rate = std::ptr::addr_of!((*fmt_ptr).nSamplesPerSec).read_unaligned();
            let channels = std::ptr::addr_of!((*fmt_ptr).nChannels).read_unaligned();
            let bits = std::ptr::addr_of!((*fmt_ptr).wBitsPerSample).read_unaligned();
            let raw_tag = std::ptr::addr_of!((*fmt_ptr).wFormatTag).read_unaligned();
            let cb_size = std::ptr::addr_of!((*fmt_ptr).cbSize).read_unaligned();

            let (_, bits) = if raw_tag == 0xFFFE && cb_size >= 22 {
                let guid_ptr = fmt_ptr.byte_add(24);
                let d1 = guid_ptr.cast::<u32>().read_unaligned();
                let d2 = guid_ptr.byte_add(4).cast::<u16>().read_unaligned();
                let d3 = guid_ptr.byte_add(6).cast::<u16>().read_unaligned();
                if d1 == 3 && d2 == 0 && d3 == 0x10 {
                    (3u16, 32u16)
                } else {
                    (raw_tag, bits)
                }
            } else {
                (raw_tag, bits)
            };

            println!("   格式: {sample_rate}Hz {channels}ch {bits}bit");

            let frame_size = (channels as usize * bits as usize / 8) as usize;

            let event = CreateEventW(None, false, false, None)?;
            let event_raw = event.0 as isize;

            let hns_period: i64 = 10_0000;
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                hns_period,
                hns_period,
                fmt_ptr,
                None,
            )?;

            audio_client.SetEventHandle(event)?;

            let buf = audio_client.GetBufferSize()?;
            println!("   缓冲区: {buf} 帧 ({:.1}ms)", buf as f64 / sample_rate as f64 * 1000.0);

            let capture_client: IAudioCaptureClient = audio_client.GetService()?;

            Ok(Self {
                audio_client,
                capture_client,
                event_raw,
                sample_rate,
                channels,
                bits_per_sample: bits,
                frame_size,
                capture_buffer: Mutex::new(Vec::new()),
            })
        }
    }

    // ── 固定时长捕获 ──────────────────────────────

    pub fn capture(&self, duration_secs: u64) -> Result<Vec<u8>> {
        unsafe {
            self.audio_client.Start()?;
            println!("▶️  捕获 {duration_secs}s...\n");

            let mut data = Vec::new();
            let start = Instant::now();
            let mut last_print = start;

            loop {
                if (Instant::now() - start).as_secs() >= duration_secs { break; }

                let wait = WaitForSingleObject(
                    windows::Win32::Foundation::HANDLE(self.event_raw as *mut _), 100);
                if wait.0 != 0 { continue; }

                loop {
                    let sz = self.capture_client.GetNextPacketSize()?;
                    if sz == 0 { break; }
                    let mut ptr: *mut u8 = std::ptr::null_mut();
                    let mut frames: u32 = 0;
                    let mut flags: u32 = 0;
                    self.capture_client.GetBuffer(&mut ptr, &mut frames, &mut flags, None, None)?;
                    if frames > 0 && !ptr.is_null() {
                        data.extend_from_slice(std::slice::from_raw_parts(
                            ptr, frames as usize * self.frame_size));
                    }
                    self.capture_client.ReleaseBuffer(frames)?;
                }

                let now = Instant::now();
                if (now - last_print).as_millis() > 500 {
                    let elapsed_s = (now - start).as_secs_f64();
                    let cap_s = data.len() as f64 / self.bytes_per_second() as f64;
                    let db = vu_db(&data, self.frame_size, self.bits_per_sample);
                    let bar = "█".repeat(((db + 60.0).clamp(0.0, 60.0) / 60.0 * 30.0) as usize);
                    print!("\r  [{bar:<30}] {elapsed_s:5.1}s/{duration_secs}s | {cap_s:.1}s audio  ");
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

    // ── 直接捕获（hotkey 用）─────────────────────

    pub fn new_for_stream() -> Result<Self> {
        let s = Self::new()?;
        unsafe { s.audio_client.Start()?; }
        Ok(s)
    }

    /// 直接从 WASAPI 读取约 ms 毫秒音频（阻塞、实时）
    pub fn read_direct(&self, ms: u64) -> Result<Vec<u8>> {
        let target = self.bytes_per_second() * ms as usize / 1000;
        let mut data = Vec::with_capacity(target);
        let deadline = Instant::now() + std::time::Duration::from_millis(ms * 2);
        unsafe {
            while data.len() < target && Instant::now() < deadline {
                let w = WaitForSingleObject(
                    windows::Win32::Foundation::HANDLE(self.event_raw as *mut _), 50);
                if w.0 != 0 { continue; }
                loop {
                    let sz = self.capture_client.GetNextPacketSize()?;
                    if sz == 0 { break; }
                    let mut ptr: *mut u8 = std::ptr::null_mut();
                    let mut frames: u32 = 0;
                    let mut flags: u32 = 0;
                    self.capture_client.GetBuffer(&mut ptr, &mut frames, &mut flags, None, None)?;
                    if frames > 0 && !ptr.is_null() {
                        data.extend_from_slice(std::slice::from_raw_parts(
                            ptr, frames as usize * self.frame_size));
                    }
                    self.capture_client.ReleaseBuffer(frames)?;
                }
            }
        }
        Ok(data)
    }

    pub fn append_to_buffer(&self, data: &[u8]) {
        self.capture_buffer.lock().unwrap().extend_from_slice(data);
    }
    pub fn buffer_len(&self) -> usize { self.capture_buffer.lock().unwrap().len() }
    pub fn buffer_vu_db(&self) -> f64 {
        let g = self.capture_buffer.lock().unwrap();
        vu_db(&g, self.frame_size, self.bits_per_sample)
    }
    pub fn into_buffer(&self) -> Vec<u8> {
        std::mem::take(&mut *self.capture_buffer.lock().unwrap())
    }

    // ── 工具 ─────────────────────────────────────

    pub fn frame_size(&self) -> usize { self.frame_size }
    pub fn bytes_per_second(&self) -> usize {
        self.sample_rate as usize * self.channels as usize * self.bits_per_sample as usize / 8
    }

    /// 将原始音频转换为 mono 16kHz i16 PCM（用于 STT）
    pub fn to_mono_16k(&self, data: &[u8]) -> Vec<u8> {
        if self.bits_per_sample != 32 || data.len() < 8 { return Vec::new(); }
        let samples: &[f32] = bytemuck::cast_slice(&data[..data.len() - data.len() % 4]);
        let frames = samples.len() / 2;
        let step = 3; // 48000/16000
        let mut out = Vec::with_capacity(frames / step * 2);
        for i in (0..frames).step_by(step) {
            let mono = (samples[i * 2] + samples[i * 2 + 1]) * 0.5;
            let s = (mono.clamp(-1.0, 1.0) * 32767.0) as i16;
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    pub fn save_wav(&self, data: &[u8], path: &str) -> Result<()> {
        use std::io::{BufWriter, Write, Seek};
        let pcm = if self.bits_per_sample == 32 && data.len() >= 4 {
            let len = data.len() - data.len() % 4;
            let samples: &[f32] = bytemuck::cast_slice(&data[..len]);
            samples.iter().map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16).collect()
        } else if self.bits_per_sample == 16 && data.len() >= 2 {
            let len = data.len() - data.len() % 2;
            bytemuck::cast_slice::<u8, i16>(&data[..len]).to_vec()
        } else { return Ok(()); };
        if pcm.is_empty() { return Ok(()); }

        let mut f = BufWriter::with_capacity(256 * 1024, std::fs::File::create(path)?);
        let byte_rate = self.sample_rate * self.channels as u32 * 16 / 8;
        let block_align: u16 = (self.channels as u32 * 16 / 8) as u16;

        f.write_all(b"RIFF")?;
        f.write_all(&0u32.to_le_bytes())?;
        f.write_all(b"WAVE")?;
        f.write_all(b"fmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&1u16.to_le_bytes())?;
        f.write_all(&self.channels.to_le_bytes())?;
        f.write_all(&self.sample_rate.to_le_bytes())?;
        f.write_all(&byte_rate.to_le_bytes())?;
        f.write_all(&block_align.to_le_bytes())?;
        f.write_all(&16u16.to_le_bytes())?;
        f.write_all(b"data")?;
        let data_pos = f.stream_position()?;
        f.write_all(&0u32.to_le_bytes())?;

        // 批量写入 i16（远快于逐个写）
        let pcm_bytes: &[u8] = bytemuck::cast_slice(&pcm);
        f.write_all(pcm_bytes)?;
        let bytes_written = pcm_bytes.len() as u32;

        let riff_size = 36 + bytes_written;
        f.seek(std::io::SeekFrom::Start(4))?;
        f.write_all(&riff_size.to_le_bytes())?;
        f.seek(std::io::SeekFrom::Start(data_pos))?;
        f.write_all(&bytes_written.to_le_bytes())?;
        f.flush()?;

        let dur = bytes_written as f64 / byte_rate as f64;
        println!("💾 {path} ({dur:.1}s, {:.2}MB)", bytes_written as f64 / 1048576.0);
        Ok(())
    }
}

impl Drop for LoopbackCapture {
    fn drop(&mut self) { unsafe { let _ = self.audio_client.Stop(); } }
}

fn vu_db(data: &[u8], frame_size: usize, bits: u16) -> f64 {
    if data.len() < frame_size * 100 { return -60.0; }
    let slice = &data[data.len() - frame_size * 100..];
    match bits {
        16 => {
            let samples: &[i16] = bytemuck::cast_slice(slice);
            let sq: f64 = samples.iter().map(|&s| (s as f64 / 32768.0).powi(2)).sum();
            20.0 * (sq / samples.len() as f64).sqrt().max(1e-10).log10()
        }
        32 => {
            let samples: &[f32] = bytemuck::cast_slice(slice);
            let sq: f64 = samples.iter().map(|&s| s as f64 * s as f64).sum();
            20.0 * (sq / samples.len() as f64).sqrt().max(1e-10).log10()
        }
        _ => -60.0,
    }
}
