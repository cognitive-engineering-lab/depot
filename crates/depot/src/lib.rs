use self::commands::Command;
use anyhow::{Context, Result};
use clap::Parser;
use commands::{
  build::BuildCommand,
  clean::CleanCommand,
  doc::DocCommand,
  fmt::FmtCommand,
  init::InitCommand,
  new::NewCommand,
  setup::{GlobalConfig, SetupCommand},
  test::TestCommand,
};
use workspace::{package::PackageName, Workspace};

mod commands;
mod logger;
mod utils;
mod workspace;

#[derive(clap::Parser, Default)]
pub struct CommonArgs {
  #[clap(short, long)]
  only: Option<PackageName>,

  #[clap(short, long, action)]
  watch: bool,
}

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
  #[command(subcommand)]
  command: Command,

  #[command(flatten)]
  common: CommonArgs,
}

pub async fn run() -> Result<()> {
  let Args { command, common } = Args::parse();

  let command = match command {
    Command::Setup(args) => return SetupCommand::new(args).run(),
    command => command,
  };

  let global_config =
    GlobalConfig::load().context("Depot has not been setup yet. Run `depot setup` to proceed.")?;

  let command = match command {
    Command::New(args) => return NewCommand::new(args, global_config).await.run().await,
    command => command,
  };

  let ws = Workspace::load(global_config, None, common).await?;

  // TODO: merge all tasks into a single task graph like Cargo
  let command = match command {
    Command::Init(args) => InitCommand::new(args).kind(),
    Command::Build(args) => BuildCommand::new(args).kind(),
    Command::Test(args) => TestCommand::new(args).kind(),
    Command::Fmt(args) => FmtCommand::new(args).kind(),
    Command::Clean(args) => CleanCommand::new(args).kind(),
    Command::Doc(args) => DocCommand::new(args).kind(),
    Command::Setup(..) | Command::New(..) => unreachable!(),
  };

  ws.run(command).await?;

  Ok(())
}
