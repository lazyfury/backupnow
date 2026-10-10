use crate::cli::SaveArgs;
use crate::commands::{locate, merge_ignore};
use crate::config::{AppConfig, SyncFile};
use crate::git::{self, Git};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn run(app: &AppConfig, dest: &Path, a: SaveArgs) -> Result<()> {
    let (mut sf, repo) = locate(dest, &a.name)?;
    let source = PathBuf::from(&sf.source.path);
    if !source.is_dir() {
        bail!("源目录不存在: {}", source.display());
    }

    if sf.mode == "mirror" {
        save_mirror(&sf, &repo, &source)
    } else {
        save_snapshot(app, &mut sf, &repo, &source, &a)
    }
}

/// snapshot：把普通目录快照进 bare 仓库
fn save_snapshot(
    app: &AppConfig,
    sf: &mut SyncFile,
    repo: &Path,
    source: &Path,
    a: &SaveArgs,
) -> Result<()> {
    let g = Git::worktree(repo, source);

    // 校准忽略规则
    sf.ignore = merge_ignore(&app.default_ignore, &sf.ignore);
    sf.write_exclude(repo)?;
    sf.write_all(repo)?;

    // 取消"已被忽略但仍跟踪"的文件
    let ignored = g.run(&["ls-files", "-ci", "--exclude-standard", "-z"])?;
    let paths: Vec<&str> = ignored.split('\0').filter(|s| !s.is_empty()).collect();
    let removed = paths.len();
    if removed > 0 {
        let mut input = Vec::new();
        for p in &paths {
            input.extend_from_slice(p.as_bytes());
            input.push(0);
        }
        g.run_with_stdin(
            &[
                "rm",
                "--cached",
                "-r",
                "--quiet",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            &input,
        )?;
    }

    g.run(&["add", "-A"])?;

    let has_changes = !g.success(&["diff", "--cached", "--quiet"])?;
    if !has_changes {
        println!("没有变化，跳过。");
        if removed > 0 {
            println!("（取消了 {removed} 个被忽略文件的跟踪）");
        }
        return Ok(());
    }

    let msg = a.message.clone().unwrap_or_else(|| {
        format!(
            "backup: {} {} @ {}",
            sf.name,
            sf.source.user,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    });
    g.run(&["commit", "-m", &msg])?;
    let head = g.head_short()?;

    sf.write_all(repo)?;
    let _ = g.run(&["gc", "--auto"]);

    println!("✅ 已保存 `{}` → {}", sf.name, head);
    if removed > 0 {
        println!("   取消跟踪被忽略文件: {removed} 个");
    }
    Ok(())
}

/// mirror：把源仓库的所有 refs 拉取到盘上镜像
fn save_mirror(sf: &SyncFile, repo: &Path, source: &Path) -> Result<()> {
    // 未提交改动不会被备份 —— 警告到 stderr
    let dirty = git::dirty_count(source);
    if dirty > 0 {
        eprintln!(
            "⚠️  源仓库有 {dirty} 处未提交改动，不会被备份（mirror 只备份已提交内容）"
        );
    }

    let g = Git::bare(repo);
    let before = g.run(&["rev-parse", "--all"]).unwrap_or_default();
    g.run(&["remote", "update", "--prune"])?;
    let after = g.run(&["rev-parse", "--all"]).unwrap_or_default();

    let refs = g
        .run(&["for-each-ref", "--format=%(refname)"])
        .unwrap_or_default()
        .lines()
        .count();

    if before == after {
        println!("没有变化（{refs} 个 ref）。");
    } else {
        println!("✅ 已镜像 `{}`（{refs} 个 ref）", sf.name);
    }
    Ok(())
}
