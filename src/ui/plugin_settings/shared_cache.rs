//! Shared compilation-cache storage, measured once on the Installed overview.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use yapfast_core::plugins::storage_usage::{CachePaths, CacheUsage};

pub(super) struct State {
    paths: CachePaths,
    token: Arc<AtomicBool>,
    result: Option<CacheUsage>,
    failed: bool,
}
impl Drop for State {
    fn drop(&mut self) {
        self.token.store(true, Ordering::Relaxed);
    }
}

pub(super) fn prepare(window: &YapfastWindow, page: &mut Page) {
    if page.tab != 1 || page.plugin.is_some() {
        page.shared_cache = None;
        return;
    }
    let Some(paths) = page.source.cache_paths() else {
        page.shared_cache = None;
        display(window, None);
        return;
    };
    if page
        .shared_cache
        .as_ref()
        .is_some_and(|state| state.paths == paths)
    {
        display(window, page.shared_cache.as_ref());
        return;
    }
    let token = Arc::new(AtomicBool::new(false));
    page.shared_cache = Some(State {
        paths: paths.clone(),
        token: token.clone(),
        result: None,
        failed: false,
    });
    display(window, page.shared_cache.as_ref());
    let weak = window.as_weak();
    let worker = std::thread::Builder::new()
        .name("shared-plugin-cache".into())
        .spawn(move || {
            let usage = paths.measure(&token);
            if token.load(Ordering::Relaxed) {
                return;
            }
            let _ = weak.upgrade_in_event_loop(move |window| {
                PAGE.with_borrow_mut(|page| {
                    let Some(page) = page
                        .as_mut()
                        .filter(|page| page.open && page.tab == 1 && page.plugin.is_none())
                    else {
                        return;
                    };
                    let Some(state) = page.shared_cache.as_mut() else {
                        return;
                    };
                    if !Arc::ptr_eq(&state.token, &token) {
                        return;
                    }
                    state.result = Some(usage);
                    display(&window, Some(state));
                });
            });
        });
    if worker.is_err() {
        if let Some(state) = page.shared_cache.as_mut() {
            state.failed = true;
        }
        display(window, page.shared_cache.as_ref());
    }
}

fn display(window: &YapfastWindow, state: Option<&State>) {
    let global = window.global::<PluginPage>();
    let busy = state.is_some_and(|state| state.result.is_none() && !state.failed);
    global.set_shared_cache_busy(busy);
    let mut note = String::from(
        "Prepared plugin code that helps plugins start faster. This disk cache belongs to the plugin system as a whole and is excluded from individual plugin totals. File sizes, not RAM; compressed disk use may differ.",
    );
    let text = match state.and_then(|state| state.result.as_ref()) {
        Some(usage) if usage.partial => {
            note.push_str(" Some files could not be counted or the scan limit was reached. Refresh to try again.");
            format!("At least {}", page::bytes(usage.bytes))
        }
        Some(usage) => page::bytes(usage.bytes),
        None if busy => "Measuring…".into(),
        None => {
            note.push_str(" Storage information is currently unavailable.");
            "Unavailable".into()
        }
    };
    global.set_storage_shared(text.into());
    global.set_shared_cache_note(note.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_cache_is_separate_and_partial_measurements_are_labeled() {
        i_slint_backend_testing::init_no_event_loop();
        let window = YapfastWindow::new().unwrap();
        let global = window.global::<PluginPage>();
        global.set_storage_total("Plugin total unchanged".into());
        let mut state = State {
            paths: CachePaths::new(std::path::Path::new("unused")),
            token: Arc::new(AtomicBool::new(false)),
            result: Some(CacheUsage {
                bytes: 8 * 1024 * 1024,
                partial: false,
            }),
            failed: false,
        };
        display(&window, Some(&state));
        assert_eq!(global.get_storage_shared(), "8.0 MiB");
        assert_eq!(global.get_storage_total(), "Plugin total unchanged");
        state.result.as_mut().unwrap().partial = true;
        display(&window, Some(&state));
        assert!(global.get_storage_shared().starts_with("At least "));
        assert!(!global.get_shared_cache_busy());
    }
    #[test]
    fn shared_cache_cancellation_and_unavailable_state_do_not_show_fake_zeroes() {
        i_slint_backend_testing::init_no_event_loop();
        let window = YapfastWindow::new().unwrap();
        display(&window, None);
        assert_eq!(
            window.global::<PluginPage>().get_storage_shared(),
            "Unavailable"
        );
        let state = State {
            paths: CachePaths::new(std::path::Path::new("unused")),
            token: Arc::new(AtomicBool::new(false)),
            result: None,
            failed: false,
        };
        let token = state.token.clone();
        display(&window, Some(&state));
        assert!(window.global::<PluginPage>().get_shared_cache_busy());
        drop(state);
        assert!(token.load(Ordering::Relaxed));
    }
}
