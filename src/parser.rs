use scraper::{Html, Selector};
use std::error::Error;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use crate::models::NLabPage;

pub fn index_local_files(repo_path: &Path) -> Result<Vec<NLabPage>, Box<dyn Error>> {
    println!("\n--- 开始遍历和解析本地文件 ---");
    let mut pages: Vec<NLabPage> = Vec::new();
    let mut parsed_count = 0;

    // 定义我们需要的 CSS 选择器
    // 标题在 h1#pageName 中，但我们只需要 span.webName 之后的文本
    
    // 实际内容在 div#revision 中
    

    // 使用 WalkDir 遍历目录
    for entry in WalkDir::new(repo_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        // 过滤出 .html 文件
        if path.is_file() && path.extension().map_or(false, |ext| ext == "html") {
            if let Some(page) =
                parse_html_file(path, repo_path)?
            {
                pages.push(page);
                parsed_count += 1;
            }
        }
    }

    println!("--- 解析完成！共处理 {} 个文件。 ---", parsed_count);
    Ok(pages)
}

fn parse_html_file(
    file_path: &Path,
    repo_path: &Path,
) -> Result<Option<NLabPage>, Box<dyn Error>> {
    let relative_path = file_path
        .strip_prefix(repo_path)?
        .to_string_lossy()
        .to_string();

    let html_content = fs::read_to_string(file_path)?;
    let document = Html::parse_document(&html_content);

    // 提取标题
    let title = extract_title(&document);

    // 提取内容
    let content = extract_content(&document);

    let url = extract_url(&document);

    Ok(Some(NLabPage {
        file_path: relative_path,
        url,
        title,
        content,
    }))
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

fn extract_url(document: &Html) -> String {
    let base_url = "https://ncatlab.org/nlab/show/";

    let edit_link_selector = Selector::parse("a#edit").expect("Failed to parse selector");

    let element = document.select(&edit_link_selector).next().expect("No edit link found");
    let href = element.value().attr("href").expect("No href attribute found");
    let page_name = href.strip_prefix("/nlab/edit/").expect("Unexpected href format"); 
    let full_url = format!("{}{}", base_url, page_name);
    println!("URL:  {}\n", full_url);
    return full_url;
}
#[test]
fn test_extract_url() {
    let path = Path::new("nlab_mirror/pages/0/0/0/0/10000/content.html");

    if path.is_file() {
        let html_content = fs::read_to_string(path).expect("Failed to read HTML file");
        let document = Html::parse_document(&html_content);

        let url = extract_url(&document);
        let cmds = open::commands(&url)[0].status().expect("Failed to open URL");
        println!("Extracted URL: {}", url);
        assert!(cmds.success());
    }
}