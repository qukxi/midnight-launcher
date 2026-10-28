use sha1::{Digest, Sha1};
use std::time::Duration;

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("MidnightLauncher/0.1.0")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}
// URLs and paths never come directly from the webview. Hosts are explicitly restricted.
pub fn validate_url(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(
            url.host_str(),
            Some(
                "piston-meta.mojang.com"
                    | "piston-data.mojang.com"
                    | "launcher.mojang.com"
                    | "launchermeta.mojang.com"
            )
        )
    {
        return Err("Only official Mojang HTTPS download hosts are allowed".into());
    }
    Ok(())
}
pub fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty()
        || version.len() > 64
        || !version.as_bytes()[0].is_ascii_alphanumeric()
        || !version
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("Invalid Minecraft version ID".into());
    }
    Ok(())
}
pub fn verify_sha1(bytes: &[u8], expected: &str) -> Result<(), String> {
    if expected.len() != 40
        || !expected.bytes().all(|c| c.is_ascii_hexdigit())
        || format!("{:x}", Sha1::digest(bytes)) != expected.to_ascii_lowercase()
    {
        return Err("SHA-1 mismatch: downloaded data was not saved".into());
    }
    Ok(())
}
pub async fn fetch(http: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>, String> {
    validate_url(url)?;
    let mut response = http
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response.status().is_redirection() {
        return Err("Download redirects are not allowed".into());
    }
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err("Download exceeds size limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("Download exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_path_traversal() {
        for id in ["../x", "/tmp/x", "..", "", "a/b", "a\\b"] {
            assert!(validate_version(id).is_err(), "{id}");
        }
        assert!(validate_version("1.21.1").is_ok());
        assert!(validate_version("24w14a").is_ok());
    }
    #[test]
    fn limits_download_hosts() {
        assert!(validate_url("https://piston-data.mojang.com/v1/objects/a/client.jar").is_ok());
        for url in [
            "http://piston-data.mojang.com/x",
            "https://evil.example/x",
            "https://piston-data.mojang.com.evil.example/x",
            "https://piston-data.mojang.com:444/x",
            "https://user@piston-data.mojang.com/x",
        ] {
            assert!(validate_url(url).is_err());
        }
    }
    #[test]
    fn rejects_corrupt_downloads() {
        assert!(verify_sha1(b"abc", "a9993e364706816aba3e25717850c26c9cd0d89d").is_ok());
        assert!(verify_sha1(b"wrong", "a9993e364706816aba3e25717850c26c9cd0d89d").is_err());
    }
}
