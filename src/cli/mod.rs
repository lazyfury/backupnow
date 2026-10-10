use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "backupnow",
    version,
    about = "把目录用 git 备份到外置盘（bare 仓库 + 自描述清单）"
)]
pub struct Cli {
    /// 备份库根目录（优先级：--dest > $BACKUPNOW_DEST > 配置文件 > /Volumes/数据/Backups）
    #[arg(long, global = true)]
    pub dest: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// 新建一个备份库
    Init(InitArgs),
    /// 列出所有备份库（扫描根目录）
    List(ListArgs),
    /// 查看某库的改动状态
    Status(StatusArgs),
    /// 快照提交（add -A + 处理忽略 + commit）
    Save(SaveArgs),
    /// 查看提交历史
    Log(LogArgs),
    /// 从备份恢复
    Restore(RestoreArgs),
    /// 管理忽略规则
    Ignore(IgnoreArgs),
    /// 自检
    Doctor(DoctorArgs),
    /// 查看/修改应用配置
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// 显示配置路径与当前解析结果
    Show,
    /// 设置默认备份根（写入 ~/.config/backupnow/config.json）
    SetDest { path: PathBuf },
    /// 清除默认备份根
    UnsetDest,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// 库名（仓库目录名 = <name>.git）
    pub name: String,
    /// 源目录
    #[arg(long)]
    pub path: PathBuf,
    /// 追加忽略规则，可多次
    #[arg(short = 'x', long = "exclude")]
    pub exclude: Vec<String>,
    /// 模式：snapshot（普通目录快照）| mirror（源目录本身是 git 仓库，镜像其历史）
    #[arg(long)]
    pub mode: Option<String>,
    /// mirror 模式下不把清单写入源目录
    #[arg(long)]
    pub no_source_manifest: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// 直接指定备份根（默认用 --dest / 配置 / 编译默认）
    pub path: Option<PathBuf>,
    /// 以 JSON 输出
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    pub name: String,
}

#[derive(Args, Debug)]
pub struct SaveArgs {
    pub name: String,
    /// 提交信息
    #[arg(short, long)]
    pub message: Option<String>,
}

#[derive(Args, Debug)]
pub struct LogArgs {
    pub name: String,
    /// 条数
    #[arg(short = 'n', long, default_value_t = 20)]
    pub limit: usize,
    /// 只看某个子路径
    #[arg(long)]
    pub path: Option<String>,
}

#[derive(Args, Debug)]
pub struct RestoreArgs {
    pub name: String,
    /// 恢复目标（默认用清单里的 restoreDir）
    #[arg(long)]
    pub to: Option<PathBuf>,
    /// 版本（默认 HEAD）
    #[arg(long)]
    pub rev: Option<String>,
    /// 只恢复某个子路径
    #[arg(long)]
    pub path: Option<String>,
    /// 目标非空时强制覆盖
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct IgnoreArgs {
    pub name: String,
    #[command(subcommand)]
    pub action: IgnoreAction,
}

#[derive(Subcommand, Debug)]
pub enum IgnoreAction {
    /// 列出规则
    List,
    /// 添加规则
    Add { pattern: String },
    /// 删除规则
    Rm { pattern: String },
    /// 用 $EDITOR 编辑
    Edit,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    pub name: String,
}
