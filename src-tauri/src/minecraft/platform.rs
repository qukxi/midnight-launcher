//! Platform decisions shared by metadata rules, Java and JVM launch arguments.
use std::sync::OnceLock;
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd
}

#[cfg(windows)]
fn windows_version() -> Result<String, String> {
    use windows_sys::{
        Wdk::System::SystemServices::RtlGetVersion,
        Win32::System::SystemInformation::OSVERSIONINFOW,
    };
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        dwMajorVersion: 0,
        dwMinorVersion: 0,
        dwBuildNumber: 0,
        dwPlatformId: 0,
        szCSDVersion: [0; 128],
    };
    // SAFETY: info is a valid writable OSVERSIONINFOW with the required size field.
    if unsafe { RtlGetVersion(&mut info) } != 0 {
        return Err("INSTALL_OS_VERSION_FAILED".into());
    }
    Ok(format!(
        "{}.{}.{}",
        info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
    ))
}
#[cfg(not(windows))]
fn windows_version() -> Result<String, String> {
    Err("INSTALL_UNSUPPORTED_PLATFORM".into())
}

#[derive(Clone, Debug)]
pub struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
    pub version: String,
}
impl Platform {
    pub fn windows_x64(version: String) -> Self {
        Self {
            os: "windows",
            arch: "x86_64",
            version,
        }
    }
    pub fn mac_arm64() -> Self {
        Self {
            os: "osx",
            arch: "aarch64",
            version: String::new(),
        }
    }
    pub fn current() -> Result<&'static Self, String> {
        static CURRENT: OnceLock<Result<Platform, String>> = OnceLock::new();
        CURRENT
            .get_or_init(|| match (std::env::consts::OS, std::env::consts::ARCH) {
                ("windows", "x86_64") => Ok(Self::windows_x64(windows_version()?)),
                ("macos", "aarch64") => Ok(Self::mac_arm64()),
                _ => Err("INSTALL_UNSUPPORTED_PLATFORM".into()),
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    pub fn key(&self) -> &'static str {
        if self.os == "windows" {
            "windows-x64"
        } else {
            "macos-arm64"
        }
    }
    pub fn adoptium_os(&self) -> &'static str {
        if self.os == "windows" {
            "windows"
        } else {
            "mac"
        }
    }
    pub fn adoptium_arch(&self) -> &'static str {
        if self.os == "windows" {
            "x64"
        } else {
            "aarch64"
        }
    }
    pub fn native_classifier(&self) -> &'static str {
        if self.os == "windows" {
            "natives-windows"
        } else {
            "natives-macos-arm64"
        }
    }
    pub fn java_arch_matches(&self, arch: &str) -> bool {
        match self.arch {
            "x86_64" => matches!(arch, "amd64" | "x86_64" | "x64"),
            "aarch64" => matches!(arch, "aarch64" | "arm64"),
            _ => false,
        }
    }
    pub fn classpath_separator(&self) -> &'static str {
        if self.os == "windows" {
            ";"
        } else {
            ":"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_never_uses_mac_or_32_bit_java() {
        let windows = Platform::windows_x64("10.0.26200".into());
        assert!(windows.java_arch_matches("amd64"));
        assert!(!windows.java_arch_matches("x86"));
        assert!(!windows.java_arch_matches("aarch64"));
        assert_eq!(windows.native_classifier(), "natives-windows");
        assert_eq!(windows.classpath_separator(), ";");
        assert_eq!(Platform::mac_arm64().classpath_separator(), ":");
    }
}
