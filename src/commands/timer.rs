use crate::cli::{SaveArgs, TimerAction, TimerArgs};
use crate::commands::{locate, save};
use crate::config::AppConfig;
use crate::platform::{self, JobSpec, JobStatus, Schedule};
use anyhow::{Context, Result};
use std::path::Path;

pub fn run(app: &AppConfig, dest: &Path, a: TimerArgs) -> Result<()> {
    match a.action {
        TimerAction::Install {
            name,
            every,
            daily,
            on_mount,
        } => {
            // 校验库存在
            let (sf, _repo) = locate(dest, &name)?;
            let schedule = build_schedule(every, daily, on_mount)?;
            let binary = std::env::current_exe().context("无法确定自身可执行文件路径")?;
            let job = JobSpec {
                name: sf.name.clone(),
                dest: dest.to_path_buf(),
                binary,
                schedule: schedule.clone(),
            };
            let sched = platform::scheduler();
            sched.install(&job)?;

            println!("✅ 已安装定时任务 `{}`（后端: {}）", sf.name, sched.platform());
            println!("   调度     : {}", schedule.describe());
            println!("   命令     : backupnow timer run {} --dest {}", sf.name, dest.display());
            if let Some(hint) = sched.log_hint(&sf.name) {
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
        TimerAction::Run { name } => {
            // 调度器入口：盘不可用时安静跳过（不算失败）
            if locate(dest, &name).is_err() {
                eprintln!(
                    "[{}] 备份根不可用（盘未挂载？），跳过 `{name}`。",
                    chrono::Local::now().to_rfc3339()
                );
                return Ok(());
            }
            eprintln!(
                "[{}] 开始备份 `{name}`",
                chrono::Local::now().to_rfc3339()
            );
            save::run(
                app,
                dest,
                SaveArgs {
                    name,
                    message: None,
                },
            )?;
        }
    }
    Ok(())
}

fn build_schedule(
    every: Option<String>,
    daily: Option<String>,
    on_mount: bool,
) -> Result<Schedule> {
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
