use super::download::{client, fetch, validate_url, verify_sha1};
use serde::{Deserialize, Serialize};
pub const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
pub struct Manifest {
    pub versions: Vec<VersionEntry>,
}
#[derive(Deserialize)]
pub struct VersionEntry {
    pub id: String,
    pub url: String,
    pub sha1: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Serialize)]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Deserialize)]
pub struct VersionMetadata {
    pub id: String,
    pub downloads: Option<Downloads>,
    pub logging: Option<Logging>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub arguments: Arguments,
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndex,
    #[serde(rename = "javaVersion")]
    pub java_version: JavaVersion,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Deserialize)]
pub struct Arguments {
    pub jvm: Vec<serde_json::Value>,
    pub game: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub rules: Vec<serde_json::Value>,
    pub downloads: LibraryDownloads,
}
#[derive(Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
}
#[derive(Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}
#[derive(Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major: u32,
}
#[derive(Deserialize)]
pub struct Downloads {
    pub client: Option<Artifact>,
}
#[derive(Deserialize, Clone)]
pub struct Artifact {
    pub url: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub path: String,
}

pub async fn manifest(http: &reqwest::Client) -> Result<Manifest, String> {
    let bytes = fetch(http, MANIFEST, 4 * 1024 * 1024).await?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid Mojang manifest: {e}"))
}
pub async fn list_versions() -> Result<Vec<VersionSummary>, String> {
    Ok(manifest(&client()?)
        .await?
        .versions
        .into_iter()
        .map(|v| VersionSummary {
            id: v.id,
            kind: v.kind,
        })
        .collect())
}
pub async fn metadata(
    http: &reqwest::Client,
    version: &str,
) -> Result<(VersionMetadata, Vec<u8>), String> {
    let entry = manifest(http)
        .await?
        .versions
        .into_iter()
        .find(|v| v.id == version)
        .ok_or("Версия не найдена в официальном manifest Mojang.")?;
    validate_url(&entry.url)?;
    let bytes = fetch(http, &entry.url, 8 * 1024 * 1024).await?;
    verify_sha1(&bytes, &entry.sha1)?;
    let info: VersionMetadata =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid version metadata: {e}"))?;
    if info.id != version {
        return Err("Metadata version mismatch".into());
    }
    Ok((info, bytes))
}

#[derive(Deserialize)]
pub struct Logging {
    pub client: LoggingClient,
}
#[derive(Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
}
#[derive(Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}
