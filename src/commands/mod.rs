pub mod new;

#[derive(clap::Subcommand)]
pub enum Command {
  New(new::NewArgs),
}
