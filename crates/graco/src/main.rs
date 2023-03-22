use self::commands::Command;
use anyhow::{Context, Result};
use clap::Parser;
use commands::{
  build::BuildCommand,
  clean::CleanCommand,
  fmt::FmtCommand,
  init::InitCommand,
  new::NewCommand,
  setup::{GlobalConfig, SetupCommand},
  test::TestCommand,
};
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
    Command::Setup(args) => return SetupCommand::new(args).run(),
    command => command,
  };

  let global_config =
    GlobalConfig::load().context("Graco has not been setup yet. Run `graco setup` to proceed.")?;

  let command = match command {
    Command::New(args) => return NewCommand::new(args, global_config).await.run().await,
    command => command,
  };

  let ws = Workspace::load(global_config, None).await?;

  match command {
    Command::Init(args) => {
      let init_cmd = InitCommand::new(args);
      ws.run(&init_cmd).await?;
    }
    Command::Build(args) => {
      let init_cmd = InitCommand::new(Default::default());
      ws.run(&init_cmd).await?;
      let build_cmd = BuildCommand::new(args);
      ws.run(&build_cmd).await?;
    }
    Command::Test(args) => {
      let init_cmd = InitCommand::new(Default::default());
      ws.run(&init_cmd).await?;
      let build_cmd = BuildCommand::new(Default::default());
      ws.run(&build_cmd).await?;
      let test_cmd = TestCommand::new(args);
      ws.run_ws(&test_cmd).await?;
    }
    Command::Fmt(args) => {
      let fmt_cmd = FmtCommand::new(args);
      ws.run(&fmt_cmd).await?;
    }
    Command::Clean(args) => {
      let clean_cmd = CleanCommand::new(args);
      ws.run(&clean_cmd).await?;
      ws.run_ws(&clean_cmd).await?;
    }
    Command::Setup(..) | Command::New(..) => unreachable!(),
  };
  Ok(())
}

#[tokio::main]
async fn main() {
  env_logger::init();
  if let Err(e) = run().await {
    eprintln!("Graco failed with the error:\n");
    if cfg!(debug_assertions) {
      eprintln!("{e:?}");
    } else {
      eprintln!("{e}");
    }
    std::process::exit(1);
  }
}
