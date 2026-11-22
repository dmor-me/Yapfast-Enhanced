//! Version-checked modification of an existing Yapfast installation.
//! Only executable files are touched. Account and plugin data are never opened.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub mod bootstrap;
#[path = "../../../src/update/enhanced.rs"]
pub mod installed_manager;
pub mod maintenance;
pub mod recovery;
pub mod shutdown;
pub mod updates;
pub const META: &str = ".yapfast-enhanced";
const MAX_EXE: u64 = 512 << 20;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub app_version: String,
    pub upstream_sha256: String,
    pub patched_sha256: String,
    pub helper_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema: u32,
    pub phase: String,
    pub app_version: String,
    pub original_sha256: String,
    pub patched_sha256: String,
    pub helper_sha256: String,
    pub original_helper_sha256: Option<String>,
    pub manager_sha256: String,
    #[serde(default)]
    pub previous_patched_sha256: Option<String>,
    #[serde(default)]
    pub previous_helper_sha256: Option<String>,
}

pub fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn hash(path: &Path) -> Result<String> {
    ensure_regular(path)?;
    let mut file = File::open(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 << 10];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn ensure_regular(path: &Path) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("Missing {}", path.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "{} must be a regular file, not a link",
        path.display()
    );
    Ok(())
}

fn json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    ensure_regular(path)?;
    ensure!(
        fs::metadata(path)?.len() <= 64 << 10,
        "{} is too large",
        path.display()
    );
    Ok(serde_json::from_reader(
        File::open(path)?.take((64 << 10) + 1),
    )?)
}

pub fn manifest(bundle: &Path) -> Result<Manifest> {
    let manifest: Manifest = json(&bundle.join("manifest.json"))?;
    ensure!(manifest.schema == 1, "Unsupported Enhanced bundle format");
    ensure!(
        !manifest.app_version.is_empty()
            && manifest.app_version.len() <= 64
            && !manifest.app_version.chars().any(char::is_control),
        "Invalid app version"
    );
    for value in [
        &manifest.upstream_sha256,
        &manifest.patched_sha256,
        &manifest.helper_sha256,
    ] {
        ensure!(valid_hash(value), "Invalid bundle SHA-256");
    }
    Ok(manifest)
}

pub fn state(app: &Path) -> Result<Option<State>> {
    let path = app.join(META).join("state.json");
    if !path.exists() {
        return Ok(None);
    }
    let state: State =
        json(&path).context("Enhanced state is damaged; no application file was changed")?;
    ensure!(state.schema == 1, "Unsupported Enhanced state format");
    for value in [
        &state.original_sha256,
        &state.patched_sha256,
        &state.helper_sha256,
        &state.manager_sha256,
    ] {
        ensure!(valid_hash(value), "Invalid hash in Enhanced state");
    }
    if let Some(hash) = &state.original_helper_sha256 {
        ensure!(valid_hash(hash), "Invalid helper backup hash");
    }
    for hash in [
        &state.previous_patched_sha256,
        &state.previous_helper_sha256,
    ]
    .into_iter()
    .flatten()
    {
        ensure!(valid_hash(hash), "Invalid pending transaction hash");
    }
    Ok(Some(state))
}

fn app_directory(app: &Path) -> Result<PathBuf> {
    let app = app
        .canonicalize()
        .context("Choose the folder containing the installed Yapfast")?;
    ensure!(app.is_dir(), "The Yapfast installation folder is missing");
    ensure_regular(&app.join("yapfast.exe"))?;
    Ok(app)
}

fn meta(app: &Path) -> Result<PathBuf> {
    let app = app
        .canonicalize()
        .context("The Yapfast folder is missing")?;
    let path = app.join(META);
    fs::create_dir_all(&path)?;
    ensure!(
        path.canonicalize()?.starts_with(&app),
        "Enhanced metadata points outside the app folder"
    );
    for name in ["backups", "staging", "downloads"] {
        let folder = path.join(name);
        fs::create_dir_all(&folder)?;
        ensure!(
            folder.canonicalize()?.starts_with(&app),
            "Enhanced storage points outside the app folder"
        );
    }
    Ok(path)
}

fn lock(meta: &Path) -> Result<File> {
    let path = meta.join("install.lock");
    if path.exists() {
        ensure_regular(&path)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let file = options
        .open(path)
        .context("Another Enhanced installation or repair is running")?;
    file.try_lock().map_err(|error| {
        anyhow::anyhow!("Another Enhanced installation or repair is running: {error}")
    })?;
    Ok(file)
}

fn stopped(app: &Path) -> Result<()> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options.open(app.join("yapfast.exe")).context("Completely quit Yapfast, including its tray icon, and retry. The app folder must also be writable.")?;
    let helper = app.join("yapfast-plugins.exe");
    if helper.exists() {
        options
            .open(helper)
            .context("The plugin helper is still running. Quit Yapfast completely and retry.")?;
    }
    Ok(())
}

pub fn stopped_for_restart(app: &Path) -> bool {
    stopped(app).is_ok()
}

pub fn check(app: &Path, bundle: &Path) -> Result<String> {
    let result = compatible(app, bundle)?;
    wait_stopped(app)?;
    Ok(result)
}

fn wait_stopped(app: &Path) -> Result<()> {
    let started = std::time::Instant::now();
    loop {
        match stopped(app) {
            Ok(()) => return Ok(()),
            Err(error) => {
                let sharing = error
                    .downcast_ref::<std::io::Error>()
                    .and_then(std::io::Error::raw_os_error);
                if !cfg!(windows)
                    || !matches!(sharing, Some(32 | 33))
                    || started.elapsed() >= std::time::Duration::from_secs(5)
                {
                    return Err(error);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
}

pub fn stop_for_install(app: &Path, bundle: &Path) -> Result<String> {
    compatible(app, bundle)?;
    shutdown::stop(app)
}

fn compatible(app: &Path, bundle: &Path) -> Result<String> {
    compatible_manifest(app, &manifest(bundle)?)
}

fn compatible_manifest(app: &Path, manifest: &Manifest) -> Result<String> {
    let app = app_directory(app)?;
    let current = hash(&app.join("yapfast.exe"))?;
    let previous = state(&app)?;
    let owned = previous.as_ref().is_some_and(|state| {
        (current == state.patched_sha256
            || state.previous_patched_sha256.as_deref() == Some(&current))
            && state.original_sha256 == manifest.upstream_sha256
    });
    ensure!(
        current == manifest.upstream_sha256 || owned,
        "This installer supports the official Yapfast {} binary only. This installation has a different fingerprint. It was left unchanged; use a compatible Enhanced installer for this Yapfast version.",
        manifest.app_version
    );
    if let Some(previous) = &previous
        && (current == previous.patched_sha256
            || previous.previous_patched_sha256.as_deref() == Some(&current))
    {
        recovery::verify_backup(&app.join(META), &previous.original_sha256, false)
            .context("The original Yapfast backup is damaged; repair was refused")?;
        if let Some(digest) = &previous.original_helper_sha256 {
            recovery::verify_backup(&app.join(META), digest, true)
                .context("The original helper backup is damaged; repair was refused")?;
        }
    }
    Ok(format!(
        "Compatible with Yapfast {}. Your existing account data will be used.",
        manifest.app_version
    ))
}

fn pe(path: &Path) -> Result<()> {
    ensure_regular(path)?;
    ensure!(
        fs::metadata(path)?.len() <= MAX_EXE,
        "Executable exceeds the 512 MiB limit"
    );
    let mut file = File::open(path)?;
    let mut header = [0; 64];
    file.read_exact(&mut header)?;
    ensure!(
        &header[..2] == b"MZ",
        "Not a Windows executable: {}",
        path.display()
    );
    let offset = u32::from_le_bytes(header[60..64].try_into().unwrap());
    ensure!(
        u64::from(offset) + 6 <= file.metadata()?.len(),
        "Invalid PE header"
    );
    file.seek(SeekFrom::Start(offset.into()))?;
    let mut machine = [0; 6];
    file.read_exact(&mut machine)?;
    ensure!(
        &machine == b"PE\0\0\x64\x86",
        "Expected an x64 Windows executable"
    );
    Ok(())
}

fn temporary(folder: &Path, label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    folder.join(format!("{label}-{}-{stamp}.tmp", std::process::id()))
}

fn copy_verified(source: &Path, target: &Path, expected: &str) -> Result<()> {
    ensure_regular(source)?;
    let temp = temporary(target.parent().context("No output directory")?, "copy");
    let result = (|| {
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        let mut input = File::open(source)?.take(MAX_EXE + 1);
        ensure!(
            std::io::copy(&mut input, &mut output)? <= MAX_EXE,
            "Executable grew beyond its size limit"
        );
        output.sync_all()?;
        drop(output);
        ensure!(
            hash(&temp)? == expected,
            "SHA-256 mismatch for {}",
            source.display()
        );
        fs::rename(&temp, target).with_context(|| {
            format!(
                "Cannot replace {}; close Yapfast and retry",
                target.display()
            )
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}

fn save(meta: &Path, state: &State) -> Result<()> {
    let temp = temporary(meta, "state");
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    output.write_all(&serde_json::to_vec_pretty(state)?)?;
    output.sync_all()?;
    drop(output);
    let result = fs::rename(&temp, meta.join("state.json"));
    let _ = fs::remove_file(&temp);
    result.context("Cannot save Enhanced state")
}

fn backup(meta: &Path, source: &Path, digest: &str, helper: bool) -> Result<PathBuf> {
    recovery::backup(meta, source, digest, helper)
}

pub fn install(app: &Path, bundle: &Path) -> Result<String> {
    if bundle.is_dir() {
        return install_directory(app, bundle, None);
    }
    let app = app_directory(app)?;
    let expected = recovery::read_manifest(bundle)?;
    compatible_manifest(&app, &expected)?;
    wait_stopped(&app)?;
    recovery::verify_payload(bundle, &expected)?;
    let meta = meta(&app)?;
    let prepared = recovery::prepare(&meta, bundle)?;
    install_directory(&app, prepared.path(), Some(bundle))
}

fn install_directory(app: &Path, bundle: &Path, original_archive: Option<&Path>) -> Result<String> {
    let app = app_directory(app)?;
    check(&app, bundle)?;
    let manifest = manifest(bundle)?;
    for (name, expected) in [
        ("yapfast.exe", &manifest.patched_sha256),
        ("yapfast-plugins.exe", &manifest.helper_sha256),
    ] {
        pe(&bundle.join(name))?;
        ensure!(
            hash(&bundle.join(name))? == *expected,
            "Bundle integrity check failed for {name}"
        );
    }
    let meta = meta(&app)?;
    let _lock = lock(&meta)?;
    // Repeat after taking the writer lock: another repair may have finished.
    check(&app, bundle)?;
    let old = state(&app)?;
    let current = hash(&app.join("yapfast.exe"))?;
    if current == manifest.upstream_sha256 {
        backup(&meta, &app.join("yapfast.exe"), &current, false)?;
    }
    let helper = app.join("yapfast-plugins.exe");
    let before_helper = if helper.exists() {
        Some(hash(&helper)?)
    } else {
        None
    };
    let original_helper = match &old {
        Some(old)
            if old.original_sha256 == manifest.upstream_sha256
                && before_helper.as_deref() == Some(&old.helper_sha256) =>
        {
            old.original_helper_sha256.clone()
        }
        _ => before_helper.clone(),
    };
    if let Some(digest) = &before_helper {
        // Reinstalling an identical owned helper needs no redundant archive:
        // the staged payload already contains those exact rollback bytes.
        if digest != &manifest.helper_sha256 || original_helper.as_deref() == Some(digest.as_str())
        {
            backup(&meta, &helper, digest, true)?;
        }
    }
    recovery::stage_payload(&meta, bundle, original_archive, &manifest)?;
    let mut pending = State {
        schema: 1,
        phase: "installing".into(),
        app_version: manifest.app_version.clone(),
        original_sha256: manifest.upstream_sha256.clone(),
        patched_sha256: manifest.patched_sha256.clone(),
        helper_sha256: manifest.helper_sha256.clone(),
        original_helper_sha256: original_helper,
        manager_sha256: hash(&std::env::current_exe()?)?,
        previous_patched_sha256: old.as_ref().map(|state| state.patched_sha256.clone()),
        previous_helper_sha256: before_helper.clone(),
    };
    save(&meta, &pending)?;
    copy_verified(
        &bundle.join("yapfast-plugins.exe"),
        &helper,
        &manifest.helper_sha256,
    )?;
    if let Err(error) = copy_verified(
        &bundle.join("yapfast.exe"),
        &app.join("yapfast.exe"),
        &manifest.patched_sha256,
    ) {
        if let Some(digest) = before_helper {
            if digest == manifest.helper_sha256 {
                copy_verified(&bundle.join("yapfast-plugins.exe"), &helper, &digest)?;
            } else {
                recovery::restore_backup(&meta, &digest, true, &helper).context(
                    "Application replacement failed and the prior helper could not be restored",
                )?;
            }
        } else if hash(&helper).ok().as_deref() == Some(&manifest.helper_sha256) {
            fs::remove_file(&helper)?;
        }
        return Err(error);
    }
    pending.phase = "installed".into();
    pending.previous_patched_sha256 = None;
    pending.previous_helper_sha256 = None;
    save(&meta, &pending)?;
    recovery::commit_payload(&meta)?;
    let cleanup = match maintenance::clean(&app, &meta, &pending) {
        Ok(count) => format!(" Removed {count} obsolete managed recovery/update entries."),
        Err(error) => format!(" Installation succeeded; optional cleanup was skipped: {error:#}"),
    };
    Ok(format!(
        "Plugin support installed into Yapfast {}. Start Yapfast normally and open Settings \u{2192} Plugins.{cleanup}",
        manifest.app_version
    ))
}

pub fn restore(app: &Path) -> Result<String> {
    let app = app
        .canonicalize()
        .context("The Yapfast installation folder is missing")?;
    if !app.join(META).join("state.json").exists() {
        return Ok("No Enhanced modification is recorded.".into());
    }
    let meta = meta(&app)?;
    let _lock = lock(&meta)?;
    let Some(mut state) = state(&app)? else {
        return Ok("No Enhanced modification is recorded.".into());
    };
    if !app.join("yapfast.exe").exists() {
        let helper = app.join("yapfast-plugins.exe");
        if helper.exists() && hash(&helper)? == state.helper_sha256 {
            fs::remove_file(helper)?;
        }
        state.phase = "restored".into();
        save(&meta, &state)?;
        return Ok("The base app was already uninstalled. Its removal was respected; account and plugin data were kept.".into());
    }
    stopped(&app)?;
    let current = hash(&app.join("yapfast.exe"))?;
    let replaced = current == state.patched_sha256
        || state.previous_patched_sha256.as_deref() == Some(&current);
    if !replaced && current != state.original_sha256 {
        state.phase = "restored".into();
        save(&meta, &state)?;
        return Ok("A newer or externally changed Yapfast installation was left intact, including its helper. Account settings and plugin data were kept.".into());
    }
    if replaced {
        recovery::restore_backup(
            &meta,
            &state.original_sha256,
            false,
            &app.join("yapfast.exe"),
        )
        .context("Original backup failed its integrity check; nothing was restored")?;
    }
    let helper = app.join("yapfast-plugins.exe");
    let helper_hash = if helper.exists() {
        Some(hash(&helper)?)
    } else {
        None
    };
    if helper_hash.is_none()
        || helper_hash.as_deref() == Some(&state.helper_sha256)
        || helper_hash == state.previous_helper_sha256
    {
        match &state.original_helper_sha256 {
            Some(digest) => recovery::restore_backup(&meta, digest, true, &helper)?,
            None if helper.exists() => fs::remove_file(&helper)?,
            None => {}
        }
    }
    state.phase = "restored".into();
    save(&meta, &state)?;
    Ok(if replaced || current == state.original_sha256 {
        "Original Yapfast restored. Account settings and plugin data were kept."
    } else {
        "Plugin support removed. The newer or externally changed Yapfast executable was left intact. Account settings and plugin data were kept."
    }.into())
}

#[cfg(test)]
mod tests;

// Exercise the application's pure release decisions without compiling the UI.
#[cfg(test)]
#[path = "../../../src/update/enhanced_releases.rs"]
mod enhanced_release_tests;
