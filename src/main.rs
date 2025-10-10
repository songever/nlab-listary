
use nlab_listary_demo::LOCAL_PATH;
use nlab_listary_demo::browser::open_url;
use nlab_listary_demo::git_ops::update_local_repository;
use nlab_listary_demo::parser::index_local_files;
use nlab_listary_demo::storage::Storage;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let path = Path::new(LOCAL_PATH);
    let _repo = update_local_repository(path)?;

    if path.exists() {
        println!("正在解析本地文件...");
        let indexed_data = index_local_files(path)?;
        println!("✓ 成功解析 {} 个页面", indexed_data.len());

        // 创建或打开 sled 数据库
        println!("\n正在初始化数据库...");
        let storage = Storage::new("nlab_data.db")?;
        
        // 批量存储到数据库
        println!("正在存储数据到数据库...");
        storage.save_pages_batch(indexed_data.clone())?;
        println!("✓ 成功存储 {} 个页面到数据库", indexed_data.len());

        // 验证存储：从数据库读取前 3 个页面
        println!("\n--- 数据库验证 (前 3 条) ---");
        for page in indexed_data.iter().take(3) {
            println!("文件: {}", page.file_path);
            println!("标题: {}", page.title);
            println!("URL: {}", page.url);
            println!(
                "内容片段: {}...",
                page.content.chars().take(100).collect::<String>()
            );

            // 从数据库读取验证
            match storage.get_page(&page.id)? {
                Some(retrieved_page) => {
                    println!("✓ 从数据库成功读取");
                    assert_eq!(retrieved_page.title, page.title);
                    assert_eq!(retrieved_page.content, page.content);
                }
                None => {
                    eprintln!("✗ 从数据库读取失败");
                }
            }

            println!("---");
        }

        // 可选：测试打开浏览器（注释掉以避免打开太多标签页）
        // println!("\n正在浏览器中打开第一个页面...");
        // if let Some(first_page) = indexed_data.first() {
        //     match open_url(&first_page.url) {
        //         Ok(()) => println!("✓ 成功打开页面"),
        //         Err(e) => eprintln!("✗ 打开页面失败: {}", e),
        //     }
        // }

        println!("\n✓ 所有操作完成！");
        println!("数据库位置: nlab_data.db");
    }
    
    Ok(())
}
