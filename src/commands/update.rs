use crate::cli::UpdateArgs;
use crate::version;
use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn run(a: UpdateArgs) -> Result<()> {
    if Command::new("cargo").arg("--version").output().is_err() {
        bail!("找不到 cargo，无法自更新（请先安装 Rust 工具链）");
    }

    if a.check {
        return check(&a);
    }

    println!("当前: backupnow {}", version::VERSION);

    let mut args: Vec<String> = vec![
        "install".into(),
        "--git".into(),
        a.git.clone(),
        "--force".into(),
    ];
    if let Some(b) = &a.branch {
        args.push("--branch".into());
        args.push(b.clone());
    }

    println!("$ cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(&args)
        .status()
        .context("执行 cargo install 失败")?;
    if !status.success() {
        bail!("cargo install 失败（exit {:?}）", status.code());
    }
    println!("✅ 更新完成，重新运行 `backupnow --version` 查看新版本。");
    Ok(())
}

fn check(a: &UpdateArgs) -> Result<()> {
    let out = Command::new("git")
        .args(["ls-remote", &a.git, "HEAD"])
        .output()
        .context("执行 git ls-remote 失败")?;
    if !out.status.success() {
        bail!(
            "git ls-remote 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let remote = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string();

    println!("本地提交: {}", version::BUILD_COMMIT);
    println!("远端 HEAD: {}", &remote[..remote.len().min(7)]);

    if version::BUILD_COMMIT != "unknown" && remote.starts_with(version::BUILD_COMMIT) {
        println!("已是最新。");
    } else {
        println!("有更新可用：`backupnow update`");
    }
    Ok(())
}
