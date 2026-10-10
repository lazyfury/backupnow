use crate::cli::RestoreArgs;
use crate::commands::locate;
use crate::config::SyncFile;
use crate::git::{self, Git};
use crate::meta;
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

/// 从备份仓库读 xattr note 并写回 target
fn apply_xattr(repo: &Path, target: &Path, commit: &str, sub: Option<&str>) -> Result<usize> {
    let g = Git::bare(repo);
    let json = match g.notes_show(meta::NOTE_REF, commit)? {
        Some(j) if !j.trim().is_empty() => j,
        _ => return Ok(0),
    };
    let manifest = meta::Manifest::from_json(&json)?;
    let scoped = manifest.under(sub);
    let (n, warns) = meta::apply(target, &scoped);
    for w in warns {
        eprintln!("⚠️  xattr: {w}");
    }
    Ok(n)
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

    let commit = g.run(&["rev-parse", &rev])?.trim().to_string();
    let n = apply_xattr(repo, target, &commit, a.path.as_deref())?;

    println!("✅ 已恢复 `{}` → {}", sf.name, target.display());
    println!("   版本: {rev}");
    if n > 0 {
        println!("   xattr: 已还原 {n} 项");
    }
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
        let commit = if let Some(rev) = &a.rev {
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
            Git::bare(repo).run(&["rev-parse", rev])?.trim().to_string()
        } else {
            git::repo_head(target)?
        };
        let n = apply_xattr(repo, target, &commit, a.path.as_deref())?;
        println!("✅ 已克隆 `{}` → {}", sf.name, target.display());
        if n > 0 {
            println!("   xattr: 已还原 {n} 项");
        }
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
    let commit = g.run(&["rev-parse", &rev])?.trim().to_string();
    let n = apply_xattr(repo, target, &commit, a.path.as_deref())?;
    println!(
        "✅ 已覆盖恢复 `{}` → {}（仅默认分支工作区）",
        sf.name,
        target.display()
    );
    if n > 0 {
        println!("   xattr: 已还原 {n} 项");
    }
    Ok(())
}
