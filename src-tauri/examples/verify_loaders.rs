//! Official installation plus an optional isolated, unauthenticated title-menu smoke test.
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
    let args: Vec<_> = std::env::args().collect();
    let data = PathBuf::from(
        args.get(1)
            .ok_or("Pass an absolute verification directory")?,
    );
    if !data.is_absolute() {
        return Err("Verification directory must be absolute".into());
    }
    let kind = args.get(2).map(String::as_str).unwrap_or("neoforge");
    let game = args.get(3).map(String::as_str).unwrap_or("1.21.1");
    let versions = minecraft::loader::versions_for(game, kind).await?;
    let release = args
        .get(4)
        .filter(|s| !s.starts_with("--"))
        .or(versions.first())
        .ok_or("No compatible loader versions")?
        .clone();
    println!("Preparing {kind} {release} for Minecraft {game}");
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    let mut installed = minecraft::install(game, &data.join("minecraft"), &job).await?;
    let java = minecraft::java::ensure(
        &data.join("java"),
        "",
        installed.info.java_version.major,
        &job,
    )
    .await?;
    let loader = Some(minecraft::loader::Loader {
        kind: kind.into(),
        version: release.clone(),
    });
    minecraft::loader::apply(&mut installed, &loader, &java, &job).await?;
    if !installed.classpath.iter().all(|p| p.is_file()) {
        return Err("Missing runtime classpath entry".into());
    }
    let instance = format!("loader-check-{kind}-{game}");
    let session = auth::offline_session("LoaderCheck")?;
    let _ = minecraft::launch::args(
        &installed,
        &data.join("instances").join(&instance),
        &session,
        4096,
    )?;
    let mut report = serde_json::json!({"loader":kind,"loaderVersion":release,"game":game,"mainClass":installed.info.main_class,
        "classpathEntries":installed.classpath.len(),"authenticated":false,"gameplayTested":false,"launched":false});
    if args.iter().any(|a| a == "--launch") {
        let mut child =
            minecraft::launch::start(&installed, &java, &data, &instance, &session, 4096, &job)
                .await?;
        report["launched"] = true.into();
        report["pid"] = serde_json::json!(child.id());
        for _ in 0..45 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                report["earlyExit"] = serde_json::json!(status.code());
                std::fs::write(
                    data.join(format!("{kind}-check.json")),
                    serde_json::to_vec_pretty(&report).unwrap(),
                )
                .map_err(|e| e.to_string())?;
                return Err(format!(
                    "{kind} exited early: {status}; see logs/minecraft-{instance}.log"
                ));
            }
        }
        child.kill().await.map_err(|e| e.to_string())?;
        let _ = child.wait().await;
        report["aliveAfterSeconds"] = 45.into();
        let log = std::fs::read_to_string(
            data.join("instances")
                .join(&instance)
                .join("logs/latest.log"),
        )
        .map_err(|_| "Minecraft did not produce its own log")?;
        let ready = log.contains("Sound engine started")
            && log.contains("Created:")
            && !log.contains("Failed to start FML")
            && !log.contains("Loading errors encountered");
        report["rendererReady"] = ready.into();
        if !ready {
            std::fs::write(
                data.join(format!("{kind}-check.json")),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            return Err(format!(
                "{kind} did not finish renderer initialization; inspect logs/latest.log"
            ));
        }
    }
    std::fs::write(
        data.join(format!("{kind}-check.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("{kind} verification completed: {report}");
    Ok(())
}
