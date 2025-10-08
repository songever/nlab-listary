use nlab_listary_demo::git_ops::update_local_repository;
use nlab_listary_demo::parser::index_local_files;
use std::error::Error;
use std::path::Path;
use nlab_listary_demo::{LOCAL_PATH};

fn main() -> Result<(), Box<dyn Error>> {
    let path = Path::new(LOCAL_PATH);
    let _repo = update_local_repository(path)?;

    if path.exists() {
        let indexed_data = index_local_files(path)?;

        // 打印前 3 个页面的摘要作为验证
        println!("\n--- 索引摘要 (前 3 条) ---");
        for page in indexed_data.iter().take(3) {
            println!("文件: {}", page.file_path);
            println!("标题: {}", page.title);
            // 只打印内容的前 100 个字符
            println!(
                "内容片段: {}...",
                page.content.chars().take(100).collect::<String>()
            );
            println!("---");
        }
    }
    Ok(())
}
