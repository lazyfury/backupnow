use crate::cli::{SaveArgs, TimerAction, TimerArgs};
use crate::commands::{find_libs, locate, save};
use crate::config::AppConfig;
use crate::platform::{self, JobSpec, JobStatus, Schedule};
use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn run(app: &AppConfig, dest: &Path, a: TimerArgs) -> Result<()> {
    match a.action {
        TimerAction::Install {
            name,
            all,
            every,
            daily,
            on_mount,
            on_volume,
            skip_on_battery,
        } => {
            if all && name.is_some() {
                bail!("--all 与库名不能同时给出");
            }
            let job_name = if all {
                if find_libs(dest, 1).is_empty() {
                    bail!("{} 下没有备份库", dest.display());
                }
                "all".to_string()
            } else {
                let n = name.context("缺少库名（或使用 --all）")?;
                locate(dest, &n)?.0.name
            };

            let schedule = build_schedule(every, daily, on_mount, on_volume)?;
            let binary = std::env::current_exe().context("无法确定自身可执行文件路径")?;
            let job = JobSpec {
                name: job_name.clone(),
                dest: dest.to_path_buf(),
                binary,
                schedule: schedule.clone(),
                allow_battery: !skip_on_battery,
                all,
            };
            let sched = platform::scheduler();
            sched.install(&job)?;

            println!(
                "✅ 已安装定时任务 `{job_name}`（后端: {}）",
                sched.platform()
            );
            println!("   范围     : {}", if all { "全部库" } else { "单个库" });
            println!("   调度     : {}", schedule.describe());
            println!(
                "   电源     : {}",
                if skip_on_battery {
                    "电池时跳过"
                } else {
                    "不限"
                }
            );
            if let Some(hint) = sched.log_hint(&job_name) {
                println!("   日志     : {hint}");
            }
        }
        TimerAction::Uninstall { name } => {
            let sched = platform::scheduler();
            sched.uninstall(&name)?;
            println!("✅ 已卸载定时任务 `{name}`");
        }
        TimerAction::Status { name } => {
            let sched = platform::scheduler();
            match sched.status(&name)? {
                JobStatus::Installed {
                    label,
                    schedule,
                    loaded,
                } => {
                    println!("任务     : {name}");
                    println!("状态     : 已安装");
                    println!("label    : {label}");
                    if let Some(s) = schedule {
                        println!("调度     : {s}");
                    }
                    println!("launchd  : {}", if loaded { "已加载" } else { "未加载" });
                    if let Some(hint) = sched.log_hint(&name) {
                        println!("日志     : {hint}");
                    }
                }
                JobStatus::NotInstalled => {
                    println!("任务     : {name}");
                    println!("状态     : 未安装");
                }
            }
        }
        TimerAction::List => {
            let sched = platform::scheduler();
            let names = sched.list()?;
            if names.is_empty() {
                println!("（没有已安装的定时任务，后端: {}）", sched.platform());
                return Ok(());
            }
            println!("{:<14} {}", "任务", "调度");
            for n in names {
                let desc = match sched.status(&n)? {
                    JobStatus::Installed { schedule, .. } => {
                        schedule.unwrap_or_else(|| "（未知）".into())
                    }
                    JobStatus::NotInstalled => "-".into(),
                };
                println!("{n:<14} {desc}");
            }
        }
        TimerAction::Run {
            name,
            all,
            allow_battery,
        } => {
            if !allow_battery && on_battery() {
                eprintln!("[{}] 电池供电，跳过（--allow-battery 可强制）", now());
                return Ok(());
            }
            if all {
                if find_libs(dest, 1).is_empty() {
                    eprintln!("[{}] 备份根无可用库（盘未挂载？），跳过。", now());
                    return Ok(());
                }
                eprintln!("[{}] 开始备份全部库", now());
                save::run(
                    app,
                    dest,
                    SaveArgs {
                        name: None,
                        all: true,
                        message: None,
                        quiet: false,
                        dry_run: false,
                    },
                )?;
            } else {
                let n = name.context("缺少库名（或使用 --all）")?;
                if locate(dest, &n).is_err() {
                    eprintln!("[{}] 备份根不可用（盘未挂载？），跳过 `{n}`。", now());
                    return Ok(());
                }
                eprintln!("[{}] 开始备份 `{n}`", now());
                save::run(
                    app,
                    dest,
                    SaveArgs {
                        name: Some(n),
                        all: false,
                        message: None,
                        quiet: false,
                        dry_run: false,
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn build_schedule(
    every: Option<String>,
    daily: Option<String>,
    on_mount: bool,
    on_volume: Option<std::path::PathBuf>,
) -> Result<Schedule> {
    if let Some(p) = on_volume {
        return Ok(Schedule::OnVolume(p));
    }
    if on_mount {
        return Ok(Schedule::OnMount);
    }
    if let Some(d) = daily {
        let (hour, minute) = platform::parse_daily(&d)?;
        return Ok(Schedule::Daily { hour, minute });
    }
    let e = every.unwrap_or_else(|| "1h".into());
    Ok(Schedule::Interval(platform::parse_duration(&e)?))
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

#[cfg(target_os = "macos")]
fn on_battery() -> bool {
    std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Battery Power"))
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn on_battery() -> bool {
    false
}
