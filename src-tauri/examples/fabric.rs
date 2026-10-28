//! Exercise real Fabric library installation without launching a game.
use midnight_launcher_lib::minecraft::{self, Job};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
#[tokio::main]
async fn main() -> Result<(), String> {
    let root = PathBuf::from(std::env::args().nth(1).ok_or("Pass a test directory")?);
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    let mut installed = minecraft::install::Installed {
        info: serde_json::from_str(include_str!("../tests/fixtures/1.21.1.json"))
            .map_err(|e| e.to_string())?,
        classpath: vec![],
        root: root.clone(),
        assets: root.join("assets"),
        natives: root.join("natives"),
        logging_argument: None,
    };
    let versions = minecraft::loader::versions("1.21.1").await?;
    if versions.is_empty() {
        return Err("No Fabric versions returned".into());
    }
    minecraft::loader::apply(
        &mut installed,
        &Some(minecraft::loader::Loader {
            kind: "fabric".into(),
            version: "0.16.14".into(),
        }),
        &job,
    )
    .await?;
    println!(
        "Fabric installed: {} libraries; {}",
        installed.classpath.len(),
        installed.info.main_class
    );
    Ok(())
}
