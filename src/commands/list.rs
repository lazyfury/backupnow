use crate::cli::ListArgs;
use crate::config::{SyncFile, SYNC_FILE};
use crate::git::Git;
use anyhow::{bail, Result};
use std::path::Path;

pub fn run(dest: &Path, a: ListArgs) -> Result<()> {
    if !dest.exists() {
        bail!("备份根不存在: {}（盘没挂载？）", dest.display());
    }

    let mut libs: Vec<(SyncFile, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dest)? {
        let entry = entry?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let sync = p.join(SYNC_FILE);
        if !sync.exists() {
            continue;
        }
        match SyncFile::load(&sync) {
            Ok(sf) => libs.push((sf, p)),
            Err(e) => eprintln!("warn: {} 解析失败: {e}", sync.display()),
        }
    }
    libs.sort_by(|a, b| a.0.name.cmp(&b.0.name));

    if a.json {
        let arr: Vec<&SyncFile> = libs.iter().map(|(s, _)| s).collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }

    if libs.is_empty() {
        println!("（{} 下没有备份库）", dest.display());
        return Ok(());
    }

    println!("{:<12} {:<10} {:<34} {}", "库", "引擎", "源目录", "最近提交");
    for (sf, repo) in libs {
        let last = Git::bare(&repo)
            .run(&["log", "-1", "--format=%ci"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".into());
        println!("{:<12} {:<10} {:<34} {}", sf.name, sf.engine, sf.source.path, last);
    }
    Ok(())
}
