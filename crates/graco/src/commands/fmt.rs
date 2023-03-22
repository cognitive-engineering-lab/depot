use anyhow::{Context, Result};

use crate::workspace::{package::Package, PackageCommand};

#[derive(clap::Parser)]
pub struct FmtArgs {
  #[arg(last = true)]
  prettier_args: Option<String>,
}

pub struct FmtCommand {
  #[allow(unused)]
  args: FmtArgs,
}

impl FmtCommand {
  pub fn new(args: FmtArgs) -> Self {
    FmtCommand { args }
  }
}

#[async_trait::async_trait]
impl PackageCommand for FmtCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    let args = match &self.args.prettier_args {
      Some(args) => shlex::split(args).context("Failed to parse prettier args")?,
      None => Vec::new(),
    };
    pkg
      .exec("prettier", |cmd| {
        cmd.args(["{src,tests}/**/*.{ts,tsx}", "-w"]);
        cmd.args(args);
      })
      .await?;

    Ok(())
  }
}
