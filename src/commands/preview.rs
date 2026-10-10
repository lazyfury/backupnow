use crate::cli::PreviewArgs;
use crate::commands::{find_libs, locate};
use crate::git::Git;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// 把库的某个版本导出成可见目录（清空后重建）
pub fn export(repo: &Path, target: &Path, rev: &str) -> Result<usize> {
    // 清空目标
    if target.exists() {
        for e in std::fs::read_dir(target)? {
            let p = e?.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p)?;
            } else {
                std::fs::remove_file(&p)?;
            }
        }
    } else {
        std::fs::create_dir_all(target)
            .with_context(|| format!("无法创建预览目录: {}", target.display()))?;
    }

    // git archive 到临时 tar，再解包（不动 index，干净）
    let tmp = std::env::temp_dir().join(format!("backupnow-preview-{}.tar", std::process::id()));
    let out = Command::new("git")
        .arg("--git-dir")
        .arg(repo)
        .args(["archive", "--format=tar", "-o"])
        .arg(&tmp)
        .arg(rev)
        .output()
        .context("执行 git archive 失败")?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!(
            "git archive 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let out = Command::new("tar")
        .arg("-xf")
        .arg(&tmp)
        .arg("-C")
        .arg(target)
        .output()
        .context("执行 tar 失败")?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        bail!("tar 解包失败: {}", String::from_utf8_lossy(&out.stderr).trim());
    }

    let n = Git::bare(repo)
        .run(&["ls-tree", "-r", "--name-only", rev])
        .map(|s| s.lines().count())
        .unwrap_or(0);
    Ok(n)
}

/// 若预览目录已存在则刷新（供 save 调用）
pub fn refresh_if_exists(dest: &Path, name: &str, repo: &Path) -> Result<()> {
    let target = dest.join("previews").join(name);
    if target.is_dir() {
        export(repo, &target, "HEAD")?;
    }
    Ok(())
}

pub fn run(dest: &Path, a: PreviewArgs) -> Result<()> {
    if a.all {
        if a.name.is_some() {
            bail!("--all 与库名不能同时给出");
        }
        if a.to.is_some() {
            bail!("--all 不能与 --to 同用（每个库输出到各自 previews/<name>）");
        }
        return preview_all(dest, &a);
    }

    let name = a.name.clone().context("缺少库名（或使用 --all）")?;
    let (sf, repo) = locate(dest, &name)?;
    let target = a
        .to
        .clone()
        .unwrap_or_else(|| dest.join("previews").join(&sf.name));
    let rev = a.rev.clone().unwrap_or_else(|| "HEAD".into());
    let n = export(&repo, &target, &rev)?;
    println!("✅ 预览已生成: {}", target.display());
    println!("   版本: {rev}  文件: {n} 个");
    Ok(())
}

fn preview_all(dest: &Path, a: &PreviewArgs) -> Result<()> {
    let libs = find_libs(dest, 1);
    if libs.is_empty() {
        bail!("{} 下没有备份库", dest.display());
    }
    let rev = a.rev.clone().unwrap_or_else(|| "HEAD".into());
    let mut ok = 0;
    let mut fail = 0;
    for (sf, repo) in libs {
        let target = dest.join("previews").join(&sf.name);
        match export(&repo, &target, &rev) {
            Ok(n) => {
                ok += 1;
                println!("✅ {} → {}（{n} 文件）", sf.name, target.display());
            }
            Err(e) => {
                fail += 1;
                eprintln!("❌ {}: {e:#}", sf.name);
            }
        }
    }
    println!("\n共 {ok} 个成功，{fail} 个失败。");
    Ok(())
}
