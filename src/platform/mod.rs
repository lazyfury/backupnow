use anyhow::{bail, Context, Result};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub mod macos;
pub mod notify;

/// 调度方式（与平台无关的描述）
#[derive(Debug, Clone)]
pub enum Schedule {
    /// 固定间隔（秒）
    Interval(u64),
    /// 每天某个时刻
    Daily { hour: u8, minute: u8 },
    /// 盘挂载时触发
    OnMount,
}

impl Schedule {
    pub fn describe(&self) -> String {
        match self {
            Schedule::Interval(s) => format!("每 {}", human_duration(*s)),
            Schedule::Daily { hour, minute } => format!("每天 {hour:02}:{minute:02}"),
            Schedule::OnMount => "盘挂载时".into(),
        }
    }
}

/// 一个待安装的定时任务
#[derive(Debug, Clone)]
pub struct JobSpec {
    pub name: String,
    pub dest: PathBuf,
    pub binary: PathBuf,
    pub schedule: Schedule,
}

impl JobSpec {
    pub fn label_for(name: &str) -> String {
        format!("com.backupnow.timer.{name}")
    }
    pub fn label(&self) -> String {
        Self::label_for(&self.name)
    }
}

#[derive(Debug, Clone)]
pub enum JobStatus {
    Installed {
        label: String,
        schedule: Option<String>,
        loaded: bool,
    },
    NotInstalled,
}

/// 平台调度后端。加 Linux/Windows 时实现这个 trait 即可。
pub trait Scheduler {
    /// 后端名（macos / linux / windows / unsupported）
    fn platform(&self) -> &'static str;
    /// 安装（覆盖已存在的同名任务）
    fn install(&self, job: &JobSpec) -> Result<()>;
    /// 卸载
    fn uninstall(&self, name: &str) -> Result<()>;
    /// 查询状态
    fn status(&self, name: &str) -> Result<JobStatus>;
    /// 列出所有 backupnow 任务名
    fn list(&self) -> Result<Vec<String>>;
    /// 日志文件（展示用）
    fn log_hint(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
}

/// 选择当前平台的后端
pub fn scheduler() -> Box<dyn Scheduler> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::LaunchdScheduler::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(UnsupportedScheduler)
    }
}

/// 尚未支持的平台
#[allow(dead_code)]
pub struct UnsupportedScheduler;

impl Scheduler for UnsupportedScheduler {
    fn platform(&self) -> &'static str {
        "unsupported"
    }
    fn install(&self, _job: &JobSpec) -> Result<()> {
        bail!(
            "当前平台 `{}` 暂不支持定时任务（目前仅实现 macOS/launchd）",
            std::env::consts::OS
        )
    }
    fn uninstall(&self, _name: &str) -> Result<()> {
        bail!("当前平台暂不支持定时任务")
    }
    fn status(&self, _name: &str) -> Result<JobStatus> {
        Ok(JobStatus::NotInstalled)
    }
    fn list(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

pub fn human_duration(secs: u64) -> String {
    if secs % 86400 == 0 {
        format!("{}d", secs / 86400)
    } else if secs % 3600 == 0 {
        format!("{}h", secs / 3600)
    } else if secs % 60 == 0 {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// 解析 "30m" / "1h" / "90s" / "2d" / "3600"
pub fn parse_duration(s: &str) -> Result<u64> {
    let t = s.trim().to_lowercase();
    if t.is_empty() {
        bail!("时间间隔不能为空");
    }
    let (digits, mult) = if let Some(x) = t.strip_suffix('s') {
        (x, 1u64)
    } else if let Some(x) = t.strip_suffix('m') {
        (x, 60)
    } else if let Some(x) = t.strip_suffix('h') {
        (x, 3600)
    } else if let Some(x) = t.strip_suffix('d') {
        (x, 86400)
    } else {
        (t.as_str(), 1)
    };
    let n: u64 = digits
        .trim()
        .parse()
        .with_context(|| format!("无法解析时间间隔: {s}（示例 30m / 1h / 3600s）"))?;
    if n == 0 {
        bail!("时间间隔必须大于 0");
    }
    Ok(n * mult)
}

/// 解析 "HH:MM"
pub fn parse_daily(s: &str) -> Result<(u8, u8)> {
    let (h, m) = s
        .split_once(':')
        .with_context(|| format!("时间格式应为 HH:MM，收到: {s}"))?;
    let hour: u8 = h.trim().parse().context("小时解析失败")?;
    let minute: u8 = m.trim().parse().context("分钟解析失败")?;
    if hour > 23 || minute > 59 {
        bail!("时间超出范围: {s}");
    }
    Ok((hour, minute))
}
