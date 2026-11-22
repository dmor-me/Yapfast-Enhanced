#![windows_subsystem = "windows"]
use std::{ffi::c_void, os::windows::ffi::OsStrExt};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const c_void,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let graceful = args[0] == "graceful";
    let event = if graceful {
        let exe = std::env::current_exe().unwrap().canonicalize().unwrap();
        let mut hash: u64 = 0xcbf29ce484222325;
        for unit in exe.as_os_str().encode_wide() {
            let unit = char::from_u32(unit as u32).map_or(unit, |c| c.to_ascii_lowercase() as u16);
            for b in unit.to_le_bytes() {
                hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
            }
        }
        let name = format!("Local\\YapfastEnhancedQuit-{hash:016x}")
            .encode_utf16()
            .chain([0])
            .collect::<Vec<_>>();
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, name.as_ptr()) };
        assert!(!event.is_null());
        event
    } else {
        std::ptr::null_mut()
    };
    std::fs::write(&args[1], "ready").unwrap();
    if graceful {
        unsafe {
            WaitForSingleObject(event, u32::MAX);
            CloseHandle(event);
        }
    } else {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
