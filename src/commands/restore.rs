use crate::cli::RestoreArgs;
use crate::commands::locate;
use crate::git::Git;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn run(dest: &Path, a: RestoreArgs) -> Result<()> {
    let (sf, repo) = locate(dest, &a.name)?;

    let target: PathBuf = a
        .to
        .clone()
        .unwrap_or_else(|| sf.resolved_restore_dir());

    let non_empty = target.is_dir()
        && std::fs::read_dir(&target)
            .map(|mut it| it.next().is_some())
            .unwrap_or(false);
    if non_empty && !a.force {
        bail!(
            "目标目录非空: {}（确认覆盖请加 --force）",
            target.display()
        );
    }
    std::fs::create_dir_all(&target)?;

    let g = Git::worktree(&repo, &target);
    let rev = a.rev.clone().unwrap_or_else(|| "HEAD".into());
    let pathspec = a.path.clone().unwrap_or_else(|| ".".into());

    g.run(&["checkout", "-f", &rev, "--", &pathspec])?;

    println!("✅ 已恢复 `{}` → {}", sf.name, target.display());
    println!("   版本: {rev}");
    Ok(())
}
