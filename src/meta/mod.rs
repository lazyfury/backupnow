use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// 存 xattr 清单的 git note ref（完整名 refs/notes/backupnow/xattr）
pub const NOTE_REF: &str = "backupnow/xattr";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub v: u32,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    /// 相对路径
    pub p: String,
    /// xattr 名 -> base64(值)
    pub x: BTreeMap<String, String>,
}

impl Manifest {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// 只保留某个子路径下的条目
    pub fn under(&self, prefix: Option<&str>) -> Manifest {
        match prefix {
            None => Manifest {
                v: self.v,
                entries: self
                    .entries
                    .iter()
                    .map(|e| Entry {
                        p: e.p.clone(),
                        x: e.x.clone(),
                    })
                    .collect(),
            },
            Some(pre) => {
                let pre = pre.trim_end_matches('/');
                Manifest {
                    v: self.v,
                    entries: self
                        .entries
                        .iter()
                        .filter(|e| e.p == pre || e.p.starts_with(&format!("{pre}/")))
                        .map(|e| Entry {
                            p: e.p.clone(),
                            x: e.x.clone(),
                        })
                        .collect(),
                }
            }
        }
    }
}

fn is_ignored(name: &str, ignore: &[String]) -> bool {
    ignore.iter().any(|i| i == name)
}

/// 采集 root 下给定相对路径的 xattr，只保留有 xattr 的文件
pub fn capture(root: &Path, rel_paths: &[String], ignore: &[String]) -> Manifest {
    let mut entries = Vec::new();
    for rel in rel_paths {
        let abs = root.join(rel);
        let names = match xattr::list(&abs) {
            Ok(it) => it,
            Err(_) => continue,
        };
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for name in names {
            let name_str = name.to_string_lossy().to_string();
            if is_ignored(&name_str, ignore) {
                continue;
            }
            if let Ok(Some(val)) = xattr::get(&abs, &name) {
                map.insert(
                    name_str,
                    base64::engine::general_purpose::STANDARD.encode(&val),
                );
            }
        }
        if !map.is_empty() {
            entries.push(Entry {
                p: rel.clone(),
                x: map,
            });
        }
    }
    entries.sort_by(|a, b| a.p.cmp(&b.p));
    Manifest { v: 1, entries }
}

/// 把清单写回 root，返回 (成功数, 警告)
pub fn apply(root: &Path, manifest: &Manifest) -> (usize, Vec<String>) {
    let mut ok = 0;
    let mut warns = Vec::new();
    for e in &manifest.entries {
        let abs = root.join(&e.p);
        if !abs.exists() {
            continue;
        }
        for (name, b64) in &e.x {
            match base64::engine::general_purpose::STANDARD.decode(b64) {
                Ok(val) => match xattr::set(&abs, name, &val) {
                    Ok(()) => ok += 1,
                    Err(err) => warns.push(format!("{}: {name}: {err}", e.p)),
                },
                Err(err) => warns.push(format!("{}: {name}: base64 解码失败: {err}", e.p)),
            }
        }
    }
    (ok, warns)
}
