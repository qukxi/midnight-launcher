//! Modern Forge/NeoForge installers. Only verified official artifacts are executed.
//! Client processors run directly; Mojang mappings are fetched by our restricted downloader.
use super::{
    download,
    install::{self, Installed, Job},
    loader::Loader,
    manifest::{Arguments, Artifact, Library},
    platform,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Installer {
    spec: u32,
    minecraft: String,
    version: String,
    #[serde(default)]
    data: HashMap<String, Sided>,
    #[serde(default)]
    processors: Vec<Processor>,
    libraries: Vec<Library>,
}
#[derive(Deserialize)]
struct Sided {
    client: String,
}
#[derive(Deserialize)]
struct Processor {
    jar: String,
    #[serde(default)]
    sides: Vec<String>,
    #[serde(default)]
    classpath: Vec<String>,
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
}
#[derive(Deserialize)]
struct Runtime {
    id: String,
    #[serde(rename = "inheritsFrom")]
    inherits: String,
    #[serde(rename = "mainClass")]
    main: String,
    arguments: Arguments,
    libraries: Vec<Library>,
}
#[derive(Deserialize, Serialize)]
struct Output {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Deserialize, Serialize)]
struct Receipt {
    installer: String,
    outputs: Vec<Output>,
}

fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._+-".contains(&c))
}
pub(super) fn maven_path(name: &str) -> Result<String, String> {
    let (coordinate, extension) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<_> = coordinate.split(':').collect();
    if !(3..=4).contains(&parts.len())
        || !parts.iter().all(|p| component(p))
        || !parts[0].split('.').all(component)
        || !matches!(extension, "jar" | "zip" | "txt" | "lzma")
    {
        return Err("LOADER_INVALID_LIBRARY".into());
    }
    let classifier = parts.get(3).map(|p| format!("-{p}")).unwrap_or_default();
    let path = format!(
        "{}/{}/{}/{}-{}{classifier}.{extension}",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2]
    );
    install::safe_path(&path)?;
    Ok(path)
}
fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for part in install::safe_path(relative)?.components() {
        path.push(part);
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if super::mods::is_link(&meta) {
                return Err("INSTALL_UNSAFE_PATH".into());
            }
        }
    }
    Ok(path)
}
async fn cancelled(job: &Job) {
    loop {
        if job.check().is_err() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn fetch(
    http: &reqwest::Client,
    url: &str,
    limit: usize,
    job: &Job,
) -> Result<Vec<u8>, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "LOADER_UNTRUSTED_HOST")?;
    if parsed.host_str() == Some("files.minecraftforge.net") {
        if url != "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json" {
            return Err("LOADER_UNTRUSTED_HOST".into());
        }
    } else {
        install::public_url(url)?;
    }
    job.check()?;
    let response = tokio::select! {
        _ = cancelled(job) => return Err("OPERATION_CANCELLED".into()),
        r = http.get(url).send() => r.map_err(|_| "LOADER_NETWORK")?,
    };
    let mut response = response
        .error_for_status()
        .map_err(|_| "LOADER_HTTP_ERROR")?;
    if !response.status().is_success() {
        return Err("LOADER_HTTP_ERROR".into());
    }
    let mut bytes = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = cancelled(job) => return Err("OPERATION_CANCELLED".into()),
            c = response.chunk() => c.map_err(|_| "LOADER_NETWORK")?,
        };
        let Some(chunk) = chunk else { break };
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("LOADER_RESPONSE_TOO_LARGE".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub(super) fn neo_matches(game: &str, release: &str) -> bool {
    let parts: Vec<_> = game.split('.').collect();
    let prefix = if parts.first() == Some(&"1") && (2..=3).contains(&parts.len()) {
        format!("{}.{}.", parts[1], parts.get(2).unwrap_or(&"0"))
    } else if (2..=3).contains(&parts.len()) && parts[0].parse::<u32>().is_ok_and(|v| v >= 26) {
        format!(
            "{}.{}.{}.",
            parts[0],
            parts[1],
            parts.get(2).unwrap_or(&"0")
        )
    } else {
        return false;
    };
    release.starts_with(&prefix)
}
fn release_key(version: &str) -> (bool, Vec<u32>, String) {
    (
        !version.contains('-'),
        version
            .split(['.', '-'])
            .map(|p| p.parse().unwrap_or(0))
            .collect(),
        version.to_string(),
    )
}
pub(super) fn parse_versions(kind: &str, game: &str, bytes: &[u8]) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "LOADER_INVALID_METADATA")?;
    let entries = if kind == "neoforge" {
        value["versions"].as_array()
    } else {
        value[game].as_array()
    };
    let mut versions: Vec<String> = entries
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|version| {
            let version = if kind == "forge" {
                version.strip_prefix(&format!("{game}-"))?
            } else {
                if !neo_matches(game, version) {
                    return None;
                }
                version
            };
            (component(version) && version.as_bytes()[0].is_ascii_digit())
                .then(|| version.to_string())
        })
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(release_key(v)));
    versions.dedup();
    Ok(versions)
}
pub async fn versions(kind: &str, game: &str) -> Result<Vec<String>, String> {
    download::validate_version(game)?;
    if !matches!(kind, "forge" | "neoforge") {
        return Err("LOADER_UNSUPPORTED".into());
    }
    let numbers: Vec<_> = game
        .split('.')
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();
    let modern = numbers.first().is_some_and(|v| *v >= 26)
        || (numbers.first() == Some(&1) && numbers.get(1).is_some_and(|v| *v >= 18));
    if !modern {
        return Ok(Vec::new());
    }
    let job = Job {
        cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        app: None,
    };
    let url = if kind == "forge" {
        "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json"
    } else {
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
    };
    parse_versions(
        kind,
        game,
        &fetch(&download::client()?, url, 16 * 1024 * 1024, &job).await?,
    )
}
fn installer_url(loader: &Loader, game: &str) -> Result<String, String> {
    if !component(&loader.version) || !loader.version.as_bytes()[0].is_ascii_digit() {
        return Err("LOADER_INVALID_VERSION".into());
    }
    Ok(match loader.kind.as_str() {
        "neoforge" if neo_matches(game, &loader.version) => format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{0}/neoforge-{0}-installer.jar", loader.version),
        "forge" => format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{game}-{0}/forge-{game}-{0}-installer.jar", loader.version),
        _ => return Err("LOADER_VERSION_MISMATCH".into()),
    })
}
fn entry(jar: &Path, name: &str, limit: usize) -> Result<Vec<u8>, String> {
    install::safe_path(name)?;
    let file = std::fs::File::open(jar).map_err(|_| "LOADER_INSTALLER_READ_FAILED")?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "LOADER_INVALID_INSTALLER")?;
    let mut file = zip.by_name(name).map_err(|_| "LOADER_INVALID_INSTALLER")?;
    if file.size() > limit as u64 || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
        return Err("LOADER_RESPONSE_TOO_LARGE".into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "LOADER_INVALID_INSTALLER")?;
    if bytes.len() > limit {
        return Err("LOADER_RESPONSE_TOO_LARGE".into());
    }
    Ok(bytes)
}
fn main_class(jar: &Path) -> Result<String, String> {
    let text = String::from_utf8(entry(jar, "META-INF/MANIFEST.MF", 128 * 1024)?)
        .map_err(|_| "LOADER_INVALID_INSTALLER")?;
    let unfolded = text.replace("\r\n ", "").replace("\n ", "");
    let main = unfolded
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class: "))
        .ok_or("LOADER_INVALID_PROCESSOR")?
        .trim();
    if main.len() > 256
        || !main.split('.').all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_$".contains(&b))
        })
    {
        return Err("LOADER_INVALID_PROCESSOR".into());
    }
    Ok(main.to_string())
}
fn hash_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = std::fs::File::open(path).map_err(|_| "LOADER_PROCESSOR_OUTPUT_MISSING")?;
    let mut buf = vec![0u8; 65536];
    let mut hash = Sha256::new();
    let mut size = 0;
    loop {
        let count = file
            .read(&mut buf)
            .map_err(|_| "LOADER_PROCESSOR_OUTPUT_MISSING")?;
        if count == 0 {
            break;
        }
        hash.update(&buf[..count]);
        size += count as u64;
    }
    Ok((size, format!("{:x}", hash.finalize())))
}
fn cache_valid(work: &Path, installer: &str, patched: &Path) -> bool {
    let result = (|| -> Result<bool, String> {
        let path = inside(work, "receipt.json")?;
        if std::fs::metadata(&path)
            .map_err(|_| "LOADER_INVALID_RECEIPT")?
            .len()
            > 1024 * 1024
        {
            return Ok(false);
        }
        let bytes = std::fs::read(&path).map_err(|_| "LOADER_INVALID_RECEIPT")?;
        if bytes.len() > 1024 * 1024 {
            return Ok(false);
        }
        let receipt: Receipt =
            serde_json::from_slice(&bytes).map_err(|_| "LOADER_INVALID_RECEIPT")?;
        if receipt.installer != installer
            || receipt.outputs.is_empty()
            || receipt.outputs.len() > 128
        {
            return Ok(false);
        }
        if !receipt
            .outputs
            .iter()
            .any(|o| inside(work, &o.path).is_ok_and(|p| p == patched))
        {
            return Ok(false);
        }
        for output in receipt.outputs {
            let (size, hash) = hash_file(&inside(work, &output.path)?)?;
            if output.size != size || output.sha256 != hash {
                return Ok(false);
            }
        }
        Ok(true)
    })();
    result.unwrap_or(false)
}
fn expand(value: &str, vars: &HashMap<String, String>, work: &Path) -> Result<String, String> {
    if let Some(name) = value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return Ok(inside(work, &format!("libraries/{}", maven_path(name)?))?
            .to_string_lossy()
            .into());
    }
    let mut output = value.to_string();
    for (key, value) in vars {
        output = output.replace(&format!("{{{key}}}"), value);
    }
    if output.contains(['{', '}']) || output.len() > 32768 {
        return Err("LOADER_INVALID_PROCESSOR".into());
    }
    if output
        .replace('\\', "/")
        .split('/')
        .any(|p| p == "." || p == "..")
        || output.contains("://")
    {
        return Err("LOADER_INVALID_PROCESSOR".into());
    }
    if Path::new(&output).is_absolute()
        && !Path::new(&output).starts_with(work)
        && vars
            .get("MINECRAFT_JAR")
            .is_none_or(|allowed| allowed != &output)
    {
        return Err("LOADER_INVALID_PROCESSOR".into());
    }
    Ok(output)
}
async fn libraries(
    libs: &[Library],
    work: &Path,
    installer: &Path,
    http: &reqwest::Client,
    job: &Job,
) -> Result<(), String> {
    if libs.len() > 512 {
        return Err("LOADER_INVALID_METADATA".into());
    }
    for (i, lib) in libs.iter().enumerate() {
        job.check()?;
        if !install::allowed(&lib.rules)? {
            continue;
        }
        let Some(artifact) = &lib.downloads.artifact else {
            continue;
        };
        let relative = maven_path(&lib.name)?;
        if artifact.path != relative {
            return Err("LOADER_INVALID_LIBRARY".into());
        }
        let target = inside(work, &format!("libraries/{relative}"))?;
        if artifact.url.is_empty() {
            let bytes = entry(installer, &format!("maven/{relative}"), 128 * 1024 * 1024)?;
            download::verify_sha1(&bytes, &artifact.sha1)?;
            if bytes.len() as u64 != artifact.size {
                return Err("INSTALL_SIZE_MISMATCH".into());
            }
            tokio::fs::create_dir_all(target.parent().ok_or("INSTALL_UNSAFE_PATH")?)
                .await
                .map_err(|_| "CACHE_CREATE_FAILED")?;
            tokio::fs::write(&target, &bytes)
                .await
                .map_err(|_| "CACHE_WRITE_FAILED")?;
        } else {
            install::artifact(
                http,
                &artifact.url,
                artifact.size,
                &artifact.sha1,
                &target,
                job,
            )
            .await?;
        }
        job.report("loader-libraries", i + 1, libs.len());
    }
    Ok(())
}
async fn run_processor(
    java: &Path,
    work: &Path,
    processor: &Processor,
    args: &[String],
    index: usize,
    job: &Job,
) -> Result<(), String> {
    if !processor.jar.starts_with("net.minecraftforge:")
        && !processor.jar.starts_with("net.neoforged:")
        && !processor.jar.starts_with("net.neoforged.installertools:")
    {
        return Err("LOADER_INVALID_PROCESSOR".into());
    }
    if processor.classpath.len() > 128 || args.len() > 256 {
        return Err("LOADER_INVALID_PROCESSOR".into());
    }
    let jar = inside(work, &format!("libraries/{}", maven_path(&processor.jar)?))?;
    let mut paths = vec![jar.clone()];
    for name in &processor.classpath {
        paths.push(inside(work, &format!("libraries/{}", maven_path(name)?))?);
    }
    let classpath = std::env::join_paths(paths).map_err(|_| "LOADER_INVALID_LIBRARY")?;
    let log_path = inside(work, &format!("processor-{index}.log"))?;
    let log = std::fs::File::create(log_path).map_err(|_| "CACHE_WRITE_FAILED")?;
    let mut child = platform::command(java)
        .arg("-Xmx2G")
        .arg("-Djava.awt.headless=true")
        .arg(format!("-Djava.io.tmpdir={}", work.display()))
        .arg("-cp")
        .arg(classpath)
        .arg(main_class(&jar)?)
        .args(args)
        .current_dir(work)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().map_err(|_| "CACHE_WRITE_FAILED")?,
        ))
        .stderr(Stdio::from(log))
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "LOADER_PROCESSOR_START_FAILED")?;
    let started = Instant::now();
    loop {
        job.check()?;
        if let Some(status) = child.try_wait().map_err(|_| "LOADER_PROCESSOR_FAILED")? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "LOADER_PROCESSOR_FAILED: {} (step {})",
                    processor.jar,
                    index + 1
                ))
            };
        }
        if started.elapsed() > Duration::from_secs(20 * 60) {
            return Err("LOADER_PROCESSOR_TIMEOUT".into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
pub async fn apply(
    installed: &mut Installed,
    loader: &Loader,
    java: &Path,
    job: &Job,
) -> Result<(), String> {
    let game = installed.info.id.clone();
    let url = installer_url(loader, &game)?;
    tokio::fs::create_dir_all(&installed.root)
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    let root = dunce::canonicalize(&installed.root).map_err(|_| "CACHE_CREATE_FAILED")?;
    let work = inside(
        &root,
        &format!("loader-cache/{}/{game}-{}", loader.kind, loader.version),
    )?;
    tokio::fs::create_dir_all(&work)
        .await
        .map_err(|_| "CACHE_CREATE_FAILED")?;
    let http = download::client()?;
    job.report("loader", 0, 1);
    let checksum = String::from_utf8(fetch(&http, &format!("{url}.sha1"), 512, job).await?)
        .map_err(|_| "LOADER_INVALID_HASH")?;
    let checksum = checksum
        .split_whitespace()
        .next()
        .ok_or("LOADER_INVALID_HASH")?
        .to_lowercase();
    if checksum.len() != 40 || !checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("LOADER_INVALID_HASH".into());
    }
    let installer = inside(&work, "installer.jar")?;
    let head = http.head(&url).send().await.map_err(|_| "LOADER_NETWORK")?;
    if !head.status().is_success() {
        return Err("LOADER_HTTP_ERROR".into());
    }
    let size = head
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .ok_or("LOADER_INVALID_SIZE")?;
    if size > 64 * 1024 * 1024 {
        return Err("LOADER_RESPONSE_TOO_LARGE".into());
    }
    install::artifact(&http, &url, size, &checksum, &installer, job).await?;
    let profile: Installer =
        serde_json::from_slice(&entry(&installer, "install_profile.json", 4 * 1024 * 1024)?)
            .map_err(|_| "LOADER_INVALID_METADATA")?;
    let runtime: Runtime =
        serde_json::from_slice(&entry(&installer, "version.json", 4 * 1024 * 1024)?)
            .map_err(|_| "LOADER_INVALID_METADATA")?;
    if profile.spec != 1
        || profile.minecraft != game
        || runtime.inherits != game
        || runtime.id != profile.version
        || profile.processors.len() > 64
        || !["cpw.mods.", "net.minecraftforge.", "net.neoforged."]
            .iter()
            .any(|p| runtime.main.starts_with(p))
    {
        return Err("LOADER_VERSION_MISMATCH".into());
    }
    libraries(&profile.libraries, &work, &installer, &http, job).await?;
    libraries(&runtime.libraries, &work, &installer, &http, job).await?;
    let client_jar = root
        .join("versions")
        .join(&game)
        .join(format!("{game}.jar"));
    let mut vars = HashMap::from([
        ("ROOT".into(), work.to_string_lossy().to_string()),
        ("SIDE".into(), "client".into()),
        (
            "MINECRAFT_JAR".into(),
            client_jar.to_string_lossy().to_string(),
        ),
        ("INSTALLER".into(), installer.to_string_lossy().to_string()),
    ]);
    for (key, sided) in &profile.data {
        if !key
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            || vars.contains_key(key)
        {
            return Err("LOADER_INVALID_METADATA".into());
        }
        let value = &sided.client;
        let resolved =
            if let Some(literal) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
                literal.to_string()
            } else if let Some(name) = value.strip_prefix('/') {
                let path = inside(&work, name)?;
                tokio::fs::create_dir_all(path.parent().ok_or("INSTALL_UNSAFE_PATH")?)
                    .await
                    .map_err(|_| "CACHE_CREATE_FAILED")?;
                tokio::fs::write(&path, entry(&installer, name, 64 * 1024 * 1024)?)
                    .await
                    .map_err(|_| "CACHE_WRITE_FAILED")?;
                path.to_string_lossy().to_string()
            } else if value.starts_with('[') {
                expand(value, &vars, &work)?
            } else {
                return Err("LOADER_INVALID_METADATA".into());
            };
        let output = PathBuf::from(&resolved);
        if output.starts_with(work.join("libraries")) {
            tokio::fs::create_dir_all(output.parent().ok_or("INSTALL_UNSAFE_PATH")?)
                .await
                .map_err(|_| "CACHE_CREATE_FAILED")?;
        }
        vars.insert(key.clone(), resolved);
    }
    let patched = PathBuf::from(vars.get("PATCHED").ok_or("LOADER_INVALID_METADATA")?);
    if !patched.starts_with(work.join("libraries")) {
        return Err("INSTALL_UNSAFE_PATH".into());
    }
    if !cache_valid(&work, &checksum, &patched) {
        let processors: Vec<_> = profile
            .processors
            .iter()
            .filter(|p| p.sides.is_empty() || p.sides.iter().any(|s| s == "client"))
            .collect();
        for (index, processor) in processors.iter().enumerate() {
            job.check()?;
            job.report("loader-processors", index, processors.len());
            let args: Vec<_> = processor
                .args
                .iter()
                .map(|a| expand(a, &vars, &work))
                .collect::<Result<_, _>>()?;
            if processor.args.iter().any(|a| a == "DOWNLOAD_MOJMAPS") {
                let position = args
                    .iter()
                    .position(|a| a == "--output")
                    .ok_or("LOADER_INVALID_PROCESSOR")?;
                let output =
                    PathBuf::from(args.get(position + 1).ok_or("LOADER_INVALID_PROCESSOR")?);
                if !output.starts_with(work.join("libraries")) {
                    return Err("INSTALL_UNSAFE_PATH".into());
                }
                let raw = tokio::fs::read(
                    root.join("versions")
                        .join(&game)
                        .join(format!("{game}.json")),
                )
                .await
                .map_err(|_| "LOADER_INVALID_METADATA")?;
                let metadata: Value =
                    serde_json::from_slice(&raw).map_err(|_| "LOADER_INVALID_METADATA")?;
                let mappings: Artifact =
                    serde_json::from_value(metadata["downloads"]["client_mappings"].clone())
                        .map_err(|_| "LOADER_INVALID_METADATA")?;
                install::artifact(
                    &http,
                    &mappings.url,
                    mappings.size,
                    &mappings.sha1,
                    &output,
                    job,
                )
                .await?;
            } else {
                run_processor(java, &work, processor, &args, index, job).await?;
            }
            for (path, sha) in &processor.outputs {
                let path = PathBuf::from(expand(path, &vars, &work)?);
                if !path.starts_with(&work) {
                    return Err("INSTALL_UNSAFE_PATH".into());
                }
                let expected = expand(sha, &vars, &work)?;
                let bytes = tokio::fs::read(path)
                    .await
                    .map_err(|_| "LOADER_PROCESSOR_OUTPUT_MISSING")?;
                download::verify_sha1(&bytes, &expected)?;
            }
        }
        let mut outputs = Vec::new();
        let mut seen = HashSet::new();
        for value in vars.values() {
            let path = PathBuf::from(value);
            if path.starts_with(work.join("libraries"))
                && path.is_file()
                && seen.insert(path.clone())
            {
                let (size, sha256) = hash_file(&path)?;
                let relative = path
                    .strip_prefix(&work)
                    .map_err(|_| "INSTALL_UNSAFE_PATH")?
                    .to_string_lossy()
                    .replace('\\', "/");
                outputs.push(Output {
                    path: relative,
                    size,
                    sha256,
                });
            }
        }
        if !patched.is_file() || outputs.is_empty() {
            return Err("LOADER_PROCESSOR_OUTPUT_MISSING".into());
        }
        let receipt = serde_json::to_vec(&Receipt {
            installer: checksum,
            outputs,
        })
        .map_err(|_| "CACHE_WRITE_FAILED")?;
        job.check()?;
        tokio::fs::write(inside(&work, "receipt.json")?, receipt)
            .await
            .map_err(|_| "CACHE_WRITE_FAILED")?;
    }
    job.check()?;
    merge_runtime(installed, runtime, &work, &vars)?;
    job.report("loader", 1, 1);
    Ok(())
}

fn merge_runtime(
    installed: &mut Installed,
    runtime: Runtime,
    work: &Path,
    vars: &HashMap<String, String>,
) -> Result<(), String> {
    let game = &installed.info.id;
    let replacements: HashSet<_> = runtime
        .libraries
        .iter()
        .map(|l| l.name.split(':').take(2).collect::<Vec<_>>().join(":"))
        .collect();
    let overridden: HashSet<_> = installed
        .info
        .libraries
        .iter()
        .filter(|l| replacements.contains(&l.name.split(':').take(2).collect::<Vec<_>>().join(":")))
        .filter_map(|l| l.downloads.artifact.as_ref())
        .map(|a| installed.root.join("libraries").join(&a.path))
        .collect();
    let bootstrap = runtime.main == "cpw.mods.bootstraplauncher.BootstrapLauncher";
    installed.classpath.retain(|p| {
        !overridden.contains(p)
            && (!bootstrap
                || p.file_name().and_then(|s| s.to_str()) != Some(&format!("{game}.jar")))
    });
    let mut paths = Vec::new();
    for lib in &runtime.libraries {
        if install::allowed(&lib.rules)? {
            if let Some(a) = &lib.downloads.artifact {
                paths.push(inside(&work, &format!("libraries/{}", a.path))?);
            }
        }
    }
    // Both FML generations locate patched Minecraft in the private Maven repository.
    // Adding it here creates duplicate modules in ModLauncher, and makes newer FML
    // misidentify the production install as a development environment. New FML
    // keeps the original client on the classpath to detect the client distribution.
    if let Some(extra) = vars.get("MC_EXTRA") {
        paths.push(PathBuf::from(extra));
    }
    paths.extend(std::mem::take(&mut installed.classpath));
    installed.classpath = paths;
    let library_dir = work.join("libraries").to_string_lossy().to_string();
    installed.info.main_class = runtime.main;
    for value in runtime.arguments.jvm {
        let text = value
            .as_str()
            .ok_or("LOADER_INVALID_METADATA")?
            .replace("${library_directory}", &library_dir);
        installed.info.arguments.jvm.push(Value::String(text));
    }
    installed.info.arguments.game.extend(runtime.arguments.game);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maven_coordinates_support_classifiers_and_reject_traversal() {
        assert_eq!(
            maven_path("net.neoforged:neoform:1.21.1:mappings@txt").unwrap(),
            "net/neoforged/neoform/1.21.1/neoform-1.21.1-mappings.txt"
        );
        for value in [
            "a:b:../bad",
            "a:b:1:../../x",
            "a..b:c:1",
            "a:b:1@exe",
            "a:b:1:c:d",
            "a:b:1@jar/../../x",
        ] {
            assert!(maven_path(value).is_err(), "{value}");
        }
    }
    #[test]
    fn versions_match_exact_minecraft_releases_and_sort_numerically() {
        assert!(neo_matches("1.21.1", "21.1.265"));
        assert!(!neo_matches("1.21.1", "21.10.1"));
        assert!(neo_matches("26.3", "26.3.0.100"));
        assert!(!neo_matches("26.3", "26.3.1.2"));
        let versions = parse_versions(
            "forge",
            "1.20.1",
            br#"{"1.20.1":["1.20.1-47.4.9","1.20.1-47.4.28","1.21.1-52.0.1","../evil"]}"#,
        )
        .unwrap();
        assert_eq!(versions, ["47.4.28", "47.4.9"]);
    }
    #[test]
    fn processor_expansion_rejects_unknown_tokens_and_unsafe_coordinates() {
        let root = std::env::temp_dir().join("midnight-processor-test");
        let vars = HashMap::from([("SIDE".into(), "client".into())]);
        assert_eq!(expand("{SIDE}", &vars, &root).unwrap(), "client");
        assert!(expand("{UNKNOWN}", &vars, &root).is_err());
        assert!(expand("[x:y:../../escape]", &vars, &root).is_err());
        let vars = HashMap::from([("ROOT".into(), root.to_string_lossy().to_string())]);
        assert!(expand("{ROOT}/../escape", &vars, &root).is_err());
        assert!(expand("https://evil.example/tool.jar", &vars, &root).is_err());
    }
    #[test]
    fn official_profiles_cover_forge_and_both_generations_of_neoforge() {
        for (install, runtime) in [
            (
                include_str!("../../tests/fixtures/forge-1.20.1-install_profile.json"),
                include_str!("../../tests/fixtures/forge-1.20.1-version.json"),
            ),
            (
                include_str!("../../tests/fixtures/neoforge-1.21.1-install_profile.json"),
                include_str!("../../tests/fixtures/neoforge-1.21.1-version.json"),
            ),
            (
                include_str!("../../tests/fixtures/neoforge-26.3-install_profile.json"),
                include_str!("../../tests/fixtures/neoforge-26.3-version.json"),
            ),
        ] {
            let install: Installer = serde_json::from_str(install).unwrap();
            let runtime: Runtime = serde_json::from_str(runtime).unwrap();
            assert_eq!(install.spec, 1);
            assert_eq!(install.minecraft, runtime.inherits);
            assert_eq!(install.version, runtime.id);
            assert!(install.data["PATCHED"].client.starts_with('['));
            assert!(install
                .processors
                .iter()
                .any(|p| p.sides.is_empty() || p.sides.iter().any(|s| s == "client")));
            for lib in install.libraries.iter().chain(&runtime.libraries) {
                if let Some(a) = &lib.downloads.artifact {
                    assert_eq!(maven_path(&lib.name).unwrap(), a.path);
                    if !a.url.is_empty() {
                        install::public_url(&a.url).unwrap();
                    }
                }
            }
        }
    }
    #[test]
    fn runtime_profiles_keep_production_discovery_and_replace_conflicting_libraries() {
        for profile in [
            include_str!("../../tests/fixtures/forge-1.20.1-version.json"),
            include_str!("../../tests/fixtures/neoforge-1.21.1-version.json"),
            include_str!("../../tests/fixtures/neoforge-26.3-version.json"),
        ] {
            let runtime: Runtime = serde_json::from_str(profile).unwrap();
            let bootstrap = runtime.main == "cpw.mods.bootstraplauncher.BootstrapLauncher";
            let mut info: super::super::manifest::VersionMetadata =
                serde_json::from_str(include_str!("../../tests/fixtures/1.21.1.json")).unwrap();
            info.id = runtime.inherits.clone();
            info.libraries.push(
                serde_json::from_value(serde_json::json!({
                    "name":"org.ow2.asm:asm:0", "downloads":{"artifact":{
                        "path":"org/ow2/asm/asm/0/asm-0.jar", "size":1,"sha1":"0".repeat(40),
                        "url":"https://libraries.minecraft.net/org/ow2/asm/asm/0/asm-0.jar"
                    }}
                }))
                .unwrap(),
            );
            let root = std::env::temp_dir().join("midnight runtime profile");
            let work = root.join("loader-cache/test");
            let original = root
                .join("versions")
                .join(&info.id)
                .join(format!("{}.jar", info.id));
            let old_asm = root.join("libraries/org/ow2/asm/asm/0/asm-0.jar");
            let vanilla = root.join("libraries/vanilla-only.jar");
            let patched = work.join("libraries/patched.jar");
            let vars = HashMap::from([("PATCHED".into(), patched.to_string_lossy().to_string())]);
            let mut installed = Installed {
                info,
                root: root.clone(),
                classpath: vec![original.clone(), old_asm.clone(), vanilla.clone()],
                assets: root.join("assets"),
                natives: root.join("natives"),
                logging_argument: None,
            };
            merge_runtime(&mut installed, runtime, &work, &vars).unwrap();
            assert_eq!(installed.classpath.contains(&original), !bootstrap);
            assert!(!installed.classpath.contains(&patched));
            assert!(!installed.classpath.contains(&old_asm));
            assert!(installed.classpath.contains(&vanilla));
            assert!(installed
                .classpath
                .iter()
                .any(|p| p.starts_with(work.join("libraries/org/ow2/asm/asm"))));
            assert!(installed
                .info
                .arguments
                .jvm
                .iter()
                .filter_map(Value::as_str)
                .any(|arg| arg.contains(work.join("libraries").to_str().unwrap())));
        }
    }
    #[test]
    fn receipts_require_the_patched_jar_and_recheck_hashes() {
        let root = std::env::temp_dir().join(format!("midnight-receipt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("libraries")).unwrap();
        let patched = root.join("libraries/patched.jar");
        std::fs::write(&patched, b"verified output").unwrap();
        let (size, sha256) = hash_file(&patched).unwrap();
        let receipt = Receipt {
            installer: "official-checksum".into(),
            outputs: vec![Output {
                path: "libraries/patched.jar".into(),
                size,
                sha256,
            }],
        };
        std::fs::write(
            root.join("receipt.json"),
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
        assert!(cache_valid(&root, "official-checksum", &patched));
        assert!(!cache_valid(&root, "different-installer", &patched));
        assert!(!cache_valid(
            &root,
            "official-checksum",
            &root.join("other.jar")
        ));
        std::fs::write(&patched, b"corrupt output").unwrap();
        assert!(!cache_valid(&root, "official-checksum", &patched));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
