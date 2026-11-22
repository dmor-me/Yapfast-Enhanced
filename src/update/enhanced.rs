//! Verify the in-place mod manager before handing it an official update.
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub fn manager(program: &Path) -> Result<Option<PathBuf>, String> {
    let folder = program
        .parent()
        .ok_or("Yapfast has no installation folder")?
        .join(".yapfast-enhanced");
    let state = folder.join("state.json");
    if !state.exists() {
        return Ok(None);
    }
    let file = std::fs::File::open(state).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > 64 << 10 {
        return Err(
            "Enhanced state is too large. Run the Enhanced installer's Repair option.".into(),
        );
    }
    let state: serde_json::Value =
        serde_json::from_reader(file.take((64 << 10) + 1)).map_err(|_| {
            "Enhanced state is damaged. Run Enhanced Repair before updating.".to_owned()
        })?;
    if state["schema"].as_u64() != Some(1) || state["phase"].as_str() != Some("installed") {
        return Err(
            "Enhanced installation is incomplete. Run Enhanced Repair before updating.".into(),
        );
    }
    let expected = state["manager_sha256"]
        .as_str()
        .filter(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or("Enhanced manager has no valid integrity pin. Run Enhanced Repair.")?;
    let manager = folder.join("manager.exe");
    let mut file = std::fs::File::open(&manager)
        .map_err(|_| "Enhanced manager is missing. Run Enhanced Repair.".to_owned())?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 << 10];
    loop {
        let length = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    if format!("{:x}", digest.finalize()) != expected {
        return Err("Enhanced manager failed its integrity check. Run Enhanced Repair.".into());
    }
    Ok(Some(manager))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_yapfast_uses_its_original_update_path() {
        let dir = tempfile::tempdir().unwrap();
        assert!(manager(&dir.path().join("yapfast.exe")).unwrap().is_none());
    }
    #[test]
    fn modified_manager_is_not_executed() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join(".yapfast-enhanced");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("manager.exe"), b"manager").unwrap();
        std::fs::write(folder.join("state.json"), serde_json::to_vec(&serde_json::json!({"schema":1,"phase":"installed","manager_sha256":"0".repeat(64)})).unwrap()).unwrap();
        assert!(
            manager(&dir.path().join("yapfast.exe"))
                .unwrap_err()
                .contains("integrity")
        );
    }
    #[test]
    fn verified_manager_is_selected_from_the_fixed_installation_path() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join(".yapfast-enhanced");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("manager.exe"), b"manager").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"manager"));
        std::fs::write(folder.join("state.json"), serde_json::to_vec(&serde_json::json!({"schema":1,"phase":"installed","manager_sha256":digest,"path":"../ignored.exe"})).unwrap()).unwrap();
        assert_eq!(
            manager(&dir.path().join("yapfast.exe")).unwrap(),
            Some(folder.join("manager.exe"))
        );
    }
}
