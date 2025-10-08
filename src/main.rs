
use nlab_listary_demo::LOCAL_PATH;
use nlab_listary_demo::browser::open_url;
use nlab_listary_demo::git_ops::update_local_repository;
use nlab_listary_demo::parser::index_local_files;
use std::error::Error;
use std::path::Path;

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
            println!("URL: {}", page.url);
            // 只打印内容的前 100 个字符
            println!(
                "内容片段: {}...",
                page.content.chars().take(100).collect::<String>()
            );
            
            // 测试 open_url 函数
            println!("正在浏览器中打开: {}", page.url);
            match open_url(&page.url) {
                Ok(()) => println!("✓ 成功打开页面"),
                Err(e) => eprintln!("✗ 打开页面失败: {}", e),
            }
            
            println!("---");
            
            // 添加短暂延迟，避免同时打开太多标签页
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    Ok(())
}
