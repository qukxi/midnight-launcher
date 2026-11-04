//! ZIP extraction for verified Java packages and native libraries. Never follow links.
use super::{install::safe_path, Job};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub fn extract_zip(
    archive: &Path,
    destination: &Path,
    natives_only: bool,
    job: &Job,
) -> Result<usize, String> {
    let file = fs::File::open(archive).map_err(|_| "INSTALL_ARCHIVE_READ_FAILED")?;
    extract(
        zip::ZipArchive::new(file).map_err(|_| "INSTALL_ARCHIVE_INVALID")?,
        destination,
        natives_only,
        job,
    )
}
fn extract<R: Read + std::io::Seek>(
    mut archive: zip::ZipArchive<R>,
    destination: &Path,
    natives_only: bool,
    job: &Job,
) -> Result<usize, String> {
    if archive.len() > 20000 {
        return Err("INSTALL_ARCHIVE_TOO_LARGE".into());
    }
    let mut total = 0u64;
    let mut count = 0;
    for i in 0..archive.len() {
        job.check()?;
        let mut entry = archive.by_index(i).map_err(|_| "INSTALL_ARCHIVE_INVALID")?;
        let path = safe_path(
            entry
                .name()
                .map_err(|_| "INSTALL_ARCHIVE_INVALID")?
                .trim_end_matches('/'),
        )?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("INSTALL_ARCHIVE_UNSAFE_LINK".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("INSTALL_ARCHIVE_TOO_LARGE")?;
        if total > 1024 * 1024 * 1024 || entry.size() > 512 * 1024 * 1024 {
            return Err("INSTALL_ARCHIVE_TOO_LARGE".into());
        }
        if entry.is_dir() {
            continue;
        }
        let out = if natives_only {
            if path.components().any(|part| part.as_os_str() == "META-INF") {
                continue;
            }
            if !matches!(
                path.extension().and_then(|x| x.to_str()),
                Some("dll" | "dylib" | "jnilib" | "so")
            ) {
                continue;
            }
            destination.join(path.file_name().ok_or("INSTALL_UNSAFE_PATH")?)
        } else {
            destination.join(&path)
        };
        fs::create_dir_all(out.parent().ok_or("INSTALL_UNSAFE_PATH")?)
            .map_err(|_| "CACHE_CREATE_FAILED")?;
        // The destination is a fresh staging folder. create_new rejects duplicate names
        // and prevents a second entry from replacing a file created by the first one.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out)
            .map_err(|_| "INSTALL_ARCHIVE_DUPLICATE_ENTRY")?;
        let mut copied = 0u64;
        let mut buf = [0u8; 65536];
        loop {
            job.check()?;
            let n = entry
                .read(&mut buf)
                .map_err(|_| "INSTALL_ARCHIVE_INVALID")?;
            if n == 0 {
                break;
            }
            copied += n as u64;
            if copied > entry.size() {
                return Err("INSTALL_ARCHIVE_TOO_LARGE".into());
            }
            file.write_all(&buf[..n])
                .map_err(|_| "CACHE_WRITE_FAILED")?;
        }
        if copied != entry.size() {
            return Err("INSTALL_ARCHIVE_INVALID".into());
        }
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn job() -> Job {
        Job {
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app: None,
        }
    }
    fn zip(name: &str, bytes: &[u8]) -> zip::ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
        zip::ZipArchive::new(writer.finish().unwrap()).unwrap()
    }
    #[test]
    fn zip_paths_and_cancellation_are_checked_before_writing() {
        let root = std::env::temp_dir().join(format!("midnight-zip-{}", uuid::Uuid::new_v4()));
        for name in [
            "../outside.dll",
            "C:/outside.dll",
            "a\\outside.dll",
            "CON.dll",
        ] {
            assert!(extract(zip(name, b"bad"), &root, true, &job()).is_err());
        }
        let cancelled = job();
        cancelled
            .cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(extract(zip("good.dll", b"test"), &root, true, &cancelled).is_err());
        assert!(!root.exists());
        assert_eq!(
            extract(zip("windows/x64/glfw.dll", b"test"), &root, true, &job()).unwrap(),
            1
        );
        assert_eq!(fs::read(root.join("glfw.dll")).unwrap(), b"test");
        fs::remove_dir_all(root).unwrap();
    }
}
