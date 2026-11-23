//! Enhanced release checks, independent of the upstream updater (absent in 0.3.0).
pub mod enhanced_releases;
pub fn client() -> Result<reqwest::Client, String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    reqwest::Client::builder()
        .https_only(true)
        .user_agent("Yapfast-Enhanced/0.3.0.0")
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| e.to_string())
}
