use crate::cli::{ConfigAction, ConfigArgs};
use crate::config::{config_file_path, write_config_file, AppConfig};
use anyhow::Result;

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
        }
        ConfigAction::SetDest { path } => {
            write_config_file(Some(&path), &app.default_ignore)?;
            println!("✅ 已设置默认备份根: {}", path.display());
        }
        ConfigAction::UnsetDest => {
            write_config_file(None, &app.default_ignore)?;
            println!("✅ 已清除默认备份根（回退到 $BACKUPNOW_DEST / 编译默认）");
        }
    }
    Ok(())
}
