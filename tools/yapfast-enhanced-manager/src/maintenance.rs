//! Only complete installations may prune managed files. Unknown files, active
//! download sessions and every state-referenced recovery copy are preserved.
use crate::{META, State, hash, manifest, recovery, state, valid_hash};
use anyhow::{Result, ensure};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const OWNER: &[u8] = b"Yapfast Enhanced download session v1\n";

/// The lease closes before TempDir is dropped, including on Windows.
pub struct DownloadSession {
    _lease: File,
    pub dir: TempDir,
}
fn lease(path: &Path) -> Result<File> {
    if path.exists() {
        crate::ensure_regular(path)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock()
        .map_err(|error| anyhow::anyhow!("Download session is in use: {error}"))?;
    Ok(file)
}
pub fn download_session(app: &Path) -> Result<DownloadSession> {
    let meta = crate::meta(app)?;
    let root = meta.join("downloads");
    recovery::directory(&root)?;
    let dir = tempfile::Builder::new()
        .prefix("download-")
        .tempdir_in(root)?;
    let held = lease(&dir.path().join("lease.lock"))?;
    fs::write(dir.path().join("owner.txt"), OWNER)?;
    Ok(DownloadSession { _lease: held, dir })
}

fn remove_file(path: &Path) -> Result<()> {
    crate::ensure_regular(path)?;
    fs::remove_file(path)?;
    Ok(())
}
fn clean_downloads(meta: &Path) -> Result<usize> {
    let root = meta.join("downloads");
    recovery::directory(&root)?;
    let mut removed = 0;
    for item in fs::read_dir(&root)?.take(1024) {
        let item = item?;
        let name = item.file_name();
        let name = name.to_string_lossy();
        // Legacy/unrecognized downloads have no ownership lease: leave them.
        if !name.starts_with("download-") {
            continue;
        }
        let dir = item.path();
        if recovery::directory(&dir).is_err() {
            continue;
        }
        let owner = dir.join("owner.txt");
        if crate::ensure_regular(&owner).is_err() {
            continue;
        }
        let mut bytes = Vec::new();
        File::open(&owner)?.take(64).read_to_end(&mut bytes)?;
        if bytes != OWNER {
            continue;
        }
        let lock = match lease(&dir.join("lease.lock")) {
            Ok(lock) => lock,
            Err(_) => continue,
        };
        let entries: Vec<_> = fs::read_dir(&dir)?
            .take(9)
            .collect::<std::io::Result<Vec<_>>>()?;
        if entries.len() > 8
            || entries.iter().any(|file| {
                let name = file.file_name();
                let name = name.to_string_lossy();
                let known = matches!(name.as_ref(), "archive.zip" | "lease.lock" | "owner.txt")
                    || (name.starts_with("transfer-") && name.ends_with(".part"));
                !known || crate::ensure_regular(&file.path()).is_err()
            })
        {
            continue;
        }
        // Never recurse through a directory or remove user-added files.
        for file in entries {
            if file.file_name() != "lease.lock" {
                remove_file(&file.path())?;
            }
        }
        drop(lock);
        remove_file(&dir.join("lease.lock"))?;
        fs::remove_dir(&dir)?;
        removed += 1;
    }
    Ok(removed)
}

fn known_backup(name: &str) -> Option<(&str, bool, bool)> {
    for (suffix, helper, zipped) in [
        (".helper.exe.zip", true, true),
        (".exe.zip", false, true),
        (".helper.exe", true, false),
        (".exe", false, false),
    ] {
        if let Some(digest) = name
            .strip_suffix(suffix)
            .filter(|digest| valid_hash(digest))
        {
            return Some((digest, helper, zipped));
        }
    }
    None
}
fn required(state: &State) -> BTreeSet<(String, bool)> {
    let mut keep = BTreeSet::from([(state.original_sha256.clone(), false)]);
    if let Some(digest) = &state.original_helper_sha256 {
        keep.insert((digest.clone(), true));
    }
    if let Some(digest) = &state.previous_helper_sha256 {
        keep.insert((digest.clone(), true));
    }
    if let Some(digest) = &state.previous_patched_sha256 {
        keep.insert((digest.clone(), false));
    }
    keep
}

pub fn clean(app: &Path, meta: &Path, recorded: &State) -> Result<usize> {
    ensure!(
        recorded.phase == "installed",
        "Cleanup is paused while recovery is incomplete"
    );
    ensure!(
        hash(&app.join("yapfast.exe"))? == recorded.patched_sha256
            && hash(&app.join("yapfast-plugins.exe"))? == recorded.helper_sha256,
        "The live installation changed; cleanup was skipped"
    );
    let payload = meta.join("payload.zip");
    let expected = recovery::read_manifest(&payload)?;
    ensure!(
        expected.upstream_sha256 == recorded.original_sha256
            && expected.patched_sha256 == recorded.patched_sha256
            && expected.helper_sha256 == recorded.helper_sha256,
        "Cleanup needs a matching offline repair archive"
    );
    recovery::verify_payload(&payload, &expected)?;
    let keep = required(recorded);
    // Migrate required legacy backups first. A raw file is removed only after
    // its replacement archive has passed an uncompressed SHA-256 check.
    for (digest, helper) in &keep {
        recovery::verify_backup(meta, digest, *helper)?;
        let raw = recovery::backup_path(meta, digest, *helper, false)?;
        if raw.exists() {
            recovery::backup(meta, &raw, digest, *helper)?;
        }
    }
    let backups = meta.join("backups");
    recovery::directory(&backups)?;
    let mut removed = 0;
    let started = std::time::Instant::now();
    for file in fs::read_dir(&backups)?.take(1024) {
        if started.elapsed() >= std::time::Duration::from_secs(10) {
            break;
        }
        let file = file?;
        let name = file.file_name();
        let name = name.to_string_lossy();
        let Some((digest, helper, zipped)) = known_backup(&name) else {
            continue;
        };
        if crate::ensure_regular(&file.path()).is_err() {
            continue;
        }
        let referenced = keep.contains(&(digest.to_owned(), helper));
        if zipped {
            if referenced {
                continue;
            }
            // Verify the exact archive being discarded; damaged/unknown files stay.
            if recovery::verify_backup(meta, digest, helper).is_err() {
                continue;
            }
        } else {
            if hash(&file.path())? != digest {
                continue;
            }
            if referenced {
                recovery::verify_backup(meta, digest, helper)?;
            }
        }
        remove_file(&file.path())?;
        removed += 1;
    }
    // Old installers used an uncompressed payload/. Remove only its exact,
    // verified three-file layout after the new offline archive is committed.
    let legacy = meta.join("payload");
    if legacy.exists() && recovery::directory(&legacy).is_ok() {
        if let Ok(old) = manifest(&legacy) {
            let files: Vec<_> = fs::read_dir(&legacy)?
                .take(4)
                .collect::<std::io::Result<Vec<_>>>()?;
            let known = files.len() == 3
                && files.iter().all(|file| {
                    matches!(
                        file.file_name().to_str(),
                        Some("manifest.json" | "yapfast.exe" | "yapfast-plugins.exe")
                    ) && crate::ensure_regular(&file.path()).is_ok()
                });
            if known
                && hash(&legacy.join("yapfast.exe"))? == old.patched_sha256
                && hash(&legacy.join("yapfast-plugins.exe"))? == old.helper_sha256
            {
                for name in ["manifest.json", "yapfast.exe", "yapfast-plugins.exe"] {
                    remove_file(&legacy.join(name))?;
                    removed += 1;
                }
                fs::remove_dir(&legacy)?;
            }
        }
    }
    removed += clean_downloads(meta)?;
    Ok(removed)
}
pub fn maintain(app: &Path) -> Result<String> {
    let app = crate::app_directory(app)?;
    let meta = crate::meta(&app)?;
    let _held = crate::lock(&meta)?;
    let recorded =
        state(&app)?.ok_or_else(|| anyhow::anyhow!("No Enhanced installation is recorded"))?;
    let removed = clean(&app, &meta, &recorded)?;
    Ok(format!(
        "Removed {removed} obsolete managed recovery/update entries. Current recovery files and plugin/account data were kept."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_download_is_kept_and_completed_download_cleans_itself() {
        let app = tempfile::tempdir().unwrap();
        let session = download_session(app.path()).unwrap();
        let path = session.dir.path().to_owned();
        fs::write(path.join("archive.zip"), "active download").unwrap();
        assert_eq!(clean_downloads(&app.path().join(META)).unwrap(), 0);
        assert!(path.join("archive.zip").exists());
        drop(session);
        assert!(!path.exists());
    }
    #[test]
    fn stale_owned_download_is_removed_but_unknown_files_are_preserved() {
        let app = tempfile::tempdir().unwrap();
        let DownloadSession { _lease, dir } = download_session(app.path()).unwrap();
        drop(_lease);
        let path = dir.keep();
        fs::write(path.join("archive.zip"), "stale").unwrap();
        fs::write(path.join("personal.txt"), "keep").unwrap();
        let meta = app.path().join(META);
        assert_eq!(clean_downloads(&meta).unwrap(), 0);
        assert!(path.join("personal.txt").exists());
        fs::remove_file(path.join("personal.txt")).unwrap();
        assert_eq!(clean_downloads(&meta).unwrap(), 1);
        assert!(!path.exists());
    }
}
