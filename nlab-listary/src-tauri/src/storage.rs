use crate::{models::NLabPage, parser};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sled::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::error::EncodeError),

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] bincode::error::DecodeError),

    #[error("Page size exceeds limit: {actual} bytes (max: {max} bytes)")]
    PageSizeExceeded { actual: usize, max: usize },

    #[error("Page not found: {0}")]
    PageNotFound(String),

    #[error("Invalid metadata key: {0}")]
    InvalidMetadataKey(String),

    #[error("Parser error: {0}")]
    ParserError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// 添加从 Box<dyn Error> 的转换
impl From<parser::ParseHtmlError> for StorageError {
    fn from(err: parser::ParseHtmlError) -> Self {
        StorageError::ParserError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;

pub struct Storage {
    db: sled::Db,
}

const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

impl Storage {
    pub fn new(path: &str) -> Result<Self> {
        let db: sled::Db = sled::open(path)?;
        Ok(Self { db })
    }

    // 页面元数据存储
    // Key: page_id (String)
    // Value: NLabPage (bincode 序列化)
    pub fn save_page(&self, page: &NLabPage) -> Result<()> {
        // 先计算实际大小，避免固定大小数组的浪费
        let serialized: Vec<u8> = bincode::encode_to_vec(page, BINCODE_CONFIG)?;

        // if serialized.len() > NLAB_PAGE_SIZE {
        //     return Err(StorageError::PageSizeExceeded {
        //         actual: serialized.len(),
        //         max: NLAB_PAGE_SIZE,
        //     });
        // }

        self.db.insert(page.id.as_bytes(), serialized)?;
        Ok(())
    }

    pub fn get_page(&self, page_id: &str) -> Result<Option<NLabPage>> {
        match self.db.get(page_id.as_bytes())? {
            Some(bytes) => {
                let (page, _): (NLabPage, usize) =
                    bincode::decode_from_slice(&bytes, BINCODE_CONFIG)?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    // 批量操作（用于初始化和同步）
    pub fn save_pages_batch(&self, pages: &[NLabPage]) -> Result<()> {
        let mut batch = sled::Batch::default();

        for page in pages {
            let serialized: Vec<u8> = bincode::encode_to_vec(&page, BINCODE_CONFIG)?;

            // if serialized.len() > NLAB_PAGE_SIZE {
            //     return Err(StorageError::PageSizeExceeded {
            //         actual: serialized.len(),
            //         max: NLAB_PAGE_SIZE,
            //     });
            // }

            batch.insert(page.id.as_bytes(), serialized);
        }

        self.db.apply_batch(batch)?;
        Ok(())
    }

    // 元数据存储
    // Key: "meta:last_sync", "meta:total_pages" 等
    pub fn set_metadata(&self, key: &str, value: &[u8]) -> Result<()> {
        if !key.starts_with("meta:") {
            return Err(StorageError::InvalidMetadataKey(key.to_string()));
        }
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>> {
        if !key.starts_with("meta:") {
            return Err(StorageError::InvalidMetadataKey(key.to_string()));
        }
        match self.db.get(key.as_bytes())? {
            Some(value) => Ok(Some(value.to_vec())),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::REPO_URL;
    use std::path::Path;
    use std::{fs, u8};
    use tempfile::TempDir;

    const NLAB_PAGE_SIZE: usize = 64 * 1024;

    fn create_test_page() -> NLabPage {
        NLabPage {
            id: "test/page.md".to_string(),
            title: "Test Page".to_string(),
            file_path: "test/page.md".to_string(),
            url: "https://ncatlab.org/nlab/show/test".to_string(),
            content: "This is test content.".to_string(),
        }
    }

    #[test]
    fn test_save_and_get_page() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap())?;

        let page = create_test_page();
        storage.save_page(&page)?;

        let retrieved = storage.get_page(&page.id)?;
        assert!(retrieved.is_some());

        let retrieved_page = retrieved.unwrap();
        assert_eq!(retrieved_page.id, page.id);
        assert_eq!(retrieved_page.title, page.title);
        assert_eq!(retrieved_page.content, page.content);

        Ok(())
    }

    #[test]
    fn test_real_html_parsing_and_storage() -> Result<()> {
        use crate::parser::parse_html_file;

        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap())?;

        // 使用实际的 HTML 文件路径
        let test_html_path = Path::new("nlab_mirror/pages/4/7/4/1/1474/content.html");

        if !test_html_path.is_file() {
            println!("跳过测试：找不到测试文件 {:?}", test_html_path);
            return Ok(());
        }

        // 验证文件可读
        let html_content = fs::read_to_string(test_html_path).expect("Failed to read HTML file");
        assert!(!html_content.is_empty(), "HTML 文件内容为空");

        // 解析真实的 HTML 文件
        let page = parse_html_file(test_html_path, Path::new(REPO_URL))?.unwrap();

        println!("\n=== 解析的页面信息 ===");
        println!("  ID: {}", page.id);
        println!("  标题: {}", page.title);
        println!("  文件路径: {}", page.file_path);
        println!("  URL: {}", page.url);
        println!("  内容长度: {} 字节", page.content.len());
        println!("  内容预览: {}...", page.content);

        // 测试序列化大小
        let serialized: Vec<u8> = bincode::encode_to_vec(&page, BINCODE_CONFIG)?;
        println!(
            "  序列化后大小: {} 字节 ({:.2} KB)",
            serialized.len(),
            serialized.len() as f64 / 1024.0
        );

        // 检查是否超过大小限制
        if serialized.len() > NLAB_PAGE_SIZE {
            println!(
                "  ⚠️  警告：超过大小限制 ({} > {})",
                serialized.len(),
                NLAB_PAGE_SIZE
            );
        }

        // 存储到数据库
        storage.save_page(&page)?;
        println!("  ✓ 成功存储到数据库");

        // 从数据库读取
        let retrieved = storage.get_page(&page.id)?;
        assert!(retrieved.is_some(), "无法从数据库读取页面");

        let retrieved_page = retrieved.unwrap();

        // 验证所有字段完全一致
        assert_eq!(retrieved_page.id, page.id, "ID 不匹配");
        assert_eq!(retrieved_page.title, page.title, "标题不匹配");
        assert_eq!(retrieved_page.file_path, page.file_path, "文件路径不匹配");
        assert_eq!(retrieved_page.url, page.url, "URL 不匹配");
        assert_eq!(retrieved_page.content, page.content, "内容不匹配");

        println!("  ✓ 数据完整性验证通过");
        println!("\n✓ 真实 HTML 文件的存储和恢复测试通过\n");

        Ok(())
    }

    #[test]
    fn test_page_not_found() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap())?;

        let result = storage.get_page("nonexistent")?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_save_pages_batch() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap())?;

        let pages = vec![
            create_test_page(),
            NLabPage {
                id: "test/page2.md".to_string(),
                title: "Test Page 2".to_string(),
                file_path: "test/page2.md".to_string(),
                url: "https://ncatlab.org/nlab/show/test2".to_string(),
                content: "Second test content.".to_string(),
            },
        ];

        storage.save_pages_batch(&pages)?;

        for page in pages {
            let retrieved = storage.get_page(&page.id)?;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().id, page.id);
        }

        Ok(())
    }
}
