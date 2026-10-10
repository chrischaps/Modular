//! What GitHub says about Modular's releases, and which of them matter to
//! this copy: the ones newer than it, and the download built like it.
//!
//! Everything here is plain data, so it's tested without a network.

use semver::Version;
use serde::{Deserialize, Serialize};

/// The version this copy is.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Where releases are published.
pub const REPO: &str = "chrischaps/Modular";

/// Set by `release.yml` when it builds a release. A copy built from source
/// (`cargo run`) only says an update exists; it never replaces itself.
pub const RELEASE_BUILD: bool = option_env!("MODULAR_RELEASE_BUILD").is_some();

/// Which of the two Windows downloads this is: `asio` includes Steinberg's
/// ASIO SDK (and is GPLv3), `standard` doesn't. Read from the build's own
/// features, so it can't disagree with what's inside.
pub const BUILD_FLAVOR: &str = if cfg!(feature = "asio") { "asio" } else { "standard" };

/// The file holding every asset's SHA-256, published with each release.
pub const CHECKSUMS: &str = "SHA256SUMS";

/// The release download built like this copy (platform and flavour), as
/// `release.yml` names it. `None` where no release is built.
pub fn own_asset() -> Option<&'static str> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some(if cfg!(feature = "asio") { "modular_synth-windows-asio.zip" } else { "modular_synth-windows.zip" })
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("modular_synth-macos-apple-silicon.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("modular_synth-macos-intel.zip")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("modular_synth-linux.zip")
    } else {
        None
    }
}

/// The executable's name inside a release zip.
pub const BINARY: &str = if cfg!(windows) { "modular_synth.exe" } else { "modular_synth" };

/// A version from a tag: `v0.3.0` or `0.3.0`.
pub fn parse_version(tag: &str) -> Option<Version> {
    let tag = tag.trim();
    Version::parse(tag.strip_prefix('v').or_else(|| tag.strip_prefix('V')).unwrap_or(tag)).ok()
}

/// This copy's version.
pub fn current() -> Version {
    parse_version(CURRENT).expect("Cargo.toml's version is semver")
}

/// One file attached to a release.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

/// A release as the API lists it, with only the fields used.
#[derive(Clone, Debug, Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// A published release. Kept between launches, so a known update is still
/// offered the next day without asking GitHub again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub version: Version,
    /// Its title, e.g. "v0.3.1 — a low-latency Windows download".
    pub title: String,
    /// The notes, in Markdown.
    pub notes: String,
    /// The release's page, for the Download button.
    pub page: String,
    /// The day it went out, `yyyy-mm-dd`.
    pub date: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// The title without the version it repeats: "a low-latency Windows
    /// download". Empty when the title is only the version.
    pub fn subtitle(&self) -> &str {
        let title = self.title.trim();
        let rest = title
            .strip_prefix('v')
            .unwrap_or(title)
            .strip_prefix(&self.version.to_string())
            .map(|rest| rest.trim_start_matches(|c: char| c.is_whitespace() || "—–-:·".contains(c)));
        match rest {
            Some(rest) => rest,
            None => title,
        }
    }
}

/// Why a response couldn't be read.
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The releases in a `GET /repos/{repo}/releases` response, newest first.
/// Drafts, pre-releases and tags that aren't versions are left out.
pub fn parse_releases(json: &str) -> Result<Vec<Release>, ParseError> {
    let listed: Vec<ApiRelease> = serde_json::from_str(json).map_err(|e| ParseError(format!("not a release list: {e}")))?;
    Ok(published(listed))
}

/// The release in a `GET /repos/{repo}/releases/latest` response.
pub fn parse_latest(json: &str) -> Result<Release, ParseError> {
    let release: ApiRelease = serde_json::from_str(json).map_err(|e| ParseError(format!("not a release: {e}")))?;
    published(vec![release]).pop().ok_or_else(|| ParseError("not a published version".into()))
}

fn published(listed: Vec<ApiRelease>) -> Vec<Release> {
    let mut releases: Vec<Release> = listed
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let version = parse_version(&r.tag_name)?;
            // A pre-release version tagged as a full release is still a pre-release
            if !version.pre.is_empty() {
                return None;
            }
            Some(Release {
                title: r.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| r.tag_name.clone()),
                notes: r.body.unwrap_or_default().replace("\r\n", "\n"),
                page: r.html_url,
                date: r.published_at.map(|d| d.chars().take(10).collect()).unwrap_or_default(),
                version,
                assets: r.assets,
            })
        })
        .collect();
    releases.sort_by(|a, b| b.version.cmp(&a.version));
    releases.dedup_by(|a, b| a.version == b.version);
    releases
}

/// What's come out since `installed`, newest first: the latest is the
/// first, and every release between is there for its notes.
pub fn newer_than(releases: &[Release], installed: &Version) -> Vec<Release> {
    releases.iter().filter(|r| r.version > *installed).cloned().collect()
}

/// The SHA-256 a `SHA256SUMS` file gives `name`, in lower-case hex. Lines
/// are `sha256sum`'s: `<hex>  <name>`, or `<hex> *<name>` in binary mode.
pub fn checksum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        parse_version(s).unwrap()
    }

    #[test]
    fn versions_order_as_semver_with_or_without_a_v() {
        assert!(v("0.2.0") < v("0.2.1"));
        assert!(v("0.2.1") < v("0.3.0-beta.1"));
        assert!(v("0.3.0-beta.1") < v("0.3.0"));
        assert!(v("0.3.0") < v("0.10.0"), "numeric, not alphabetical");
        assert_eq!(v("v0.3.1"), v("0.3.1"));
        assert_eq!(v(" V1.0.0 "), v("1.0.0"));
        assert!(parse_version("latest").is_none());
        assert!(parse_version("v0.3").is_none());
        assert_eq!(current().to_string(), CURRENT);
    }

    /// Trimmed from the real `releases` response, plus a draft and a
    /// pre-release that must be passed over.
    const RELEASES: &str = include_str!("testdata/releases.json");

    #[test]
    fn the_release_list_parses_newest_first_without_drafts_or_betas() {
        let releases = parse_releases(RELEASES).unwrap();
        let versions: Vec<String> = releases.iter().map(|r| r.version.to_string()).collect();
        assert_eq!(versions, ["0.3.1", "0.3.0", "0.2.0", "0.1.0"]);

        let latest = &releases[0];
        assert_eq!(latest.title, "v0.3.1 — a low-latency Windows download");
        assert_eq!(latest.subtitle(), "a low-latency Windows download");
        assert_eq!(latest.date, "2026-10-09");
        assert_eq!(latest.page, "https://github.com/chrischaps/Modular/releases/tag/v0.3.1");
        assert!(latest.notes.contains("**Licence:**"));
        assert!(!latest.notes.contains('\r'));
        let windows = latest.asset("modular_synth-windows.zip").expect("the Windows zip");
        assert!(windows.browser_download_url.ends_with("/v0.3.1/modular_synth-windows.zip"));
        assert!(latest.asset("modular_synth-windows-asio.zip").is_some());

        // v0.1.0's title is only its tag
        assert_eq!(releases[3].subtitle(), "");
    }

    #[test]
    fn notes_cover_every_version_since_the_installed_one() {
        let releases = parse_releases(RELEASES).unwrap();
        let since: Vec<String> = newer_than(&releases, &v("0.2.0")).iter().map(|r| r.version.to_string()).collect();
        assert_eq!(since, ["0.3.1", "0.3.0"]);
        assert!(newer_than(&releases, &v("0.3.1")).is_empty());
        assert!(newer_than(&releases, &v("0.4.0-beta.1")).is_empty());
    }

    #[test]
    fn the_latest_release_parses_alone() {
        let latest = include_str!("testdata/latest.json");
        let release = parse_latest(latest).unwrap();
        assert_eq!(release.version, v("0.3.1"));
        assert_eq!(release.assets.len(), 2);
    }

    #[test]
    fn strange_responses_are_errors_not_panics() {
        for json in [
            "",
            "{",
            "null",
            r#"{"message":"API rate limit exceeded for 1.2.3.4.","documentation_url":"https://docs.github.com"}"#,
            r#"[{"tag_name": 3}]"#,
            "<html>Bad gateway</html>",
        ] {
            assert!(parse_releases(json).is_err(), "{json:?}");
            assert!(parse_latest(json).is_err(), "{json:?}");
        }
        // Well-formed, but nothing in it is a version
        assert!(parse_releases(r#"[{"tag_name":"nightly"}]"#).unwrap().is_empty());
        assert!(parse_latest(r#"{"tag_name":"v0.4.0","draft":true}"#).is_err());
    }

    #[test]
    fn checksums_are_found_by_file_name() {
        let a = "a".repeat(64);
        let b = "B".repeat(64);
        let sums = format!("{a}  modular_synth-windows.zip\n{b} *modular_synth-windows-asio.zip\nnonsense\n");
        assert_eq!(checksum_for(&sums, "modular_synth-windows.zip"), Some(a));
        assert_eq!(checksum_for(&sums, "modular_synth-windows-asio.zip"), Some("b".repeat(64)));
        assert_eq!(checksum_for(&sums, "modular_synth-linux.zip"), None);
        assert_eq!(checksum_for("abc  modular_synth-linux.zip", "modular_synth-linux.zip"), None, "not a SHA-256");
    }

    #[test]
    fn each_build_installs_its_own_flavour() {
        if cfg!(all(windows, target_arch = "x86_64")) {
            let asset = own_asset().unwrap();
            assert_eq!(asset.contains("asio"), cfg!(feature = "asio"));
            assert_eq!(BUILD_FLAVOR == "asio", cfg!(feature = "asio"));
        }
    }
}
