use crate::cli::InitArgs;
use crate::config::{validate_name, AppConfig, SyncFile, SYNC_FILE};
use crate::git::{self, Git};
use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn run(app: &AppConfig, dest: &Path, a: InitArgs) -> Result<()> {
    git::ensure_git()?;
    validate_name(&a.name)?;

    let source = std::fs::canonicalize(&a.path)
        .with_context(|| format!("源目录不存在: {}", a.path.display()))?;
    if !source.is_dir() {
        bail!("源不是目录: {}", source.display());
    }

    std::fs::create_dir_all(dest)
        .with_context(|| format!("无法创建备份根: {}", dest.display()))?;

    let repo = dest.join(format!("{}.git", a.name));
    if repo.exists() {
        bail!("库已存在: {}", repo.display());
    }

    // 决定模式：显式 --mode > 自动识别（源目录有 .git 则 mirror）
    let mode = match a.mode.as_deref() {
        Some(m) => m.to_string(),
        None if git::is_git_repo(&source) => "mirror".into(),
        None => "snapshot".into(),
    };
    if mode != "snapshot" && mode != "mirror" {
        bail!("未知模式 `{mode}`（可选 snapshot | mirror）");
    }
    if mode == "mirror" && !git::is_git_repo(&source) {
        bail!(
            "mirror 模式要求源目录本身是 git 仓库（{} 下没有 .git）",
            source.display()
        );
    }

    let source_manifest = !a.no_source_manifest;

    if mode == "mirror" {
        git::clone_mirror(&source, &repo)?;
    } else {
        git::init_bare(&repo)?;
    }

    let mut ignore = app.default_ignore.clone();
    for x in &a.exclude {
        if !ignore.contains(x) {
            ignore.push(x.clone());
        }
    }
    for p in &a.preset {
        match crate::config::preset(p) {
            Some(pats) => {
                for pat in pats {
                    let s = pat.to_string();
                    if !ignore.contains(&s) {
                        ignore.push(s);
                    }
                }
            }
            None => bail!("未知预设: {p}（可选 macos,xcode,node,python,rust）"),
        }
    }

    let sf = SyncFile::new(a.name.clone(), &source, ignore, &mode, source_manifest);
    sf.write_all(&repo)?;
    sf.write_exclude(&repo)?;

    match mode.as_str() {
        "snapshot" => {
            let g = Git::worktree(&repo, &source);
            g.run(&["add", "-A"])?;
            let msg = format!("init: {} @ {}", a.name, sf.created_at);
            g.run(&["commit", "-m", &msg])?;
            if app.xattr {
                let files = g.ls_files()?;
                let head = g.head()?;
                let _ = crate::commands::sync_xattr_note(
                    &g,
                    &files,
                    &source,
                    &app.xattr_ignore,
                    &head,
                );
            }
        }
        "mirror" => {
            let g = Git::bare(&repo);
            let _ = g.run(&["remote", "update", "--prune"]);
            if app.xattr {
                if let Ok(head) = git::repo_head(&source) {
                    let files = git::repo_ls_files(&source).unwrap_or_default();
                    let _ = crate::commands::sync_xattr_note(
                        &g,
                        &files,
                        &source,
                        &app.xattr_ignore,
                        &head,
                    );
                }
            }
        }
        _ => unreachable!(),
    }

    let g = Git::bare(&repo);
    let refs = g
        .run(&["for-each-ref", "--format=%(refname)"])
        .unwrap_or_default()
        .lines()
        .count();
    let head = g
        .run(&["rev-parse", "--short", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();

    println!("✅ 已创建库 `{}`  (模式: {mode})", a.name);
    println!("   源目录   : {}", source.display());
    println!("   仓库     : {}", repo.display());
    println!("   清单     : {}", repo.join(SYNC_FILE).display());
    if source_manifest {
        println!("              {}", source.join(SYNC_FILE).display());
    } else {
        println!("              （--no-source-manifest：不写源目录）");
    }
    if mode == "snapshot" {
        println!("   首次快照 : {head}");
        println!("   忽略规则 : {}", sf.ignore.join(", "));
    } else {
        println!("   镜像 refs: {refs}");
    }
    Ok(())
}
