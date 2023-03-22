pub mod build;
pub mod new;
pub mod setup;
pub mod test;
pub mod init;

#[derive(clap::Subcommand)]
pub enum Command {
  Setup(setup::SetupArgs),
  New(new::NewArgs),
  Init(init::InitArgs),
  Build(build::BuildArgs),
  Test(test::TestArgs)
}
