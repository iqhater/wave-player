use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use semver::Version;
use sha2::{Digest, Sha256};

use crate::shared::{APP_BUILD, APP_VERSION};

/// Public GitHub repository that hosts the releases.
const GITHUB_OWNER: &str = "iqhater";
const GITHUB_REPO: &str = "wave_player";
const USER_AGENT: &str = "wave-player-updater";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    /// Release tag, for example `v0.1.0-alpha.15-build.42`.
    pub tag: String,
    /// Tag without the leading `v`.
    pub version: String,
    pub asset_url: String,
    pub checksum: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(UpdateInfo),
    Downloading {
        info: UpdateInfo,
        downloaded: u64,
        total: Option<u64>,
    },
    ReadyToRestart(UpdateInfo),
    Error {
        info: Option<UpdateInfo>,
        message: String,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum UpdateCommand {
    Check,
    Download,
    ApplyAndRestart,
}

/// Returns `false` on platforms without a portable update path yet.
pub fn is_supported() -> bool {
    portable_asset_suffix().is_some()
}

/// Starts the background update worker. Network and file operations never run
/// on the UI thread.
pub fn spawn(
    cmd_rx: flume::Receiver<UpdateCommand>,
    state_tx: async_broadcast::Sender<UpdateState>,
    data_dir: PathBuf,
) {
    let exe = std::env::current_exe().ok();
    let app_dir = exe.as_deref().and_then(Path::parent).map(Path::to_path_buf);
    let exe_name = exe
        .as_deref()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "wave_player.exe".to_string());
    let work_dir = std::env::current_dir()
        .unwrap_or_else(|_| app_dir.clone().unwrap_or_else(|| PathBuf::from(".")));

    let _ = std::thread::Builder::new()
        .name("updater".to_string())
        .spawn(move || {
            let mut service = Service {
                data_dir,
                app_dir,
                work_dir,
                exe_name,
                available: None,
                staged: None,
            };

            let mut next_check = if is_supported() {
                Instant::now() + CHECK_INTERVAL
            } else {
                Instant::now() + Duration::from_secs(365 * 24 * 60 * 60)
            };

            loop {
                let timeout = next_check.saturating_duration_since(Instant::now());

                match cmd_rx.recv_timeout(timeout) {
                    Ok(UpdateCommand::Check) => {
                        service.check(&state_tx);
                        next_check = Instant::now() + CHECK_INTERVAL;
                    }
                    Ok(UpdateCommand::Download) => service.download(&state_tx),
                    Ok(UpdateCommand::ApplyAndRestart) => {
                        if service.apply(&state_tx) {
                            std::process::exit(0);
                        }
                    }
                    Err(flume::RecvTimeoutError::Timeout) => {
                        service.check(&state_tx);
                        next_check = Instant::now() + CHECK_INTERVAL;
                    }
                    Err(flume::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
}

struct Service {
    data_dir: PathBuf,
    app_dir: Option<PathBuf>,
    work_dir: PathBuf,
    exe_name: String,
    available: Option<UpdateInfo>,
    staged: Option<PathBuf>,
}

impl Service {
    fn check(&mut self, state_tx: &async_broadcast::Sender<UpdateState>) {
        if let (Some(info), Some(_)) = (&self.available, &self.staged) {
            emit(state_tx, UpdateState::ReadyToRestart(info.clone()));
            return;
        }

        emit(state_tx, UpdateState::Checking);

        match latest_update() {
            Ok(Some(info)) => {
                self.available = Some(info.clone());
                self.staged = None;
                emit(state_tx, UpdateState::Available(info));
            }
            Ok(None) => {
                self.available = None;
                self.staged = None;
                emit(state_tx, UpdateState::UpToDate);
            }
            Err(message) => emit(
                state_tx,
                UpdateState::Error {
                    info: self.available.clone(),
                    message,
                },
            ),
        }
    }

    fn download(&mut self, state_tx: &async_broadcast::Sender<UpdateState>) {
        let Some(info) = self.available.clone() else {
            self.check(state_tx);
            return;
        };

        if self.staged.is_some() {
            emit(state_tx, UpdateState::ReadyToRestart(info));
            return;
        }

        emit(
            state_tx,
            UpdateState::Downloading {
                info: info.clone(),
                downloaded: 0,
                total: (info.size > 0).then_some(info.size),
            },
        );

        match self.download_inner(&info, state_tx) {
            Ok(staging) => {
                self.staged = Some(staging);
                emit(state_tx, UpdateState::ReadyToRestart(info));
            }
            Err(message) => emit(
                state_tx,
                UpdateState::Error {
                    info: Some(info),
                    message,
                },
            ),
        }
    }

    fn download_inner(
        &self,
        info: &UpdateInfo,
        state_tx: &async_broadcast::Sender<UpdateState>,
    ) -> Result<PathBuf, String> {
        let updates_dir = self.data_dir.join("updates");
        fs::create_dir_all(&updates_dir)
            .map_err(|error| format!("Cannot create the update directory: {error}"))?;

        let archive_path = updates_dir.join("update.zip");
        download_asset(info, state_tx, &archive_path)?;

        let staging = updates_dir.join("pending");
        if staging.exists() {
            fs::remove_dir_all(&staging)
                .map_err(|error| format!("Cannot clean the update directory: {error}"))?;
        }
        fs::create_dir_all(&staging)
            .map_err(|error| format!("Cannot create the update directory: {error}"))?;

        let file = File::open(&archive_path)
            .map_err(|error| format!("Cannot open the downloaded update: {error}"))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|error| format!("Cannot read the update archive: {error}"))?;
        archive
            .extract(&staging)
            .map_err(|error| format!("Cannot extract the update archive: {error}"))?;

        let _ = fs::remove_file(&archive_path);

        if !staging.join(&self.exe_name).is_file() {
            return Err(format!(
                "The update archive does not contain {}",
                self.exe_name
            ));
        }

        Ok(staging)
    }

    fn apply(&mut self, state_tx: &async_broadcast::Sender<UpdateState>) -> bool {
        let Some(info) = self.available.clone() else {
            emit(
                state_tx,
                UpdateState::Error {
                    info: None,
                    message: "No downloaded update is available".to_string(),
                },
            );
            return false;
        };

        let Some(staging) = self.staged.clone() else {
            emit(
                state_tx,
                UpdateState::Error {
                    info: Some(info),
                    message: "The update has not been downloaded yet".to_string(),
                },
            );
            return false;
        };

        match apply_platform(
            self.app_dir.as_deref(),
            &self.work_dir,
            &staging,
            &self.exe_name,
        ) {
            Ok(()) => true,
            Err(message) => {
                emit(
                    state_tx,
                    UpdateState::Error {
                        info: Some(info),
                        message,
                    },
                );
                false
            }
        }
    }
}

fn emit(state_tx: &async_broadcast::Sender<UpdateState>, state: UpdateState) {
    let _ = state_tx.try_broadcast(state);
}

fn latest_update() -> Result<Option<UpdateInfo>, String> {
    let Some(suffix) = portable_asset_suffix() else {
        return Err("Automatic updates are not supported on this platform yet".to_string());
    };

    let url =
        format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases?per_page=30");

    let response = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(format_ureq_error)?;

    let releases: Vec<GithubRelease> = response
        .into_json()
        .map_err(|error| format!("Invalid GitHub response: {error}"))?;

    let local_version =
        Version::parse(APP_VERSION).map_err(|error| format!("Invalid local version: {error}"))?;
    let local_build = local_build();

    let mut best: Option<(RemoteVersion, &GithubRelease, &GithubAsset, &GithubAsset)> = None;

    for release in releases.iter().filter(|release| !release.draft) {
        let Some(remote) = parse_tag(&release.tag_name) else {
            continue;
        };

        if !is_newer(&remote, &local_version, local_build) {
            continue;
        }

        let Some(asset) = release
            .assets
            .iter()
            .find(|asset| asset.name.ends_with(suffix))
        else {
            continue;
        };

        let Some(checksums) = release
            .assets
            .iter()
            .find(|asset| asset.name == "SHA256SUMS")
        else {
            continue;
        };

        if best
            .as_ref()
            .is_none_or(|(version, _, _, _)| remote > *version)
        {
            best = Some((remote, release, asset, checksums));
        }
    }

    let Some((_, release, asset, checksums)) = best else {
        return Ok(None);
    };

    let checksum = fetch_checksum(&checksums.browser_download_url, &asset.name)?;

    Ok(Some(UpdateInfo {
        tag: release.tag_name.clone(),
        version: release.tag_name.trim_start_matches('v').to_string(),
        asset_url: asset.browser_download_url.clone(),
        checksum,
        size: asset.size,
    }))
}

fn download_asset(
    info: &UpdateInfo,
    state_tx: &async_broadcast::Sender<UpdateState>,
    destination: &Path,
) -> Result<(), String> {
    let response = ureq::get(&info.asset_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(format_ureq_error)?;

    let total = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
        .or((info.size > 0).then_some(info.size));

    let mut reader = response.into_reader();
    let mut file = File::create(destination)
        .map_err(|error| format!("Cannot create the update file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut last_emit = Instant::now();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Cannot download the update: {error}"))?;

        if read == 0 {
            break;
        }

        file.write_all(&buffer[..read])
            .map_err(|error| format!("Cannot write the update file: {error}"))?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;

        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            emit(
                state_tx,
                UpdateState::Downloading {
                    info: info.clone(),
                    downloaded,
                    total,
                },
            );
            last_emit = Instant::now();
        }
    }

    emit(
        state_tx,
        UpdateState::Downloading {
            info: info.clone(),
            downloaded,
            total: total.or(Some(downloaded)),
        },
    );

    let actual = hex_digest(&hasher.finalize());

    if !actual.eq_ignore_ascii_case(&info.checksum) {
        let _ = fs::remove_file(destination);
        return Err(format!(
            "Checksum mismatch: expected {}, got {actual}",
            info.checksum
        ));
    }

    Ok(())
}

fn fetch_checksum(url: &str, asset_name: &str) -> Result<String, String> {
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(format_ureq_error)?;

    let text = response
        .into_string()
        .map_err(|error| format!("Cannot read SHA256SUMS: {error}"))?;

    find_checksum(&text, asset_name)
        .ok_or_else(|| format!("No SHA-256 checksum published for {asset_name}"))
}

fn find_checksum(text: &str, asset_name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');

        (name == asset_name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

fn parse_tag(tag: &str) -> Option<RemoteVersion> {
    let tag = tag.trim();
    let tag = tag.strip_prefix('v').unwrap_or(tag);

    let (version, build) = match tag.rsplit_once("-build.") {
        Some((version, build)) => (version, build.parse::<u64>().ok()),
        None => (tag, None),
    };

    Version::parse(version)
        .ok()
        .map(|version| RemoteVersion { version, build })
}

fn is_newer(remote: &RemoteVersion, local: &Version, local_build: u64) -> bool {
    match remote.version.cmp(local) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => remote.build.unwrap_or(0) > local_build,
    }
}

fn local_build() -> u64 {
    APP_BUILD
        .and_then(|build| build.parse::<u64>().ok())
        .unwrap_or(0)
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        let _ = write!(&mut output, "{byte:02x}");
    }

    output
}

fn format_ureq_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, response) => {
            let body = response.into_string().unwrap_or_default();
            let body = body.trim();

            if body.is_empty() {
                format!("GitHub returned HTTP {code}")
            } else {
                format!("GitHub returned HTTP {code}: {body}")
            }
        }
        ureq::Error::Transport(error) => format!("Network error: {error}"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RemoteVersion {
    version: Version,
    build: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

fn portable_asset_suffix() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some("_windows_x64_portable.zip")
    }

    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        Some("_windows_arm64_portable.zip")
    }

    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn apply_platform(
    app_dir: Option<&Path>,
    work_dir: &Path,
    staging: &Path,
    exe_name: &str,
) -> Result<(), String> {
    let app_dir =
        app_dir.ok_or_else(|| "Cannot determine the application directory".to_string())?;

    ensure_writable(app_dir)?;
    spawn_windows_helper(app_dir, work_dir, staging, exe_name)
}

#[cfg(not(target_os = "windows"))]
fn apply_platform(
    _app_dir: Option<&Path>,
    _work_dir: &Path,
    _staging: &Path,
    _exe_name: &str,
) -> Result<(), String> {
    Err("Automatic updates are not supported on this platform yet".to_string())
}

#[cfg(target_os = "windows")]
fn ensure_writable(directory: &Path) -> Result<(), String> {
    let probe = directory.join(".wave_player_update_probe");

    fs::write(&probe, b"probe").map_err(|error| {
        format!(
            "The application directory is read-only. Download the installer from GitHub Releases instead: {error}"
        )
    })?;

    let _ = fs::remove_file(&probe);

    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_windows_helper(
    app_dir: &Path,
    work_dir: &Path,
    staging: &Path,
    exe_name: &str,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let script_path =
        std::env::temp_dir().join(format!("wave_player_update_{}.ps1", std::process::id()));

    fs::write(&script_path, WINDOWS_UPDATE_SCRIPT)
        .map_err(|error| format!("Cannot write the update helper: {error}"))?;

    let mut command = std::process::Command::new("powershell");
    command
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
        ])
        .arg(&script_path)
        .arg("-AppPid")
        .arg(std::process::id().to_string())
        .arg("-AppDir")
        .arg(app_dir)
        .arg("-WorkDir")
        .arg(work_dir)
        .arg("-StagingDir")
        .arg(staging)
        .arg("-ExeName")
        .arg(exe_name)
        .creation_flags(CREATE_NO_WINDOW);

    let mut child = command
        .spawn()
        .map_err(|error| format!("Cannot start the update helper: {error}"))?;

    std::thread::sleep(Duration::from_millis(400));

    match child.try_wait() {
        Ok(Some(status)) if status.success() => {
            Err("The update helper exited before the application could close".to_string())
        }
        Ok(Some(status)) => Err(format!("The update helper failed: {status}")),
        Ok(None) => Ok(()),
        Err(error) => Err(format!("Cannot monitor the update helper: {error}")),
    }
}

#[cfg(target_os = "windows")]
const WINDOWS_UPDATE_SCRIPT: &str = r#"param(
    [int]$AppPid,
    [string]$AppDir,
    [string]$WorkDir,
    [string]$StagingDir,
    [string]$ExeName
)

$ErrorActionPreference = "Stop"
$exePath = Join-Path $AppDir $ExeName
$assetsPath = Join-Path $AppDir "assets"
$stagedExe = Join-Path $StagingDir $ExeName
$stagedAssets = Join-Path $StagingDir "assets"
$backup = Join-Path $env:TEMP ("wave_player_backup_" + [guid]::NewGuid().ToString("N"))

while (Get-Process -Id $AppPid -ErrorAction SilentlyContinue) {
    Start-Sleep -Milliseconds 200
}

Start-Sleep -Milliseconds 500

try {
    New-Item -ItemType Directory -Force -Path $backup | Out-Null
    Copy-Item -Force $exePath (Join-Path $backup $ExeName)
    if (Test-Path $assetsPath) {
        Copy-Item -Recurse -Force $assetsPath (Join-Path $backup "assets")
    }

    $copied = $false
    for ($i = 0; $i -lt 20; $i++) {
        try {
            Copy-Item -Force $stagedExe $exePath
            $copied = $true
            break
        } catch {
            Start-Sleep -Milliseconds 250
        }
    }

    if (-not $copied) {
        throw "Cannot replace the application executable"
    }

    if (Test-Path $stagedAssets) {
        Remove-Item -Recurse -Force $assetsPath -ErrorAction SilentlyContinue
        Copy-Item -Recurse -Force $stagedAssets $assetsPath
    }

    Copy-Item -Force (Join-Path $StagingDir "README.md") $AppDir -ErrorAction SilentlyContinue
    Copy-Item -Force (Join-Path $StagingDir "LICENSE") $AppDir -ErrorAction SilentlyContinue
} catch {
    if (Test-Path (Join-Path $backup $ExeName)) {
        Copy-Item -Force (Join-Path $backup $ExeName) $exePath -ErrorAction SilentlyContinue
    }
    if (Test-Path (Join-Path $backup "assets")) {
        Remove-Item -Recurse -Force $assetsPath -ErrorAction SilentlyContinue
        Copy-Item -Recurse -Force (Join-Path $backup "assets") $assetsPath -ErrorAction SilentlyContinue
    }
}

Remove-Item -Recurse -Force $backup -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force $StagingDir -ErrorAction SilentlyContinue
if (-not (Test-Path $WorkDir)) { $WorkDir = $AppDir }
Start-Process -FilePath $exePath -WorkingDirectory $WorkDir
Remove-Item -Force $MyInvocation.MyCommand.Path -ErrorAction SilentlyContinue
"#;

#[cfg(test)]
mod tests {
    use super::{RemoteVersion, UpdateInfo, find_checksum, is_newer, parse_tag};
    use semver::Version;

    #[test]
    fn tag_should_parse_version_and_build() {
        let parsed = parse_tag("v0.1.0-alpha.15-build.42").unwrap();

        assert_eq!(parsed.version, Version::parse("0.1.0-alpha.15").unwrap());
        assert_eq!(parsed.build, Some(42));
    }

    #[test]
    fn tag_without_build_should_still_parse() {
        let parsed = parse_tag("0.1.0-alpha.15").unwrap();

        assert_eq!(parsed.version, Version::parse("0.1.0-alpha.15").unwrap());
        assert_eq!(parsed.build, None);
    }

    #[test]
    fn newer_version_should_win_over_build_number() {
        let local = Version::parse("0.1.0-alpha.15").unwrap();
        let remote = parse_tag("v0.1.0-alpha.16").unwrap();

        assert!(is_newer(&remote, &local, 99));
    }

    #[test]
    fn newer_build_should_update_same_version() {
        let local = Version::parse("0.1.0-alpha.15").unwrap();
        let remote = parse_tag("v0.1.0-alpha.15-build.42").unwrap();

        assert!(is_newer(&remote, &local, 41));
        assert!(!is_newer(&remote, &local, 42));
    }

    #[test]
    fn manual_tag_should_not_override_same_build() {
        let local = Version::parse("0.1.0-alpha.15").unwrap();
        let remote = parse_tag("v0.1.0-alpha.15").unwrap();

        assert!(!is_newer(&remote, &local, 7));
    }

    #[test]
    fn checksum_should_be_found_by_asset_name() {
        let text = "\
0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  WavePlayer_test_windows_x64_portable.zip
fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210  other.zip
";

        assert_eq!(
            find_checksum(text, "WavePlayer_test_windows_x64_portable.zip"),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string())
        );
    }

    #[test]
    fn missing_checksum_should_return_none() {
        assert_eq!(find_checksum("", "file.zip"), None);
    }

    #[test]
    fn remote_version_should_order_by_version_then_build() {
        let older = RemoteVersion {
            version: Version::parse("0.1.0-alpha.15").unwrap(),
            build: None,
        };
        let newer = parse_tag("v0.1.0-alpha.15-build.1").unwrap();

        assert!(newer > older);
    }

    #[test]
    fn github_release_should_deserialize() {
        let json = r#"[{
            "tag_name": "v0.1.0-alpha.15-build.42",
            "draft": false,
            "assets": [
                {
                    "name": "WavePlayer_0.1.0-alpha.15_windows_x64_portable.zip",
                    "browser_download_url": "https://example.com/app.zip",
                    "size": 123
                },
                {
                    "name": "SHA256SUMS",
                    "browser_download_url": "https://example.com/SHA256SUMS",
                    "size": 64
                }
            ]
        }]"#;

        let releases: Vec<super::GithubRelease> = serde_json::from_str(json).unwrap();

        assert_eq!(releases[0].tag_name, "v0.1.0-alpha.15-build.42");
        assert_eq!(releases[0].assets.len(), 2);
        assert_eq!(releases[0].assets[0].size, 123);
    }

    #[test]
    fn checksum_with_binary_marker_should_be_found() {
        let text = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef *WavePlayer_test_windows_x64_portable.zip\n";

        assert_eq!(
            find_checksum(text, "WavePlayer_test_windows_x64_portable.zip"),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string())
        );
    }

    #[test]
    fn update_info_should_keep_tag_and_asset() {
        let info = UpdateInfo {
            tag: "v0.1.0-alpha.15-build.1".to_string(),
            version: "0.1.0-alpha.15-build.1".to_string(),
            asset_url: "https://example.com/asset.zip".to_string(),
            checksum: "0".repeat(64),
            size: 10,
        };

        assert_eq!(info.tag, "v0.1.0-alpha.15-build.1");
    }

    #[test]
    fn supported_platform_should_match_portable_asset() {
        if cfg!(target_os = "windows") {
            assert!(super::portable_asset_suffix().is_some());
        }
    }
}
