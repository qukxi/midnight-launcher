//! Local JAR management. No extraction, execution, or arbitrary filesystem paths.
use serde::Serialize;
use std::{
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

pub const MAX_MOD_SIZE: usize = 512 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
}

pub(crate) fn component(value: &str) -> Result<(), String> {
    let path = super::install::safe_path(value).map_err(|_| "MODS_UNSAFE_PATH")?;
    if path.components().count() != 1
        || value.len() > 240
        || value
            .chars()
            .any(|c| c.is_control() || "<>\"|?*".contains(c))
    {
        return Err("MODS_UNSAFE_PATH".into());
    }
    Ok(())
}

fn enabled_name(name: &str) -> Option<&str> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".jar.disabled") {
        Some(&name[..name.len() - 9])
    } else if lower.ends_with(".jar") {
        Some(name)
    } else {
        None
    }
}

pub(crate) fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub fn directory(data: &Path, instance: &str, create: bool) -> Result<Option<PathBuf>, String> {
    component(instance)?;
    if create {
        fs::create_dir_all(data).map_err(|_| "MODS_WRITE_FAILED")?;
    }
    let mut path = data.to_path_buf();
    for part in ["instances", instance, "mods"] {
        path.push(part);
        if create {
            match fs::create_dir(&path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(_) => return Err("MODS_WRITE_FAILED".into()),
            }
        }
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() && !is_link(&meta) => (),
            Ok(_) => return Err("MODS_UNSAFE_PATH".into()),
            Err(e) if !create && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("MODS_READ_FAILED".into()),
        }
    }
    Ok(Some(path))
}

pub fn list(data: &Path, instance: &str) -> Result<Vec<ModFile>, String> {
    let Some(dir) = directory(data, instance, false)? else {
        return Ok(vec![]);
    };
    let mut mods = Vec::new();
    for entry in fs::read_dir(dir).map_err(|_| "MODS_READ_FAILED")? {
        let entry = entry.map_err(|_| "MODS_READ_FAILED")?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if enabled_name(&name).is_none() || component(&name).is_err() {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path()).map_err(|_| "MODS_READ_FAILED")?;
        if meta.is_file() && !is_link(&meta) {
            mods.push(ModFile {
                enabled: name.to_ascii_lowercase().ends_with(".jar"),
                file_name: name,
                size: meta.len(),
            });
        }
    }
    mods.sort_by_key(|m| m.file_name.to_lowercase());
    Ok(mods)
}

fn conflict(dir: &Path, name: &str, except: Option<&str>) -> Result<bool, String> {
    for entry in fs::read_dir(dir).map_err(|_| "MODS_READ_FAILED")? {
        let entry = entry.map_err(|_| "MODS_READ_FAILED")?;
        let value = entry.file_name();
        let Some(other) = value.to_str() else {
            continue;
        };
        if Some(other) == except {
            continue;
        }
        if enabled_name(other).is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn import(data: &Path, instance: &str, name: &str, bytes: &[u8]) -> Result<(), String> {
    component(name)?;
    if !name.to_ascii_lowercase().ends_with(".jar") {
        return Err("MODS_JAR_REQUIRED".into());
    }
    // Leave enough filename space for the disabled suffix on all supported platforms.
    if name.len() > 220 {
        return Err("MODS_UNSAFE_PATH".into());
    }
    if bytes.len() > MAX_MOD_SIZE {
        return Err("MODS_TOO_LARGE".into());
    }
    let archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "MODS_INVALID_JAR")?;
    if archive.is_empty() {
        return Err("MODS_INVALID_JAR".into());
    }
    let dir = directory(data, instance, true)?.ok_or("MODS_WRITE_FAILED")?;
    if conflict(&dir, name, None)? {
        return Err("MODS_FILE_EXISTS".into());
    }
    let target = dir.join(name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                "MODS_FILE_EXISTS"
            } else {
                "MODS_WRITE_FAILED"
            }
        })?;
    if file.write_all(bytes).and_then(|_| file.sync_all()).is_err() {
        drop(file);
        let _ = fs::remove_file(target);
        return Err("MODS_WRITE_FAILED".into());
    }
    Ok(())
}

pub(crate) fn open_file(data: &Path, instance: &str, name: &str) -> Result<fs::File, String> {
    component(name)?;
    let dir = directory(data, instance, false)?.ok_or("MODS_NOT_FOUND")?;
    let path = dir.join(name);
    let meta = fs::symlink_metadata(&path).map_err(|_| "MODS_READ_FAILED")?;
    if !meta.is_file() || is_link(&meta) || meta.len() > MAX_MOD_SIZE as u64 {
        return Err("MODS_UNSAFE_PATH".into());
    }
    fs::File::open(path).map_err(|_| "MODS_READ_FAILED".into())
}

#[cfg(test)]
pub(crate) fn read_file(data: &Path, instance: &str, name: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    open_file(data, instance, name)?
        .take(MAX_MOD_SIZE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "MODS_READ_FAILED")?;
    if bytes.len() > MAX_MOD_SIZE {
        return Err("MODS_TOO_LARGE".into());
    }
    Ok(bytes)
}

pub(crate) fn file_hash(file: &mut fs::File, job: Option<&super::Job>) -> Result<String, String> {
    use sha1::{Digest, Sha1};
    file.rewind().map_err(|_| "MODS_READ_FAILED")?;
    let mut hash = Sha1::new();
    let mut buffer = vec![0u8; 65536];
    let mut size = 0usize;
    loop {
        if let Some(job) = job {
            job.check()?;
        }
        let count = file.read(&mut buffer).map_err(|_| "MODS_READ_FAILED")?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count)
            .filter(|n| *n <= MAX_MOD_SIZE)
            .ok_or("MODS_TOO_LARGE")?;
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

// Staging lives in the same mods directory, so publishing is atomic and cannot overwrite a file.
pub(crate) fn publish_staged(
    data: &Path,
    instance: &str,
    name: &str,
    source: &Path,
) -> Result<(), String> {
    component(name)?;
    if !name.to_ascii_lowercase().ends_with(".jar") {
        return Err("MODS_JAR_REQUIRED".into());
    }
    if name.len() > 220 {
        return Err("MODS_UNSAFE_PATH".into());
    }
    let meta = fs::symlink_metadata(source).map_err(|_| "MODS_READ_FAILED")?;
    if !meta.is_file() || is_link(&meta) {
        return Err("MODS_UNSAFE_PATH".into());
    }
    if meta.len() > MAX_MOD_SIZE as u64 {
        return Err("MODS_TOO_LARGE".into());
    }
    let file = fs::File::open(source).map_err(|_| "MODS_READ_FAILED")?;
    let archive = zip::ZipArchive::new(file).map_err(|_| "MODS_INVALID_JAR")?;
    if archive.is_empty() {
        return Err("MODS_INVALID_JAR".into());
    }
    let dir = directory(data, instance, true)?.ok_or("MODS_WRITE_FAILED")?;
    if conflict(&dir, name, None)? {
        return Err("MODS_FILE_EXISTS".into());
    }
    fs::hard_link(source, dir.join(name)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            "MODS_FILE_EXISTS".into()
        } else {
            "MODS_WRITE_FAILED".into()
        }
    })
}

// Roll back only an unchanged file created by the current catalogue transaction.
pub(crate) fn rollback(data: &Path, instance: &str, name: &str, hash: &str) {
    if let Ok(mut file) = open_file(data, instance, name) {
        if file_hash(&mut file, None).is_ok_and(|actual| actual == hash) {
            drop(file);
            if let Ok(Some(dir)) = directory(data, instance, false) {
                let _ = fs::remove_file(dir.join(name));
            }
        }
    }
}

pub fn set_enabled(data: &Path, instance: &str, name: &str, enabled: bool) -> Result<(), String> {
    component(name)?;
    let base = enabled_name(name).ok_or("MODS_JAR_REQUIRED")?;
    let dir = directory(data, instance, false)?.ok_or("MODS_NOT_FOUND")?;
    let source = dir.join(name);
    let meta = fs::symlink_metadata(&source).map_err(|_| "MODS_NOT_FOUND")?;
    if !meta.is_file() || is_link(&meta) {
        return Err("MODS_UNSAFE_PATH".into());
    }
    let was_enabled = name.to_ascii_lowercase().ends_with(".jar");
    if was_enabled == enabled {
        return Ok(());
    }
    if conflict(&dir, base, Some(name))? {
        return Err("MODS_FILE_EXISTS".into());
    }
    let target = dir.join(if enabled {
        base.to_owned()
    } else {
        format!("{base}.disabled")
    });
    // hard_link has create-new semantics on Windows and Unix; rename can overwrite on Unix.
    fs::hard_link(&source, &target).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            "MODS_FILE_EXISTS"
        } else {
            "MODS_WRITE_FAILED"
        }
    })?;
    if fs::remove_file(&source).is_err() {
        let _ = fs::remove_file(target);
        return Err("MODS_WRITE_FAILED".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("midnight-mods-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn jar() -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"id":"fixture","version":"1.0"}"#)
            .unwrap();
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn imports_and_toggles_without_losing_bytes_or_crossing_instances() {
        let root = Fixture::new();
        let bytes = jar();
        assert!(list(&root.0, "one").unwrap().is_empty());
        import(&root.0, "one", "Мой мод.jar", &bytes).unwrap();
        set_enabled(&root.0, "one", "Мой мод.jar", false).unwrap();
        let files = list(&root.0, "one").unwrap();
        assert_eq!(files[0].file_name, "Мой мод.jar.disabled");
        assert!(!files[0].enabled);
        assert!(list(&root.0, "two").unwrap().is_empty());
        set_enabled(&root.0, "one", "Мой мод.jar.disabled", true).unwrap();
        assert_eq!(
            fs::read(root.0.join("instances/one/mods/Мой мод.jar")).unwrap(),
            bytes
        );
    }
    #[test]
    fn rejects_traversal_invalid_archives_and_duplicate_disabled_mods() {
        let root = Fixture::new();
        let bytes = jar();
        for bad in ["../outside", "nested/path", "C:\\secret", "NUL", "bad*"] {
            assert!(list(&root.0, bad).is_err());
        }
        assert!(import(&root.0, "one", "../bad.jar", &bytes).is_err());
        assert!(import(&root.0, "one", "bad.exe", &bytes).is_err());
        assert!(import(&root.0, "one", "bad.jar", b"not a zip").is_err());
        import(&root.0, "one", "test.jar", &bytes).unwrap();
        set_enabled(&root.0, "one", "test.jar", false).unwrap();
        assert_eq!(
            import(&root.0, "one", "TEST.JAR", &bytes).unwrap_err(),
            "MODS_FILE_EXISTS"
        );
        let dir = root.0.join("instances/one/mods");
        fs::write(dir.join("test.jar"), b"keep this").unwrap();
        assert_eq!(
            set_enabled(&root.0, "one", "test.jar.disabled", true).unwrap_err(),
            "MODS_FILE_EXISTS"
        );
        assert_eq!(fs::read(dir.join("test.jar")).unwrap(), b"keep this");
    }
    #[test]
    fn refuses_redirected_directories_and_ignores_unrelated_files() {
        let root = Fixture::new();
        let dir = directory(&root.0, "one", true).unwrap().unwrap();
        fs::write(dir.join("notes.txt"), b"keep").unwrap();
        fs::create_dir(dir.join("folder.jar")).unwrap();
        assert!(list(&root.0, "one").unwrap().is_empty());
        fs::write(root.0.join("instances/blocked"), b"not a directory").unwrap();
        assert_eq!(list(&root.0, "blocked").unwrap_err(), "MODS_UNSAFE_PATH");
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_to_external_files_and_directories() {
        use std::os::unix::fs::symlink;
        let root = Fixture::new();
        let outside = Fixture::new();
        let dir = directory(&root.0, "one", true).unwrap().unwrap();
        fs::write(outside.0.join("keep.jar"), jar()).unwrap();
        symlink(outside.0.join("keep.jar"), dir.join("link.jar")).unwrap();
        assert!(list(&root.0, "one").unwrap().is_empty());
        assert!(set_enabled(&root.0, "one", "link.jar", false).is_err());
        symlink(&outside.0, root.0.join("instances/escape")).unwrap();
        assert!(import(&root.0, "escape", "test.jar", &jar()).is_err());
    }
}
