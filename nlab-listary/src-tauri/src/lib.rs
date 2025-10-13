
use tauri::State;
use crate::{git_ops::update_local_repository, models::SearchIndex, search::SearchEngine};
use crate::parser::{index_local_files, parse_html_file};

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

pub struct AppState {
    search_engine: search::TantivySearch,
    storage: storage::Storage,
}

impl AppState {
    pub fn new(search_engine: search::TantivySearch, storage: storage::Storage) -> Self {
        Self {
            search_engine,
            storage,
        }
    }
}

#[tauri::command]
fn get_search_results(state: State<AppState>, query: String) -> Result<Vec<SearchIndex>, String> {
    let results = state
        .search_engine
        .search(&query, 10)
        .map_err(|e| format!("搜索失败: {}", e))?;
    
    let search_results = results
        .into_iter()
        .filter_map(|res| {
            state.storage
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
            open_url,
            get_search_results
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
        Ok(AppState::new(search_engine, storage))
    }
}
