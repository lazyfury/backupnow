use crate::cli::DoctorArgs;
use crate::commands::locate;
use crate::config::{AppConfig, SYNC_FILE};
use crate::git::{self, Git};
use anyhow::Result;
use std::path::Path;

pub fn run(_app: &AppConfig, dest: &Path, a: DoctorArgs) -> Result<()> {
    let mut problems = 0;

    let git_ok = git::ensure_git().is_ok();
    report("git 可用", git_ok);
    if !git_ok {
        problems += 1;
    }

    report(
        &format!("备份根存在 ({})", dest.display()),
        dest.is_dir(),
    );

    let (sf, repo) = match locate(dest, &a.name) {
        Ok(v) => v,
        Err(e) => {
            println!("❌ {}", e);
            return Ok(());
        }
    };

    report(&format!("仓库目录存在 ({})", repo.display()), repo.is_dir());
    report(
        &format!("清单存在 ({})", repo.join(SYNC_FILE).display()),
        repo.join(SYNC_FILE).exists(),
    );
    report(
        &format!("源目录存在 ({})", sf.source.path),
        Path::new(&sf.source.path).is_dir(),
    );

    let exclude = repo.join("info/exclude");
    report("info/exclude 存在", exclude.exists());

    if exclude.exists() {
        let on_disk = std::fs::read_to_string(&exclude).unwrap_or_default();
        let expected: String = sf
            .ignore
            .iter()
            .map(|p| format!("{p}\n"))
            .collect();
        let on_disk_rules: String = on_disk
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .map(|l| format!("{l}\n"))
            .collect();
        report("忽略规则与清单一致", on_disk_rules == expected);
        if on_disk_rules != expected {
            problems += 1;
            println!("   （执行 `backupnow save {}` 可自动校准）", sf.name);
        }
    }

    if Path::new(&sf.source.path).is_dir() && repo.is_dir() {
        let g = Git::worktree(&repo, &sf.source.path);
        match g.success(&["rev-parse", "--verify", "HEAD"]) {
            Ok(true) => report("已有提交", true),
            _ => {
                report("已有提交", false);
                problems += 1;
            }
        }
    }

    // 源目录里的副本
    let src_copy = Path::new(&sf.source.path).join(SYNC_FILE);
    report(
        &format!("源目录清单副本 ({})", src_copy.display()),
        src_copy.exists(),
    );

    println!();
    if problems == 0 {
        println!("一切正常。");
    } else {
        println!("发现 {problems} 个问题。");
    }
    Ok(())
}

fn report(label: &str, ok: bool) {
    println!("{} {}", if ok { "✅" } else { "❌" }, label);
}
