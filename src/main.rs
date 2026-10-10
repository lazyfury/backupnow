mod cli;
mod commands;
mod config;
mod git;
mod meta;
mod platform;

use clap::Parser;
use cli::{Cli, Command};
use config::AppConfig;

fn main() {
    // 管道被提前关闭（如 `backupnow list | head`）时像普通 Unix 工具一样静默退出，
    // 而不是让 println! 触发 broken pipe panic。
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let app = AppConfig::load(cli.dest);
    let dest = app.dest.clone();
    match cli.command {
        Command::Init(a) => commands::init::run(&app, &dest, a),
        Command::List(a) => {
            let root = a.path.clone().unwrap_or_else(|| dest.clone());
            commands::list::run(&root, a)
        }
        Command::Status(a) => commands::status::run(&dest, a),
        Command::Save(a) => commands::save::run(&app, &dest, a),
        Command::Log(a) => commands::log::run(&dest, a),
        Command::Restore(a) => commands::restore::run(&dest, a),
        Command::Ignore(a) => commands::ignore::run(&dest, a),
        Command::Doctor(a) => commands::doctor::run(&app, &dest, a),
        Command::Config(a) => commands::config::run(&app, a),
        Command::Timer(a) => commands::timer::run(&app, &dest, a),
        Command::Completions(a) => commands::completions::run(a),
        Command::Man(a) => commands::man::run(a),
        Command::Open(a) => commands::open::run(&dest, a),
    }
}
