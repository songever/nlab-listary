
use nlab_listary_demo::LOCAL_PATH;
use nlab_listary_demo::git_ops::update_local_repository;
use nlab_listary_demo::parser::index_local_files;
use nlab_listary_demo::storage::Storage;
use nlab_listary_demo::search::*;
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
        storage.save_pages_batch(&indexed_data)?;
        println!("✓ 成功存储 {} 个页面到数据库", indexed_data.len());

        // 初始化搜索引擎并构建索引
        println!("\n正在初始化搜索引擎...");
        let index_dir = Path::new("tantivy_index");
        let mut search_engine = TantivySearch::new(index_dir)?;
        
        println!("正在构建搜索索引...");
        search_engine.build_index(&indexed_data.clone())?;
        println!("✓ 成功构建搜索索引");

        // 验证搜索功能
        println!("\n--- 搜索功能验证 ---");
        
        // 测试 1: 基本搜索
        println!("\n测试 1: 搜索 'category'");
        let results = search_engine.search("category", 5)?;
        println!("找到 {} 个结果:", results.len());
        for (i, result) in results.iter().enumerate() {
            println!("  {}. {} (分数: {:.2})", i + 1, result.title, result.score);
            println!("     内容: {}...", result.content.chars().take(80).collect::<String>());
        }

        // 测试 2: 仅标题搜索
        println!("\n测试 2: 仅在标题中搜索 'theory'");
        let title_results = search_engine.search_with_filters(
            "theory",
            5,
            SearchFilters::new(true, 0.5),
        )?;
        println!("找到 {} 个结果:", title_results.len());
        for (i, result) in title_results.iter().enumerate() {
            println!("  {}. {} (分数: {:.2})", i + 1, result.title, result.score);
        }

        // 测试 3: 带最小分数过滤
        println!("\n测试 3: 搜索 'mathematics' (最小分数 1.0)");
        let filtered_results = search_engine.search_with_filters(
            "mathematics",
            10,
            SearchFilters::new(false, 1.0),
        )?;
        println!("找到 {} 个高质量结果:", filtered_results.len());
        for (i, result) in filtered_results.iter().enumerate() {
            println!("  {}. {} (分数: {:.2})", i + 1, result.title, result.score);
        }

        // 数据库验证（保留原有逻辑）
        println!("\n--- 数据库验证 (前 3 条) ---");
        for page in indexed_data.iter().take(3) {
            println!("文件: {}", page.file_path);
            println!("标题: {}", page.title);
            
            match storage.get_page(&page.id)? {
                Some(retrieved_page) => {
                    println!("✓ 从数据库成功读取");
                    assert_eq!(retrieved_page.title, page.title);
                }
                None => {
                    eprintln!("✗ 从数据库读取失败");
                }
            }
            println!("---");
        }

        println!("\n✓ 所有操作完成！");
        println!("数据库位置: nlab_data.db");
        println!("索引位置: tantivy_index/");
    }
    
    Ok(())
}
