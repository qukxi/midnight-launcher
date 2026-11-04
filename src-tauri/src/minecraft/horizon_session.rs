//! Private, single-use handoff required by the pinned Horizon release.
//! Tokens never enter IPC, argv, environment variables, or diagnostic messages.
use super::{mods, Job};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const ERROR: &str = "HORIZON_SESSION_FAILED";

fn directory(path: &Path) -> Result<(), String> {
    match fs::create_dir(path) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err(ERROR.into()),
    }
    let meta = fs::symlink_metadata(path).map_err(|_| ERROR)?;
    if !meta.is_dir() || mods::is_link(&meta) {
        return Err(ERROR.into());
    }
    Ok(())
}
pub(super) fn home(data: &Path, instance: &str) -> Result<PathBuf, String> {
    mods::component(instance).map_err(|_| "LAUNCH_INVALID_INSTANCE")?;
    if !data.is_absolute() {
        return Err(ERROR.into());
    }
    fs::create_dir_all(data).map_err(|_| ERROR)?;
    let mut path = data.to_path_buf();
    directory(&path)?;
    for part in ["instances", instance, "horizon"] {
        path.push(part);
        directory(&path)?;
    }
    Ok(path)
}
fn encode(session: &crate::auth::Session) -> Result<Vec<u8>, String> {
    if session.token.is_empty()
        || session.token == "0"
        || session.token.len() > 65536
        || session.profile.name.is_empty()
        || session.profile.name.len() > 16
        || !session
            .profile
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(ERROR.into());
    }
    let uuid = uuid::Uuid::parse_str(&session.profile.id).map_err(|_| ERROR)?;
    if uuid.is_nil() {
        return Err(ERROR.into());
    }
    serde_json::to_vec(&serde_json::json!({
        "name": session.profile.name, "uuid": uuid.simple().to_string(),
        "access_token": session.token, "user_type": "msa", "xuid": session.xuid,
    }))
    .map_err(|_| ERROR.into())
}

pub(super) struct SessionFile {
    path: PathBuf,
}
impl SessionFile {
    pub fn create(home: &Path, session: &crate::auth::Session) -> Result<Self, String> {
        let bytes = encode(session)?;
        directory(home)?;
        let sessions = home.join("sessions");
        directory(&sessions)?;
        // Remove only expired handoffs created by Midnight, never game/user files.
        for entry in fs::read_dir(&sessions).map_err(|_| ERROR)? {
            let entry = entry.map_err(|_| ERROR)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(id) = name
                .strip_prefix("midnight-")
                .and_then(|s| s.strip_suffix(".json"))
            else {
                continue;
            };
            if uuid::Uuid::parse_str(id).is_err() {
                continue;
            }
            let meta = fs::symlink_metadata(entry.path()).map_err(|_| ERROR)?;
            if meta.is_file()
                && !mods::is_link(&meta)
                && meta
                    .modified()
                    .ok()
                    .and_then(|time| time.elapsed().ok())
                    .is_some_and(|age| age > Duration::from_secs(120))
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        let path = sessions.join(format!("midnight-{}.json", uuid::Uuid::new_v4()));
        let mut file = private_file(&path).map_err(|_| ERROR)?;
        let handoff = Self { path };
        let result = file.write_all(&bytes).and_then(|_| file.sync_all());
        // Close before rollback: Windows does not unlink an exclusively open file.
        drop(file);
        result.map_err(|_| ERROR)?;
        Ok(handoff)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for SessionFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) async fn wait_consumed(
    file: &SessionFile,
    child: &mut tokio::process::Child,
    job: &Job,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        job.check()?;
        if child.try_wait().map_err(|_| ERROR)?.is_some() {
            return Err("HORIZON_START_FAILED".into());
        }
        match fs::symlink_metadata(file.path()) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Ok(meta) if meta.is_file() && !mods::is_link(&meta) => (),
            _ => return Err(ERROR.into()),
        }
        if Instant::now() >= deadline {
            return Err(ERROR.into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(unix)]
fn private_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
#[cfg(windows)]
fn private_file(path: &Path) -> std::io::Result<fs::File> {
    windows::private_file(path)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::c_void,
        os::windows::{
            ffi::OsStrExt,
            io::{FromRawHandle, OwnedHandle},
        },
        ptr::null_mut,
    };
    use windows_sys::Win32::{
        Foundation::{LocalFree, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SDDL_REVISION_1,
            },
            GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    struct Allocation(*mut c_void);
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn sid_string(sid: *mut c_void) -> std::io::Result<String> {
        let mut text = null_mut();
        // Windows allocates a terminated SID string; the guard frees it on every path.
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let _memory = Allocation(text.cast());
        let mut length = 0;
        while length < 256 && unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        if length == 256 {
            return Err(std::io::Error::other(ERROR));
        }
        Ok(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(text, length)
        }))
    }
    fn user_sid() -> std::io::Result<String> {
        let mut token = null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        // The API returned a new real token handle, owned here until the SID is copied.
        let _token = unsafe { OwnedHandle::from_raw_handle(token) };
        let mut needed = 0;
        unsafe {
            GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        }
        if needed == 0 || needed > 65536 {
            return Err(std::io::Error::other(ERROR));
        }
        // usize storage provides TOKEN_USER's pointer alignment.
        let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        sid_string(user.User.Sid)
    }
    pub(super) fn private_file(path: &Path) -> std::io::Result<fs::File> {
        // A protected DACL grants file access only to the current Windows user.
        // Parent-directory ACLs are not inherited, even on a shared/custom data folder.
        let sddl: Vec<u16> = format!("D:P(A;;FA;;;{})", user_sid()?)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let _descriptor = Allocation(descriptor);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
        if name.contains(&0) {
            return Err(std::io::Error::other(ERROR));
        }
        name.push(0);
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                0,
                &attributes,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        // CREATE_NEW returned a unique owned file handle; std::fs::File closes it.
        Ok(unsafe { fs::File::from_raw_handle(handle) })
    }
    #[cfg(test)]
    pub(super) fn assert_private(path: &Path) {
        use windows_sys::Win32::Security::{
            Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
            GetAce, GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION,
            SE_DACL_PROTECTED,
        };
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut descriptor = null_mut();
        let mut acl = null_mut();
        assert_eq!(
            unsafe {
                GetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    &mut acl,
                    null_mut(),
                    &mut descriptor,
                )
            },
            0
        );
        let _descriptor = Allocation(descriptor);
        assert!(!acl.is_null());
        assert_eq!(unsafe { (*acl).AceCount }, 1);
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
            0
        );
        assert_ne!(control & SE_DACL_PROTECTED, 0);
        let mut ace = null_mut();
        assert_ne!(unsafe { GetAce(acl, 0, &mut ace) }, 0);
        let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
        assert_eq!(allowed.Header.AceType, 0);
        let sid = std::ptr::addr_of!(allowed.SidStart)
            .cast_mut()
            .cast::<c_void>();
        assert_eq!(sid_string(sid).unwrap(), user_sid().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, Arc};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("midnight horizon session {}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn session() -> crate::auth::Session {
        let mut session = crate::auth::offline_session("MidnightCheck").unwrap();
        session.token = "mock-private-token".into();
        session.xuid = "123456789".into();
        session
    }
    fn job(cancelled: bool) -> Job {
        Job {
            app: None,
            cancel: Arc::new(AtomicBool::new(cancelled)),
        }
    }
    #[test]
    fn private_handoff_has_the_upstream_schema_and_is_removed_on_drop() {
        let root = Fixture::new();
        let home = home(&root.0, "rust").unwrap();
        let path;
        {
            let file = SessionFile::create(&home, &session()).unwrap();
            path = file.path().to_path_buf();
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(value["name"], "MidnightCheck");
            assert_eq!(value["user_type"], "msa");
            assert_eq!(value["access_token"], "mock-private-token");
            assert_eq!(value["xuid"], "123456789");
            assert_eq!(value["uuid"].as_str().unwrap().len(), 32);
            assert!(!path.to_string_lossy().contains("mock-private-token"));
            #[cfg(windows)]
            windows::assert_private(&path);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
        assert!(!path.exists());
    }
    #[test]
    fn handoff_rejects_invalid_identities_without_exposing_payloads() {
        let root = Fixture::new();
        let mut invalid = session();
        invalid.profile.id = "bad-uuid".into();
        assert_eq!(SessionFile::create(&root.0, &invalid).err().unwrap(), ERROR);
        invalid = session();
        invalid.profile.name = "bad name".into();
        assert!(encode(&invalid).is_err());
        invalid = session();
        invalid.token = "0".into();
        assert!(encode(&invalid).is_err());
        assert!(fs::read_dir(&root.0).unwrap().next().is_none());
        for id in ["../outside", "/absolute", "C:/outside", "CON", "one/two"] {
            assert!(home(&root.0, id).is_err());
        }
    }
    #[test]
    fn only_expired_generated_files_are_cleaned_and_existing_files_are_never_overwritten() {
        let root = Fixture::new();
        let home = home(&root.0, "rust").unwrap();
        let old = SessionFile::create(&home, &session()).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(old.path())
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(300))
            .unwrap();
        let unrelated = home.join("sessions/keep.json");
        fs::write(&unrelated, b"user file").unwrap();
        let fresh = SessionFile::create(&home, &session()).unwrap();
        assert!(!old.path().exists());
        assert_eq!(fs::read(&unrelated).unwrap(), b"user file");
        assert!(private_file(fresh.path()).is_err());
        assert!(fs::read_to_string(fresh.path())
            .unwrap()
            .contains("mock-private-token"));
        assert!(fs::read(&unrelated).is_ok());
    }
    #[test]
    fn file_in_place_of_session_directory_is_preserved() {
        let root = Fixture::new();
        let home = home(&root.0, "rust").unwrap();
        fs::write(home.join("sessions"), b"keep").unwrap();
        assert!(SessionFile::create(&home, &session()).is_err());
        assert_eq!(fs::read(home.join("sessions")).unwrap(), b"keep");
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_session_directory_is_rejected() {
        let root = Fixture::new();
        let home = home(&root.0, "rust").unwrap();
        let other = root.0.join("other");
        fs::create_dir(&other).unwrap();
        std::os::unix::fs::symlink(&other, home.join("sessions")).unwrap();
        assert!(SessionFile::create(&home, &session()).is_err());
        assert_eq!(fs::read_dir(other).unwrap().count(), 0);
    }
    #[test]
    fn mock_session_consumer() {
        let Some(path) = std::env::var_os("MIDNIGHT_TEST_HANDOFF") else {
            return;
        };
        let path = PathBuf::from(path);
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["access_token"], "mock-private-token");
        fs::remove_file(&path).unwrap();
        std::thread::sleep(Duration::from_millis(300));
    }
    #[tokio::test]
    async fn handoff_is_consumed_by_a_child_and_cancellation_is_respected() {
        let root = Fixture::new();
        let home = home(&root.0, "rust").unwrap();
        let file = SessionFile::create(&home, &session()).unwrap();
        let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
            .arg("mock_session_consumer")
            .env("MIDNIGHT_TEST_HANDOFF", file.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        wait_consumed(&file, &mut child, &job(false)).await.unwrap();
        assert!(!file.path().exists());
        assert!(child.wait().await.unwrap().success());
        let file = SessionFile::create(&home, &session()).unwrap();
        let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        assert_eq!(
            wait_consumed(&file, &mut child, &job(true))
                .await
                .unwrap_err(),
            "OPERATION_CANCELLED"
        );
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    #[tokio::test]
    async fn native_output_redacts_the_handed_off_access_token() {
        use tokio::io::AsyncWriteExt;
        let root = Fixture::new();
        let log = root.0.join("output.log");
        let (reader, mut writer) = tokio::io::duplex(128);
        writer
            .write_all(b"session mock-private-token in output\n")
            .await
            .unwrap();
        drop(writer);
        super::super::launch::log_stream(reader, log.clone(), "mock-private-token".into()).await;
        let text = fs::read_to_string(log).unwrap();
        assert!(text.contains("[REDACTED]"));
        assert!(!text.contains("mock-private-token"));
    }
}
