use super::*;
use tempfile::TempDir;

fn executable(marker: u8) -> Vec<u8> {
    let mut bytes = vec![marker; 128];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
    bytes
}

fn fixture() -> (TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("Yapfast");
    let bundle = dir.path().join("payload");
    fs::create_dir(&app).unwrap();
    fs::create_dir(&bundle).unwrap();
    fs::write(app.join("yapfast.exe"), executable(1)).unwrap();
    fs::write(app.join("account-data.txt"), "untouched user data").unwrap();
    fs::write(bundle.join("yapfast.exe"), executable(2)).unwrap();
    fs::write(bundle.join("yapfast-plugins.exe"), executable(3)).unwrap();
    let manifest = Manifest {
        schema: 1,
        app_version: "0.4.1".into(),
        upstream_sha256: hash(&app.join("yapfast.exe")).unwrap(),
        patched_sha256: hash(&bundle.join("yapfast.exe")).unwrap(),
        helper_sha256: hash(&bundle.join("yapfast-plugins.exe")).unwrap(),
    };
    fs::write(
        bundle.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    (dir, app, bundle)
}

#[test]
fn installs_in_place_and_restores_exact_original_without_touching_accounts() {
    let (_dir, app, bundle) = fixture();
    let original = hash(&app.join("yapfast.exe")).unwrap();
    install(&app, &bundle).unwrap();
    assert_eq!(
        hash(&app.join("yapfast.exe")).unwrap(),
        manifest(&bundle).unwrap().patched_sha256
    );
    assert_eq!(state(&app).unwrap().unwrap().phase, "installed");
    restore(&app).unwrap();
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), original);
    assert!(!app.join("yapfast-plugins.exe").exists());
    assert_eq!(
        fs::read_to_string(app.join("account-data.txt")).unwrap(),
        "untouched user data"
    );
}

#[test]
fn unsupported_official_build_is_not_modified() {
    let (_dir, app, bundle) = fixture();
    fs::write(app.join("yapfast.exe"), executable(9)).unwrap();
    let original = hash(&app.join("yapfast.exe")).unwrap();
    assert!(
        install(&app, &bundle)
            .unwrap_err()
            .to_string()
            .contains("different fingerprint")
    );
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), original);
    assert!(!app.join(META).exists());
}

#[test]
fn corrupted_payload_is_refused_before_changing_the_app() {
    let (_dir, app, bundle) = fixture();
    let original = hash(&app.join("yapfast.exe")).unwrap();
    fs::write(bundle.join("yapfast.exe"), executable(8)).unwrap();
    assert!(install(&app, &bundle).is_err());
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), original);
    assert!(!app.join(META).exists());
}

#[test]
fn repair_recovers_after_reinstalling_the_same_official_version() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    fs::write(app.join("yapfast.exe"), executable(1)).unwrap();
    install(&app, &bundle).unwrap();
    restore(&app).unwrap();
    assert_eq!(
        hash(&app.join("yapfast.exe")).unwrap(),
        manifest(&bundle).unwrap().upstream_sha256
    );
}

#[test]
fn uninstall_never_downgrades_an_unknown_newer_official_update() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    fs::write(app.join("yapfast.exe"), executable(9)).unwrap();
    fs::write(app.join("yapfast-plugins.exe"), executable(7)).unwrap();
    let newer = hash(&app.join("yapfast.exe")).unwrap();
    let newer_helper = hash(&app.join("yapfast-plugins.exe")).unwrap();
    restore(&app).unwrap();
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), newer);
    assert_eq!(
        hash(&app.join("yapfast-plugins.exe")).unwrap(),
        newer_helper
    );
}

#[test]
fn preserves_and_restores_a_preexisting_helper() {
    let (_dir, app, bundle) = fixture();
    fs::write(app.join("yapfast-plugins.exe"), executable(6)).unwrap();
    let original = hash(&app.join("yapfast-plugins.exe")).unwrap();
    install(&app, &bundle).unwrap();
    restore(&app).unwrap();
    assert_eq!(hash(&app.join("yapfast-plugins.exe")).unwrap(), original);
}

#[test]
fn a_corrupted_original_backup_is_never_restored() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let state = state(&app).unwrap().unwrap();
    fs::write(
        app.join(META)
            .join("backups")
            .join(format!("{}.exe.zip", state.original_sha256)),
        executable(8),
    )
    .unwrap();
    assert!(restore(&app).is_err());
    assert_eq!(
        hash(&app.join("yapfast.exe")).unwrap(),
        state.patched_sha256
    );
}

#[test]
fn a_repair_cannot_change_the_supported_official_base() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let mut manifest = manifest(&bundle).unwrap();
    manifest.upstream_sha256 = "a".repeat(64);
    fs::write(
        bundle.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(check(&app, &bundle).is_err());
}

#[test]
fn interrupted_mod_replacement_remains_recoverable() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let mut pending = state(&app).unwrap().unwrap();
    pending.phase = "installing".into();
    pending.previous_patched_sha256 = Some(pending.patched_sha256.clone());
    pending.patched_sha256 = "a".repeat(64);
    save(&app.join(META), &pending).unwrap();
    restore(&app).unwrap();
    assert_eq!(
        hash(&app.join("yapfast.exe")).unwrap(),
        pending.original_sha256
    );
}

#[cfg(windows)]
#[test]
fn a_running_or_locked_app_is_not_modified() {
    use std::os::windows::fs::OpenOptionsExt;
    let (_dir, app, bundle) = fixture();
    let _busy = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(app.join("yapfast.exe"))
        .unwrap();
    assert!(install(&app, &bundle).is_err());
    assert!(!app.join(META).exists());
}

#[test]
fn archives_cannot_extract_an_arbitrary_path() {
    let (_dir, _app, bundle) = fixture();
    let archive = bundle.join("test.zip");
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    for name in ["manifest.json", "yapfast.exe", "yapfast-plugins.exe"] {
        zip.start_file(name, zip::write::FileOptions::default())
            .unwrap();
        zip.write_all(&fs::read(bundle.join(name)).unwrap())
            .unwrap();
    }
    zip.start_file("../escaped.txt", zip::write::FileOptions::default())
        .unwrap();
    zip.write_all(b"must not be extracted").unwrap();
    zip.finish().unwrap();
    let output = bundle.join("unpacked");
    updates::unpack(
        &archive,
        &output,
        &manifest(&bundle).unwrap().upstream_sha256,
    )
    .unwrap();
    assert!(!bundle.join("escaped.txt").exists());
}

#[test]
fn removing_the_mod_after_the_base_app_does_not_resurrect_yapfast() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    fs::remove_file(app.join("yapfast.exe")).unwrap();
    restore(&app).unwrap();
    assert!(!app.join("yapfast.exe").exists());
    assert!(!app.join("yapfast-plugins.exe").exists());
    assert_eq!(
        fs::read_to_string(app.join("account-data.txt")).unwrap(),
        "untouched user data"
    );
}

#[test]
fn archived_recovery_preserves_exact_original_and_offline_repair() {
    let (_dir, app, bundle) = fixture();
    let original = hash(&app.join("yapfast.exe")).unwrap();
    install(&app, &bundle).unwrap();
    let meta = app.join(META);
    assert!(
        recovery::backup_path(&meta, &original, false, true)
            .unwrap()
            .exists()
    );
    assert!(
        !recovery::backup_path(&meta, &original, false, false)
            .unwrap()
            .exists()
    );
    assert!(meta.join("payload.zip").exists());
    assert!(!meta.join("payload.next.zip").exists());
    fs::write(app.join("yapfast.exe"), executable(1)).unwrap();
    updates::repair_online(&app).unwrap();
    assert_eq!(
        hash(&app.join("yapfast.exe")).unwrap(),
        manifest(&bundle).unwrap().patched_sha256
    );
    restore(&app).unwrap();
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), original);
    assert_eq!(
        fs::read_to_string(app.join("account-data.txt")).unwrap(),
        "untouched user data"
    );
}

#[test]
fn legacy_raw_backups_and_payload_migrate_only_after_verified_install() {
    let (_dir, app, bundle) = fixture();
    let original = hash(&app.join("yapfast.exe")).unwrap();
    let meta = meta(&app).unwrap();
    let raw = recovery::backup_path(&meta, &original, false, false).unwrap();
    fs::copy(app.join("yapfast.exe"), &raw).unwrap();
    let legacy = meta.join("payload");
    fs::create_dir(&legacy).unwrap();
    for name in ["manifest.json", "yapfast.exe", "yapfast-plugins.exe"] {
        fs::copy(bundle.join(name), legacy.join(name)).unwrap();
    }
    install(&app, &bundle).unwrap();
    assert!(!raw.exists());
    assert!(!legacy.exists());
    recovery::verify_backup(&meta, &original, false).unwrap();
    restore(&app).unwrap();
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), original);
}

#[test]
fn legacy_raw_restore_remains_supported() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let state = state(&app).unwrap().unwrap();
    let meta = app.join(META);
    let old = recovery::backup_path(&meta, &state.original_sha256, false, false).unwrap();
    fs::write(&old, executable(1)).unwrap();
    fs::remove_file(recovery::backup_path(&meta, &state.original_sha256, false, true).unwrap())
        .unwrap();
    restore(&app).unwrap();
    assert_eq!(
        hash(&app.join("yapfast.exe")).unwrap(),
        state.original_sha256
    );
}

#[test]
fn cleanup_keeps_required_helpers_unknown_files_and_modified_backups() {
    let (_dir, app, bundle) = fixture();
    fs::write(app.join("yapfast-plugins.exe"), executable(6)).unwrap();
    let original_helper = hash(&app.join("yapfast-plugins.exe")).unwrap();
    install(&app, &bundle).unwrap();
    let meta = app.join(META);
    let extra = meta.join("backups/outdated.tmp");
    fs::write(&extra, executable(7)).unwrap();
    let extra_hash = hash(&extra).unwrap();
    let obsolete = recovery::backup_path(&meta, &extra_hash, false, false).unwrap();
    fs::rename(extra, &obsolete).unwrap();
    let note = meta.join("backups/notes.txt");
    fs::write(&note, "keep").unwrap();
    let changed = recovery::backup_path(&meta, &"f".repeat(64), false, false).unwrap();
    fs::write(&changed, "changed file").unwrap();
    maintenance::maintain(&app).unwrap();
    assert!(!obsolete.exists());
    assert!(note.exists() && changed.exists());
    recovery::verify_backup(&meta, &original_helper, true).unwrap();
    restore(&app).unwrap();
    assert_eq!(
        hash(&app.join("yapfast-plugins.exe")).unwrap(),
        original_helper
    );
}

#[test]
fn cleanup_never_runs_during_interrupted_install_or_without_valid_recovery() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let meta = app.join(META);
    let raw = meta.join("backups/protected.tmp");
    fs::write(&raw, executable(7)).unwrap();
    let digest = hash(&raw).unwrap();
    let protected = recovery::backup_path(&meta, &digest, false, false).unwrap();
    fs::rename(raw, &protected).unwrap();
    let mut recorded = state(&app).unwrap().unwrap();
    recorded.phase = "installing".into();
    save(&meta, &recorded).unwrap();
    assert!(maintenance::maintain(&app).is_err());
    assert!(protected.exists());
    recorded.phase = "installed".into();
    save(&meta, &recorded).unwrap();
    fs::write(meta.join("payload.zip"), "damaged archive").unwrap();
    assert!(maintenance::maintain(&app).is_err());
    assert!(protected.exists());
    restore(&app).unwrap();
}

#[test]
fn interrupted_archive_commit_recovers_offline() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let meta = app.join(META);
    fs::rename(meta.join("payload.zip"), meta.join("payload.next.zip")).unwrap();
    updates::repair_online(&app).unwrap();
    assert!(meta.join("payload.zip").exists());
    assert!(!meta.join("payload.next.zip").exists());
    restore(&app).unwrap();
}

#[test]
fn cleanup_does_not_remove_a_legacy_payload_with_unknown_files() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let legacy = app.join(META).join("payload");
    fs::create_dir(&legacy).unwrap();
    for name in ["manifest.json", "yapfast.exe", "yapfast-plugins.exe"] {
        fs::copy(bundle.join(name), legacy.join(name)).unwrap();
    }
    fs::write(legacy.join("user-note.txt"), "keep").unwrap();
    maintenance::maintain(&app).unwrap();
    assert!(legacy.join("user-note.txt").exists());
    assert!(legacy.join("yapfast.exe").exists());
}

#[test]
fn corrupt_archive_is_refused_without_touching_live_files() {
    let (dir, app, _bundle) = fixture();
    let before = hash(&app.join("yapfast.exe")).unwrap();
    let archive = dir.path().join("broken.zip");
    fs::write(&archive, "broken ZIP").unwrap();
    assert!(install(&app, &archive).is_err());
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), before);
    assert!(!app.join(META).exists());
}

#[test]
fn a_damaged_required_helper_backup_blocks_repair_before_live_changes() {
    let (_dir, app, bundle) = fixture();
    fs::write(app.join("yapfast-plugins.exe"), executable(6)).unwrap();
    let original_helper = hash(&app.join("yapfast-plugins.exe")).unwrap();
    install(&app, &bundle).unwrap();
    let before = hash(&app.join("yapfast.exe")).unwrap();
    let damaged = recovery::backup_path(&app.join(META), &original_helper, true, true).unwrap();
    fs::write(damaged, "damaged").unwrap();
    assert!(install(&app, &bundle).is_err());
    assert_eq!(hash(&app.join("yapfast.exe")).unwrap(), before);
}

#[test]
fn an_active_install_lease_prevents_parallel_cleanup() {
    let (_dir, app, bundle) = fixture();
    install(&app, &bundle).unwrap();
    let _held = lock(&app.join(META)).unwrap();
    assert!(maintenance::maintain(&app).is_err());
    assert!(app.join(META).join("payload.zip").exists());
}

#[cfg(windows)]
#[test]
fn a_running_helper_prevents_replacing_its_executable() {
    use std::os::windows::fs::OpenOptionsExt;
    let (_dir, app, bundle) = fixture();
    fs::copy(
        bundle.join("yapfast-plugins.exe"),
        app.join("yapfast-plugins.exe"),
    )
    .unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(app.join("yapfast-plugins.exe"))
        .unwrap();
    assert!(stopped(&app).is_err());
    drop(held);
    assert!(stopped(&app).is_ok());
}
