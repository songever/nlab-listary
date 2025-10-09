use thiserror::Error;
use crate::models::NLabPage;
use std::{fmt::Display, path::Path};

trait SearchEngine {
    fn new(index_dir: impl AsRef<Path>) -> Result<Self, SearchError>
    where
        Self: Sized;
    // 索引构建（初始化时调用）
    fn build_index(docs: Vec<NLabPage>) -> Result<(), SearchError>;

    // 增量更新（同步时调用）
    fn update_page(&mut self, page: &NLabPage) -> Result<(), SearchError>;
    fn delete_page(&mut self, page_id: &str) -> Result<(), SearchError>;

    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, SearchError>;
    fn search_with_filters(
        &self,
        query: &str,
        limit: usize,
        filters: SearchFilters,
    ) -> Result<Vec<SearchResult>, SearchError>;
}

#[derive(Debug)]
struct SearchResult {
    id: String,
    score: f32,
    title: String,
    content: String,
}

#[derive(Error, Debug)]
enum SearchError {
    Io(std::io::Error),
    Tantivy(tantivy::TantivyError),
    Other(String),
}

impl Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::Io(e) => write!(f, "IO Error: {}", e),
            SearchError::Tantivy(e) => write!(f, "Tantivy Error: {}", e),
            SearchError::Other(msg) => write!(f, "Other Error: {}", msg),
        }
    }
}

struct TantivySearch {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
}

struct SearchFilters {
    title_only: bool,
    min_score: f32,
}

