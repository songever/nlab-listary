use git2::build::CheckoutBuilder;
use git2::{FetchOptions, RemoteCallbacks};
use git2::{build::RepoBuilder, Repository};
use scraper::{Html, Selector};
use walkdir::WalkDir;
use std::fs;
use std::path::Path;
use std::error::Error;
use std::io::Write;

const REPO_URL: &str = "https://github.com/ncatlab/nlab-content-html.git";
const LOCAL_PATH: &str = "nlab_mirror";

fn main() -> Result<(), Box<dyn Error>> {
    let path = Path::new(LOCAL_PATH);
    let repo = update_local_repository(path)?;

    if path.exists() {
        let indexed_data = index_local_files(path)?;

        // 打印前 3 个页面的摘要作为验证
        println!("\n--- 索引摘要 (前 3 条) ---");
        for page in indexed_data.iter().take(3) {
            println!("文件: {}", page.file_path);
            println!("标题: {}", page.title);
            // 只打印内容的前 100 个字符
            println!("内容片段: {}...", page.content.chars().take(100).collect::<String>());
            println!("---");
        }
    }
    Ok(())
}

fn update_local_repository(path: &Path) -> Result<Repository, git2::Error> {
    if path.exists() {
        println!("本地仓库已存在，正在更新...");
        let repo = Repository::open(path)?;
        
        // --- 1. 执行 FETCH (获取远程最新状态) ---
        {
            let mut remote = repo.find_remote("origin")?;
            let mut fetch_options = FetchOptions::new();
            // 可以复用 clone_with_progress 中的回调函数，这里简化处理
            remote.fetch::<&str>(&[], Some(&mut fetch_options), None)?;
        }
        
        // --- 2. 获取 FETCH_HEAD 并分析合并类型 ---
        let (analysis, oid) = {
            let fetch_head = repo.find_reference("FETCH_HEAD")?;
            let oid = fetch_head.target().unwrap();
            let remote_commit = repo.find_annotated_commit(oid)?;
            (repo.merge_analysis(&[&remote_commit])?, oid)
        };

        if analysis.0.is_up_to_date() {
            println!("本地仓库已是最新版本。");
        } else if analysis.0.is_fast_forward() {
            println!("正在执行快进合并...");

            // 4. 执行快进合并
            let reference_name = "refs/heads/main";
            let mut reference = repo.find_reference(reference_name)?;

            // 更新本地分支引用到远程最新的 OID
            reference.set_target(oid, "Fast-Forward Merge")?;

            // 移动 HEAD 指针
            repo.set_head(reference_name)?;

            // 检出新的提交内容到工作目录
            let mut checkout_builder = CheckoutBuilder::new();
            checkout_builder.recreate_missing(true).force();
            repo.checkout_head(Some(&mut checkout_builder))?;

            println!("更新完成。");

        } else if analysis.0.is_normal() {
             // 如果是普通合并（需要产生一个新的合并提交），逻辑会复杂得多
            println!("发现需要普通合并的情况，请手动处理或使用更复杂的合并逻辑。");
        } else {
            println!("发现复杂或不可处理的 Git 状态。");
        }

        Ok(repo)
    } else {
        // --- 路径不存在：执行克隆 (Clone) 操作 ---
        println!("本地仓库不存在，正在克隆...");
        let repo = clone_with_progress(REPO_URL, path)?;
        println!("克隆完成。");
        Ok(repo)
    }
}

fn clone_with_progress(url: &str, path: &Path) -> Result<Repository, git2::Error> {
    let mut callbacks = RemoteCallbacks::new();
    let mut last_printed = 0;
    callbacks.transfer_progress(|stats| {
        let received = stats.received_objects();
        if stats.received_objects() == stats.total_objects() {
            // 使用 \r 可以在同一行更新进度，提高用户体验
            print!("\r接收完成：{}/{}", stats.received_objects(), stats.total_objects());
        } else if received - last_printed >= 1000 || received == 1 {
            print!("\r接收中：{}/{} ({})", stats.received_objects(), stats.total_objects(), stats.indexed_deltas());
            last_printed = received;
        }
        std::io::stdout().flush().unwrap();
        true
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut checkout_options = CheckoutBuilder::new();
    let mut checkout_last_printed = 0;
    checkout_options.progress(|_path, completed_steps, total_steps| {
        // ... (进度打印逻辑) ...
        if total_steps > 0 && (completed_steps - checkout_last_printed >= 1000 || completed_steps == total_steps) {
            print!("\r检出：{}/{}", completed_steps, total_steps);
            checkout_last_printed = completed_steps;
            std::io::stdout().flush().unwrap();
        }
    }).force(); // 强制检出以覆盖文件

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);
    builder.with_checkout(checkout_options);

    builder.clone(url, path)
}

// 定义一个结构体来存储提取到的数据
#[derive(Debug)]
struct NLabPage {
    file_path: String,
    title: String,
    content: String,
}

fn index_local_files(repo_path: &Path) -> Result<Vec<NLabPage>, Box<dyn Error>> {
    println!("\n--- 开始遍历和解析本地文件 ---");
    let mut pages: Vec<NLabPage> = Vec::new();
    let mut parsed_count = 0;

    // 1. 定义我们需要的 CSS 选择器
    // 注意：Selector::parse() 应该只被调用一次，以避免重复工作
    let title_selector = Selector::parse("h1").unwrap();
    let content_selector = Selector::parse("#content").unwrap();

    // 2. 使用 WalkDir 遍历目录
    for entry in WalkDir::new(repo_path) {
        let entry = entry?;
        let path = entry.path();

        // 3. 过滤出 .html 文件
        if path.is_file() && path.extension().map_or(false, |ext| ext == "html") {
            // 获取文件名作为页面的唯一标识（例如：HomePage.html）
            let file_name = path.file_name().unwrap().to_string_lossy().to_string();

            // 4. 读取文件内容
            let html_content = fs::read_to_string(path)?;

            // 5. 使用 scraper 解析 HTML
            let document = Html::parse_document(&html_content);
            
            // 6. 提取数据
            
            // 提取标题：找到第一个 h1 标签
            let title = document
                .select(&title_selector)
                .next()
                .map(|h1| h1.text().collect::<String>()) // 提取标签内的文本
                .unwrap_or_else(|| "Untitled Page".to_string()); // 提供默认值以防找不到

            // 提取内容：找到 #content 区域
            let content_div = document
                .select(&content_selector)
                .next();

            let content = if let Some(div) = content_div {
                // 提取整个 content div 中的所有文本，并去除多余空白
                div.text().collect::<String>().trim().to_string()
            } else {
                eprintln!("警告：文件 {} 未找到核心内容。", file_name);
                "".to_string()
            };

            // 7. 存储结果
            pages.push(NLabPage {
                file_path: file_name,
                title,
                content,
            });
            parsed_count += 1;
        }
    }

    println!("--- 解析完成！共处理 {} 个文件。 ---", parsed_count);
    Ok(pages)
}
