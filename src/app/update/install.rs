//! Installing an update in place: the download, checked against the
//! release's `SHA256SUMS` before anything is unpacked, then a swap of one
//! executable for another. Soba is one file (fonts and examples are
//! compiled in), so that's all an update is.
//!
//! Windows won't let a running `.exe` be overwritten, but it will let one be
//! renamed. So the running copy steps aside as `soba.old`, the new
//! one takes its name, and the old one stays there, with its version beside
//! it, to roll back to.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use semver::Version;

use super::net::{self, Progress};
use super::release::{self, Release, BINARY, CHECKSUMS, RELEASE_BUILD};
use super::signature;

/// The folder beside the executable that a download is unpacked in. On the
/// same volume, so the swap is a rename.
const STAGING: &str = ".soba-update";

/// The executable, as it was when the app started. Read once: after a swap
/// the OS may report the running file under its new name, `.old`.
pub fn exe_path() -> Option<&'static Path> {
    static EXE: OnceLock<Option<PathBuf>> = OnceLock::new();
    EXE.get_or_init(|| std::env::current_exe().ok().map(|p| std::fs::canonicalize(&p).map(simplify).unwrap_or(p)))
        .as_deref()
}

/// Windows' canonical paths start `\\?\`, which other programs read badly.
fn simplify(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(plain) if !plain.starts_with("UNC") => PathBuf::from(plain),
        _ => path,
    }
}

/// The previous version's executable, kept for rolling back.
fn backup_path(exe: &Path) -> PathBuf {
    exe.with_file_name(format!("{}.old", exe.file_stem().and_then(|s| s.to_str()).unwrap_or("soba")))
}

/// The file beside the backup that says which version it is.
fn backup_version_path(exe: &Path) -> PathBuf {
    let mut name = backup_path(exe).into_os_string();
    name.push(".version");
    PathBuf::from(name)
}

/// How this copy can be updated.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// Download, check and swap in place.
    InPlace,
    /// Only point at the release page, for the reason given.
    DownloadOnly(String),
}

/// Whether this copy can replace itself, or should send people to the
/// release page instead.
pub fn mode() -> Mode {
    if !RELEASE_BUILD {
        return Mode::DownloadOnly("This copy was built from source, so it doesn't replace itself.".into());
    }
    if release::own_asset().is_none() {
        return Mode::DownloadOnly("No release is built for this computer.".into());
    }
    let Some(exe) = exe_path() else {
        return Mode::DownloadOnly("Couldn't find where Soba is installed.".into());
    };
    if cfg!(target_os = "linux") && ["/usr/", "/snap/", "/nix/", "/app/"].iter().any(|p| exe.starts_with(p)) {
        return Mode::DownloadOnly("This copy was installed by a package manager, which updates it.".into());
    }
    if cfg!(target_os = "macos") && quarantined(exe) {
        return Mode::DownloadOnly("macOS doesn't let a downloaded, unsigned app replace itself.".into());
    }
    let folder = exe.parent().unwrap_or(Path::new("."));
    if !writable(folder) {
        return Mode::DownloadOnly(format!("Soba can't write to {}.", folder.display()));
    }
    Mode::InPlace
}

/// Whether a file can be made in `folder`.
fn writable(folder: &Path) -> bool {
    let probe = folder.join(format!(".soba-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Whether macOS has marked the file as downloaded, or is running it from a
/// translocated copy.
#[cfg(target_os = "macos")]
fn quarantined(exe: &Path) -> bool {
    exe.to_string_lossy().contains("/AppTranslocation/")
        || std::process::Command::new("xattr")
            .args(["-p", "com.apple.quarantine"])
            .arg(exe)
            .output()
            .is_ok_and(|out| out.status.success())
}

#[cfg(not(target_os = "macos"))]
fn quarantined(_exe: &Path) -> bool {
    false
}

/// A downloaded, checked update, unpacked beside the executable and ready
/// to swap in.
#[derive(Debug)]
pub struct Staged {
    pub version: Version,
    folder: PathBuf,
    binary: PathBuf,
    /// The text files beside the executable in the zip (an ASIO build's
    /// licence and notice), to copy beside it.
    extras: Vec<PathBuf>,
}

/// Downloads `release`'s zip for this build, checks it and unpacks it. On
/// any failure nothing outside the staging folder has changed.
pub fn stage(release: &Release, progress: &Progress) -> Result<Staged, String> {
    let exe = exe_path().ok_or("couldn't find where Soba is installed")?;
    let name = release::own_asset().ok_or("no release is built for this computer")?;
    let asset = release.asset(name).ok_or_else(|| format!("{} has no {name}", release.title))?;
    let sums = release
        .asset(CHECKSUMS)
        .ok_or_else(|| format!("{} has no {CHECKSUMS}, so its download can't be checked", release.title))?;
    let sums = net::fetch_text(&sums.browser_download_url)?;
    let expected = release::checksum_for(&sums, name).ok_or_else(|| format!("{CHECKSUMS} doesn't list {name}"))?;

    let folder = exe.parent().unwrap_or(Path::new(".")).join(STAGING);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).map_err(|e| format!("couldn't make {}: {e}", folder.display()))?;
    let zip = folder.join(name);
    let result = net::download(&asset.browser_download_url, &zip, asset.size, progress)
        .and_then(|actual| unpack(&zip, &expected, &actual, &folder));
    match result {
        Ok((binary, extras)) => {
            let _ = std::fs::remove_file(&zip);
            if let Err(e) = signature::check(exe, &binary) {
                let _ = std::fs::remove_dir_all(&folder);
                return Err(e);
            }
            Ok(Staged { version: release.version.clone(), folder, binary, extras })
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&folder);
            Err(e)
        }
    }
}

/// What a download that doesn't match its checksum is told.
pub const TAMPERED: &str = "The download didn't match the release's checksum, so it wasn't installed. It may have been damaged on the way.";

/// Checks a downloaded zip's SHA-256 against `expected`, and only then
/// unpacks it: the executable, and any text files beside it.
fn unpack(zip: &Path, expected: &str, actual: &str, into: &Path) -> Result<(PathBuf, Vec<PathBuf>), String> {
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(TAMPERED.into());
    }
    let file = std::fs::File::open(zip).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("the download isn't a zip: {e}"))?;
    let unpacked = into.join("new");
    std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;

    let (mut binary, mut extras) = (None, Vec::new());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("the zip is damaged: {e}"))?;
        // Only files at the top: nothing can be written outside the folder
        let Some(name) = entry.enclosed_name().filter(|p| p.components().count() == 1) else { continue };
        if entry.is_dir() {
            continue;
        }
        let is_binary = name == Path::new(BINARY);
        if !is_binary && name.extension().is_none_or(|ext| ext != "txt") {
            continue;
        }
        let dest = unpacked.join(&name);
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("couldn't unpack {}: {e}", name.display()))?;
        if is_binary {
            binary = Some(dest);
        } else {
            extras.push(dest);
        }
    }
    let binary = binary.ok_or_else(|| format!("the download has no {BINARY}"))?;
    // Zips made on Linux from CI artifacts lose the executable bit
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    Ok((binary, extras))
}

/// Puts the staged version in the running one's place, keeping the running
/// one as `.old`. If the new one can't be moved in, the old one goes back.
pub fn swap(staged: &Staged) -> Result<(), String> {
    let exe = exe_path().ok_or("couldn't find where Soba is installed")?;
    swap_at(exe, staged, release::CURRENT)
}

fn swap_at(exe: &Path, staged: &Staged, current: &str) -> Result<(), String> {
    let backup = backup_path(exe);
    match std::fs::remove_file(&backup) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("couldn't remove the previous backup, {}: {e}", backup.display())),
    }
    std::fs::rename(exe, &backup).map_err(|e| format!("couldn't move {} aside: {e}", exe.display()))?;
    if let Err(e) = std::fs::rename(&staged.binary, exe) {
        let _ = std::fs::rename(&backup, exe);
        return Err(format!("couldn't put the new version in place: {e}"));
    }
    let _ = std::fs::write(backup_version_path(exe), current);
    let folder = exe.parent().unwrap_or(Path::new("."));
    for extra in &staged.extras {
        if let Some(name) = extra.file_name() {
            let _ = std::fs::copy(extra, folder.join(name));
        }
    }
    let _ = std::fs::remove_dir_all(&staged.folder);
    Ok(())
}

/// The version kept from before the last update, if there is one to go
/// back to.
pub fn rollback_version() -> Option<Version> {
    let exe = exe_path()?;
    backup_version(exe)
}

fn backup_version(exe: &Path) -> Option<Version> {
    if !backup_path(exe).is_file() {
        return None;
    }
    release::parse_version(&std::fs::read_to_string(backup_version_path(exe)).ok()?)
}

/// Swaps the running version and the kept one, so the next start is the
/// kept one. Returns the version rolled back to.
pub fn roll_back() -> Result<Version, String> {
    let exe = exe_path().ok_or("couldn't find where Soba is installed")?;
    roll_back_at(exe, release::CURRENT)
}

fn roll_back_at(exe: &Path, current: &str) -> Result<Version, String> {
    let version = backup_version(exe).ok_or("there's no earlier version kept")?;
    let backup = backup_path(exe);
    let aside = exe.with_extension("rollback");
    let _ = std::fs::remove_file(&aside);
    std::fs::rename(exe, &aside).map_err(|e| format!("couldn't move {} aside: {e}", exe.display()))?;
    if let Err(e) = std::fs::rename(&backup, exe) {
        let _ = std::fs::rename(&aside, exe);
        return Err(format!("couldn't put {version} back: {e}"));
    }
    // What was running is kept in its turn, so the roll back can be undone
    let _ = std::fs::rename(&aside, &backup);
    let _ = std::fs::write(backup_version_path(exe), current);
    Ok(version)
}

/// Clears what an interrupted update left: a half-finished download, or a
/// roll back's file in passing.
pub fn clean_up() {
    let Some(exe) = exe_path() else { return };
    let staging = exe.parent().unwrap_or(Path::new(".")).join(STAGING);
    if staging.exists() {
        let _ = std::fs::remove_dir_all(staging);
    }
    let aside = exe.with_extension("rollback");
    if aside.exists() {
        let _ = std::fs::remove_file(aside);
    }
}

/// Starts the executable (the new version, after a swap) with `args`.
pub fn relaunch(args: &[std::ffi::OsString]) -> Result<(), String> {
    let exe = exe_path().ok_or("couldn't find where Soba is installed")?;
    std::process::Command::new(exe)
        .args(args)
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .spawn()
        .map(drop)
        .map_err(|e| format!("couldn't start {}: {e}", exe.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("soba-install-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn a_tampered_download_is_refused_before_it_is_unpacked() {
        let dir = scratch("tamper");
        let zip = dir.join("release.zip");
        make_zip(&zip, &[(BINARY, b"new version"), ("README-LOW-LATENCY.txt", b"notice")]);
        let good = net::sha256_file(&zip).unwrap();

        // One byte changed
        let mut bytes = std::fs::read(&zip).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0x01;
        std::fs::write(&zip, &bytes).unwrap();
        let actual = net::sha256_file(&zip).unwrap();
        assert_ne!(actual, good);
        assert_eq!(unpack(&zip, &good, &actual, &dir).unwrap_err(), TAMPERED);
        assert!(!dir.join("new").exists(), "nothing was unpacked");
    }

    #[test]
    fn a_good_download_unpacks_only_the_app_and_its_notes() {
        let dir = scratch("unpack");
        let zip = dir.join("release.zip");
        make_zip(
            &zip,
            &[(BINARY, b"new version"), ("GPL-3.0.txt", b"licence"), ("../escape.txt", b"no"), ("sub/inner.txt", b"no"), ("other.dll", b"no")],
        );
        let hash = net::sha256_file(&zip).unwrap();
        let (binary, extras) = unpack(&zip, &hash.to_uppercase(), &hash, &dir).unwrap();
        assert_eq!(std::fs::read(&binary).unwrap(), b"new version");
        let names: Vec<_> = extras.iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_string()).collect();
        assert_eq!(names, ["GPL-3.0.txt"]);
        assert!(!dir.join("escape.txt").exists());

        let empty = dir.join("empty.zip");
        make_zip(&empty, &[("notes.txt", b"")]);
        let hash = net::sha256_file(&empty).unwrap();
        assert!(unpack(&empty, &hash, &hash, &dir).unwrap_err().contains("has no"));
    }

    #[test]
    fn swapping_keeps_the_old_version_and_rolling_back_swaps_again() {
        let dir = scratch("swap");
        let exe = dir.join(BINARY);
        std::fs::write(&exe, b"0.3.1").unwrap();
        let staging = dir.join(STAGING);
        std::fs::create_dir_all(staging.join("new")).unwrap();
        let binary = staging.join("new").join(BINARY);
        std::fs::write(&binary, b"0.4.0").unwrap();
        let notice = staging.join("new").join("NOTICE.txt");
        std::fs::write(&notice, b"notice").unwrap();

        let staged = Staged { version: Version::new(0, 4, 0), folder: staging.clone(), binary, extras: vec![notice] };
        swap_at(&exe, &staged, "0.3.1").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"0.4.0");
        assert_eq!(std::fs::read(backup_path(&exe)).unwrap(), b"0.3.1");
        assert_eq!(backup_path(&exe).file_name().unwrap(), "soba.old");
        assert_eq!(backup_version(&exe), Some(Version::new(0, 3, 1)));
        assert!(dir.join("NOTICE.txt").exists());
        assert!(!staging.exists());

        // Back to 0.3.1, keeping 0.4.0 to go forward to
        assert_eq!(roll_back_at(&exe, "0.4.0").unwrap(), Version::new(0, 3, 1));
        assert_eq!(std::fs::read(&exe).unwrap(), b"0.3.1");
        assert_eq!(backup_version(&exe), Some(Version::new(0, 4, 0)));

        std::fs::remove_file(backup_path(&exe)).unwrap();
        assert_eq!(backup_version(&exe), None);
        assert!(roll_back_at(&exe, "0.3.1").is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"0.3.1", "a failed roll back changes nothing");
    }

    #[test]
    fn source_builds_never_replace_themselves() {
        if !RELEASE_BUILD {
            assert!(matches!(mode(), Mode::DownloadOnly(_)));
        }
    }
}
