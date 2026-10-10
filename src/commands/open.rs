use crate::cli::OpenArgs;
use crate::commands::locate;
use anyhow::Result;
use std::path::Path;

pub fn run(dest: &Path, a: OpenArgs) -> Result<()> {
    let (sf, repo) = locate(dest, &a.name)?;

    // 优先打开预览目录（若存在）
    let preview = crate::commands::preview::preview_dir(dest, &sf);
    let target = if preview.is_dir() { preview } else { repo };

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&target).status()?;
        println!("已在 Finder 打开: {}", target.display());
    }
    #[cfg(not(target_os = "macos"))]
    {
        println!("{}", target.display());
    }
    Ok(())
}
