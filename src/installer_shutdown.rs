//! An installation-scoped quit event also reaches a tray-only window.
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};

pub fn listen() {
    let Ok(exe) = std::env::current_exe().and_then(|path| path.canonicalize()) else {
        return;
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
    let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, name.as_ptr()) };
    if event.is_null() {
        return;
    }
    let address = event as usize;
    if std::thread::Builder::new()
        .name("enhanced-installer".into())
        .stack_size(64 * 1024)
        .spawn(move || {
            let event = address as windows_sys::Win32::Foundation::HANDLE;
            let result = unsafe { WaitForSingleObject(event, INFINITE) };
            unsafe {
                CloseHandle(event);
            }
            if result == WAIT_OBJECT_0 {
                let _ = slint::invoke_from_event_loop(|| {
                    let _ = slint::quit_event_loop();
                });
            }
        })
        .is_err()
    {
        unsafe {
            CloseHandle(event);
        }
    }
}
