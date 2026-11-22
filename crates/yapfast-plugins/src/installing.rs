//! Asking the helper to read a plugin out of its repository, and turning what
//! it found into a review and a package the store can write.
//!
//! The reading happens in `yapfast-plugins fetch`, which runs for seconds in a
//! process of its own and prints one JSON object ([`Report`]); this side copies
//! the module and the icon it names out of the helper's scratch directory and
//! deletes that directory, so neither side leaves anything behind. Nothing here
//! runs the plugin: the manifest is parsed as JSON, and the module was already
//! checked against the ABI by the fetch that read it (`docs/plugins/host.md`).

use std::path::Path;

use serde::Deserialize;
use yapfast_plugin_api::PluginManifest;

use crate::page::{Dependency, Review};
use crate::spawn;
use crate::store::Package;

/// What a fetch found: one plugin, or the folders of a repository that holds
/// several.
#[derive(Debug)]
pub enum Outcome {
    Plugin(Fetched),
    /// A repository whose index lists several plugins: each is fetched by its
    /// own `#path=` URL, so nothing is installed for this answer.
    Index {
        source: String,
        paths: Vec<String>,
    },
}

/// One plugin, read out of its repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetched {
    /// The id its manifest declares, which is what the store files it under.
    pub id: yapfast_plugin_api::PluginId,
    pub source: String,
    /// The folder in it, empty when the manifest is at the repository's root.
    pub path: String,
    pub reference: String,
    pub rev: String,
    /// `yapfast-plugin.json`'s text, exactly as the repository holds it.
    pub manifest: String,
    /// The module, `None` for a plugin that only changes settings or the
    /// interface.
    pub module: Option<Vec<u8>>,
    pub icon: Option<Vec<u8>>,
}

/// What the helper prints, one object tagged by `kind`.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Report {
    Plugin {
        source: String,
        path: String,
        reference: String,
        rev: String,
        manifest: String,
        wasm: Option<String>,
        icon: Option<String>,
    },
    Index {
        source: String,
        paths: Vec<String>,
    },
    Error {
        error: String,
    },
}

impl Fetched {
    /// What the review dialog shows: the manifest's own words, and what the
    /// plugin may do.
    pub fn review(&self) -> Review {
        let manifest = PluginManifest::parse(&self.manifest).ok();
        Review {
            id: self.id.to_string(),
            name: manifest
                .as_ref()
                .map(|manifest| manifest.name.clone())
                .unwrap_or_default(),
            version: manifest
                .as_ref()
                .map(|manifest| manifest.version.to_string())
                .unwrap_or_default(),
            icon: None,
            source: self.source.clone(),
            path: self.path.clone(),
            commit: self.rev.clone(),
            // Only the marketplace's own entries are reviewed; a URL install is
            // an unreviewed source.
            reviewed: false,
            capabilities: manifest
                .as_ref()
                .map(|manifest| {
                    manifest
                        .permissions
                        .capabilities
                        .iter()
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            network: manifest
                .as_ref()
                .map(|manifest| {
                    manifest
                        .permissions
                        .network
                        .iter()
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            memory_pages: manifest
                .as_ref()
                .map(|manifest| manifest.limits.memory_pages)
                .unwrap_or_default(),
            dependencies: dependencies(manifest.as_ref()),
            replaces: None,
        }
    }

    /// What the store writes, once the person has accepted the review.
    pub fn package(&self) -> Package {
        Package {
            id: self.id.clone(),
            source: self.source.clone(),
            path: self.path.clone(),
            reference: if self.reference.is_empty() {
                "HEAD".to_owned()
            } else {
                self.reference.clone()
            },
            rev: self.rev.clone(),
            manifest: self.manifest.clone(),
            module: self.module.clone().unwrap_or_default(),
            icon: self.icon.clone(),
        }
    }
}

/// What a plugin's manifest requires of others, for the review to list.
fn dependencies(manifest: Option<&PluginManifest>) -> Vec<Dependency> {
    manifest
        .map(|manifest| {
            manifest
                .dependencies
                .iter()
                .map(|(id, requirement)| Dependency {
                    name: id.to_string(),
                    version: requirement.to_string(),
                    installed: false,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reads a plugin from a file on this computer: a `.wasm`, or the
/// `yapfast-plugin.json` beside it, for trying one without a repository.
///
/// The manifest is what the plugin is — its id, its commands, its settings,
/// what it may reach, and when it runs — so it is what decides what gets
/// installed. It may be beside the file, which is how a plugin's folder is laid
/// out, or in a folder above it: a built module usually sits in
/// `target/wasm32-unknown-unknown/release`, two folders under its own. A module
/// with no manifest anywhere is refused rather than installed as something
/// nothing can call. A manifest without `wasm` is a declarative plugin; no
/// module is read for it.
pub fn from_wasm(path: &Path) -> Result<Fetched, String> {
    read_plugin(path)
}

/// Reads a plugin out of a folder: its `yapfast-plugin.json`, and the module
/// that manifest names.
///
/// A folder is what a plugin's author has open, so this is also what the
/// development loop reads again when it reloads one. Its `source` is the folder,
/// which is what keeps two folders that declare one id from replacing each
/// other: the store refuses the second.
pub fn from_folder(folder: &Path) -> Result<Fetched, String> {
    let manifest = folder.join(crate::MANIFEST);
    if !manifest.is_file() {
        return Err(missing_manifest(folder));
    }
    read_plugin(&manifest)
}

fn read_plugin(path: &Path) -> Result<Fetched, String> {
    let manifest_path = manifest_for(path).ok_or_else(|| missing_manifest(path))?;
    let manifest_path = manifest_path
        .canonicalize()
        .map_err(|error| format!("could not locate {}: {error}", manifest_path.display()))?;
    let folder = manifest_path.parent().ok_or("the manifest has no folder")?;
    let text = String::from_utf8(read_limited(&manifest_path, 1 << 20)?)
        .map_err(|error| format!("{} isn't UTF-8: {error}", manifest_path.display()))?;
    let manifest = PluginManifest::parse(&text)
        .map_err(|error| format!("{} can't be used: {error}", manifest_path.display()))?;
    // No module means a declarative plugin. Never treat the manifest itself,
    // or an unrelated file the person picked, as executable bytes.
    let module = manifest
        .wasm
        .as_deref()
        .map(|name| {
            let file = local_asset(folder, name)?;
            let bytes = read_limited(&file, yapfast_plugin_api::limits::MAX_WASM_BYTES)?;
            if !bytes.starts_with(b"\0asm\x01\0\0\0") {
                return Err(format!(
                    "{} isn't a WebAssembly module: expected a version 1 header",
                    file.display()
                ));
            }
            Ok(bytes)
        })
        .transpose()?;
    let icon = manifest
        .icon
        .as_deref()
        .map(|name| {
            let file = local_asset(folder, name)?;
            read_limited(&file, yapfast_plugin_api::limits::MAX_ICON_BYTES)
        })
        .transpose()?;
    Ok(Fetched {
        id: manifest.id,
        source: format!("file://{}", folder.display()),
        path: String::new(),
        reference: String::new(),
        rev: String::new(),
        manifest: text,
        module,
        icon,
    })
}

/// Canonical paths catch a symlink or Windows junction that leaves the folder.
/// This is an import check, not protection against a local process racing it.
fn local_asset(folder: &Path, name: &Path) -> Result<std::path::PathBuf, String> {
    let file = folder
        .join(name)
        .canonicalize()
        .map_err(|error| format!("could not locate {}: {error}", folder.join(name).display()))?;
    if !file.starts_with(folder) {
        return Err(format!(
            "{} points outside the plugin folder",
            name.display()
        ));
    }
    Ok(file)
}

/// Bound the actual read as well as metadata, even if a file grows mid-import.
fn read_limited(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} isn't a regular file", path.display()));
    }
    if metadata.len() > maximum {
        return Err(format!(
            "{} exceeds the {maximum}-byte import limit",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if bytes.len() as u64 > maximum {
        return Err(format!(
            "{} exceeds the {maximum}-byte import limit",
            path.display()
        ));
    }
    Ok(bytes)
}

/// How far above a picked module its manifest is looked for: a plugin's folder,
/// then the build tree it was built in.
const MANIFEST_SEARCH: usize = 6;

/// The manifest that describes `picked`: itself when it is one, then the one in
/// its folder, then the ones above.
fn manifest_for(picked: &Path) -> Option<std::path::PathBuf> {
    if picked
        .extension()
        .is_some_and(|extension| extension == "json")
        && picked.is_file()
    {
        return Some(picked.to_path_buf());
    }
    picked
        .parent()?
        .ancestors()
        .take(MANIFEST_SEARCH)
        .map(|folder| folder.join(crate::MANIFEST))
        .find(|candidate| candidate.is_file())
}

/// Why a file on its own can't be installed, in the words of someone reading it
/// for the first time.
fn missing_manifest(picked: &Path) -> String {
    format!(
        "There's no yapfast-plugin.json for {} — not in its folder, and not in the folders above it. \
         The manifest is what says what a plugin is called, which commands it answers and what it may \
         reach, so a module on its own can't be one. Build the plugin the way its README says, or \
         choose the plugin.wasm in the folder that holds its yapfast-plugin.json.",
        picked.display()
    )
}

/// Reads one plugin out of its repository with the helper.
pub fn fetch(url: &str) -> Result<Outcome, String> {
    let program = spawn::program().map_err(|error| format!("{error:#}"))?;
    let output = spawn::background_command(&program)
        .arg("fetch")
        .arg(url)
        .output()
        .map_err(|error| format!("could not start {}: {error}", program.display()))?;
    let printed = String::from_utf8_lossy(&output.stdout);
    let report: Report = serde_json::from_str(&printed).map_err(|_| {
        // The helper prints an error object for what it could not fetch, so a
        // line that isn't one comes from the helper failing to start at all.
        if output.status.success() {
            "the plugin helper printed something that isn't a fetch report".to_owned()
        } else {
            "the plugin helper couldn't fetch that repository".to_owned()
        }
    })?;
    match report {
        Report::Error { error } => Err(error),
        Report::Index { source, paths } => Ok(Outcome::Index { source, paths }),
        Report::Plugin {
            source,
            path,
            reference,
            rev,
            manifest,
            wasm,
            icon,
        } => {
            // The scratch directory held the module and the icon; nothing else
            // needs it once they are here, whoever named the two files.
            let scratch = wasm.clone().or_else(|| icon.clone());
            let module = match &wasm {
                Some(file) => Some(read_and_remove(file)?),
                None => None,
            };
            let icon = match &icon {
                Some(file) => Some(read_and_remove(file)?),
                None => None,
            };
            if let Some(file) = &scratch {
                remove_scratch(file);
            }
            let id = PluginManifest::parse(&manifest)
                .map(|manifest| manifest.id)
                .map_err(|error| format!("its yapfast-plugin.json can't be used: {error}"))?;
            Ok(Outcome::Plugin(Fetched {
                id,
                source,
                path,
                reference,
                rev,
                manifest,
                module,
                icon,
            }))
        }
    }
}

/// One file the helper wrote, and its removal.
fn read_and_remove(file: &str) -> Result<Vec<u8>, String> {
    std::fs::read(file).map_err(|error| format!("could not read {file}: {error}"))
}

/// Removes the directory the helper wrote a file into: its parent is the scratch
/// directory, and the helper leaves it for this side to delete.
fn remove_scratch(file: &str) {
    let Some(directory) = Path::new(file).parent() else {
        return;
    };
    if directory
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".yapfast-plugin-"))
        || directory.join("plugin.wasm").exists()
        || directory.join("yapfast-plugin.json").exists()
    {
        let _ = std::fs::remove_dir_all(directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_is_read_as_a_review() {
        let fetched = Fetched {
            id: yapfast_plugin_api::PluginId::parse("dev.local.echo").expect("a plugin id"),
            source: "https://example.org/ada/echo".to_owned(),
            path: "plugins/echo".to_owned(),
            reference: "main".to_owned(),
            rev: "0".repeat(40),
            manifest: r#"{
                "api": 1,
                "id": "dev.local.echo",
                "name": "Echo",
                "version": "0.1.0",
                "wasm": "plugin.wasm",
                "limits": { "memory_pages": 64 },
                "permissions": { "capabilities": ["messages.read"], "network": ["api.example.com"] }
            }"#
            .to_owned(),
            module: Some(vec![0, 97, 115, 109]),
            icon: None,
        };
        let review = fetched.review();
        assert_eq!(review.id, "dev.local.echo");
        assert_eq!(review.version, "0.1.0");
        assert_eq!(review.memory_pages, 64);
        assert_eq!(review.capabilities, ["messages.read"]);
        assert_eq!(review.network, ["api.example.com"]);
        // A URL install is never the marketplace's reviewed listing.
        assert!(!review.reviewed);
        assert_eq!(review.path, "plugins/echo");
        assert_eq!(fetched.package().path, "plugins/echo");
        assert_eq!(fetched.package().module.len(), 4);
    }

    #[test]
    fn a_manifest_that_makes_no_sense_does_not_panic_the_review() {
        let fetched = Fetched {
            manifest: "{".to_owned(),
            ..Fetched {
                id: yapfast_plugin_api::PluginId::parse("dev.local.echo").expect("a plugin id"),
                source: "https://example.org/x".to_owned(),
                path: String::new(),
                reference: String::new(),
                rev: "0".repeat(40),
                manifest: String::new(),
                module: None,
                icon: None,
            }
        };
        let review = fetched.review();
        // The id came from the manifest the helper validated, so the review still
        // knows which plugin it is about.
        assert_eq!(review.id, "dev.local.echo");
        assert!(review.capabilities.is_empty());
    }
    fn local_manifest(folder: &Path, fields: &str) {
        std::fs::write(
            folder.join(crate::MANIFEST),
            format!(
                r#"{{"api":1,"id":"local.import","name":"Local import","version":"1.0.0"{fields}}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn local_declarative_plugins_need_no_module() {
        let folder = tempfile::tempdir().unwrap();
        local_manifest(
            folder.path(),
            r#", "settings":[{"key":"enabled","type":"bool","label":"Enabled"}]"#,
        );
        let fetched = from_folder(folder.path()).unwrap();
        assert!(fetched.module.is_none());
        assert!(fetched.package().module.is_empty());
    }

    #[test]
    fn local_import_reads_the_declared_module_and_icon() {
        let folder = tempfile::tempdir().unwrap();
        local_manifest(folder.path(), r#", "wasm":"named.wasm", "icon":"icon.png""#);
        std::fs::write(folder.path().join("named.wasm"), b"\0asm\x01\0\0\0").unwrap();
        std::fs::write(folder.path().join("icon.png"), b"icon bytes").unwrap();
        let fetched = from_wasm(&folder.path().join(crate::MANIFEST)).unwrap();
        assert_eq!(
            fetched.module.as_deref(),
            Some(b"\0asm\x01\0\0\0".as_slice())
        );
        assert_eq!(fetched.icon.as_deref(), Some(b"icon bytes".as_slice()));
    }

    #[test]
    fn local_import_refuses_truncated_wasm_and_oversized_files() {
        let folder = tempfile::tempdir().unwrap();
        local_manifest(folder.path(), r#", "wasm":"plugin.wasm""#);
        let wasm = folder.path().join("plugin.wasm");
        std::fs::write(&wasm, b"\0asm").unwrap();
        assert!(
            from_folder(folder.path())
                .unwrap_err()
                .contains("version 1")
        );
        std::fs::File::create(&wasm)
            .unwrap()
            .set_len(yapfast_plugin_api::limits::MAX_WASM_BYTES + 1)
            .unwrap();
        assert!(
            from_folder(folder.path())
                .unwrap_err()
                .contains("import limit")
        );
        std::fs::File::create(folder.path().join(crate::MANIFEST))
            .unwrap()
            .set_len((1 << 20) + 1)
            .unwrap();
        assert!(
            from_folder(folder.path())
                .unwrap_err()
                .contains("import limit")
        );
    }

    #[test]
    fn local_import_refuses_missing_declared_assets() {
        let folder = tempfile::tempdir().unwrap();
        local_manifest(folder.path(), r#", "wasm":"missing.wasm""#);
        assert!(
            from_folder(folder.path())
                .unwrap_err()
                .contains("missing.wasm")
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_import_refuses_symlinks_outside_the_plugin_folder() {
        let folder = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let wasm = outside.path().join("outside.wasm");
        std::fs::write(&wasm, b"\0asm\x01\0\0\0").unwrap();
        std::os::unix::fs::symlink(&wasm, folder.path().join("plugin.wasm")).unwrap();
        local_manifest(folder.path(), r#", "wasm":"plugin.wasm""#);
        assert!(
            from_folder(folder.path())
                .unwrap_err()
                .contains("outside the plugin folder")
        );
    }
}
