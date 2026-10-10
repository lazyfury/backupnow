use crate::cli::{IgnoreAction, IgnoreArgs};
use crate::commands::locate;
use anyhow::{bail, Result};
use std::path::Path;

pub fn run(dest: &Path, a: IgnoreArgs) -> Result<()> {
    let (mut sf, repo) = locate(dest, &a.name)?;

    match a.action {
        IgnoreAction::List => {
            for p in &sf.ignore {
                println!("{p}");
            }
        }
        IgnoreAction::Add { pattern } => {
            if sf.ignore.contains(&pattern) {
                println!("规则已存在: {pattern}");
            } else {
                sf.ignore.push(pattern.clone());
                sf.write_all(&repo)?;
                sf.write_exclude(&repo)?;
                println!("已添加: {pattern}");
            }
        }
        IgnoreAction::Rm { pattern } => {
            let before = sf.ignore.len();
            sf.ignore.retain(|p| p != &pattern);
            if sf.ignore.len() == before {
                bail!("规则不存在: {pattern}");
            }
            sf.write_all(&repo)?;
            sf.write_exclude(&repo)?;
            println!("已删除: {pattern}");
        }
        IgnoreAction::Edit => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
            let tmp = std::env::temp_dir().join(format!("backupnow-{}.ignore", sf.name));
            let mut text = String::from("# 每行一条忽略规则，保存退出即生效\n");
            for p in &sf.ignore {
                text.push_str(p);
                text.push('\n');
            }
            std::fs::write(&tmp, text)?;
            let status = std::process::Command::new(&editor).arg(&tmp).status()?;
            if !status.success() {
                bail!("编辑器退出异常，未修改");
            }
            let edited = std::fs::read_to_string(&tmp)?;
            sf.ignore = edited
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect();
            sf.write_all(&repo)?;
            sf.write_exclude(&repo)?;
            let _ = std::fs::remove_file(&tmp);
            println!("已更新，共 {} 条规则", sf.ignore.len());
        }
    }
    Ok(())
}
