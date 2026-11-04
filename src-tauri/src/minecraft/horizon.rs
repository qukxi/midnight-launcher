//! Direct integration with Horizon's pinned game release, without its launcher.
//! Resource preparation uses the release's own horizon-prepare and verified Mojang files.
use super::{
    download,
    install::{artifact, safe_path},
    manifest, Job,
};
use futures_util::{stream, StreamExt, TryStreamExt};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tokio::process::{Child, Command};

pub const VERSION: &str = "0.2.97";
const MINECRAFT: &str = "26.3";
// Use a version JSON explicitly accepted by the pinned preparer, even after Mojang republishes it.
const MANIFEST_SHA1: &str = "5df1795e16eea19d75b923d66ecca9ce183353c5";
const CLIENT_SHA1: &str = "e877b6a07acd633fb3bb475002175cec036e7b87";
const SERVER_SHA1: &str = "33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c";
const MAX_ARCHIVE: usize = 64 * 1024 * 1024;

struct Package {
    name: &'static str,
    size: usize,
    sha256: &'static str,
}
fn package(os: &str, arch: &str) -> Result<Package, String> {
    let (name, size, sha256) = match (os, arch) {
        ("windows", "x86_64") => (
            "horizon-windows-x64.zip",
            31775574,
            "29d3cef224aecdafea314cf6a3acb5caf8228edef9a3c5d73a0b4fe3b2696fca",
        ),
        ("macos", "aarch64") => (
            "horizon-macos-arm64.zip",
            29490171,
            "587fb8a653324e6f1b3bf59f1f2227ad99201ccaaf3f31024db25ccd4f37c63e",
        ),
        ("linux", "x86_64") => (
            "horizon-linux-x86_64.tar.gz",
            33468900,
            "16e62f7a9ac95d6be788e92cd5fb50a569926d2378531a4e0ea3a07337b79c11",
        ),
        _ => return Err("HORIZON_UNSUPPORTED_PLATFORM".into()),
    };
    Ok(Package { name, size, sha256 })
}
pub fn validate(version: &str) -> Result<(), String> {
    if version != VERSION {
        return Err("HORIZON_UNSUPPORTED_VERSION".into());
    }
    package(std::env::consts::OS, std::env::consts::ARCH)?;
    Ok(())
}
fn binary(root: &Path, name: &str) -> PathBuf {
    root.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    })
}
fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW: no helper console windows.
    cmd
}
fn release_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && match url.host_str() {
            Some("github.com") => url.path().starts_with(&format!(
                "/horizon-rust/horizon/releases/download/v{VERSION}/"
            )),
            Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com") => true,
            _ => false,
        }
}
async fn fetch_package(package: &Package, job: &Job) -> Result<Vec<u8>, String> {
    let http = reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && release_url(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Untrusted Horizon redirect")
            }
        }))
        .build()
        .map_err(|_| "HORIZON_NETWORK")?;
    let url = format!(
        "https://github.com/horizon-rust/horizon/releases/download/v{VERSION}/{}",
        package.name
    );
    let mut reply = http
        .get(url)
        .send()
        .await
        .map_err(|_| "HORIZON_NETWORK")?
        .error_for_status()
        .map_err(|_| "HORIZON_DOWNLOAD_FAILED")?;
    let mut bytes = Vec::new();
    while let Some(chunk) = reply.chunk().await.map_err(|_| "HORIZON_NETWORK")? {
        job.check()?;
        if bytes.len().saturating_add(chunk.len()) > package.size.min(MAX_ARCHIVE) {
            return Err("HORIZON_CHECKSUM_MISMATCH".into());
        }
        bytes.extend_from_slice(&chunk);
        job.report("horizon-download", bytes.len(), package.size);
    }
    verify_package(&bytes, package)?;
    Ok(bytes)
}
fn verify_package(bytes: &[u8], package: &Package) -> Result<(), String> {
    if bytes.len() != package.size || format!("{:x}", Sha256::digest(bytes)) != package.sha256 {
        return Err("HORIZON_CHECKSUM_MISMATCH".into());
    }
    Ok(())
}
// Only byte-for-byte pinned archives reach the system's ZIP/tar reader. Inspect entries
// before extraction as well; arbitrary URLs, local archives and unverified updates are forbidden.
async fn unpack(archive: &Path, destination: &Path, job: &Job) -> Result<(), String> {
    let tar = if cfg!(windows) {
        PathBuf::from(std::env::var_os("SystemRoot").ok_or("HORIZON_EXTRACT_FAILED")?)
            .join("System32/tar.exe")
    } else {
        PathBuf::from("/usr/bin/tar")
    };
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        command(&tar)
            .arg("-tf")
            .arg(archive)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "HORIZON_EXTRACT_FAILED")?
    .map_err(|_| "HORIZON_EXTRACT_FAILED")?;
    if !output.status.success() {
        return Err("HORIZON_EXTRACT_FAILED".into());
    }
    let listing = std::str::from_utf8(&output.stdout).map_err(|_| "HORIZON_EXTRACT_FAILED")?;
    if listing.lines().count() > 20000 {
        return Err("HORIZON_EXTRACT_FAILED".into());
    }
    for name in listing.lines() {
        safe_path(name.trim_end_matches('/'))?;
    }
    let mut child = command(&tar)
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "HORIZON_EXTRACT_FAILED")?;
    wait(&mut child, job, "HORIZON_EXTRACT_FAILED").await
}
async fn wait(child: &mut Child, job: &Job, failure: &str) -> Result<(), String> {
    loop {
        if let Err(error) = job.check() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
        if let Some(status) = child.try_wait().map_err(|_| failure.to_owned())? {
            return if status.success() {
                Ok(())
            } else {
                Err(failure.to_owned())
            };
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn runtime(data: &Path, job: &Job) -> Result<PathBuf, String> {
    let package = package(std::env::consts::OS, std::env::consts::ARCH)?;
    let parent = data.join("horizon").join(VERSION);
    let folder = package
        .name
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".zip");
    let root = parent.join(folder);
    if tokio::fs::read_to_string(parent.join("verified.sha256"))
        .await
        .ok()
        .as_deref()
        == Some(package.sha256)
        && binary(&root, "horizon-client").is_file()
        && binary(&root, "horizon-prepare").is_file()
    {
        return Ok(root);
    }
    let bytes = fetch_package(&package, job).await?;
    let staging = data
        .join("horizon")
        .join(format!("install-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    let result = async {
        let archive = staging.join(package.name);
        tokio::fs::write(&archive, bytes)
            .await
            .map_err(|_| "CACHE_WRITE_FAILED")?;
        unpack(&archive, &staging, job).await?;
        job.check()?;
        if !binary(&staging.join(folder), "horizon-client").is_file()
            || !binary(&staging.join(folder), "horizon-prepare").is_file()
        {
            return Err("HORIZON_EXTRACT_FAILED".to_owned());
        }
        tokio::fs::remove_file(&archive)
            .await
            .map_err(|_| "CACHE_WRITE_FAILED")?;
        tokio::fs::write(staging.join("verified.sha256"), package.sha256)
            .await
            .map_err(|_| "CACHE_WRITE_FAILED")?;
        if parent.exists() {
            tokio::fs::remove_dir_all(&parent)
                .await
                .map_err(|_| "CACHE_WRITE_FAILED")?;
        }
        tokio::fs::rename(&staging, &parent)
            .await
            .map_err(|_| "CACHE_WRITE_FAILED")?;
        Ok(root)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_dir_all(&staging).await;
    }
    result
}

async fn java(data: &Path, custom: &str, job: &Job) -> Result<PathBuf, String> {
    if cfg!(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64")
    )) {
        return super::java::ensure(&data.join("java"), custom, 25, job).await;
    }
    let java = if !custom.trim().is_empty() {
        let path = PathBuf::from(custom);
        if !path.is_absolute() || !path.is_file() {
            return Err("HORIZON_JAVA_REQUIRED".into());
        }
        path
    } else if let Some(home) = std::env::var_os("JAVA_HOME") {
        binary(&PathBuf::from(home).join("bin"), "java")
    } else {
        PathBuf::from(if cfg!(windows) { "java.exe" } else { "java" })
    };
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        command(&java)
            .args(["-XshowSettings:properties", "-version"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "HORIZON_JAVA_REQUIRED")?
    .map_err(|_| "HORIZON_JAVA_REQUIRED")?;
    let properties = String::from_utf8_lossy(&output.stderr);
    if !output.status.success()
        || !properties
            .lines()
            .any(|line| line.trim() == "java.specification.version = 25")
    {
        return Err("HORIZON_JAVA_REQUIRED".into());
    }
    Ok(java)
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: std::collections::HashMap<String, Asset>,
}
#[derive(Deserialize)]
struct Asset {
    hash: String,
    size: u64,
}
async fn minecraft(data: &Path, job: &Job) -> Result<PathBuf, String> {
    let root = data.join("minecraft");
    let http = download::client()?;
    job.report("metadata", 0, 0);
    let raw = download::fetch(
        &http,
        &format!("https://piston-meta.mojang.com/v1/packages/{MANIFEST_SHA1}/{MINECRAFT}.json"),
        8 * 1024 * 1024,
    )
    .await?;
    download::verify_sha1(&raw, MANIFEST_SHA1)?;
    let info: manifest::VersionMetadata =
        serde_json::from_slice(&raw).map_err(|_| "HORIZON_METADATA_INVALID")?;
    let json: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|_| "HORIZON_METADATA_INVALID")?;
    let dir = root.join("versions").join(MINECRAFT);
    for (kind, checksum, path) in [
        ("client", CLIENT_SHA1, dir.join(format!("{MINECRAFT}.jar"))),
        (
            "server",
            SERVER_SHA1,
            data.join("horizon").join(format!("server-{MINECRAFT}.jar")),
        ),
    ] {
        let entry: manifest::Artifact = serde_json::from_value(json["downloads"][kind].clone())
            .map_err(|_| "HORIZON_METADATA_INVALID")?;
        if entry.sha1 != checksum {
            return Err("HORIZON_METADATA_INVALID".into());
        }
        job.report(kind, 0, 1);
        artifact(&http, &entry.url, entry.size, checksum, &path, job).await?;
    }
    tokio::fs::write(dir.join(format!("{MINECRAFT}.json")), raw)
        .await
        .map_err(|_| "CACHE_WRITE_FAILED")?;
    // The preparer applies its own platform rules when importing. Keeping all listed
    // artifacts also provides the Java-only exporter dependencies on every supported OS.
    for (index, lib) in info.libraries.iter().enumerate() {
        job.report("libraries", index, info.libraries.len());
        if let Some(entry) = &lib.downloads.artifact {
            let path = root.join("libraries").join(safe_path(&entry.path)?);
            artifact(&http, &entry.url, entry.size, &entry.sha1, &path, job).await?;
        }
    }
    let index = &info.asset_index;
    let file = root
        .join("assets/indexes")
        .join(safe_path(&format!("{}.json", index.id))?);
    artifact(&http, &index.url, index.size, &index.sha1, &file, job).await?;
    let bytes = tokio::fs::read(file)
        .await
        .map_err(|_| "CACHE_READ_FAILED")?;
    let assets: AssetIndex =
        serde_json::from_slice(&bytes).map_err(|_| "HORIZON_METADATA_INVALID")?;
    if assets.objects.len() > 100000 {
        return Err("HORIZON_METADATA_INVALID".into());
    }
    let objects: std::collections::HashMap<String, u64> = assets
        .objects
        .into_values()
        .map(|asset| (asset.hash, asset.size))
        .collect();
    let total = objects.len();
    let completed = AtomicUsize::new(0);
    stream::iter(objects.into_iter().map(|(hash, size)| {
        let http = &http;
        let root = &root;
        let completed = &completed;
        async move {
            if hash.len() != 40 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("HORIZON_METADATA_INVALID".into());
            }
            let suffix = format!("{}/{}", &hash[..2], hash);
            artifact(
                http,
                &format!("https://resources.download.minecraft.net/{suffix}"),
                size,
                &hash,
                &root.join("assets/objects").join(&suffix),
                job,
            )
            .await?;
            job.report(
                "assets",
                completed.fetch_add(1, Ordering::Relaxed) + 1,
                total,
            );
            Ok::<(), String>(())
        }
    }))
    .buffer_unordered(16)
    .try_collect::<Vec<_>>()
    .await?;
    Ok(root)
}

async fn prepare_process(
    root: &Path,
    minecraft: &Path,
    java: &Path,
    server: Option<&Path>,
    log: &Path,
    secret: &str,
    job: &Job,
) -> Result<(), String> {
    let mut cmd = command(binary(root, "horizon-prepare"));
    if let Some(server) = server {
        cmd.arg("--prepare").arg("--server-jar").arg(server);
    } else {
        cmd.arg("sounds");
    }
    cmd.arg("--root")
        .arg(root)
        .arg("--minecraft-dir")
        .arg(minecraft)
        .current_dir(root);
    // The upstream preparer probes JAVA_HOME first. Resolve a PATH Java via its
    // own java.home property rather than making assumptions about symlinks.
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        command(java)
            .args(["-XshowSettings:properties", "-version"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "HORIZON_JAVA_REQUIRED")?
    .map_err(|_| "HORIZON_JAVA_REQUIRED")?;
    let text = String::from_utf8_lossy(&output.stderr);
    let home = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("java.home = "))
        .ok_or("HORIZON_JAVA_REQUIRED")?;
    cmd.env("JAVA_HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|_| "HORIZON_PREPARE_FAILED")?;
    let mut streams = Vec::new();
    if let Some(out) = child.stdout.take() {
        streams.push(tokio::spawn(super::launch::log_stream(
            out,
            log.to_owned(),
            secret.to_owned(),
        )));
    }
    if let Some(err) = child.stderr.take() {
        streams.push(tokio::spawn(super::launch::log_stream(
            err,
            log.to_owned(),
            secret.to_owned(),
        )));
    }
    let result = wait(&mut child, job, "HORIZON_PREPARE_FAILED").await;
    for stream in streams {
        // A Java exporter may still hold the pipe after its parent is cancelled.
        // Do not let an inherited pipe keep the launcher operation busy forever.
        if result.is_err() {
            stream.abort();
        }
        let _ = stream.await;
    }
    result
}

pub struct Prepared {
    root: PathBuf,
    resources: PathBuf,
}
impl Prepared {
    pub fn resources(&self) -> &Path {
        &self.resources
    }
}
pub struct MinecraftSource<'a> {
    pub directory: &'a Path,
    pub server_jar: &'a Path,
}

pub async fn prepare(
    data: &Path,
    instance: &str,
    custom_java: &str,
    source: Option<MinecraftSource<'_>>,
    job: &Job,
) -> Result<Prepared, String> {
    validate(VERSION)?;
    let relative = safe_path(instance)?;
    if relative.components().count() != 1 {
        return Err("LAUNCH_INVALID_INSTANCE".into());
    }
    let home = data.join("instances").join(relative).join("horizon");
    let log = data.join("logs").join(format!("minecraft-{instance}.log"));
    tokio::fs::create_dir_all(&home)
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    tokio::fs::create_dir_all(data.join("logs"))
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    let root = runtime(data, job).await?;
    let resources = root.join(".local/horizon-client").join(MINECRAFT);
    if !resources.join("horizon-resources.json").is_file()
        || !resources.join("assets/minecraft/sounds.json").is_file()
    {
        let java = java(data, custom_java, job).await?;
        let (minecraft, server) = if let Some(source) = source {
            (
                source.directory.to_path_buf(),
                source.server_jar.to_path_buf(),
            )
        } else {
            (
                minecraft(data, job).await?,
                data.join("horizon").join(format!("server-{MINECRAFT}.jar")),
            )
        };
        job.report("horizon-prepare", 0, 0);
        prepare_process(&root, &minecraft, &java, Some(&server), &log, "0", job).await?;
        job.report("horizon-sounds", 0, 0);
        prepare_process(&root, &minecraft, &java, None, &log, "0", job).await?;
        if !resources.join("horizon-resources.json").is_file()
            || !resources.join("assets/minecraft/sounds.json").is_file()
        {
            return Err("HORIZON_PREPARE_FAILED".into());
        }
    }
    Ok(Prepared { root, resources })
}

pub async fn start(
    prepared: &Prepared,
    data: &Path,
    instance: &str,
    session: &crate::auth::Session,
    offline_name: Option<&str>,
    job: &Job,
) -> Result<Child, String> {
    job.check()?;
    let home = super::horizon_session::home(data, instance)?;
    let log = data.join("logs").join(format!("minecraft-{instance}.log"));
    let handoff = if offline_name.is_none() {
        Some(super::horizon_session::SessionFile::create(&home, session)?)
    } else {
        None
    };
    let mut cmd = command(binary(&prepared.root, "horizon-client"));
    cmd.arg("--resources")
        .arg(&prepared.resources)
        .arg("--name")
        .arg(offline_name.unwrap_or(&session.profile.name))
        .env_remove("HORIZON_SESSION_FILE")
        .env("HORIZON_HOME", &home)
        .current_dir(&home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);
    if let Some(file) = &handoff {
        cmd.arg("--session-file").arg(file.path());
    }
    let mut child = cmd.spawn().map_err(|_| "HORIZON_START_FAILED")?;
    if let Some(out) = child.stdout.take() {
        tokio::spawn(super::launch::log_stream(
            out,
            log.clone(),
            session.token.clone(),
        ));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(super::launch::log_stream(err, log, session.token.clone()));
    }
    if let Some(file) = &handoff {
        if let Err(error) = super::horizon_session::wait_consumed(file, &mut child, job).await {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
    }
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_mojang_metadata_matches_the_preparer_and_both_game_archives() {
        let raw = include_bytes!("../../tests/fixtures/horizon-26.3.json");
        download::verify_sha1(raw, MANIFEST_SHA1).unwrap();
        let info: serde_json::Value = serde_json::from_slice(raw).unwrap();
        assert_eq!(info["id"], MINECRAFT);
        assert_eq!(info["downloads"]["client"]["sha1"], CLIENT_SHA1);
        assert_eq!(info["downloads"]["server"]["sha1"], SERVER_SHA1);
        assert_eq!(info["javaVersion"]["majorVersion"], 25);
    }
    #[test]
    fn platform_and_release_are_pinned() {
        for (os, arch) in [
            ("windows", "x86_64"),
            ("linux", "x86_64"),
            ("macos", "aarch64"),
        ] {
            let p = package(os, arch).unwrap();
            assert_eq!(p.sha256.len(), 64);
            assert!(p.size < MAX_ARCHIVE);
            assert!(verify_package(b"tampered", &p).is_err());
        }
        assert!(package("windows", "aarch64").is_err());
        assert!(validate("../other").is_err());
    }
    #[test]
    fn checksum_rejects_same_size_corruption() {
        let package = Package {
            name: "test",
            size: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert!(verify_package(b"abc", &package).is_ok());
        assert!(verify_package(b"abd", &package).is_err());
    }
    #[test]
    fn download_redirects_cannot_escape_release_hosts() {
        for url in [
            "http://github.com/x",
            "https://github.com/evil/horizon/releases/download/v0.2.97/a",
            "https://github.com/horizon-rust/horizon/releases/download/v0.2.96/a",
            "https://user@release-assets.githubusercontent.com/a",
            "https://release-assets.githubusercontent.com.evil.test/a",
        ] {
            assert!(!release_url(&reqwest::Url::parse(url).unwrap()), "{url}");
        }
        assert!(release_url(&reqwest::Url::parse("https://github.com/horizon-rust/horizon/releases/download/v0.2.97/horizon-windows-x64.zip").unwrap()));
    }
    #[tokio::test]
    async fn cancellation_stops_preparation_helpers() {
        let mut child = command(std::env::current_exe().unwrap())
            .arg("--list")
            .kill_on_drop(true)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let job = Job {
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            app: None,
        };
        assert_eq!(
            wait(&mut child, &job, "FAILED").await.unwrap_err(),
            "OPERATION_CANCELLED"
        );
        assert!(child.try_wait().unwrap().is_some());
    }
}
