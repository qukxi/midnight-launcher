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
    minecraft::loader::apply(&mut installed, &loader, &job).await?;
    let java = minecraft::java::ensure(
        &data.join("java"),
        &java_path,
        installed.info.java_version.major,
        &job,
    )
    .await?;
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
    let offline = offline_name
        .as_deref()
        .map(auth::offline_session)
        .transpose()?;
    let job = job(&app, &state);
    if offline.is_none() {
        let mut manager = accounts.manager.lock().await;
        auth::session(&mut manager).await?;
    }
    job.check()?;
    let data = paths(&app)?;
    minecraft::loader::validate(&loader)?;
    let mut installed = minecraft::install(&version, &data.join("minecraft"), &job).await?;
    minecraft::loader::apply(&mut installed, &loader, &job).await?;
    let java = minecraft::java::ensure(
        &data.join("java"),
        &java_path,
        installed.info.java_version.major,
        &job,
    )
    .await?;
    job.check()?;
    let mut manager = accounts.manager.lock().await;
    let session = if let Some(ref local) = offline {
        local
    } else {
        let online = auth::session(&mut manager).await?;
        auth::verify_entitlement(online).await?;
        online
    };
    job.check()?;
    let child = minecraft::launch::start(
        &installed,
        &java,
        &data,
        &instance_id,
        session,
        memory,
        &job,
    )
    .await?;
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
async fn loader_versions(version: String) -> Result<Vec<String>, String> {
    minecraft::loader::versions(&version).await
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(OperationState::default())
        .manage(AuthState::default())
        .manage(GameState::default())
        .invoke_handler(tauri::generate_handler![
            list_versions,
            auth_status,
            begin_login,
            poll_login,
            cancel_login,
            open_login,
            get_account,
            restore_account,
            sign_out,
            cancel_operation,
            install_game,
            game_status,
            stop_game,
            launch_game,
            loader_versions,
            read_game_log
        ])
        .run(tauri::generate_context!())
        .expect("Could not start Midnight Launcher");
}
