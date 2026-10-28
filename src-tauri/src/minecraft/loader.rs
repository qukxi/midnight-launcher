use super::{
    download,
    install::{self, Installed, Job},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Deserialize, Serialize)]
pub struct Loader {
    pub kind: String,
    pub version: String,
}
#[derive(Deserialize)]
struct Entry {
    loader: Release,
}
#[derive(Deserialize)]
struct Release {
    version: String,
    stable: bool,
}
#[derive(Deserialize)]
struct Profile {
    #[serde(rename = "inheritsFrom")]
    inherits: String,
    #[serde(rename = "mainClass")]
    main: String,
    arguments: super::manifest::Arguments,
    libraries: Vec<Library>,
}
#[derive(Deserialize)]
struct Library {
    name: String,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}
fn valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-+".contains(&c))
}
pub fn validate(loader: &Option<Loader>) -> Result<(), String> {
    if let Some(l) = loader {
        if l.kind != "fabric" || !valid(&l.version) {
            return Err("LOADER_UNSUPPORTED".into());
        }
    }
    Ok(())
}
async fn get(http: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>, String> {
    let mut r = http
        .get(url)
        .send()
        .await
        .map_err(|_| "LOADER_NETWORK")?
        .error_for_status()
        .map_err(|_| "LOADER_HTTP_ERROR")?;
    if !r.status().is_success() {
        return Err("LOADER_HTTP_ERROR".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = r.chunk().await.map_err(|_| "LOADER_NETWORK")? {
        if bytes.len() + chunk.len() > limit {
            return Err("LOADER_RESPONSE_TOO_LARGE".into());
        }
        bytes.extend(chunk);
    }
    Ok(bytes)
}
pub async fn versions(game: &str) -> Result<Vec<String>, String> {
    download::validate_version(game)?;
    let raw = get(
        &download::client()?,
        &format!("https://meta.fabricmc.net/v2/versions/loader/{game}"),
        4 * 1024 * 1024,
    )
    .await?;
    let entries: Vec<Entry> =
        serde_json::from_slice(&raw).map_err(|_| "LOADER_INVALID_METADATA")?;
    Ok(entries
        .into_iter()
        .filter(|e| e.loader.stable && valid(&e.loader.version))
        .map(|e| e.loader.version)
        .collect())
}
fn library_path(name: &str) -> Result<String, String> {
    let parts: Vec<_> = name.split(':').collect();
    if parts.len() != 3 || !parts.iter().all(|p| valid(p)) {
        return Err("LOADER_INVALID_LIBRARY".into());
    }
    Ok(format!(
        "{}/{}/{}/{}-{}.jar",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2]
    ))
}
pub async fn apply(
    installed: &mut Installed,
    loader: &Option<Loader>,
    job: &Job,
) -> Result<(), String> {
    validate(loader)?;
    let Some(l) = loader else {
        return Ok(());
    };
    let http = download::client()?;
    let raw = get(
        &http,
        &format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
            installed.info.id, l.version
        ),
        4 * 1024 * 1024,
    )
    .await?;
    let p: Profile = serde_json::from_slice(&raw).map_err(|_| "LOADER_INVALID_METADATA")?;
    if p.inherits != installed.info.id
        || !p.main.starts_with("net.fabricmc.")
        || p.libraries.len() > 100
    {
        return Err("LOADER_INVALID_METADATA".into());
    }
    job.report("loader", 0, p.libraries.len());
    let mut paths = Vec::new();
    for (i, lib) in p.libraries.iter().enumerate() {
        job.check()?;
        if lib.url != "https://maven.fabricmc.net/" {
            return Err("LOADER_UNTRUSTED_HOST".into());
        }
        let relative = library_path(&lib.name)?;
        let url = format!("{}{relative}", lib.url);
        let hash = if let Some(ref sha) = lib.sha1 {
            sha.clone()
        } else {
            String::from_utf8(get(&http, &format!("{url}.sha1"), 256).await?)
                .map_err(|_| "LOADER_INVALID_HASH")?
                .trim()
                .to_string()
        };
        let size = if let Some(size) = lib.size {
            size
        } else {
            let r = http.head(&url).send().await.map_err(|_| "LOADER_NETWORK")?;
            if !r.status().is_success() {
                return Err("LOADER_HTTP_ERROR".into());
            }
            r.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or("LOADER_INVALID_SIZE")?
        };
        let path = installed
            .root
            .join("libraries")
            .join(install::safe_path(&relative)?);
        install::artifact(&http, &url, size, &hash, &path, job)
            .await
            .map_err(|e| format!("{e}: {} ({size} bytes)", lib.name))?;
        paths.push(path);
        job.report("loader", i + 1, p.libraries.len());
    }
    // Loader libraries take precedence over Vanilla copies of ASM and Mixin.
    paths.extend(std::mem::take(&mut installed.classpath));
    installed.classpath = paths;
    installed.info.main_class = p.main;
    installed.info.arguments.jvm.extend(p.arguments.jvm);
    installed.info.arguments.game.extend(p.arguments.game);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsupported_loaders_and_paths() {
        assert!(validate(&Some(Loader {
            kind: "forge".into(),
            version: "1".into()
        }))
        .is_err());
        assert!(library_path("net.fabricmc:loader:../../bad").is_err());
        assert_eq!(
            library_path("net.fabricmc:fabric-loader:0.16.14").unwrap(),
            "net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar"
        );
    }
    #[test]
    fn official_profile_has_expected_entrypoint() {
        let p: Profile =
            serde_json::from_str(include_str!("../../tests/fixtures/fabric-1.21.1.json")).unwrap();
        assert_eq!(p.inherits, "1.21.1");
        assert_eq!(p.main, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        for lib in p.libraries {
            assert!(library_path(&lib.name).is_ok());
        }
    }
}
