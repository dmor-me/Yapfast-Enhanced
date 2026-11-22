use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[cfg(windows)]
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*};
use yapfast_enhanced_module::{CONFIG_ENV, Config, MODULE_ABI, NONCE_ENV, sha256, x64_pe};

fn value(args: &[String], name: &str) -> Result<PathBuf> {
    args.windows(2)
        .find(|a| a[0] == name)
        .map(|a| PathBuf::from(&a[1]))
        .with_context(|| format!("Required: {name} PATH"))
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Enhanced loader prototype: {error:#}");
        std::process::exit(1)
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help") {
        println!(
            "Yapfast Enhanced loader prototype (Windows x64)\n\n--app PATH --runtime-helper PATH --plugin-dir PATH --state-dir NEW_DIR [--self-test] [--close-after-test]\n\nLaunches only a new isolated --preview session. Does not install, patch files, attach to an existing process, or provide chat/history APIs."
        );
        return Ok(());
    }
    #[cfg(not(windows))]
    bail!("The loader prototype requires Windows x64");
    #[cfg(windows)]
    run_windows(&args)
}

#[cfg(windows)]
unsafe extern "system" fn find_window(hwnd: HWND, data: LPARAM) -> i32 {
    let state = unsafe { &mut *(data as *mut (u32, HWND, u32)) };
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if pid == state.0
        && unsafe { IsWindowVisible(hwnd) } != 0
        && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null()
        && unsafe { GetClientRect(hwnd, &mut rect) } != 0
        && rect.right >= 400
        && rect.bottom >= 250
    {
        state.1 = hwnd;
        state.2 = thread;
        return 0;
    }
    1
}

#[cfg(windows)]
fn run_windows(args: &[String]) -> Result<()> {
    let app = value(args, "--app")?.canonicalize()?;
    if !app
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("yapfast.exe"))
    {
        bail!("The target must be an explicit yapfast.exe, never another application")
    }
    let helper = value(args, "--runtime-helper")?.canonicalize()?;
    let plugin_dir = value(args, "--plugin-dir")?.canonicalize()?;
    let wasm = plugin_dir.join("plugin.wasm").canonicalize()?;
    let plugin: serde_json::Value =
        serde_json::from_slice(&std::fs::read(plugin_dir.join("yapfast-plugin.json"))?)?;
    if plugin["id"] != "com.example.ping"
        || plugin["permissions"]["capabilities"] != json!([])
        || plugin["permissions"]["network"] != json!([])
    {
        bail!("This prototype accepts only the capability-free Ping example")
    }
    let module = std::env::current_exe()?
        .parent()
        .context("Loader has no directory")?
        .join("yapfast_enhanced_module.dll")
        .canonicalize()?;
    x64_pe(&app, false)?;
    x64_pe(&helper, false)?;
    x64_pe(&module, true)?;
    let before = sha256(&app)?;
    let session = value(args, "--state-dir")?;
    if session.exists() {
        bail!("Choose a new empty session path; existing profiles are never reused")
    }
    std::fs::create_dir_all(&session)?;
    let session = session.canonicalize()?;
    let profile = session.join("preview-profile");
    std::fs::create_dir(&profile)?;
    let nonce = format!(
        "{:032x}-{:08x}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
        std::process::id()
    );
    let config = Config {
        schema: MODULE_ABI,
        nonce: nonce.clone(),
        app: app.clone(),
        helper_sha256: sha256(&helper)?,
        helper,
        wasm_sha256: sha256(&wasm)?,
        wasm,
        session: session.clone(),
    };
    let config_path = session.join("launch.json");
    std::fs::write(&config_path, serde_json::to_vec_pretty(&config)?)?;
    let mut child = std::process::Command::new(&app)
        .args(["--preview", "--data-dir"])
        .arg(&profile)
        .env(CONFIG_ENV, &config_path)
        .env(NONCE_ENV, &nonce)
        .env("YAPFAST_LOG", "warn")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let result = (|| unsafe {
        let start = Instant::now();
        let mut found: (u32, HWND, u32) = (child.id(), std::ptr::null_mut(), 0u32);
        while found.1.is_null() && start.elapsed() < Duration::from_secs(40) {
            if let Some(code) = child.try_wait()? {
                bail!("Official app exited before its window appeared: {code}")
            }
            EnumWindows(Some(find_window), &mut found as *mut _ as LPARAM);
            pump();
            std::thread::sleep(Duration::from_millis(100));
        }
        if found.1.is_null() || found.2 == 0 {
            bail!("No new Yapfast main window appeared")
        }
        let dll = LoadLibraryW(wide(module.as_os_str()).as_ptr());
        if dll.is_null() {
            bail!("Cannot load module: {}", std::io::Error::last_os_error())
        }
        let abi = GetProcAddress(dll, c"yfe_module_abi".as_ptr() as *const u8)
            .context("Module ABI export missing")?;
        let abi: unsafe extern "C" fn() -> u32 = std::mem::transmute(abi);
        if abi() != MODULE_ABI {
            FreeLibrary(dll);
            bail!("Module ABI differs from loader")
        }
        let callback = GetProcAddress(dll, c"yfe_get_message_hook".as_ptr() as *const u8)
            .context("Module UI export missing")?;
        let hook = SetWindowsHookExW(
            WH_GETMESSAGE,
            Some(std::mem::transmute::<
                unsafe extern "system" fn() -> isize,
                unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
            >(callback)),
            dll,
            found.2,
        );
        if hook.is_null() {
            FreeLibrary(dll);
            bail!(
                "Thread-scoped module hook failed: {}",
                std::io::Error::last_os_error()
            )
        }
        let hooked = (|| -> Result<()> {
            let start = Instant::now();
            let ready_path = session.join("module-ready.json");
            while !ready_path.exists() && start.elapsed() < Duration::from_secs(20) {
                PostMessageW(found.1, WM_NULL, 0, 0);
                pump();
                std::thread::sleep(Duration::from_millis(100));
                if child.try_wait()?.is_some() {
                    bail!("App exited while loading the module")
                }
            }
            if !ready_path.exists() {
                let detail =
                    std::fs::read_to_string(session.join("module-error.json")).unwrap_or_default();
                bail!(
                    "Module did not confirm attachment; no host functionality is claimed. {detail}"
                )
            }
            let ready: serde_json::Value = serde_json::from_slice(&std::fs::read(&ready_path)?)?;
            if ready["nonce"] != nonce || ready["pid"] != child.id() {
                bail!("Module confirmation belongs to another session")
            }
            println!(
                "External module attached to the unchanged official app. Toolbar is a Win32 child overlay, not Slint/chat integration."
            );
            if args.iter().any(|a| a == "--self-test") {
                let toolbar = ready["toolbar_hwnd"]
                    .as_u64()
                    .context("Missing toolbar handle")? as HWND;
                PostMessageW(toolbar, WM_COMMAND, 61401, 0);
                let start = Instant::now();
                let result_path = session.join("ping-result.json");
                while !result_path.exists() && start.elapsed() < Duration::from_secs(40) {
                    pump();
                    std::thread::sleep(Duration::from_millis(100));
                    if child.try_wait()?.is_some() {
                        bail!("App exited during plugin test")
                    }
                }
                let report: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(&result_path)
                        .context("No real plugin result before deadline")?,
                )?;
                if report["nonce"] != nonce || report["ok"] != true || report["text"] != "pong" {
                    bail!("Real plugin test failed: {}", report["text"])
                }
                println!(
                    "PASS: the existing Extism helper executed the shipped Ping Wasm plugin and returned pong."
                );
            }
            if args.iter().any(|a| a == "--close-after-test") {
                PostMessageW(found.1, WM_CLOSE, 0, 0);
            }
            let stop = Instant::now();
            while child.try_wait()?.is_none() {
                pump();
                std::thread::sleep(Duration::from_millis(50));
                if args.iter().any(|a| a == "--close-after-test")
                    && stop.elapsed() > Duration::from_secs(5)
                {
                    child.kill()?;
                    break;
                }
            }
            let _ = child.wait();
            Ok(())
        })();
        // Destroy target-side window procedures before unhooking on a failed run.
        if hooked.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        UnhookWindowsHookEx(hook);
        FreeLibrary(dll);
        hooked
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let after = sha256(&app)?;
    let report = json!({"schema":1,"app":app,"app_sha256_before":before,"app_sha256_after":after,"file_unchanged":before==after,"module_sha256":sha256(&module)?,"module_abi":MODULE_ABI,"passed":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),"scope":"module loading and capability-free plugin command only","chat_integration":false,"memory_tools":false});
    std::fs::write(
        session.join("loader-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    if before != after {
        bail!("Target executable changed during the test")
    }
    result
}

#[cfg(windows)]
fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.encode_wide().chain(Some(0)).collect()
}
#[cfg(windows)]
unsafe fn pump() {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
