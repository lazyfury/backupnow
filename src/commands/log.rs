use crate::cli::LogArgs;
use crate::commands::locate;
use crate::git::Git;
use anyhow::Result;
use std::path::Path;

pub fn run(dest: &Path, a: LogArgs) -> Result<()> {
    let (_sf, repo) = locate(dest, &a.name)?;
    let g = Git::bare(&repo);
    let n = format!("-n{}", a.limit);
    let mut args: Vec<&str> = vec!["log", "--oneline", "--date=short", n.as_str()];
    if let Some(p) = &a.path {
        args.push("--");
        args.push(p);
    }
    let out = g.run(&args)?;
    if out.trim().is_empty() {
        println!("（无提交）");
    } else {
        print!("{out}");
    }
    Ok(())
}
