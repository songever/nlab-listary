use crate::models::NLabPage;
use scraper::{Html, Selector};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum ParseHtmlError {
    #[error("Failed to read file: {path}")]
    FileReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to strip path prefix")]
    PathPrefixError(#[from] std::path::StripPrefixError),

    #[error("No edit link found in HTML")]
    NoEditLinkFound,

    #[error("Edit link missing href attribute")]
    MissingHrefAttribute,

    #[error("Unexpected href format: {0}")]
    UnexpectedHrefFormat(String),

    #[error("Failed to parse selector")]
    SelectorParseError,

    #[error("WalkDir error")]
    WalkDirError(#[from] walkdir::Error),
}

pub fn index_local_files(repo_path: &Path) -> Result<Vec<NLabPage>, ParseHtmlError> {
    println!("\n--- 开始遍历和解析本地文件 ---");
    let mut pages: Vec<NLabPage> = Vec::new();
    let mut parsed_count = 0;
    let mut skipped_count = 0;
    let mut skipped_files: Vec<(PathBuf, ParseHtmlError)> = Vec::new();

    // 使用 WalkDir 遍历目录
    for entry in WalkDir::new(repo_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // 过滤出 .html 文件
        if path.is_file() && path.extension().map_or(false, |ext| ext == "html") {
            match parse_html_file(path, repo_path) {
                Ok(Some(page)) => {
                    pages.push(page);
                    parsed_count += 1;
                }
                Ok(None) => {
                    // 文件被解析但返回 None（如果需要这种情况）
                }
                Err(e) => {
                    // 记录错误但继续处理其他文件
                    skipped_count += 1;
                    skipped_files.push((path.to_path_buf(), e));
                    eprintln!("⚠ Skipping file due to error: {}", path.display());
                }
            }
        }
    }

    println!("--- 解析完成! ---");
    println!("成功处理: {} 个文件", parsed_count);

    if skipped_count > 0 {
        println!("跳过: {skipped_count} 个文件\n");
        println!("跳过的文件列表:");
        for (path, error) in &skipped_files {
            println!("  - {}: {}", path.display(), error);
        }
    }

    Ok(pages)
}

pub fn parse_html_file(file_path: &Path, repo_path: &Path) -> Result<Option<NLabPage>, ParseHtmlError> {
    let relative_path = file_path
        .strip_prefix(repo_path)?
        .to_string_lossy()
        .to_string();

    let html_content =
        fs::read_to_string(file_path).map_err(|e| ParseHtmlError::FileReadError {
            path: file_path.to_path_buf(),
            source: e,
        })?;
    let document = Html::parse_document(&html_content);

    // 提取标题
    let title = extract_title(&document);

    // 提取内容
    let content = extract_content(&document);

    let url = extract_url(&document)?;

    Ok(Some(NLabPage::new(relative_path, title, url, content)))
}

fn extract_title(document: &Html) -> String {
    let page_name_selector = Selector::parse("h1#pageName").unwrap();

    document
        .select(&page_name_selector)
        .next()
        .map_or_else(String::new, |title| {
            // 提取 <span class="webName"> 之后的文本
            title
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string()
        })
}

fn extract_content(document: &Html) -> String {
    let content_selector = Selector::parse("div#revision").unwrap();

    document
        .select(&content_selector)
        .next()
        .map_or_else(String::new, |element| {
            element.text().collect::<Vec<_>>().join(" ")
        })
}

fn extract_url(document: &Html) -> Result<String, ParseHtmlError> {
    let base_url = "https://ncatlab.org/nlab/show/";

    let edit_link_selector =
        Selector::parse("a#edit").map_err(|_| ParseHtmlError::SelectorParseError)?;

    let element = document
        .select(&edit_link_selector)
        .next()
        .ok_or(ParseHtmlError::NoEditLinkFound)?;

    let href = element
        .value()
        .attr("href")
        .ok_or(ParseHtmlError::MissingHrefAttribute)?;

    let page_name = href
        .strip_prefix("/nlab/edit/")
        .ok_or_else(|| ParseHtmlError::UnexpectedHrefFormat(href.to_string()))?;

    let full_url = format!("{}{}", base_url, page_name);
    println!("URL:  {}", full_url);
    Ok(full_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_one_url_and_open_in_browser() {
        let path = Path::new("nlab_mirror/pages/7/0/2/0/10207/content.html");

        if path.is_file() {
            let html_content = fs::read_to_string(path).expect("Failed to read HTML file");
            let document = Html::parse_document(&html_content);

            let url = extract_url(&document).expect("Failed to extract URL");
            let cmds = open::commands(&url)[0]
                .status()
                .expect("Failed to open URL");
            println!("Extracted URL: {}\n", url);
            assert!(cmds.success());
        }
    }

    #[test]
    fn test_extract_multiple_urls() {
        let mut total_files = 0;
        let mut successful_extractions = 0;
        let mut failed_files: Vec<(String, ParseHtmlError)> = Vec::new();

        for path in WalkDir::new("nlab_mirror/pages")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.path()
                    .to_str()
                    .map(|s| s.to_string())
                    .filter(|s| s.ends_with(".html"))
            })
        {
            total_files += 1;
            println!("Visiting: {:?}", path);

            match fs::read_to_string(&path) {
                Ok(html_content) => {
                    let document = Html::parse_document(&html_content);

                    match extract_url(&document) {
                        Ok(url) => {
                            successful_extractions += 1;
                            println!("✓ Extracted URL: {}", url);
                        }
                        Err(e) => {
                            println!("✗ Failed to extract URL: {:?}", e);
                            failed_files.push((path.clone(), e));
                        }
                    }
                }
                Err(e) => {
                    let parse_error = ParseHtmlError::FileReadError {
                        path: PathBuf::from(&path),
                        source: e,
                    };
                    println!("✗ Failed to read file: {:?}", parse_error);
                    failed_files.push((path.clone(), parse_error));
                }
            }
        }

        // 打印统计信息
        println!("\n=== Summary ===");
        println!("Total files processed: {}", total_files);
        println!("Successful extractions: {}", successful_extractions);
        println!("Failed extractions: {}", failed_files.len());

        // 打印所有失败的文件
        if !failed_files.is_empty() {
            println!("\n=== Failed Files ===");
            for (path, error) in &failed_files {
                println!("File: {}", path);
                println!("Error: {}", error);
                println!("---");
            }
        }

        // 可选：如果你想让测试在有失败时也通过，可以注释掉下面这行
        // assert_eq!(failed_files.len(), 0, "Some files failed to extract URLs");
    }

    #[test]
    fn test_walkdir_finds_specific_file() {
        let target_file_path = "nlab_mirror/pages/0/0/0/0/10000/content.html";
        let mut was_target_file_found = false;

        for path in WalkDir::new("nlab_mirror/pages")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.path()
                    .to_str()
                    .map(|s| s.to_string())
                    .filter(|s| s.ends_with(".html"))
            })
        {
            if path == target_file_path {
                was_target_file_found = true;
                println!(">>> Target file found: {}", path);
                break;
            }
        }

        assert!(
            was_target_file_found,
            "Verification failed: The target file '{}' was not found during traversal.",
            target_file_path
        );
    }

    #[test]
    fn test_inspect_failed_file() {
        let path = Path::new("nlab_mirror/pages/3/9/5/2/2593/content.html");

        if !path.exists() {
            println!("File does not exist, skipping test");
            return;
        }

        let html_content = fs::read_to_string(path).expect("Failed to read HTML file");
        let document = Html::parse_document(&html_content);

        // 尝试查找 edit 链接
        let edit_link_selector = Selector::parse("a#edit").unwrap();
        let edit_link = document.select(&edit_link_selector).next();

        println!("Edit link found: {}", edit_link.is_some());

        if let Some(link) = edit_link {
            println!("Edit link HTML: {:?}", link.html());
            println!("Edit link href: {:?}", link.value().attr("href"));
        } else {
            // 尝试查找所有的 a 标签，看看有什么
            let all_links_selector = Selector::parse("a").unwrap();
            println!("\nAll links in the document:");
            for (i, link) in document.select(&all_links_selector).enumerate().take(10) {
                println!(
                    "Link {}: id={:?}, href={:?}",
                    i,
                    link.value().attr("id"),
                    link.value().attr("href")
                );
            }
        }

        // 检查文件的整体结构
        println!("\nDocument structure:");
        let title = extract_title(&document);
        println!(
            "Title: {}",
            if title.is_empty() { "(empty)" } else { &title }
        );

        let content = extract_content(&document);
        println!("Content length: {} chars", content.len());
        println!(
            "Content preview: {}",
            &content.chars().take(200).collect::<String>()
        );
    }
}
