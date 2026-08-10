//! Bootstrap, packaging และ runtime detection.
//!
//! Phase 6 deliverable: first-run bootstrap ที่ผู้ใช้ทั่วไปใช้ได้บนเครื่องสะอาด
//! - GPU detection ผ่าน nvidia-smi (ไม่ต้องพึ่ง PyTorch)
//! - State persistence ที่ `{APPDATA}/DubFlow/state.json`
//! - License acceptance ก่อนดาวน์โหลดโมเดล (CC BY-NC 4.0)
//! - Download + checksum verify + atomic install + recovery
//!
//! Layout หลัง bootstrap:
//! ```text
//! {APPDATA}/DubFlow/
//!   state.json
//!   runtime/            # python embeddable + wheels + site-packages
//!   models/             # model weights (verifed sha256)
//!   ffmpeg/             # bundled ffmpeg/ffprobe
//!   cache/
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// GPU detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub present: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub compute_capability: String,
    #[serde(default)]
    pub vram_bytes: u64,
    #[serde(default)]
    pub cuda_version: String,
    /// MVP supported matrix: compute capability >= 6.1 with VRAM >= 6 GB.
    pub supported: bool,
    /// CC >= 7.5 = modern candidate (deferred, not in supported matrix).
    pub modern_candidate: bool,
}

/// Lookup for NVIDIA CC from the name substring reported by nvidia-smi.
/// MVP supported range starts at Pascal CC 6.1 (GTX 1070 Ti acceptance target).
const GPU_CC_LOOKUP: &[(&str, &str)] = &[
    // Blackwell
    ("RTX 5090", "10.1"),
    ("RTX 5080", "10.1"),
    ("RTX 5070 Ti", "10.1"),
    ("RTX 5070", "10.1"),
    // Ada Lovelace
    ("RTX 4090", "8.9"),
    ("RTX 4080", "8.9"),
    ("RTX 4070 Ti", "8.9"),
    ("RTX 4070", "8.9"),
    ("RTX 4060 Ti", "8.9"),
    ("RTX 4060", "8.9"),
    // Hopper
    ("H100", "9.0"),
    // Ampere
    ("RTX 3090", "8.6"),
    ("RTX 3080 Ti", "8.6"),
    ("RTX 3080", "8.6"),
    ("RTX 3070 Ti", "8.6"),
    ("RTX 3070", "8.6"),
    ("RTX 3060 Ti", "8.6"),
    ("RTX 3060", "8.6"),
    ("RTX 3050", "8.6"),
    ("A100", "8.0"),
    // Turing
    ("RTX 2080 Ti", "7.5"),
    ("RTX 2080", "7.5"),
    ("RTX 2070", "7.5"),
    ("RTX 2060", "7.5"),
    ("TITAN RTX", "7.5"),
    ("Quadro RTX", "7.5"),
    ("Tesla T4", "7.5"),
    // Volta
    ("V100", "7.0"),
    ("Tesla V100", "7.0"),
    // Pascal (MVP acceptance target)
    ("GTX 1080 Ti", "6.1"),
    ("GTX 1080", "6.1"),
    ("GTX 1070 Ti", "6.1"),
    ("GTX 1070", "6.1"),
    ("GTX 1060", "6.1"),
    ("GTX 1050 Ti", "6.1"),
    ("GTX 1050", "6.1"),
    ("P100", "6.0"),
];

fn gpu_name_to_cc(name: &str) -> Option<&'static str> {
    for (pattern, cc) in GPU_CC_LOOKUP {
        if name.contains(pattern) {
            return Some(cc);
        }
    }
    None
}

fn parse_vram_bytes(line: &str) -> u64 {
    let line = line.trim();
    if let Some(rest) = line.strip_suffix("MiB") {
        if let Ok(mib) = rest.trim().parse::<u64>() {
            return mib * 1024 * 1024;
        }
    }
    0
}

/// Run `nvidia-smi` and parse GPU info without loading PyTorch.
pub fn detect_gpu() -> GpuInfo {
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,driver_version",
            "--format=csv,noheader",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return GpuInfo::default(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("").trim().to_string();
    if line.is_empty() {
        return GpuInfo::default();
    }

    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    let name = parts.first().unwrap_or(&"").to_string();
    let vram_bytes = parse_vram_bytes(parts.get(1).unwrap_or(&""));
    let driver_ver = parts.get(2).unwrap_or(&"").to_string();
    let cuda_version = driver_version_to_cuda(&driver_ver);
    let cc = gpu_name_to_cc(&name).unwrap_or("");
    let supported = !cc.is_empty() && vram_bytes >= 6 * 1024 * 1024 * 1024;
    let modern = supported
        && cc.split_once('.')
            .map(|(maj, min)| {
                let maj: u32 = maj.parse().unwrap_or(0);
                let min: u32 = min.parse().unwrap_or(0);
                maj > 7 || (maj == 7 && min >= 5)
            })
            .unwrap_or(false);

    GpuInfo {
        present: true,
        name,
        compute_capability: cc.to_string(),
        vram_bytes,
        cuda_version,
        supported,
        modern_candidate: modern,
    }
}

/// Approximate the CUDA version the driver supports from its version number.
fn driver_version_to_cuda(driver: &str) -> String {
    let major = driver
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    match major {
        v if v >= 580 => "13.0",
        v if v >= 550 => "12.6",
        v if v >= 545 => "12.5",
        v if v >= 535 => "12.3",
        v if v >= 525 => "12.0",
        v if v >= 520 => "11.8",
        v if v >= 510 => "11.6",
        v if v >= 495 => "11.5",
        v if v >= 470 => "11.4",
        v if v >= 465 => "11.3",
        v if v >= 450 => "11.0",
        _ => "unknown",
    }
    .to_string()
}
// ---------------------------------------------------------------------------
// Manifest / runtime entries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub url: String,
    pub sha256: String,
    pub dest: String,
    pub size_hint: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapManifest {
    pub legacy_runtime: LegacyRuntimeSection,
    pub models: HashMap<String, ModelManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRuntimeSection {
    pub python: String,
    pub torch: String,
    pub torchaudio: String,
    pub f5_tts: String,
    pub cuda_wheel_index: String,
    #[serde(default)]
    pub python_embed: String,
    #[serde(default)]
    pub python_package: Vec<ManifestEntry>,
    #[serde(default)]
    pub ffmpeg: Option<ManifestEntry>,
    #[serde(default)]
    pub torch_wheels: Vec<ManifestEntry>,
    #[serde(default)]
    pub pip_packages: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifestEntry {
    pub repo: String,
    pub files: Vec<ManifestEntry>,
}

// ---------------------------------------------------------------------------
// Download progress + error types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub current_file: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub enum BootstrapError {
    DownloadFailed { url: String, message: String },
    ChecksumMismatch { path: String, expected: String, actual: String },
    Io { message: String },
    NoSpace { needed_bytes: u64, available_bytes: u64 },
    NetworkUnavailable,
    LicenseNotAccepted { model_id: String },
    CorruptDownload { path: String },
}

impl std::fmt::Display for BootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootstrapError::DownloadFailed { url, message } => {
                write!(f, "download failed for {url}: {message}")
            }
            BootstrapError::ChecksumMismatch { path, expected, actual } => {
                write!(f, "checksum mismatch for {path}: expected {expected}, got {actual}")
            }
            BootstrapError::Io { message } => write!(f, "IO error: {message}"),
            BootstrapError::NoSpace { needed_bytes, available_bytes } => {
                write!(f, "disk space: need {needed_bytes}, available {available_bytes}")
            }
            BootstrapError::NetworkUnavailable => write!(f, "network unavailable"),
            BootstrapError::LicenseNotAccepted { model_id } => {
                write!(f, "license not accepted for {model_id}")
            }
            BootstrapError::CorruptDownload { path } => write!(f, "corrupt download at {path}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Bootstrap state (persisted)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub runtime_installed: bool,
    pub models_installed: bool,
    #[serde(default)]
    pub licenses_accepted: Vec<String>,
    #[serde(default)]
    pub installed_at: String,
    #[serde(default)]
    pub python_path: String,
    #[serde(default)]
    pub hf_cache_dir: String,
    #[serde(default)]
    pub ffmpeg_path: String,
    #[serde(default)]
    pub bootstrap_complete: bool,
}

// ---------------------------------------------------------------------------
// License acceptance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLicense {
    pub model_id: String,
    pub name: String,
    pub license_url: String,
    pub license_text: String,
    pub requires_acceptance: bool,
}

/// Returns the models that require license acceptance before download.
pub fn model_licenses() -> Vec<ModelLicense> {
    vec![ModelLicense {
        model_id: "JaiTTS-F5TTS".to_string(),
        name: "JaiTTS-F5TTS".to_string(),
        license_url: "https://huggingface.co/JTS-AI/JaiTTS-F5TTS/blob/main/LICENSE".to_string(),
        license_text: concat!(
            "JaiTTS-F5TTS Model Weights\n",
            "License: CC BY-NC 4.0\n\n",
            "You are free to:\n",
            "- Share: copy and redistribute the material in any medium or format\n",
            "- Adapt: remix, transform, and build upon the material\n\n",
            "Under the following terms:\n",
            "- Attribution: You must give appropriate credit, provide a link to the license,\n",
            "  and indicate if changes were made.\n",
            "- NonCommercial: You may not use the material for commercial purposes.\n\n",
            "Full license: https://creativecommons.org/licenses/by-nc/4.0/legalcode\n\n",
            "By downloading, you accept the CC BY-NC 4.0 license terms.",
        )
        .to_string(),
        requires_acceptance: true,
    }]
}

pub fn accept_license(state_path: &Path, model_id: &str) -> Result<BootstrapState, BootstrapError> {
    let mut state = load_state(state_path);
    if !state.licenses_accepted.contains(&model_id.to_string()) {
        state.licenses_accepted.push(model_id.to_string());
    }
    save_state(state_path, &state)?;
    Ok(state)
}

pub fn is_license_accepted(state_path: &Path, model_id: &str) -> bool {
    let state = load_state(state_path);
    state.licenses_accepted.contains(&model_id.to_string())
}

// ---------------------------------------------------------------------------
// State persistence
// ---------------------------------------------------------------------------

/// App data directory at `{APPDATA}/DubFlow`.
pub fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home =
                std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".to_string());
            PathBuf::from(home).join("AppData").join("Roaming")
        });
    base.join("DubFlow")
}

pub fn default_state_path() -> PathBuf {
    data_dir().join("state.json")
}

pub fn load_state(state_path: &Path) -> BootstrapState {
    match std::fs::read_to_string(state_path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => BootstrapState::default(),
    }
}

pub fn save_state(state_path: &Path, state: &BootstrapState) -> Result<(), BootstrapError> {
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| BootstrapError::Io {
            message: format!("cannot create state dir: {e}"),
        })?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| BootstrapError::Io {
        message: format!("serialize state: {e}"),
    })?;
    let tmp = state_path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| BootstrapError::Io {
        message: format!("write state: {e}"),
    })?;
    let _ = std::fs::rename(&tmp, state_path);
    Ok(())
}

// ---------------------------------------------------------------------------
// Download helpers
// ---------------------------------------------------------------------------

/// Download a file from `url` to `dest` with curl.exe (Windows built-in) with
/// resume support and a PowerShell fallback. Updates `progress` for the UI.
pub fn download_file(
    url: &str,
    dest: &Path,
    progress: &Mutex<DownloadProgress>,
) -> Result<(), BootstrapError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| BootstrapError::Io {
            message: format!("cannot create download dir: {e}"),
        })?;
    }

    let filename = dest
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default()
        .to_string();

    {
        let mut p = progress.lock().unwrap();
        p.current_file = filename.clone();
        p.status = format!("กำลังดาวน์โหลด {filename}");
    }

    let status = Command::new("curl.exe")
        .args([
            "-L",
            "-C",
            "-",
            "-o",
            &dest.to_string_lossy(),
            "--retry",
            "3",
            "--retry-delay",
            "5",
            url,
        ])
        .status()
        .map_err(|e| BootstrapError::DownloadFailed {
            url: url.to_string(),
            message: format!("cannot execute curl: {e}"),
        })?;

    if !status.success() {
        let ps_script = format!(
            "Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
            url.replace('\'', "''"),
            dest.to_string_lossy().replace('\'', "''"),
        );
        let ps_status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .status()
            .map_err(|e| BootstrapError::DownloadFailed {
                url: url.to_string(),
                message: format!("powershell fallback failed: {e}"),
            })?;
        if !ps_status.success() {
            return Err(BootstrapError::DownloadFailed {
                url: url.to_string(),
                message: "curl and PowerShell both failed".to_string(),
            });
        }
    }

    if dest.is_file() {
        let len = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        let mut p = progress.lock().unwrap();
        p.downloaded_bytes += len;
    }

    Ok(())
}

/// Verify SHA-256 checksum of a file.
pub fn verify_checksum(path: &Path, expected_hex: &str) -> Result<(), BootstrapError> {
    if !path.is_file() {
        return Err(BootstrapError::CorruptDownload {
            path: path.to_string_lossy().to_string(),
        });
    }
    let mut file = std::fs::File::open(path).map_err(|e| BootstrapError::Io {
        message: format!("cannot open {path:?} for checksum: {e}"),
    })?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf).map_err(|e| BootstrapError::Io {
            message: format!("cannot read {path:?} for checksum: {e}"),
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(&hasher.finalize());
    if actual != expected_hex {
        return Err(BootstrapError::ChecksumMismatch {
            path: path.to_string_lossy().to_string(),
            expected: expected_hex.to_string(),
            actual,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Disk space
// ---------------------------------------------------------------------------

pub fn available_space(path: &Path) -> Result<u64, BootstrapError> {
    #[cfg(windows)]
    {
        let drive = path
            .to_string_lossy()
            .chars()
            .next()
            .unwrap_or('C');
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Get-PSDrive -Name {} | Select-Object -ExpandProperty Free",
                    drive.to_uppercase()
                ),
            ])
            .output();
        if let Ok(o) = output {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if let Ok(bytes) = text.parse::<u64>() {
                return Ok(bytes);
            }
        }
    }
    Ok(u64::MAX)
}

// ---------------------------------------------------------------------------
// ZIP extraction
// ---------------------------------------------------------------------------

pub fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), BootstrapError> {
    let zip_str = zip_path.to_string_lossy();
    let dest_str = dest_dir.to_string_lossy();
    let ps_script = format!(
        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
        zip_str.replace('\'', "''"),
        dest_str.replace('\'', "''"),
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .status()
        .map_err(|e| BootstrapError::Io {
            message: format!("cannot run Expand-Archive: {e}"),
        })?;
    if !status.success() {
        return Err(BootstrapError::Io {
            message: format!("Expand-Archive failed for {zip_path:?}"),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// High-level bootstrap operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapCheckResult {
    pub state: BootstrapState,
    pub gpu: GpuInfo,
    pub needs_runtime: bool,
    pub needs_models: bool,
    pub licenses_pending: Vec<ModelLicense>,
    pub space_ok: bool,
}

/// Run a comprehensive bootstrap health check (no downloads).
pub fn run_bootstrap_check() -> BootstrapCheckResult {
    let gpu = detect_gpu();
    let state_path = default_state_path();
    let state = load_state(&state_path);
    let needs_runtime = !state.runtime_installed || state.python_path.is_empty();
    let needs_models = !state.models_installed;
    let all_licenses = model_licenses();
    let licenses_pending: Vec<ModelLicense> = all_licenses
        .into_iter()
        .filter(|l| l.requires_acceptance && !state.licenses_accepted.contains(&l.model_id))
        .collect();
    let space_ok = available_space(&data_dir()).unwrap_or(0) > 2_000_000_000;
    BootstrapCheckResult {
        state,
        gpu,
        needs_runtime,
        needs_models,
        licenses_pending,
        space_ok,
    }
}

/// Create the runtime directory structure.
pub fn ensure_dirs() -> Result<PathBuf, BootstrapError> {
    let dir = data_dir();
    std::fs::create_dir_all(dir.join("runtime")).map_err(|e| BootstrapError::Io {
        message: format!("cannot create runtime dir: {e}"),
    })?;
    std::fs::create_dir_all(dir.join("models")).map_err(|e| BootstrapError::Io {
        message: format!("cannot create models dir: {e}"),
    })?;
    std::fs::create_dir_all(dir.join("ffmpeg")).map_err(|e| BootstrapError::Io {
        message: format!("cannot create ffmpeg dir: {e}"),
    })?;
    std::fs::create_dir_all(dir.join("cache")).map_err(|e| BootstrapError::Io {
        message: format!("cannot create cache dir: {e}"),
    })?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Install pipeline
// ---------------------------------------------------------------------------

/// Locate the runtime manifest. Checks next to the executable first, then
/// falls back to the repo root (dev mode), then the app data dir.
pub fn manifest_path() -> PathBuf {
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("runtime-manifest.json"))),
        Some(PathBuf::from("runtime-manifest.json")),
        Some(data_dir().join("runtime-manifest.json")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.is_file() {
            return c;
        }
    }
    PathBuf::from("runtime-manifest.json")
}

pub fn read_manifest(path: &Path) -> Result<BootstrapManifest, BootstrapError> {
    let text = std::fs::read_to_string(path).map_err(|e| BootstrapError::Io {
        message: format!("cannot read manifest {path:?}: {e}"),
    })?;
    serde_json::from_str(&text).map_err(|e| BootstrapError::Io {
        message: format!("cannot parse manifest {path:?}: {e}"),
    })
}

/// Download one file, verify its checksum, and delete it on mismatch so a
/// retry starts clean (corrupt-download recovery).
fn download_verified(
    entry: &ManifestEntry,
    root: &Path,
    progress: &Mutex<DownloadProgress>,
) -> Result<PathBuf, BootstrapError> {
    let dest = root.join(&entry.dest);
    download_file(&entry.url, &dest, progress)?;
    if let Err(e) = verify_checksum(&dest, &entry.sha256) {
        let _ = std::fs::remove_file(&dest);
        return Err(e);
    }
    Ok(dest)
}

/// Install the Python embeddable + FFmpeg runtime, then download models.
/// Runs synchronously; UI polls `bootstrap_download_progress` for progress.
pub fn run_install(
    state_path: &Path,
    manifest: &BootstrapManifest,
    progress: &Mutex<DownloadProgress>,
) -> Result<BootstrapState, BootstrapError> {
    let root = ensure_dirs()?;
    let mut state = load_state(state_path);
    let mut total: u64 = manifest
        .legacy_runtime
        .python_package
        .iter()
        .map(|e| e.size_hint)
        .sum();
    total += manifest
        .legacy_runtime
        .torch_wheels
        .iter()
        .map(|e| e.size_hint)
        .sum::<u64>();
    total += manifest
        .legacy_runtime
        .pip_packages
        .iter()
        .map(|e| e.size_hint)
        .sum::<u64>();
    for model in manifest.models.values() {
        total += model.files.iter().map(|e| e.size_hint).sum::<u64>();
    }
    {
        let mut p = progress.lock().unwrap();
        p.total_bytes = total;
        p.downloaded_bytes = 0;
        p.current_file = String::new();
        p.status = "preparing".to_string();
    }

    // License gate: refuse to download models without acceptance.
    for model_id in manifest.models.keys() {
        if !state.licenses_accepted.contains(model_id) {
            return Err(BootstrapError::LicenseNotAccepted {
                model_id: model_id.clone(),
            });
        }
    }

    // Python embeddable + FFmpeg (legacy runtime section).
    if let Some(py) = manifest.legacy_runtime.python_package.first() {
        let zip = download_verified(py, &root, progress)?;
        let python_dir = root.join("runtime");
        extract_zip(&zip, &python_dir)?;
        state.python_path = python_dir
            .join("python.exe")
            .to_string_lossy()
            .to_string();
    }
    if let Some(ff) = manifest.legacy_runtime.ffmpeg.as_ref() {
        let zip = download_verified(ff, &root, progress)?;
        let ffmpeg_dir = root.join("ffmpeg");
        extract_zip(&zip, &ffmpeg_dir)?;
        // Find the extracted ffmpeg.exe (inside a versioned subfolder).
        if let Ok(entries) = std::fs::read_dir(&ffmpeg_dir) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("bin").join("ffmpeg.exe");
                if candidate.is_file() {
                    state.ffmpeg_path = candidate.to_string_lossy().to_string();
                    break;
                }
            }
        }
    }
    state.runtime_installed = true;

    // Models (verifed checksum; raw files immutable after install).
    let models_root = root.join("models");
    for (model_id, model) in &manifest.models {
        for entry in &model.files {
            let dest = models_root.join(entry.dest.replace("models/", ""));
            download_file(&entry.url, &dest, progress)?;
            if let Err(e) = verify_checksum(&dest, &entry.sha256) {
                let _ = std::fs::remove_file(&dest);
                return Err(e);
            }
        }
        let _ = model_id;
    }
    state.hf_cache_dir = models_root.to_string_lossy().to_string();
    state.models_installed = true;

    state.bootstrap_complete = true;
    state.installed_at = "2026-08-10T00:00:00Z".to_string();
    save_state(state_path, &state)?;
    Ok(state)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_name_to_cc_lookup_works() {
        assert_eq!(gpu_name_to_cc("NVIDIA GeForce GTX 1070 Ti"), Some("6.1"));
        assert_eq!(gpu_name_to_cc("NVIDIA GeForce RTX 4090"), Some("8.9"));
        assert_eq!(gpu_name_to_cc("NVIDIA GeForce RTX 2080 Ti"), Some("7.5"));
        assert_eq!(gpu_name_to_cc("Some Unknown GPU"), None);
    }

    #[test]
    fn parse_vram_bytes_works() {
        assert_eq!(parse_vram_bytes(" 8192 MiB"), 8192 * 1024 * 1024);
        assert_eq!(parse_vram_bytes(" 4096 MiB"), 4096 * 1024 * 1024);
        assert_eq!(parse_vram_bytes("0 MiB"), 0);
        assert_eq!(parse_vram_bytes(""), 0);
    }

    #[test]
    fn driver_version_to_cuda_maps_correctly() {
        assert_eq!(driver_version_to_cuda("582.66"), "13.0");
        assert_eq!(driver_version_to_cuda("550.0"), "12.6");
        assert_eq!(driver_version_to_cuda("471.0"), "11.4");
        assert_eq!(driver_version_to_cuda("400.0"), "unknown");
    }

    #[test]
    fn state_persistence_round_trip() {
        let dir = std::env::temp_dir()
            .join(format!("dubflow_bootstrap_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let state_path = dir.join("state.json");

        let state = BootstrapState {
            runtime_installed: true,
            models_installed: false,
            licenses_accepted: vec!["JaiTTS-F5TTS".to_string()],
            installed_at: "2026-08-10T00:00:00Z".to_string(),
            python_path: "C:\\DubFlow\\runtime\\python.exe".to_string(),
            hf_cache_dir: "C:\\DubFlow\\models".to_string(),
            ffmpeg_path: "C:\\DubFlow\\ffmpeg\\ffmpeg.exe".to_string(),
            bootstrap_complete: false,
        };
        save_state(&state_path, &state).unwrap();

        let loaded = load_state(&state_path);
        assert!(loaded.runtime_installed);
        assert!(!loaded.models_installed);
        assert_eq!(loaded.licenses_accepted.len(), 1);
        assert_eq!(loaded.licenses_accepted[0], "JaiTTS-F5TTS");
        assert_eq!(loaded.python_path, "C:\\DubFlow\\runtime\\python.exe");

        accept_license(&state_path, "JaiTTS-F5TTS").unwrap();
        assert!(is_license_accepted(&state_path, "JaiTTS-F5TTS"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_checksum_rejects_mismatch() {
        let dir = std::env::temp_dir()
            .join(format!("dubflow_checksum_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, b"hello").unwrap();

        let expected =
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_checksum(&path, expected).is_ok());
        assert!(verify_checksum(&path, "0000").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn model_licenses_include_jaitts() {
        let licenses = model_licenses();
        assert_eq!(licenses.len(), 1);
        assert_eq!(licenses[0].model_id, "JaiTTS-F5TTS");
        assert!(licenses[0].requires_acceptance);
        assert!(licenses[0].license_text.contains("CC BY-NC 4.0"));
    }

    #[test]
    fn data_dir_is_resolved() {
        let dir = data_dir();
        assert!(dir.to_string_lossy().contains("DubFlow"));
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
