pub mod build;
pub mod new;
pub mod setup;

#[derive(clap::Subcommand)]
pub enum Command {
  New(new::NewArgs),
  Build(build::BuildArgs),
  Setup(setup::SetupArgs),
}
