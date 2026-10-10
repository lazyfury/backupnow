use crate::cli::ListArgs;
use crate::commands::find_libs;
use crate::config::{expand_home, SyncFile};
use crate::git::Git;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn run(dest: &Path, a: ListArgs) -> Result<()> {
    // (清单, 仓库目录, 卷名)
    let mut libs: Vec<(SyncFile, PathBuf, Option<String>)> = Vec::new();

    if a.drives {
        let volumes = Path::new("/Volumes");
        if let Ok(rd) = std::fs::read_dir(volumes) {
            for e in rd.flatten() {
                let vol = e.path();
                if !vol.is_dir() {
                    continue;
                }
                let volname = e.file_name().to_string_lossy().to_string();
                for base in [vol.clone(), vol.join("Backups")] {
                    for (sf, repo) in find_libs(&base, 1) {
                        libs.push((sf, repo, Some(volname.clone())));
                    }
                }
            }
        }
        libs.sort_by(|a, b| a.1.cmp(&b.1));
        libs.dedup_by(|a, b| a.1 == b.1);
    } else {
        if !dest.exists() {
            bail!("备份根不存在: {}（盘没挂载？）", dest.display());
        }
        for (sf, repo) in find_libs(dest, 1) {
            libs.push((sf, repo, None));
        }
    }

    if a.json {
        let arr: Vec<&SyncFile> = libs.iter().map(|(s, _, _)| s).collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }

    if libs.is_empty() {
        if a.drives {
            println!("（没有扫描到备份库）");
        } else {
            println!("（{} 下没有备份库）", dest.display());
        }
        return Ok(());
    }

    if a.drives {
        println!(
            "{:<10} {:<12} {:<9} {:<34} {}",
            "卷", "库", "模式", "源目录", "最近提交"
        );
    } else {
        println!("{:<12} {:<9} {:<34} {}", "库", "模式", "源目录", "最近提交");
    }

    for (sf, repo, vol) in &libs {
        let last = Git::bare(repo)
            .run(&["log", "-1", "--format=%ci"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "-".into());

        // 源路径不存在时，提示本机可恢复到的位置（新机器）
        let expanded = expand_home(&sf.source.path_template);
        let src = if Path::new(&sf.source.path).exists() {
            sf.source.path.clone()
        } else if expanded != sf.source.path {
            format!("{} → {expanded}", sf.source.path)
        } else {
            sf.source.path.clone()
        };

        if a.drives {
            println!(
                "{:<10} {:<12} {:<9} {:<34} {}",
                vol.clone().unwrap_or_default(),
                sf.name,
                sf.mode,
                src,
                last
            );
        } else {
            println!("{:<12} {:<9} {:<34} {}", sf.name, sf.mode, src, last);
        }
    }
    Ok(())
}
