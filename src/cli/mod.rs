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
    /// 定时任务（平台后端：macOS/launchd）
    Timer(TimerArgs),
    /// 生成 shell 补全脚本
    Completions(CompletionsArgs),
    /// 生成 man page
    Man(ManArgs),
    /// 在 Finder 打开盘上仓库
    Open(OpenArgs),
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// 目标 shell
    pub shell: clap_complete::Shell,
}

#[derive(Args, Debug)]
pub struct ManArgs {
    /// 输出目录（省略则打印到 stdout）
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct OpenArgs {
    pub name: String,
}

#[derive(Args, Debug)]
pub struct TimerArgs {
    #[command(subcommand)]
    pub action: TimerAction,
}

#[derive(Subcommand, Debug)]
pub enum TimerAction {
    /// 安装定时任务（默认 --every 1h）
    Install {
        /// 库名（省略时需配合 --all）
        name: Option<String>,
        /// 为该备份根下所有库安装一个任务
        #[arg(long)]
        all: bool,
        /// 固定间隔，如 30m / 1h / 3600s
        #[arg(long, conflicts_with_all = ["daily", "on_mount", "on_volume"])]
        every: Option<String>,
        /// 每天某时刻，如 03:00
        #[arg(long, conflicts_with_all = ["every", "on_mount", "on_volume"])]
        daily: Option<String>,
        /// 盘挂载时触发
        #[arg(long, conflicts_with_all = ["every", "daily", "on_volume"])]
        on_mount: bool,
        /// 指定卷挂载时触发（卷路径，如 /Volumes/数据）
        #[arg(long, conflicts_with_all = ["every", "daily", "on_mount"])]
        on_volume: Option<PathBuf>,
        /// 电池供电时跳过（默认不跳）
        #[arg(long)]
        skip_on_battery: bool,
    },
    /// 卸载定时任务
    Uninstall { name: String },
    /// 查看某个任务状态
    Status { name: String },
    /// 列出所有任务
    List,
    /// 内部入口：由调度器调用（含挂载/电源检查）
    #[command(hide = true)]
    Run {
        /// 库名（省略时需配合 --all）
        name: Option<String>,
        /// 备份所有库
        #[arg(long)]
        all: bool,
        /// 电池供电时也运行
        #[arg(long)]
        allow_battery: bool,
    },
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
    /// 开关 xattr 采集/还原（on/off）
    SetXattr { value: String },
    /// 开关系统通知（on/off）
    SetNotify { value: String },
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
    /// 忽略预设，逗号分隔：macos,xcode,node,python,rust
    #[arg(long, value_delimiter = ',')]
    pub preset: Vec<String>,
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
    /// 扫描所有挂载卷寻找备份库
    #[arg(long)]
    pub drives: bool,
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
    /// 库名（省略时需配合 --all）
    pub name: Option<String>,
    /// 备份根下所有库
    #[arg(long)]
    pub all: bool,
    /// 提交信息
    #[arg(short, long)]
    pub message: Option<String>,
    /// 本次不发系统通知
    #[arg(long)]
    pub quiet: bool,
    /// 只预览，不提交
    #[arg(long)]
    pub dry_run: bool,
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
    /// 库名（省略时需配合 --all）
    pub name: Option<String>,
    /// 恢复根目录下所有库（到各自 restoreDir）
    #[arg(long)]
    pub all: bool,
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
    /// 只预览，不写入
    #[arg(long)]
    pub dry_run: bool,
    /// 覆盖前先建本地 APFS 快照（tmutil localsnapshot）
    #[arg(long)]
    pub snapshot_first: bool,
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
