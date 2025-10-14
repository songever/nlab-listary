use std::sync::{Arc, Mutex, RwLock};

use crate::parser::index_local_files;
use crate::{git_ops::update_local_repository, models::SearchIndex, search::SearchEngine};
use tauri::State;

pub const REPO_URL: &str = "https://github.com/ncatlab/nlab-content-html.git";
pub const GIT_REPO_PATH: &str = "nlab_mirror";
pub const DB_PATH: &str = "nlab_page_data.db";
pub const INDEX_PATH: &str = "nlab_pagr_index";

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

#[cfg(ignore)]
#[tauri::command]
fn sync_local_repo(state: State<AppState>) -> Result<(), String> {
    use std::path::Path;
    let path = Path::new(GIT_REPO_PATH);
    let mut state = state.lock().unwrap();

    update_local_repository(path).map_err(|e| format!("Syncronizing local repo failed: {}", e))?;

    let pages = index_local_files(path).map_err(|e| format!("Parsing htmls failed: {}", e))?;

    state
        .storage
        .save_pages_batch(&pages)
        .map_err(|e| format!("Saving pages to storage failed: {}", e))?;

    pages
        .iter()
        .try_for_each(|page| state.search_engine.update_page(page))
        .map_err(|e| format!("Building search index failed: {}", e))?;
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    use browser::open_url;
    open_url(&url).map_err(|e| format!("打开URL失败: {}", e))
}

#[tauri::command]
fn is_ready(state: State<AppState>) -> Result<bool, String> {
    let state = state
        .read()
        .map_err(|e| format!("failed to lock state: {}", e))?;
    Ok(state.search_engine.is_some() && state.storage.is_some())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
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
            is_ready
        ])
        .setup(move |_app| {
            std::thread::spawn(move || {
                eprintln!("initializing ...");
                match initialize_components() {
                    Ok((search_engine, storage)) => {
                        let mut state = state_clone.write().unwrap();
                        state.search_engine = Some(search_engine);
                        state.storage = Some(storage);
                        eprintln!("initialized successfully");
                    }
                    Err(e) => {
                        eprintln!("failed to initialize app state: {}", e);
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn initialize_components(
) -> Result<(search::TantivySearch, storage::Storage), Box<dyn std::error::Error>> {
    use std::path::Path;
    let path = Path::new(GIT_REPO_PATH);
    let _repo = update_local_repository(path)?;

    if !path.exists() {
        Err("local repo should exist after update".into())
    } else {
        let pages = index_local_files(path)?;

        // 1. 初始化存储
        let storage_path = Path::new(DB_PATH).join("storage");
        let storage = storage::Storage::new(storage_path.to_str().unwrap())?;
        storage.save_pages_batch(&pages)?;

        // 2. 初始化搜索引擎
        let index_path = Path::new(INDEX_PATH).join("index");
        let mut search_engine = search::TantivySearch::new(index_path.to_str().unwrap())?;
        search_engine.build_index(&pages)?;

        Ok((search_engine, storage))
    }
}

#[cfg(ignore)]
fn initialize_app_state() -> Result<AppState, Box<dyn std::error::Error>> {
    use std::path::Path;
    let path = Path::new(GIT_REPO_PATH);
    let _repo = update_local_repository(path)?;

    if !path.exists() {
        Err("local repo should ".into())
    } else {
        let pages = index_local_files(path)?;

        // 1. 初始化存储
        let storage_path = Path::new(DB_PATH).join("storage");
        let storage = storage::Storage::new(storage_path.to_str().unwrap())?;
        storage.save_pages_batch(&pages)?;

        // 2. 初始化搜索引擎
        let index_path = Path::new(INDEX_PATH).join("index");
        let mut search_engine = search::TantivySearch::new(index_path.to_str().unwrap())?;
        search_engine.build_index(&pages)?;
        Ok(Mutex::new(AppStateInner::new(search_engine, storage)))
    }
}
