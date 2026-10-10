use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SYNC_FILE: &str = "backupnow_sync.json";
pub const DEFAULT_DEST: &str = "/Volumes/数据/Backups";
pub const DEFAULT_IGNORE: &[&str] = &[".DS_Store", "agent/install", "node_modules", "agent/sessions"];

/// 应用级默认值（~/.config/backupnow/config.json）
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub dest: PathBuf,
    pub default_ignore: Vec<String>,
}

impl AppConfig {
    pub fn load(cli_dest: Option<PathBuf>) -> Self {
        let mut dest: Option<PathBuf> = None;
        let mut ignore: Vec<String> = DEFAULT_IGNORE.iter().map(|s| s.to_string()).collect();

        if let Some(path) = config_file_path() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(d) = v.get("dest").and_then(|x| x.as_str()) {
                        dest = Some(PathBuf::from(d));
                    }
                    if let Some(arr) = v.get("ignore").and_then(|x| x.as_array()) {
                        let list: Vec<String> = arr
                            .iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect();
                        if !list.is_empty() {
                            ignore = list;
                        }
                    }
                }
            }
        }

        if let Ok(e) = std::env::var("BACKUPNOW_DEST") {
            if !e.trim().is_empty() {
                dest = Some(PathBuf::from(e));
            }
        }
        if let Some(d) = cli_dest {
            dest = Some(d);
        }

        Self {
            dest: dest.unwrap_or_else(|| PathBuf::from(DEFAULT_DEST)),
            default_ignore: ignore,
        }
    }
}

pub fn config_file_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(Path::new(&home).join(".config/backupnow/config.json"))
}

/// 写入配置文件。`dest = None` 表示移除 dest 字段。
pub fn write_config_file(dest: Option<&Path>, ignore: &[String]) -> Result<()> {
    let path = config_file_path().context("无法确定配置路径（$HOME 未设置？）")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建配置目录: {}", parent.display()))?;
    }
    let mut obj = serde_json::Map::new();
    if let Some(d) = dest {
        obj.insert("dest".into(), serde_json::Value::String(d.to_string_lossy().into()));
    }
    obj.insert(
        "ignore".into(),
        serde_json::Value::Array(ignore.iter().map(|s| serde_json::Value::String(s.clone())).collect()),
    );
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(obj))? + "\n";
    std::fs::write(&path, text).with_context(|| format!("写入配置失败: {}", path.display()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub host: String,
    pub user: String,
    pub path: String,
    #[serde(rename = "pathTemplate")]
    pub path_template: String,
}

/// 仓库内 / 源目录内的 backupnow_sync.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFile {
    pub schema: u32,
    pub name: String,
    pub engine: String,
    pub repo: String,
    /// snapshot = 把普通目录快照进 bare 仓库；mirror = 镜像本身就是 git 仓库的源
    #[serde(default = "default_mode")]
    pub mode: String,
    /// 是否把清单写入源目录（mirror 模式下可关掉，避免污染项目）
    #[serde(default = "default_true", rename = "sourceManifest")]
    pub source_manifest: bool,
    pub source: SourceRef,
    #[serde(rename = "restoreDir")]
    pub restore_dir: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub ignore: Vec<String>,
}

fn default_mode() -> String {
    "snapshot".into()
}

fn default_true() -> bool {
    true
}

impl SyncFile {
    pub fn new(
        name: String,
        source: &Path,
        ignore: Vec<String>,
        mode: &str,
        source_manifest: bool,
    ) -> Self {
        let path = source.to_string_lossy().to_string();
        Self {
            schema: 1,
            name: name.clone(),
            engine: "git".into(),
            repo: format!("{name}.git"),
            mode: mode.to_string(),
            source_manifest,
            source: SourceRef {
                host: hostname(),
                user: username(),
                path_template: templatize(&path),
                path,
            },
            restore_dir: source.to_string_lossy().to_string(),
            created_at: now_rfc3339(),
            ignore,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("读取清单失败: {}", path.display()))?;
        let sf: SyncFile = serde_json::from_str(&text)
            .with_context(|| format!("解析清单失败: {}", path.display()))?;
        Ok(sf)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = serde_json::to_string_pretty(self)? + "\n";
        std::fs::write(path, text).with_context(|| format!("写入清单失败: {}", path.display()))
    }

    /// 同时写入仓库目录与源目录（后者可被 sourceManifest 关掉）
    pub fn write_all(&self, repo_dir: &Path) -> Result<()> {
        self.write(&repo_dir.join(SYNC_FILE))?;
        if self.source_manifest && Path::new(&self.source.path).is_dir() {
            self.write(&Path::new(&self.source.path).join(SYNC_FILE))?;
        }
        Ok(())
    }

    /// 生成 <repo>/info/exclude，供 git 忽略规则（仅 snapshot 模式使用）
    pub fn write_exclude(&self, repo_dir: &Path) -> Result<()> {
        if self.mode != "snapshot" {
            return Ok(());
        }
        let mut text = String::from("# 由 backupnow 生成；请用 `backupnow ignore` 管理\n");
        for p in &self.ignore {
            text.push_str(p);
            text.push('\n');
        }
        let info = repo_dir.join("info");
        std::fs::create_dir_all(&info).ok();
        std::fs::write(info.join("exclude"), text)
            .with_context(|| format!("写入 exclude 失败: {}", info.display()))
    }

    /// 默认恢复目标。
    ///
    /// 同机上 `restoreDir` 存在则用它；否则退回可移植的 `pathTemplate`
    /// （`{HOME}/.pi`），保证换机器/换用户名也能落到正确的家目录。
    pub fn resolved_restore_dir(&self) -> PathBuf {
        let rd = expand_home(&self.restore_dir);
        let templated = self.source.path_template.contains("{HOME}");
        if Path::new(&rd).exists() || !templated {
            PathBuf::from(rd)
        } else {
            PathBuf::from(expand_home(&self.source.path_template))
        }
    }
}

pub fn expand_home(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("{HOME}") {
        if let Ok(h) = std::env::var("HOME") {
            return format!("{h}{rest}");
        }
    }
    p.to_string()
}

fn templatize(path: &str) -> String {
    if let Ok(h) = std::env::var("HOME") {
        if let Some(rest) = path.strip_prefix(&h) {
            return format!("{{HOME}}{rest}");
        }
    }
    path.to_string()
}

pub fn hostname() -> String {
    if let Ok(out) = std::process::Command::new("hostname").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into())
}

pub fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

pub fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("库名不能为空");
    }
    if name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        bail!("非法库名: {name}");
    }
    Ok(())
}
