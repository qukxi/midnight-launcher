pub mod auth;
pub mod minecraft;
use auth::AuthState;
use minecraft::{launch::GameState, Job};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
#[derive(Default)]
struct OperationState {
    gate: tokio::sync::Mutex<()>,
    cancel: Arc<AtomicBool>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallReport {
    version: String,
    directory: String,
    java_path: String,
}
fn paths(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|_| "APP_DATA_PATH_FAILED".into())
}
fn job(app: &tauri::AppHandle, state: &OperationState) -> Job {
    state.cancel.store(false, Ordering::Relaxed);
    Job {
        cancel: state.cancel.clone(),
        app: Some(app.clone()),
    }
}
#[tauri::command]
async fn list_versions() -> Result<Vec<minecraft::VersionSummary>, String> {
    minecraft::list_versions().await
}
#[tauri::command]
fn auth_status(client_id: String) -> auth::AuthStatus {
    auth::status(&client_id)
}
#[tauri::command]
async fn auth_diagnostics(
    state: tauri::State<'_, AuthState>,
) -> Result<Option<auth::AuthDiagnostics>, String> {
    Ok(auth::diagnostics(&*state.manager.lock().await))
}
#[tauri::command]
async fn begin_login(
    state: tauri::State<'_, AuthState>,
    client_id: String,
) -> Result<auth::DevicePrompt, String> {
    let mut manager = state.manager.lock().await;
    state.cancelled.store(false, Ordering::Relaxed);
    auth::begin(&mut manager, &client_id).await
}
#[tauri::command]
async fn poll_login(
    state: tauri::State<'_, AuthState>,
    id: String,
) -> Result<auth::PollResult, String> {
    auth::poll(&mut *state.manager.lock().await, &id, &state.cancelled).await
}
#[tauri::command]
async fn cancel_login(state: tauri::State<'_, AuthState>) -> Result<(), String> {
    state.cancelled.store(true, Ordering::Relaxed);
    let mut manager = state.manager.lock().await;
    auth::cancel(&mut manager);
    auth::sign_out(&mut manager)
}
#[tauri::command]
async fn open_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, AuthState>,
    id: String,
) -> Result<(), String> {
    let uri = auth::login_uri(&*state.manager.lock().await, &id)?;
    app.opener()
        .open_url(uri, None::<&str>)
        .map_err(|_| "AUTH_BROWSER_OPEN_FAILED".into())
}
#[tauri::command]
async fn get_account(state: tauri::State<'_, AuthState>) -> Result<Option<auth::Profile>, String> {
    auth::profile(&*state.manager.lock().await)
}
#[tauri::command]
async fn get_offline_account(
    state: tauri::State<'_, AuthState>,
) -> Result<Option<auth::Profile>, String> {
    auth::offline_account(&*state.manager.lock().await)
}
#[tauri::command]
async fn select_offline_profile(
    state: tauri::State<'_, AuthState>,
    name: String,
) -> Result<auth::Profile, String> {
    auth::launch_offline_identity(&*state.manager.lock().await, Some(&name))?
        .map(|session| session.profile)
        .ok_or_else(|| "OFFLINE_LOGIN_REQUIRED".into())
}
#[tauri::command]
async fn restore_account(state: tauri::State<'_, AuthState>) -> Result<auth::Profile, String> {
    Ok(auth::session(&mut *state.manager.lock().await)
        .await?
        .profile
        .clone())
}
#[tauri::command]
async fn sign_out(state: tauri::State<'_, AuthState>) -> Result<(), String> {
    state.cancelled.store(true, Ordering::Relaxed);
    auth::sign_out(&mut *state.manager.lock().await)
}
#[tauri::command]
fn cancel_operation(state: tauri::State<'_, OperationState>) {
    state.cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
async fn install_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    version: String,
    java_path: String,
    loader: Option<minecraft::loader::Loader>,
) -> Result<InstallReport, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    let job = job(&app, &state);
    let data = paths(&app)?;
    minecraft::loader::validate(&loader)?;
    let mut installed = minecraft::install(&version, &data.join("minecraft"), &job).await?;
    let java = minecraft::java::ensure(
        &data.join("java"),
        &java_path,
        installed.info.java_version.major,
        &job,
    )
    .await?;
    minecraft::loader::apply(&mut installed, &loader, &java, &job).await?;
    job.report("ready", 1, 1);
    Ok(InstallReport {
        version,
        directory: installed.root.to_string_lossy().to_string(),
        java_path: java.to_string_lossy().to_string(),
    })
}
#[tauri::command]
async fn game_status(
    state: tauri::State<'_, GameState>,
) -> Result<minecraft::launch::GameStatus, String> {
    minecraft::launch::status(&mut *state.0.lock().await)
}
#[tauri::command]
async fn stop_game(state: tauri::State<'_, GameState>) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    if let Some(child) = guard.as_mut() {
        child.kill().await.map_err(|_| "GAME_STOP_FAILED")?;
    }
    *guard = None;
    Ok(())
}
#[tauri::command]
async fn launch_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    accounts: tauri::State<'_, AuthState>,
    game: tauri::State<'_, GameState>,
    instance_id: String,
    version: String,
    java_path: String,
    memory: u32,
    offline_name: Option<String>,
    minimize_on_launch: Option<bool>,
    restore_on_exit: Option<bool>,
    loader: Option<minecraft::loader::Loader>,
    engine: Option<String>,
) -> Result<minecraft::launch::GameStatus, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    if minecraft::launch::status(&mut *game.0.lock().await)?.running {
        return Err("GAME_ALREADY_RUNNING".into());
    }
    if !(1024..=16384).contains(&memory) {
        return Err("LAUNCH_INVALID_MEMORY".into());
    }
    let relative = minecraft::install::safe_path(&instance_id)?;
    if relative.components().count() != 1 {
        return Err("LAUNCH_INVALID_INSTANCE".into());
    }
    let native = match engine.as_deref() {
        None | Some("java") => false,
        Some("horizon") => true,
        _ => return Err("LAUNCH_INVALID_ENGINE".into()),
    };
    if native && loader.is_some() {
        return Err("HORIZON_NO_JAVA_LOADER".into());
    }
    if native {
        minecraft::horizon::validate(&version)?;
    }
    let offline = {
        let manager = accounts.manager.lock().await;
        auth::launch_offline_identity(&manager, offline_name.as_deref())?
    };
    let job = job(&app, &state);
    if offline.is_none() {
        let mut manager = accounts.manager.lock().await;
        auth::session(&mut manager).await?;
    }
    job.check()?;
    let data = paths(&app)?;
    let child = if native {
        if offline.is_none() {
            let mut manager = accounts.manager.lock().await;
            let session = auth::session(&mut manager).await?;
            auth::verify_entitlement(session).await?;
        }
        let prepared =
            minecraft::horizon::prepare(&data, &instance_id, &java_path, None, &job).await?;
        // Preparation can take several minutes. Resolve a fresh session after it finishes.
        let mut manager = accounts.manager.lock().await;
        let session = if let Some(ref local) = offline {
            auth::require_offline_access(&manager)?;
            local
        } else {
            auth::session(&mut manager).await?
        };
        minecraft::horizon::start(
            &prepared,
            &data,
            &instance_id,
            session,
            offline.as_ref().map(|s| s.profile.name.as_str()),
            &job,
        )
        .await?
    } else {
        minecraft::loader::validate(&loader)?;
        let mut installed = minecraft::install(&version, &data.join("minecraft"), &job).await?;
        let java = minecraft::java::ensure(
            &data.join("java"),
            &java_path,
            installed.info.java_version.major,
            &job,
        )
        .await?;
        minecraft::loader::apply(&mut installed, &loader, &java, &job).await?;
        job.check()?;
        let mut manager = accounts.manager.lock().await;
        let session = if let Some(ref local) = offline {
            auth::require_offline_access(&manager)?;
            local
        } else {
            let online = auth::session(&mut manager).await?;
            auth::verify_entitlement(online).await?;
            online
        };
        job.check()?;
        minecraft::launch::start(
            &installed,
            &java,
            &data,
            &instance_id,
            session,
            memory,
            &job,
        )
        .await?
    };
    let mut guard = game.0.lock().await;
    let launched_pid = child.id();
    *guard = Some(child);
    if minimize_on_launch.unwrap_or(false) {
        if let Some(window) = app.get_webview_window("main") {
            if window.minimize().is_ok() && restore_on_exit.unwrap_or(true) {
                let monitor_app = app.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let state = monitor_app.state::<GameState>();
                        let mut guard = state.0.lock().await;
                        if let Some(game) = guard.as_ref() {
                            if game.id() != launched_pid {
                                return;
                            }
                        }
                        let exited = minecraft::launch::status(&mut guard)
                            .map(|s| !s.running)
                            .unwrap_or(false);
                        drop(guard);
                        if exited {
                            if let Some(window) = monitor_app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                            }
                            return;
                        }
                    }
                });
            }
        }
    }
    job.report("running", 1, 1);
    minecraft::launch::status(&mut guard)
}
#[tauri::command]
async fn read_game_log(app: tauri::AppHandle, instance_id: String) -> Result<String, String> {
    minecraft::logs::read(&paths(&app)?, &instance_id).await
}
#[tauri::command]
async fn loader_versions(version: String, kind: Option<String>) -> Result<Vec<String>, String> {
    minecraft::loader::versions_for(&version, kind.as_deref().unwrap_or("fabric")).await
}
#[tauri::command]
async fn list_mods(
    app: tauri::AppHandle,
    instance_id: String,
) -> Result<Vec<minecraft::mods::ModFile>, String> {
    let data = paths(&app)?;
    tauri::async_runtime::spawn_blocking(move || minecraft::mods::list(&data, &instance_id))
        .await
        .map_err(|_| "MODS_READ_FAILED")?
}
#[tauri::command]
async fn import_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    game: tauri::State<'_, GameState>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    if minecraft::launch::status(&mut *game.0.lock().await)?.running {
        return Err("MODS_GAME_RUNNING".into());
    }
    let instance = request
        .headers()
        .get("x-instance-id")
        .and_then(|h| h.to_str().ok())
        .ok_or("MODS_UNSAFE_PATH")?
        .to_owned();
    let name: String = serde_json::from_str(
        request
            .headers()
            .get("x-mod-name")
            .and_then(|h| h.to_str().ok())
            .ok_or("MODS_UNSAFE_PATH")?,
    )
    .map_err(|_| "MODS_UNSAFE_PATH")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("MODS_INVALID_JAR".into());
    };
    if bytes.len() > minecraft::mods::MAX_MOD_SIZE {
        return Err("MODS_TOO_LARGE".into());
    }
    let bytes = bytes.clone();
    let data = paths(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        minecraft::mods::import(&data, &instance, &name, &bytes)
    })
    .await
    .map_err(|_| "MODS_WRITE_FAILED")?
}
#[tauri::command]
async fn set_mod_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    game: tauri::State<'_, GameState>,
    instance_id: String,
    file_name: String,
    enabled: bool,
) -> Result<(), String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    if minecraft::launch::status(&mut *game.0.lock().await)?.running {
        return Err("MODS_GAME_RUNNING".into());
    }
    let data = paths(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        minecraft::mods::set_enabled(&data, &instance_id, &file_name, enabled)
    })
    .await
    .map_err(|_| "MODS_WRITE_FAILED")?
}
#[tauri::command]
async fn open_mods_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    instance_id: String,
) -> Result<(), String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    let data = paths(&app)?;
    let dir = tauri::async_runtime::spawn_blocking(move || {
        minecraft::mods::directory(&data, &instance_id, true)
    })
    .await
    .map_err(|_| "MODS_WRITE_FAILED")??
    .ok_or("MODS_WRITE_FAILED")?;
    app.opener()
        .open_path(dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|_| "MODS_OPEN_FAILED".into())
}
#[tauri::command]
async fn catalog_status(
    state: tauri::State<'_, minecraft::catalog::CatalogState>,
) -> Result<bool, String> {
    Ok(state.key.lock().await.is_some())
}
#[tauri::command]
async fn connect_curseforge(
    state: tauri::State<'_, minecraft::catalog::CatalogState>,
    key: String,
) -> Result<(), String> {
    let key = minecraft::catalog::connect(key).await?;
    *state.key.lock().await = Some(key);
    Ok(())
}
#[tauri::command]
async fn disconnect_curseforge(
    state: tauri::State<'_, minecraft::catalog::CatalogState>,
) -> Result<(), String> {
    *state.key.lock().await = None;
    Ok(())
}
#[tauri::command]
async fn search_catalog(
    state: tauri::State<'_, minecraft::catalog::CatalogState>,
    source: minecraft::catalog::Source,
    query: String,
    game_version: String,
    offset: usize,
    sort: Option<minecraft::catalog::SearchSort>,
    loader_kind: Option<String>,
) -> Result<minecraft::catalog::SearchPage, String> {
    let key = state.key.lock().await.clone();
    minecraft::catalog::search_for_loader(
        source,
        key,
        &query,
        &game_version,
        offset,
        sort.unwrap_or_default(),
        loader_kind.as_deref().unwrap_or("fabric"),
    )
    .await
}
#[tauri::command]
async fn catalog_versions(
    state: tauri::State<'_, minecraft::catalog::CatalogState>,
    source: minecraft::catalog::Source,
    project_id: String,
    game_version: String,
    loader_kind: Option<String>,
) -> Result<Vec<minecraft::catalog::Release>, String> {
    let key = state.key.lock().await.clone();
    minecraft::catalog::releases_for_loader(
        source,
        key,
        &project_id,
        &game_version,
        loader_kind.as_deref().unwrap_or("fabric"),
    )
    .await
}
#[tauri::command]
async fn install_catalog_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    catalog: tauri::State<'_, minecraft::catalog::CatalogState>,
    game: tauri::State<'_, GameState>,
    source: minecraft::catalog::Source,
    project_id: String,
    version_id: String,
    game_version: String,
    instance_id: String,
    loader: Option<minecraft::loader::Loader>,
    engine: String,
) -> Result<minecraft::catalog::InstallResult, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    if minecraft::launch::status(&mut *game.0.lock().await)?.running {
        return Err("MODS_GAME_RUNNING".into());
    }
    minecraft::loader::validate(&loader)?;
    if engine != "java" || loader.is_none() {
        return Err("CATALOG_LOADER_REQUIRED".into());
    }
    let key = catalog.key.lock().await.clone();
    minecraft::catalog::install_for_loader(
        source,
        key,
        &paths(&app)?,
        &instance_id,
        &project_id,
        &version_id,
        &game_version,
        &job(&app, &state),
        &loader.as_ref().ok_or("CATALOG_LOADER_REQUIRED")?.kind,
    )
    .await
}
#[tauri::command]
async fn open_catalog_project(
    app: tauri::AppHandle,
    source: minecraft::catalog::Source,
    slug: String,
) -> Result<(), String> {
    app.opener()
        .open_url(minecraft::catalog::page_url(source, &slug)?, None::<&str>)
        .map_err(|_| "CATALOG_OPEN_FAILED".into())
}
#[tauri::command]
async fn open_curseforge_setup(app: tauri::AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("https://console.curseforge.com/", None::<&str>)
        .map_err(|_| "CATALOG_OPEN_FAILED".into())
}
#[tauri::command]
async fn search_modpacks(
    query: String,
    offset: usize,
    sort: minecraft::catalog::SearchSort,
) -> Result<minecraft::catalog::SearchPage, String> {
    minecraft::modpacks::search(&query, offset, sort).await
}
#[tauri::command]
async fn modpack_versions(
    project_id: String,
) -> Result<Vec<minecraft::modpacks::PackRelease>, String> {
    minecraft::modpacks::releases(&project_id).await
}
#[tauri::command]
async fn install_modpack(
    app: tauri::AppHandle,
    state: tauri::State<'_, OperationState>,
    game: tauri::State<'_, GameState>,
    project_id: String,
    version_id: String,
    include_optional: bool,
) -> Result<minecraft::modpacks::InstallResult, String> {
    let _guard = state
        .gate
        .try_lock()
        .map_err(|_| "OPERATION_ALREADY_RUNNING")?;
    if minecraft::launch::status(&mut *game.0.lock().await)?.running {
        return Err("MODS_GAME_RUNNING".into());
    }
    minecraft::modpacks::install(
        &paths(&app)?,
        &project_id,
        &version_id,
        include_optional,
        &job(&app, &state),
    )
    .await
}
#[tauri::command]
async fn open_modpack_project(app: tauri::AppHandle, project_id: String) -> Result<(), String> {
    minecraft::mods::component(&project_id)?;
    if !project_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err("CATALOG_INVALID_ID".into());
    }
    app.opener()
        .open_url(
            format!("https://modrinth.com/modpack/{project_id}"),
            None::<&str>,
        )
        .map_err(|_| "CATALOG_OPEN_FAILED".into())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(OperationState::default())
        .manage(AuthState::default())
        .manage(GameState::default())
        .manage(minecraft::catalog::CatalogState::default())
        .invoke_handler(tauri::generate_handler![
            list_versions,
            auth_status,
            auth_diagnostics,
            begin_login,
            poll_login,
            cancel_login,
            open_login,
            get_account,
            get_offline_account,
            select_offline_profile,
            restore_account,
            sign_out,
            cancel_operation,
            install_game,
            game_status,
            stop_game,
            launch_game,
            loader_versions,
            read_game_log,
            list_mods,
            import_mod,
            set_mod_enabled,
            open_mods_folder,
            catalog_status,
            connect_curseforge,
            disconnect_curseforge,
            search_catalog,
            catalog_versions,
            install_catalog_mod,
            open_catalog_project,
            open_curseforge_setup,
            search_modpacks,
            modpack_versions,
            install_modpack,
            open_modpack_project
        ])
        .run(tauri::generate_context!())
        .expect("Could not start Midnight Launcher");
}

#[cfg(test)]
mod command_stack_tests {
    use super::*;

    fn future_size<F: std::future::Future>(_: impl FnOnce() -> F) -> usize {
        std::mem::size_of::<F>()
    }

    // Only infer argument types; this closure is never called and never reads credentials.
    fn unused<T>() -> T {
        unreachable!()
    }

    #[test]
    fn launch_request_fits_the_windows_event_thread_stack() {
        // Tauri nests and moves command futures through several IPC/runtime wrappers.
        // Reserve most of the 1 MiB Windows stack for those frames and WebView callbacks.
        for (command, bytes) in [
            (
                "launch_game",
                future_size(|| {
                    launch_game(
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                        unused(),
                    )
                }),
            ),
            (
                "install_game",
                future_size(|| install_game(unused(), unused(), unused(), unused(), unused())),
            ),
            (
                "horizon::prepare",
                future_size(|| {
                    minecraft::horizon::prepare(unused(), unused(), unused(), unused(), unused())
                }),
            ),
        ] {
            assert!(bytes < 32 * 1024, "{command} future occupies {bytes} bytes");
        }
    }
}
