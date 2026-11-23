//! A storage scan belongs only to the open plugin details page. Results are
//! delivered to the UI thread, and cancelled/stale requests cannot redraw it.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use yapfast_core::plugins::storage_usage::{Paths, Usage};

pub(super) struct State {
    plugin: String,
    paths: Paths,
    cancelled: Arc<AtomicBool>,
    result: Option<Usage>,
    failed: bool,
}

impl Drop for State {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

pub(super) fn prepare(window: &YapfastWindow, page: &mut Page) {
    let selected = page
        .plugin
        .as_deref()
        .and_then(|id| page.source.view().plugin(id))
        .and_then(|plugin| {
            plugin
                .storage
                .clone()
                .map(|paths| (plugin.id.clone(), paths))
        });
    let Some((plugin, paths)) = selected else {
        page.storage = None;
        display(window, None);
        return;
    };
    if page
        .storage
        .as_ref()
        .is_some_and(|state| state.plugin == plugin && state.paths == paths)
    {
        display(window, page.storage.as_ref());
        return;
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    page.storage = Some(State {
        plugin,
        paths: paths.clone(),
        cancelled: cancelled.clone(),
        result: None,
        failed: false,
    });
    display(window, page.storage.as_ref());
    let weak = window.as_weak();
    let token = cancelled.clone();
    let worker = std::thread::Builder::new()
        .name("plugin-storage".into())
        .spawn(move || {
            let usage = paths.measure(&token);
            if token.load(Ordering::Relaxed) {
                return;
            }
            let _ = weak.upgrade_in_event_loop(move |window| {
                PAGE.with_borrow_mut(|page| {
                    let Some(page) = page.as_mut().filter(|page| page.open) else {
                        return;
                    };
                    let Some(state) = page.storage.as_mut() else {
                        return;
                    };
                    if !Arc::ptr_eq(&state.cancelled, &token)
                        || page.plugin.as_deref() != Some(&state.plugin)
                    {
                        return;
                    }
                    state.result = Some(usage);
                    display(&window, Some(state));
                });
            });
        });
    if worker.is_err() {
        if let Some(state) = page.storage.as_mut() {
            state.failed = true;
        }
        display(window, page.storage.as_ref());
    }
}

fn display(window: &YapfastWindow, state: Option<&State>) {
    let global = window.global::<PluginPage>();
    let usage = state.and_then(|state| state.result.as_ref());
    let busy = state.is_some_and(|state| state.result.is_none() && !state.failed);
    global.set_storage_busy(busy);
    let value = |bytes: Option<u64>| -> SharedString {
        bytes
            .map(page::bytes)
            .unwrap_or_else(|| if busy { "Measuring…" } else { "Unavailable" }.into())
            .into()
    };
    let total = match usage {
        Some(usage) if usage.partial || usage.external => {
            format!("At least {}", page::bytes(usage.total())).into()
        }
        _ => value(usage.map(Usage::total)),
    };
    global.set_storage_total(total);
    global.set_storage_files(if usage.is_some_and(|usage| usage.external) {
        "External folder (excluded)".into()
    } else {
        value(usage.map(|usage| usage.plugin_files))
    });
    global.set_storage_data(value(usage.map(|usage| usage.data)));
    global.set_storage_rollback(value(usage.map(|usage| usage.rollback)));
    let mut note = String::from(
        "File sizes, not RAM. Compressed disk allocation may differ. Saved data includes any cache the plugin stores privately. ",
    );
    if let Some(usage) = usage {
        if usage.partial {
            note.push_str(" Some files were skipped, unreadable or exceeded the scan limit; amounts are lower bounds. Refresh to try again.");
        }
        if usage.external {
            note.push_str(" External folder-plugin code is not included in this total.");
        }
    } else if !busy {
        note = if state.is_some() {
            "Storage could not be measured. Refresh to try again."
        } else {
            "Storage information is unavailable for this preview or plugin source."
        }
        .into();
    }
    global.set_storage_note(note.into());
}
#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State {
            plugin: "local.one".into(),
            paths: Paths::new(
                std::path::Path::new("unused-test-root"),
                &yapfast_core::plugins::api::PluginId::parse("local.one").unwrap(),
                &yapfast_core::plugins::store::LockEntry::default(),
            ),
            cancelled: Arc::new(AtomicBool::new(false)),
            result: Some(Usage {
                plugin_files: 1024,
                data: 2048,
                rollback: 4096,
                partial: false,
                external: false,
            }),
            failed: false,
        }
    }

    #[test]
    fn storage_rows_keep_shared_cache_out_of_total() {
        i_slint_backend_testing::init_no_event_loop();
        let window = YapfastWindow::new().unwrap();
        let state = state();
        window
            .global::<PluginPage>()
            .set_storage_shared("Shared cache unchanged".into());
        display(&window, Some(&state));
        let page = window.global::<PluginPage>();
        assert_eq!(page.get_storage_total(), page::bytes(7168));
        assert_eq!(page.get_storage_files(), page::bytes(1024));
        assert_eq!(page.get_storage_data(), page::bytes(2048));
        assert_eq!(page.get_storage_rollback(), page::bytes(4096));
        assert_eq!(page.get_storage_shared(), "Shared cache unchanged");
        assert!(!page.get_storage_busy());
    }

    #[test]
    fn storage_rows_show_partial_and_unavailable_results_honestly() {
        i_slint_backend_testing::init_no_event_loop();
        let window = YapfastWindow::new().unwrap();
        let mut state = state();
        state.result.as_mut().unwrap().partial = true;
        display(&window, Some(&state));
        let page = window.global::<PluginPage>();
        assert!(page.get_storage_total().starts_with("At least "));
        assert!(page.get_storage_note().contains("lower bounds"));
        state.result = None;
        state.failed = true;
        display(&window, Some(&state));
        assert_eq!(page.get_storage_total(), "Unavailable");
        assert!(!page.get_storage_busy());
        assert!(page.get_storage_note().contains("Refresh"));
    }

    #[test]
    fn dropping_storage_snapshot_cancels_its_worker() {
        let state = state();
        let token = state.cancelled.clone();
        assert!(!token.load(Ordering::Relaxed));
        drop(state);
        assert!(token.load(Ordering::Relaxed));
    }
}
