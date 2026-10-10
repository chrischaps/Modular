//! The two kinds of request the updater makes: one for the list of releases,
//! and one per download. Each says which app is asking (`User-Agent:
//! modular_synth/<version>`, which GitHub requires) and nothing else: no
//! identifiers, no telemetry. They block, so they run on worker threads.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::release::{self, Release, REPO};

/// Overrides where releases are listed (a full `.../releases` API URL), to
/// try an update against a fork or a local server.
pub const URL_OVERRIDE: &str = "MODULAR_UPDATE_URL";

/// The releases list to ask for.
pub fn releases_url() -> String {
    std::env::var(URL_OVERRIDE)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| format!("https://api.github.com/repos/{REPO}/releases?per_page=20"))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("modular_synth/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Why a request didn't work, in words for the toast.
fn describe(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(403 | 429, _) => "GitHub is limiting requests from this network; try again in an hour".into(),
        ureq::Error::Status(404, _) => "the releases weren't found".into(),
        ureq::Error::Status(code, _) => format!("GitHub answered {code}"),
        ureq::Error::Transport(t) => match t.kind() {
            ureq::ErrorKind::Dns | ureq::ErrorKind::ConnectionFailed => "couldn't reach GitHub (offline?)".into(),
            _ => t.to_string(),
        },
    }
}

/// Every published release, newest first.
pub fn fetch_releases() -> Result<Vec<Release>, String> {
    let url = releases_url();
    let body = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(describe)?
        .into_string()
        .map_err(|e| format!("the answer was cut off: {e}"))?;
    release::parse_releases(&body).map_err(|e| e.to_string())
}

/// A small text file, such as `SHA256SUMS`.
pub fn fetch_text(url: &str) -> Result<String, String> {
    agent().get(url).call().map_err(describe)?.into_string().map_err(|e| e.to_string())
}

/// How far a download has got, shared with the UI.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    /// 0 until the size is known.
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

impl Progress {
    /// From 0 to 1, if the size is known.
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.done.load(Ordering::Relaxed) as f64 / total as f64).min(1.0) as f32)
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// The message a cancelled download ends with.
pub const CANCELLED: &str = "cancelled";

/// Downloads `url` into `dest`, returning its SHA-256 in lower-case hex.
/// `size` is what the release says, used when the server doesn't.
pub fn download(url: &str, dest: &Path, size: u64, progress: &Progress) -> Result<String, String> {
    let response = agent().get(url).call().map_err(describe)?;
    let total = response.header("Content-Length").and_then(|l| l.parse().ok()).unwrap_or(size);
    progress.total.store(total, Ordering::Relaxed);

    let mut file = std::fs::File::create(dest).map_err(|e| format!("couldn't write {}: {e}", dest.display()))?;
    let mut reader = response.into_reader();
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        if progress.cancelled() {
            return Err(CANCELLED.into());
        }
        let n = reader.read(&mut buffer).map_err(|e| format!("the download broke off: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        file.write_all(&buffer[..n]).map_err(|e| format!("couldn't write the download: {e}"))?;
        progress.done.fetch_add(n as u64, Ordering::Relaxed);
    }
    file.sync_all().map_err(|e| format!("couldn't write the download: {e}"))?;
    Ok(hex(&hasher.finalize()))
}

/// A file's SHA-256, in lower-case hex.
#[cfg(test)]
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_sha256sum() {
        let path = std::env::temp_dir().join("modular-sha256-test.txt");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(sha256_file(&path).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let _ = std::fs::remove_file(path);
    }
}
