//! Modrinth client modpacks, installed transactionally into a new instance.
use super::{
    catalog::{self, Api, SearchSort, Source},
    loader::Loader,
    mods, Job,
};
use futures_util::{stream, StreamExt, TryStreamExt};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

const MAX_ARCHIVE: usize = 128 * 1024 * 1024;
const MAX_FILE: u64 = 128 * 1024 * 1024;
const MAX_DOWNLOADS: u64 = 2 * 1024 * 1024 * 1024;
const MAX_OVERRIDES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 10000;
const MAX_FILES: usize = 2048;
const MANIFEST: &str = ".midnight-modpack.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackIndex {
    format_version: u32,
    game: String,
    version_id: String,
    name: String,
    dependencies: HashMap<String, String>,
    files: Vec<PackFile>,
}
#[derive(Clone, Deserialize)]
struct Hashes {
    sha1: String,
    sha512: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackFile {
    path: String,
    hashes: Hashes,
    downloads: Vec<String>,
    file_size: u64,
    env: Option<HashMap<String, String>>,
}
struct Override {
    index: usize,
    path: String,
    layer: u8,
}
struct Plan {
    game: String,
    loader: Loader,
    files: Vec<PackFile>,
    overrides: BTreeMap<String, Override>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRelease {
    pub id: String,
    pub name: String,
    pub number: String,
    pub published: String,
    pub channel: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackOrigin {
    pub project_id: String,
    pub version_id: String,
    pub version_name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInstance {
    pub id: String,
    pub name: String,
    pub version: String,
    pub engine: &'static str,
    pub loader: Loader,
    pub modpack: PackOrigin,
}
#[derive(Serialize)]
pub struct InstallResult {
    pub instance: PackInstance,
    pub files: usize,
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
// The UI validates string lengths in UTF-16 code units.
fn bounded_text(value: &str, limit: usize) -> String {
    let mut units = 0;
    value
        .chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= limit
        })
        .collect()
}
fn download_error(error: String) -> String {
    if error == "CATALOG_TOO_LARGE" {
        "MODPACK_INTEGRITY".into()
    } else {
        error
    }
}
fn valid_hash(value: &str, size: usize) -> bool {
    value.len() == size && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn check_hashes(hashes: &Hashes) -> Result<(), String> {
    if !valid_hash(&hashes.sha1, 40) || !valid_hash(&hashes.sha512, 128) {
        return Err("MODPACK_INVALID".into());
    }
    Ok(())
}
fn verify(bytes: &[u8], size: u64, hashes: &Hashes) -> Result<(), String> {
    check_hashes(hashes)?;
    if bytes.len() as u64 != size
        || !format!("{:x}", Sha1::digest(bytes)).eq_ignore_ascii_case(&hashes.sha1)
        || !format!("{:x}", Sha512::digest(bytes)).eq_ignore_ascii_case(&hashes.sha512)
    {
        return Err("MODPACK_INTEGRITY".into());
    }
    Ok(())
}
fn pack_path(value: &str) -> Result<PathBuf, String> {
    let path = super::install::safe_path(value).map_err(|_| "MODPACK_UNSAFE_PATH")?;
    if value.len() > 1024 || value.eq_ignore_ascii_case(MANIFEST) {
        return Err("MODPACK_UNSAFE_PATH".into());
    }
    for part in value.split('/') {
        mods::component(part).map_err(|_| "MODPACK_UNSAFE_PATH")?;
    }
    Ok(path)
}
fn allowed_urls(downloads: &[String]) -> Result<Vec<Url>, String> {
    if downloads.is_empty() || downloads.len() > 8 {
        return Err("MODPACK_INVALID".into());
    }
    let urls: Vec<_> = downloads
        .iter()
        .filter_map(|s| Url::parse(s).ok())
        .filter(|url| catalog::url_allowed(Source::Modrinth, url, false))
        .collect();
    if urls.is_empty() {
        return Err("MODPACK_EXTERNAL_DOWNLOAD".into());
    }
    Ok(urls)
}
fn dependencies(index: &PackIndex) -> Result<(String, Loader), String> {
    if index.format_version != 1
        || index.game != "minecraft"
        || index.name.trim().is_empty()
        || index.version_id.is_empty()
    {
        return Err("MODPACK_INVALID".into());
    }
    if index.dependencies.keys().any(|key| {
        !matches!(
            key.as_str(),
            "minecraft" | "fabric-loader" | "forge" | "neoforge"
        )
    }) {
        return Err("MODPACK_LOADER_UNSUPPORTED".into());
    }
    let game = index
        .dependencies
        .get("minecraft")
        .ok_or("MODPACK_INVALID")?
        .clone();
    super::download::validate_version(&game).map_err(|_| "MODPACK_INVALID")?;
    let choices: Vec<_> = [
        ("fabric-loader", "fabric"),
        ("forge", "forge"),
        ("neoforge", "neoforge"),
    ]
    .into_iter()
    .filter_map(|(key, kind)| {
        index.dependencies.get(key).map(|version| Loader {
            kind: kind.into(),
            version: version.clone(),
        })
    })
    .collect();
    if choices.len() != 1 {
        return Err("MODPACK_LOADER_UNSUPPORTED".into());
    }
    let loader = choices
        .into_iter()
        .next()
        .ok_or("MODPACK_LOADER_UNSUPPORTED")?;
    super::loader::validate(&Some(loader.clone())).map_err(|_| "MODPACK_LOADER_UNSUPPORTED")?;
    Ok((game, loader))
}

fn plan(bytes: &[u8], include_optional: bool, job: &Job) -> Result<Plan, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "MODPACK_INVALID")?;
    if archive.len() > MAX_ENTRIES {
        return Err("MODPACK_TOO_LARGE".into());
    }
    let mut seen = HashSet::new();
    let mut index = None;
    let mut overrides: BTreeMap<String, Override> = BTreeMap::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        job.check()?;
        let mut entry = archive.by_index(i).map_err(|_| "MODPACK_INVALID")?;
        let name = entry
            .name()
            .map_err(|_| "MODPACK_INVALID")?
            .trim_end_matches('/')
            .to_owned();
        pack_path(&name)?;
        if !seen.insert(name.to_lowercase()) {
            return Err("MODPACK_DUPLICATE_PATH".into());
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("MODPACK_UNSAFE_PATH".into());
        }
        total = total.checked_add(entry.size()).ok_or("MODPACK_TOO_LARGE")?;
        if total > MAX_OVERRIDES || entry.size() > MAX_FILE {
            return Err("MODPACK_TOO_LARGE".into());
        }
        if entry.is_dir() {
            continue;
        }
        if name == "modrinth.index.json" {
            if entry.size() > 8 * 1024 * 1024 {
                return Err("MODPACK_TOO_LARGE".into());
            }
            let mut raw = Vec::new();
            (&mut entry)
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut raw)
                .map_err(|_| "MODPACK_INVALID")?;
            if raw.len() > 8 * 1024 * 1024 {
                return Err("MODPACK_TOO_LARGE".into());
            }
            index = Some(serde_json::from_slice::<PackIndex>(&raw).map_err(|_| "MODPACK_INVALID")?);
        } else if let Some((prefix, path)) = name.split_once('/') {
            let layer = match prefix {
                "overrides" => 0,
                "client-overrides" => 1,
                _ => continue,
            };
            pack_path(path)?;
            let key = path.to_lowercase();
            if overrides.get(&key).is_none_or(|old| old.layer < layer) {
                overrides.insert(
                    key,
                    Override {
                        index: i,
                        path: path.into(),
                        layer,
                    },
                );
            }
        }
    }
    let mut index = index.ok_or("MODPACK_INVALID")?;
    let (game, loader) = dependencies(&index)?;
    if index.files.len() > MAX_FILES {
        return Err("MODPACK_TOO_LARGE".into());
    }
    let mut paths = HashSet::new();
    let mut download_size = 0u64;
    let mut files = Vec::new();
    for file in index.files.drain(..) {
        job.check()?;
        pack_path(&file.path)?;
        if !paths.insert(file.path.to_lowercase()) {
            return Err("MODPACK_DUPLICATE_PATH".into());
        }
        check_hashes(&file.hashes)?;
        if file.file_size > MAX_FILE {
            return Err("MODPACK_TOO_LARGE".into());
        }
        if let Some(env) = &file.env {
            if env
                .values()
                .any(|v| !matches!(v.as_str(), "required" | "optional" | "unsupported"))
            {
                return Err("MODPACK_INVALID".into());
            }
            match env.get("client").map(String::as_str) {
                Some("unsupported") => continue,
                Some("optional") if !include_optional => continue,
                _ => (),
            }
        }
        if overrides.contains_key(&file.path.to_lowercase()) {
            continue;
        }
        allowed_urls(&file.downloads)?;
        download_size = download_size
            .checked_add(file.file_size)
            .ok_or("MODPACK_TOO_LARGE")?;
        if download_size > MAX_DOWNLOADS {
            return Err("MODPACK_TOO_LARGE".into());
        }
        files.push(file);
    }
    Ok(Plan {
        game,
        loader,
        files,
        overrides,
    })
}

// Only fresh staging directories are written. The installed instance is published at the end.
struct Staging {
    path: PathBuf,
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn instances_dir(data: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(data).map_err(|_| "MODPACK_WRITE_FAILED")?;
    for dir in [data.to_path_buf(), data.join("instances")] {
        match fs::create_dir(&dir) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return Err("MODPACK_WRITE_FAILED".into()),
        }
        let meta = fs::symlink_metadata(&dir).map_err(|_| "MODPACK_WRITE_FAILED")?;
        if !meta.is_dir() || mods::is_link(&meta) {
            return Err("MODPACK_UNSAFE_PATH".into());
        }
    }
    Ok(data.join("instances"))
}
fn write_new(root: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
    let target = root.join(pack_path(path)?);
    fs::create_dir_all(target.parent().ok_or("MODPACK_UNSAFE_PATH")?)
        .map_err(|_| "MODPACK_WRITE_FAILED")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| "MODPACK_DUPLICATE_PATH")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "MODPACK_WRITE_FAILED".into())
}
fn unpack_overrides(bytes: &[u8], plan: &Plan, root: &Path, job: &Job) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "MODPACK_INVALID")?;
    for (n, item) in plan.overrides.values().enumerate() {
        job.check()?;
        job.report("modpack-extract", n, plan.overrides.len());
        let mut entry = archive
            .by_index(item.index)
            .map_err(|_| "MODPACK_INVALID")?;
        let target = root.join(pack_path(&item.path)?);
        fs::create_dir_all(target.parent().ok_or("MODPACK_UNSAFE_PATH")?)
            .map_err(|_| "MODPACK_WRITE_FAILED")?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|_| "MODPACK_DUPLICATE_PATH")?;
        let mut count = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            job.check()?;
            let n = entry.read(&mut buffer).map_err(|_| "MODPACK_INVALID")?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > entry.size() || count > MAX_FILE {
                return Err("MODPACK_TOO_LARGE".into());
            }
            file.write_all(&buffer[..n])
                .map_err(|_| "MODPACK_WRITE_FAILED")?;
        }
        if count != entry.size() {
            return Err("MODPACK_INVALID".into());
        }
        file.sync_all().map_err(|_| "MODPACK_WRITE_FAILED")?;
    }
    Ok(())
}
fn publish(stage: &Staging, destination: &Path, job: &Job) -> Result<(), String> {
    job.check()?;
    match fs::symlink_metadata(destination) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        _ => return Err("MODPACK_INSTANCE_EXISTS".into()),
    }
    fs::rename(&stage.path, destination).map_err(|_| "MODPACK_WRITE_FAILED".into())
}
pub async fn search(
    query: &str,
    offset: usize,
    sort: SearchSort,
) -> Result<catalog::SearchPage, String> {
    if query.len() > 200 || offset > 9980 {
        return Err("CATALOG_INVALID_QUERY".into());
    }
    let value = Api::new(Source::Modrinth, None)?
        .get(
            "search",
            &[
                ("query", query.into()),
                (
                    "facets",
                    json!([
                        ["project_type:modpack"],
                        [
                            "categories:fabric",
                            "categories:forge",
                            "categories:neoforge"
                        ],
                        ["client_side!=unsupported"]
                    ])
                    .to_string(),
                ),
                ("limit", "20".into()),
                ("offset", offset.to_string()),
                ("index", sort.field(Source::Modrinth).into()),
            ],
            None,
        )
        .await?;
    catalog::parse_search(Source::Modrinth, &value, offset)
}
fn supported_release(v: &Value) -> bool {
    v["loaders"].as_array().is_some_and(|a| {
        a.iter()
            .any(|l| l == "fabric" || l == "forge" || l == "neoforge")
    }) && !matches!(
        text(v, "environment"),
        "server_only" | "dedicated_server_only"
    ) && v["client_side"] != "unsupported"
}
fn archive_file(v: &Value) -> Result<&Value, String> {
    let files = v["files"].as_array().ok_or("MODPACK_INVALID")?;
    let is_pack = |f: &&Value| {
        text(f, "filename")
            .to_ascii_lowercase()
            .ends_with(".mrpack")
    };
    files
        .iter()
        .filter(is_pack)
        .find(|f| f["primary"] == true)
        .or_else(|| files.iter().find(is_pack))
        .ok_or_else(|| "MODPACK_INVALID".into())
}
pub async fn releases(project_id: &str) -> Result<Vec<PackRelease>, String> {
    catalog::id(Source::Modrinth, project_id)?;
    let api = Api::new(Source::Modrinth, None)?;
    let project = api.get(&format!("project/{project_id}"), &[], None).await?;
    if project["project_type"] != "modpack" {
        return Err("MODPACK_INVALID".into());
    }
    catalog::id(Source::Modrinth, text(&project, "id"))?;
    let value = api
        .get(
            &format!("project/{project_id}/version"),
            &[
                (
                    "loaders",
                    json!(["fabric", "forge", "neoforge"]).to_string(),
                ),
                ("include_changelog", "false".into()),
            ],
            None,
        )
        .await?;
    let values = value.as_array().ok_or("MODPACK_INVALID")?;
    let mut result = Vec::new();
    for v in values {
        if !supported_release(v) || archive_file(v).is_err() || v["project_id"] != project["id"] {
            continue;
        }
        catalog::id(Source::Modrinth, text(v, "id"))?;
        let game_versions = v["game_versions"]
            .as_array()
            .ok_or("MODPACK_INVALID")?
            .iter()
            .filter_map(Value::as_str)
            .filter(|g| super::download::validate_version(g).is_ok())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if game_versions.is_empty() {
            continue;
        }
        result.push(PackRelease {
            id: text(v, "id").into(),
            name: text(v, "name").into(),
            number: text(v, "version_number").into(),
            published: text(v, "date_published").into(),
            channel: text(v, "version_type").into(),
            game_versions,
            loaders: v["loaders"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|v| matches!(*v, "fabric" | "forge" | "neoforge"))
                .map(str::to_owned)
                .collect(),
        });
    }
    result.sort_by(|a, b| {
        (b.channel == "release")
            .cmp(&(a.channel == "release"))
            .then_with(|| b.published.cmp(&a.published))
    });
    Ok(result)
}
pub async fn install(
    data: &Path,
    project_id: &str,
    version_id: &str,
    include_optional: bool,
    job: &Job,
) -> Result<InstallResult, String> {
    job.check()?;
    catalog::id(Source::Modrinth, project_id)?;
    catalog::id(Source::Modrinth, version_id)?;
    job.report("modpack-archive", 0, 0);
    let api = Api::new(Source::Modrinth, None)?;
    let project = api
        .get(&format!("project/{project_id}"), &[], Some(job))
        .await?;
    let version = api
        .get(&format!("version/{version_id}"), &[], Some(job))
        .await?;
    if project["project_type"] != "modpack"
        || project["id"] != version["project_id"]
        || text(&version, "id") != version_id
    {
        return Err("MODPACK_INVALID".into());
    }
    if !supported_release(&version) {
        return Err("MODPACK_LOADER_UNSUPPORTED".into());
    }
    catalog::id(Source::Modrinth, text(&project, "id"))?;
    let archive = archive_file(&version)?;
    let size = archive["size"].as_u64().ok_or("MODPACK_INVALID")?;
    if size == 0 || size > MAX_ARCHIVE as u64 {
        return Err("MODPACK_TOO_LARGE".into());
    }
    let hashes: Hashes =
        serde_json::from_value(archive["hashes"].clone()).map_err(|_| "MODPACK_INVALID")?;
    check_hashes(&hashes)?;
    let url = Url::parse(text(archive, "url")).map_err(|_| "MODPACK_INVALID")?;
    let client = catalog::http()?;
    let bytes = catalog::fetch(
        &client,
        Source::Modrinth,
        None,
        url,
        false,
        size as usize,
        Some(job),
    )
    .await
    .map_err(download_error)?;
    verify(&bytes, size, &hashes)?;
    let plan = plan(&bytes, include_optional, job)?;
    if !version["game_versions"]
        .as_array()
        .is_some_and(|a| a.iter().any(|v| v == &plan.game))
    {
        return Err("MODPACK_INVALID".into());
    }
    let id = format!("modrinth-{}", uuid::Uuid::new_v4());
    let instances = instances_dir(data)?;
    let staging_path = instances.join(format!(".pack-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging_path).map_err(|_| "MODPACK_WRITE_FAILED")?;
    let stage = Staging { path: staging_path };
    let count = AtomicUsize::new(0);
    let total = plan.files.len();
    job.report("modpack-download", 0, total);
    let downloads: Vec<_> = plan
        .files
        .iter()
        .map(|file: &PackFile| {
            let client = &client;
            let stage = &stage;
            let count = &count;
            async move {
                let mut last_error = "CATALOG_NETWORK".to_owned();
                for url in allowed_urls(&file.downloads)? {
                    job.check()?;
                    let result = catalog::fetch(
                        client,
                        Source::Modrinth,
                        None,
                        url,
                        false,
                        file.file_size as usize,
                        Some(job),
                    )
                    .await;
                    let content = match result {
                        Ok(content) => content,
                        Err(error) if error == "OPERATION_CANCELLED" => return Err(error),
                        Err(error) => {
                            last_error = error;
                            continue;
                        }
                    };
                    if let Err(error) = verify(&content, file.file_size, &file.hashes) {
                        last_error = download_error(error);
                        continue;
                    }
                    job.check()?;
                    write_new(&stage.path, &file.path, &content)?;
                    job.report(
                        "modpack-download",
                        count.fetch_add(1, Ordering::Relaxed) + 1,
                        total,
                    );
                    return Ok::<(), String>(());
                }
                Err(last_error)
            }
        })
        .collect();
    stream::iter(downloads)
        .buffer_unordered(3)
        .try_collect::<Vec<_>>()
        .await?;
    unpack_overrides(&bytes, &plan, &stage.path, job)?;
    let mut name = bounded_text(text(&project, "title").trim(), 40);
    if name.is_empty() {
        name = "Modrinth".into();
    }
    let instance = PackInstance {
        id,
        name,
        version: plan.game,
        engine: "java",
        loader: plan.loader,
        modpack: PackOrigin {
            project_id: text(&project, "id").into(),
            version_id: version_id.into(),
            version_name: bounded_text(text(&version, "version_number"), 120),
        },
    };
    fs::write(
        stage.path.join(MANIFEST),
        serde_json::to_vec_pretty(&instance).map_err(|_| "MODPACK_INVALID")?,
    )
    .map_err(|_| "MODPACK_WRITE_FAILED")?;
    job.report("modpack-install", 0, 1);
    publish(&stage, &instances.join(&instance.id), job)?;
    job.report("modpack-install", 1, 1);
    Ok(InstallResult {
        instance,
        files: plan.files.len() + plan.overrides.len(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn accepts_one_supported_loader_and_rejects_mixed_loader_dependencies() {
        for (key, version, kind) in [
            ("forge", "47.4.28", "forge"),
            ("neoforge", "21.1.265", "neoforge"),
            ("fabric-loader", "0.16.14", "fabric"),
        ] {
            let value = serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":"v1","name":"Fixture","dependencies":{"minecraft":"1.21.1",key:version},"files":[]});
            let index: PackIndex = serde_json::from_value(value).unwrap();
            let (_, loader) = dependencies(&index).unwrap();
            assert_eq!(loader.kind, kind);
            assert_eq!(loader.version, version);
        }
        let index: PackIndex = serde_json::from_value(serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":"v1","name":"Fixture","dependencies":{"minecraft":"1.21.1","forge":"52.1.14","fabric-loader":"0.16.14"},"files":[]})).unwrap();
        assert!(dependencies(&index).is_err());
    }
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("midnight-pack-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn job() -> Job {
        Job {
            cancel: Arc::new(AtomicBool::new(false)),
            app: None,
        }
    }
    #[test]
    fn names_fit_persisted_ui_limits_even_with_supplementary_unicode() {
        let name = bounded_text(&"🌙".repeat(40), 40);
        assert_eq!(name.encode_utf16().count(), 40);
        assert_eq!(name.chars().count(), 20);
    }
    fn hashes(bytes: &[u8]) -> Hashes {
        Hashes {
            sha1: format!("{:x}", Sha1::digest(bytes)),
            sha512: format!("{:x}", Sha512::digest(bytes)),
        }
    }
    fn file(path: &str, env: &str) -> Value {
        let h = hashes(b"test");
        json!({"path": path, "hashes": {"sha1": h.sha1, "sha512": h.sha512}, "fileSize": 4,
            "env": {"client": env, "server": "required"}, "downloads": ["https://cdn.modrinth.com/data/project/versions/release/file.jar"]})
    }
    fn index(files: Vec<Value>) -> Value {
        json!({"formatVersion": 1, "game": "minecraft", "name": "Test Pack", "versionId": "1.0",
            "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.14"}, "files": files})
    }
    fn pack(index: Value, extras: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(&index).unwrap())
            .unwrap();
        for (name, bytes) in extras {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    #[test]
    fn client_environment_and_optional_choice_control_downloads() {
        let mut server = file("mods/server.jar", "unsupported");
        server["downloads"] = json!(["https://outside.example/server.jar"]);
        let bytes = pack(
            index(vec![
                file("mods/required.jar", "required"),
                file("mods/optional.jar", "optional"),
                server,
            ]),
            &[],
        );
        let all = plan(&bytes, true, &job()).unwrap();
        assert_eq!(all.files.len(), 2);
        assert_eq!(all.game, "1.21.1");
        assert_eq!(all.loader.version, "0.16.14");
        let required = plan(&bytes, false, &job()).unwrap();
        assert_eq!(required.files.len(), 1);
        assert_eq!(required.files[0].path, "mods/required.jar");
    }
    #[test]
    fn client_overrides_win_independently_of_zip_order_and_server_files_are_skipped() {
        let bytes = pack(
            index(vec![file("config/test.json", "required")]),
            &[
                ("client-overrides/config/test.json", b"client"),
                ("overrides/config/test.json", b"common"),
                ("server-overrides/config/server.json", b"server"),
                ("overrides/options.txt", b"options"),
            ],
        );
        let prepared = plan(&bytes, true, &job()).unwrap();
        assert!(prepared.files.is_empty());
        assert_eq!(prepared.overrides.len(), 2);
        let root = Fixture::new();
        unpack_overrides(&bytes, &prepared, &root.0, &job()).unwrap();
        assert_eq!(
            fs::read(root.0.join("config/test.json")).unwrap(),
            b"client"
        );
        assert_eq!(fs::read(root.0.join("options.txt")).unwrap(), b"options");
        assert!(!root.0.join("config/server.json").exists());
    }
    #[test]
    fn manifest_and_override_paths_cannot_escape_or_replace_launcher_metadata() {
        for path in [
            "../outside",
            "/absolute",
            "C:/outside",
            "mods\\outside",
            "config/../outside",
            "NUL.jar",
            "mods/a:stream",
            "mods/trailing.",
            "mods/a?.jar",
            MANIFEST,
        ] {
            assert!(
                plan(
                    &pack(index(vec![file(path, "required")]), &[]),
                    true,
                    &job()
                )
                .is_err(),
                "{path}"
            );
            let entry = format!("overrides/{path}");
            assert!(
                plan(
                    &pack(index(vec![]), &[(entry.as_str(), b"test")]),
                    true,
                    &job()
                )
                .is_err(),
                "{entry}"
            );
        }
    }
    #[test]
    fn duplicate_paths_are_rejected_even_on_case_sensitive_hosts() {
        let bytes = pack(
            index(vec![
                file("mods/Test.jar", "required"),
                file("mods/test.jar", "required"),
            ]),
            &[],
        );
        assert_eq!(
            plan(&bytes, true, &job()).err().as_deref(),
            Some("MODPACK_DUPLICATE_PATH")
        );
        let bytes = pack(
            index(vec![]),
            &[
                ("overrides/config/A.txt", b"a"),
                ("overrides/config/a.txt", b"b"),
            ],
        );
        assert_eq!(
            plan(&bytes, true, &job()).err().as_deref(),
            Some("MODPACK_DUPLICATE_PATH")
        );
    }
    #[test]
    fn symlinks_in_archives_are_rejected_before_writing() {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "overrides/link",
                "../outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        assert_eq!(
            plan(&bytes, true, &job()).err().as_deref(),
            Some("MODPACK_UNSAFE_PATH")
        );
    }
    #[test]
    fn format_dependencies_and_resource_limits_are_validated() {
        for patch in [
            json!({"formatVersion": 2}),
            json!({"game": "other"}),
            json!({"dependencies": {"minecraft": "../outside", "fabric-loader": "0.16.14"}}),
            json!({"dependencies": {"minecraft": "1.21.1", "forge": "1.0"}}),
            json!({"dependencies": {"minecraft": "1.21.1", "fabric-loader": "../bad"}}),
        ] {
            let mut value = index(vec![]);
            value
                .as_object_mut()
                .unwrap()
                .extend(patch.as_object().unwrap().clone());
            assert!(plan(&pack(value, &[]), true, &job()).is_err());
        }
        let mut oversized = file("mods/test.jar", "required");
        oversized["fileSize"] = json!(MAX_FILE + 1);
        assert_eq!(
            plan(&pack(index(vec![oversized]), &[]), true, &job())
                .err()
                .as_deref(),
            Some("MODPACK_TOO_LARGE")
        );
        assert_eq!(
            plan(
                &pack(
                    index(vec![file("mods/test.jar", "required"); MAX_FILES + 1]),
                    &[]
                ),
                true,
                &job()
            )
            .err()
            .as_deref(),
            Some("MODPACK_TOO_LARGE")
        );
    }
    #[test]
    fn downloads_require_modrinth_origin_size_and_both_hashes() {
        let bytes = b"test";
        let expected = hashes(bytes);
        verify(bytes, 4, &expected).unwrap();
        assert!(verify(bytes, 5, &expected).is_err());
        assert!(verify(b"fake", 4, &expected).is_err());
        let mut bad = expected.clone();
        bad.sha512 = "0".repeat(128);
        assert!(verify(bytes, 4, &bad).is_err());
        let mut bad = expected;
        bad.sha1 = "0".repeat(40);
        assert!(verify(bytes, 4, &bad).is_err());
        for url in [
            "http://cdn.modrinth.com/data/a",
            "https://cdn.modrinth.com.evil.example/data/a",
            "https://user@cdn.modrinth.com/data/a",
            "https://cdn.modrinth.com/private/a",
            "https://127.0.0.1/data/a",
            "https://github.com/example/file.jar",
        ] {
            let mut entry = file("mods/test.jar", "required");
            entry["downloads"] = json!([url]);
            assert_eq!(
                plan(&pack(index(vec![entry]), &[]), true, &job())
                    .err()
                    .as_deref(),
                Some("MODPACK_EXTERNAL_DOWNLOAD"),
                "{url}"
            );
        }
    }
    #[test]
    fn cancellation_and_failed_publish_leave_existing_instances_untouched() {
        let root = Fixture::new();
        let instances = instances_dir(&root.0).unwrap();
        let existing = instances.join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("options.txt"), b"user data").unwrap();
        let staging_path = instances.join(".test-stage");
        fs::create_dir(&staging_path).unwrap();
        {
            let stage = Staging {
                path: staging_path.clone(),
            };
            write_new(&stage.path, "config/test.json", b"new").unwrap();
            let cancelled = job();
            cancelled.cancel.store(true, Ordering::Relaxed);
            assert_eq!(
                publish(&stage, &instances.join("new"), &cancelled).unwrap_err(),
                "OPERATION_CANCELLED"
            );
            assert_eq!(
                publish(&stage, &existing, &job()).unwrap_err(),
                "MODPACK_INSTANCE_EXISTS"
            );
        }
        assert!(!staging_path.exists());
        assert!(!instances.join("new").exists());
        assert_eq!(
            fs::read(existing.join("options.txt")).unwrap(),
            b"user data"
        );
        let cancelled = job();
        cancelled.cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            plan(&pack(index(vec![]), &[]), true, &cancelled)
                .err()
                .as_deref(),
            Some("OPERATION_CANCELLED")
        );
    }
    #[test]
    fn publishing_a_complete_staging_directory_keeps_its_files() {
        let root = Fixture::new();
        let path = root.0.join(".test-stage");
        fs::create_dir(&path).unwrap();
        let destination = root.0.join("installed");
        {
            let stage = Staging { path };
            write_new(&stage.path, "mods/test.jar", b"verified bytes").unwrap();
            publish(&stage, &destination, &job()).unwrap();
        }
        assert_eq!(
            fs::read(destination.join("mods/test.jar")).unwrap(),
            b"verified bytes"
        );
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
    }
}
