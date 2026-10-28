//! Downloads and verifies real vanilla files + Java, without credentials or game launch.
use midnight_launcher_lib::minecraft::{self, Job};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
#[tokio::main]
async fn main() -> Result<(), String> {
    let data = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Pass an explicit application data directory")?,
    );
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    let installed = minecraft::install("1.21.1", &data.join("minecraft"), &job).await?;
    let java = minecraft::java::ensure(
        &data.join("java"),
        "",
        installed.info.java_version.major,
        &job,
    )
    .await?;
    if let Some(report) = std::env::args().nth(2) {
        std::fs::write(report,serde_json::to_vec(&serde_json::json!({"java":java,"classpath":installed.classpath,"mainClass":installed.info.main_class})).unwrap()).map_err(|e|e.to_string())?;
    }
    println!(
        "Verified vanilla {}: {} classpath entries, Java {} at {}",
        installed.info.id,
        installed.classpath.len(),
        installed.info.java_version.major,
        java.display()
    );
    Ok(())
}
