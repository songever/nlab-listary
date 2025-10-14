use bincode::{Decode, Encode};

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct SearchIndex {
    pub title: String,
    pub url: String,
}

// 定义一个结构体来存储提取到的数据
#[derive(Debug, Encode, Decode, Clone)]
pub struct NLabPage {
    pub id: String,
    /// 页面标题
    pub title: String,
    /// 文件相对于仓库根目录的路径
    pub file_path: String,

    pub url: String,

    /// 页面的文本内容（已清理格式）
    pub content: String,
}

impl NLabPage {
    /// 从文件路径创建 ID
    pub fn new(file_path: String, title: String, url: String, content: String) -> Self {
        Self {
            id: file_path.clone(),
            title,
            file_path,
            url,
            content,
        }
    }

    pub fn id_from_url(url: &str) -> Option<String> {
        url.split("/show/").nth(1).map(|s| s.to_string())
    }
}
