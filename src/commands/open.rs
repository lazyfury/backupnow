use crate::cli::OpenArgs;
use crate::commands::locate;
use anyhow::Result;
use std::path::Path;

pub fn run(dest: &Path, a: OpenArgs) -> Result<()> {
    let (_sf, repo) = locate(dest, &a.name)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&repo).status()?;
        println!("已在 Finder 打开: {}", repo.display());
    }
    #[cfg(not(target_os = "macos"))]
    {
        println!("{}", repo.display());
    }
    Ok(())
}
