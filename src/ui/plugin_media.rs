//! Generic native media-browser rendering, grant-checked previews and chosen JSON files.
use super::*;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use yapfast_core::plugins::api::{PluginId, call::HostReturn};
use yapfast_core::plugins::media_browser::MediaBrowser;
use yapfast_core::plugins::network::{Asking, Outcome};
const CACHE_BYTES: usize = 64 << 20;
const CACHE_ENTRIES: usize = 64;
const PREVIEW_BYTES: u64 = 4 << 20;

type PreviewKey = (String, String);
type Priority = (u8, Reverse<usize>);
type MediaModel = (String, MediaBrowser, ModelRc<MediaCard>);

#[derive(Clone, Copy)]
struct Viewport {
    top: f32,
    height: f32,
    columns: usize,
}

impl Viewport {
    fn priority(self, index: usize) -> Priority {
        let tile_top = (index / self.columns) as f32 * 210.;
        let tile_bottom = tile_top + 202.;
        let bottom = self.top + self.height;
        let tier = if tile_top >= self.top && tile_bottom <= bottom {
            3
        } else if tile_top < bottom && tile_bottom > self.top {
            2
        } else if tile_top < bottom + 210. && tile_bottom > self.top - 210. {
            1
        } else {
            0
        };
        (tier, Reverse(index))
    }
}

#[derive(Clone, Copy)]
enum Attempt {
    Failed,
    Released(usize),
}

#[derive(Default)]
pub(super) struct State {
    cache: HashMap<PreviewKey, (Arc<yapfast_core::chat::Picture>, usize)>,
    pending: HashSet<PreviewKey>,
    attempts: HashMap<PreviewKey, Attempt>,
    viewport: HashMap<String, Viewport>,
    session: Option<(Arc<yapfast_core::plugins::world::World>, Option<String>)>,
    files_open: bool,
    budget: Option<usize>,
}

impl State {
    fn priorities(&self, models: &[MediaModel]) -> HashMap<PreviewKey, Priority> {
        let mut priorities = HashMap::new();
        for (plugin, browser, _) in models {
            let Some(viewport) = self.viewport.get(plugin) else {
                continue;
            };
            for (index, card) in browser.cards.iter().enumerate() {
                let priority = viewport.priority(index);
                if card.src.is_empty() || (browser.lazy && priority.0 == 0) {
                    continue;
                }
                priorities
                    .entry((plugin.clone(), card.src.clone()))
                    .and_modify(|old: &mut Priority| *old = (*old).max(priority))
                    .or_insert(priority);
            }
        }
        priorities
    }

    /// A preview may replace only lower-priority entries. Apply the same
    /// rule before retrying a released preview to avoid repeated downloads.
    fn eviction_plan<'a>(
        &'a self,
        priorities: &HashMap<PreviewKey, Priority>,
        priority: Priority,
        cost: usize,
    ) -> Option<Vec<&'a PreviewKey>> {
        let budget = self.budget.unwrap_or(CACHE_BYTES);
        if cost > budget {
            return None;
        }
        let mut bytes = cost + self.cache.values().map(|(_, bytes)| bytes).sum::<usize>();
        let mut entries = 1 + self.cache.len();
        let mut victims = Vec::new();
        if bytes <= budget && entries <= CACHE_ENTRIES {
            return Some(victims);
        }
        let mut candidates = self
            .cache
            .iter()
            .filter(|(key, _)| priorities.get(*key) < Some(&priority))
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|(key, _)| (priorities.get(*key), *key));
        for (key, (_, size)) in candidates {
            bytes -= size;
            entries -= 1;
            victims.push(key);
            if bytes <= budget && entries <= CACHE_ENTRIES {
                return Some(victims);
            }
        }
        None
    }
}
fn strings(values: impl Iterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values.map(SharedString::from).collect::<Vec<_>>(),
    ))
}
fn index(choices: &[yapfast_core::plugins::media_browser::Choice], id: &str) -> i32 {
    choices.iter().position(|c| c.id == id).unwrap_or(0) as i32
}
pub(super) fn view(browser: Option<&MediaBrowser>) -> MediaView {
    let Some(b) = browser else {
        return MediaView::default();
    };
    MediaView {
        available: true,
        title: b.title.as_str().into(),
        query: b.query.as_str().into(),
        collection_ids: strings(b.collections.iter().map(|c| c.id.clone())),
        collection_names: strings(b.collections.iter().map(|c| c.name.clone())),
        collection_index: index(&b.collections, &b.collection),
        collection_name: b.collection_name.as_str().into(),
        navigation: ModelRc::new(VecModel::from(
            b.navigation
                .iter()
                .map(|n| MediaNavigation {
                    id: n.id.as_str().into(),
                    name: n.name.as_str().into(),
                    count: n.count.min(100_000) as i32,
                    category: n.category,
                    indent: n.indent,
                    collapsed: n.collapsed,
                    editable: n.editable,
                })
                .collect::<Vec<_>>(),
        )),
        active: b.active.as_str().into(),
        cards: ModelRc::new(VecModel::from(
            b.cards
                .iter()
                .map(|c| MediaCard {
                    id: c.id.as_str().into(),
                    label: c.label.as_str().into(),
                    selected: c.selected,
                    folder_index: index(&b.folders, &c.folder),
                    note: "Loading preview…".into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )),
        folder_ids: strings(b.folders.iter().map(|c| c.id.clone())),
        folder_names: strings(b.folders.iter().map(|c| c.name.clone())),
        category_ids: strings(b.categories.iter().map(|c| c.id.clone())),
        category_names: strings(b.categories.iter().map(|c| c.name.clone())),
        destination_index: index(&b.folders, &b.destination),
        parent_index: index(&b.categories, &b.form_parent),
        total: b.total as i32,
        page: b.page as i32,
        pages: b.pages.max(1) as i32,
        selection_mode: b.selection_mode,
        selected_count: b.selected_count as i32,
        form_name: b.form_name.as_str().into(),
        form_kind: b.form_kind.as_str().into(),
        edit_id: b.edit_id.as_str().into(),
        edit_name: b.edit_name.as_str().into(),
        confirm_text: b.confirm_text.as_str().into(),
        error: b.error.as_str().into(),
        status: b.status.as_str().into(),
        virtualize: b.virtualize,
        lazy: b.lazy,
        pause_offscreen: b.pause_offscreen,
        hover_only: b.hover_only,
    }
}

pub(super) fn bind(window: &YapfastWindow, adapter: &Rc<RefCell<Adapter>>) {
    let weak = Rc::downgrade(adapter);
    window
        .global::<MediaBrowserUi>()
        .on_viewport(move |plugin, top, height, columns| {
            let weak = weak.clone();
            slint::Timer::single_shot(Duration::ZERO, move || {
                let Some(adapter) = weak.upgrade() else {
                    return;
                };
                let Ok(mut a) = adapter.try_borrow_mut() else {
                    return;
                };
                if height <= 0. {
                    return;
                }
                a.media.viewport.insert(
                    plugin.to_string(),
                    Viewport {
                        top,
                        height,
                        columns: columns.max(1) as usize,
                    },
                );
                a.update_media_previews();
            });
        });
}

impl Adapter {
    pub(super) fn media_changed(&mut self) {
        self.media.attempts.clear();
        if self
            .window
            .upgrade()
            .is_none_or(|w| w.get_plugin_dialogs().row_count() == 0)
        {
            self.media.cache.clear();
            self.media.viewport.clear();
        }
        self.update_media_previews();
    }
    fn media_models(&self) -> Vec<MediaModel> {
        let Some(window) = self.window.upgrade() else {
            return vec![];
        };
        let Some(slots) = self.app.state.plugin_slots.as_ref() else {
            return vec![];
        };
        let parts = window.get_plugin_dialogs();
        slots
            .drawn(yapfast_core::plugins::slots::Slot::Dialog)
            .filter_map(|drawn| {
                let browser = drawn.tree.media_browser.clone()?;
                let plugin = slots.names()[drawn.plugin].id.clone();
                let shown = parts.iter().find(|p| p.plugin.as_str() == plugin)?;
                Some((plugin, browser, shown.browser.cards))
            })
            .collect()
    }
    pub(super) fn media_budget(&mut self, mib: u32) {
        self.media.budget = Some(mib.clamp(16, 256) as usize * 1024 * 1024);
        let priorities = self.media.priorities(&self.media_models());
        while self
            .media
            .cache
            .values()
            .map(|(_, bytes)| bytes)
            .sum::<usize>()
            > self.media.budget.unwrap()
        {
            let Some(key) = self
                .media
                .cache
                .keys()
                .min_by_key(|key| priorities.get(*key))
                .cloned()
            else {
                break;
            };
            if let Some((_, cost)) = self.media.cache.remove(&key) {
                self.clear_media_picture(&key);
                self.media.attempts.insert(key, Attempt::Released(cost));
            }
        }
        self.update_media_previews();
    }

    pub(super) fn release_unused_media(&mut self) -> usize {
        let priorities = self.media.priorities(&self.media_models());
        let keys = self
            .media
            .cache
            .keys()
            .filter(|key| priorities.get(*key).is_none_or(|priority| priority.0 < 2))
            .cloned()
            .collect::<Vec<_>>();
        let mut released = 0;
        for key in keys {
            if let Some((_, cost)) = self.media.cache.remove(&key) {
                released += cost;
                self.clear_media_picture(&key);
                self.media.attempts.insert(key, Attempt::Released(cost));
            }
        }
        released
    }

    fn update_media_previews(&mut self) {
        let Some(state) = self.app.state.plugins.as_ref() else {
            return;
        };
        let gate = state.network_gate();
        let world = state.world().clone();
        let account = world.me().map(|m| m.id);
        if self
            .media
            .session
            .as_ref()
            .is_none_or(|(old, id)| !Arc::ptr_eq(old, &world) || *id != account)
        {
            self.media.cache.clear();
            self.media.pending.clear();
            self.media.attempts.clear();
            self.media.session = Some((world.clone(), account.clone()));
        }
        let models = self.media_models();
        let priorities = self.media.priorities(&models);
        for (plugin, browser, model) in models {
            for (i, card) in browser.cards.iter().enumerate() {
                let key = (plugin.clone(), card.src.clone());
                if let Some((picture, _)) = self.media.cache.get(&key) {
                    if let Some(mut row) = model.row_data(i) {
                        if !row.note.is_empty() {
                            row.picture = super::pictures::picture(picture);
                            let (frames, delays) = super::pictures::animation(picture);
                            row.frames = frames;
                            row.delays = delays;
                            row.note = "".into();
                            model.set_row_data(i, row);
                        }
                    }
                    continue;
                }
                if card.src.is_empty() {
                    if let Some(mut row) = model.row_data(i) {
                        row.note = "No supported preview".into();
                        model.set_row_data(i, row);
                    }
                    continue;
                }
            }
        }
        let slots = 4usize.saturating_sub(self.media.pending.len());
        if slots == 0 {
            return;
        }
        let mut queue = priorities
            .iter()
            .filter(|(key, _)| {
                !self.media.cache.contains_key(*key) && !self.media.pending.contains(*key)
            })
            .collect::<Vec<_>>();
        queue.sort_unstable_by_key(|(key, priority)| (Reverse(**priority), *key));
        let mut started = 0;
        for (key, priority) in queue {
            match self.media.attempts.get(key) {
                Some(Attempt::Failed) => continue,
                Some(Attempt::Released(cost))
                    if self
                        .media
                        .eviction_plan(&priorities, *priority, *cost)
                        .is_none() =>
                {
                    continue;
                }
                _ => {}
            }
            let Ok(plugin) = PluginId::parse(&key.0) else {
                continue;
            };
            self.media.pending.insert(key.clone());
            let key = key.clone();
            // Only the exact preview source of an account favorite may use the
            // same native thumbnail path as the built-in picker.
            let favorite = self
                .backend
                .as_ref()
                .filter(|_| {
                    gate.check_media_read(&plugin).is_ok()
                        && self
                            .app
                            .state
                            .favorite_gifs
                            .as_ref()
                            .is_some_and(|favorites| {
                                favorites.iter().any(|favorite| favorite.src == key.1)
                            })
                })
                .and_then(|backend| backend.favorite_preview(key.1.clone()));
            let gate = gate.clone();
            let world = world.clone();
            let account = account.clone();
            let weak = self.weak.clone();
            let runtime = self.runtime.clone();
            let decode_runtime = runtime.clone();
            let request_key = key.clone();
            let request_world = world.clone();
            let request_account = account.clone();
            let work = runtime.spawn(async move {
                if let Some(mut favorite) = favorite {
                    if let Ok(Some(Some(picture))) =
                        yapfast_core::runtime::timeout(Duration::from_secs(20), favorite.recv())
                            .await
                    {
                        let cost = picture.rgba.len();
                        return (key, Preview::Ready(picture, cost), world, account);
                    }
                }
                let fetched = yapfast_core::runtime::timeout(
                    Duration::from_secs(20),
                    gate.fetch_media_preview(&plugin, &key.1, PREVIEW_BYTES),
                )
                .await;
                match fetched {
                    Ok(Ok(Outcome::Done(answer))) => {
                        if !(200..300).contains(&answer.response.status) {
                            return (
                                key,
                                Preview::Failed(format!("Preview HTTP {}", answer.response.status)),
                                world,
                                account,
                            );
                        }
                        let reading = decode_runtime.spawn_blocking(move || {
                            use std::io::Read;
                            let bytes = if let Some(path) = answer.body {
                                let result = (|| {
                                    let mut bytes = Vec::new();
                                    std::fs::File::open(&path)
                                        .ok()?
                                        .take(PREVIEW_BYTES + 1)
                                        .read_to_end(&mut bytes)
                                        .ok()?;
                                    Some(bytes)
                                })();
                                // `path` owns the temporary file; it stays alive
                                // through the complete read, including failure.
                                result
                            } else {
                                answer
                                    .response
                                    .body_base64
                                    .as_deref()
                                    .and_then(yapfast_core::plugins::api::call::base64::decode)
                            }?;
                            if bytes.len() > PREVIEW_BYTES as usize {
                                return None;
                            }
                            let cost = bytes.len() + 256 * 180 * 4;
                            let picture = yapfast_core::chat::decode_picture(&bytes, 256, 180)?;
                            Some((Arc::new(picture), cost))
                        });
                        let result = reading.await.ok().flatten().map_or_else(
                            || Preview::Failed("Preview could not be decoded".into()),
                            |(p, n)| Preview::Ready(p, n),
                        );
                        (key, result, world, account)
                    }
                    Ok(Ok(Outcome::AwaitingConsent { host, asking })) => (
                        key,
                        Preview::Consent(host, asking == Asking::Opened),
                        world,
                        account,
                    ),
                    Ok(Err(error)) => (key, Preview::Failed(error.to_string()), world, account),
                    Err(_) => (
                        key,
                        Preview::Failed("Preview timed out".into()),
                        world,
                        account,
                    ),
                }
            });
            let _ = slint::spawn_local(async move {
                let completed = work.await;
                let Some(adapter) = weak.upgrade() else {
                    return;
                };
                let mut a = adapter.borrow_mut();
                let Ok((key, result, world, account)) = completed else {
                    // Leave failures retryable when the dialog's model is refreshed.
                    if same_session(&a, &request_world, &request_account)
                        && a.media.pending.remove(&request_key)
                    {
                        a.media_note(&request_key, "Preview loading stopped");
                        a.media.attempts.insert(request_key, Attempt::Failed);
                        a.update_media_previews();
                    }
                    return;
                };
                if !same_session(&a, &world, &account) {
                    return;
                }
                a.media.pending.remove(&key);
                let models = a.media_models();
                let active = models
                    .iter()
                    .any(|(id, b, _)| id == &key.0 && b.cards.iter().any(|c| c.src == key.1));
                if !active {
                    a.update_media_previews();
                    return;
                }
                if !a.app.state.plugins.as_ref().is_some_and(|state| {
                    PluginId::parse(&key.0)
                        .is_ok_and(|plugin| state.network_gate().check_media_read(&plugin).is_ok())
                }) {
                    a.media_note(&key, "Preview needs media permission");
                    a.media.attempts.insert(key, Attempt::Failed);
                    a.update_media_previews();
                    return;
                }
                match result {
                    Preview::Ready(picture, cost) => {
                        let priorities = a.media.priorities(&models);
                        let victims = priorities
                            .get(&key)
                            .and_then(|priority| {
                                a.media.eviction_plan(&priorities, *priority, cost)
                            })
                            .map(|keys| keys.into_iter().cloned().collect::<Vec<_>>());
                        if let Some(victims) = victims {
                            for old in victims {
                                if let Some((_, bytes)) = a.media.cache.remove(&old) {
                                    a.clear_media_picture(&old);
                                    a.media.attempts.insert(old, Attempt::Released(bytes));
                                }
                            }
                            a.media.attempts.remove(&key);
                            a.media.cache.insert(key, (picture, cost));
                        } else {
                            a.clear_media_picture(&key);
                            a.media.attempts.insert(key, Attempt::Released(cost));
                        }
                    }
                    Preview::Consent(host, opened) => {
                        a.media_note(
                            &key,
                            "Preview needs permission. Allow the host, then refresh.",
                        );
                        a.media.attempts.insert(key.clone(), Attempt::Failed);
                        if opened {
                            if let Ok(plugin) = PluginId::parse(&key.0) {
                                a.apply(Event::Plugin(
                                    yapfast_core::application::plugins::News::Notice(
                                        yapfast_core::plugins::state::Notice::Consent {
                                            plugin,
                                            host: host.to_string(),
                                        },
                                    ),
                                ));
                            }
                        }
                    }
                    Preview::Failed(message) => {
                        a.media_note(&key, &message);
                        a.media.attempts.insert(key, Attempt::Failed);
                    }
                }
                a.update_media_previews();
            });
            started += 1;
            if started == slots {
                break;
            }
        }
    }
    fn media_note(&self, key: &(String, String), message: &str) {
        for (plugin, browser, model) in self.media_models() {
            if plugin != key.0 {
                continue;
            }
            for (i, c) in browser.cards.iter().enumerate() {
                if c.src == key.1 {
                    if let Some(mut row) = model.row_data(i) {
                        row.note = message.chars().take(180).collect::<String>().into();
                        model.set_row_data(i, row);
                    }
                }
            }
        }
    }
    fn clear_media_picture(&self, key: &(String, String)) {
        for (plugin, browser, model) in self.media_models() {
            if plugin != key.0 {
                continue;
            }
            for (i, c) in browser.cards.iter().enumerate() {
                if c.src == key.1 {
                    if let Some(mut row) = model.row_data(i) {
                        row.picture = Default::default();
                        row.frames = Default::default();
                        row.delays = Default::default();
                        row.note = "Preview released to save memory".into();
                        model.set_row_data(i, row);
                    }
                }
            }
        }
    }
    pub(super) fn plugin_file_dialog(
        &mut self,
        plugin: PluginId,
        call: u64,
        save: Option<(String, String)>,
    ) {
        let Some(state) = self.app.state.plugins.as_mut() else {
            return;
        };
        if self.media.files_open {
            state.answer_call(
                &plugin,
                call,
                HostReturn::Error {
                    message: "Another native file dialog is already open".into(),
                },
            );
            return;
        }
        if !state
            .listed()
            .iter()
            .any(|p| p.enabled && p.manifest.id == plugin)
        {
            return;
        }
        let world = state.world().clone();
        let account = world.me().map(|m| m.id);
        self.media.files_open = true;
        let weak = self.weak.clone();
        let runtime = self.runtime.clone();
        let _ = slint::spawn_local(async move {
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(format!("JSON collection — {plugin}"))
                .add_filter("JSON collection", &["json"]);
            let selected = match &save {
                Some((name, _)) => dialog.set_file_name(name).save_file().await,
                None => dialog.pick_file().await,
            };
            let Some(adapter) = weak.upgrade() else {
                return;
            };
            if !same_session(&adapter.borrow(), &world, &account) {
                adapter.borrow_mut().media.files_open = false;
                return;
            }
            let work = runtime.spawn_blocking(
                move || -> std::result::Result<serde_json::Value, String> {
                    use std::io::{Read, Write};
                    let Some(selected) = selected else {
                        return Ok(serde_json::Value::Null);
                    };
                    if let Some((_, text)) = save {
                        let parent = selected
                            .path()
                            .parent()
                            .ok_or("The chosen folder is invalid")?;
                        let mut temp =
                            tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                        temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
                        temp.as_file().sync_all().map_err(|e| e.to_string())?;
                        temp.persist(selected.path()).map_err(|e| e.to_string())?;
                        Ok(serde_json::Value::Bool(true))
                    } else {
                        let mut bytes = Vec::new();
                        std::fs::File::open(selected.path())
                            .map_err(|e| e.to_string())?
                            .take(900_001)
                            .read_to_end(&mut bytes)
                            .map_err(|e| e.to_string())?;
                        if bytes.len() > 900_000 {
                            return Err("The collection file exceeds 900 KB".into());
                        }
                        let text = String::from_utf8(bytes)
                            .map_err(|_| "The collection file must be UTF-8 JSON".to_string())?;
                        let value = serde_json::Value::String(text);
                        if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > 980_000 {
                            return Err(
                                "The encoded collection exceeds the plugin transport limit".into(),
                            );
                        }
                        Ok(value)
                    }
                },
            );
            let answer = match work.await {
                Ok(Ok(value)) => HostReturn::Ok(value),
                Ok(Err(message)) => HostReturn::Error { message },
                Err(_) => HostReturn::Error {
                    message: "The file operation stopped".into(),
                },
            };
            let Some(adapter) = weak.upgrade() else {
                return;
            };
            let mut a = adapter.borrow_mut();
            a.media.files_open = false;
            if same_session(&a, &world, &account) {
                if let Some(state) = a.app.state.plugins.as_mut() {
                    state.answer_call(&plugin, call, answer)
                }
            }
        });
    }
}
fn same_session(
    adapter: &Adapter,
    world: &Arc<yapfast_core::plugins::world::World>,
    account: &Option<String>,
) -> bool {
    adapter
        .app
        .state
        .plugins
        .as_ref()
        .is_some_and(|s| Arc::ptr_eq(s.world(), world) && s.world().me().map(|m| m.id) == *account)
}
enum Preview {
    Ready(Arc<yapfast_core::chat::Picture>, usize),
    Consent(String, bool),
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partly_visible_and_prefetched_rows_cannot_displace_full_rows() {
        let viewport = Viewport {
            top: 0.,
            height: 1090.,
            columns: 11,
        };
        assert_eq!(viewport.priority(54).0, 3);
        assert_eq!(viewport.priority(55).0, 2);
        assert_eq!(viewport.priority(66).0, 1);
        assert_eq!(viewport.priority(77).0, 0);

        let mut state = State::default();
        let mut priorities = HashMap::new();
        let picture = Arc::new(yapfast_core::chat::Picture::default());
        for index in 0..CACHE_ENTRIES {
            let key = ("gif-folders".into(), index.to_string());
            priorities.insert(key.clone(), viewport.priority(index));
            state.cache.insert(key, (picture.clone(), 1));
        }
        // A late completion at the bottom must not blank the first row.
        assert!(
            state
                .eviction_plan(&priorities, viewport.priority(64), 1)
                .is_none()
        );

        // At its new scroll position the released preview is fully visible.
        // The old top row is now only partly visible and can be released.
        let scrolled = Viewport {
            top: 180.,
            ..viewport
        };
        for (key, priority) in &mut priorities {
            *priority = scrolled.priority(key.1.parse().unwrap());
        }
        let victims = state
            .eviction_plan(&priorities, scrolled.priority(64), 1)
            .unwrap();
        assert_eq!(victims[0].1, "10");
    }

    #[test]
    fn a_visible_preview_reclaims_enough_space_to_fit_the_byte_limit() {
        let mut state = State::default();
        let mut priorities = HashMap::new();
        let picture = Arc::new(yapfast_core::chat::Picture::default());
        for index in 0..4 {
            let key = ("gif-folders".into(), index.to_string());
            priorities.insert(key.clone(), (1, Reverse(index)));
            state.cache.insert(key, (picture.clone(), CACHE_BYTES / 4));
        }
        assert!(
            state
                .eviction_plan(&priorities, (3, Reverse(0)), CACHE_BYTES + 1)
                .is_none()
        );
        let victims = state
            .eviction_plan(&priorities, (3, Reverse(0)), CACHE_BYTES / 2)
            .unwrap();
        assert_eq!(victims.len(), 2);
        assert_eq!(victims[0].1, "3");
        assert_eq!(victims[1].1, "2");
    }
}
