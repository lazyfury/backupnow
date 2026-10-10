use crate::cli::StatusArgs;
use crate::commands::locate;
use crate::git::{self, Git};
use anyhow::{bail, Result};
use std::path::Path;

pub fn run(dest: &Path, a: StatusArgs) -> Result<()> {
    let (sf, repo) = locate(dest, &a.name)?;

    println!("库       : {}", sf.name);
    println!("模式     : {}", sf.mode);
    println!("源       : {}", sf.source.path);
    println!("恢复目标 : {}", sf.restore_dir);
    println!("仓库     : {}", repo.display());
    if sf.mode == "snapshot" {
        println!("忽略     : {}", sf.ignore.join(", "));
    }

    if !Path::new(&sf.source.path).is_dir() {
        bail!("源目录不存在: {}", sf.source.path);
    }

    if sf.mode == "mirror" {
        let dirty = git::dirty_count(Path::new(&sf.source.path));
        if dirty > 0 {
            println!("未提交   : {dirty} 处（不会被备份）");
        } else {
            println!("未提交   : 无");
        }
        let g = Git::bare(&repo);
        let refs = g
            .run(&["for-each-ref", "--format=%(refname) %(objectname:short)"])
            .unwrap_or_default();
        println!("镜像 refs:");
        print!("{refs}");
    } else {
        let g = Git::worktree(&repo, &sf.source.path);
        let st = g.run(&["status", "--short"])?;
        if st.trim().is_empty() {
            println!("状态     : 无改动");
        } else {
            println!("状态     :");
            print!("{st}");
        }
    }
    Ok(())
}
