use crate::cli::StatusArgs;
use crate::commands::locate;
use crate::git::Git;
use anyhow::{bail, Result};
use std::path::Path;

pub fn run(dest: &Path, a: StatusArgs) -> Result<()> {
    let (sf, repo) = locate(dest, &a.name)?;

    println!("库       : {}", sf.name);
    println!("源       : {}", sf.source.path);
    println!("恢复目标 : {}", sf.restore_dir);
    println!("仓库     : {}", repo.display());
    println!("忽略     : {}", sf.ignore.join(", "));

    if !Path::new(&sf.source.path).is_dir() {
        bail!("源目录不存在: {}", sf.source.path);
    }

    let g = Git::worktree(&repo, &sf.source.path);
    let st = g.run(&["status", "--short"])?;
    if st.trim().is_empty() {
        println!("状态     : 无改动");
    } else {
        println!("状态     :");
        print!("{st}");
    }
    Ok(())
}
