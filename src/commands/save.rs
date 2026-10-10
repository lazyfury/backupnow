use crate::cli::SaveArgs;
use crate::commands::{find_libs, locate, merge_ignore, sync_xattr_note};
use crate::config::{AppConfig, SyncFile};
use crate::git::{self, Git};
use crate::platform::notify;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// 一次 save 的结果摘要（用于通知）
pub struct Outcome {
    pub detail: String,
}

pub fn run(app: &AppConfig, dest: &Path, a: SaveArgs) -> Result<()> {
    if a.all {
        if a.name.is_some() {
            bail!("--all 与库名不能同时给出");
        }
        return save_all(app, dest, &a);
    }
    let name = a.name.clone().context("缺少库名（或使用 --all）")?;
    save_one(app, dest, &name, &a)
}

fn save_one(app: &AppConfig, dest: &Path, name: &str, a: &SaveArgs) -> Result<()> {
    let result = run_inner(app, dest, name, a);
    if app.notify && !a.quiet {
        match &result {
            Ok(o) => notify::notify(&format!("backupnow · {name}"), &o.detail),
            Err(e) => notify::notify(&format!("backupnow 失败 · {name}"), &format!("{e:#}")),
        }
    }
    result.map(|_| ())
}

fn save_all(app: &AppConfig, dest: &Path, a: &SaveArgs) -> Result<()> {
    let libs = find_libs(dest, 1);
    if libs.is_empty() {
        bail!("{} 下没有备份库", dest.display());
    }

    let mut ok = 0;
    let mut fail = 0;
    let mut details: Vec<String> = Vec::new();

    for (sf, _repo) in libs {
        let name = sf.name.clone();
        let sub = SaveArgs {
            name: Some(name.clone()),
            all: false,
            message: a.message.clone(),
            quiet: true,
            dry_run: a.dry_run,
        };
        match run_inner(app, dest, &name, &sub) {
            Ok(o) => {
                ok += 1;
                details.push(format!("{name}: {}", o.detail));
            }
            Err(e) => {
                fail += 1;
                eprintln!("❌ {name}: {e:#}");
                details.push(format!("{name}: 失败"));
            }
        }
    }

    println!("\n共 {ok} 个成功，{fail} 个失败。");
    if app.notify && !a.quiet {
        let title = if fail == 0 {
            format!("backupnow · 全部（{ok} 个）")
        } else {
            format!("backupnow · 全部（{ok} 成功 {fail} 失败）")
        };
        notify::notify(&title, &details.join("\n"));
    }
    Ok(())
}

fn run_inner(app: &AppConfig, dest: &Path, name: &str, a: &SaveArgs) -> Result<Outcome> {
    let (mut sf, repo) = locate(dest, name)?;
    let source = PathBuf::from(&sf.source.path);
    if !source.is_dir() {
        bail!("源目录不存在: {}", source.display());
    }

    let outcome = if sf.mode == "mirror" {
        save_mirror(app, &sf, &repo, &source, a.dry_run)?
    } else {
        save_snapshot(app, &mut sf, &repo, &source, a)?
    };

    // 若已生成预览，则保持刷新
    if !a.dry_run {
        let _ = crate::commands::preview::refresh_if_exists(dest, &sf, &repo);
    }
    Ok(outcome)
}

/// snapshot：把普通目录快照进 bare 仓库
fn save_snapshot(
    app: &AppConfig,
    sf: &mut SyncFile,
    repo: &Path,
    source: &Path,
    a: &SaveArgs,
) -> Result<Outcome> {
    let g = Git::worktree(repo, source);

    // 校准忽略规则
    sf.ignore = merge_ignore(&app.default_ignore, &sf.ignore);
    sf.write_exclude(repo)?;

    if a.dry_run {
        let g = Git::worktree(repo, source);
        let status = g.run(&["status", "--porcelain"])?;
        let changed = status.lines().filter(|l| !l.trim().is_empty()).count();
        let detrack = g.run(&["ls-files", "-ci", "--exclude-standard", "-z"])?;
        let detrack_n = detrack.split('\0').filter(|s| !s.is_empty()).count();
        println!(
            "[dry-run] `{}`：{changed} 项改动，{detrack_n} 项将取消跟踪",
            sf.name
        );
        for l in status.lines().filter(|l| !l.trim().is_empty()).take(50) {
            println!("  {l}");
        }
        return Ok(Outcome {
            detail: format!("dry-run: {changed} 项改动"),
        });
    }

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
    let xa = if xattr_updated { "（xattr 已更新）" } else { "" };

    let detail = if has_changes {
        println!("✅ 已保存 `{}` → {short}", sf.name);
        if removed > 0 {
            println!("   取消跟踪被忽略文件: {removed} 个");
        }
        if xattr_updated {
            println!("   已更新 xattr 清单");
        }
        format!("已保存 → {short}{xa}")
    } else {
        println!("没有变化，跳过。");
        if removed > 0 {
            println!("（取消了 {removed} 个被忽略文件的跟踪）");
        }
        if xattr_updated {
            println!("   已更新 xattr 清单");
        }
        format!("没有变化{xa}")
    };

    Ok(Outcome { detail })
}

/// mirror：把源仓库的所有 refs 拉取到盘上镜像
fn save_mirror(
    app: &AppConfig,
    sf: &SyncFile,
    repo: &Path,
    source: &Path,
    dry_run: bool,
) -> Result<Outcome> {
    let dirty = git::dirty_count(source);
    if dirty > 0 {
        eprintln!("⚠️  源仓库有 {dirty} 处未提交改动，不会被备份（mirror 只备份已提交内容）");
    }

    let g = Git::bare(repo);

    if dry_run {
        let out = std::process::Command::new("git")
            .arg("--git-dir")
            .arg(repo)
            .args(["fetch", "--dry-run", "--prune"])
            .output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        println!("[dry-run] `{}` 将 fetch：", sf.name);
        if text.trim().is_empty() {
            println!("  （无更新）");
        } else {
            print!("{text}");
        }
        return Ok(Outcome {
            detail: "dry-run".into(),
        });
    }

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

    let detail = if before == after {
        println!("没有变化（{refs} 个 ref）。");
        format!("没有变化（{refs} 个 ref）")
    } else {
        println!("✅ 已镜像 `{}`（{refs} 个 ref）", sf.name);
        format!("已镜像 {refs} 个 ref")
    };
    if xattr_updated {
        println!("   已更新 xattr 清单");
    }

    Ok(Outcome { detail })
}
