use crate::cli::{Cli, ManArgs};
use anyhow::Result;
use clap::CommandFactory;

pub fn run(a: ManArgs) -> Result<()> {
    let cmd = Cli::command();
    match a.out {
        None => {
            let man = clap_mangen::Man::new(cmd);
            man.render(&mut std::io::stdout())?;
        }
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;
            clap_mangen::generate_to(cmd, &dir)?;
            println!("已生成 man page 到 {}", dir.display());
        }
    }
    Ok(())
}
