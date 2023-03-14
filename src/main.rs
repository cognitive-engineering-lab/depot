use self::commands::Command;
use anyhow::{Context, Result};
use clap::Parser;
use commands::setup::GlobalConfig;
use workspace::Workspace;

mod commands;
mod logger;
mod utils;
mod workspace;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
  #[command(subcommand)]
  command: Command,
}

async fn run() -> Result<()> {
  let Args { command } = Args::parse();

  let command = match command {
    Command::Setup(args) => return commands::setup::SetupCommand::new(args).run(),
    command => command,
  };

  let global_config =
    GlobalConfig::load().context("Graco has not been setup yet. Run `graco setup` to proceed.")?;

  let command = match command {
    Command::New(args) => return commands::new::NewCommand::new(args, global_config).run(),
    command => command,
  };

  let ws = Workspace::load(global_config, None)?;

  match command {
    Command::Build(args) => {
      let cmd = commands::build::BuildCommand::new(args, ws.clone());
      ws.run(cmd).await?;
    }
    Command::Setup(..) | Command::New(..) => unreachable!(),
  };
  Ok(())
}

fn main() {
  env_logger::init();
  if let Err(e) = futures::executor::block_on(run()) {
    eprintln!("Graco failed with the error:\n");
    if cfg!(debug_assertions) {
      eprintln!("{e:?}");
    } else {
      eprintln!("{e}");
    }
  }
}
