//! WAV 验证工具 — 诊断 captured.wav 是否格式正确
//!
//! 用法:
//!   cargo run --release --bin verify captured.wav
//!   或直接双击 verify.exe 拖入 wav 文件

use std::fs;
use anyhow::{anyhow, Result};

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "captured.wav".to_string());

    println!("╔═════════════════════════════════╗");
    println!("║   WAV 验证工具                  ║");
    println!("╚═════════════════════════════════╝\n");
    println!("📁 文件: {}\n", path);

    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            println!("❌ 无法读取: {e}");
            return Ok(());
        }
    };

    if data.len() < 44 {
        println!("❌ 文件太小（< 44 字节），不是有效的 WAV");
        return Ok(());
    }

    // 解析 header（手动解析，避免依赖）
    let riff_id = [data[0], data[1], data[2], data[3]];
    let riff_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let wave_id = [data[8], data[9], data[10], data[11]];
    let fmt_id = [data[12], data[13], data[14], data[15]];
    let fmt_size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let audio_format = u16::from_le_bytes([data[20], data[21]]);
    let channels = u16::from_le_bytes([data[22], data[23]]);
    let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
    let byte_rate = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
    let block_align = u16::from_le_bytes([data[32], data[33]]);
    let bits_per_sample = u16::from_le_bytes([data[34], data[35]]);

    // data chunk 可能在 offset 36 或更远（如果有其他 chunk）
    let (data_id, data_size, audio_offset) = if fmt_size == 16 {
        // 标准 PCM，data 紧跟 fmt
        let id = [data[36], data[37], data[38], data[39]];
        let size = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        (id, size, 44u64)
    } else {
        // fmt 后面可能有 fact chunk 或其他，需要查找 "data"
        find_data_chunk(&data, 20 + fmt_size as u64)?
    };

    let expected_riff_size = (4 + 8 + fmt_size + 8 + data_size) as u32;
    let expected_file_size = 8 + expected_riff_size as u64;
    let riff_ok = riff_size == expected_riff_size;
    let data_ok = data_size as u64 + audio_offset == data.len() as u64;
    let sample_count = if channels > 0 && bits_per_sample > 0 {
        data_size as u64 / (channels as u64 * bits_per_sample as u64 / 8)
    } else {
        println!("   ⚠️  channels={channels} bits={bits_per_sample} → 无法计算样本数");
        0
    };
    let duration_secs = if byte_rate > 0 {
        data_size as f64 / byte_rate as f64
    } else {
        println!("   ⚠️  byte_rate=0 → 无法计算时长");
        0.0
    };

    // ─── 原始字节 dump ─────────────────────
    println!("\n📊 原始 Header 字节 (前 44 字节):");
    for i in 0..44.min(data.len()) {
        if i % 16 == 0 { print!("  {:04X}: ", i); }
        print!("{:02X} ", data[i]);
        if i % 16 == 15 { println!(); }
    }
    if data.len() % 16 != 0 { println!(); }

    // ─── 输出 ─────────────────────────────────────
    println!("📊 RIFF 头");
    println!("   ID:     {:?}", std::str::from_utf8(&riff_id).unwrap_or("???"));
    println!("   Size:   {riff_size} (0x{riff_size:08X})");
    if !riff_ok {
        println!("   ⚠️  预期: {expected_riff_size}, 实际声明: {riff_size}");
        println!("   ⚠️  声明文件大小应为: {expected_file_size}, 实际: {}", data.len());
        println!("   ⚠️  偏差: {} 字节", data.len() as i64 - expected_file_size as i64);
    }
    println!("   WAVE:   {:?}", std::str::from_utf8(&wave_id).unwrap_or("???"));

    println!("\n📊 fmt 块");
    println!("   ID:     {:?}", std::str::from_utf8(&fmt_id).unwrap_or("???"));
    println!("   Size:   {fmt_size}");
    println!("   Format: {audio_format} ({})", fmt_name(audio_format));
    println!("   Chan:   {channels}");
    println!("   Rate:   {sample_rate} Hz");
    println!("   B/sec:  {byte_rate}");
    println!("   Align:  {block_align}");
    println!("   Bits:   {bits_per_sample}");

    // 验证 fmt 参数一致性
    let calc_byte_rate = sample_rate as u64 * channels as u64 * bits_per_sample as u64 / 8;
    let calc_block_align = channels as u64 * bits_per_sample as u64 / 8;
    let fmt_ok = byte_rate as u64 == calc_byte_rate
        && block_align as u64 == calc_block_align;

    println!("   ✓ Rate:   {} ← 验证: {}", byte_rate, calc_byte_rate);
    println!("   ✓ Align:  {} ← 验证: {}", block_align, calc_block_align);

    println!("\n📊 data 块");
    println!("   ID:     {:?}", std::str::from_utf8(&data_id).unwrap_or("???"));
    println!("   Size:   {data_size} (0x{data_size:08X})");
    println!("   起始:   offset {audio_offset}");
    println!("   样本数: {sample_count} ({:.1}s)", duration_secs);

    // 最终判定
    println!("\n{}", "=".repeat(40));

    let all_ids_ok = &riff_id == b"RIFF"
        && &wave_id == b"WAVE"
        && &fmt_id == b"fmt "
        && &data_id == b"data";

    let checks = [
        ("RIFF/WAVE/fmt/data ID", all_ids_ok),
        ("fmt 参数一致", fmt_ok),
        ("RIFF size 正确", riff_ok),
        ("data size 与文件一致", data_ok),
        ("音频格式可播放", audio_format == 1 || audio_format == 3),
    ];

    let mut all_ok = true;
    for (name, ok) in &checks {
        let icon = if *ok { "✅" } else { "❌" };
        println!("  {icon} {name}");
        if !ok { all_ok = false; }
    }

    println!();
    if all_ok && data_size > 0 {
        println!("✅🎉 文件完全正确，可以播放！");
        println!("   如果仍然无法播放，检查播放器是否支持 {}bit WAV", bits_per_sample);
        println!("   建议用 VLC、Audacity 或 ffplay 打开");
    } else if data_size == 0 {
        println!("⚠️  音频数据为 0 字节（没有捕获到声音）");
        println!("   打开 YouTube 播放视频后重试");
    } else {
        println!("❌ 文件存在问题，详情见上方 ❌ 标记");
        println!();

        // 自动修复尝试
        if !riff_ok || !data_ok {
            println!("🔧 尝试自动修复...");
            // 修复：重写正确的 RIFF size
            if data_size as u64 + audio_offset <= data.len() as u64 {
                let fixed_path = path.replace(".wav", "_fixed.wav");
                let mut fixed = data.clone();
                let correct_riff = (4 + 8 + fmt_size + 8 + data_size) as u32;
                fixed[4..8].copy_from_slice(&correct_riff.to_le_bytes());
                fixed.truncate((8 + correct_riff as u64) as usize);
                fs::write(&fixed_path, &fixed)?;
                println!("   ✅ 已修复: {fixed_path}");
                println!("   请用播放器打开 fixed 版本");
            }
        }
    }

    // 显示前几个音频样本
    if data_size >= 10 {
        println!("\n📊 前 5 帧音频样本:");
        let start = audio_offset as usize;
        match bits_per_sample {
            16 => {
                let samples: &[i16] = bytemuck::cast_slice(&data[start..start + data_size as usize]);
                for i in 0..5.min(samples.len() / channels as usize) {
                    let offset = i * channels as usize;
                    print!("  [{i:3}] ");
                    for c in 0..channels as usize {
                        print!("ch{c}={:6} ", samples[offset + c]);
                    }
                    println!();
                }
            }
            32 => {
                // 可能是 int32 或 float32
                if audio_format == 3 {
                    let samples: &[f32] = bytemuck::cast_slice(&data[start..start + data_size as usize]);
                    for i in 0..5.min(samples.len() / channels as usize) {
                        let offset = i * channels as usize;
                        print!("  [{i:3}] ");
                        for c in 0..channels as usize {
                            print!("ch{c}={:+.6} ", samples[offset + c]);
                        }
                        println!();
                    }
                }
            }
            _ => println!("  (不显示 {bits_per_sample}bit 样本)"),
        }
    }

    Ok(())
}

fn fmt_name(tag: u16) -> &'static str {
    match tag {
        0x0001 => "PCM",
        0x0003 => "IEEE_FLOAT",
        0x0006 => "ALAW",
        0x0007 => "MULAW",
        0xFFFE => "EXTENSIBLE",
        _ => "UNKNOWN",
    }
}

fn find_data_chunk(data: &[u8], start: u64) -> Result<([u8; 4], u32, u64)> {
    let mut pos = start as usize;
    while pos + 8 <= data.len() {
        let id = [data[pos], data[pos+1], data[pos+2], data[pos+3]];
        let size = u32::from_le_bytes([data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
        if &id == b"data" {
            return Ok((id, size, (pos + 8) as u64));
        }
        pos += 8 + size as usize;
    }
    Err(anyhow!("找不到 data chunk"))
}
