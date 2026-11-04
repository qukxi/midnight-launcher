//! Official mod catalogues. The webview passes IDs, never download URLs or hashes.
use super::{mods, Job};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const PAGE_SIZE: usize = 20;
const MAX_FILES: usize = 64;
const MAX_TOTAL: usize = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Modrinth,
    Curseforge,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    Relevance,
    #[default]
    Downloads,
    Updated,
    Newest,
}

impl SearchSort {
    pub(super) fn field(self, source: Source) -> &'static str {
        match (source, self) {
            (Source::Modrinth, Self::Relevance) => "relevance",
            (Source::Modrinth, Self::Downloads) => "downloads",
            (Source::Modrinth, Self::Updated) => "updated",
            (Source::Modrinth, Self::Newest) => "newest",
            // CurseForge offers popularity rather than a relevance index.
            (Source::Curseforge, Self::Relevance) => "2",
            (Source::Curseforge, Self::Downloads) => "6",
            (Source::Curseforge, Self::Updated) => "3",
            (Source::Curseforge, Self::Newest) => "11",
        }
    }
}

#[derive(Default)]
pub struct CatalogState {
    pub key: tokio::sync::Mutex<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub can_download: bool,
    pub icon_url: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub projects: Vec<Project>,
    pub total: usize,
    pub has_more: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    pub name: String,
    pub number: String,
    pub published: String,
    pub channel: String,
    pub size: u64,
    pub can_download: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub installed: usize,
    pub reused: usize,
    pub disabled: usize,
}

#[derive(Clone)]
struct Dependency {
    project: Option<String>,
    version: Option<String>,
    incompatible: bool,
}
#[derive(Clone)]
struct Artifact {
    project: String,
    version: String,
    file_name: String,
    url: Option<String>,
    size: usize,
    sha1: String,
    sha512: Option<String>,
    dependencies: Vec<Dependency>,
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}
fn array(value: &Value, key: &str) -> Vec<Value> {
    value[key].as_array().cloned().unwrap_or_default()
}
pub(super) fn id(source: Source, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|c| match source {
            Source::Modrinth => c.is_ascii_alphanumeric() || c == b'-' || c == b'_',
            Source::Curseforge => c.is_ascii_digit(),
        })
    {
        return Err("CATALOG_INVALID_ID".into());
    }
    Ok(())
}
fn game_version(value: &str) -> Result<(), String> {
    super::download::validate_version(value).map_err(|_| "CATALOG_INVALID_VERSION".into())
}
pub(super) fn http() -> Result<Client, String> {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("MidnightLauncher/0.2.0 (Minecraft Java mod manager)")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|_| "CATALOG_NETWORK".into())
}
pub(super) fn url_allowed(source: Source, url: &Url, api: bool) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && match (source, api, url.host_str()) {
            (Source::Modrinth, true, Some("api.modrinth.com")) => url.path().starts_with("/v2/"),
            (Source::Modrinth, false, Some("cdn.modrinth.com")) => url.path().starts_with("/data/"),
            (Source::Curseforge, true, Some("api.curseforge.com")) => {
                url.path().starts_with("/v1/")
            }
            (Source::Curseforge, false, Some("edge.forgecdn.net" | "mediafilez.forgecdn.net")) => {
                url.path().starts_with("/files/")
            }
            _ => false,
        }
}
async fn cancelled(job: Option<&Job>) {
    if let Some(job) = job {
        loop {
            if job.check().is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    } else {
        std::future::pending::<()>().await;
    }
}
async fn response(
    client: &Client,
    source: Source,
    key: Option<&str>,
    mut url: Url,
    api: bool,
    limit: usize,
    job: Option<&Job>,
) -> Result<reqwest::Response, String> {
    if let Some(job) = job {
        job.check()?;
    }
    let mut redirects = 0;
    let response = loop {
        if !url_allowed(source, &url, api) {
            return Err("CATALOG_UNSAFE_URL".into());
        }
        let mut request = client.get(url.clone());
        if !api {
            request = request.timeout(Duration::from_secs(30 * 60));
        }
        if source == Source::Curseforge {
            let mut header = reqwest::header::HeaderValue::from_str(
                key.ok_or("CATALOG_CURSEFORGE_KEY_REQUIRED")?,
            )
            .map_err(|_| "CATALOG_CURSEFORGE_KEY_INVALID")?;
            header.set_sensitive(true);
            request = request.header("x-api-key", header);
        }
        let response = tokio::select! {
            _ = cancelled(job) => return Err("OPERATION_CANCELLED".into()),
            result = request.send() => result.map_err(|_| "CATALOG_NETWORK")?,
        };
        if response.status().is_redirection() && !api && redirects < 3 {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .ok_or("CATALOG_UNSAFE_URL")?;
            url = url.join(location).map_err(|_| "CATALOG_UNSAFE_URL")?;
            redirects += 1;
        } else {
            break response;
        }
    };
    match response.status().as_u16() {
        200 => (),
        401 | 403 if source == Source::Curseforge => {
            return Err("CATALOG_CURSEFORGE_KEY_INVALID".into())
        }
        404 => return Err("CATALOG_NOT_FOUND".into()),
        429 => return Err("CATALOG_RATE_LIMIT".into()),
        _ => return Err("CATALOG_NETWORK".into()),
    }
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err("CATALOG_TOO_LARGE".into());
    }
    Ok(response)
}

pub(super) async fn fetch(
    client: &Client,
    source: Source,
    key: Option<&str>,
    url: Url,
    api: bool,
    limit: usize,
    job: Option<&Job>,
) -> Result<Vec<u8>, String> {
    let mut response = response(client, source, key, url, api, limit, job).await?;
    let mut bytes = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = cancelled(job) => return Err("OPERATION_CANCELLED".into()),
            result = response.chunk() => result.map_err(|_| "CATALOG_NETWORK")?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("CATALOG_TOO_LARGE".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub(super) struct Api {
    source: Source,
    client: Client,
    key: Option<String>,
    loader: &'static str,
}
fn loader_kind(value: &str) -> Result<&'static str, String> {
    match value {
        "fabric" => Ok("fabric"),
        "forge" => Ok("forge"),
        "neoforge" => Ok("neoforge"),
        _ => Err("CATALOG_LOADER_REQUIRED".into()),
    }
}
fn loader_label(kind: &str) -> &'static str {
    match kind {
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "Fabric",
    }
}
fn curse_loader(kind: &str) -> &'static str {
    match kind {
        "forge" => "1",
        "neoforge" => "6",
        _ => "4",
    }
}
impl Api {
    pub(super) fn new(source: Source, key: Option<String>) -> Result<Self, String> {
        if source == Source::Curseforge && key.is_none() {
            return Err("CATALOG_CURSEFORGE_KEY_REQUIRED".into());
        }
        Ok(Self {
            source,
            client: http()?,
            key,
            loader: "fabric",
        })
    }
    fn with_loader(mut self, kind: &str) -> Result<Self, String> {
        self.loader = loader_kind(kind)?;
        Ok(self)
    }
    pub(super) async fn get(
        &self,
        path: &str,
        params: &[(&str, String)],
        job: Option<&Job>,
    ) -> Result<Value, String> {
        let base = match self.source {
            Source::Modrinth => "https://api.modrinth.com/v2/",
            Source::Curseforge => "https://api.curseforge.com/v1/",
        };
        let mut url = Url::parse(&format!("{base}{path}")).map_err(|_| "CATALOG_INVALID_ID")?;
        url.query_pairs_mut()
            .extend_pairs(params.iter().map(|(k, v)| (*k, v)));
        let bytes = fetch(
            &self.client,
            self.source,
            self.key.as_deref(),
            url,
            true,
            8 * 1024 * 1024,
            job,
        )
        .await?;
        serde_json::from_slice(&bytes).map_err(|_| "CATALOG_METADATA_INVALID".into())
    }
    async fn project(&self, project: &str, job: Option<&Job>) -> Result<Value, String> {
        id(self.source, project)?;
        let value = self
            .get(
                &match self.source {
                    Source::Modrinth => format!("project/{project}"),
                    Source::Curseforge => format!("mods/{project}"),
                },
                &[],
                job,
            )
            .await?;
        let value = if self.source == Source::Curseforge {
            value["data"].clone()
        } else {
            value
        };
        let valid = match self.source {
            Source::Modrinth => {
                value["project_type"] == "mod" && value["client_side"] != "unsupported"
            }
            Source::Curseforge => value["gameId"] == 432 && value["classId"] == 6,
        };
        if !valid {
            return Err("CATALOG_METADATA_INVALID".into());
        }
        Ok(value)
    }
    async fn versions(
        &self,
        project: &str,
        game: &str,
        job: Option<&Job>,
    ) -> Result<Vec<Value>, String> {
        id(self.source, project)?;
        game_version(game)?;
        let (path, params) = match self.source {
            Source::Modrinth => (
                format!("project/{project}/version"),
                vec![
                    ("loaders", json!([self.loader]).to_string()),
                    ("game_versions", json!([game]).to_string()),
                ],
            ),
            Source::Curseforge => (
                format!("mods/{project}/files"),
                vec![
                    ("gameVersion", game.into()),
                    ("modLoaderType", curse_loader(self.loader).into()),
                    ("pageSize", "50".into()),
                ],
            ),
        };
        let value = self.get(&path, &params, job).await?;
        let mut versions = match self.source {
            Source::Modrinth => value.as_array().cloned().unwrap_or_default(),
            Source::Curseforge => array(&value, "data"),
        };
        versions.retain(|v| compatible_for(self.source, v, game, self.loader));
        versions.sort_by(|a, b| {
            let (channel, date) = match self.source {
                Source::Modrinth => ("version_type", "date_published"),
                Source::Curseforge => ("releaseType", "fileDate"),
            };
            let stable = |v: &Value| {
                if self.source == Source::Modrinth {
                    v[channel] == "release"
                } else {
                    v[channel] == 1
                }
            };
            stable(b)
                .cmp(&stable(a))
                .then_with(|| text(b, date).cmp(&text(a, date)))
        });
        Ok(versions)
    }
    async fn version(
        &self,
        project: Option<&str>,
        version: &str,
        game: &str,
        job: &Job,
    ) -> Result<Value, String> {
        id(self.source, version)?;
        let path = match self.source {
            Source::Modrinth => format!("version/{version}"),
            Source::Curseforge => {
                let project = project.ok_or("CATALOG_METADATA_INVALID")?;
                id(self.source, project)?;
                format!("mods/{project}/files/{version}")
            }
        };
        let v = self.get(&path, &[], Some(job)).await?;
        let v = if self.source == Source::Curseforge {
            v["data"].clone()
        } else {
            v
        };
        if !compatible_for(self.source, &v, game, self.loader) {
            return Err("CATALOG_NO_COMPATIBLE_VERSION".into());
        }
        Ok(v)
    }
}
fn contains(v: &Value, key: &str, value: &str) -> bool {
    v[key]
        .as_array()
        .is_some_and(|a| a.iter().any(|s| s == value))
}
#[cfg(test)]
fn compatible(source: Source, v: &Value, game: &str) -> bool {
    compatible_for(source, v, game, "fabric")
}
fn compatible_for(source: Source, v: &Value, game: &str, loader: &str) -> bool {
    match source {
        Source::Modrinth => contains(v, "game_versions", game) && contains(v, "loaders", loader),
        Source::Curseforge => {
            contains(v, "gameVersions", game)
                && contains(v, "gameVersions", loader_label(loader))
                && v["isAvailable"] != false
        }
    }
}
fn artifact(source: Source, v: &Value) -> Result<Artifact, String> {
    let (project, version, file, deps) = match source {
        Source::Modrinth => {
            let files = array(v, "files");
            let file = files
                .iter()
                .find(|f| {
                    f["primary"] == true
                        && text(f, "filename").to_ascii_lowercase().ends_with(".jar")
                })
                .or_else(|| {
                    files
                        .iter()
                        .find(|f| text(f, "filename").to_ascii_lowercase().ends_with(".jar"))
                })
                .ok_or("CATALOG_METADATA_INVALID")?
                .clone();
            let deps = array(v, "dependencies")
                .iter()
                .filter(|d| {
                    matches!(
                        d["dependency_type"].as_str(),
                        Some("required" | "incompatible")
                    )
                })
                .map(|d| Dependency {
                    project: d["project_id"].as_str().map(str::to_owned),
                    version: d["version_id"].as_str().map(str::to_owned),
                    incompatible: d["dependency_type"] == "incompatible",
                })
                .collect();
            (text(v, "project_id"), text(v, "id"), file, deps)
        }
        Source::Curseforge => {
            let deps = array(v, "dependencies")
                .iter()
                .filter(|d| d["relationType"] == 3 || d["relationType"] == 5)
                .map(|d| Dependency {
                    project: d["modId"].as_u64().map(|n| n.to_string()),
                    version: None,
                    incompatible: d["relationType"] == 5,
                })
                .collect();
            (
                v["modId"]
                    .as_u64()
                    .ok_or("CATALOG_METADATA_INVALID")?
                    .to_string(),
                v["id"]
                    .as_u64()
                    .ok_or("CATALOG_METADATA_INVALID")?
                    .to_string(),
                v.clone(),
                deps,
            )
        }
    };
    id(source, &project)?;
    id(source, &version)?;
    let (name, url, size, sha1, sha512) = match source {
        Source::Modrinth => (
            text(&file, "filename"),
            file["url"].as_str().map(str::to_owned),
            file["size"].as_u64(),
            text(&file["hashes"], "sha1"),
            file["hashes"]["sha512"].as_str().map(str::to_owned),
        ),
        Source::Curseforge => (
            text(&file, "fileName"),
            file["downloadUrl"].as_str().map(str::to_owned),
            file["fileLength"].as_u64(),
            array(&file, "hashes")
                .iter()
                .find(|h| h["algo"] == 1)
                .map(|h| text(h, "value"))
                .unwrap_or_default(),
            None,
        ),
    };
    mods::component(&name)?;
    if !name.to_ascii_lowercase().ends_with(".jar") || name.len() > 220 {
        return Err("MODS_JAR_REQUIRED".into());
    }
    let size = size
        .filter(|n| *n > 0 && *n <= mods::MAX_MOD_SIZE as u64)
        .ok_or("CATALOG_TOO_LARGE")? as usize;
    if !valid_hash(&sha1, 40) || sha512.as_ref().is_some_and(|h| !valid_hash(h, 128)) {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    if let Some(ref url) = url {
        if !url_allowed(
            source,
            &Url::parse(url).map_err(|_| "CATALOG_UNSAFE_URL")?,
            false,
        ) {
            return Err("CATALOG_UNSAFE_URL".into());
        }
    }
    Ok(Artifact {
        project,
        version,
        file_name: name,
        url,
        size,
        sha1,
        sha512,
        dependencies: deps,
    })
}
fn valid_hash(h: &str, len: usize) -> bool {
    h.len() == len && h.bytes().all(|b| b.is_ascii_hexdigit())
}

pub async fn connect(key: String) -> Result<String, String> {
    let key = key.trim().to_owned();
    if key.len() < 10 || key.len() > 512 || !key.bytes().all(|c| (33..=126).contains(&c)) {
        return Err("CATALOG_CURSEFORGE_KEY_INVALID".into());
    }
    let api = Api::new(Source::Curseforge, Some(key.clone()))?;
    api.get(
        "mods/search",
        &[
            ("gameId", "432".into()),
            ("classId", "6".into()),
            ("pageSize", "1".into()),
        ],
        None,
    )
    .await?;
    Ok(key)
}

pub async fn search(
    source: Source,
    key: Option<String>,
    query: &str,
    game: &str,
    offset: usize,
    sort: SearchSort,
) -> Result<SearchPage, String> {
    search_for_loader(source, key, query, game, offset, sort, "fabric").await
}
pub async fn search_for_loader(
    source: Source,
    key: Option<String>,
    query: &str,
    game: &str,
    offset: usize,
    sort: SearchSort,
    loader: &str,
) -> Result<SearchPage, String> {
    game_version(game)?;
    if query.len() > 200 || offset > 9980 {
        return Err("CATALOG_INVALID_QUERY".into());
    }
    let api = Api::new(source, key)?.with_loader(loader)?;
    let (path, params) = match source {
        Source::Modrinth => (
            "search",
            vec![
                ("query", query.into()),
                (
                    "facets",
                    json!([
                        ["project_type:mod"],
                        [format!("categories:{}", api.loader)],
                        [format!("versions:{game}")],
                        ["client_side!=unsupported"]
                    ])
                    .to_string(),
                ),
                ("limit", PAGE_SIZE.to_string()),
                ("offset", offset.to_string()),
                ("index", sort.field(source).into()),
            ],
        ),
        Source::Curseforge => (
            "mods/search",
            vec![
                ("gameId", "432".into()),
                ("classId", "6".into()),
                ("modLoaderType", curse_loader(api.loader).into()),
                ("gameVersion", game.into()),
                ("searchFilter", query.into()),
                ("sortField", sort.field(source).into()),
                ("sortOrder", "desc".into()),
                ("pageSize", PAGE_SIZE.to_string()),
                ("index", offset.to_string()),
            ],
        ),
    };
    let value = api.get(path, &params, None).await?;
    parse_search(source, &value, offset)
}
pub(super) fn parse_search(
    source: Source,
    value: &Value,
    offset: usize,
) -> Result<SearchPage, String> {
    let (items, total) = match source {
        Source::Modrinth => (array(value, "hits"), value["total_hits"].as_u64()),
        Source::Curseforge => (
            array(value, "data"),
            value["pagination"]["totalCount"].as_u64(),
        ),
    };
    let total = total.ok_or("CATALOG_METADATA_INVALID")?.min(10000) as usize;
    let count = items.len();
    let projects = items
        .iter()
        .map(|p| {
            let result = match source {
                Source::Modrinth => Project {
                    id: text(p, "project_id"),
                    slug: text(p, "slug"),
                    title: text(p, "title"),
                    description: text(p, "description"),
                    author: text(p, "author"),
                    downloads: p["downloads"].as_u64().unwrap_or(0),
                    can_download: true,
                    icon_url: icon_url(source, p["icon_url"].as_str()),
                },
                Source::Curseforge => Project {
                    id: p["id"]
                        .as_u64()
                        .ok_or("CATALOG_METADATA_INVALID")?
                        .to_string(),
                    slug: text(p, "slug"),
                    title: text(p, "name"),
                    description: text(p, "summary"),
                    author: array(p, "authors")
                        .iter()
                        .map(|a| text(a, "name"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    downloads: p["downloadCount"].as_f64().unwrap_or(0.0) as u64,
                    can_download: p["allowModDistribution"] != false,
                    icon_url: icon_url(source, p["logo"]["thumbnailUrl"].as_str()),
                },
            };
            id(source, &result.id)?;
            Ok(result)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(SearchPage {
        projects,
        total,
        has_more: count > 0 && offset + count < total && offset + count <= 9980,
    })
}
fn icon_url(source: Source, value: Option<&str>) -> Option<String> {
    let url = Url::parse(value?).ok()?;
    let allowed = url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && match (source, url.host_str()) {
            (Source::Modrinth, Some("cdn.modrinth.com")) => url.path().starts_with("/data/"),
            (Source::Curseforge, Some("media.forgecdn.net" | "mediafilez.forgecdn.net")) => {
                url.path().starts_with("/avatars/")
            }
            _ => false,
        };
    allowed.then(|| url.to_string())
}
pub async fn releases(
    source: Source,
    key: Option<String>,
    project: &str,
    game: &str,
) -> Result<Vec<Release>, String> {
    releases_for_loader(source, key, project, game, "fabric").await
}
pub async fn releases_for_loader(
    source: Source,
    key: Option<String>,
    project: &str,
    game: &str,
    loader: &str,
) -> Result<Vec<Release>, String> {
    let api = Api::new(source, key)?.with_loader(loader)?;
    let p = api.project(project, None).await?;
    let allowed = source != Source::Curseforge || p["allowModDistribution"] != false;
    let versions = api.versions(project, game, None).await?;
    versions
        .iter()
        .take(50)
        .map(|v| {
            let a = artifact(source, v)?;
            Ok(Release {
                id: a.version,
                name: if source == Source::Modrinth {
                    text(v, "name")
                } else {
                    text(v, "displayName")
                },
                number: if source == Source::Modrinth {
                    text(v, "version_number")
                } else {
                    text(v, "fileName")
                },
                published: text(
                    v,
                    if source == Source::Modrinth {
                        "date_published"
                    } else {
                        "fileDate"
                    },
                ),
                channel: if source == Source::Modrinth {
                    text(v, "version_type")
                } else {
                    match v["releaseType"].as_u64() {
                        Some(1) => "release",
                        Some(2) => "beta",
                        _ => "alpha",
                    }
                    .into()
                },
                size: a.size as u64,
                can_download: allowed && a.url.is_some(),
            })
        })
        .collect()
}

async fn resolve(
    api: &Api,
    project: &str,
    version: &str,
    game: &str,
    job: &Job,
) -> Result<Vec<Artifact>, String> {
    id(api.source, project)?;
    id(api.source, version)?;
    game_version(game)?;
    let mut queue = VecDeque::from([(Some(project.to_owned()), Some(version.to_owned()))]);
    let mut selected: HashMap<String, Artifact> = HashMap::new();
    let mut incompatible = Vec::new();
    let mut steps = 0;
    while let Some((project, version)) = queue.pop_front() {
        job.check()?;
        steps += 1;
        if steps > 256 || selected.len() >= MAX_FILES {
            return Err("CATALOG_DEPENDENCY_LIMIT".into());
        }
        if let Some(ref project) = project {
            if let Some(old) = selected.get(project) {
                if version.as_ref().is_some_and(|v| v != &old.version) {
                    return Err("CATALOG_DEPENDENCY_CONFLICT".into());
                }
                continue;
            }
        }
        let v = if let Some(ref version) = version {
            api.version(project.as_deref(), version, game, job).await?
        } else {
            api.versions(
                project.as_deref().ok_or("CATALOG_METADATA_INVALID")?,
                game,
                Some(job),
            )
            .await?
            .into_iter()
            .next()
            .ok_or("CATALOG_NO_COMPATIBLE_VERSION")?
        };
        let a = artifact(api.source, &v)?;
        if version.as_ref().is_some_and(|v| v != &a.version) {
            return Err("CATALOG_METADATA_INVALID".into());
        }
        if project.as_ref().is_some_and(|p| p != &a.project) {
            return Err("CATALOG_METADATA_INVALID".into());
        }
        if let Some(old) = selected.get(&a.project) {
            if old.version != a.version {
                return Err("CATALOG_DEPENDENCY_CONFLICT".into());
            }
            continue;
        }
        let p = api.project(&a.project, Some(job)).await?;
        if api.source == Source::Curseforge && p["allowModDistribution"] == false || a.url.is_none()
        {
            return Err("CATALOG_DOWNLOAD_RESTRICTED".into());
        }
        for d in &a.dependencies {
            if d.incompatible {
                incompatible.push(d.clone());
            } else {
                queue.push_back((d.project.clone(), d.version.clone()));
            }
        }
        selected.insert(a.project.clone(), a);
    }
    if incompatible.iter().any(|d| {
        selected.values().any(|a| match (&d.project, &d.version) {
            (Some(p), Some(v)) => p == &a.project && v == &a.version,
            (Some(p), None) => p == &a.project,
            (None, Some(v)) => v == &a.version,
            _ => false,
        })
    }) {
        return Err("CATALOG_DEPENDENCY_CONFLICT".into());
    }
    let mut files: Vec<_> = selected.into_values().collect();
    files.sort_by(|a, b| a.project.cmp(&b.project));
    total_size(&files)?;
    Ok(files)
}

fn total_size(files: &[Artifact]) -> Result<usize, String> {
    files.iter().try_fold(0usize, |size, file| {
        size.checked_add(file.size)
            .filter(|n| *n <= MAX_TOTAL)
            .ok_or_else(|| "CATALOG_TOO_LARGE".into())
    })
}

fn fabric_id(reader: impl Read + Seek) -> Result<String, String> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|_| "MODS_INVALID_JAR")?;
    let mut file = zip
        .by_name("fabric.mod.json")
        .map_err(|_| "CATALOG_NOT_FABRIC")?;
    if file.size() > 128 * 1024 {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    let mut metadata = Vec::new();
    file.by_ref()
        .take(128 * 1024 + 1)
        .read_to_end(&mut metadata)
        .map_err(|_| "CATALOG_METADATA_INVALID")?;
    if metadata.len() > 128 * 1024 {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    let value: Value = serde_json::from_slice(&metadata).map_err(|_| "CATALOG_METADATA_INVALID")?;
    let id = text(&value, "id");
    if id.is_empty() || id.len() > 64 {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    Ok(id)
}
#[cfg(test)]
struct Downloaded {
    artifact: Artifact,
    bytes: Vec<u8>,
}
struct StagedDownload {
    artifact: Artifact,
    path: PathBuf,
}
struct Staging(PathBuf);
impl Staging {
    fn new(data: &Path, instance: &str) -> Result<Self, String> {
        let dir = mods::directory(data, instance, true)?.ok_or("MODS_WRITE_FAILED")?;
        let path = dir.join(format!(".midnight-catalog-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).map_err(|_| "MODS_WRITE_FAILED")?;
        Ok(Self(path))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
fn mod_ids(bytes: &[u8], loader: &str) -> Result<Vec<String>, String> {
    mod_ids_reader(std::io::Cursor::new(bytes), loader)
}
fn mod_ids_reader(reader: impl Read + Seek, loader: &str) -> Result<Vec<String>, String> {
    if loader == "fabric" {
        return fabric_id(reader).map(|id| vec![id]);
    }
    loader_kind(loader)?;
    let mut zip = zip::ZipArchive::new(reader).map_err(|_| "MODS_INVALID_JAR")?;
    let name =
        if loader == "neoforge" && zip.index_for_name("META-INF/neoforge.mods.toml").is_some() {
            "META-INF/neoforge.mods.toml"
        } else {
            "META-INF/mods.toml"
        };
    let mut file = zip.by_name(name).map_err(|_| "CATALOG_WRONG_LOADER")?;
    if file.size() > 128 * 1024 {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    let mut text = String::new();
    file.by_ref()
        .take(128 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| "CATALOG_METADATA_INVALID")?;
    if text.len() > 128 * 1024 {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    let metadata: toml::Value = toml::from_str(&text).map_err(|_| "CATALOG_METADATA_INVALID")?;
    let mods = metadata
        .get("mods")
        .and_then(toml::Value::as_array)
        .ok_or("CATALOG_WRONG_LOADER")?;
    if mods.is_empty() || mods.len() > 64 {
        return Err("CATALOG_METADATA_INVALID".into());
    }
    mods.iter()
        .map(|m| {
            let id = m
                .get("modId")
                .and_then(toml::Value::as_str)
                .ok_or("CATALOG_METADATA_INVALID")?;
            if id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            {
                return Err("CATALOG_METADATA_INVALID".into());
            }
            Ok(id.to_string())
        })
        .collect()
}
struct DownloadVerifier<'a> {
    artifact: &'a Artifact,
    received: usize,
    sha1: Sha1,
    sha512: Sha512,
}
impl<'a> DownloadVerifier<'a> {
    fn new(artifact: &'a Artifact) -> Self {
        Self {
            artifact,
            received: 0,
            sha1: Sha1::new(),
            sha512: Sha512::new(),
        }
    }
    fn update(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.received = self
            .received
            .checked_add(bytes.len())
            .filter(|n| *n <= self.artifact.size && *n <= mods::MAX_MOD_SIZE)
            .ok_or("CATALOG_TOO_LARGE")?;
        self.sha1.update(bytes);
        self.sha512.update(bytes);
        Ok(())
    }
    fn finish(self) -> Result<(), String> {
        if self.received != self.artifact.size
            || format!("{:x}", self.sha1.finalize()) != self.artifact.sha1.to_ascii_lowercase()
            || self.artifact.sha512.as_ref().is_some_and(|hash| {
                format!("{:x}", self.sha512.finalize()) != hash.to_ascii_lowercase()
            })
        {
            return Err("CATALOG_HASH_MISMATCH".into());
        }
        Ok(())
    }
}

async fn download_to_file(
    mut response: reqwest::Response,
    artifact: &Artifact,
    path: &Path,
    completed: usize,
    total: usize,
    job: &Job,
) -> Result<(), String> {
    job.check()?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "MODS_WRITE_FAILED")?;
    let mut verifier = DownloadVerifier::new(artifact);
    let mut reported = 0;
    loop {
        let chunk = tokio::select! {
            _ = cancelled(Some(job)) => return Err("OPERATION_CANCELLED".into()),
            result = response.chunk() => result.map_err(|_| "CATALOG_NETWORK")?,
        };
        let Some(chunk) = chunk else { break };
        job.check()?;
        verifier.update(&chunk)?;
        file.write_all(&chunk).map_err(|_| "MODS_WRITE_FAILED")?;
        if verifier.received - reported >= 1024 * 1024 {
            reported = verifier.received;
            job.report("catalog-download", completed + reported, total);
        }
    }
    verifier.finish()?;
    file.sync_all().map_err(|_| "MODS_WRITE_FAILED")?;
    job.check()?;
    job.report("catalog-download", completed + artifact.size, total);
    Ok(())
}

#[cfg(test)]
fn verify_download(a: &Artifact, bytes: &[u8]) -> Result<(), String> {
    let mut verifier = DownloadVerifier::new(a);
    verifier.update(bytes)?;
    verifier.finish()
}

#[cfg(test)]
fn commit(
    data: &Path,
    instance: &str,
    downloads: Vec<Downloaded>,
    job: &Job,
) -> Result<InstallResult, String> {
    job.check()?;
    let staging = Staging::new(data, instance)?;
    let files = downloads
        .into_iter()
        .enumerate()
        .map(|(index, download)| {
            let path = staging.0.join(format!("{index}.part"));
            fs::write(&path, download.bytes).map_err(|_| "MODS_WRITE_FAILED")?;
            Ok(StagedDownload {
                artifact: download.artifact,
                path,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    commit_for(data, instance, files, job, "fabric")
}
fn commit_for(
    data: &Path,
    instance: &str,
    downloads: Vec<StagedDownload>,
    job: &Job,
    loader: &str,
) -> Result<InstallResult, String> {
    job.check()?;
    let mut hashes = HashMap::new();
    let mut identities = HashSet::new();
    for m in mods::list(data, instance)? {
        let mut file = mods::open_file(data, instance, &m.file_name)?;
        hashes.insert(mods::file_hash(&mut file, Some(job))?, m.enabled);
        if let Ok(ids) = mod_ids_reader(file, loader) {
            identities.extend(ids);
        }
    }
    let mut install = Vec::new();
    let mut names = HashSet::new();
    let mut result = InstallResult {
        installed: 0,
        reused: 0,
        disabled: 0,
    };
    for d in downloads {
        job.check()?;
        let hash = d.artifact.sha1.to_ascii_lowercase();
        if let Some(enabled) = hashes.get(&hash) {
            result.reused += 1;
            if !enabled {
                result.disabled += 1;
            }
            continue;
        }
        let file = fs::File::open(&d.path).map_err(|_| "MODS_READ_FAILED")?;
        let ids = mod_ids_reader(file, loader)?;
        if !names.insert(d.artifact.file_name.to_lowercase())
            || ids.into_iter().any(|id| !identities.insert(id))
        {
            return Err("CATALOG_MOD_CONFLICT".into());
        }
        hashes.insert(hash, true);
        install.push(d);
    }
    let mut created: Vec<(String, String)> = Vec::new();
    for d in install {
        let done = job
            .check()
            .and_then(|_| mods::publish_staged(data, instance, &d.artifact.file_name, &d.path));
        if let Err(error) = done {
            for (name, hash) in created.iter().rev() {
                mods::rollback(data, instance, name, hash);
            }
            return Err(error);
        }
        created.push((d.artifact.file_name, d.artifact.sha1.to_ascii_lowercase()));
    }
    if let Err(error) = job.check() {
        for (name, hash) in created.iter().rev() {
            mods::rollback(data, instance, name, hash);
        }
        return Err(error);
    }
    result.installed = created.len();
    Ok(result)
}
pub async fn install(
    source: Source,
    key: Option<String>,
    data: &Path,
    instance: &str,
    project: &str,
    version: &str,
    game: &str,
    job: &Job,
) -> Result<InstallResult, String> {
    install_for_loader(
        source, key, data, instance, project, version, game, job, "fabric",
    )
    .await
}
pub async fn install_for_loader(
    source: Source,
    key: Option<String>,
    data: &Path,
    instance: &str,
    project: &str,
    version: &str,
    game: &str,
    job: &Job,
    loader: &str,
) -> Result<InstallResult, String> {
    // Validate the destination before any network request.
    mods::directory(data, instance, false)?;
    let api = Api::new(source, key)?.with_loader(loader)?;
    job.report("catalog-resolving", 0, 0);
    let files = resolve(&api, project, version, game, job).await?;
    let total = total_size(&files)?;
    job.check()?;
    let staging = Staging::new(data, instance)?;
    let mut completed = 0;
    let mut downloaded = Vec::new();
    for (index, a) in files.into_iter().enumerate() {
        job.check()?;
        job.report("catalog-download", completed, total);
        let url = Url::parse(a.url.as_deref().ok_or("CATALOG_DOWNLOAD_RESTRICTED")?)
            .map_err(|_| "CATALOG_UNSAFE_URL")?;
        let reply = response(
            &api.client,
            source,
            api.key.as_deref(),
            url,
            false,
            a.size,
            Some(job),
        )
        .await?;
        let path = staging.0.join(format!("{index}.part"));
        download_to_file(reply, &a, &path, completed, total, job).await?;
        completed += a.size;
        downloaded.push(StagedDownload { artifact: a, path });
    }
    job.report("catalog-installing", 0, 0);
    let data = data.to_path_buf();
    let instance = instance.to_owned();
    let job = job.clone();
    let loader = loader_kind(loader)?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = commit_for(&data, &instance, downloaded, &job, loader);
        drop(staging);
        result
    })
    .await
    .map_err(|_| "MODS_WRITE_FAILED")?
}

pub fn page_url(source: Source, slug: &str) -> Result<String, String> {
    id(Source::Modrinth, slug)?;
    Ok(match source {
        Source::Modrinth => format!("https://modrinth.com/mod/{slug}"),
        Source::Curseforge => format!("https://www.curseforge.com/minecraft/mc-mods/{slug}"),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn loader_filters_are_exact_for_modrinth_and_curseforge() {
        for loader in ["fabric", "forge", "neoforge"] {
            let mr = serde_json::json!({"game_versions":["1.21.1"],"loaders":[loader]});
            let cf = serde_json::json!({"gameVersions":["1.21.1",loader_label(loader)]});
            for selected in ["fabric", "forge", "neoforge"] {
                assert_eq!(
                    compatible_for(Source::Modrinth, &mr, "1.21.1", selected),
                    selected == loader
                );
                assert_eq!(
                    compatible_for(Source::Curseforge, &cf, "1.21.1", selected),
                    selected == loader
                );
            }
        }
        assert_eq!(curse_loader("forge"), "1");
        assert_eq!(curse_loader("neoforge"), "6");
        assert!(loader_kind("quilt").is_err());
    }
    #[test]
    fn forge_and_neoforge_metadata_preserve_all_ids_and_reject_wrong_loader_files() {
        for (loader, name) in [
            ("forge", "META-INF/mods.toml"),
            ("neoforge", "META-INF/neoforge.mods.toml"),
        ] {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, b"modLoader = 'javafml'\n[[mods]]\nmodId = 'first_mod'\n[[mods]]\nmodId = 'second_mod'\n").unwrap();
            let bytes = writer.finish().unwrap().into_inner();
            assert_eq!(
                mod_ids(&bytes, loader).unwrap(),
                ["first_mod", "second_mod"]
            );
            assert!(mod_ids(&bytes, "fabric").is_err());
        }
    }
    use super::*;
    #[test]
    fn search_sort_accepts_only_supported_ipc_values() {
        for (value, modrinth, curseforge) in [
            ("relevance", "relevance", "2"),
            ("downloads", "downloads", "6"),
            ("updated", "updated", "3"),
            ("newest", "newest", "11"),
        ] {
            let sort: SearchSort = serde_json::from_value(json!(value)).unwrap();
            assert_eq!(sort.field(Source::Modrinth), modrinth);
            assert_eq!(sort.field(Source::Curseforge), curseforge);
        }
        assert!(serde_json::from_value::<SearchSort>(json!("unknown")).is_err());
        assert_eq!(SearchSort::default().field(Source::Modrinth), "downloads");
    }

    use std::{
        fs,
        io::Write,
        path::PathBuf,
        sync::{atomic::AtomicBool, Arc},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("midnight-catalog-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn job(cancelled: bool) -> Job {
        Job {
            cancel: Arc::new(AtomicBool::new(cancelled)),
            app: None,
        }
    }
    fn downloaded(name: &str, identity: &str, version: &str) -> Downloaded {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            json!({"schemaVersion":1,"id":identity,"version":version})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let artifact = Artifact {
            project: identity.into(),
            version: version.into(),
            file_name: name.into(),
            url: Some("https://cdn.modrinth.com/data/test/test.jar".into()),
            size: bytes.len(),
            sha1: format!("{:x}", Sha1::digest(&bytes)),
            sha512: Some(format!("{:x}", Sha512::digest(&bytes))),
            dependencies: vec![],
        };
        Downloaded { artifact, bytes }
    }
    #[test]
    fn restricts_catalog_hosts_and_never_allows_keys_on_other_origins() {
        assert!(icon_url(
            Source::Modrinth,
            Some("https://cdn.modrinth.com/data/project/icon.png")
        )
        .is_some());
        assert!(icon_url(
            Source::Curseforge,
            Some("https://media.forgecdn.net/avatars/thumbnails/1/2/logo.png")
        )
        .is_some());
        for value in [
            "https://evil.example/icon.png",
            "https://cdn.modrinth.com.evil.example/data/a.png",
            "http://cdn.modrinth.com/data/a.png",
            "https://user@cdn.modrinth.com/data/a.png",
            "data:image/svg+xml,<svg/>",
        ] {
            assert!(icon_url(Source::Modrinth, Some(value)).is_none());
        }
        assert!(url_allowed(
            Source::Curseforge,
            &Url::parse("https://edge.forgecdn.net/files/1/test.jar").unwrap(),
            false
        ));
        for value in [
            "http://edge.forgecdn.net/files/a",
            "https://edge.forgecdn.net.evil.org/files/a",
            "https://user@edge.forgecdn.net/files/a",
            "https://edge.forgecdn.net:444/files/a",
            "https://api.modrinth.com/v2/search",
        ] {
            assert!(!url_allowed(
                Source::Curseforge,
                &Url::parse(value).unwrap(),
                false
            ));
        }
        assert!(!url_allowed(
            Source::Modrinth,
            &Url::parse("https://edge.forgecdn.net/files/a").unwrap(),
            false
        ));
        assert!(page_url(Source::Curseforge, "../../settings").is_err());
        assert!(id(Source::Curseforge, "1/2").is_err());
    }
    #[test]
    fn selects_only_exact_game_and_fabric_releases_for_both_providers() {
        let mr = json!({"game_versions":["1.21.1"],"loaders":["fabric"]});
        assert!(compatible(Source::Modrinth, &mr, "1.21.1"));
        assert!(!compatible(Source::Modrinth, &mr, "1.21"));
        assert!(!compatible(
            Source::Modrinth,
            &json!({"game_versions":["1.21.1"],"loaders":["forge"]}),
            "1.21.1"
        ));
        let cf = json!({"gameVersions":["1.21.1","Fabric"],"isAvailable":true});
        assert!(compatible(Source::Curseforge, &cf, "1.21.1"));
        assert!(!compatible(
            Source::Curseforge,
            &json!({"gameVersions":["1.21.1","NeoForge"]}),
            "1.21.1"
        ));
    }
    #[test]
    fn parses_official_shapes_and_respects_restricted_curseforge_downloads() {
        let v: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/catalog-curseforge.json"))
                .unwrap();
        let page = parse_search(Source::Curseforge, &v["search"], 0).unwrap();
        assert_eq!(page.projects[0].id, "123456");
        assert!(!page.projects[0].can_download);
        let a = artifact(Source::Curseforge, &v["file"]).unwrap();
        assert!(a.url.is_none());
        assert_eq!(a.dependencies.len(), 2);
        assert_eq!(a.dependencies[0].project.as_deref(), Some("654321"));
        assert!(!a.dependencies[0].incompatible);
        assert!(a.dependencies[1].incompatible);
        let mr: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/catalog-modrinth.json"))
                .unwrap();
        let a = artifact(Source::Modrinth, &mr).unwrap();
        assert_eq!(a.dependencies.len(), 1);
        assert_eq!(a.dependencies[0].version.as_deref(), Some("dep00001"));
        let mut unsafe_file = mr.clone();
        unsafe_file["files"][0]["filename"] = json!("../outside.jar");
        assert!(artifact(Source::Modrinth, &unsafe_file).is_err());
        let mut unsafe_url = mr;
        unsafe_url["files"][0]["url"] = json!("https://evil.example/mod.jar");
        assert!(artifact(Source::Modrinth, &unsafe_url).is_err());
    }
    #[test]
    fn verifies_size_and_both_hashes_before_installing() {
        let d = downloaded("mod.jar", "testmod", "1.0");
        assert!(verify_download(&d.artifact, &d.bytes).is_ok());
        let mut bytes = d.bytes.clone();
        bytes[0] ^= 1;
        assert!(verify_download(&d.artifact, &bytes).is_err());
        let mut a = d.artifact.clone();
        a.sha512 = Some("0".repeat(128));
        assert!(verify_download(&a, &d.bytes).is_err());
    }
    #[test]
    fn accepts_wonderland_and_enforces_file_and_transaction_limits() {
        let wonderland: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/catalog-wonderland.json"))
                .unwrap();
        let large = artifact(Source::Modrinth, &wonderland).unwrap();
        assert_eq!(large.size, 175374232);
        assert!(compatible_for(
            Source::Modrinth,
            &wonderland,
            "1.20.1",
            "forge"
        ));
        assert!(total_size(&[large]).is_ok());
        for source in [Source::Modrinth, Source::Curseforge] {
            let mut value = if source == Source::Modrinth {
                wonderland.clone()
            } else {
                serde_json::from_str::<Value>(include_str!(
                    "../../tests/fixtures/catalog-curseforge.json"
                ))
                .unwrap()["file"]
                    .clone()
            };
            let size = if source == Source::Modrinth {
                &mut value["files"][0]["size"]
            } else {
                &mut value["fileLength"]
            };
            *size = json!(mods::MAX_MOD_SIZE);
            let at_limit = artifact(source, &value).unwrap();
            assert_eq!(total_size(&vec![at_limit.clone(); 4]).unwrap(), MAX_TOTAL);
            assert!(total_size(&vec![at_limit; 5]).is_err());
            let size = if source == Source::Modrinth {
                &mut value["files"][0]["size"]
            } else {
                &mut value["fileLength"]
            };
            *size = json!(mods::MAX_MOD_SIZE + 1);
            assert_eq!(artifact(source, &value).err().unwrap(), "CATALOG_TOO_LARGE");
        }
    }
    #[test]
    fn chunked_verification_detects_overruns_truncation_and_corruption() {
        let download = downloaded("mod.jar", "testmod", "1.0");
        let mut verifier = DownloadVerifier::new(&download.artifact);
        for chunk in download.bytes.chunks(7) {
            verifier.update(chunk).unwrap();
        }
        verifier.finish().unwrap();
        let mut verifier = DownloadVerifier::new(&download.artifact);
        verifier
            .update(&download.bytes[..download.bytes.len() - 1])
            .unwrap();
        assert_eq!(verifier.finish().unwrap_err(), "CATALOG_HASH_MISMATCH");
        let mut verifier = DownloadVerifier::new(&download.artifact);
        verifier.update(&download.bytes).unwrap();
        assert_eq!(verifier.update(b"extra").unwrap_err(), "CATALOG_TOO_LARGE");
    }
    #[tokio::test]
    async fn streams_verified_files_then_publishes_and_removes_staging() {
        let root = Fixture::new();
        let download = downloaded("mod.jar", "testmod", "1.0");
        let staging = Staging::new(&root.0, "one").unwrap();
        let stage_path = staging.0.clone();
        let path = staging.0.join("0.part");
        let reply = reqwest::Response::from(http::Response::new(download.bytes.clone()));
        download_to_file(
            reply,
            &download.artifact,
            &path,
            0,
            download.bytes.len(),
            &job(false),
        )
        .await
        .unwrap();
        assert!(mods::list(&root.0, "one").unwrap().is_empty());
        let result = commit_for(
            &root.0,
            "one",
            vec![StagedDownload {
                artifact: download.artifact,
                path,
            }],
            &job(false),
            "fabric",
        )
        .unwrap();
        assert_eq!(result.installed, 1);
        drop(staging);
        assert!(!stage_path.exists());
        assert_eq!(
            mods::read_file(&root.0, "one", "mod.jar").unwrap(),
            download.bytes
        );
    }
    #[tokio::test]
    async fn failed_or_cancelled_streams_never_publish_files_and_clean_up() {
        let root = Fixture::new();
        for cancel in [false, true] {
            let download = downloaded("mod.jar", "testmod", "1.0");
            let staging = Staging::new(&root.0, "one").unwrap();
            let stage_path = staging.0.clone();
            let operation = job(false);
            let flag = operation.cancel.clone();
            let mut count = 0;
            let chunks = download
                .bytes
                .chunks(7)
                .map(|chunk| Ok::<_, std::io::Error>(chunk.to_vec()))
                .collect::<Vec<_>>();
            let body = futures_util::stream::StreamExt::inspect(
                futures_util::stream::iter(chunks),
                move |_| {
                    count += 1;
                    if cancel && count == 2 {
                        flag.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                },
            );
            let mut artifact = download.artifact;
            if !cancel {
                artifact.sha512 = Some("0".repeat(128));
            }
            let reply =
                reqwest::Response::from(http::Response::new(reqwest::Body::wrap_stream(body)));
            let error = download_to_file(
                reply,
                &artifact,
                &staging.0.join("0.part"),
                0,
                artifact.size,
                &operation,
            )
            .await
            .unwrap_err();
            assert_eq!(
                error,
                if cancel {
                    "OPERATION_CANCELLED"
                } else {
                    "CATALOG_HASH_MISMATCH"
                }
            );
            drop(staging);
            assert!(!stage_path.exists());
            assert!(mods::list(&root.0, "one").unwrap().is_empty());
            assert_eq!(
                fs::read_dir(mods::directory(&root.0, "one", false).unwrap().unwrap())
                    .unwrap()
                    .count(),
                0
            );
        }
    }
    #[test]
    fn reuses_identical_files_without_enabling_disabled_mods_or_installing_second_versions() {
        let root = Fixture::new();
        let first = commit(
            &root.0,
            "one",
            vec![downloaded("mod.jar", "testmod", "1.0")],
            &job(false),
        )
        .unwrap();
        assert_eq!(first.installed, 1);
        mods::set_enabled(&root.0, "one", "mod.jar", false).unwrap();
        let second = commit(
            &root.0,
            "one",
            vec![downloaded("other-provider-name.jar", "testmod", "1.0")],
            &job(false),
        )
        .unwrap();
        assert_eq!(
            (second.installed, second.reused, second.disabled),
            (0, 1, 1)
        );
        assert_eq!(
            commit(
                &root.0,
                "one",
                vec![downloaded("new.jar", "testmod", "2.0")],
                &job(false)
            )
            .unwrap_err(),
            "CATALOG_MOD_CONFLICT"
        );
        assert_eq!(mods::list(&root.0, "one").unwrap().len(), 1);
    }
    #[test]
    fn rolls_back_only_new_files_when_a_later_file_conflicts() {
        let root = Fixture::new();
        let existing = downloaded("keep.jar", "existing", "1.0");
        mods::import(&root.0, "one", "keep.jar", &existing.bytes).unwrap();
        let outcome = commit(
            &root.0,
            "one",
            vec![
                downloaded("new.jar", "newmod", "1.0"),
                downloaded("keep.jar", "different", "1.0"),
            ],
            &job(false),
        );
        assert!(outcome.is_err());
        let files = mods::list(&root.0, "one").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name, "keep.jar");
        assert_eq!(
            mods::read_file(&root.0, "one", "keep.jar").unwrap(),
            existing.bytes
        );
        assert!(commit(
            &root.0,
            "one",
            vec![downloaded("cancel.jar", "cancelled", "1.0")],
            &job(true)
        )
        .is_err());
        assert_eq!(mods::list(&root.0, "one").unwrap().len(), 1);
    }
    #[tokio::test]
    async fn missing_credentials_and_cancelled_requests_do_not_reach_network() {
        assert!(Api::new(Source::Curseforge, None).is_err());
        let result = fetch(
            &http().unwrap(),
            Source::Modrinth,
            None,
            Url::parse("https://cdn.modrinth.com/data/test/test.jar").unwrap(),
            false,
            100,
            Some(&job(true)),
        )
        .await;
        assert_eq!(result.unwrap_err(), "OPERATION_CANCELLED");
        assert_eq!(
            connect("bad\nkey".into()).await.unwrap_err(),
            "CATALOG_CURSEFORGE_KEY_INVALID"
        );
    }
}
