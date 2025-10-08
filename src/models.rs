// 定义一个结构体来存储提取到的数据
#[derive(Debug)]
pub struct NLabPage {
    /// 文件相对于仓库根目录的路径
    pub file_path: String,
    pub url: String,
    /// 页面标题
    pub title: String,
    /// 页面的文本内容（已清理格式）
    pub content: String,
}
