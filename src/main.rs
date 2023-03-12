use self::commands::Command;
use anyhow::Result;
use clap::Parser;

mod commands;
mod workspace;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
  #[command(subcommand)]
  command: Command,
}

fn run() -> Result<()> {
  let args = Args::parse();
  match args.command {
    Command::New(args) => commands::new::NewCommand::new(args).run()?,
  };
  Ok(())
}

fn main() {
  env_logger::init();
  if let Err(e) = run() {
    eprintln!("Graco failed with the error:\n");
    if cfg!(debug_assertions) {
      eprintln!("{e:?}");
    } else {
      eprintln!("{e}");
    }
  }
}
