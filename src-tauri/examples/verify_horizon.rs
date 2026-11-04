//! Prepare the pinned Rust client against an explicitly supplied existing Minecraft copy.
//! An optional isolated title-screen smoke test does not verify Microsoft sign-in or gameplay.
use midnight_launcher_lib::{
    auth,
    minecraft::{horizon, Job},
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let data = PathBuf::from(args.get(1).ok_or("Pass an isolated output directory")?);
    let minecraft = PathBuf::from(
        args.get(2)
            .ok_or("Pass your existing Minecraft directory")?,
    );
    let server = PathBuf::from(args.get(3).ok_or("Pass your existing 26.3 server JAR")?);
    if [&data, &minecraft, &server]
        .iter()
        .any(|p| !p.is_absolute())
    {
        return Err("Use absolute paths".into());
    }
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    let prepared = horizon::prepare(
        &data,
        "rust-smoke",
        "",
        Some(horizon::MinecraftSource {
            directory: &minecraft,
            server_jar: &server,
        }),
        &job,
    )
    .await?;
    let mut report = serde_json::json!({"horizon": horizon::VERSION, "minecraft": "26.3", "resources": prepared.resources(), "authenticated": false, "gameplayTested": false});
    if args.iter().any(|arg| arg == "--launch") {
        let session = auth::offline_session("MidnightCheck")?;
        let mut child = horizon::start(
            &prepared,
            &data,
            "rust-smoke",
            &session,
            Some("MidnightCheck"),
            &job,
        )
        .await?;
        println!("Started native title-screen smoke process {:?}", child.id());
        let seconds = if args.iter().any(|arg| arg == "--inspect") {
            120
        } else {
            30
        };
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "Could not inspect native process")?
        {
            return Err(format!("Native client exited early: {status}"));
        }
        report["runningAfter30Seconds"] = serde_json::json!(true);
        report["observedSeconds"] = serde_json::json!(seconds);
        child
            .kill()
            .await
            .map_err(|_| "Could not stop native smoke process")?;
        let _ = child.wait().await;
    }
    std::fs::write(
        data.join("horizon-check.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|_| "Could not write verification report")?;
    println!("{report}");
    Ok(())
}
