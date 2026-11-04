//! Real Modrinth search, dependency installation and idempotency in a separate directory.
use midnight_launcher_lib::minecraft::{
    catalog::{self, Source},
    Job,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let root = PathBuf::from(
        args.get(1)
            .ok_or("Pass an isolated test output directory")?,
    );
    let query = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("lambdynamiclights");
    let game = args.get(3).map(String::as_str).unwrap_or("1.21.1");
    let loader = args.get(4).map(String::as_str).unwrap_or("fabric");
    let page = catalog::search_for_loader(
        Source::Modrinth,
        None,
        query,
        game,
        0,
        catalog::SearchSort::Downloads,
        loader,
    )
    .await?;
    let project = page
        .projects
        .iter()
        .find(|p| p.slug == query)
        .ok_or("Project not found")?;
    let versions =
        catalog::releases_for_loader(Source::Modrinth, None, &project.id, game, loader).await?;
    let release = versions
        .iter()
        .find(|v| v.can_download)
        .ok_or("No compatible release")?;
    let job = Job {
        cancel: Arc::new(AtomicBool::new(false)),
        app: None,
    };
    println!(
        "Installing {} {} for Minecraft {game} / {loader}: {} bytes",
        project.slug, release.number, release.size
    );
    let first = catalog::install_for_loader(
        Source::Modrinth,
        None,
        &root,
        "catalog-check",
        &project.id,
        &release.id,
        game,
        &job,
        loader,
    )
    .await?;
    let second = catalog::install_for_loader(
        Source::Modrinth,
        None,
        &root,
        "catalog-check",
        &project.id,
        &release.id,
        game,
        &job,
        loader,
    )
    .await?;
    if first.installed + first.reused < 2 || second.installed != 0 || second.reused < 2 {
        return Err("Dependency installation or reuse failed".into());
    }
    let files = midnight_launcher_lib::minecraft::mods::list(&root, "catalog-check")?;
    let report = serde_json::json!({"project":project,"release":release,"game":game,"loader":loader,"first":first,"second":second,"files":files,"gameplayTested":false});
    std::fs::create_dir_all(&root).map_err(|_| "Could not create test directory")?;
    std::fs::write(
        root.join("catalog-check.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|_| "Could not write report")?;
    println!("{report}");
    Ok(())
}
