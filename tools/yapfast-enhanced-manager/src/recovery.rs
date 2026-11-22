//! Compressed, hash-checked recovery copies. Extraction is confined to owned
//! staging directories; live executables are always ordinary uncompressed files.
use crate::{MAX_EXE, Manifest, copy_verified, ensure_regular, hash, manifest, valid_hash};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempDir};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::FileOptions};

pub fn directory(path: &Path) -> Result<()> {
    let info = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        info.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = info.file_type().is_symlink();
    ensure!(
        info.is_dir() && !linked,
        "Recovery directory must not be a filesystem link: {}",
        path.display()
    );
    Ok(())
}

fn digest_reader(mut input: impl Read, limit: u64) -> Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 << 10];
    let mut count = 0u64;
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count = count.saturating_add(read as u64);
        ensure!(count <= limit, "Recovery entry exceeds its size limit");
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn archive(path: &Path) -> Result<ZipArchive<File>> {
    ensure_regular(path)?;
    ensure!(
        fs::metadata(path)?.len() <= MAX_EXE,
        "Recovery archive is oversized"
    );
    let zip = ZipArchive::new(File::open(path)?)?;
    ensure!(zip.len() <= 32, "Recovery archive has too many entries");
    Ok(zip)
}
fn entry<'a>(
    zip: &'a mut ZipArchive<File>,
    name: &str,
    limit: u64,
) -> Result<zip::read::ZipFile<'a>> {
    let entry = zip
        .by_name(name)
        .with_context(|| format!("Recovery archive is missing {name}"))?;
    ensure!(
        entry.is_file() && entry.size() <= limit,
        "Invalid or oversized recovery entry {name}"
    );
    ensure!(
        entry
            .unix_mode()
            .is_none_or(|mode| mode & 0o170000 != 0o120000),
        "Recovery entries cannot be symlinks"
    );
    Ok(entry)
}
pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let mut zip = archive(path)?;
    let mut bytes = Vec::new();
    entry(&mut zip, "manifest.json", 64 << 10)?
        .take((64 << 10) + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 64 << 10, "Recovery manifest is oversized");
    let value: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        value.schema == 1
            && valid_hash(&value.upstream_sha256)
            && valid_hash(&value.patched_sha256)
            && valid_hash(&value.helper_sha256),
        "Invalid recovery manifest"
    );
    Ok(value)
}
pub fn verify_payload(path: &Path, expected: &Manifest) -> Result<()> {
    ensure!(
        read_manifest(path)? == *expected,
        "Recovery archive manifest does not match this installation"
    );
    let mut zip = archive(path)?;
    for (name, digest, maximum) in [
        ("yapfast.exe", &expected.patched_sha256, MAX_EXE),
        ("yapfast-plugins.exe", &expected.helper_sha256, 128 << 20),
    ] {
        ensure!(
            digest_reader(entry(&mut zip, name, maximum)?, maximum)? == *digest,
            "Recovery archive SHA-256 mismatch for {name}"
        );
    }
    Ok(())
}

pub enum Prepared {
    Folder(PathBuf),
    Archive(TempDir),
}
impl Prepared {
    pub fn path(&self) -> &Path {
        match self {
            Self::Folder(path) => path,
            Self::Archive(dir) => dir.path(),
        }
    }
}
pub fn prepare(meta: &Path, input: &Path) -> Result<Prepared> {
    if input.is_dir() {
        return Ok(Prepared::Folder(input.to_owned()));
    }
    let found = read_manifest(input)?;
    let staging = meta.join("staging");
    directory(&staging)?;
    let temp = tempfile::Builder::new()
        .prefix("recovery-")
        .tempdir_in(&staging)?;
    crate::updates::unpack(input, temp.path(), &found.upstream_sha256)?;
    Ok(Prepared::Archive(temp))
}

pub fn backup_path(meta: &Path, digest: &str, helper: bool, zipped: bool) -> Result<PathBuf> {
    ensure!(valid_hash(digest), "Invalid recovery fingerprint");
    let suffix = if helper { "helper.exe" } else { "exe" };
    Ok(meta.join("backups").join(format!(
        "{digest}.{suffix}{}",
        if zipped { ".zip" } else { "" }
    )))
}
pub fn verify_backup(meta: &Path, digest: &str, helper: bool) -> Result<()> {
    directory(&meta.join("backups"))?;
    let zipped = backup_path(meta, digest, helper, true)?;
    if zipped.exists() {
        let mut zip = archive(&zipped)?;
        ensure!(
            zip.len() == 1,
            "Original backup archive has unexpected entries"
        );
        ensure!(
            digest_reader(entry(&mut zip, "executable", MAX_EXE)?, MAX_EXE)? == digest,
            "Original backup archive failed its SHA-256 check"
        );
    } else {
        ensure!(
            hash(&backup_path(meta, digest, helper, false)?)? == digest,
            "Original backup failed its SHA-256 check"
        );
    }
    Ok(())
}
pub fn backup(meta: &Path, source: &Path, digest: &str, helper: bool) -> Result<PathBuf> {
    directory(&meta.join("backups"))?;
    let target = backup_path(meta, digest, helper, true)?;
    if target.exists() {
        verify_backup(meta, digest, helper)?;
        return Ok(target);
    }
    ensure!(
        hash(source)? == digest,
        "Backup source does not match its fingerprint"
    );
    let mut temp = NamedTempFile::new_in(meta.join("backups"))?;
    {
        let mut writer = ZipWriter::new(temp.as_file_mut());
        writer.start_file(
            "executable",
            FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(6)),
        )?;
        std::io::copy(&mut File::open(source)?.take(MAX_EXE + 1), &mut writer)?;
        writer.finish()?;
    }
    temp.as_file().sync_all()?;
    // Verify before publishing; an archive never replaces a usable recovery copy unchecked.
    let mut zip = archive(temp.path())?;
    ensure!(
        digest_reader(entry(&mut zip, "executable", MAX_EXE)?, MAX_EXE)? == digest,
        "New backup archive failed verification"
    );
    drop(zip);
    temp.persist(&target).map_err(|error| error.error)?;
    Ok(target)
}
pub fn restore_backup(meta: &Path, digest: &str, helper: bool, target: &Path) -> Result<()> {
    verify_backup(meta, digest, helper)?;
    let zipped = backup_path(meta, digest, helper, true)?;
    if !zipped.exists() {
        return copy_verified(&backup_path(meta, digest, helper, false)?, target, digest);
    }
    directory(&meta.join("staging"))?;
    let mut temp = NamedTempFile::new_in(meta.join("staging"))?;
    let mut zip = archive(&zipped)?;
    let count = std::io::copy(
        &mut entry(&mut zip, "executable", MAX_EXE)?.take(MAX_EXE + 1),
        temp.as_file_mut(),
    )?;
    ensure!(
        count <= MAX_EXE && hash(temp.path())? == digest,
        "Extracted backup failed its integrity check"
    );
    copy_verified(temp.path(), target, digest)
}

/// Stage a new offline payload before touching live files. On interruption the
/// old payload and payload.next.zip remain available until an install commits.
pub fn stage_payload(
    meta: &Path,
    bundle: &Path,
    original_archive: Option<&Path>,
    expected: &Manifest,
) -> Result<()> {
    let next = meta.join("payload.next.zip");
    if let Some(source) = original_archive {
        if source.canonicalize()? == next.canonicalize().unwrap_or_default() {
            return verify_payload(&next, expected);
        }
    }
    let mut temp = NamedTempFile::new_in(meta)?;
    if let Some(source) = original_archive {
        ensure_regular(source)?;
        let count = std::io::copy(
            &mut File::open(source)?.take(MAX_EXE + 1),
            temp.as_file_mut(),
        )?;
        ensure!(count <= MAX_EXE, "Repair archive is oversized");
    } else {
        let mut writer = ZipWriter::new(temp.as_file_mut());
        for name in ["manifest.json", "yapfast.exe", "yapfast-plugins.exe"] {
            ensure_regular(&bundle.join(name))?;
            writer.start_file(
                name,
                FileOptions::default()
                    .compression_method(CompressionMethod::Deflated)
                    .compression_level(Some(6)),
            )?;
            std::io::copy(
                &mut File::open(bundle.join(name))?.take(MAX_EXE + 1),
                &mut writer,
            )?;
        }
        writer.finish()?;
    }
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;
    verify_payload(temp.path(), expected)?;
    if next.exists() {
        ensure_regular(&next)?;
    }
    temp.persist(next).map_err(|error| error.error)?;
    Ok(())
}
pub fn commit_payload(meta: &Path) -> Result<()> {
    let next = meta.join("payload.next.zip");
    let target = meta.join("payload.zip");
    ensure_regular(&next)?;
    if target.exists() {
        ensure_regular(&target)?;
    }
    fs::rename(next, target).context("Plugin support was installed, but its offline repair archive could not be committed; payload.next.zip was kept for recovery")
}
