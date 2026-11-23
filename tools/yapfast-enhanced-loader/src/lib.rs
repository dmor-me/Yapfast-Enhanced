//! Experimental module loader. No private Rust/Slint layout is accessed.
//! The runtime protocol is the existing Yapfast plugin API, not another engine.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const MODULE_ABI: u32 = 1;
pub const CONFIG_ENV: &str = "YFE_LOADER_CONFIG";
pub const NONCE_ENV: &str = "YFE_LOADER_NONCE";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub schema: u32,
    pub nonce: String,
    pub app: PathBuf,
    pub helper: PathBuf,
    pub helper_sha256: String,
    pub wasm: PathBuf,
    pub wasm_sha256: String,
    pub session: PathBuf,
}

impl Config {
    pub fn read(path: &Path, nonce: &str) -> Result<Self> {
        let mut file = File::open(path)?;
        if file.metadata()?.len() > 64 * 1024 {
            bail!("Module configuration exceeds 64 KiB")
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let value: Self = serde_json::from_slice(&bytes)?;
        if value.schema != MODULE_ABI || value.nonce != nonce || nonce.len() < 24 {
            bail!("Module ABI or launch-session identity does not match")
        }
        Ok(value)
    }

    pub fn report(&self, name: &str, body: &serde_json::Value) -> Result<()> {
        let allowed = ["module-ready.json", "ping-result.json", "module-error.json"];
        if !allowed.contains(&name) {
            bail!("Unknown report name")
        }
        // Session directory is new and explicit, outside the user's real profile.
        let path = self.session.join(name);
        let temp = self.session.join(format!("{name}.part"));
        std::fs::write(&temp, serde_json::to_vec_pretty(body)?)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(temp, path)?;
        Ok(())
    }
}

pub fn sha256(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let len = input.read(&mut buf)?;
        if len == 0 {
            break;
        }
        hasher.update(&buf[..len]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn x64_pe(path: &Path, dll: bool) -> Result<()> {
    let mut file =
        File::open(path).with_context(|| format!("Could not open {}", path.display()))?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)?;
    if &dos[..2] != b"MZ" {
        bail!("Expected a Windows PE executable")
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into()?);
    if offset as u64 + 24 > file.metadata()?.len() {
        bail!("Truncated PE header")
    }
    file.seek(SeekFrom::Start(offset as u64))?;
    let mut pe = [0u8; 24];
    file.read_exact(&mut pe)?;
    if &pe[..6] != b"PE\0\0\x64\x86" {
        bail!("Only Windows x64 is supported")
    }
    let is_dll = u16::from_le_bytes(pe[22..24].try_into()?) & 0x2000 != 0;
    if dll != is_dll {
        bail!("Wrong PE image kind")
    }
    Ok(())
}

#[cfg(windows)]
mod native;
pub mod runtime;

#[unsafe(no_mangle)]
pub extern "C" fn yfe_module_abi() -> u32 {
    MODULE_ABI
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn folder() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "yfe-unit-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn configuration_rejects_wrong_abi_and_session() {
        let dir = folder();
        let path = dir.join("config.json");
        let cfg = Config {
            schema: 1,
            nonce: "a".repeat(32),
            app: "app".into(),
            helper: "helper".into(),
            helper_sha256: String::new(),
            wasm: "wasm".into(),
            wasm_sha256: String::new(),
            session: dir.clone(),
        };
        std::fs::write(&path, serde_json::to_vec(&cfg).unwrap()).unwrap();
        assert!(Config::read(&path, &cfg.nonce).is_ok());
        assert!(Config::read(&path, &"b".repeat(32)).is_err());
        let mut bad = cfg;
        bad.schema = 2;
        std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(Config::read(&path, &bad.nonce).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn pe_check_rejects_wrong_architecture_and_image_kind() {
        let dir = folder();
        let path = dir.join("test.exe");
        let mut pe = vec![0u8; 88];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        std::fs::write(&path, &pe).unwrap();
        assert!(x64_pe(&path, false).is_ok());
        assert!(x64_pe(&path, true).is_err());
        pe[69] = 0;
        std::fs::write(&path, &pe).unwrap();
        assert!(x64_pe(&path, false).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
