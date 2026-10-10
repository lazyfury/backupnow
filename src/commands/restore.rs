use crate::cli::RestoreArgs;
use crate::commands::locate;
use crate::config::SyncFile;
use crate::git::Git;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(dest: &Path, a: RestoreArgs) -> Result<()> {
    let (sf, repo) = locate(dest, &a.name)?;
    let target: PathBuf = a.to.clone().unwrap_or_else(|| sf.resolved_restore_dir());

    if sf.mode == "mirror" {
        return restore_mirror(&sf, &repo, &target, &a);
    }
    restore_snapshot(&sf, &repo, &target, &a)
}

fn restore_snapshot(sf: &SyncFile, repo: &Path, target: &Path, a: &RestoreArgs) -> Result<()> {
    let non_empty = target.is_dir()
        && std::fs::read_dir(target)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false);
    if non_empty && !a.force {
        bail!("目标目录非空: {}（确认覆盖请加 --force）", target.display());
    }
    std::fs::create_dir_all(target)?;

    let g = Git::worktree(repo, target);
    let rev = a.rev.clone().unwrap_or_else(|| "HEAD".into());
    let pathspec = a.path.clone().unwrap_or_else(|| ".".into());
    g.run(&["checkout", "-f", &rev, "--", &pathspec])?;

    println!("✅ 已恢复 `{}` → {}", sf.name, target.display());
    println!("   版本: {rev}");
    Ok(())
}

fn restore_mirror(sf: &SyncFile, repo: &Path, target: &Path, a: &RestoreArgs) -> Result<()> {
    let non_empty = target.is_dir()
        && std::fs::read_dir(target)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false);

    if !non_empty {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let out = Command::new("git")
            .arg("clone")
            .arg(repo)
            .arg(target)
            .output()?;
        if !out.status.success() {
            bail!("git clone 失败: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        if let Some(rev) = &a.rev {
            let out = Command::new("git")
                .arg("-C")
                .arg(target)
                .args(["checkout", rev])
                .output()?;
            if !out.status.success() {
                bail!(
                    "checkout {rev} 失败: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
        }
        println!("✅ 已克隆 `{}` → {}", sf.name, target.display());
        return Ok(());
    }

    if !a.force {
        bail!(
            "目标目录非空: {}（mirror 恢复用 git clone，需要空目录；确认覆盖工作区请加 --force）",
            target.display()
        );
    }
    let g = Git::worktree(repo, target);
    let rev = a.rev.clone().unwrap_or_else(|| "HEAD".into());
    let pathspec = a.path.clone().unwrap_or_else(|| ".".into());
    g.run(&["checkout", "-f", &rev, "--", &pathspec])?;
    println!(
        "✅ 已覆盖恢复 `{}` → {}（仅默认分支工作区）",
        sf.name,
        target.display()
    );
    Ok(())
}
