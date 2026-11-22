//! Online dispatcher for exact-version offline installers. No application files
//! are changed here: the verified version-specific installer owns installation.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{io::Read, path::Path, process::Command};

const API: &str = "https://api.github.com/repos/dmor-me/Yapfast-Enhanced/releases";
const PAGES: &str = "https://github.com/dmor-me/Yapfast-Enhanced/releases/tag/";
const DOWNLOADS: &str = "https://github.com/dmor-me/Yapfast-Enhanced/releases/download/";
const INSTALLER: &str = "Yapfast-Enhanced-Installer.exe";
const MAX_INSTALLER: u64 = 256 << 20;

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
    size: u64,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}
struct Selection<'a> {
    release: &'a Release,
    installer: &'a Asset,
    base: String,
}

fn version(tag: &str) -> Option<[u16; 4]> {
    let parts: Vec<_> = tag.strip_prefix('v')?.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut number = [0; 4];
    for (at, part) in parts.into_iter().enumerate() {
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        number[at] = part.parse().ok()?;
    }
    Some(number)
}
fn listed(release: &Release) -> Option<[u16; 4]> {
    if release.draft
        || release.prerelease
        || release.html_url != format!("{PAGES}{}", release.tag_name)
    {
        return None;
    }
    version(&release.tag_name)
}
fn trusted_asset(release: &Release, name: &str) -> bool {
    release.assets.iter().any(|asset| {
        asset.name == name
            && asset.browser_download_url == format!("{DOWNLOADS}{}/{name}", release.tag_name)
    })
}
fn select<'a>(releases: &'a [Release], fingerprint: &str) -> Result<Selection<'a>> {
    ensure!(
        crate::valid_hash(fingerprint),
        "Invalid installed application fingerprint"
    );
    let marker = format!("yapfast-enhanced-for-{fingerprint}.zip");
    let matched = releases
        .iter()
        .filter_map(|release| {
            let number = listed(release)?;
            trusted_asset(release, &marker).then_some((number, release))
        })
        .max_by_key(|(number, _)| *number);
    let Some((number, release)) = matched else {
        let latest = releases
            .iter()
            .filter_map(|r| {
                let n = listed(r)?;
                (trusted_asset(r, INSTALLER)
                    && r.assets.iter().any(|a| {
                        a.name
                            .strip_prefix("yapfast-enhanced-for-")
                            .and_then(|s| s.strip_suffix(".zip"))
                            .is_some_and(crate::valid_hash)
                    }))
                .then_some((n, r))
            })
            .max_by_key(|(n, _)| *n);
        if let Some((n, r)) = latest {
            bail!(
                "No Enhanced package supports this exact Yapfast installation. Nothing was changed. The latest listed compatible Yapfast version is {}.{}.{} (Enhanced {}). See {}. Install a different official Yapfast version yourself only if you choose to; this installer never downgrades it.",
                n[0],
                n[1],
                n[2],
                r.tag_name,
                r.html_url
            );
        }
        bail!(
            "No compatible stable Windows Enhanced releases have been published yet. Nothing was changed. See https://github.com/dmor-me/Yapfast-Enhanced/releases"
        );
    };
    let installer = release
        .assets
        .iter()
        .find(|a| {
            a.name == INSTALLER
                && a.browser_download_url == format!("{DOWNLOADS}{}/{INSTALLER}", release.tag_name)
        })
        .context("The matching release has no trusted Windows installer; nothing was changed")?;
    ensure!(
        installer.size > 0 && installer.size <= MAX_INSTALLER,
        "The matching installer exceeds its download size limit"
    );
    ensure!(
        installer
            .digest
            .as_deref()
            .and_then(|s| s.strip_prefix("sha256:"))
            .is_some_and(crate::valid_hash),
        "GitHub has not supplied the installer's SHA-256 digest; download was refused"
    );
    Ok(Selection {
        release,
        installer,
        base: format!("{}.{}.{}", number[0], number[1], number[2]),
    })
}
fn fingerprint(app: &Path) -> Result<(String, String)> {
    let executable = app.join("yapfast.exe");
    crate::ensure_regular(&executable)?;
    ensure!(
        std::fs::metadata(&executable)?.len() <= crate::MAX_EXE,
        "The selected Yapfast executable exceeds the supported size limit"
    );
    let current = crate::hash(&executable)?;
    let original = match crate::state(app)? {
        Some(state)
            if state.phase == "installed"
                && (current == state.patched_sha256
                    || state.previous_patched_sha256.as_deref() == Some(&current)) =>
        {
            state.original_sha256
        }
        _ => current.clone(),
    };
    Ok((current, original))
}
fn releases(client: &reqwest::blocking::Client) -> Result<Vec<Release>> {
    let mut all = Vec::new();
    for page in 1..=10 {
        let response = client.get(format!("{API}?per_page=100&page={page}")).send()
            .context("Could not reach GitHub. The universal installer needs internet; use a version-specific offline installer instead")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("Enhanced releases are not published yet. Nothing was changed");
        }
        let chunk: Vec<Release> = serde_json::from_slice(&crate::updates::read_small(
            response.error_for_status()?,
            4 << 20,
        )?)
        .context("GitHub returned an invalid release catalog")?;
        let end = chunk.len() < 100;
        all.extend(chunk);
        if end {
            return Ok(all);
        }
    }
    bail!(
        "The release catalog exceeds the supported page limit; choose a version-specific installer"
    )
}
fn verified(path: &Path, asset: &Asset) -> Result<()> {
    ensure!(
        std::fs::metadata(path)?.len() == asset.size && asset.size <= MAX_INSTALLER,
        "Installer size verification failed"
    );
    let expected = asset
        .digest
        .as_deref()
        .and_then(|s| s.strip_prefix("sha256:"))
        .filter(|s| crate::valid_hash(s))
        .context("Missing installer digest")?;
    ensure!(
        crate::hash(path)? == expected,
        "Installer SHA-256 verification failed; nothing was launched"
    );
    Ok(())
}
pub fn check(app: &Path) -> Result<String> {
    let (_, fingerprint) = fingerprint(app)?;
    let client = crate::updates::client()?;
    let releases = releases(&client)?;
    let found = select(&releases, &fingerprint)?;
    Ok(format!(
        "Detected supported Yapfast {}. Enhanced {} is available. {}",
        found.base, found.release.tag_name, found.release.html_url
    ))
}
pub fn install(app: &Path) -> Result<String> {
    let app = app
        .canonicalize()
        .context("Choose the existing Yapfast installation folder")?;
    let (current, fingerprint) = fingerprint(&app)?;
    let client = crate::updates::client()?;
    let releases = releases(&client)?;
    let found = select(&releases, &fingerprint)?;
    // Owned temporary storage is removed on download, cancellation and launch errors.
    let temp = tempfile::Builder::new()
        .prefix("yapfast-enhanced-universal-")
        .tempdir()?;
    let file = temp.path().join(INSTALLER);
    let mut response = client
        .get(&found.installer.browser_download_url)
        .send()?
        .error_for_status()?;
    let mut output = std::fs::File::create(&file)?;
    let count = std::io::copy(
        &mut response.by_ref().take(found.installer.size + 1),
        &mut output,
    )?;
    output.sync_all()?;
    drop(output);
    ensure!(
        count == found.installer.size,
        "Installer download was truncated or oversized"
    );
    verified(&file, found.installer)?;
    ensure!(
        crate::hash(&app.join("yapfast.exe"))? == current,
        "Yapfast changed during the download. Retry detection; nothing was launched"
    );
    let mut destination = std::ffi::OsString::from("/DIR=");
    destination.push(&app);
    let status = Command::new(&file)
        .args([
            std::ffi::OsString::from("/SP-"),
            std::ffi::OsString::from("/NORESTART"),
            destination,
        ])
        .status()
        .context("Could not launch the verified matching installer")?;
    ensure!(
        status.success(),
        "The matching installer was cancelled or failed (exit {:?}). Check that installer's message; no other Yapfast version was selected",
        status.code()
    );
    Ok(format!(
        "The Enhanced {} installer completed for Yapfast {}.",
        found.release.tag_name, found.base
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, hash: &str) -> serde_json::Value {
        serde_json::json!({"tag_name":tag,"html_url":format!("{PAGES}{tag}"),"assets":[
            {"name":format!("yapfast-enhanced-for-{hash}.zip"),"browser_download_url":format!("{DOWNLOADS}{tag}/yapfast-enhanced-for-{hash}.zip"),"size":100},
            {"name":INSTALLER,"browser_download_url":format!("{DOWNLOADS}{tag}/{INSTALLER}"),"size":123,"digest":format!("sha256:{}","a".repeat(64))}]})
    }
    fn decode(values: Vec<serde_json::Value>) -> Vec<Release> {
        serde_json::from_value(serde_json::json!(values)).unwrap()
    }
    #[test]
    fn initial_zero_and_numeric_revisions_work() {
        assert_eq!(version("v0.4.1.0"), Some([0, 4, 1, 0]));
        for tag in [
            "v0.4.1",
            "v0.4.1.00",
            "v0.4.1.-1",
            "v0.4.1.65536",
            "v0.4.1.0-beta",
            "../v0.4.1.0",
            "enhanced-v0.4.1.0",
        ] {
            assert!(version(tag).is_none());
        }
        let hash = "b".repeat(64);
        let releases = decode(vec![
            release("v0.4.1.9", &hash),
            release("v0.4.1.10", &hash),
            release("v0.5.0.0", &"c".repeat(64)),
        ]);
        assert_eq!(
            select(&releases, &hash).unwrap().release.tag_name,
            "v0.4.1.10"
        );
        let releases = decode(vec![release("v0.1.0.0", &hash)]);
        assert_eq!(select(&releases, &hash).unwrap().base, "0.1.0");
    }
    #[test]
    fn untrusted_incomplete_and_unpublished_releases_are_refused() {
        let hash = "b".repeat(64);
        for change in ["draft", "prerelease", "page", "url", "size", "digest"] {
            let mut r = release("v0.4.1.0", &hash);
            match change {
                "draft" | "prerelease" => r[change] = true.into(),
                "page" => r["html_url"] = "https://example.com".into(),
                "url" => {
                    r["assets"][1]["browser_download_url"] =
                        "https://example.com/installer.exe".into()
                }
                "size" => r["assets"][1]["size"] = (MAX_INSTALLER + 1).into(),
                _ => r["assets"][1]["digest"] = serde_json::Value::Null,
            }
            assert!(select(&decode(vec![r]), &hash).is_err(), "{change}");
        }
    }
    #[test]
    fn unsupported_binary_gets_latest_supported_base_without_downgrading() {
        let releases = decode(vec![
            release("v0.3.0.0", &"b".repeat(64)),
            release("v0.4.1.0", &"c".repeat(64)),
        ]);
        let message = select(&releases, &"d".repeat(64))
            .err()
            .unwrap()
            .to_string();
        assert!(message.contains("0.4.1") && message.contains("never downgrades"));
        assert!(select(&[], &"b".repeat(64)).is_err());
    }
    #[test]
    fn detection_reads_only_the_executable_and_managed_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("yapfast.exe");
        std::fs::write(&exe, b"official fixture").unwrap();
        let original = crate::hash(&exe).unwrap();
        assert_eq!(
            fingerprint(dir.path()).unwrap(),
            (original.clone(), original.clone())
        );
        std::fs::write(&exe, b"enhanced fixture").unwrap();
        let patched = crate::hash(&exe).unwrap();
        let meta = dir.path().join(crate::META);
        std::fs::create_dir(&meta).unwrap();
        let state = crate::State {
            schema: 1,
            phase: "installed".into(),
            app_version: "0.1.0".into(),
            original_sha256: original.clone(),
            patched_sha256: patched.clone(),
            helper_sha256: "a".repeat(64),
            manager_sha256: "b".repeat(64),
            original_helper_sha256: None,
            previous_patched_sha256: None,
            previous_helper_sha256: None,
        };
        std::fs::write(meta.join("state.json"), serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(fingerprint(dir.path()).unwrap(), (patched, original));
        std::fs::write(&exe, b"external update").unwrap();
        let current = crate::hash(&exe).unwrap();
        assert_eq!(fingerprint(dir.path()).unwrap(), (current.clone(), current));
    }
    #[test]
    fn downloaded_bytes_require_both_size_and_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("setup.exe");
        std::fs::write(&path, b"fixture").unwrap();
        let mut asset: Asset =
            serde_json::from_value(release("v0.4.1.0", &"b".repeat(64))["assets"][1].clone())
                .unwrap();
        asset.size = 7;
        asset.digest = Some(format!("sha256:{}", crate::hash(&path).unwrap()));
        assert!(verified(&path, &asset).is_ok());
        std::fs::write(&path, b"changed").unwrap();
        assert!(verified(&path, &asset).is_err());
        asset.size = 8;
        assert!(verified(&path, &asset).is_err());
    }
}
