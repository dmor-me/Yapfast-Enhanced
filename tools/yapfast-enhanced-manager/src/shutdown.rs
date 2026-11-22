//! Bounded shutdown of only the selected installation's app/helper processes.
use super::*;

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        mem,
        os::windows::ffi::{OsStrExt, OsStringExt},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, WAIT_OBJECT_0},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
                PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
                WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct Process {
        pid: u32,
        handle: Handle,
    }
    fn same_path(left: &Path, right: &Path) -> bool {
        match (left.canonicalize(), right.canonicalize()) {
            (Ok(a), Ok(b)) => a
                .as_os_str()
                .encode_wide()
                .map(|c| if (65..=90).contains(&c) { c + 32 } else { c })
                .eq(b
                    .as_os_str()
                    .encode_wide()
                    .map(|c| if (65..=90).contains(&c) { c + 32 } else { c })),
            _ => false,
        }
    }
    fn matching(app: &Path) -> Result<Vec<Process>> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        ensure!(
            snapshot != INVALID_HANDLE_VALUE,
            "Could not inspect Yapfast processes. Quit it manually from the tray and retry."
        );
        let snapshot = Handle(snapshot);
        let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        let mut processes = Vec::new();
        let mut count = 0;
        while next != 0 && count < 16384 {
            count += 1;
            let end = entry
                .szExeFile
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if ["yapfast.exe", "yapfast-plugins.exe"]
                .iter()
                .any(|n| name.eq_ignore_ascii_case(n))
            {
                let raw = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                        0,
                        entry.th32ProcessID,
                    )
                };
                if !raw.is_null() {
                    let handle = Handle(raw);
                    let mut path = vec![0u16; 32768];
                    let mut size = path.len() as u32;
                    if unsafe {
                        QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut size)
                    } != 0
                    {
                        let path =
                            PathBuf::from(std::ffi::OsString::from_wide(&path[..size as usize]));
                        if ["yapfast.exe", "yapfast-plugins.exe"]
                            .iter()
                            .any(|n| same_path(&path, &app.join(n)))
                        {
                            processes.push(Process {
                                pid: entry.th32ProcessID,
                                handle,
                            });
                        }
                    }
                }
            }
            next = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        Ok(processes)
    }
    unsafe extern "system" fn close_window(window: HWND, data: LPARAM) -> i32 {
        let processes = unsafe { &*(data as *const Vec<Process>) };
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if processes.iter().any(|p| p.pid == pid) {
            unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
        }
        1
    }
    fn wait(app: &Path, limit: Duration) -> bool {
        let start = Instant::now();
        loop {
            if stopped(app).is_ok() {
                return true;
            }
            if start.elapsed() >= limit {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    pub fn stop(app: &Path) -> Result<String> {
        let app = app_directory(app)?;
        // Avoid Windows Restart Manager's lengthy scan even when nothing runs.
        if stopped(&app).is_ok() {
            return Ok("Yapfast is already closed and ready for installation.".into());
        }
        let _ = request_enhanced_quit(&app.join("yapfast.exe"));
        let processes = matching(&app)?;
        unsafe {
            EnumWindows(
                Some(close_window),
                &processes as *const Vec<Process> as LPARAM,
            )
        };
        if wait(&app, Duration::from_secs(5)) {
            return Ok("Yapfast closed gracefully and is ready for installation.".into());
        }
        // Setup explicitly warns to finish work before continuing. Handles are
        // opened only after comparing the full executable path; holding each
        // handle prevents PID reuse from targeting a different process.
        let remaining = matching(&app)?;
        for process in &remaining {
            if unsafe { WaitForSingleObject(process.handle.0, 0) } != WAIT_OBJECT_0 {
                ensure!(
                    unsafe { TerminateProcess(process.handle.0, 0) } != 0,
                    "Could not close Yapfast. Quit it manually from its tray icon; this may speed up setup, then retry."
                );
            }
        }
        ensure!(
            wait(&app, Duration::from_secs(2)),
            "Yapfast's files are still in use. Quit Yapfast manually, including its tray icon; this may speed up setup. Finish active work and retry."
        );
        Ok("Yapfast is closed and ready for installation.".into())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn different_installations_are_never_the_same_path() {
            let temp = tempfile::tempdir().unwrap();
            let a = temp.path().join("a");
            let b = temp.path().join("b");
            fs::create_dir(&a).unwrap();
            fs::create_dir(&b).unwrap();
            fs::write(a.join("yapfast.exe"), "same").unwrap();
            fs::write(b.join("yapfast.exe"), "same").unwrap();
            assert!(!same_path(&a.join("yapfast.exe"), &b.join("yapfast.exe")));
            assert!(same_path(&a.join("yapfast.exe"), &a.join("yapfast.exe")));
        }
    }
}
#[cfg(windows)]
pub fn stop(app: &Path) -> Result<String> {
    native::stop(app)
}

#[cfg(windows)]
fn request_enhanced_quit(exe: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> *mut std::ffi::c_void;
        fn SetEvent(event: *mut std::ffi::c_void) -> i32;
        fn CloseHandle(event: *mut std::ffi::c_void) -> i32;
    }
    let Ok(exe) = exe.canonicalize() else {
        return false;
    };
    let mut hash: u64 = 0xcbf29ce484222325;
    for unit in exe.as_os_str().encode_wide() {
        let unit = char::from_u32(unit as u32).map_or(unit, |c| c.to_ascii_lowercase() as u16);
        for byte in unit.to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
        }
    }
    let name = format!("Local\\YapfastEnhancedQuit-{hash:016x}")
        .encode_utf16()
        .chain([0])
        .collect::<Vec<_>>();
    unsafe {
        let event = OpenEventW(2, 0, name.as_ptr());
        if event.is_null() {
            return false;
        }
        let sent = SetEvent(event) != 0;
        CloseHandle(event);
        sent
    }
}

#[cfg(not(windows))]
pub fn stop(_: &Path) -> Result<String> {
    anyhow::bail!("Automatic installer shutdown is available on Windows.")
}
