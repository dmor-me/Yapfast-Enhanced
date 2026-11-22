//! Give the repair/update utility the same Enhanced icon as the installer.
use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let icon = root
        .join("packaging/windows/branding/yapfast-enhanced.ico")
        .canonicalize()
        .unwrap();
    println!("cargo:rerun-if-changed={}", icon.display());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let script = out.join("enhanced.rc");
    let resource = out.join("enhanced.res");
    let icon = icon
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    fs::write(&script, format!("1 ICON \"{icon}\"\n")).unwrap();
    let status = Command::new("llvm-rc")
        .arg("-no-preprocess")
        .arg("-FO")
        .arg(&resource)
        .arg(&script)
        .status()
        .expect("llvm-rc is required to build the Windows manager icon");
    assert!(
        status.success(),
        "the Enhanced manager resource could not be compiled"
    );
    println!("cargo:rustc-link-arg-bins={}", resource.display());
}
