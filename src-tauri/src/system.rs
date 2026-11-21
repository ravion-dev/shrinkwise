use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::System;

use crate::portable;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub brand: String,
    pub vendor: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vendor_label: String,
    pub memory_mb: u64,
    pub driver: String,
    pub software: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu: CpuInfo,
    pub memory_total_mb: u64,
    pub memory_available_mb: u64,
    pub gpus: Vec<GpuInfo>,
    pub recommended_workers: usize,
    pub recommended_encoder: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CpuBenchmark {
    pub score_mbps: f64,
    pub elapsed_ms: u64,
    pub recommended_workers: usize,
    pub label: String,
}

pub fn detect() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let logical_cores = sys.cpus().len().max(1);
    let physical_cores = sys.physical_core_count().unwrap_or(logical_cores);
    let first_cpu = sys.cpus().first();
    let cpu = CpuInfo {
        brand: first_cpu
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "Unknown CPU".into()),
        vendor: first_cpu
            .map(|c| c.vendor_id().trim().to_string())
            .unwrap_or_default(),
        physical_cores,
        logical_cores,
        frequency_mhz: first_cpu.map(|c| c.frequency()).unwrap_or(0),
    };

    let gpus = detect_gpus();
    let recommended_encoder = recommend_encoder(&gpus);
    let recommended_workers = recommend_workers(logical_cores, physical_cores);

    SystemInfo {
        os: format!(
            "{} {}",
            System::name().unwrap_or_else(|| "Windows".into()),
            System::os_version().unwrap_or_default()
        )
        .trim()
        .to_string(),
        arch: std::env::consts::ARCH.into(),
        cpu,
        memory_total_mb: sys.total_memory() / (1024 * 1024),
        memory_available_mb: sys.available_memory() / (1024 * 1024),
        gpus,
        recommended_workers,
        recommended_encoder,
    }
}

fn recommend_workers(logical: usize, physical: usize) -> usize {
    // Ağır video işlerinde fiziksel çekirdek sayısı daha iyi sonuç verir;
    // küçük dosyalarda ise mantıksal çekirdeklere kadar çıkılabilir.
    logical.max(physical).clamp(1, 16)
}

fn recommend_encoder(gpus: &[GpuInfo]) -> String {
    let has_nvidia = gpus.iter().any(|g| g.vendor == "nvidia" && !g.software);
    let has_amd = gpus.iter().any(|g| g.vendor == "amd" && !g.software);
    let has_intel = gpus.iter().any(|g| g.vendor == "intel" && !g.software);
    if has_nvidia {
        "nvenc".into()
    } else if has_amd {
        "amf".into()
    } else if has_intel {
        "qsv".into()
    } else {
        "cpu".into()
    }
}

fn vendor_label(vendor: &str) -> &'static str {
    match vendor {
        "nvidia" => "NVIDIA",
        "amd" => "AMD",
        "intel" => "Intel",
        "microsoft" => "Microsoft",
        _ => "Other",
    }
}

pub fn vendor_from_name(name: &str) -> &'static str {
    let low = name.to_lowercase();
    if low.contains("nvidia") || low.contains("geforce") || low.contains("quadro") {
        "nvidia"
    } else if low.contains("amd") || low.contains("radeon") || low.contains("firepro") {
        "amd"
    } else if low.contains("intel") || low.contains("arc") || low.contains("iris") {
        "intel"
    } else if low.contains("microsoft") || low.contains("basic render") {
        "microsoft"
    } else {
        "other"
    }
}

// ---------------------------------------------------------------------------
//  DXGI ile GPU listesi (Windows)
// ---------------------------------------------------------------------------
#[cfg(windows)]
fn detect_gpus() -> Vec<GpuInfo> {
    if let Ok(gpus) = detect_gpus_dxgi()
        && !gpus.is_empty()
    {
        return gpus;
    }
    detect_gpus_powershell()
}

#[cfg(windows)]
fn detect_gpus_dxgi() -> anyhow::Result<Vec<GpuInfo>> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1,
    };

    let mut out = Vec::new();
    // SAFETY: DXGI factory creation and adapter enumeration calls follow the Win32 DXGI COM contract.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut index = 0u32;
        loop {
            let adapter = match factory.EnumAdapters1(index) {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            index += 1;
            let desc = match adapter.GetDesc1() {
                Ok(desc) => desc,
                Err(_) => continue,
            };
            let name = String::from_utf16_lossy(
                &desc
                    .Description
                    .iter()
                    .take_while(|ch| **ch != 0)
                    .copied()
                    .collect::<Vec<u16>>(),
            )
            .trim()
            .to_string();
            if name.is_empty() {
                continue;
            }
            let vendor = vendor_from_name(&name);
            let software = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0
                || vendor == "microsoft"
                || name.to_lowercase().contains("basic render");
            out.push(GpuInfo {
                name,
                vendor: vendor.into(),
                vendor_label: vendor_label(vendor).into(),
                memory_mb: (desc.DedicatedVideoMemory as u64) / (1024 * 1024),
                driver: String::new(),
                software,
            });
        }
    }
    Ok(out)
}

#[cfg(not(windows))]
fn detect_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

#[cfg(windows)]
fn detect_gpus_powershell() -> Vec<GpuInfo> {
    let script = "@(Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM,DriverVersion,VideoProcessor) | ConvertTo-Json -Compress";
    let mut cmd = portable::new_command("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    let Ok(output) = cmd.output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim()) else {
        return Vec::new();
    };
    let items = match value {
        serde_json::Value::Array(items) => items,
        other => vec![other],
    };
    items
        .into_iter()
        .filter_map(|item| {
            let name = item.get("Name")?.as_str()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let vendor = vendor_from_name(&name);
            let ram = item.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);
            let driver = item
                .get("DriverVersion")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            Some(GpuInfo {
                name,
                vendor: vendor.into(),
                vendor_label: vendor_label(vendor).into(),
                // WMI 32-bit alanı 4 GB'ta doyar; bilgi amaçlı gösterilir.
                memory_mb: ram / (1024 * 1024),
                driver,
                software: vendor == "microsoft",
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
//  Hızlı CPU ölçümü (işçi sayısı önerisi için)
// ---------------------------------------------------------------------------
pub fn benchmark_cpu() -> CpuBenchmark {
    let start = Instant::now();
    // 16 MB yarı sıkıştırılamaz veri: bellek + ALU karışımı.
    let mut data = vec![0u8; 16 * 1024 * 1024];
    let mut state = 0x1234_5678u64;
    for chunk in data.chunks_mut(8) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let bytes = state.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    // Sikistirma da olculen is yukunun parcasi; sonuc yalnizca CPU yukunu olcmek icin.
    let _compressed = zstd::stream::encode_all(&data[..], 3).unwrap_or_default();
    let elapsed = start.elapsed();
    let mbps = if elapsed.as_secs_f64() > 0.0 {
        (data.len() as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let recommended = if mbps < 180.0 {
        (logical / 2).max(2)
    } else if mbps < 420.0 {
        (logical * 3 / 4).max(2)
    } else {
        logical
    }
    .clamp(1, 32);
    let label = if mbps < 180.0 {
        "Low / older CPU"
    } else if mbps < 420.0 {
        "Mid-range"
    } else if mbps < 900.0 {
        "Fast"
    } else {
        "Very fast"
    };
    CpuBenchmark {
        score_mbps: (mbps * 10.0).round() / 10.0,
        elapsed_ms: elapsed.as_millis() as u64,
        recommended_workers: recommended,
        label: label.into(),
    }
}

pub fn wait_for_process_exit(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        #[cfg(windows)]
        {
            let out = portable::new_command("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output();
            if let Ok(out) = out
                && !String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
            {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}
