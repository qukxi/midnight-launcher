use super::platform::Platform;
use super::{
    download::{client, validate_version},
    manifest::{self, Artifact, VersionMetadata},
};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct Job {
    pub cancel: Arc<AtomicBool>,
    pub app: Option<tauri::AppHandle>,
}
#[derive(Clone, Serialize)]
pub struct Progress {
    pub stage: String,
    pub current: usize,
    pub total: usize,
}
impl Job {
    pub fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("OPERATION_CANCELLED".into())
        } else {
            Ok(())
        }
    }
    pub fn report(&self, stage: &str, current: usize, total: usize) {
        if let Some(app) = &self.app {
            let _ = app.emit(
                "launcher-progress",
                Progress {
                    stage: stage.into(),
                    current,
                    total,
                },
            );
        } else if current == 0 || current == total || current % 500 == 0 {
            eprintln!("{stage}: {current}/{total}");
        }
    }
}
pub struct Installed {
    pub info: VersionMetadata,
    pub classpath: Vec<PathBuf>,
    pub root: PathBuf,
    pub assets: PathBuf,
    pub natives: PathBuf,
    pub logging_argument: Option<String>,
}
#[derive(Deserialize)]
struct Assets {
    objects: std::collections::HashMap<String, Asset>,
}
#[derive(Deserialize)]
struct Asset {
    hash: String,
    size: u64,
}

pub fn safe_path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value.split('/').any(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with(['.', ' '])
                || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        })
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("INSTALL_UNSAFE_PATH".into());
    }
    Ok(path.to_path_buf())
}
pub fn public_url(value: &str) -> Result<(), String> {
    let u = reqwest::Url::parse(value).map_err(|_| "INSTALL_INVALID_URL")?;
    if u.scheme() != "https"
        || u.port_or_known_default() != Some(443)
        || !u.username().is_empty()
        || u.password().is_some()
        || !matches!(
            u.host_str(),
            Some(
                "piston-meta.mojang.com"
                    | "piston-data.mojang.com"
                    | "launcher.mojang.com"
                    | "launchermeta.mojang.com"
                    | "libraries.minecraft.net"
                    | "maven.fabricmc.net"
                    | "maven.minecraftforge.net"
                    | "maven.neoforged.net"
                    | "resources.download.minecraft.net"
            )
        )
    {
        return Err("INSTALL_UNTRUSTED_HOST".into());
    }
    Ok(())
}
async fn hash_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| "CACHE_READ_FAILED")?;
    let mut hasher = Sha1::new();
    let mut length = 0;
    // An inline buffer is stored inside this future and every awaiting launch/IPC future.
    // Keep it on the heap so constructing the command fits Windows' event-thread stack.
    let mut buf = vec![0u8; 65536];
    loop {
        let n = file.read(&mut buf).await.map_err(|_| "CACHE_READ_FAILED")?;
        if n == 0 {
            break;
        }
        length += n as u64;
        hasher.update(&buf[..n]);
    }
    Ok((length, format!("{:x}", hasher.finalize())))
}
pub async fn artifact(
    http: &reqwest::Client,
    url: &str,
    size: u64,
    hash: &str,
    path: &Path,
    job: &Job,
) -> Result<(), String> {
    public_url(url)?;
    job.check()?;
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || size > 512 * 1024 * 1024
    {
        return Err("INSTALL_INVALID_ARTIFACT".into());
    }
    if let Ok((length, checksum)) = hash_file(path).await {
        if length == size && checksum.eq_ignore_ascii_case(hash) {
            return Ok(());
        }
    }
    let parent = path.parent().ok_or("INSTALL_INVALID_PATH")?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    let tmp = path.with_extension("part");
    for attempt in 0..3 {
        job.check()?;
        let result = async {
            let mut response = http
                .get(url)
                .send()
                .await
                .map_err(|_| "INSTALL_NETWORK")?
                .error_for_status()
                .map_err(|_| "INSTALL_HTTP_ERROR")?;
            if response.status().is_redirection() {
                return Err("INSTALL_REDIRECT_REJECTED");
            }
            if response.content_length().is_some_and(|n| n > size) {
                return Err("INSTALL_SIZE_MISMATCH");
            }
            let mut file = tokio::fs::File::create(&tmp)
                .await
                .map_err(|_| "CACHE_WRITE_FAILED")?;
            let mut checksum = Sha1::new();
            let mut length = 0;
            while let Some(chunk) = response.chunk().await.map_err(|_| "INSTALL_NETWORK")? {
                job.check().map_err(|_| "OPERATION_CANCELLED")?;
                length += chunk.len() as u64;
                if length > size {
                    return Err("INSTALL_SIZE_MISMATCH");
                }
                checksum.update(&chunk);
                file.write_all(&chunk)
                    .await
                    .map_err(|_| "CACHE_WRITE_FAILED")?;
            }
            if length != size || format!("{:x}", checksum.finalize()) != hash.to_ascii_lowercase() {
                return Err("INSTALL_CHECKSUM_MISMATCH");
            }
            file.flush().await.map_err(|_| "CACHE_WRITE_FAILED")?;
            drop(file);
            #[cfg(target_os = "windows")]
            if path.exists() {
                tokio::fs::remove_file(path)
                    .await
                    .map_err(|_| "CACHE_WRITE_FAILED")?;
            }
            tokio::fs::rename(&tmp, path)
                .await
                .map_err(|_| "CACHE_WRITE_FAILED")?;
            Ok::<(), &str>(())
        }
        .await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                if attempt == 2 || e == "OPERATION_CANCELLED" {
                    return Err(e.into());
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }
    }
    Err("INSTALL_NETWORK".into())
}
// Rule order matters: the last matching rule wins; absent feature flags are false.
pub fn allowed(rules: &[Value]) -> Result<bool, String> {
    allowed_for(rules, Platform::current()?)
}
pub fn allowed_for(rules: &[Value], platform: &Platform) -> Result<bool, String> {
    if rules.is_empty() {
        return Ok(true);
    }
    let mut allow = false;
    for rule in rules {
        if let Some(os) = rule.get("os") {
            if let Some(name) = os["name"].as_str() {
                if name != platform.os {
                    continue;
                }
            }
            if let Some(arch) = os["arch"].as_str() {
                if !platform.java_arch_matches(arch) {
                    continue;
                }
            }
            if let Some(version) = os["version"].as_str() {
                if platform.version.is_empty() {
                    return Err("INSTALL_OS_VERSION_RULE_UNSUPPORTED".into());
                }
                let expression = regex::Regex::new(version).map_err(|_| "INSTALL_INVALID_RULE")?;
                if !expression.is_match(&platform.version) {
                    continue;
                }
            }
        }
        if let Some(features) = rule["features"].as_object() {
            if features.values().any(|v| v.as_bool() != Some(false)) {
                continue;
            }
        }
        allow = match rule["action"].as_str() {
            Some("allow") => true,
            Some("disallow") => false,
            _ => return Err("INSTALL_INVALID_RULE".into()),
        };
    }
    Ok(allow)
}
pub fn selected_libraries(info: &VersionMetadata) -> Result<Vec<&Artifact>, String> {
    Ok(library_plan(info, Platform::current()?)?.classpath)
}
pub struct LibraryPlan<'a> {
    pub classpath: Vec<&'a Artifact>,
    pub natives: Vec<&'a Artifact>,
}
pub fn library_plan<'a>(
    info: &'a VersionMetadata,
    platform: &Platform,
) -> Result<LibraryPlan<'a>, String> {
    let mut selected = LibraryPlan {
        classpath: Vec::new(),
        natives: Vec::new(),
    };
    for lib in &info.libraries {
        if !allowed_for(&lib.rules, platform)? {
            continue;
        }
        let native = lib
            .name
            .split(':')
            .nth(3)
            .is_some_and(|s| s.starts_with("natives-"));
        if native {
            if lib.name.split(':').nth(3) != Some(platform.native_classifier()) {
                continue;
            }
        }
        if let Some(artifact) = &lib.downloads.artifact {
            safe_path(&artifact.path)?;
            selected.classpath.push(artifact);
            if native {
                selected.natives.push(artifact);
            }
        }
        if let Some(classifier) = lib.natives.get(platform.os) {
            let classifier = classifier.replace("${arch}", "64");
            // Old macOS native archives contain Intel binaries, not Apple Silicon.
            if platform.os == "osx" && !classifier.contains("arm64") {
                return Err("INSTALL_ARM64_VERSION_REQUIRED".into());
            }
            let artifact = lib
                .downloads
                .classifiers
                .get(&classifier)
                .ok_or("INSTALL_NATIVE_NOT_FOUND")?;
            safe_path(&artifact.path)?;
            selected.natives.push(artifact);
        }
    }
    if selected.natives.is_empty() {
        return Err("INSTALL_NATIVE_NOT_FOUND".into());
    }
    Ok(selected)
}
pub async fn install(version: &str, root: &Path, job: &Job) -> Result<Installed, String> {
    let platform = Platform::current()?;
    validate_version(version)?;
    job.check()?;
    job.report("metadata", 0, 0);
    let http = client()?;
    let (info, raw) = manifest::metadata(&http, version).await?;
    if !matches!(info.java_version.major, 17 | 21 | 25) {
        return Err("JAVA_UNSUPPORTED_VERSION".into());
    }
    let selected = library_plan(&info, platform)?;
    let dir = root.join("versions").join(version);
    let jar = dir.join(format!("{version}.jar"));
    let client_artifact = info
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .ok_or("INSTALL_NO_CLIENT")?;
    job.report("client", 0, 1);
    artifact(
        &http,
        &client_artifact.url,
        client_artifact.size,
        &client_artifact.sha1,
        &jar,
        job,
    )
    .await?;
    tokio::fs::write(dir.join(format!("{version}.json")), &raw)
        .await
        .map_err(|_| "CACHE_WRITE_FAILED")?;
    let mut classpath = Vec::new();
    let count = selected.classpath.len();
    for (index, lib) in selected.classpath.iter().enumerate() {
        job.check()?;
        job.report("libraries", index, count);
        let path = root.join("libraries").join(safe_path(&lib.path)?);
        artifact(&http, &lib.url, lib.size, &lib.sha1, &path, job).await?;
        classpath.push(path);
    }
    classpath.push(jar);
    let natives = dir.join(format!("natives-{}", platform.key()));
    let staging = dir.join(format!("natives-staging-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    let extraction = async {
        for (index, lib) in selected.natives.iter().enumerate() {
            job.report("natives", index, selected.natives.len());
            let path = root.join("libraries").join(safe_path(&lib.path)?);
            artifact(&http, &lib.url, lib.size, &lib.sha1, &path, job).await?;
            let output = staging.clone();
            let extract_job = job.clone();
            tokio::task::spawn_blocking(move || {
                super::archive::extract_zip(&path, &output, true, &extract_job)
            })
            .await
            .map_err(|_| "INSTALL_NATIVE_EXTRACT_FAILED")??;
        }
        job.check()?;
        if natives.exists() {
            tokio::fs::remove_dir_all(&natives)
                .await
                .map_err(|_| "CACHE_WRITE_FAILED")?;
        }
        tokio::fs::rename(&staging, &natives)
            .await
            .map_err(|_| "CACHE_WRITE_FAILED")?;
        Ok::<(), String>(())
    }
    .await;
    if let Err(error) = extraction {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    let logging_argument = if let Some(logging) = &info.logging {
        let file = &logging.client.file;
        let path = root.join("log_configs").join(safe_path(&file.id)?);
        artifact(&http, &file.url, file.size, &file.sha1, &path, job).await?;
        if !logging
            .client
            .argument
            .starts_with("-Dlog4j.configurationFile=")
        {
            return Err("INSTALL_INVALID_LOGGING_ARGUMENT".into());
        }
        Some(
            logging
                .client
                .argument
                .replace("${path}", &path.to_string_lossy()),
        )
    } else {
        None
    };
    let assets = root.join("assets");
    let index_path = assets
        .join("indexes")
        .join(format!("{}.json", info.asset_index.id));
    safe_path(&info.asset_index.id)?;
    artifact(
        &http,
        &info.asset_index.url,
        info.asset_index.size,
        &info.asset_index.sha1,
        &index_path,
        job,
    )
    .await?;
    let bytes = tokio::fs::read(&index_path)
        .await
        .map_err(|_| "CACHE_READ_FAILED")?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("INSTALL_ASSET_INDEX_TOO_LARGE".into());
    }
    let index: Assets =
        serde_json::from_slice(&bytes).map_err(|_| "INSTALL_INVALID_ASSET_INDEX")?;
    if index.objects.len() > 100000 {
        return Err("INSTALL_TOO_MANY_ASSETS".into());
    }
    // Deduplicate objects referenced by multiple logical asset names.
    let objects: std::collections::HashMap<String, u64> = index
        .objects
        .into_values()
        .map(|a| (a.hash, a.size))
        .collect();
    let total = objects.len();
    let done = AtomicUsize::new(0);
    job.report("assets", 0, total);
    stream::iter(objects.into_iter().map(|(hash, size)| {
        let http = &http;
        let done = &done;
        let assets = &assets;
        async move {
            if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("INSTALL_INVALID_ASSET_HASH".into());
            }
            let relative = format!("{}/{}", &hash[..2], hash);
            let url = format!("https://resources.download.minecraft.net/{relative}");
            artifact(
                http,
                &url,
                size,
                &hash,
                &assets.join("objects").join(relative),
                job,
            )
            .await?;
            let count = done.fetch_add(1, Ordering::Relaxed) + 1;
            if count % 10 == 0 || count == total {
                job.report("assets", count, total);
            }
            Ok::<(), String>(())
        }
    }))
    .buffer_unordered(12)
    .try_collect::<Vec<_>>()
    .await?;
    job.report("installed", 1, 1);
    Ok(Installed {
        info,
        classpath,
        root: root.to_path_buf(),
        assets,
        natives,
        logging_argument,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cached_artifact_is_verified_across_multiple_buffer_reads() {
        let root = std::env::temp_dir().join(format!("midnight-hash-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir(&root).await.unwrap();
        let path = root.join("cached.jar");
        let bytes: Vec<u8> = (0..200_003).map(|index| (index % 251) as u8).collect();
        tokio::fs::write(&path, &bytes).await.unwrap();
        let expected = format!("{:x}", Sha1::digest(&bytes));
        assert_eq!(
            hash_file(&path).await.unwrap(),
            (bytes.len() as u64, expected.clone())
        );
        let job = Job {
            cancel: Arc::new(AtomicBool::new(false)),
            app: None,
        };
        // A valid cache must succeed without connecting to this nonexistent URL.
        artifact(
            &client().unwrap(),
            "https://piston-data.mojang.com/never-request-this-cached-artifact",
            bytes.len() as u64,
            &expected,
            &path,
            &job,
        )
        .await
        .unwrap();
        tokio::fs::remove_dir_all(&root).await.unwrap();
    }

    #[test]
    fn paths_and_hosts_are_restricted() {
        for p in ["../outside", "/tmp/file", "a/../../x", "a\\b", "C:/x", ""] {
            assert!(safe_path(p).is_err());
        }
        assert!(safe_path("org/lwjgl/a.jar").is_ok());
        assert!(public_url("https://resources.download.minecraft.net/aa/file").is_ok());
        assert!(public_url("https://libraries.minecraft.net.evil.org/a").is_err());
    }
    #[test]
    fn rule_order_and_features() {
        let platform = Platform::mac_arm64();
        assert!(allowed_for(&[], &platform).unwrap());
        assert!(!allowed_for(
            &[serde_json::json!({"action":"allow","features":{"is_demo_user":true}})],
            &platform
        )
        .unwrap());
        assert!(!allowed_for(
            &[
                serde_json::json!({"action":"allow"}),
                serde_json::json!({"action":"disallow","os":{"name":"osx"}})
            ],
            &platform
        )
        .unwrap());
        assert!(!allowed_for(
            &[serde_json::json!({"action":"allow","os":{"arch":"x86"}})],
            &platform
        )
        .unwrap());
    }
    #[test]
    fn official_metadata_selects_only_arm64_natives() {
        let info: VersionMetadata =
            serde_json::from_str(include_str!("../../tests/fixtures/1.21.1.json")).unwrap();
        let libs = library_plan(&info, &Platform::mac_arm64())
            .unwrap()
            .classpath;
        assert!(libs.iter().any(|l| l.path.contains("natives-macos-arm64")));
        assert!(!libs.iter().any(|l| l.path.contains("natives-windows")
            || l.path.contains("natives-linux")
            || l.path.ends_with("natives-macos.jar")));
    }
    #[test]
    fn windows_metadata_excludes_x86_arm_and_mac_natives() {
        let info: VersionMetadata =
            serde_json::from_str(include_str!("../../tests/fixtures/1.21.1.json")).unwrap();
        let plan = library_plan(&info, &Platform::windows_x64("10.0.26200".into())).unwrap();
        assert_eq!(plan.natives.len(), 8);
        assert!(plan
            .natives
            .iter()
            .all(|a| a.path.ends_with("-natives-windows.jar")));
        assert!(!plan
            .classpath
            .iter()
            .any(|a| a.path.contains("natives-macos")
                || a.path.contains("natives-linux")
                || a.path.contains("natives-windows-arm64")
                || a.path.contains("natives-windows-x86")));
    }
    #[test]
    fn windows_version_rules_and_legacy_classifiers() {
        let windows = Platform::windows_x64("10.0.26200.0".into());
        assert!(allowed_for(
            &[serde_json::json!({"action":"allow","os":{"name":"windows","version":"^10\\."}})],
            &windows
        )
        .unwrap());
        assert!(!allowed_for(
            &[serde_json::json!({"action":"allow","os":{"name":"windows","version":"^6\\."}})],
            &windows
        )
        .unwrap());
        let mut info: VersionMetadata =
            serde_json::from_str(include_str!("../../tests/fixtures/1.21.1.json")).unwrap();
        info.libraries = vec![serde_json::from_value(serde_json::json!({
            "name": "test:library:1", "natives": {"windows":"natives-windows-${arch}"},
            "downloads": {"artifact": {"url":"https://libraries.minecraft.net/test/library.jar","sha1":"a".repeat(40),"size":1,"path":"test/library.jar"},
            "classifiers": {"natives-windows-64": {"url":"https://libraries.minecraft.net/test/native.jar","sha1":"b".repeat(40),"size":1,"path":"test/native.jar"}}}
        })).unwrap()];
        let plan = library_plan(&info, &windows).unwrap();
        assert_eq!(plan.classpath.len(), 1);
        assert_eq!(plan.natives[0].path, "test/native.jar");
    }
}
