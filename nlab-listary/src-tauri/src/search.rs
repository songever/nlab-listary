
use crate::models::NLabPage;
use std::path::Path;
use tantivy::schema::Value;
use tantivy::{doc, query::QueryParser, IndexWriter, TantivyDocument};
use thiserror::Error;

pub trait SearchEngine {
    // 创建或打开索引
    fn new(index_dir: impl AsRef<Path>) -> Result<Self, SearchError>
    where
        Self: Sized;

    // 构建索引（实例方法）
    fn build_index(&mut self, docs: &[NLabPage]) -> Result<(), SearchError>;

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
    score: f32,
    pub title: String,
    content: String,
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

#[derive(Clone)]
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

// 辅助方法：创建 schema
fn create_schema() -> tantivy::schema::Schema {
    let mut schema_builder = tantivy::schema::Schema::builder();
    // 使用 STRING 而不是 TEXT，因为 id 需要精确匹配，不需要分词
    schema_builder.add_text_field("id", tantivy::schema::STRING | tantivy::schema::STORED);
    schema_builder.add_text_field("title", tantivy::schema::TEXT | tantivy::schema::STORED);
    schema_builder.add_text_field("content", tantivy::schema::TEXT | tantivy::schema::STORED);
    schema_builder.build()
}

impl SearchEngine for TantivySearch {
    fn new(index_dir: impl AsRef<Path>) -> Result<Self, SearchError> {
        let index_path = index_dir.as_ref();

        // 如果索引不存在，创建新索引
        let index = if index_path.exists() {
            tantivy::Index::open_in_dir(index_path)?
        } else {
            std::fs::create_dir_all(index_path)?;
            tantivy::Index::create_in_dir(index_path, create_schema())?
        };

        let reader = index.reader()?;
        Ok(TantivySearch { index, reader })
    }

    fn build_index(&mut self, docs: &[NLabPage]) -> Result<(), SearchError> {
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
            let id = retrieved_doc
                .get_first(page_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = retrieved_doc
                .get_first(page_title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = retrieved_doc
                .get_first(page_content)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                id,
                score,
                title,
                content,
            });
        }

        Ok(results)
    }
}

impl TantivySearch {
    pub fn update_pages_batch(&mut self, pages: &[NLabPage]) -> Result<(), SearchError> {
        let schema = self.index.schema();
        let page_id = schema.get_field("id").unwrap();
        let page_title = schema.get_field("title").unwrap();
        let page_content = schema.get_field("content").unwrap();

        let mut writer = self.index.writer(50_000_000)?;
        
        println!("Starting batch update for {} pages", pages.len());
        
        for page in pages {
            println!("Deleting page with id: {}", page.id);
            let page_id_term = tantivy::Term::from_field_text(page_id, &page.id);
            writer.delete_term(page_id_term.clone());
            
            println!("Adding page: {} (id: {})", page.title, page.id);
            writer.add_document(doc!(
                page_id => page.id.clone(),
                page_title => page.title.clone(),
                page_content => page.content.clone(),
            ))?;
        }
        
        println!("Committing changes...");
        writer.commit()?;

        println!("Reloading reader...");
        self.reader.reload()?;
        
        println!("Batch update completed successfully");
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_page(id: &str, title: &str, content: &str) -> NLabPage {
        NLabPage {
            id: id.to_string(),
            title: title.to_string(),
            content: content.to_string(),
            file_path: format!("/test/path/{}.html", id),
            url: format!("https://example.com/{}", id),
        }
    }

    // 辅助函数：创建测试用的搜索引擎
    fn create_test_search_engine() -> (TantivySearch, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path();
        
        // 确保目录存在并创建新索引
        std::fs::create_dir_all(index_path).unwrap();
        let index = tantivy::Index::create_in_dir(index_path, create_schema()).unwrap();
        let reader = index.reader().unwrap();
        
        let search_engine = TantivySearch { index, reader };
        (search_engine, temp_dir)
    }

    #[test]
    fn test_update_pages_batch_delete_and_add() {
        // 创建临时目录用于测试索引
        let (mut search_engine, _temp_dir) = create_test_search_engine();

        // 1. 首先添加一些初始页面
        let initial_pages = vec![
            create_test_page("page1", "First Page", "This is the first page content"),
            create_test_page("page2", "Second Page", "This is the second page content"),
            create_test_page("page3", "Third Page", "This is the third page content"),
        ];

        search_engine.build_index(&initial_pages).unwrap();

        // 验证初始页面已添加
        let results = search_engine.search("first", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "page1");
        println!("✓ Initial pages added successfully");

        // 2. 测试批量更新：删除旧内容并添加新内容
        let updated_pages = vec![
            create_test_page("page1", "Updated First Page", "This is the updated first page with new content"),
            create_test_page("page2", "Updated Second Page", "This is the updated second page with new content"),
        ];

        println!("\n--- Testing batch update (delete + add) ---");
        search_engine.update_pages_batch(&updated_pages).unwrap();

        // 3. 验证更新后的内容
        // 搜索新内容应该能找到
        let new_results = search_engine.search("updated first", 10).unwrap();
        assert!(new_results.len() >= 1, "Should find updated content");
        
        // 验证找到的是新内容而不是旧内容
        let page1_result = new_results.iter().find(|r| r.id == "page1");
        assert!(page1_result.is_some(), "Should find page1");
        assert_eq!(page1_result.unwrap().title, "Updated First Page");
        println!("✓ New content added successfully");

        // 验证第二个更新的页面
        let second_results = search_engine.search("updated second", 10).unwrap();
        assert!(second_results.len() >= 1, "Should find updated second page");
        let page2_result = second_results.iter().find(|r| r.id == "page2");
        assert!(page2_result.is_some(), "Should find page2");
        assert_eq!(page2_result.unwrap().title, "Updated Second Page");
        println!("✓ Second page updated successfully");

        // 4. 验证未更新的页面仍然存在
        let third_results = search_engine.search("third", 10).unwrap();
        assert_eq!(third_results.len(), 1);
        assert_eq!(third_results[0].id, "page3");
        assert_eq!(third_results[0].title, "Third Page");
        println!("✓ Untouched page still exists");

        // 4.5 验证更新后page1的搜索结果只有一个
        println!("\n--- Verifying no duplicate pages after update ---");
        let page1_results = search_engine.search("first", 10).unwrap();
        let page1_matches: Vec<_> = page1_results.iter().filter(|r| r.id == "page1").collect();
        assert_eq!(page1_matches.len(), 1, "Page1 should appear exactly once after update, found {} times", page1_matches.len());
        assert_eq!(page1_matches[0].title, "Updated First Page", "Should be the updated version");
        println!("✓ Page1 appears exactly once with updated content");
        
        // 验证page2也只有一个结果
        let page2_results = search_engine.search("second", 10).unwrap();
        let page2_matches: Vec<_> = page2_results.iter().filter(|r| r.id == "page2").collect();
        assert_eq!(page2_matches.len(), 1, "Page2 should appear exactly once after update, found {} times", page2_matches.len());
        assert_eq!(page2_matches[0].title, "Updated Second Page", "Should be the updated version");
        println!("✓ Page2 appears exactly once with updated content");

        // 5. 测试添加新页面
        let new_pages = vec![
            create_test_page("page4", "Fourth Page", "This is a brand new page"),
        ];

        println!("\n--- Testing adding new page via batch update ---");
        search_engine.update_pages_batch(&new_pages).unwrap();

        let fourth_results = search_engine.search("brand new", 10).unwrap();
        assert!(fourth_results.len() >= 1, "Should find new page");
        let page4_result = fourth_results.iter().find(|r| r.id == "page4");
        assert!(page4_result.is_some(), "Should find page4");
        println!("✓ New page added successfully");

        // 6. 通过ID搜索验证更新确实生效
        println!("\n--- Verifying updates by searching for specific content ---");
        
        // 搜索只有新内容才有的词
        let unique_new_content = search_engine.search("updated", 10).unwrap();
        println!("Found {} results for 'updated'", unique_new_content.len());
        assert!(unique_new_content.len() >= 2, "Should find at least 2 updated pages");
        
        // 验证这些结果包含正确的页面
        let has_page1 = unique_new_content.iter().any(|r| r.id == "page1" && r.title == "Updated First Page");
        let has_page2 = unique_new_content.iter().any(|r| r.id == "page2" && r.title == "Updated Second Page");
        assert!(has_page1, "Should find updated page1");
        assert!(has_page2, "Should find updated page2");
        println!("✓ Both updated pages verified with correct titles");

        println!("\n=== All tests passed! ===");
    }

    #[test]
    fn test_update_pages_batch_empty() {
        let (mut search_engine, _temp_dir) = create_test_search_engine();

        // 测试空批量更新
        let empty_pages: Vec<NLabPage> = vec![];
        let result = search_engine.update_pages_batch(&empty_pages);
        assert!(result.is_ok());
        println!("✓ Empty batch update handled correctly");
    }
}
