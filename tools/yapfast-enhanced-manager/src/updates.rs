//! The official app still downloads and verifies official updates. This
//! short-lived helper waits for it to quit, runs that installer, and attaches
//! a matching Enhanced bundle when one has been published. No resident daemon.
use crate::{META, hash, install, manifest, valid_hash};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

pub const REPOSITORY: &str = "dmor-me/Yapfast-Enhanced";
const RELEASES: &str = "https://api.github.com/repos/dmor-me/Yapfast-Enhanced/releases?per_page=30";
const PREFIX: &str = "https://github.com/dmor-me/Yapfast-Enhanced/releases/download/";
const MAX_ARCHIVE: u64 = 384 << 20;

#[derive(Deserialize)]
struct Release {
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent("Yapfast-Enhanced-Manager/0.1")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(240))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?)
}

fn read_small(mut input: impl Read, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.by_ref().take(maximum + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= maximum,
        "Response exceeds its size limit"
    );
    Ok(bytes)
}

pub struct FetchedBundle {
    pub path: PathBuf,
    _session: crate::maintenance::DownloadSession,
}

pub fn fetch_bundle(app: &Path) -> Result<FetchedBundle> {
    let current = hash(&app.join("yapfast.exe"))?;
    let fingerprint = match crate::state(app)? {
        Some(state)
            if current == state.patched_sha256
                || state.previous_patched_sha256.as_deref() == Some(&current) =>
        {
            state.original_sha256
        }
        _ => current,
    };
    let name = format!("yapfast-enhanced-for-{fingerprint}.zip");
    let client = client()?;
    let response = client.get(RELEASES).send()?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("No Enhanced compatibility releases have been published to {REPOSITORY} yet");
    }
    let releases: Vec<Release> =
        serde_json::from_slice(&read_small(response.error_for_status()?, 4 << 20)?)?;
    let asset = releases
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .flat_map(|release| release.assets)
        .find(|asset| asset.name == name)
        .context(
            "There is no compatible Enhanced package for this exact official Yapfast build yet",
        )?;
    ensure!(
        asset.browser_download_url.starts_with(PREFIX),
        "Compatibility package is outside the configured GitHub repository"
    );
    ensure!(
        asset.size > 0 && asset.size <= MAX_ARCHIVE,
        "Compatibility archive exceeds its size limit"
    );
    let expected = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|hash| valid_hash(hash))
        .context("GitHub did not publish a SHA-256 digest for this package")?;
    let session = crate::maintenance::download_session(app)?;
    let archive = session.dir.path().join("archive.zip");
    let mut part = tempfile::Builder::new()
        .prefix("transfer-")
        .suffix(".part")
        .tempfile_in(session.dir.path())?;
    let mut response = client
        .get(&asset.browser_download_url)
        .send()?
        .error_for_status()?;
    let copied = std::io::copy(
        &mut response.by_ref().take(MAX_ARCHIVE + 1),
        part.as_file_mut(),
    )?;
    part.as_file().sync_all()?;
    ensure!(
        copied <= MAX_ARCHIVE && copied == asset.size && hash(part.path())? == expected,
        "Compatibility archive failed its size or SHA-256 check"
    );
    part.persist(&archive).map_err(|error| error.error)?;
    let found = crate::recovery::read_manifest(&archive)?;
    ensure!(
        found.upstream_sha256 == fingerprint,
        "Compatibility package targets another official build"
    );
    crate::recovery::verify_payload(&archive, &found)?;
    Ok(FetchedBundle {
        path: archive,
        _session: session,
    })
}

/// Only three fixed names are extracted. Archive paths never become output
/// paths, so a ../ entry or symlink cannot write outside the bundle directory.
pub fn unpack(archive: &Path, output: &Path, upstream: &str) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    ensure!(
        zip.len() <= 32,
        "Compatibility archive contains too many entries"
    );
    fs::create_dir_all(output)?;
    ensure!(
        !fs::symlink_metadata(output)?.file_type().is_symlink(),
        "Bundle directory must not be a link"
    );
    for (name, limit) in [
        ("manifest.json", 64 << 10),
        ("yapfast.exe", 512 << 20),
        ("yapfast-plugins.exe", 128 << 20),
    ] {
        let mut entry = zip
            .by_name(name)
            .with_context(|| format!("Compatibility archive is missing {name}"))?;
        ensure!(
            entry.is_file() && entry.size() <= limit,
            "Invalid or oversized archive entry {name}"
        );
        if let Some(mode) = entry.unix_mode() {
            ensure!(
                mode & 0o170000 != 0o120000,
                "Archive entry must not be a symlink"
            );
        }
        let target = output.join(name);
        if target.exists() {
            ensure!(
                !fs::symlink_metadata(&target)?.file_type().is_symlink(),
                "Output is a symlink"
            );
        }
        let mut file = File::create(&target)?;
        let count = std::io::copy(&mut entry.by_ref().take(limit + 1), &mut file)?;
        ensure!(count <= limit, "Archive entry exceeds its size limit");
        file.sync_all()?;
    }
    let found = manifest(output)?;
    ensure!(
        found.upstream_sha256 == upstream,
        "Compatibility package targets another official Yapfast build"
    );
    for (name, expected) in [
        ("yapfast.exe", found.patched_sha256),
        ("yapfast-plugins.exe", found.helper_sha256),
    ] {
        ensure!(
            hash(&output.join(name))? == expected,
            "Compatibility package hash mismatch for {name}"
        );
    }
    Ok(())
}

pub fn repair_online(app: &Path) -> Result<String> {
    crate::stopped(app)?;
    let meta = crate::meta(app)?;
    for name in ["payload.next.zip", "payload.zip", "payload"] {
        let input = meta.join(name);
        if !input.exists() {
            continue;
        }
        if let Ok(prepared) = crate::recovery::prepare(&meta, &input) {
            if crate::check(app, prepared.path()).is_ok() {
                return crate::install_directory(
                    app,
                    prepared.path(),
                    input.is_file().then_some(input.as_path()),
                );
            }
        }
    }
    let bundle = fetch_bundle(app)?;
    // The lease protects the download until installation has persisted recovery.
    // Its TempDir then removes the archive on both success and failure.
    install(app, &bundle.path)
}

#[cfg(windows)]
fn hidden(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}
#[cfg(not(windows))]
fn hidden(_: &mut Command) {}

pub fn notify(message: &str) {
    #[cfg(windows)]
    unsafe {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(
                window: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                flags: u32,
            ) -> i32;
        }
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let caption: Vec<u16> = "Yapfast Enhanced".encode_utf16().chain(Some(0)).collect();
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), 0x40);
    }
    #[cfg(not(windows))]
    eprintln!("{message}");
}

pub fn restart(app: &Path, arguments: &[std::ffi::OsString]) -> Result<()> {
    let mut restart = Command::new(app.join("yapfast.exe"));
    restart.args(arguments);
    hidden(&mut restart);
    restart.spawn().context("Yapfast could not be restarted")?;
    Ok(())
}

pub fn managed_update(
    app: &Path,
    installer: &Path,
    arguments: &[std::ffi::OsString],
) -> Result<String> {
    ensure!(
        installer.is_file(),
        "The verified official installer is missing"
    );
    let deadline = Instant::now() + Duration::from_secs(90);
    while crate::stopped(app).is_err() {
        ensure!(
            Instant::now() < deadline,
            "Yapfast did not close; the official update was not run"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    let mut command = Command::new(installer);
    command
        .args([
            "/SILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/CLOSEAPPLICATIONS",
        ])
        .arg(format!("/DIR={}", app.display()));
    hidden(&mut command);
    let status = command
        .status()
        .context("Cannot start the verified official Yapfast installer")?;
    let repaired = if status.success() {
        repair_online(app)
    } else {
        Err(anyhow::anyhow!(
            "The official installer failed with {status}; no Enhanced repair was attempted"
        ))
    };
    let message = match &repaired {
        Ok(message) => message.clone(),
        Err(error) if status.success() => format!(
            "Yapfast was updated successfully. Plugin support is paused: {error:#}. The updated official app was left intact. Run Enhanced Repair when a compatible package is available."
        ),
        Err(error) => format!(
            "The Yapfast update could not finish: {error:#}. The current application will be restarted. Your account data was not changed."
        ),
    };
    let _ = fs::write(app.join(META).join("last-update.txt"), &message);
    restart(app, arguments)?;
    if repaired.is_err() {
        notify(&message);
    }
    Ok(message)
}
