//! Real Windows installation and optional offline title-menu smoke test.
//! Never restores credentials or claims authenticated gameplay.
use midnight_launcher_lib::{
    auth,
    minecraft::{self, Job},
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let data = PathBuf::from(
        args.get(1)
            .ok_or("Pass an explicit verification directory")?,
    );
    if !data.is_absolute() {
        return Err("Verification directory must be absolute".into());
    }
    let version = args.get(2).map(String::as_str).unwrap_or("1.21.1");
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    let installed = minecraft::install(version, &data.join("minecraft"), &job).await?;
    let java = minecraft::java::ensure(
        &data.join("java"),
        "",
        installed.info.java_version.major,
        &job,
    )
    .await?;
    let files: Vec<_> = std::fs::read_dir(&installed.natives)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let mut report = serde_json::json!({"version":version,"java":java,"natives":installed.natives,"nativeFiles":files,"classpathEntries":installed.classpath.len(),"authenticated":false});
    if args.iter().any(|arg| arg == "--launch") {
        let session = auth::offline_session("MidnightCheck")?;
        let mut child = minecraft::launch::start(
            &installed,
            &java,
            &data,
            "windows-smoke",
            &session,
            2048,
            &job,
        )
        .await?;
        let seconds = if args.iter().any(|arg| arg == "--inspect") {
            120
        } else {
            30
        };
        println!(
            "Started offline title-menu smoke process {:?}; stopping after {seconds} seconds",
            child.id()
        );
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            report["earlyExit"] = serde_json::json!(status.code());
            std::fs::write(
                data.join("windows-verification.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            return Err(format!("Game exited early: {status}"));
        }
        report["runningAfter30Seconds"] = serde_json::json!(true);
        child.kill().await.map_err(|e| e.to_string())?;
        let _ = child.wait().await;
        // Let the redacted stdout/stderr reader flush before exiting the harness.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    std::fs::write(
        data.join("windows-verification.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "Windows verification report: {}",
        data.join("windows-verification.json").display()
    );
    Ok(())
}
