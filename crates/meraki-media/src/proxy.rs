use crate::{command, probe, Asset, MediaResult, Tools};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::UNIX_EPOCH,
};

static ENABLED: AtomicBool = AtomicBool::new(false);
static REGISTRY: OnceLock<Mutex<BTreeMap<PathBuf, (Asset, Asset)>>> = OnceLock::new();
fn registry() -> &'static Mutex<BTreeMap<PathBuf, (Asset, Asset)>> {
    REGISTRY.get_or_init(Default::default)
}
pub fn enable_proxies(enabled: bool) {
    ENABLED.store(enabled, Ordering::Release);
}
pub fn proxies_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}
/// Called exclusively by the preview compositor. Offline render/audio retain the original catalog.
pub fn preview_asset(original: &Asset) -> Asset {
    if proxies_enabled() {
        if let Some((source, proxy)) = registry().lock().unwrap().get(&original.path) {
            if source == original && proxy.path.is_file() {
                return proxy.clone();
            }
        }
    }
    original.clone()
}
pub fn proxy_key(asset: &Asset) -> MediaResult<String> {
    let stat = std::fs::metadata(&asset.path).map_err(|e| e.to_string())?;
    let stamp = stat
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    // Stable cache key, not a security hash. Manifest metadata is checked separately.
    let signature = format!("v1|{}|{}|{}", asset.path.display(), stat.len(), stamp);
    let hash = signature.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    Ok(format!("{hash:016x}"))
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    source: Asset,
    proxy: Asset,
}
pub fn load_proxy(asset: &Asset, directory: &Path) -> MediaResult<bool> {
    let key = proxy_key(asset)?;
    let path = directory.join(format!("{key}.json"));
    if !path.is_file() {
        registry().lock().unwrap().remove(&asset.path);
        return Ok(false);
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Manifesto de proxy inválido".into());
    }
    let mut m: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    // IDs are project-local; source identity is the path + current file stamp/cache key.
    m.source.id = asset.id;
    m.proxy.id = asset.id;
    if m.source != *asset || !m.proxy.path.is_file() {
        registry().lock().unwrap().remove(&asset.path);
        return Ok(false);
    }
    m.proxy.validate()?;
    registry()
        .lock()
        .unwrap()
        .insert(asset.path.clone(), (asset.clone(), m.proxy));
    Ok(true)
}
/// Blocking transcode: invoke on a dedicated bounded background worker.
pub fn generate_proxy(asset: &Asset, directory: &Path, stop: &AtomicBool) -> MediaResult<()> {
    if asset.audio_only {
        return Err("Proxy requer vídeo".into());
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    if load_proxy(asset, directory)? {
        return Ok(());
    }
    let key = proxy_key(asset)?;
    let output = directory.join(format!("{key}.mp4"));
    let temporary = directory.join(format!("{key}.{}.part.mp4", std::process::id()));
    let tools = Tools::discover();
    let mut child=command(&tools.ffmpeg).args(["-hide_banner","-loglevel","error","-nostdin","-y","-protocol_whitelist","file,pipe","-i"])
        .arg(&asset.path).args(["-map",&format!("0:{}",asset.stream_index),"-an","-sn","-dn","-vf","scale=1280:720:force_original_aspect_ratio=decrease:force_divisible_by=2,setsar=1,setpts=PTS-STARTPTS","-fps_mode","passthrough","-enc_time_base:v","demux","-c:v","libx264","-preset","veryfast","-crf","20","-g","1","-pix_fmt","yuv420p","-movflags","+faststart"])
        .arg(&temporary).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e|e.to_string())?;
    let result = (|| {
        loop {
            if stop.load(Ordering::Acquire) {
                return Err("Proxy cancelado".into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err("FFmpeg não conseguiu gerar o proxy".into());
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if key != proxy_key(asset)? {
            return Err("A fonte mudou durante a geração do proxy".into());
        }
        let mut proxy = probe(&tools, &temporary, asset.id)?;
        let delta = (proxy.duration.numerator() as f64 / proxy.duration.denominator() as f64
            - asset.duration.numerator() as f64 / asset.duration.denominator() as f64)
            .abs();
        if delta > 0.15 {
            return Err("Duração do proxy difere da mídia original".into());
        }
        std::fs::rename(&temporary, &output).map_err(|e| e.to_string())?;
        proxy.path = output.canonicalize().map_err(|e| e.to_string())?;
        let manifest = Manifest {
            source: asset.clone(),
            proxy: proxy.clone(),
        };
        let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
        let manifest_tmp = directory.join(format!("{key}.part.json"));
        std::fs::write(&manifest_tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(manifest_tmp, directory.join(format!("{key}.json")))
            .map_err(|e| e.to_string())?;
        registry()
            .lock()
            .unwrap()
            .insert(asset.path.clone(), (asset.clone(), proxy));
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(temporary);
    result
}
