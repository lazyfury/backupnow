pub mod config;
pub mod doctor;
pub mod ignore;
pub mod init;
pub mod list;
pub mod log;
pub mod restore;
pub mod save;
pub mod status;
pub mod timer;

use crate::config::{SyncFile, SYNC_FILE};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// 定位某个库。
///
/// `name` 可以是：
/// - 裸名字 → `<dest>/<name>.git`
/// - 仓库目录路径（含 `backupnow_sync.json` 或结尾 `.git`）
/// - 备份根路径（若其中只有唯一一个库）
pub fn locate(dest: &Path, name: &str) -> Result<(SyncFile, PathBuf)> {
    let repo = resolve_repo(dest, name)?;
    let sync_path = repo.join(SYNC_FILE);
    if !sync_path.exists() {
        bail!(
            "找不到库 `{name}`（{} 不存在）。用 `backupnow list` 查看现有库。",
            sync_path.display()
        );
    }
    let sf = SyncFile::load(&sync_path)?;
    Ok((sf, repo))
}

fn resolve_repo(dest: &Path, name: &str) -> Result<PathBuf> {
    let looks_like_path = name.contains('/') || name.ends_with(".git");
    if !looks_like_path {
        return Ok(dest.join(format!("{name}.git")));
    }
    let p = PathBuf::from(name);
    if p.join(SYNC_FILE).exists() {
        return Ok(p);
    }
    if p.is_dir() {
        let mut found: Vec<PathBuf> = Vec::new();
        for e in std::fs::read_dir(&p)? {
            let e = e?;
            if e.path().join(SYNC_FILE).exists() {
                found.push(e.path());
            }
        }
        found.sort();
        match found.len() {
            0 => bail!("{} 下没有备份库", p.display()),
            1 => return Ok(found.pop().unwrap()),
            _ => {
                let names: Vec<String> = found
                    .iter()
                    .filter_map(|x| x.file_name().map(|n| n.to_string_lossy().to_string()))
                    .collect();
                bail!(
                    "{} 下有多个库，请指定具体名字：{}",
                    p.display(),
                    names.join(", ")
                );
            }
        }
    }
    bail!("不是有效的库路径: {name}");
}

/// 合并默认忽略 + 已有规则，去重保持顺序
pub fn merge_ignore(defaults: &[String], current: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in defaults.iter().chain(current.iter()) {
        if !out.contains(p) {
            out.push(p.clone());
        }
    }
    out
}
