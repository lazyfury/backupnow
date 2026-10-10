use crate::cli::{Cli, CompletionsArgs};
use anyhow::Result;
use clap::CommandFactory;

pub fn run(a: CompletionsArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(a.shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}
