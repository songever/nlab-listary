use std::sync::Mutex;

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
    search_engine: search::TantivySearch,
    storage: storage::Storage,
}
type AppState = Mutex<AppStateInner>;

impl AppStateInner {
    pub fn new(search_engine: search::TantivySearch, storage: storage::Storage) -> Self {
        Self {
            search_engine,
            storage,
        }
    }
}

#[tauri::command]
fn get_search_results(state: State<AppState>, query: String) -> Result<Vec<SearchIndex>, String> {
    let state = state.lock().unwrap();

    let results = state
        .search_engine
        .search(&query, 10)
        .map_err(|e| format!("搜索失败: {}", e))?;

    let search_results = results
        .into_iter()
        .filter_map(|res| {
            state
                .storage
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = match initialize_app_state() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("初始化应用状态失败: {}", e);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            sync_local_repo,
            get_search_results,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

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
