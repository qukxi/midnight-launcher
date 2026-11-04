//! Downloads a real Modrinth modpack into a caller-supplied, isolated test directory.
use midnight_launcher_lib::minecraft::{modpacks, Job};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

#[tokio::main]
async fn main() -> Result<(), String> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Pass an isolated test output directory")?,
    );
    let project = std::env::args().nth(2).unwrap_or_else(|| "sop".into());
    let releases = modpacks::releases(&project).await?;
    let release = releases.first().ok_or("No supported modpack release")?;
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    let installed = modpacks::install(&root, &project, &release.id, true, &job).await?;
    let directory = root.join("instances").join(&installed.instance.id);
    if !directory.join("mods").is_dir() || !directory.join(".midnight-modpack.json").is_file() {
        return Err("Installed files are missing".into());
    }
    let report = serde_json::json!({ "release": release, "installed": installed, "directory": directory, "gameplayTested": false });
    std::fs::write(
        root.join("modpack-check.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|_| "Could not write report")?;
    println!("{report}");
    Ok(())
}
