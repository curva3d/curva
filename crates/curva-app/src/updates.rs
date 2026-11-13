//! Updating Curva (spec `docs/specs/updates-and-welcome.md`): on opening, ask GitHub for the
//! newest release; if it is newer, offer it in the command bar; when asked, download the file for
//! this system, check it against the release's checksums, install it and open Curva again.
//!
//! The network is reached through the system's own `curl`, which Windows 10 and later, macOS and
//! Linux all have, rather than a TLS library: that keeps Curva's licences and build simple.

use std::path::{Path, PathBuf};

/// Where Curva's releases are listed.
const RELEASES: &str = "https://api.github.com/repos/curva3d/curva/releases?per_page=10";
/// The page for one release's notes.
pub const NOTES: &str = "https://github.com/curva3d/curva/releases/tag/v";
/// The file listing every release file's SHA-256, which the release workflow publishes.
const CHECKSUMS: &str = "SHA256SUMS.txt";

/// A version as three numbers.
pub type Version = (u32, u32, u32);

/// A version written `0.4.0` or `v0.4.0`, if it is one.
#[must_use]
pub fn parse_version(text: &str) -> Option<Version> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut number = || {
        parts
            .next()?
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()
    };
    Some((number()?, number()?, number()?))
}

/// The version running.
#[must_use]
pub fn running() -> Version {
    parse_version(env!("CARGO_PKG_VERSION")).unwrap_or((0, 0, 0))
}

/// A release newer than the one running, and its files.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Its version.
    pub version: Version,
    /// Its tag, `v0.4.0`.
    pub tag: String,
    /// Each file's name and download address.
    pub files: Vec<(String, String)>,
}

impl Release {
    /// The version as text, `0.4.0`.
    #[must_use]
    pub fn version_text(&self) -> String {
        let (a, b, c) = self.version;
        format!("{a}.{b}.{c}")
    }

    /// The file for this system, if the release has one.
    #[must_use]
    pub fn file_for_this_system(&self) -> Option<&(String, String)> {
        let ending = system_ending();
        self.files.iter().find(|(name, _)| name.ends_with(ending))
    }
}

/// How this system's release file ends.
#[must_use]
pub fn system_ending() -> &'static str {
    if cfg!(target_os = "windows") {
        "-windows-x64-setup.exe"
    } else if cfg!(target_os = "macos") {
        ".dmg"
    } else {
        ".AppImage"
    }
}

/// The newest release in GitHub's list (as JSON), if it is newer than `than`.
#[must_use]
pub fn newest(list: &serde_json::Value, than: Version) -> Option<Release> {
    let newest = list
        .as_array()?
        .iter()
        .filter(|release| !release["draft"].as_bool().unwrap_or(false))
        .filter_map(|release| {
            let tag = release["tag_name"].as_str()?.to_owned();
            let version = parse_version(&tag)?;
            let files = release["assets"]
                .as_array()?
                .iter()
                .filter_map(|asset| {
                    Some((
                        asset["name"].as_str()?.to_owned(),
                        asset["browser_download_url"].as_str()?.to_owned(),
                    ))
                })
                .collect();
            Some(Release {
                version,
                tag,
                files,
            })
        })
        .max_by_key(|release| release.version)?;
    (newest.version > than).then_some(newest)
}

/// Fetches `url` with the system's curl.
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let output = crate::quiet_command("curl")
        .args(["-fsSL", "--max-time", "20", "-H", "User-Agent: Curva", url])
        .output()
        .map_err(|error| format!("curl could not run: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// Asks GitHub for a release newer than the one running. Network trouble means none.
#[must_use]
pub fn check() -> Option<Release> {
    let body = fetch(RELEASES).ok()?;
    let list: serde_json::Value = serde_json::from_slice(&body).ok()?;
    newest(&list, running())
}

/// The SHA-256 a checksums file gives for `name`.
#[must_use]
pub fn listed_checksum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let sum = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        (file == name).then(|| sum.to_lowercase())
    })
}

/// The SHA-256 of some bytes, as lowercase hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Downloads this system's file of `release` into the temporary folder and checks it against
/// the release's checksums. Returns where it is.
pub fn download(release: &Release) -> Result<PathBuf, String> {
    let (name, url) = release
        .file_for_this_system()
        .ok_or_else(|| "this release has no file for this system".to_owned())?;
    let bytes = fetch(url)?;
    let sums_url = release
        .files
        .iter()
        .find(|(file, _)| file == CHECKSUMS)
        .map(|(_, url)| url.clone())
        .ok_or_else(|| "the release lists no checksums".to_owned())?;
    let sums = String::from_utf8_lossy(&fetch(&sums_url)?).into_owned();
    let expected = listed_checksum(&sums, name).ok_or_else(|| format!("no checksum for {name}"))?;
    if sha256(&bytes) != expected {
        return Err("the download does not match its checksum".to_owned());
    }
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

/// Installs a downloaded release and opens Curva again (with `reopen`, the model that was open),
/// then returns so the running Curva can close.
///
/// - Windows: runs the installer silently in the background, which then starts Curva.
/// - Linux: puts the new AppImage in place of the running one and starts it.
/// - macOS: opens the disk image for the user to drag Curva to Applications (an unsigned app
///   cannot replace itself), and leaves a small helper that opens the new copy once it is there.
pub fn install(file: &Path, version: &str, reopen: Option<&Path>) -> Result<(), String> {
    let model = reopen.map(|p| p.display().to_string()).unwrap_or_default();
    if cfg!(target_os = "windows") {
        let folder = std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
        let exe = format!("{folder}\\Programs\\Curva\\curva.exe");
        // Waits a moment for this Curva to close, installs silently, then opens Curva again.
        let script = format!(
            "Start-Sleep -Seconds 2; Start-Process -Wait -FilePath '{}' -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART'; Start-Process -FilePath '{exe}' {}",
            file.display(),
            if model.is_empty() {
                String::new()
            } else {
                format!("-ArgumentList '\"{model}\"'")
            },
        );
        crate::quiet_command("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    if cfg!(target_os = "macos") {
        crate::quiet_command("open")
            .arg(file)
            .spawn()
            .map_err(|e| e.to_string())?;
        // Waits up to ten minutes for the new Curva in Applications, then opens it.
        let script = format!(
            "for i in $(seq 1 300); do v=$(defaults read /Applications/Curva.app/Contents/Info CFBundleShortVersionString 2>/dev/null); if [ \"$v\" = \"{version}\" ]; then open -a /Applications/Curva.app {}; exit 0; fi; sleep 2; done",
            if model.is_empty() {
                String::new()
            } else {
                format!("--args '{model}'")
            },
        );
        crate::quiet_command("sh")
            .args(["-c", &script])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    // Linux: the running AppImage is named by APPIMAGE.
    let running = std::env::var("APPIMAGE")
        .map_err(|_| "Curva is not running from an AppImage".to_owned())?;
    make_executable(file)?;
    std::fs::copy(file, format!("{running}.new")).map_err(|e| e.to_string())?;
    std::fs::rename(format!("{running}.new"), &running).map_err(|e| e.to_string())?;
    let mut command = crate::quiet_command(&running);
    if !model.is_empty() {
        command.arg(&model);
    }
    command.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Lets a file be run, on systems with Unix permissions.
#[cfg(unix)]
fn make_executable(file: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(file)
        .map_err(|e| e.to_string())?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(file, permissions).map_err(|e| e.to_string())
}

/// Files can be run as they are on Windows.
#[cfg(not(unix))]
fn make_executable(_file: &Path) -> Result<(), String> {
    Ok(())
}

/// The version Curva last ran as, kept beside the layout, so a newer one can say it was updated.
fn last_run_file() -> Option<PathBuf> {
    Some(crate::recovery::app_folder()?.join("last-version.txt"))
}

/// If Curva was updated since it last ran, the version it is now; records this run either way.
#[must_use]
pub fn just_updated() -> Option<Version> {
    let file = last_run_file()?;
    let before = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| parse_version(&t));
    let now = running();
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let _ = std::fs::write(&file, env!("CARGO_PKG_VERSION"));
    before.filter(|b| *b < now).map(|_| now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_and_the_newest_release_is_found() {
        assert_eq!(parse_version("v0.4.0"), Some((0, 4, 0)));
        assert_eq!(parse_version("0.10.2"), Some((0, 10, 2)));
        assert!(parse_version("0.10.2").unwrap() > parse_version("0.9.9").unwrap());
        let list = serde_json::json!([
            {"tag_name": "v0.3.0", "draft": false, "assets": []},
            {"tag_name": "v0.4.0", "draft": false, "assets": [
                {"name": "curva-v0.4.0-windows-x64-setup.exe", "browser_download_url": "w"},
                {"name": "curva-v0.4.0-linux-x86_64.AppImage", "browser_download_url": "l"},
                {"name": "curva-v0.4.0-macos-arm64.dmg", "browser_download_url": "m"},
                {"name": "SHA256SUMS.txt", "browser_download_url": "s"}
            ]},
            {"tag_name": "v0.5.0", "draft": true, "assets": []}
        ]);
        let release = newest(&list, (0, 3, 0)).unwrap();
        assert_eq!(release.version, (0, 4, 0));
        assert!(release.file_for_this_system().is_some());
        assert!(newest(&list, (0, 4, 0)).is_none());
    }

    #[test]
    fn checksums_are_read_and_checked() {
        let sums = "abc123  curva-v0.4.0-macos-arm64.dmg\nDEF456 *SHA256SUMS.txt\n";
        assert_eq!(
            listed_checksum(sums, "curva-v0.4.0-macos-arm64.dmg").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            listed_checksum(sums, "SHA256SUMS.txt").as_deref(),
            Some("def456")
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
