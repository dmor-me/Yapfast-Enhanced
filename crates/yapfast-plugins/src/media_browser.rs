//! Bounded native media-browser data. Collection logic remains in the Wasm guest.
use serde::{Deserialize, Serialize};
fn yes() -> bool {
    true
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Navigation {
    pub id: String,
    pub name: String,
    pub count: usize,
    pub category: bool,
    pub indent: bool,
    pub collapsed: bool,
    pub editable: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Card {
    pub id: String,
    pub src: String,
    pub label: String,
    pub folder: String,
    pub selected: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaBrowser {
    pub version: u8,
    pub title: String,
    pub query: String,
    pub collections: Vec<Choice>,
    pub collection: String,
    pub collection_name: String,
    pub navigation: Vec<Navigation>,
    pub active: String,
    pub cards: Vec<Card>,
    pub folders: Vec<Choice>,
    pub categories: Vec<Choice>,
    pub destination: String,
    pub total: usize,
    pub page: usize,
    pub pages: usize,
    pub selection_mode: bool,
    pub selected_count: usize,
    pub form_name: String,
    pub form_kind: String,
    pub form_parent: String,
    pub edit_id: String,
    pub edit_name: String,
    pub confirm_text: String,
    pub error: String,
    pub status: String,
    #[serde(default = "yes")]
    pub virtualize: bool,
    #[serde(default = "yes")]
    pub lazy: bool,
    #[serde(default = "yes")]
    pub pause_offscreen: bool,
    pub hover_only: bool,
}
impl Default for MediaBrowser {
    fn default() -> Self {
        Self {
            version: 1,
            title: String::new(),
            query: String::new(),
            collections: vec![],
            collection: String::new(),
            collection_name: String::new(),
            navigation: vec![],
            active: "all".into(),
            cards: vec![],
            folders: vec![],
            categories: vec![],
            destination: "unfiled".into(),
            total: 0,
            page: 0,
            pages: 1,
            selection_mode: false,
            selected_count: 0,
            form_name: String::new(),
            form_kind: "folder".into(),
            form_parent: String::new(),
            edit_id: String::new(),
            edit_name: String::new(),
            confirm_text: String::new(),
            error: String::new(),
            status: String::new(),
            virtualize: true,
            lazy: true,
            pause_offscreen: true,
            hover_only: false,
        }
    }
}
impl MediaBrowser {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.cards.len() > 200
            || self.navigation.len() > 352
            || self.collections.len() > 50
            || self.folders.len() > 251
            || self.categories.len() > 101
        {
            return Err("Unsupported or oversized media browser".into());
        }
        if self.total > 100_000
            || self.pages > 10_000
            || self.page > 10_000
            || self.selected_count > 10_000
        {
            return Err("Media browser counters exceed their limits".into());
        }
        for text in [
            &self.title,
            &self.query,
            &self.collection_name,
            &self.form_name,
            &self.form_kind,
            &self.form_parent,
            &self.edit_id,
            &self.edit_name,
            &self.confirm_text,
            &self.error,
            &self.status,
        ] {
            if text.len() > 4096 {
                return Err("Media browser text exceeds 4 KiB".into());
            }
        }
        for choices in [&self.collections, &self.folders, &self.categories] {
            let mut ids = std::collections::HashSet::new();
            for c in choices {
                if c.id.len() > 200 || c.name.chars().count() > 80 || !ids.insert(&c.id) {
                    return Err("Media browser choices are invalid".into());
                }
            }
        }
        let mut ids = std::collections::HashSet::new();
        for card in &self.cards {
            if card.id.is_empty()
                || card.id.len() > 200
                || card.src.len() > 4096
                || card.label.len() > 1000
                || !ids.insert(&card.id)
            {
                return Err("Media cards are duplicated or invalid".into());
            }
            if !card.src.is_empty() && crate::network::Target::parse(&card.src).is_err() {
                return Err("Media previews require a plain public HTTPS URL".into());
            }
        }
        let mut navs = std::collections::HashSet::new();
        for n in &self.navigation {
            if n.id.is_empty()
                || n.id.len() > 200
                || n.name.chars().count() > 80
                || !navs.insert(&n.id)
            {
                return Err("Media navigation is duplicated or invalid".into());
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_pages_and_untrusted_urls() {
        let mut b = MediaBrowser::default();
        b.cards.push(Card {
            id: "a".into(),
            src: "https://media.tenor.com/a.gif".into(),
            ..Default::default()
        });
        assert!(b.validate().is_ok());
        b.cards[0].src = "file:///C:/secret".into();
        assert!(b.validate().is_err());
        b.cards.clear();
        b.cards.resize(201, Card::default());
        assert!(b.validate().is_err());
    }
    #[test]
    fn duplicate_actions_are_refused() {
        let mut b = MediaBrowser::default();
        b.navigation = vec![
            Navigation {
                id: "x".into(),
                ..Default::default()
            };
            2
        ];
        assert!(b.validate().is_err());
    }
}
