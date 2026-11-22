//! Pure decisions for the manual Enhanced release check. No updater side effects.
use serde::Deserialize;

pub const VERSION: &str = "0.1.0.0";
pub const RELEASES: &str =
    "https://api.github.com/repos/dmor-me/Yapfast-Enhanced/releases?per_page=30";
const PAGES: &str = "https://github.com/dmor-me/Yapfast-Enhanced/releases/tag/";
const DOWNLOADS: &str = "https://github.com/dmor-me/Yapfast-Enhanced/releases/download/";
// The exact official base used to build this plugin-enabled executable.
const BASE: &str = "2e6c332fc3b03cc74d7d3ccb6d07fb26e25f48ec6e019d639fe94a4eb8595fcd";

pub struct Check {
    pub message: String,
    pub page: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

// Product versions are <Yapfast major>.<minor>.<patch>.<Enhanced revision>.
// Accept legacy three-part tags as revision zero so older releases remain
// comparable. Compare numbers, never strings; Windows components fit in u16.
fn version(tag: &str) -> Option<[u16; 4]> {
    let tag = tag
        .strip_prefix("enhanced-v")
        .or_else(|| tag.strip_prefix('v'))
        .unwrap_or(tag);
    let parts: Vec<_> = tag.split('.').collect();
    if !matches!(parts.len(), 3 | 4) {
        return None;
    }
    let mut result = [0; 4];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty()
            || !part.bytes().all(|b| b.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        result[index] = part.parse().ok()?;
    }
    Some(result)
}

pub fn release_page(page: &str) -> bool {
    page.strip_prefix(PAGES)
        .is_some_and(|tag| !tag.is_empty() && version(tag).is_some())
}

pub fn parse(body: &[u8]) -> Result<Check, String> {
    let releases: Vec<Release> = serde_json::from_slice(body)
        .map_err(|_| "GitHub returned an invalid Enhanced release list.".to_owned())?;
    let current = version(VERSION).expect("the build's Enhanced version is stable");
    let latest = releases
        .iter()
        .filter_map(|release| {
            if release.draft
                || release.prerelease
                || !release_page(&release.html_url)
                || release.html_url != format!("{PAGES}{}", release.tag_name)
            {
                return None;
            }
            let number = version(&release.tag_name)?;
            // A source-only release is not an installable Windows Enhanced update.
            let installer = release.assets.iter().any(|asset| {
                asset.name == "Yapfast-Enhanced-Installer.exe"
                    && asset.browser_download_url
                        == format!("{DOWNLOADS}{}/{}", release.tag_name, asset.name)
            });
            installer.then_some((number, release))
        })
        .max_by_key(|(number, _)| *number);
    let Some((number, latest)) = latest else {
        return Ok(Check {
            message: "No stable Windows Enhanced releases have been published yet.".into(),
            page: None,
        });
    };
    if number <= current {
        return Ok(Check {
            message: "You're up to date.".into(),
            page: None,
        });
    }
    let compatible = latest.assets.iter().any(|asset| {
        asset.name == format!("yapfast-enhanced-for-{BASE}.zip")
            && asset.browser_download_url
                == format!("{DOWNLOADS}{}/{}", latest.tag_name, asset.name)
    });
    let note = if compatible {
        "A package for this Yapfast build is listed. Open the release to install it manually."
    } else {
        "No compatibility package for this Yapfast build is listed. Check the release requirements before installing."
    };
    Ok(Check {
        message: format!("Yapfast Enhanced {} is available. {note}", latest.tag_name),
        page: Some(latest.html_url.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, compatible: bool) -> serde_json::Value {
        let mut assets = vec![serde_json::json!({"name":"Yapfast-Enhanced-Installer.exe",
            "browser_download_url":format!("{DOWNLOADS}{tag}/Yapfast-Enhanced-Installer.exe")})];
        if compatible {
            let name = format!("yapfast-enhanced-for-{BASE}.zip");
            assets.push(serde_json::json!({"name":name,"browser_download_url":format!("{DOWNLOADS}{tag}/{name}")}));
        }
        serde_json::json!({"tag_name":tag,"html_url":format!("{PAGES}{tag}"),"assets":assets})
    }
    fn check(releases: Vec<serde_json::Value>) -> Check {
        parse(&serde_json::to_vec(&releases).unwrap()).unwrap()
    }
    #[test]
    fn empty_channel_does_not_claim_up_to_date() {
        assert!(check(vec![]).message.contains("No stable"));
        assert!(parse(b"{}").is_err());
    }
    #[test]
    fn stable_versions_sort_numerically_and_filter_drafts() {
        let mut draft = release("enhanced-v99.0.0.1", true);
        draft["draft"] = true.into();
        let mut pre = release("enhanced-v98.0.0.1", true);
        pre["prerelease"] = true.into();
        let found = check(vec![
            release("enhanced-v0.1.0.9", true),
            release("enhanced-v0.1.0.10", true),
            draft,
            pre,
        ]);
        assert!(found.message.contains("enhanced-v0.1.0.10"));
        assert!(
            found
                .message
                .contains("package for this Yapfast build is listed")
        );
        assert!(found.page.is_some());
    }
    #[test]
    fn same_or_older_release_is_current() {
        assert!(
            check(vec![release("enhanced-v0.1.0.0", true)])
                .message
                .contains("up to date")
        );
        assert!(check(vec![release("v0.0.9", true)]).page.is_none());
    }
    #[test]
    fn incompatible_release_warns_without_offering_install() {
        assert!(
            check(vec![release("enhanced-v0.1.0.2", false)])
                .message
                .contains("No compatibility package")
        );
    }
    #[test]
    fn untrusted_pages_and_assets_are_not_offered() {
        let mut bad = release("v0.1.0.2", true);
        bad["html_url"] = "https://evil.example/releases/tag/v0.1.0.2".into();
        assert!(check(vec![bad]).page.is_none());
        let mut bad = release("v0.1.0.2", true);
        bad["assets"][0]["browser_download_url"] = "https://evil.example/app.exe".into();
        assert!(check(vec![bad]).page.is_none());
        for tag in ["1.2", "1.02.3", "1.2.3-beta", "1.2.3?x", "../v1.2.3"] {
            assert!(version(tag).is_none());
        }
    }
    #[test]
    fn base_versions_and_legacy_tags_compare_numerically() {
        assert_eq!(version("v0.4.1.0"), Some([0, 4, 1, 0]));
        assert_eq!(version("enhanced-v0.1.0.0"), Some([0, 1, 0, 0]));
        assert_eq!(version("v0.1.0"), Some([0, 1, 0, 0]));
        assert!(check(vec![release("enhanced-v0.1.0", true)]).page.is_none());
        assert!(check(vec![release("v0.1.0", true)]).page.is_none());
        let found = check(vec![release("v0.1.0.99", true), release("v0.4.0.1", false)]);
        assert!(found.message.contains("v0.4.0.1"));
        assert!(found.message.contains("No compatibility package"));
        assert!(version("v0.1.0.10") > version("v0.1.0.9"));
        for tag in [
            "0.1.0.01",
            "0.1.0E1",
            "0.1.0.1.1",
            "0.1.0.0-beta",
            "0.1.0.65536",
            "0.1.0.-1",
            "0.1.0.0?x",
        ] {
            assert!(version(tag).is_none(), "accepted malformed tag: {tag}");
        }
    }
    #[test]
    fn source_only_releases_are_not_windows_updates() {
        let mut source = release("v0.1.0.2", true);
        source["assets"] = serde_json::json!([]);
        assert!(check(vec![source]).page.is_none());
        assert!(RELEASES.contains("dmor-me/Yapfast-Enhanced"));
    }
}
