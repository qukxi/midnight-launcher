use super::{download::client, install::Job};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    path::{Component, Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
#[derive(Deserialize)]
struct Release {
    binary: Binary,
}
#[derive(Deserialize)]
struct Binary {
    package: Package,
}
#[derive(Deserialize)]
struct Package {
    link: String,
    checksum: String,
    size: u64,
}

pub async fn verify(path: &Path, major: u32) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err("JAVA_EXECUTABLE_NOT_FOUND".into());
    }
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        Command::new(path)
            .args(["-XshowSettings:properties", "-version"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "JAVA_CHECK_TIMEOUT")?
    .map_err(|_| "JAVA_CHECK_FAILED")?;
    let output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&result.stderr),
        String::from_utf8_lossy(&result.stdout)
    );
    if !result.status.success() {
        return Err("JAVA_CHECK_FAILED".into());
    }
    if !output
        .lines()
        .any(|line| line.trim() == format!("java.specification.version = {major}"))
    {
        return Err(format!("JAVA_MAJOR_REQUIRED_{major}"));
    }
    if !output
        .lines()
        .any(|line| matches!(line.trim(), "os.arch = aarch64" | "os.arch = arm64"))
    {
        return Err("JAVA_ARM64_REQUIRED".into());
    }
    Ok(())
}
fn java_in(path: &Path, depth: u8) -> Option<PathBuf> {
    if depth > 4 {
        return None;
    }
    let java = path.join("Contents/Home/bin/java");
    if java.is_file() {
        return Some(java);
    }
    let java = path.join("bin/java");
    if java.is_file() {
        return Some(java);
    }
    for entry in std::fs::read_dir(path).ok()?.flatten() {
        if entry.file_type().ok()?.is_dir() {
            if let Some(java) = java_in(&entry.path(), depth + 1) {
                return Some(java);
            }
        }
    }
    None
}
fn relative_inside(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|p| {
            !matches!(
                p,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}
fn safe_link(path: &Path, link: &Path, hard: bool) -> bool {
    if link.is_absolute() {
        return false;
    }
    let joined = if hard {
        link.to_path_buf()
    } else {
        path.parent().unwrap_or(Path::new("")).join(link)
    };
    let mut depth = 0i32;
    for component in joined.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}
fn unpack(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|_| "JAVA_ARCHIVE_READ_FAILED")?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut total = 0u64;
    for (index, item) in tar
        .entries()
        .map_err(|_| "JAVA_ARCHIVE_INVALID")?
        .enumerate()
    {
        if index > 20000 {
            return Err("JAVA_ARCHIVE_TOO_LARGE".into());
        }
        let mut entry = item.map_err(|_| "JAVA_ARCHIVE_INVALID")?;
        total = total.saturating_add(entry.size());
        if total > 1024 * 1024 * 1024 {
            return Err("JAVA_ARCHIVE_TOO_LARGE".into());
        }
        let path = entry
            .path()
            .map_err(|_| "JAVA_ARCHIVE_UNSAFE_PATH")?
            .into_owned();
        if !relative_inside(&path) {
            return Err("JAVA_ARCHIVE_UNSAFE_PATH".into());
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
            return Err("JAVA_ARCHIVE_UNSUPPORTED_ENTRY".into());
        }
        if kind.is_symlink() || kind.is_hard_link() {
            let link = entry
                .link_name()
                .map_err(|_| "JAVA_ARCHIVE_UNSAFE_LINK")?
                .ok_or("JAVA_ARCHIVE_UNSAFE_LINK")?;
            if !safe_link(&path, &link, kind.is_hard_link()) {
                return Err("JAVA_ARCHIVE_UNSAFE_LINK".into());
            }
        }
        if !entry
            .unpack_in(destination)
            .map_err(|_| "JAVA_EXTRACT_FAILED")?
        {
            return Err("JAVA_ARCHIVE_UNSAFE_PATH".into());
        }
    }
    Ok(())
}
pub async fn ensure(root: &Path, custom: &str, major: u32, job: &Job) -> Result<PathBuf, String> {
    job.check()?;
    job.report("java", 0, 0);
    if !custom.trim().is_empty() {
        let path = PathBuf::from(custom.trim());
        verify(&path, major).await?;
        return Ok(path);
    }
    let home = Command::new("/usr/libexec/java_home")
        .args(["-v", &major.to_string()])
        .kill_on_drop(true)
        .output()
        .await;
    if let Ok(output) = home {
        if output.status.success() {
            let path =
                PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join("bin/java");
            if verify(&path, major).await.is_ok() {
                return Ok(path);
            }
        }
    }
    let destination = root.join(format!("temurin-{major}-aarch64"));
    if let Some(java) = java_in(&destination, 0) {
        if verify(&java, major).await.is_ok() {
            return Ok(java);
        }
    }
    if !matches!(major, 17 | 21 | 25) {
        return Err("JAVA_UNSUPPORTED_VERSION".into());
    }
    let endpoint=format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture=aarch64&heap_size=normal&image_type=jre&jvm_impl=hotspot&os=mac&vendor=eclipse");
    let reply = client()?
        .get(endpoint)
        .send()
        .await
        .map_err(|_| "JAVA_CATALOG_NETWORK")?
        .error_for_status()
        .map_err(|_| "JAVA_CATALOG_HTTP")?;
    if reply.content_length().is_some_and(|n| n > 1024 * 1024) {
        return Err("JAVA_CATALOG_INVALID".into());
    }
    let data = reply.bytes().await.map_err(|_| "JAVA_CATALOG_NETWORK")?;
    if data.len() > 1024 * 1024 {
        return Err("JAVA_CATALOG_INVALID".into());
    }
    let releases: Vec<Release> =
        serde_json::from_slice(&data).map_err(|_| "JAVA_CATALOG_INVALID")?;
    let package = &releases
        .first()
        .ok_or("JAVA_RELEASE_NOT_FOUND")?
        .binary
        .package;
    let url = reqwest::Url::parse(&package.link).map_err(|_| "JAVA_UNTRUSTED_DOWNLOAD")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.path().starts_with(&format!(
            "/adoptium/temurin{major}-binaries/releases/download/"
        ))
        || package.checksum.len() != 64
        || !package.checksum.bytes().all(|c| c.is_ascii_hexdigit())
        || package.size > 300 * 1024 * 1024
        || package.size == 0
    {
        return Err("JAVA_UNTRUSTED_DOWNLOAD".into());
    }
    let http = reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 {
                return attempt.error("Too many redirects");
            }
            if attempt.url().scheme() == "https"
                && matches!(
                    attempt.url().host_str(),
                    Some(
                        "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                )
            {
                attempt.follow()
            } else {
                attempt.error("Untrusted Java redirect")
            }
        }))
        .build()
        .map_err(|_| "JAVA_NETWORK_CLIENT")?;
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|_| "JAVA_CREATE_DIRECTORY")?;
    let archive = root.join(format!("temurin-{major}.tar.gz.part"));
    let mut reply = http
        .get(url)
        .send()
        .await
        .map_err(|_| "JAVA_DOWNLOAD_NETWORK")?
        .error_for_status()
        .map_err(|_| "JAVA_DOWNLOAD_HTTP")?;
    let mut file = tokio::fs::File::create(&archive)
        .await
        .map_err(|_| "JAVA_DOWNLOAD_WRITE")?;
    let mut checksum = Sha256::new();
    let mut length = 0;
    while let Some(chunk) = reply.chunk().await.map_err(|_| "JAVA_DOWNLOAD_NETWORK")? {
        job.check()?;
        length += chunk.len() as u64;
        if length > package.size {
            return Err("JAVA_DOWNLOAD_SIZE".into());
        };
        checksum.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|_| "JAVA_DOWNLOAD_WRITE")?;
        job.report("java", length as usize, package.size as usize);
    }
    file.flush().await.map_err(|_| "JAVA_DOWNLOAD_WRITE")?;
    drop(file);
    if length != package.size
        || format!("{:x}", checksum.finalize()) != package.checksum.to_ascii_lowercase()
    {
        let _ = tokio::fs::remove_file(&archive).await;
        return Err("JAVA_CHECKSUM_MISMATCH".into());
    }
    job.check()?;
    let staging = root.join(format!("unpack-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|_| "JAVA_CREATE_DIRECTORY")?;
    let input = archive.clone();
    let output = staging.clone();
    let result = tokio::task::spawn_blocking(move || unpack(&input, &output))
        .await
        .map_err(|_| "JAVA_EXTRACT_FAILED")?;
    if let Err(error) = result {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    job.check()?;
    let java = java_in(&staging, 0).ok_or("JAVA_EXECUTABLE_NOT_FOUND")?;
    verify(&java, major).await?;
    if destination.exists() {
        tokio::fs::remove_dir_all(&destination)
            .await
            .map_err(|_| "JAVA_REPLACE_FAILED")?;
    }
    tokio::fs::rename(&staging, &destination)
        .await
        .map_err(|_| "JAVA_REPLACE_FAILED")?;
    let _ = tokio::fs::remove_file(&archive).await;
    java_in(&destination, 0).ok_or_else(|| "JAVA_EXECUTABLE_NOT_FOUND".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_links_cannot_escape() {
        assert!(!relative_inside(Path::new("../outside")));
        assert!(!safe_link(
            Path::new("jdk/lib/a"),
            Path::new("../../../outside"),
            false
        ));
        assert!(safe_link(
            Path::new("jdk/Contents/MacOS/libjli.dylib"),
            Path::new("../Home/lib/libjli.dylib"),
            false
        ));
        assert!(!safe_link(Path::new("a"), Path::new("/etc/passwd"), false));
    }
}
