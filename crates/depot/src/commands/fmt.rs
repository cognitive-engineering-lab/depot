use anyhow::{Context, Result};

use crate::workspace::{package::Package, Command, CoreCommand, PackageCommand};

/// Format source files with biome
#[derive(clap::Parser, Debug)]
pub struct FmtArgs {
  /// If true, don't write to files and instead fail if they aren't formatted
  #[arg(short, long, action)]
  pub check: bool,

  /// Additional arguments to pass to biome
  #[arg(last = true)]
  pub biome_args: Option<String>,
}

#[derive(Debug)]
pub struct FmtCommand {
  #[allow(unused)]
  args: FmtArgs,
}

impl FmtCommand {
  pub fn new(args: FmtArgs) -> Self {
    FmtCommand { args }
  }

  pub fn kind(self) -> Command {
    Command::package(self)
  }
}

impl CoreCommand for FmtCommand {
  fn name(&self) -> String {
    "fmt".into()
  }
}

#[async_trait::async_trait]
impl PackageCommand for FmtCommand {
  async fn run_pkg(&self, pkg: &Package) -> Result<()> {
    let extra = match &self.args.biome_args {
      Some(args) => shlex::split(args).context("Failed to parse biome args")?,
      None => Vec::new(),
    };

    pkg
      .exec("biome", |cmd| {
        cmd.arg("format");
        if !self.args.check {
          cmd.arg("--write");
        }
        cmd.args(pkg.source_files());
        cmd.args(extra);
      })
      .await
  }
}
