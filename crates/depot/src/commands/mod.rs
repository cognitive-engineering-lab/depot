pub mod build;
pub mod clean;
pub mod doc;
pub mod fmt;
pub mod init;
pub mod new;
pub mod setup;
pub mod test;

#[derive(clap::Subcommand)]
pub enum Command {
  Setup(setup::SetupArgs),
  New(new::NewArgs),
  Init(init::InitArgs),
  Build(build::BuildArgs),
  Test(test::TestArgs),
  Fmt(fmt::FmtArgs),
  Clean(clean::CleanArgs),
  Doc(doc::DocArgs),
}
