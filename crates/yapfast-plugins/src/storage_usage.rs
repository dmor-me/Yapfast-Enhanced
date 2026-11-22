//! Read-only, bounded disk-size snapshots. Nothing here reads plugin contents,
//! starts a plugin, changes its grants, or follows filesystem links.
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use crate::store::LockEntry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
    current: Vec<PathBuf>,
    previous: Vec<PathBuf>,
    data: PathBuf,
}

impl Paths {
    pub fn new(root: &Path, plugin: &crate::api::PluginId, entry: &LockEntry) -> Self {
        fn candidates(entry: &LockEntry) -> Vec<PathBuf> {
            [&entry.store, &entry.wasm_sha256, &entry.manifest_sha256]
                .into_iter()
                // Lock-file strings must never become arbitrary filesystem paths.
                .filter(|sha| sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(|sha| PathBuf::from("store").join(sha.to_ascii_lowercase()))
                .collect()
        }
        Self {
            root: root.to_owned(),
            current: candidates(entry),
            previous: entry
                .previous
                .as_deref()
                .map(candidates)
                .unwrap_or_default(),
            data: PathBuf::from("data").join(plugin.as_str()),
        }
    }

    /// File lengths, not RAM or physical allocation after filesystem compression.
    /// Call on a worker. The result is a snapshot; it does not flush pending data.
    pub fn measure(&self, cancelled: &AtomicBool) -> Usage {
        self.measure_with_limit(cancelled, 100_000)
    }

    fn measure_with_limit(&self, cancelled: &AtomicBool, limit: usize) -> Usage {
        let mut scan = Scan {
            root: &self.root,
            cancelled,
            deadline: Instant::now() + Duration::from_secs(5),
            remaining: limit,
            partial: false,
            seen: HashSet::new(),
        };
        let plugin_files = scan.version(&self.current);
        let rollback = scan.version(&self.previous);
        let data = scan.tree(&self.data, 0);
        // Network spill files live beside plugins/, not in a plugin's private data.
        // They cannot be reliably attributed to a plugin, so aren't in its total.
        Usage {
            plugin_files,
            data,
            rollback,
            partial: scan.partial,
            external: self.current.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub plugin_files: u64,
    pub data: u64,
    pub rollback: u64,
    pub partial: bool,
    pub external: bool,
}

impl Usage {
    pub fn total(&self) -> u64 {
        self.plugin_files
            .saturating_add(self.data)
            .saturating_add(self.rollback)
    }
}

/// A shared resource with a lifecycle independent of individual plugin details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachePaths {
    root: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheUsage {
    pub bytes: u64,
    pub partial: bool,
}

impl CachePaths {
    pub fn new(plugin_root: &Path) -> Self {
        Self {
            root: plugin_root.to_owned(),
        }
    }
    pub fn measure(&self, cancelled: &AtomicBool) -> CacheUsage {
        let mut scan = Scan {
            root: &self.root,
            cancelled,
            deadline: Instant::now() + Duration::from_secs(5),
            remaining: 100_000,
            partial: false,
            seen: HashSet::new(),
        };
        let bytes = scan.tree(Path::new("cache"), 0);
        CacheUsage {
            bytes,
            partial: scan.partial,
        }
    }
}

struct Scan<'a> {
    root: &'a Path,
    cancelled: &'a AtomicBool,
    deadline: Instant,
    remaining: usize,
    partial: bool,
    seen: HashSet<PathBuf>,
}

fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0 // Includes junctions and all reparse points.
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

impl Scan<'_> {
    fn available(&mut self) -> bool {
        if self.remaining == 0
            || self.cancelled.load(Ordering::Relaxed)
            || Instant::now() >= self.deadline
        {
            self.partial = true;
            false
        } else {
            self.remaining -= 1;
            true
        }
    }

    /// Check every component before reading a child: symlink_metadata on just
    /// the leaf would still follow a junction in store/ or data/.
    fn metadata(&mut self, relative: &Path) -> Option<fs::Metadata> {
        let mut path = self.root.to_owned();
        let parts = std::iter::once(None).chain(relative.components().map(Some));
        let mut last = None;
        for part in parts {
            if let Some(part) = part {
                if !matches!(part, std::path::Component::Normal(_)) {
                    self.partial = true;
                    return None;
                }
                path.push(part);
            }
            match fs::symlink_metadata(&path) {
                Ok(meta) if !linked(&meta) => last = Some(meta),
                Ok(_) => {
                    self.partial = true;
                    return None;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
                Err(_) => {
                    self.partial = true;
                    return None;
                }
            }
        }
        last
    }

    fn version(&mut self, candidates: &[PathBuf]) -> u64 {
        for relative in candidates {
            if !self.available() {
                return 0;
            }
            if self.metadata(&relative.join("manifest.json")).is_some() {
                if !self.seen.insert(relative.to_owned()) {
                    return 0;
                }
                return self.tree(relative, 0);
            }
        }
        if !candidates.is_empty() {
            self.partial = true;
        }
        0
    }

    fn tree(&mut self, relative: &Path, depth: usize) -> u64 {
        if !self.available() {
            return 0;
        }
        if depth > 64 {
            self.partial = true;
            return 0;
        }
        let Some(meta) = self.metadata(relative) else {
            return 0;
        };
        if meta.is_file() {
            return meta.len();
        }
        if !meta.is_dir() {
            self.partial = true;
            return 0;
        }
        let Ok(entries) = fs::read_dir(self.root.join(relative)) else {
            self.partial = true;
            return 0;
        };
        let mut bytes = 0u64;
        for entry in entries {
            if !self.available() {
                break;
            }
            match entry {
                Ok(entry) => {
                    bytes = bytes
                        .saturating_add(self.tree(&relative.join(entry.file_name()), depth + 1))
                }
                Err(_) => self.partial = true,
            }
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let entry = LockEntry {
            store: "a".repeat(64),
            previous: Some(Box::new(LockEntry {
                store: "b".repeat(64),
                ..Default::default()
            })),
            ..Default::default()
        };
        let paths = Paths::new(
            dir.path(),
            &crate::api::PluginId::parse("local.one").unwrap(),
            &entry,
        );
        for (name, data) in [
            (
                format!("store/{}/manifest.json", "a".repeat(64)),
                b"{}".as_slice(),
            ),
            (
                format!("store/{}/plugin.wasm", "a".repeat(64)),
                b"module".as_slice(),
            ),
            (
                format!("store/{}/manifest.json", "b".repeat(64)),
                b"old".as_slice(),
            ),
            (
                "data/local.one/nested/storage.json".into(),
                b"saved".as_slice(),
            ),
            ("cache/shared".into(), b"compile".as_slice()),
            (
                "data/local.other/storage.json".into(),
                b"not this plugin".as_slice(),
            ),
        ] {
            let file = dir.path().join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, data).unwrap();
        }
        (dir, paths)
    }

    #[test]
    fn storage_usage_separates_owned_data_rollback_and_shared_cache() {
        let (_dir, paths) = fixture();
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!((usage.plugin_files, usage.rollback, usage.data), (8, 3, 5));
        let shared = CachePaths::new(&paths.root).measure(&AtomicBool::new(false));
        assert_eq!(shared.bytes, 7);
        assert!(!shared.partial);
        assert_eq!(usage.total(), 16);
        assert!(!usage.partial);
    }

    #[test]
    fn storage_usage_does_not_count_same_version_twice() {
        let (_dir, mut paths) = fixture();
        paths.previous = paths.current.clone();
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!(usage.rollback, 0);
        assert_eq!(usage.total(), 13);
    }

    #[test]
    fn storage_usage_rejects_paths_from_untrusted_lock_fields() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(
            dir.path(),
            &crate::api::PluginId::parse("local.one").unwrap(),
            &LockEntry {
                store: "../../private".into(),
                wasm_sha256: "C:\\private".into(),
                ..Default::default()
            },
        );
        assert!(paths.current.is_empty());
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!(usage.total(), 0);
        assert!(usage.external);
    }

    #[test]
    fn storage_usage_cancellation_and_limits_are_reported_as_partial() {
        let (_dir, paths) = fixture();
        let usage = paths.measure(&AtomicBool::new(true));
        assert!(usage.partial);
        assert_eq!(usage.total(), 0);
        assert!(paths.measure_with_limit(&AtomicBool::new(false), 2).partial);
    }

    #[test]
    fn storage_usage_refresh_sees_changes_and_missing_data_is_zero() {
        let (dir, paths) = fixture();
        fs::remove_dir_all(dir.path().join("data/local.one")).unwrap();
        assert_eq!(paths.measure(&AtomicBool::new(false)).data, 0);
        fs::create_dir_all(dir.path().join("data/local.one")).unwrap();
        fs::write(dir.path().join("data/local.one/new"), b"new data").unwrap();
        assert_eq!(paths.measure(&AtomicBool::new(false)).data, 8);
    }

    #[test]
    fn shared_cache_is_available_without_any_installed_plugins() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("cache")).unwrap();
        fs::write(dir.path().join("cache/compiled"), [0u8; 83]).unwrap();
        let cache = CachePaths::new(dir.path());
        assert_eq!(cache.measure(&AtomicBool::new(false)).bytes, 83);
        assert!(cache.measure(&AtomicBool::new(true)).partial);
    }

    #[test]
    fn missing_shared_cache_is_zero_without_creating_directories() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CachePaths::new(&dir.path().join("plugins"));
        assert_eq!(
            cache.measure(&AtomicBool::new(false)),
            CacheUsage::default()
        );
        assert!(!dir.path().join("plugins").exists());
    }

    #[cfg(windows)]
    #[test]
    fn storage_usage_never_follows_windows_junctions() {
        let (dir, paths) = fixture();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("large"), [0u8; 5000]).unwrap();
        let link = dir.path().join("data/local.one/link");
        let create = |link: &Path| {
            let output = std::process::Command::new("cmd.exe")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(link)
                .arg(outside.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "junction creation failed: {output:?}"
            );
        };
        create(&link);
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!(usage.data, 5);
        assert!(usage.partial);
        fs::remove_dir(&link).unwrap();
        let ancestor = dir.path().join("data/local.one");
        fs::remove_dir_all(&ancestor).unwrap();
        create(&ancestor);
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!(usage.data, 0);
        assert!(usage.partial);
        fs::remove_dir(&ancestor).unwrap();
        assert!(outside.path().join("large").exists());
    }

    #[cfg(unix)]
    #[test]
    fn storage_usage_never_follows_links_in_ancestors_or_children() {
        let (dir, paths) = fixture();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("large"), [0u8; 5000]).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("data/local.one/link")).unwrap();
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!(usage.data, 5);
        assert!(usage.partial);
        fs::remove_dir_all(dir.path().join("data/local.one")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("data/local.one")).unwrap();
        let usage = paths.measure(&AtomicBool::new(false));
        assert_eq!(usage.data, 0);
        assert!(usage.partial);
    }
}
