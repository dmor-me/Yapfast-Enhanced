//! Thread-scoped Windows hook. No executable rewriting or private function offsets.
use super::*;
use serde_json::json;
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::*, Threading::*},
    UI::WindowsAndMessaging::*,
};

const CLASS: &str = "YapfastEnhancedPrototypeToolbarV1";
const BUTTON: usize = 61401;
static READY: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
struct Ui {
    parent: HWND,
    window: HWND,
    button: HWND,
    config: Config,
    receiver: Option<std::sync::mpsc::Receiver<String>>,
}
thread_local! { static UI:RefCell<Option<Ui>>=const {RefCell::new(None)}; }
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

unsafe extern "system" fn find_window(hwnd: HWND, data: LPARAM) -> i32 {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if pid == unsafe { GetCurrentProcessId() }
        && unsafe { IsWindowVisible(hwnd) } != 0
        && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null()
        && unsafe { GetClientRect(hwnd, &mut rect) } != 0
        && rect.right >= 400
        && rect.bottom >= 250
    {
        unsafe {
            *(data as *mut HWND) = hwnd;
        }
        return 0;
    }
    1
}

fn initialize() -> Result<()> {
    let config_path = std::env::var_os(CONFIG_ENV)
        .context("This process was not launched by the Enhanced prototype")?;
    let nonce = std::env::var(NONCE_ENV)?;
    let config = Config::read(Path::new(&config_path), &nonce)?;
    if std::env::current_exe()?.canonicalize()? != config.app.canonicalize()? {
        bail!("Module target does not match its launch configuration")
    }
    unsafe {
        let mut parent: HWND = null_mut();
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(find_window),
            &mut parent as *mut HWND as LPARAM,
        );
        if parent.is_null() {
            bail!("No eligible Yapfast window on this thread")
        }
        let mut instance = null_mut();
        // Window procedures must remain valid even if the launcher closes.
        // Pin for this process lifetime only. Restarting the app unloads it.
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            yfe_get_message_hook as *const u16,
            &mut instance,
        ) == 0
        {
            bail!("Could not retain module while its window procedures are registered")
        }
        let class = wide(CLASS);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Enhanced loader prototype").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
            0,
            0,
            260,
            72,
            parent,
            null_mut(),
            instance,
            null(),
        );
        if window.is_null() {
            bail!(
                "Could not create module toolbar: {}",
                std::io::Error::last_os_error()
            )
        }
        CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("Enhanced loader prototype").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            8,
            5,
            244,
            20,
            window,
            null_mut(),
            instance,
            null(),
        );
        let button = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("Run Ping plugin").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            8,
            28,
            244,
            33,
            window,
            BUTTON as HMENU,
            instance,
            null(),
        );
        if button.is_null() {
            DestroyWindow(window);
            bail!("Could not create plugin command button")
        }
        UI.with(|ui| {
            *ui.borrow_mut() = Some(Ui {
                parent,
                window,
                button,
                config: config.clone(),
                receiver: None,
            })
        });
        if SetTimer(window, 1, 500, None) == 0 {
            bail!("Could not start toolbar layout timer")
        }
        place(window, parent);
        config.report("module-ready.json",&json!({"schema":1,"nonce":nonce,"pid":GetCurrentProcessId(),"module_abi":MODULE_ABI,"toolbar_hwnd":window as usize,"button_hwnd":button as usize,"capabilities":["ui.win32_child_toolbar","plugin.command.capability_free"],"chat_integration":false,"memory_tools":false}))?;
    }
    READY.store(true, Ordering::Release);
    Ok(())
}

unsafe fn place(window: HWND, parent: HWND) {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        if GetClientRect(parent, &mut rect) != 0 {
            SetWindowPos(
                window,
                HWND_TOP,
                (rect.right - 276).max(0),
                (rect.bottom - 156).max(0),
                260,
                72,
                SWP_NOACTIVATE,
            );
        }
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // No panic may unwind into the OS window procedure.
    let result = std::panic::catch_unwind(|| unsafe {
        match msg {
            WM_COMMAND if w & 0xffff == BUTTON => {
                UI.with(|slot| {
                    let mut slot=slot.borrow_mut();let Some(ui)=slot.as_mut() else{return};
                    if ui.receiver.is_some(){return}
                    SetWindowTextW(ui.button,wide("Running real WebAssembly...").as_ptr());
                    let config=ui.config.clone();
                    let _=std::fs::remove_file(config.session.join("ping-result.json"));
                    let (tx,rx)=std::sync::mpsc::sync_channel(1);ui.receiver=Some(rx);
                    std::thread::spawn(move||{
                        let result=runtime::ping(&config);let ok=result.is_ok();
                        let text=match result {Ok(text)=>text,Err(error)=>format!("{error:#}")};
                        let _=config.report("ping-result.json",&json!({"schema":1,"nonce":config.nonce,"ok":ok,"text":text,"runtime":"existing_extism_helper","privileged_host_calls":"denied"}));
                        let _=tx.send(if ok {format!("Ping: {text}")}else{"Ping failed (see report)".into()});
                    });
                });
                0
            }
            WM_TIMER => {
                UI.with(|slot| {
                    let mut slot = slot.borrow_mut();
                    if let Some(ui) = slot.as_mut() {
                        place(ui.window, ui.parent);
                        if let Some(rx) = &ui.receiver
                            && let Ok(text) = rx.try_recv()
                        {
                            SetWindowTextW(ui.button, wide(&text).as_ptr());
                            ui.receiver = None;
                        }
                    }
                });
                0
            }
            WM_CTLCOLORSTATIC => {
                SetTextColor(w as HDC, 0x0047bfff);
                SetBkMode(w as HDC, TRANSPARENT as i32);
                GetStockObject(BLACK_BRUSH) as LRESULT
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                READY.store(false, Ordering::Release);
                STARTED.store(false, Ordering::Release);
                0
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    });
    result.unwrap_or_else(|_| unsafe { DefWindowProcW(hwnd, msg, w, l) })
}

/// The loader scopes this to the UI thread of the child it just launched.
/// Message contents and keyboard input are never read or recorded.
///
/// # Safety
/// Only Windows may invoke this as a WH_GETMESSAGE callback. Arguments must be
/// the values Windows supplies for the current thread's hook chain.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn yfe_get_message_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 && !READY.load(Ordering::Acquire) && !STARTED.swap(true, Ordering::AcqRel) {
        let result = std::panic::catch_unwind(initialize);
        if let Ok(Err(error)) = &result
            && let (Some(path), Ok(nonce)) =
                (std::env::var_os(CONFIG_ENV), std::env::var(NONCE_ENV))
            && let Ok(config) = Config::read(Path::new(&path), &nonce)
        {
            let _ = config.report(
                "module-error.json",
                &json!({"error":format!("{error:#}"),"nonce":nonce}),
            );
        }
        if !matches!(result, Ok(Ok(()))) {
            // Initialization may precede a main window. Retry on a later message.
            STARTED.store(false, Ordering::Release);
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}
