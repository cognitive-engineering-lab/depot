#![warn(clippy::pedantic)]
#![allow(
  clippy::format_collect,
  clippy::similar_names,
  clippy::module_name_repetitions,
  clippy::single_match_else
)]

use self::commands::Command;
use anyhow::{Context, Result};
use clap::Parser;
use commands::{
  build::BuildCommand,
  clean::CleanCommand,
  doc::DocCommand,
  fix::FixCommand,
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
  /// Only run the command for a given package and its dependencies
  #[clap(short, long)]
  package: Option<PackageName>,

  /// Disable incremental compilation
  #[clap(long)]
  no_incremental: bool,

  /// Disable fullscreen UI
  #[clap(long)]
  no_fullscreen: bool,
}

#[derive(clap::Parser)]
#[command(name = "depot", author, version, about, long_about = None)]
struct Args {
  #[command(subcommand)]
  command: Command,

  #[command(flatten)]
  common: CommonArgs,
}

#[allow(clippy::missing_errors_doc)]
pub async fn run() -> Result<()> {
  let Args { command, common } = Args::parse();

  let command = match command {
    Command::Setup(args) => return SetupCommand::new(args).run().await,
    command => command,
  };

  let global_config =
    GlobalConfig::load().context("Depot has not been setup yet. Run `depot setup` to proceed.")?;

  let command = match command {
    Command::New(args) => return NewCommand::new(args, global_config).await.run(),
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
    Command::Fix(args) => FixCommand::new(args).kind(),
    Command::Setup(..) | Command::New(..) => unreachable!(),
  };

  ws.run(command).await?;

  Ok(())
}
