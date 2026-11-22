//! Enhanced settings and measurements, kept independent of upstream About.
use super::*;
use std::sync::Mutex;

const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const BUDGETS: [u32; 5] = [16, 32, 64, 128, 256];

#[derive(Default)]
pub(super) struct State {
    timer: slint::Timer,
    monitor: slint::Timer,
    checking: bool,
    checked_once: bool,
    message: String,
    page: String,
    measuring: bool,
    cache_busy: bool,
    tool_message: String,
    system: Arc<Mutex<sysinfo::System>>,
}

impl State {
    pub(super) fn timer_running(&self) -> bool {
        self.timer.running()
    }
}

pub(super) fn bind(window: &YapfastWindow, adapter: &Rc<RefCell<Adapter>>) {
    {
        let a = adapter.borrow();
        let shown = window.global::<EnhancedUpdates>();
        shown.set_enhanced_version(crate::update::enhanced_releases::VERSION.into());
        shown.set_check_enabled(a.layout.enhanced_check_updates);
        shown.set_server_memory(a.layout.release_server_memory);
        shown.set_cache_index(
            BUDGETS
                .iter()
                .position(|n| Some(*n) == a.layout.media_cache_mib)
                .unwrap_or(2) as i32,
        );
        shown.set_plugins_paused(yapfast_core::plugins::safe_mode());
        shown.set_tool_busy(a.enhanced.cache_busy);
        shown.set_tool_message(a.enhanced.tool_message.as_str().into());
        shown.set_enhanced_checking(a.enhanced.checking);
        shown.set_enhanced_message(if a.enhanced.message.is_empty() {
            "Check GitHub for Yapfast Enhanced releases.".into()
        } else {
            a.enhanced.message.as_str().into()
        });
        shown.set_enhanced_page(a.enhanced.page.as_str().into());
    }
    let shown = window.global::<EnhancedUpdates>();
    shown.on_open_github(|| browser::open("https://github.com/dmor-me/Yapfast-Enhanced"));
    let weak = Rc::downgrade(adapter);
    shown.on_enhanced_check_now(move || {
        if let Some(a) = weak.upgrade() {
            check(&a);
        }
    });
    let weak = Rc::downgrade(adapter);
    shown.on_check_toggled(move |enabled| {
        if let Some(adapter) = weak.upgrade() {
            {
                let mut a = adapter.borrow_mut();
                a.layout.enhanced_check_updates = enabled;
                a.layout.write(&a.root);
            }
            schedule(&adapter);
            if enabled {
                check(&adapter);
            }
        }
    });
    let weak = window.as_weak();
    shown.on_enhanced_view(move || {
        if let Some(window) = weak.upgrade() {
            let page = window.global::<EnhancedUpdates>().get_enhanced_page();
            if crate::update::enhanced_releases::release_page(&page) {
                browser::open(&page);
            }
        }
    });
    let weak = Rc::downgrade(adapter);
    shown.on_server_memory_toggled(move |release| {
        if let Some(adapter) = weak.upgrade() {
            let mut a = adapter.borrow_mut();
            a.layout.release_server_memory = release;
            a.layout.write(&a.root);
            if let Some(window) = a.window.upgrade() {
                window
                    .global::<EnhancedUpdates>()
                    .set_server_memory(release);
                a.action(&window, Action::ReleaseServerMemory(release));
            }
        }
    });
    let weak = Rc::downgrade(adapter);
    shown.on_cache_chosen(move |index| {
        let Some(budget) = BUDGETS.get(index as usize).copied() else {
            return;
        };
        if let Some(adapter) = weak.upgrade() {
            let mut a = adapter.borrow_mut();
            a.layout.media_cache_mib = Some(budget);
            a.layout.write(&a.root);
            if let Some(window) = a.window.upgrade() {
                window.global::<EnhancedUpdates>().set_cache_index(index);
            }
            a.media_budget(budget);
        }
    });
    let weak = Rc::downgrade(adapter);
    shown.on_release_previews(move || {
        if let Some(adapter) = weak.upgrade() {
            let mut a = adapter.borrow_mut();
            let bytes = a.release_unused_media() + pictures::release_unused_previews();
            if let Some(backend) = &a.backend {
                let _ = backend.send(Command::Discord(DiscordCommand::ReleaseUnusedPreviews));
            }
            if let Some(window) = a.window.upgrade() {
                window.global::<EnhancedUpdates>().set_tool_message(
                    format!("Released {} of preview cache references. Active views keep their images; disk files are preserved.", memory(bytes as u64)).into()
                );
            }
            pictures::prune_unused_previews();
        }
    });
    for (clean, bind) in [(false, 0), (true, 1)] {
        let weak = Rc::downgrade(adapter);
        let callback = move || {
            if let Some(a) = weak.upgrade() {
                cache_work(&a, clean);
            }
        };
        if bind == 0 {
            shown.on_scan_cache(callback);
        } else {
            shown.on_clean_cache(callback);
        }
    }
    let weak = Rc::downgrade(adapter);
    shown.on_plugin_restart(move |paused| {
        if let Some(a) = weak.upgrade() {
            a.borrow_mut().plugin_restart = Some(paused);
        }
    });
    schedule(adapter);
    let weak = Rc::downgrade(adapter);
    adapter.borrow_mut().enhanced.monitor.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(3),
        move || {
            if let Some(a) = weak.upgrade() {
                measure(&a);
            }
        },
    );
    let should_check = {
        let a = adapter.borrow();
        a.layout.enhanced_check_updates && !a.enhanced.checked_once
    };
    if should_check {
        check(adapter);
    }
}

fn schedule(adapter: &Rc<RefCell<Adapter>>) {
    let a = adapter.borrow();
    if !a.layout.enhanced_check_updates {
        a.enhanced.timer.stop();
    } else if !a.enhanced.timer.running() {
        let weak = Rc::downgrade(adapter);
        a.enhanced
            .timer
            .start(slint::TimerMode::Repeated, EVERY, move || {
                if let Some(a) = weak.upgrade() {
                    check(&a);
                }
            });
    }
}

fn check(adapter: &Rc<RefCell<Adapter>>) {
    let mut a = adapter.borrow_mut();
    if a.enhanced.checking {
        return;
    }
    a.enhanced.checking = true;
    a.enhanced.checked_once = true;
    a.enhanced.message = "Checking GitHub releases…".into();
    a.enhanced.page.clear();
    show_check(&a);
    let work = a.runtime.spawn(super::enhanced_updates::check_enhanced());
    let weak = Rc::downgrade(adapter);
    drop(a);
    if let Err(error) = slint::spawn_local(async move {
        let result = work
            .await
            .unwrap_or_else(|_| Err("The release check stopped.".into()));
        let Some(adapter) = weak.upgrade() else {
            return;
        };
        let mut a = adapter.borrow_mut();
        a.enhanced.checking = false;
        match result {
            Ok(result) => {
                a.enhanced.message = result.message;
                a.enhanced.page = result.page.unwrap_or_default();
            }
            Err(error) => a.enhanced.message = format!("Couldn't check Enhanced releases: {error}"),
        }
        show_check(&a);
    }) {
        let mut a = adapter.borrow_mut();
        a.enhanced.checking = false;
        a.enhanced.message = format!("Couldn't start the Enhanced check: {error}");
        show_check(&a);
    }
}

fn show_check(a: &Adapter) {
    if let Some(window) = a.window.upgrade() {
        let shown = window.global::<EnhancedUpdates>();
        shown.set_enhanced_checking(a.enhanced.checking);
        shown.set_enhanced_message(a.enhanced.message.as_str().into());
        shown.set_enhanced_page(a.enhanced.page.as_str().into());
    }
}

fn memory(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024. * 1024.))
}

fn measure(adapter: &Rc<RefCell<Adapter>>) {
    let mut a = adapter.borrow_mut();
    let Some(window) = a.window.upgrade() else {
        return;
    };
    if a.enhanced.measuring
        || !window.get_settings_open()
        || window.get_settings_section() != 12
        || !window.global::<EnhancedUpdates>().get_monitor_open()
    {
        return;
    }
    a.enhanced.measuring = true;
    let system = a.enhanced.system.clone();
    let work = a.runtime.spawn_blocking(move || {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
        let mut system = system.lock().unwrap_or_else(|e| e.into_inner());
        let first = system.processes().is_empty();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().without_tasks(),
        );
        let me = sysinfo::Pid::from_u32(std::process::id());
        let mut targets = vec![me];
        targets.extend(system.processes().iter().filter_map(|(pid, process)| {
            (process.parent() == Some(me)
                && process
                    .name()
                    .to_string_lossy()
                    .starts_with("yapfast-plugins"))
            .then_some(*pid)
        }));
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&targets),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_cpu()
                .with_memory(),
        );
        let cores = std::thread::available_parallelism().map_or(1., |n| n.get() as f32);
        let reading = |p: &sysinfo::Process| {
            (
                memory(p.memory()),
                if first {
                    "Sampling…".into()
                } else {
                    format!("{:.2}%", p.cpu_usage() / cores)
                },
            )
        };
        let main = system.process(me).map(reading);
        let helpers = system
            .processes()
            .values()
            .filter(|p| {
                p.parent() == Some(me) && p.name().to_string_lossy().starts_with("yapfast-plugins")
            })
            .collect::<Vec<_>>();
        let helper = if helpers.is_empty() {
            ("Not running".into(), "—".into())
        } else {
            (
                memory(helpers.iter().map(|p| p.memory()).sum()),
                if first {
                    "Sampling…".into()
                } else {
                    format!(
                        "{:.2}%",
                        helpers.iter().map(|p| p.cpu_usage()).sum::<f32>() / cores
                    )
                },
            )
        };
        (main, helper)
    });
    let weak = Rc::downgrade(adapter);
    drop(a);
    let _ = slint::spawn_local(async move {
        let result = work.await;
        let Some(adapter) = weak.upgrade() else {
            return;
        };
        let mut a = adapter.borrow_mut();
        a.enhanced.measuring = false;
        let Some(window) = a.window.upgrade() else {
            return;
        };
        let shown = window.global::<EnhancedUpdates>();
        match result {
            Ok((main, helper)) => {
                let main = main.unwrap_or(("Unavailable".into(), "Unavailable".into()));
                shown.set_process_memory(main.0.into());
                shown.set_process_cpu(main.1.into());
                shown.set_helper_memory(helper.0.into());
                shown.set_helper_cpu(helper.1.into());
            }
            Err(_) => {
                shown.set_process_memory("Unavailable".into());
                shown.set_helper_memory("Unavailable".into());
            }
        }
        let resources = a
            .app
            .state
            .plugins
            .as_ref()
            .map(|plugins| plugins.listed())
            .unwrap_or_default()
            .into_iter()
            .map(|p| EnhancedPluginResource {
                name: p.manifest.name.as_str().into(),
                status: match p.status {
                    yapfast_core::plugins::api::message::PluginStatus::Installed => "Installed",
                    yapfast_core::plugins::api::message::PluginStatus::Enabled => "Idle",
                    yapfast_core::plugins::api::message::PluginStatus::Active => "Running",
                    yapfast_core::plugins::api::message::PluginStatus::Faulted { .. } => "Faulted",
                    yapfast_core::plugins::api::message::PluginStatus::Suspended { .. } => {
                        "Suspended"
                    }
                }
                .into(),
                memory: "Unavailable".into(),
                limit: memory(p.manifest.limits.memory_pages as u64 * 65536).into(),
            })
            .collect::<Vec<_>>();
        shown.set_resources(ModelRc::new(VecModel::from(resources)));
    });
}

fn cache_work(adapter: &Rc<RefCell<Adapter>>, clean: bool) {
    let mut a = adapter.borrow_mut();
    let Some(window) = a.window.upgrade() else {
        return;
    };
    if a.enhanced.cache_busy {
        return;
    }
    a.enhanced.cache_busy = true;
    window.global::<EnhancedUpdates>().set_tool_busy(true);
    window
        .global::<EnhancedUpdates>()
        .set_tool_message("Reading media cache…".into());
    let root = a.root.clone();
    let work = a.runtime.spawn_blocking(move || disk_cache(&root, clean));
    let weak = Rc::downgrade(adapter);
    drop(a);
    let _ = slint::spawn_local(async move {
        let result = work.await;
        let Some(adapter) = weak.upgrade() else {
            return;
        };
        let mut a = adapter.borrow_mut();
        a.enhanced.cache_busy = false;
        let Some(window) = a.window.upgrade() else {
            return;
        };
        let shown = window.global::<EnhancedUpdates>();
        shown.set_tool_busy(false);
        a.enhanced.tool_message = match result {
            Ok(Ok((total, old, removed, failed))) => format!(
                "Media cache: {}. Older than 7 days: {}. Removed: {}. Files that could not be removed: {failed}.",
                memory(total),
                memory(old),
                memory(removed)
            ),
            Ok(Err(error)) => format!("Couldn't read the media cache: {error}"),
            Err(_) => "The media cache operation stopped.".into(),
        };
        shown.set_tool_message(a.enhanced.tool_message.as_str().into());
    });
}

/// Only regenerable media directories, regular files and an explicit age cutoff.
fn disk_cache(root: &std::path::Path, clean: bool) -> std::io::Result<(u64, u64, u64, usize)> {
    let root = root.canonicalize()?;
    let linked = |metadata: &std::fs::Metadata| {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            metadata.file_attributes() & 0x400 != 0
        }
        #[cfg(not(windows))]
        {
            metadata.file_type().is_symlink()
        }
    };
    let mut result = (0, 0, 0, 0);
    for name in [
        "discord-pictures",
        "discord-previews",
        "discord-gifs",
        "discord-gif-home",
    ] {
        let directory = root.join(name);
        if !directory.exists() {
            continue;
        }
        let meta = std::fs::symlink_metadata(&directory)?;
        if linked(&meta) || !directory.canonicalize()?.starts_with(&root) {
            continue;
        }
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let metadata = match std::fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if !metadata.is_file()
                || linked(&metadata)
                || entry.path().extension().is_some_and(|ext| ext == "partial")
            {
                continue;
            }
            result.0 += metadata.len();
            let old = metadata.modified()?.elapsed().unwrap_or_default()
                >= Duration::from_secs(7 * 24 * 60 * 60);
            if old {
                result.1 += metadata.len();
                if clean {
                    match std::fs::remove_file(entry.path()) {
                        Ok(()) => result.2 += metadata.len(),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(_) => result.3 += 1,
                    }
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_cleanup_removes_only_old_media_and_preserves_accounts_and_recent_files() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        let directory = root.join("discord-previews");
        std::fs::create_dir(&directory).unwrap();
        let old = directory.join("old-preview");
        std::fs::write(&old, b"old").unwrap();
        let modified = std::time::SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        std::fs::write(directory.join("recent-preview"), b"recent").unwrap();
        std::fs::write(directory.join("download.partial"), b"pending").unwrap();
        std::fs::File::options()
            .write(true)
            .open(directory.join("download.partial"))
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        std::fs::write(root.join("discord-account.json"), b"account").unwrap();
        let sizes = disk_cache(root, false).unwrap();
        assert_eq!((sizes.0, sizes.1, sizes.2), (9, 3, 0));
        let cleaned = disk_cache(root, true).unwrap();
        assert_eq!(cleaned.2, 3);
        assert!(!old.exists());
        assert!(directory.join("recent-preview").exists());
        assert!(directory.join("download.partial").exists());
        assert!(root.join("discord-account.json").exists());
    }
}
