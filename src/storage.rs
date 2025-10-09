
use crate::models::NLabPage;
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
}

pub type Result<T> = std::result::Result<T, StorageError>;

pub struct Storage {
    db: sled::Db,
}

const NLAB_PAGE_SIZE: usize = 4 * 1024;
const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

impl Storage {
    pub fn new(path: &str) -> Result<Self> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    // 页面元数据存储
    // Key: page_id (String)
    // Value: NLabPage (bincode 序列化)
    pub fn save_page(&self, page: &NLabPage) -> Result<()> {
        // 先计算实际大小，避免固定大小数组的浪费
        let serialized: Vec<u8> = bincode::encode_to_vec(page, BINCODE_CONFIG)?;
        
        if serialized.len() > NLAB_PAGE_SIZE {
            return Err(StorageError::PageSizeExceeded {
                actual: serialized.len(),
                max: NLAB_PAGE_SIZE,
            });
        }
        
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
    pub fn save_pages_batch(&self, pages: Vec<NLabPage>) -> Result<()> {
        let mut batch = sled::Batch::default();
        
        for page in pages {
            let serialized: Vec<u8> = bincode::encode_to_vec(&page, BINCODE_CONFIG)?;
            
            if serialized.len() > NLAB_PAGE_SIZE {
                return Err(StorageError::PageSizeExceeded {
                    actual: serialized.len(),
                    max: NLAB_PAGE_SIZE,
                });
            }
            
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
    use tempfile::TempDir;

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
        
        storage.save_pages_batch(pages.clone())?;
        
        for page in pages {
            let retrieved = storage.get_page(&page.id)?;
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().id, page.id);
        }
        
        Ok(())
    }

    #[test]
    fn test_metadata_operations() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap())?;
        
        let key = "meta:last_sync";
        let value = b"2024-01-01";
        
        storage.set_metadata(key, value)?;
        
        let retrieved = storage.get_metadata(key)?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), value);
        
        Ok(())
    }

    #[test]
    fn test_invalid_metadata_key() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        let result = storage.set_metadata("invalid_key", b"value");
        assert!(matches!(result, Err(StorageError::InvalidMetadataKey(_))));
    }

    #[test]
    fn test_page_size_exceeded() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        let large_page = NLabPage {
            id: "large.md".to_string(),
            title: "Large Page".to_string(),
            file_path: "large.md".to_string(),
            url: "https://ncatlab.org/nlab/show/large".to_string(),
            content: "x".repeat(NLAB_PAGE_SIZE + 1000),
        };
        
        let result = storage.save_page(&large_page);
        assert!(matches!(result, Err(StorageError::PageSizeExceeded { .. })));
    }
}
