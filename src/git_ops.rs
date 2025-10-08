use super::REPO_URL;
use git2::build::CheckoutBuilder;
use git2::{FetchOptions, RemoteCallbacks};
use git2::{Repository, build::RepoBuilder};
use std::io::Write;
use std::path::Path;

pub fn update_local_repository(path: &Path) -> Result<Repository, git2::Error> {
    if path.exists() {
        println!("本地仓库已存在，正在更新...");
        let repo = Repository::open(path)?;

        // 1. 执行 FETCH (获取远程最新状态)
        fetch_repo(&repo)?;

        // 2. 获取 FETCH_HEAD 并分析合并类型
        let (analysis, oid) = get_fetch_head(&repo)?;

        if analysis.0.is_up_to_date() {
            println!("本地仓库已是最新版本。");
        } else if analysis.0.is_fast_forward() {
            println!("正在执行快进合并...");
            // 3. 执行快进合并
            // 获取 HEAD 指向的引用 (例如 refs/heads/main)
            let mut reference = repo.head()?.resolve()?;
            fast_forward(&repo, &mut reference, oid)?;
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

pub fn clone_with_progress(url: &str, path: &Path) -> Result<Repository, git2::Error> {
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
    checkout_options
        .progress(|_path, completed_steps, total_steps| {
            if total_steps > 0
                && (completed_steps - checkout_last_printed >= 1000
                    || completed_steps == total_steps)
            {
                print!("\r检出：{}/{}", completed_steps, total_steps);
                checkout_last_printed = completed_steps;
                std::io::stdout().flush().unwrap();
            }
        })
        .force(); // 强制检出以覆盖文件

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

fn fetch_repo(repo: &Repository) -> Result<(), git2::Error> {
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

    Ok(())
}

fn get_fetch_head(
    repo: &Repository,
) -> Result<((git2::MergeAnalysis, git2::MergePreference), git2::Oid), git2::Error> {
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let oid = fetch_head.target().ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "FETCH_HEAD 没有目标 OID",
        )
    })?;

    let remote_commit = repo.find_annotated_commit(oid)?;
    Ok((repo.merge_analysis(&[&remote_commit])?, oid))
}

fn fast_forward(
    repo: &Repository,
    reference: &mut git2::Reference,
    oid: git2::Oid,
) -> Result<(), git2::Error> {
    // 获取该引用的名称，用于后续操作
    let ref_name = reference
        .name()
        .ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::InvalidSpec,
                git2::ErrorClass::Reference,
                "无法获取 HEAD 引用的名称",
            )
        })?
        .to_string();

    println!("正在快进本地引用: {}", ref_name);

    // 将该引用直接指向 fetch 下来的 commit (oid)
    reference.set_target(oid, "Fast-Forward")?;

    // 更新 HEAD 指向，并检出工作目录以匹配
    repo.set_head(&ref_name)?;
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    println!("更新完成。");
    Ok(())
}
