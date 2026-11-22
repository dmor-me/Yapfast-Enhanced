//! Enhanced GitHub release fetching.
pub(super) use super::enhanced_tools::{State, bind};
use super::*;

pub(super) async fn check_enhanced() -> Result<crate::update::enhanced_releases::Check, String> {
    use crate::update::enhanced_releases as enhanced;
    let mut response = crate::update::client()?
        .get(enhanced::RELEASES)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| error.without_url().to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("The Enhanced release channel isn't available yet.".into());
    }
    response
        .error_for_status_ref()
        .map_err(|error| error.without_url().to_string())?;
    const LIMIT: usize = 4 << 20;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| error.without_url().to_string())?
    {
        if body.len().saturating_add(chunk.len()) > LIMIT {
            return Err("GitHub's release list exceeded the size limit.".into());
        }
        body.extend_from_slice(&chunk);
    }
    enhanced::parse(&body)
}
