use super::{JobSpec, JobStatus, Schedule, Scheduler};
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

/// macOS：launchd LaunchAgent
pub struct LaunchdScheduler;

impl LaunchdScheduler {
    pub fn new() -> Self {
        Self
    }

    fn home() -> Result<PathBuf> {
        Ok(PathBuf::from(std::env::var("HOME").context("$HOME 未设置")?))
    }

    fn launch_agents_dir() -> Result<PathBuf> {
        Ok(Self::home()?.join("Library/LaunchAgents"))
    }

    fn log_dir() -> Result<PathBuf> {
        Ok(Self::home()?.join("Library/Logs/backupnow"))
    }

    fn plist_path(name: &str) -> Result<PathBuf> {
        Ok(Self::launch_agents_dir()?.join(format!("{}.plist", JobSpec::label_for(name))))
    }

    fn uid() -> u32 {
        unsafe { libc::getuid() }
    }

    fn domain() -> String {
        format!("gui/{}", Self::uid())
    }
}

impl Scheduler for LaunchdScheduler {
    fn platform(&self) -> &'static str {
        "macos (launchd)"
    }

    fn install(&self, job: &JobSpec) -> Result<()> {
        let agents = Self::launch_agents_dir()?;
        std::fs::create_dir_all(&agents)?;
        let log_dir = Self::log_dir()?;
        std::fs::create_dir_all(&log_dir)?;

        let out_log = log_dir.join(format!("{}.out.log", job.name));
        let err_log = log_dir.join(format!("{}.err.log", job.name));
        let plist = Self::plist_path(&job.name)?;
        std::fs::write(&plist, plist_xml(job, &out_log, &err_log))
            .with_context(|| format!("写入 plist 失败: {}", plist.display()))?;

        // 先卸载旧的（忽略错误），再加载
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("{}/{}", Self::domain(), job.label())])
            .output();

        let out = Command::new("launchctl")
            .args(["bootstrap", &Self::domain(), &plist.to_string_lossy()])
            .output()
            .context("执行 launchctl bootstrap 失败")?;
        if !out.status.success() {
            bail!(
                "launchctl bootstrap 失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    fn uninstall(&self, name: &str) -> Result<()> {
        let label = JobSpec::label_for(name);
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("{}/{label}", Self::domain())])
            .output();
        let plist = Self::plist_path(name)?;
        if plist.exists() {
            std::fs::remove_file(&plist)
                .with_context(|| format!("删除 plist 失败: {}", plist.display()))?;
        }
        Ok(())
    }

    fn status(&self, name: &str) -> Result<JobStatus> {
        let label = JobSpec::label_for(name);
        let plist = Self::plist_path(name)?;
        if !plist.exists() {
            return Ok(JobStatus::NotInstalled);
        }
        let schedule = std::fs::read_to_string(&plist)
            .ok()
            .map(|t| describe_plist(&t));
        let loaded = Command::new("launchctl")
            .args(["print", &format!("{}/{label}", Self::domain())])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        Ok(JobStatus::Installed {
            label,
            schedule,
            loaded,
        })
    }

    fn list(&self) -> Result<Vec<String>> {
        let dir = Self::launch_agents_dir()?;
        let mut names = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let f = e.file_name().to_string_lossy().to_string();
                if let Some(rest) = f.strip_prefix("com.backupnow.timer.") {
                    if let Some(name) = rest.strip_suffix(".plist") {
                        names.push(name.to_string());
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    fn log_hint(&self, name: &str) -> Option<String> {
        Self::log_dir()
            .ok()
            .map(|d| format!("{}/", d.join(name).display()))
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn plist_xml(job: &JobSpec, out_log: &std::path::Path, err_log: &std::path::Path) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
    s.push_str("<plist version=\"1.0\">\n<dict>\n");
    s.push_str(&format!(
        "  <key>Label</key><string>{}</string>\n",
        xml_escape(&job.label())
    ));

    s.push_str("  <key>ProgramArguments</key>\n  <array>\n");
    let args = {
        let mut v = vec![
            job.binary.to_string_lossy().to_string(),
            "timer".to_string(),
            "run".to_string(),
        ];
        if job.all {
            v.push("--all".to_string());
        } else {
            v.push(job.name.clone());
        }
        v.push("--dest".to_string());
        v.push(job.dest.to_string_lossy().to_string());
        if job.allow_battery {
            v.push("--allow-battery".to_string());
        }
        v
    };
    for a in args {
        s.push_str(&format!("    <string>{}</string>\n", xml_escape(&a)));
    }
    s.push_str("  </array>\n");

    match &job.schedule {
        Schedule::Interval(secs) => {
            s.push_str(&format!("  <key>StartInterval</key><integer>{secs}</integer>\n"));
        }
        Schedule::Daily { hour, minute } => {
            s.push_str("  <key>StartCalendarInterval</key>\n  <dict>\n");
            s.push_str(&format!("    <key>Hour</key><integer>{hour}</integer>\n"));
            s.push_str(&format!("    <key>Minute</key><integer>{minute}</integer>\n"));
            s.push_str("  </dict>\n");
        }
        Schedule::OnMount => {
            s.push_str("  <key>StartOnMount</key><true/>\n");
        }
        Schedule::OnVolume(p) => {
            let watch = p.parent().unwrap_or(std::path::Path::new("/Volumes"));
            s.push_str("  <key>WatchPaths</key>\n  <array>\n");
            s.push_str(&format!(
                "    <string>{}</string>\n",
                xml_escape(&watch.to_string_lossy())
            ));
            s.push_str("  </array>\n");
        }
    }

    s.push_str("  <key>ProcessType</key><string>Background</string>\n");
    s.push_str("  <key>LowPriorityIO</key><true/>\n");
    s.push_str("  <key>Nice</key><integer>5</integer>\n");
    s.push_str(&format!(
        "  <key>StandardOutPath</key><string>{}</string>\n",
        xml_escape(&out_log.to_string_lossy())
    ));
    s.push_str(&format!(
        "  <key>StandardErrorPath</key><string>{}</string>\n",
        xml_escape(&err_log.to_string_lossy())
    ));
    s.push_str("</dict>\n</plist>\n");
    s
}

/// 从 plist 文本里粗略还原调度描述
fn describe_plist(text: &str) -> String {
    if text.contains("WatchPaths") {
        return "卷挂载时".into();
    }
    if text.contains("StartOnMount") {
        return "盘挂载时".into();
    }
    if text.contains("StartCalendarInterval") {
        let hour = extract_int(text, "Hour");
        let minute = extract_int(text, "Minute");
        if let (Some(h), Some(m)) = (hour, minute) {
            return format!("每天 {h:02}:{m:02}");
        }
        return "每天（自定义）".into();
    }
    if let Some(secs) = extract_int(text, "StartInterval") {
        return format!("每 {}", super::human_duration(secs));
    }
    "（未知）".into()
}

fn extract_int(text: &str, key: &str) -> Option<u64> {
    let marker = format!("<key>{key}</key><integer>");
    let start = text.find(&marker)? + marker.len();
    let end = text[start..].find("</integer>")? + start;
    text[start..end].trim().parse().ok()
}
