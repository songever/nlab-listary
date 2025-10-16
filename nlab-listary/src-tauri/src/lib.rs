
use std::sync::{Arc, RwLock};

use crate::parser::index_local_files;
use crate::{git_ops::update_local_repository, models::SearchIndex, search::SearchEngine};
use tauri::{Emitter, State};

pub const REPO_URL: &str = "https://github.com/ncatlab/nlab-content-html.git";
pub const GIT_REPO_PATH: &str = "nlab_mirror";
pub const DB_PATH: &str = "nlab_page_data.db";
pub const INDEX_PATH: &str = "nlab_page_index";

mod browser;
mod git_ops;
mod models;
mod parser;
mod search;
mod storage;

pub struct AppStateInner {
    search_engine: Option<search::TantivySearch>,
    storage: Option<storage::Storage>,
}

type AppState = Arc<RwLock<AppStateInner>>;

#[tauri::command]
fn get_search_results(state: State<AppState>, query: String) -> Result<Vec<SearchIndex>, String> {
    let state = state
        .read()
        .map_err(|e| format!("failed to lock state: {}", e))?;

    let search_engine = state
        .search_engine
        .as_ref()
        .ok_or_else(|| "search engine is not initialized".to_string())?;
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| "storage is not initialized".to_string())?;

    let results = search_engine
        .search(&query, 10)
        .map_err(|e| format!("failed to search: {}", e))?;

    let search_results = results
        .into_iter()
        .filter_map(|res| {
            storage
                .get_page(&res.id)
                .ok()
                .flatten()
                .map(|page| SearchIndex {
                    title: res.title,
                    url: page.url,
                })
        })
        .collect();

    Ok(search_results)
}

#[cfg(feature = "ignore")]
#[tauri::command]
fn sync_local_repo(state: State<AppState>) -> Result<(), String> {
    use std::path::Path;
    let path = Path::new(GIT_REPO_PATH);
    
    let mut state = state
        .write()
        .map_err(|e| format!("failed to lock state: {}", e))?;

    update_local_repository(path)
        .map_err(|e| format!("Synchronizing local repo failed: {}", e))?;

    let pages = index_local_files(path)
        .map_err(|e| format!("Parsing htmls failed: {}", e))?;

    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| "storage is not initialized".to_string())?;
    
    let search_engine = state
        .search_engine
        .as_mut()
        .ok_or_else(|| "search engine is not initialized".to_string())?;

    storage
        .save_pages_batch(&pages)
        .map_err(|e| format!("Saving pages to storage failed: {}", e))?;

    search_engine
        .update_pages_batch(&pages)
        .map_err(|e| format!("Building search index failed: {}", e))?;
    
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    use browser::open_url;

    // Validate URL format first
    if url.is_empty() {
        return Err("URL cannot be empty".to_string());
    }

    // Try to open the URL
    match open_url(&url) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("Failed to open URL '{}': {}", url, e);
            Err(format!("Failed to open URL: {}", e))
        }
    }
}

#[tauri::command]
fn is_ready(state: State<AppState>) -> Result<bool, String> {
    let state = state
        .read()
        .map_err(|e| format!("failed to lock state: {}", e))?;
    Ok(state.search_engine.is_some() && state.storage.is_some())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(RwLock::new(AppStateInner {
        search_engine: None,
        storage: None,
    }));

    let state_clone = app_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_search_results,
            open_url,
            is_ready,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            std::thread::spawn(move || {
                eprintln!("initializing ...");
                let _ = app_handle.emit("init-status", "Initializing...");

                match initialize_components(&app_handle) {
                    Ok((search_engine, storage)) => {
                        let mut state = state_clone.write().unwrap();
                        state.search_engine = Some(search_engine);
                        state.storage = Some(storage);
                        eprintln!("initialized successfully");
                        let _ = app_handle.emit("init-complete", true);
                    }
                    Err(e) => {
                        eprintln!("failed to initialize app state: {}", e);
                        let _ = app_handle.emit("init-error", format!("{}", e));
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn initialize_components(
    app_handle: &tauri::AppHandle,
) -> Result<(search::TantivySearch, storage::Storage), Box<dyn std::error::Error>> {
    use std::path::Path;
    let path = Path::new(GIT_REPO_PATH);
    let storage_path = Path::new(DB_PATH).join("storage");
    let index_path = Path::new(INDEX_PATH).join("index");

    let _ = app_handle.emit("init-status", "Synchronizing repository...");
    let _repo = update_local_repository(path)?;

    if !path.exists() {
        Err("local repo should exist after update".into())
    } else {
        let pages = index_local_files(path)?;
        let needs_full_rebuild = !storage_path.exists() || !index_path.exists();
        if needs_full_rebuild {
            let _ = app_handle.emit("init-status", "Parsing pages...");

            let _ = app_handle.emit("init-status", "Initializing storage...");
            let storage = storage::Storage::new(storage_path.to_str().unwrap())?;
            storage.save_pages_batch(&pages)?;

            let _ = app_handle.emit("init-status", "Building search index...");
            let mut search_engine = search::TantivySearch::new(index_path.to_str().unwrap())?;
            search_engine.build_index(&pages)?;

            Ok((search_engine, storage))
        } else {
            let _ = app_handle.emit("init-status", "Loading existing data...");

            let storage = storage::Storage::new(storage_path.to_str().unwrap())?;

            let _ = app_handle.emit("init-status", "Checking for index updates...");
            let mut search_engine = search::TantivySearch::new(index_path.to_str().unwrap())?;
            search_engine.update_pages_batch(&pages)?;

            Ok((search_engine, storage))
        }
    }
}
