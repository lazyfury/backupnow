use crate::cli::RestoreArgs;
use crate::commands::{find_libs, locate};
use crate::config::SyncFile;
use crate::git::{self, Git};
use crate::meta;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(dest: &Path, a: RestoreArgs) -> Result<()> {
    if a.all {
        if a.name.is_some() {
            bail!("--all 与库名不能同时给出");
        }
        if a.to.is_some() {
            bail!("--all 不能与 --to 同用（每个库恢复到各自的 restoreDir）");
        }
        return restore_all(dest, &a);
    }

    let name = a.name.clone().context("缺少库名（或使用 --all）")?;
    let (sf, repo) = locate(dest, &name)?;
    let target: PathBuf = a.to.clone().unwrap_or_else(|| sf.resolved_restore_dir());

    if sf.mode == "mirror" {
        restore_mirror(&sf, &repo, &target, &a)
    } else {
        restore_snapshot(&sf, &repo, &target, &a)
    }
}

fn restore_all(dest: &Path, a: &RestoreArgs) -> Result<()> {
    let libs = find_libs(dest, 1);
    if libs.is_empty() {
        bail!("{} 下没有备份库", dest.display());
    }
    let mut ok = 0;
    let mut fail = 0;
    for (sf, repo) in libs {
        let target = sf.resolved_restore_dir();
        let sub = RestoreArgs {
            name: Some(sf.name.clone()),
            all: false,
            to: Some(target.clone()),
            rev: a.rev.clone(),
            path: a.path.clone(),
            force: a.force,
            dry_run: a.dry_run,
            snapshot_first: a.snapshot_first,
        };
        let r = if sf.mode == "mirror" {
            restore_mirror(&sf, &repo, &target, &sub)
        } else {
            restore_snapshot(&sf, &repo, &target, &sub)
        };
        match r {
            Ok(()) => ok += 1,
            Err(e) => {
                eprintln!("❌ {}: {e:#}", sf.name);
                fail += 1;
            }
        }
    }
    println!("\n共 {ok} 个成功，{fail} 个失败。");
    Ok(())
}

/// 覆盖前创建本地 APFS 快照（best-effort）
#[cfg(target_os = "macos")]
fn local_snapshot() {
    match std::process::Command::new("tmutil")
        .arg("localsnapshot")
        .output()
    {
        Ok(o) if o.status.success() => {
            let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
            println!("   已创建本地 APFS 快照: {name}");
        }
        Ok(o) => eprintln!(
            "⚠️  tmutil localsnapshot 失败: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => eprintln!("⚠️  无法执行 tmutil: {e}"),
    }
}

#[cfg(not(target_os = "macos"))]
fn local_snapshot() {}

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
    if a.snapshot_first {
        local_snapshot();
    }
    if a.dry_run {
        let g = Git::bare(repo);
        let rev = a.rev.clone().unwrap_or_else(|| "HEAD".into());
        let mut args: Vec<&str> = vec!["ls-tree", "-r", "--name-only", &rev];
        if let Some(p) = &a.path {
            args.push("--");
            args.push(p);
        }
        let out = g.run(&args)?;
        let n = out.lines().count();
        println!("[dry-run] `{}` 将恢复 {n} 个文件 → {}", sf.name, target.display());
        for l in out.lines().take(50) {
            println!("  {l}");
        }
        return Ok(());
    }

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
    if a.dry_run {
        println!(
            "[dry-run] `{}` 将 git clone {} → {}",
            sf.name,
            repo.display(),
            target.display()
        );
        return Ok(());
    }

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
