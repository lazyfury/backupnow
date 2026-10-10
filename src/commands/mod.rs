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
use crate::git::Git;
use crate::meta;
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

/// 在 root 下按深度扫描备份库（返回 清单 + 仓库目录）
pub fn find_libs(root: &Path, depth: usize) -> Vec<(SyncFile, PathBuf)> {
    let mut out: Vec<(SyncFile, PathBuf)> = Vec::new();
    walk_libs(root, depth, &mut out);
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    out
}

fn walk_libs(dir: &Path, remaining: usize, out: &mut Vec<(SyncFile, PathBuf)>) {
    if remaining == 0 {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let manifest = p.join(SYNC_FILE);
        if manifest.exists() {
            if let Ok(sf) = SyncFile::load(&manifest) {
                out.push((sf, p.clone()));
            }
            continue;
        }
        // 不深入隐藏目录 / 版本目录
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        walk_libs(&p, remaining - 1, out);
    }
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

/// 采集并写入 xattr git note；返回是否有变化
pub fn sync_xattr_note(
    g: &Git,
    files: &[String],
    source: &Path,
    ignore: &[String],
    commit: &str,
) -> Result<bool> {
    let manifest = meta::capture(source, files, ignore);
    let json = manifest.to_json();
    let existing = g.notes_show(meta::NOTE_REF, commit)?;
    if existing.as_deref().map(str::trim) == Some(json.trim()) {
        return Ok(false);
    }
    g.notes_add(meta::NOTE_REF, commit, &json)?;
    Ok(true)
}
