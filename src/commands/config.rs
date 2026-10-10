use crate::cli::{ConfigAction, ConfigArgs};
use crate::config::{config_file_path, write_config_file, AppConfig};
use anyhow::{bail, Result};

pub fn run(app: &AppConfig, a: ConfigArgs) -> Result<()> {
    match a.action {
        ConfigAction::Show => {
            let (path, exists) = match config_file_path() {
                Some(p) => (p.display().to_string(), p.exists()),
                None => ("（$HOME 未设置）".into(), false),
            };
            println!("配置文件  : {path}{}", if exists { "" } else { "  (不存在)" });
            println!("备份根    : {}", app.dest.display());
            println!("默认忽略  : {}", app.default_ignore.join(", "));
            println!("xattr     : {}", if app.xattr { "开启" } else { "关闭" });
            if !app.xattr_ignore.is_empty() {
                println!("忽略 xattr: {}", app.xattr_ignore.join(", "));
            }
        }
        ConfigAction::SetDest { path } => {
            write_config_file(Some(&path), app)?;
            println!("✅ 已设置默认备份根: {}", path.display());
        }
        ConfigAction::UnsetDest => {
            write_config_file(None, app)?;
            println!("✅ 已清除默认备份根（回退到 $BACKUPNOW_DEST / 编译默认）");
        }
        ConfigAction::SetXattr { value } => {
            let on = match value.as_str() {
                "on" | "true" | "1" | "yes" => true,
                "off" | "false" | "0" | "no" => false,
                _ => bail!("参数应为 on/off，收到: {value}"),
            };
            let mut next = app.clone();
            next.xattr = on;
            write_config_file(Some(&app.dest), &next)?;
            println!("✅ xattr 采集/还原已{}", if on { "开启" } else { "关闭" });
        }
    }
    Ok(())
}
