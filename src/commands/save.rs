use crate::cli::SaveArgs;
use crate::commands::{locate, merge_ignore, sync_xattr_note};
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
        save_mirror(app, &sf, &repo, &source)
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

    if has_changes {
        let msg = a.message.clone().unwrap_or_else(|| {
            format!(
                "backup: {} {} @ {}",
                sf.name,
                sf.source.user,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            )
        });
        g.run(&["commit", "-m", &msg])?;
    }

    let head = g.head()?;

    // xattr 清单（内容没变但 xattr 变了也会更新）
    let mut xattr_updated = false;
    if app.xattr {
        let files = g.ls_files()?;
        xattr_updated = sync_xattr_note(&g, &files, source, &app.xattr_ignore, &head)?;
    }

    sf.write_all(repo)?;
    let _ = g.run(&["gc", "--auto"]);

    let short = &head[..head.len().min(7)];
    if has_changes {
        println!("✅ 已保存 `{}` → {short}", sf.name);
        if removed > 0 {
            println!("   取消跟踪被忽略文件: {removed} 个");
        }
    } else {
        println!("没有变化，跳过。");
        if removed > 0 {
            println!("（取消了 {removed} 个被忽略文件的跟踪）");
        }
    }
    if xattr_updated {
        println!("   已更新 xattr 清单");
    }
    Ok(())
}

/// mirror：把源仓库的所有 refs 拉取到盘上镜像
fn save_mirror(app: &AppConfig, sf: &SyncFile, repo: &Path, source: &Path) -> Result<()> {
    let dirty = git::dirty_count(source);
    if dirty > 0 {
        eprintln!("⚠️  源仓库有 {dirty} 处未提交改动，不会被备份（mirror 只备份已提交内容）");
    }

    let g = Git::bare(repo);
    let before = g.run(&["rev-parse", "--all"]).unwrap_or_default();
    g.run(&["remote", "update", "--prune"])?;
    let after = g.run(&["rev-parse", "--all"]).unwrap_or_default();

    // xattr 清单挂到源 HEAD 对应的镜像提交上
    let mut xattr_updated = false;
    if app.xattr {
        if let Ok(head) = git::repo_head(source) {
            let files = git::repo_ls_files(source).unwrap_or_default();
            xattr_updated = sync_xattr_note(&g, &files, source, &app.xattr_ignore, &head)?;
        }
    }

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
    if xattr_updated {
        println!("   已更新 xattr 清单");
    }
    Ok(())
}
