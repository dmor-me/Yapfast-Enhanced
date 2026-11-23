//! The open channel and those open lately, and who is typing in them.

use super::*;
use std::{collections::VecDeque, sync::Mutex};

/// Decoded previews shared by all recent channels. Active rows hold their
/// own handles; closing a channel leaves only this bounded cache owning pixels.
#[derive(Default)]
pub(super) struct PreviewCache {
    pictures: VecDeque<(String, Arc<Picture>)>,
    bytes: usize,
}

pub(super) const PREVIEW_CACHE_BYTES: usize = 8 * 1024 * 1024;
pub(super) type SharedPreviews = Arc<Mutex<PreviewCache>>;

impl PreviewCache {
    pub(super) fn clear_unused(&mut self) {
        self.pictures
            .retain(|(_, picture)| Arc::strong_count(picture) > 1);
        self.bytes = self
            .pictures
            .iter()
            .map(|(_, picture)| picture.rgba.len())
            .sum();
    }

    fn remove_exclusive(&mut self, released: &HashSet<String>, shared: &HashSet<String>) {
        self.pictures
            .retain(|(url, _)| !released.contains(url) || shared.contains(url));
        self.bytes = self
            .pictures
            .iter()
            .map(|(_, picture)| picture.rgba.len())
            .sum();
    }

    pub(super) fn get(&mut self, url: &str) -> Option<Arc<Picture>> {
        let index = self.pictures.iter().position(|(key, _)| key == url)?;
        let kept = self.pictures.remove(index).unwrap();
        let picture = kept.1.clone();
        self.pictures.push_back(kept);
        Some(picture)
    }

    pub(super) fn insert(&mut self, url: String, picture: Arc<Picture>) {
        if let Some(index) = self.pictures.iter().position(|(key, _)| key == &url) {
            self.bytes -= self.pictures.remove(index).unwrap().1.rgba.len();
        }
        let bytes = picture.rgba.len();
        if bytes > PREVIEW_CACHE_BYTES {
            return;
        }
        while self.bytes + bytes > PREVIEW_CACHE_BYTES {
            self.bytes -= self.pictures.pop_front().unwrap().1.rgba.len();
        }
        self.bytes += bytes;
        self.pictures.push_back((url, picture));
    }
}

/// The open channel and those open lately, kept so going back to one shows
/// it at once. Closed channels follow live events too, and the latest page
/// loads again when one opens, for what they missed.
#[derive(Default)]
pub struct Channels {
    pub(super) open: Option<OpenChannel>,
    /// A thread shown beside the open channel, kept as live as it.
    pub(super) aside: Option<OpenChannel>,
    /// Newest last.
    pub(super) recent: Vec<OpenChannel>,
    pub(super) previews: SharedPreviews,
    release_dms: bool,
    release_servers: bool,
    /// Sends outlive the timeline, including failed sends the user may retry.
    pub(super) dm_sends: HashMap<String, Vec<open_channel::Pending>>,
    /// Who is typing in each channel, by user id, with their name and until when.
    pub(super) typing: HashMap<String, HashMap<String, (String, Instant)>>,
    /// Where this account last told Discord it's typing, and when.
    pub(super) typed: Option<(String, Instant)>,
    /// Counts member searches, so one typed over before it went is dropped.
    pub(super) searches: Arc<std::sync::atomic::AtomicU64>,
}

/// How long a member search waits for more typing before it goes to Discord.
pub(super) const SEARCH_PAUSE: Duration = Duration::from_millis(300);

impl Channels {
    /// Forgets who stopped typing in `room`, and says who still is.
    pub(super) fn typing_changed(&mut self, room: &str, events: &UnboundedSender<Event>) {
        let now = Instant::now();
        let mut names = Vec::new();
        if let Some(typing) = self.typing.get_mut(room) {
            typing.retain(|_, (_, until)| *until > now);
            names = typing.values().map(|(name, _)| name.clone()).collect();
            names.sort();
            if typing.is_empty() {
                self.typing.remove(room);
            }
        }
        let _ = events.send(Event::Typing {
            room: room.to_owned(),
            names,
        });
    }

    pub(super) fn close(&mut self, next: Option<&str>) {
        let channel = self.open.take();
        if let Some(channel) = channel {
            if ((self.release_dms && channel.guild == discord::DM_GUILD)
                || (self.release_servers && channel.guild != discord::DM_GUILD))
                && next != Some(channel.room.as_str())
            {
                self.release_dm(channel);
            } else {
                self.keep(Some(channel));
            }
        }
    }

    /// Stops showing the thread beside the open channel.
    pub(super) fn close_aside(&mut self) {
        let channel = self.aside.take();
        if let Some(channel) = channel {
            if (self.release_dms && channel.guild == discord::DM_GUILD)
                || (self.release_servers && channel.guild != discord::DM_GUILD)
            {
                self.release_dm(channel);
            } else {
                self.keep(Some(channel));
            }
        }
    }

    /// Keeps a channel that closed among the recent ones.
    fn keep(&mut self, channel: Option<OpenChannel>) {
        if let Some(mut channel) = channel {
            channel.shelve();
            self.recent.push(channel);
            if self.recent.len() > KEPT_CHANNELS {
                let mut gone = self.recent.remove(0);
                save(gone.take_changes(), gone.guild == discord::DM_GUILD);
            }
        }
    }

    pub(super) fn set_release_dms(&mut self, release: bool) {
        self.release_dms = release;
        if release {
            let (direct, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.recent)
                .into_iter()
                .partition(|channel| channel.guild == discord::DM_GUILD);
            self.recent = others;
            for channel in direct {
                self.release_dm(channel);
            }
        }
    }

    pub(super) fn set_release_servers(&mut self, release: bool) {
        self.release_servers = release;
        if release {
            let (servers, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.recent)
                .into_iter()
                .partition(|channel| channel.guild != discord::DM_GUILD);
            self.recent = others;
            for channel in servers {
                self.release_dm(channel);
            }
        }
    }

    fn release_dm(&mut self, mut channel: OpenChannel) {
        let previews = channel.preview_sources();
        if !previews.is_empty() {
            let shared: HashSet<String> = self
                .open
                .iter()
                .chain(&self.aside)
                .chain(&self.recent)
                .flat_map(OpenChannel::preview_sources)
                .collect();
            self.previews
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove_exclusive(&previews, &shared);
        }
        // Commit the most recent snapshot before a rapid return can read it.
        save(channel.take_changes(), true);
        if !channel.pending.is_empty() {
            self.dm_sends
                .insert(channel.room.clone(), std::mem::take(&mut channel.pending));
        }
    }

    /// Applies a live event to the closed channels. Each keeps only its
    /// newest messages, as on closing; a busy one would otherwise grow for
    /// the whole session.
    pub(super) fn follow(&mut self, event: &Live) {
        for kept in &mut self.recent {
            if kept.live(event) {
                kept.trim();
            }
        }
    }

    /// Live events went by without them, so every channel loads its latest
    /// page again when it next opens.
    pub(super) fn missed(&mut self) {
        for channel in self
            .open
            .iter_mut()
            .chain(&mut self.aside)
            .chain(&mut self.recent)
        {
            channel.missed();
        }
    }

    /// Puts the newest messages of the channels that changed on disk.
    pub(super) fn save(&mut self) {
        for channel in self
            .open
            .iter_mut()
            .chain(&mut self.aside)
            .chain(&mut self.recent)
        {
            save(channel.take_changes(), channel.guild == discord::DM_GUILD);
        }
    }

    pub(super) fn reopen(&mut self, room: &str) -> Option<OpenChannel> {
        let index = self
            .recent
            .iter()
            .position(|channel| channel.room == room)?;
        Some(self.recent.remove(index))
    }

    pub(super) fn get_mut(&mut self, room: &str) -> Option<&mut OpenChannel> {
        self.open
            .iter_mut()
            .chain(&mut self.aside)
            .chain(&mut self.recent)
            .find(|channel| channel.room == room)
    }

    /// The open channel, or the thread beside it, if it's `room`.
    pub(super) fn shown_mut(&mut self, room: &str) -> Option<&mut OpenChannel> {
        self.open
            .iter_mut()
            .chain(&mut self.aside)
            .find(|channel| channel.room == room)
    }

    /// Whether `room` is on screen: the open channel or the thread beside it.
    pub(super) fn is_open(&self, room: &str) -> bool {
        self.open
            .iter()
            .chain(&self.aside)
            .any(|channel| channel.room == room)
    }
}

/// DM snapshots are written in actor order so an older periodic write cannot
/// overwrite a newer snapshot when a closed DM is immediately reopened.
fn save(saved: Option<(String, Vec<Message>)>, direct: bool) {
    if let Some((channel, messages)) = saved {
        if direct {
            yapfast_discord::cache::save_messages(&channel, messages);
        } else {
            tokio::task::spawn_blocking(move || {
                yapfast_discord::cache::save_messages(&channel, messages)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(bytes: usize) -> Arc<Picture> {
        Arc::new(Picture {
            rgba: crate::chat::Pixels::new(0, 0, vec![0; bytes]),
            ..Default::default()
        })
    }

    #[test]
    fn releasing_unused_previews_preserves_shared_images() {
        let mut cache = PreviewCache::default();
        let shared = picture(32);
        cache.insert("visible".into(), shared.clone());
        cache.insert("unused".into(), picture(64));
        cache.clear_unused();
        assert_eq!(cache.bytes, 32);
        assert!(cache.get("unused").is_none());
        assert!(Arc::ptr_eq(&shared, &cache.get("visible").unwrap()));
        drop(shared);
        cache.clear_unused();
        assert_eq!(cache.bytes, 0);
    }

    #[test]
    fn previews_evict_by_bytes_and_recent_use_without_invalidating_visible_handles() {
        let mut cache = PreviewCache::default();
        let first = picture(PREVIEW_CACHE_BYTES / 2);
        let weak = Arc::downgrade(&first);
        cache.insert("first".into(), first.clone());
        cache.insert("second".into(), picture(PREVIEW_CACHE_BYTES / 2));
        assert!(Arc::ptr_eq(&first, &cache.get("first").unwrap()));
        cache.insert("third".into(), picture(PREVIEW_CACHE_BYTES / 2));
        assert!(
            cache.get("second").is_none(),
            "least recently used leaves first"
        );
        cache.insert("fourth".into(), picture(PREVIEW_CACHE_BYTES / 2));
        assert!(cache.get("first").is_none());
        assert!(
            weak.upgrade().is_some(),
            "a visible row still owns its handle"
        );
        drop(first);
        assert!(
            weak.upgrade().is_none(),
            "eviction releases the cache's ownership"
        );
        assert_eq!(cache.bytes, PREVIEW_CACHE_BYTES);
        cache.insert("third".into(), picture(64));
        assert_eq!(cache.bytes, PREVIEW_CACHE_BYTES / 2 + 64);
        cache.insert("oversized".into(), picture(PREVIEW_CACHE_BYTES + 1));
        assert!(cache.get("oversized").is_none());
        assert_eq!(cache.bytes, PREVIEW_CACHE_BYTES / 2 + 64);
    }
}
