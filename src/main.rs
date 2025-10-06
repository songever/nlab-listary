
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
    let _repo = update_local_repository(path)?;

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
        
        // 1. 执行 FETCH (获取远程最新状态)
        {
            let mut remote = repo.find_remote("origin")?;
            
            // 为 fetch 添加 sideband_progress 回调
            let mut callbacks = RemoteCallbacks::new();
            callbacks.sideband_progress(|data| {
                print!("\r远程: {}", String::from_utf8_lossy(data));
                std::io::stdout().flush().unwrap();
                true
            });

            let mut fetch_options = FetchOptions::new();
            fetch_options.remote_callbacks(callbacks);

            // 使用带有回调的 fetch_options
            remote.fetch::<&str>(&[], Some(&mut fetch_options), None)?;
            println!(); // fetch 进度打印完成后换行
        }
        
        // 2. 获取 FETCH_HEAD 并分析合并类型
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

            // 3. 执行快进合并
            // 获取 HEAD 指向的引用 (例如 refs/heads/main)
            let mut reference = repo.head()?.resolve()?;
            
            // 获取该引用的名称，用于后续操作
            let ref_name = reference.name().ok_or_else(|| 
                git2::Error::new(git2::ErrorCode::InvalidSpec, git2::ErrorClass::Reference, "无法获取 HEAD 引用的名称")
            )?.to_string();

            println!("正在快进本地引用: {}", ref_name);

            // 将该引用直接指向 fetch 下来的 commit (oid)
            reference.set_target(oid, "Fast-Forward")?;

            // 更新 HEAD 指向，并检出工作目录以匹配
            repo.set_head(&ref_name)?;
            repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

            println!("更新完成。");

        } else if analysis.0.is_normal() {
            // 如果是普通合并（需要产生一个新的合并提交），逻辑会复杂得多
            println!("发现需要普通合并的情况，请手动处理或使用更复杂的合并逻辑。");
        } else {
            println!("发现复杂或不可处理的 Git 状态。");
        }

        Ok(repo)
    } else {
        // 路径不存在：执行克隆 (Clone) 操作
        println!("本地仓库不存在，正在克隆...");
        let repo = clone_with_progress(REPO_URL, path)?;
        println!("克隆完成。");
        Ok(repo)
    }
}

fn clone_with_progress(url: &str, path: &Path) -> Result<Repository, git2::Error> {
    let mut callbacks = RemoteCallbacks::new();
    
    // 使用 sideband_progress 来实时显示下载进度
    callbacks.sideband_progress(|data| {
        print!("\r远程: {}", String::from_utf8_lossy(data));
        std::io::stdout().flush().unwrap();
        true
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let mut checkout_last_printed = 0;
    let mut checkout_options = CheckoutBuilder::new();
    checkout_options.progress(|_path, completed_steps, total_steps| {
        if total_steps > 0 && (completed_steps - checkout_last_printed >= 1000 || completed_steps == total_steps) {
            print!("\r检出：{}/{}", completed_steps, total_steps);
            checkout_last_printed = completed_steps;
            std::io::stdout().flush().unwrap();
        }
    }).force(); // 强制检出以覆盖文件

    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);
    builder.with_checkout(checkout_options);

    let repo = builder.clone(url, path)?;

    // 确保克隆后处于健康的分支状态
    {
        // 找到远程 HEAD 指向的提交
        let head_commit = repo.head()?.peel_to_commit()?;
        // 在该提交上创建本地 'master' 分支
        repo.branch("master", &head_commit, false)?;
        // 将 HEAD 指向新创建的 'master' 分支
        repo.set_head("refs/heads/master")?;
    }

    Ok(repo)
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

    // 定义我们需要的 CSS 选择器
    // 标题在 h1#pageName 中，但我们只需要 span.webName 之后的文本
    let page_name_selector = Selector::parse("h1#pageName").unwrap();
    // 实际内容在 div#revision 中
    let content_selector = Selector::parse("div#revision").unwrap();

    // 使用 WalkDir 遍历目录
    for entry in WalkDir::new(repo_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // 过滤出 .html 文件
        if path.is_file() && path.extension().map_or(false, |ext| ext == "html") {
            // 获取相对于仓库根目录的路径，用于日志和标识
            let relative_path = path.strip_prefix(repo_path).unwrap().to_string_lossy();

            // 读取文件内容
            let html_content = fs::read_to_string(path)?;

            // 使用 scraper 解析 HTML
            let document = Html::parse_document(&html_content);
            
            // 提取标题：从 h1#pageName 中提取，跳过 span.webName
            let title = document
                .select(&page_name_selector)
                .next()
                .map(|h1| {
                    // 获取所有文本节点
                    let full_text: String = h1.text().collect();
                    // 移除 "nLab" 前缀和多余空白
                    full_text
                        .replace("nLab", "")
                        .trim()
                        .to_string()
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Untitled Page".to_string());

            // 提取内容：找到 div#revision 区域
            let content_div = document
                .select(&content_selector)
                .next();

            let content = if let Some(div) = content_div {
                // 提取整个 revision div 中的所有文本，并去除多余空白
                div.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                eprintln!("警告：文件 {} 未找到核心内容。", relative_path);
                String::new()
            };

            // 存储结果
            pages.push(NLabPage {
                file_path: relative_path.to_string(),
                title,
                content,
            });
            parsed_count += 1;
        }
    }

    println!("--- 解析完成！共处理 {} 个文件。 ---", parsed_count);
    Ok(pages)
}
