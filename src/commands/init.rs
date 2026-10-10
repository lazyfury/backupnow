use crate::cli::InitArgs;
use crate::config::{validate_name, AppConfig, SyncFile, SYNC_FILE};
use crate::git::{self, Git};
use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn run(app: &AppConfig, dest: &Path, a: InitArgs) -> Result<()> {
    git::ensure_git()?;
    validate_name(&a.name)?;

    let source = std::fs::canonicalize(&a.path)
        .with_context(|| format!("源目录不存在: {}", a.path.display()))?;
    if !source.is_dir() {
        bail!("源不是目录: {}", source.display());
    }

    std::fs::create_dir_all(dest)
        .with_context(|| format!("无法创建备份根: {}", dest.display()))?;

    let repo = dest.join(format!("{}.git", a.name));
    if repo.exists() {
        bail!("库已存在: {}", repo.display());
    }

    git::init_bare(&repo)?;

    let mut ignore = app.default_ignore.clone();
    for x in &a.exclude {
        if !ignore.contains(x) {
            ignore.push(x.clone());
        }
    }

    let sf = SyncFile::new(a.name.clone(), &source, ignore);
    sf.write_all(&repo)?;
    sf.write_exclude(&repo)?;

    // 首次快照
    let g = Git::worktree(&repo, &source);
    g.run(&["add", "-A"])?;
    let msg = format!("init: {} @ {}", a.name, sf.created_at);
    g.run(&["commit", "-m", &msg])?;
    let head = g.head_short()?;

    println!("✅ 已创建库 `{}`", a.name);
    println!("   源目录   : {}", source.display());
    println!("   仓库     : {}", repo.display());
    println!("   清单     : {}", repo.join(SYNC_FILE).display());
    println!("              {}", source.join(SYNC_FILE).display());
    println!("   首次快照 : {head}");
    println!("   忽略规则 : {}", sf.ignore.join(", "));
    Ok(())
}
