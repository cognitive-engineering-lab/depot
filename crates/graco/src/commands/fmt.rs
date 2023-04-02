use anyhow::{Context, Result};

use crate::workspace::{package::Package, PackageCommand};

#[derive(clap::Parser)]
pub struct FmtArgs {
  #[arg(short, long)]
  check: Option<bool>,

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
    let check = self.args.check.unwrap_or(false);
    let extra = match &self.args.prettier_args {
      Some(args) => shlex::split(args).context("Failed to parse prettier args")?,
      None => Vec::new(),
    };

    pkg
      .exec("prettier", |cmd| {
        cmd.arg(if check { "-c" } else { "-w" });
        cmd.args(pkg.source_files());
        cmd.args(extra);
      })
      .await
  }
}
