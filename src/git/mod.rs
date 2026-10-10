use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn ensure_git() -> Result<()> {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|_| ())
        .context("找不到 git，请先安装")
}

/// `git init --bare`，得到纯净 bare 仓库
pub fn init_bare(repo: &Path) -> Result<()> {
    let out = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(repo)
        .output()
        .context("执行 git init --bare 失败")?;
    if !out.status.success() {
        bail!(
            "git init --bare 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    // 保持 core.bare=true：CLI 每次显式传 --work-tree 即可 add/commit，
    // 同时仓库仍是标准 bare，可 clone / push / 网络服务。
    Ok(())
}

/// 调用 git 的封装。work_tree 为 None 时只传 --git-dir（log/rev-parse 等用）。
pub struct Git {
    git_dir: PathBuf,
    work_tree: Option<PathBuf>,
}

impl Git {
    pub fn bare(git_dir: impl Into<PathBuf>) -> Self {
        Self {
            git_dir: git_dir.into(),
            work_tree: None,
        }
    }

    pub fn worktree(git_dir: impl Into<PathBuf>, work_tree: impl Into<PathBuf>) -> Self {
        Self {
            git_dir: git_dir.into(),
            work_tree: Some(work_tree.into()),
        }
    }

    fn base(&self) -> Command {
        let mut c = Command::new("git");
        c.arg("--git-dir").arg(&self.git_dir);
        if let Some(w) = &self.work_tree {
            c.arg("--work-tree").arg(w);
        }
        c
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        let out = self
            .base()
            .args(args)
            .output()
            .with_context(|| format!("执行 git {} 失败", args.join(" ")))?;
        if !out.status.success() {
            bail!(
                "git {} 失败 (exit {:?}): {}",
                args.join(" "),
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// 返回是否成功（用于 --quiet 类判断）
    pub fn success(&self, args: &[&str]) -> Result<bool> {
        let out = self.base().args(args).output()?;
        Ok(out.status.success())
    }

    pub fn run_with_stdin(&self, args: &[&str], input: &[u8]) -> Result<String> {
        let mut child = self
            .base()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("启动 git 失败")?;
        child
            .stdin
            .as_mut()
            .context("无法写入 git stdin")?
            .write_all(input)?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!(
                "git {} 失败: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// 完整 HEAD sha
    pub fn head(&self) -> Result<String> {
        Ok(self.run(&["rev-parse", "HEAD"])?.trim().to_string())
    }

    /// 追踪的相对路径列表
    pub fn ls_files(&self) -> Result<Vec<String>> {
        let out = self.run(&["ls-files", "-z"])?;
        Ok(out
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect())
    }

    /// 成功返回 stdout，失败返回 None（不报错）
    pub fn try_run(&self, args: &[&str]) -> Result<Option<String>> {
        let out = self.base().args(args).output()?;
        if out.status.success() {
            Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
        } else {
            Ok(None)
        }
    }

    /// 写 git note（-F - 从 stdin 读）
    pub fn notes_add(&self, refname: &str, commit: &str, content: &str) -> Result<()> {
        self.run_with_stdin(
            &[
                "notes",
                &format!("--ref={refname}"),
                "add",
                "-f",
                "-F",
                "-",
                commit,
            ],
            content.as_bytes(),
        )?;
        Ok(())
    }

    /// 读 git note
    pub fn notes_show(&self, refname: &str, commit: &str) -> Result<Option<String>> {
        self.try_run(&["notes", &format!("--ref={refname}"), "show", commit])
    }
}

/// `git clone --mirror <source> <dest>`，用于镜像已有 git 仓库
pub fn clone_mirror(source: &Path, dest: &Path) -> Result<()> {
    let out = Command::new("git")
        .arg("clone")
        .arg("--mirror")
        .arg(source)
        .arg(dest)
        .output()
        .context("执行 git clone --mirror 失败")?;
    if !out.status.success() {
        bail!(
            "git clone --mirror 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

/// 检查一个源目录是否是 git 仓库
pub fn is_git_repo(source: &Path) -> bool {
    source.join(".git").exists()
}

/// 统计源仓库未提交改动数（git status --porcelain），忽略工具自身写入的清单
pub fn dirty_count(source: &Path) -> usize {
    Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["status", "--porcelain"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter(|l| !l.contains(crate::config::SYNC_FILE))
                .count()
        })
        .unwrap_or(0)
}

/// 某个仓库的 HEAD 完整 sha（git -C <repo> rev-parse HEAD）
pub fn repo_head(repo: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("执行 git rev-parse HEAD 失败")?;
    if !out.status.success() {
        bail!(
            "rev-parse HEAD 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 某个仓库追踪的相对路径（git -C <repo> ls-files -z）
pub fn repo_ls_files(repo: &Path) -> Result<Vec<String>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "-z"])
        .output()
        .context("执行 git ls-files 失败")?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect())
}
