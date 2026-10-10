mod cli;
mod commands;
mod config;
mod git;

use clap::Parser;
use cli::{Cli, Command};
use config::AppConfig;

fn main() {
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
    }
}
