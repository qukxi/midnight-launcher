use super::install::{allowed, safe_path, Installed};
use crate::auth::Session;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
};
#[derive(Default)]
pub struct GameState(pub tokio::sync::Mutex<Option<Child>>);
#[derive(Serialize)]
pub struct GameStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
}
pub fn status(child: &mut Option<Child>) -> Result<GameStatus, String> {
    if let Some(game) = child.as_mut() {
        let pid = game.id();
        if let Some(exit) = game.try_wait().map_err(|_| "GAME_STATUS_FAILED")? {
            *child = None;
            return Ok(GameStatus {
                running: false,
                pid: None,
                exit_code: exit.code(),
            });
        }
        return Ok(GameStatus {
            running: true,
            pid,
            exit_code: None,
        });
    }
    Ok(GameStatus {
        running: false,
        pid: None,
        exit_code: None,
    })
}
pub fn expand(
    values: &[serde_json::Value],
    variables: &HashMap<&str, String>,
) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for value in values {
        let items = if let Some(text) = value.as_str() {
            vec![text.to_string()]
        } else {
            let rules = value["rules"]
                .as_array()
                .ok_or("LAUNCH_INVALID_ARGUMENT_RULE")?;
            if !allowed(rules)? {
                continue;
            }
            if let Some(text) = value["value"].as_str() {
                vec![text.to_string()]
            } else {
                value["value"]
                    .as_array()
                    .ok_or("LAUNCH_INVALID_ARGUMENT")?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_string)
                            .ok_or("LAUNCH_INVALID_ARGUMENT")
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        for mut text in items {
            for (key, value) in variables {
                text = text.replace(&format!("${{{key}}}"), value);
            }
            if text.contains("${") {
                return Err("LAUNCH_UNKNOWN_ARGUMENT_PLACEHOLDER".into());
            }
            result.push(text);
        }
    }
    Ok(result)
}
pub fn args(
    installed: &Installed,
    game_dir: &Path,
    session: &Session,
    memory: u32,
) -> Result<Vec<String>, String> {
    if !(1024..=16384).contains(&memory) {
        return Err("LAUNCH_INVALID_MEMORY".into());
    }
    let classpath = std::env::join_paths(&installed.classpath)
        .map_err(|_| "LAUNCH_INVALID_CLASSPATH")?
        .to_string_lossy()
        .to_string();
    let vars = HashMap::from([
        (
            "natives_directory",
            installed.natives.to_string_lossy().to_string(),
        ),
        ("launcher_name", "Midnight Launcher".into()),
        ("launcher_version", "0.2.0".into()),
        ("classpath", classpath),
        ("auth_player_name", session.profile.name.clone()),
        ("version_name", installed.info.id.clone()),
        ("game_directory", game_dir.to_string_lossy().to_string()),
        (
            "assets_root",
            installed.assets.to_string_lossy().to_string(),
        ),
        ("assets_index_name", installed.info.asset_index.id.clone()),
        ("auth_uuid", session.profile.id.clone()),
        ("auth_access_token", session.token.clone()),
        ("clientid", session.client.clone()),
        ("auth_xuid", session.xuid.clone()),
        (
            "user_type",
            if session.client.is_empty() {
                "legacy"
            } else {
                "msa"
            }
            .into(),
        ),
        ("version_type", installed.info.kind.clone()),
        (
            "library_directory",
            installed
                .root
                .join("libraries")
                .to_string_lossy()
                .to_string(),
        ),
        ("classpath_separator", ":".into()),
    ]);
    let mut args = vec!["-Xms512M".into(), format!("-Xmx{memory}M")];
    args.extend(expand(&installed.info.arguments.jvm, &vars)?);
    if !args.iter().any(|a| a == "-XstartOnFirstThread") {
        args.push("-XstartOnFirstThread".into());
    }
    if let Some(argument) = &installed.logging_argument {
        args.push(argument.clone());
    }
    args.push(installed.info.main_class.clone());
    args.extend(expand(&installed.info.arguments.game, &vars)?);
    Ok(args)
}
async fn log_stream<R: tokio::io::AsyncRead + Unpin>(stream: R, path: PathBuf, secret: String) {
    let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    while let Ok(n) = reader.read_until(b'\n', &mut line).await {
        if n == 0 {
            break;
        }
        let text = String::from_utf8_lossy(&line);
        let text = if secret == "0" {
            text.into_owned()
        } else {
            text.replace(&secret, "[REDACTED]")
        };
        let _ = file.write_all(text.as_bytes()).await;
        line.clear();
    }
}
pub async fn start(
    installed: &Installed,
    java: &Path,
    data: &Path,
    instance: &str,
    session: &Session,
    memory: u32,
    job: &super::Job,
) -> Result<Child, String> {
    let instance = safe_path(instance)?;
    if instance.components().count() != 1 {
        return Err("LAUNCH_INVALID_INSTANCE".into());
    }
    let game = data.join("instances").join(&instance);
    tokio::fs::create_dir_all(&game)
        .await
        .map_err(|_| "LAUNCH_CREATE_DIRECTORY")?;
    let logs = data.join("logs");
    tokio::fs::create_dir_all(&logs)
        .await
        .map_err(|_| "LAUNCH_CREATE_DIRECTORY")?;
    let log = logs.join(format!("minecraft-{}.log", instance.to_string_lossy()));
    job.check()?;
    let arguments = args(installed, &game, session, memory)?;
    // No shell and no token/argument logging. JVM credentials remain in Rust.
    let mut child = Command::new(java)
        .args(arguments)
        .current_dir(&game)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .spawn()
        .map_err(|_| "LAUNCH_JAVA_FAILED")?;
    if let Some(out) = child.stdout.take() {
        tokio::spawn(log_stream(out, log.clone(), session.token.clone()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(log_stream(err, log, session.token.clone()));
    }
    Ok(child)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_are_not_shell_split() {
        let vars = HashMap::from([("game_directory", "/path with spaces/game".to_string())]);
        assert_eq!(
            expand(
                &[
                    serde_json::json!("--gameDir"),
                    serde_json::json!("${game_directory}")
                ],
                &vars
            )
            .unwrap(),
            vec!["--gameDir", "/path with spaces/game"]
        );
        assert!(expand(&[serde_json::json!("${missing}")], &vars).is_err());
    }
}
