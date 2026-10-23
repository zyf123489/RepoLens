use crate::{
    edit::{Change, Preview, WriteResult},
    model::*,
    scan::{ScanOptions, ScanState},
    service::Service,
};
use std::sync::Arc;
use tauri::{Manager, State};
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppError::new("task", "后台任务中断，请重试。"))?
}
#[tauri::command]
async fn diagnostics(state: State<'_, Arc<Service>>) -> Result<Diagnostics> {
    let s = state.inner().clone();
    blocking(move || Ok(s.diagnostics())).await
}
#[tauri::command]
async fn list_entries(state: State<'_, Arc<Service>>) -> Result<Vec<Entry>> {
    let s = state.inner().clone();
    blocking(move || s.entries()).await
}
#[tauri::command]
async fn inspect_path(state: State<'_, Arc<Service>>, path: String) -> Result<Context> {
    let s = state.inner().clone();
    blocking(move || s.inspect(&path)).await
}
#[tauri::command]
async fn add_entries(state: State<'_, Arc<Service>>, paths: Vec<String>) -> Result<Vec<Entry>> {
    let s = state.inner().clone();
    blocking(move || s.add(paths)).await
}
#[tauri::command]
async fn read_snapshot(state: State<'_, Arc<Service>>, id: String) -> Result<Snapshot> {
    let s = state.inner().clone();
    blocking(move || s.snapshot(&id)).await
}
#[tauri::command]
async fn update_entry(
    state: State<'_, Arc<Service>>,
    id: String,
    alias: String,
    group: Option<String>,
    policy: Policy,
) -> Result<Entry> {
    let s = state.inner().clone();
    blocking(move || s.store.lock().unwrap().update(&id, &alias, &group, &policy)).await
}
#[tauri::command]
async fn remove_entry(state: State<'_, Arc<Service>>, id: String) -> Result<()> {
    let s = state.inner().clone();
    blocking(move || s.store.lock().unwrap().remove(&id)).await
}
#[tauri::command]
async fn list_groups(state: State<'_, Arc<Service>>) -> Result<Vec<String>> {
    let s = state.inner().clone();
    blocking(move || s.store.lock().unwrap().groups()).await
}
#[tauri::command]
async fn save_group(state: State<'_, Arc<Service>>, name: String) -> Result<()> {
    let s = state.inner().clone();
    blocking(move || s.store.lock().unwrap().add_group(&name)).await
}
#[tauri::command]
async fn delete_group(state: State<'_, Arc<Service>>, name: String) -> Result<()> {
    let s = state.inner().clone();
    blocking(move || s.store.lock().unwrap().delete_group(&name)).await
}
#[tauri::command]
async fn set_git(state: State<'_, Arc<Service>>, path: String) -> Result<Diagnostics> {
    let s = state.inner().clone();
    blocking(move || {
        if !valid_text(&path, 4096) {
            return Err(AppError::new("git_path", "Git 路径无效。"));
        }
        crate::git::Git::new(path.clone()).version()?;
        s.store.lock().unwrap().set_git(&path)?;
        Ok(s.diagnostics())
    })
    .await
}
#[tauri::command]
async fn create_preview(
    state: State<'_, Arc<Service>>,
    id: String,
    changes: Vec<Change>,
) -> Result<Preview> {
    let s = state.inner().clone();
    blocking(move || s.preview(&id, changes)).await
}
#[tauri::command]
async fn apply_preview(state: State<'_, Arc<Service>>, token: String) -> Result<WriteResult> {
    let s = state.inner().clone();
    blocking(move || s.apply(&token)).await
}
#[tauri::command]
async fn start_scan(state: State<'_, Arc<Service>>, options: ScanOptions) -> Result<String> {
    let s = state.inner().clone();
    blocking(move || s.scan_start(options)).await
}
#[tauri::command]
fn poll_scan(state: State<'_, Arc<Service>>, id: String) -> Result<ScanState> {
    state.scan_poll(&id)
}
#[tauri::command]
fn cancel_scan(state: State<'_, Arc<Service>>, id: String) -> Result<()> {
    state.scan_cancel(&id)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(Service::open(&dir)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            diagnostics,
            list_entries,
            inspect_path,
            add_entries,
            read_snapshot,
            update_entry,
            remove_entry,
            list_groups,
            save_group,
            delete_group,
            set_git,
            create_preview,
            apply_preview,
            start_scan,
            poll_scan,
            cancel_scan
        ])
        .run(tauri::generate_context!())
        .expect("RepoLens desktop runtime failed");
}
