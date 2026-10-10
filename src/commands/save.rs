use crate::cli::SaveArgs;
use crate::config::AppConfig;
use crate::commands::{locate, merge_ignore};
use crate::git::Git;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn run(app: &AppConfig, dest: &Path, a: SaveArgs) -> Result<()> {
    let (mut sf, repo) = locate(dest, &a.name)?;
    let source = PathBuf::from(&sf.source.path);
    if !source.is_dir() {
        bail!("源目录不存在: {}", source.display());
    }

    let g = Git::worktree(&repo, &source);

    // 校准忽略规则
    sf.ignore = merge_ignore(&app.default_ignore, &sf.ignore);
    sf.write_exclude(&repo)?;
    sf.write_all(&repo)?;

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

    let msg = a.message.unwrap_or_else(|| {
        format!(
            "backup: {} {} @ {}",
            sf.name,
            sf.source.user,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        )
    });
    g.run(&["commit", "-m", &msg])?;
    let head = g.head_short()?;

    // 回写松散副本
    sf.write_all(&repo)?;
    let _ = g.run(&["gc", "--auto"]);

    println!("✅ 已保存 `{}` → {}", a.name, head);
    if removed > 0 {
        println!("   取消跟踪被忽略文件: {removed} 个");
    }
    Ok(())
}
