pub mod build;
pub mod clean;
pub mod doc;
pub mod fix;
pub mod fmt;
pub mod init;
pub mod new;
pub mod setup;
pub mod test;

#[derive(clap::Subcommand)]
pub enum Command {
  #[clap(visible_alias = "n")]
  New(new::NewArgs),

  #[clap(visible_alias = "b")]
  Build(build::BuildArgs),

  #[clap(visible_alias = "t")]
  Test(test::TestArgs),

  #[clap(visible_alias = "c")]
  Clean(clean::CleanArgs),

  #[clap(visible_alias = "d")]
  Doc(doc::DocArgs),

  Fmt(fmt::FmtArgs),

  Fix(fix::FixArgs),

  Init(init::InitArgs),

  Setup(setup::SetupArgs),
}
