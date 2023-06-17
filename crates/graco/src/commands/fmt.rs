use anyhow::{Context, Result};

use crate::workspace::{package::Package, PackageCommand};

/// Automatically format source files with prettier
#[derive(clap::Parser)]
pub struct FmtArgs {
  /// If true, don't write to files and instead fail if they aren't formatted
  #[arg(short, long, action)]
  pub check: bool,

  /// Additional arguments to pass to prettier
  #[arg(last = true)]
  pub prettier_args: Option<String>,
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
    let extra = match &self.args.prettier_args {
      Some(args) => shlex::split(args).context("Failed to parse prettier args")?,
      None => Vec::new(),
    };

    pkg
      .exec("prettier", |cmd| {
        cmd.arg(if self.args.check { "-c" } else { "-w" });
        cmd.args(pkg.source_files());
        cmd.args(extra);
      })
      .await
  }
}
