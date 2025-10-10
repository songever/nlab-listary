
use thiserror::Error;
use crate::models::NLabPage;
use std::{path::Path};
use tantivy::{doc, query::QueryParser, IndexWriter, TantivyDocument};
use tantivy::schema::Value;

pub trait SearchEngine {
    // 创建或打开索引
    fn new(index_dir: impl AsRef<Path>) -> Result<Self, SearchError>
    where
        Self: Sized;
    
    // 构建索引（实例方法）
    fn build_index(&mut self, docs: Vec<NLabPage>) -> Result<(), SearchError>;

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
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub title: String,
    pub content: String,
}

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Tantivy error: {0}")]
    TantivyError(#[from] tantivy::TantivyError),
    
    #[error("Query parsing error: {0}")]
    QueryParseError(#[from] tantivy::query::QueryParserError),
}

pub struct TantivySearch {
    index: tantivy::Index,
    reader: tantivy::IndexReader,
}

pub struct SearchFilters {
    pub title_only: bool,
    pub min_score: f32,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            title_only: false,
            min_score: 0.0,
        }
    }
}

impl SearchFilters {
    pub fn new(title_only: bool, min_score: f32) -> Self {
        Self { title_only, min_score }
    }
}

impl TantivySearch {
    // 辅助方法：创建 schema
    fn create_schema() -> tantivy::schema::Schema {
        let mut schema_builder = tantivy::schema::Schema::builder();
        schema_builder.add_text_field("id", tantivy::schema::STORED);
        schema_builder.add_text_field("title", tantivy::schema::TEXT | tantivy::schema::STORED);
        schema_builder.add_text_field("content", tantivy::schema::TEXT | tantivy::schema::STORED);
        schema_builder.build()
    }
}

impl SearchEngine for TantivySearch {
    fn new(index_dir: impl AsRef<Path>) -> Result<Self, SearchError> {
        let index_path = index_dir.as_ref();
        
        // 如果索引不存在，创建新索引
        let index = if index_path.exists() {
            tantivy::Index::open_in_dir(index_path)?
        } else {
            std::fs::create_dir_all(index_path)?;
            tantivy::Index::create_in_dir(index_path, Self::create_schema())?
        };
        
        let reader = index.reader()?;
        Ok(TantivySearch { index, reader })
    }

    fn build_index(&mut self, docs: Vec<NLabPage>) -> Result<(), SearchError> {
        let schema = self.index.schema();
        let page_id = schema.get_field("id").unwrap();
        let page_title = schema.get_field("title").unwrap();
        let page_content = schema.get_field("content").unwrap();

        let mut writer = self.index.writer(50_000_000)?;

        for doc in docs {
            writer.add_document(doc!(
                page_id => doc.id,
                page_title => doc.title,
                page_content => doc.content,
            ))?;
        }
        
        writer.commit()?;
        
        // 重新加载 reader 以看到新数据
        self.reader.reload()?;

        Ok(())
    }

    fn update_page(&mut self, page: &NLabPage) -> Result<(), SearchError> {
        let schema = self.index.schema();
        let page_id = schema.get_field("id").unwrap();
        let page_title = schema.get_field("title").unwrap();
        let page_content = schema.get_field("content").unwrap();

        let mut writer = self.index.writer(50_000_000)?;
        writer.delete_term(tantivy::Term::from_field_text(page_id, &page.id));
        writer.add_document(doc!(
            page_id => page.id.clone(),
            page_title => page.title.clone(),
            page_content => page.content.clone(),
        ))?;
        writer.commit()?;
        
        self.reader.reload()?;
        Ok(())
    }
    
    fn delete_page(&mut self, page_id: &str) -> Result<(), SearchError> {
        let schema = self.index.schema();
        let page_id_field = schema.get_field("id").unwrap();

        let mut writer: IndexWriter<TantivyDocument> = self.index.writer(50_000_000)?;
        writer.delete_term(tantivy::Term::from_field_text(page_id_field, page_id));
        writer.commit()?;
        
        self.reader.reload()?;
        Ok(())
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, SearchError> {
        self.search_with_filters(query, limit, SearchFilters::default())
    }
    
    fn search_with_filters(
        &self,
        query: &str,
        limit: usize,
        filters: SearchFilters,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let schema = self.index.schema();
        let page_id = schema.get_field("id").unwrap();
        let page_title = schema.get_field("title").unwrap();
        let page_content = schema.get_field("content").unwrap();

        let searcher = self.reader.searcher();
        let query_parser = if filters.title_only {
            QueryParser::for_index(&self.index, vec![page_title])
        } else {
            QueryParser::for_index(&self.index, vec![page_title, page_content])
        };
        let query = query_parser.parse_query(query)?;

        let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            if score < filters.min_score {
                continue;
            }
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let id = retrieved_doc.get_first(page_id).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let title = retrieved_doc.get_first(page_title).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let content = retrieved_doc.get_first(page_content).and_then(|v| v.as_str()).unwrap_or("").to_string();

            results.push(SearchResult { id, score, title, content });
        }

        Ok(results)
    }
}
